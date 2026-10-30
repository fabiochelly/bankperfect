//! Plantages et échecs au démarrage : l'exécutable Windows n'a pas de
//! console, une panique ou une erreur d'eframe y fermait l'application sans
//! rien montrer. Elles sont désormais écrites dans `crash.log` (dossier de
//! configuration) et, sous Windows, affichées dans une boîte de message.
//!
//! Restaient invisibles un plantage hors de Rust (pilote graphique, couche
//! Vulkan installée par un autre logiciel) et un démarrage bloqué avant
//! l'apparition de la fenêtre : sous Windows, le curseur tourne quelques
//! secondes puis plus rien. Le premier passe par le filtre des exceptions
//! non gérées, le second par un fil de veille.

use bp_core::tr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const LOG_FILE: &str = "crash.log";

/// Délai au-delà duquel un démarrage sans première image est tenu pour
/// bloqué : large, l'ouverture d'un gros fichier passé en argument a lieu
/// avant la première image.
const STALLED: Duration = Duration::from_secs(30);

/// Étape du démarrage en cours, citée si le démarrage se bloque.
static STEP: Mutex<&str> = Mutex::new("réglages");
/// Carte graphique et API retenues, citées dans chaque relevé.
static GPU: Mutex<Option<String>> = Mutex::new(None);
/// Première image dessinée : le fil de veille s'arrête.
static READY: AtomicBool = AtomicBool::new(false);

/// Installe le relevé des paniques, des plantages hors de Rust (Windows) et
/// du démarrage bloqué. La langue des messages est celle des réglages,
/// l'interface n'ayant peut-être pas encore démarré.
pub fn install() {
    bp_core::i18n::set_lang(crate::settings::Settings::load().language());
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let thread = std::thread::current();
        let text = format!(
            "panique dans le fil « {} » : {info}\n\n{}",
            thread.name().unwrap_or("?"),
            std::backtrace::Backtrace::force_capture()
        );
        let path = write_log(&text);
        // Seule une panique du fil principal ferme l'application.
        if thread.name() == Some("main") {
            show(
                tr!("BankPerfect s'est arrêté à la suite d'une erreur."),
                path,
            );
        }
    }));
    #[cfg(windows)]
    native::install();
    let _ = std::thread::Builder::new()
        .name("veille du démarrage".into())
        .spawn(watch_startup);
}

/// Note l'étape du démarrage en cours.
pub fn step(name: &'static str) {
    if let Ok(mut step) = STEP.lock() {
        *step = name;
    }
}

/// Note la carte graphique retenue par wgpu.
pub fn set_gpu(info: &eframe::wgpu::AdapterInfo) {
    if let Ok(mut gpu) = GPU.lock() {
        *gpu = Some(format!(
            "{} ({:?}, pilote {} {})",
            info.name, info.backend, info.driver, info.driver_info
        ));
    }
}

/// Démarrage terminé (première image, ou échec déjà signalé) : le fil de
/// veille s'arrête.
pub fn startup_done() {
    if !READY.load(Ordering::Relaxed) {
        READY.store(true, Ordering::Relaxed);
    }
}

/// Signale l'échec du démarrage (fenêtre ou carte graphique).
pub fn startup_failed(error: &dyn std::fmt::Display) {
    startup_done();
    let text = tr!("BankPerfect n'a pas pu démarrer : {}", error);
    let path = write_log(&text);
    show(&text, path);
}

/// Sans première image après `STALLED`, consigne l'étape où le démarrage
/// s'est arrêté et, sous Windows, propose de relancer avec OpenGL (ou de
/// quitter, le processus restant sinon invisible).
fn watch_startup() {
    let started = std::time::Instant::now();
    while started.elapsed() < STALLED {
        if READY.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let step = STEP.lock().map(|s| *s).unwrap_or("?");
    let text = tr!(
        "BankPerfect ne parvient pas à afficher sa fenêtre (étape : {}).",
        step
    );
    let path = write_log(&text);
    let gl = std::env::var_os("WGPU_BACKEND").is_some();
    let question = if gl {
        tr!("Quitter BankPerfect ?")
    } else {
        tr!("Relancer BankPerfect avec OpenGL ?")
    };
    if ask(&text, question, path) {
        if !gl {
            relaunch_with_gl();
        }
        std::process::exit(1);
    }
}

/// Relance l'application avec OpenGL, sans attendre qu'elle se termine.
fn relaunch_with_gl() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .env("WGPU_BACKEND", "gl")
            .spawn();
    }
}

/// Ajoute `text` daté à `crash.log` ; renvoie son chemin.
fn write_log(text: &str) -> Option<std::path::PathBuf> {
    use std::io::Write;
    let path = crate::settings::config_file(LOG_FILE)?;
    std::fs::create_dir_all(path.parent()?).ok()?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let gpu = GPU.lock().ok().and_then(|gpu| gpu.clone());
    writeln!(
        file,
        "── {now} · BankPerfect {} ({}) · carte graphique : {} ──\n{text}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        gpu.as_deref().unwrap_or("?")
    )
    .ok()?;
    Some(path)
}

/// `message` suivi du chemin du relevé.
fn with_details(message: &str, log: Option<std::path::PathBuf>) -> String {
    let mut text = message.to_string();
    if let Some(log) = log {
        text.push_str("\n\n");
        text.push_str(&tr!("Détails : {}", log.display()));
    }
    eprintln!("{text}");
    text
}

/// Boîte de message sous Windows ; ailleurs, la sortie d'erreur suffit.
fn show(message: &str, log: Option<std::path::PathBuf>) {
    let text = with_details(message, log);
    #[cfg(windows)]
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("BankPerfect")
        .set_description(text)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    #[cfg(not(windows))]
    let _ = text;
}

/// Question Oui/Non sous Windows ; ailleurs, la sortie d'erreur suffit et
/// la réponse est non.
fn ask(message: &str, question: &str, log: Option<std::path::PathBuf>) -> bool {
    let text = with_details(&format!("{message}\n\n{question}"), log);
    #[cfg(windows)]
    return rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title("BankPerfect")
        .set_description(text)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes;
    #[cfg(not(windows))]
    {
        let _ = text;
        false
    }
}

/// Plantages hors de Rust (violation d'accès dans un pilote, débordement de
/// pile) : le code de l'exception et le module fautif suffisent souvent à
/// désigner le coupable.
#[cfg(windows)]
mod native {
    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::System::Diagnostics::Debug::{
        EXCEPTION_POINTERS, SetUnhandledExceptionFilter,
    };
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        GetModuleFileNameW, GetModuleHandleExW,
    };

    /// Laisse Windows poursuivre (rapport d'erreurs, fin du processus).
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

    pub fn install() {
        // SAFETY : `filter` a la signature attendue et vit tout le programme.
        unsafe {
            SetUnhandledExceptionFilter(Some(filter));
        }
    }

    unsafe extern "system" fn filter(pointers: *const EXCEPTION_POINTERS) -> i32 {
        // SAFETY : Windows passe un pointeur valide le temps de l'appel.
        let record = unsafe { pointers.as_ref().and_then(|p| p.ExceptionRecord.as_ref()) };
        if let Some(record) = record {
            let address = record.ExceptionAddress;
            let text = format!(
                "exception {:#010x} à l'adresse {address:?} dans {}",
                record.ExceptionCode as u32,
                module(address)
            );
            let path = super::write_log(&text);
            super::show(
                bp_core::tr!("BankPerfect s'est arrêté à la suite d'une erreur."),
                path,
            );
        }
        EXCEPTION_CONTINUE_SEARCH
    }

    /// Fichier du module (exe ou DLL) qui contient `address`.
    fn module(address: *const core::ffi::c_void) -> String {
        let mut handle: HMODULE = std::ptr::null_mut();
        let mut name = [0u16; 260];
        // SAFETY : `address` sert seulement de clé de recherche ; `name` est
        // un tampon de la taille annoncée.
        let len = unsafe {
            if GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                address.cast(),
                &mut handle,
            ) == 0
            {
                return "?".into();
            }
            GetModuleFileNameW(handle, name.as_mut_ptr(), name.len() as u32)
        };
        String::from_utf16_lossy(&name[..len as usize])
    }
}
