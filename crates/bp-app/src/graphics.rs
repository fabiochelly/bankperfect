//! Moteur de rendu et carte graphique.
//!
//! Par défaut, wgpu : DirectX 12 sous Windows, Vulkan sous Linux, Metal sous
//! macOS, et OpenGL 3.3 (toujours par wgpu) si le premier échoue.
//!
//! Une carte trop ancienne pour DirectX 12 (Intel HD Graphics 2000/3000,
//! dont les pilotes Windows s'arrêtent à DirectX 10.1 et OpenGL 3.1) ne fait
//! pas échouer wgpu : Windows lui offre WARP, un rendu logiciel, et chaque
//! image est alors dessinée par le processeur, lentement sur un portable de
//! cette époque. Dans ce cas (adaptateur « CPU »), l'application se relance
//! avec glow, qui se contente d'OpenGL 2.1 et se sert donc de la carte. Si
//! glow échoue à son tour, retour à wgpu sans plus essayer glow.
//!
//! Un moteur en échec au démarrage (wgpu en OpenGL, glow) est retenu et
//! n'est plus essayé : la relance passe au suivant, jusqu'au moteur par
//! défaut, qui démarre presque toujours (WARP au pire). Sans cela, wgpu en
//! OpenGL en échec après glow ne laissait plus aucun recours, et
//! l'application refusait de démarrer à chaque lancement.
//!
//! Quand le rendu reste logiciel, l'affichage est allégé ([`software`]) :
//! ni ombres ni fond vitré, pas d'animations.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Moteur retenu pour les lancements suivants, une indication par ligne :
/// « gl » (wgpu en OpenGL) ou « glow » ; « sans-gl » et « sans-glow » quand
/// ces moteurs ont échoué, la raison de l'échec de glow suivant en
/// commentaire (« # … »). Le supprimer revient au moteur par défaut.
const MEMORY_FILE: &str = "moteur-graphique.txt";
const GL: &str = "gl";
const GLOW: &str = "glow";
const NO_GL: &str = "sans-gl";
const NO_GLOW: &str = "sans-glow";

/// Moteur de rendu demandé : « glow » ou « wgpu ». Fixé par l'utilisateur ou
/// par une relance.
const RENDERER_VAR: &str = "BP_RENDERER";
/// Posé par une relance : les variables de moteur viennent de l'application,
/// pas de l'utilisateur, et le passage à glow reste permis.
const AUTO_VAR: &str = "BP_AUTO_ENGINE";

/// Le passage à glow sur rendu logiciel est permis (choix automatique, glow
/// pas encore en échec).
static GLOW_ALLOWED: AtomicBool = AtomicBool::new(false);
/// L'adaptateur wgpu est logiciel et glow reste à essayer : `main` relance.
static SWITCH_TO_GLOW: AtomicBool = AtomicBool::new(false);
/// Rendu logiciel : affichage allégé.
static SOFTWARE: AtomicBool = AtomicBool::new(false);
/// Carte graphique et moteur retenus.
static INFO: Mutex<Option<Info>> = Mutex::new(None);
/// Raison de l'échec de glow, lue au démarrage.
static GLOW_FAILURE: Mutex<Option<String>> = Mutex::new(None);

struct Info {
    name: String,
    api: String,
    driver: String,
}

#[derive(Default)]
struct Memory {
    /// « gl » ou « glow ».
    engine: Option<String>,
    /// wgpu en OpenGL a échoué.
    no_gl: bool,
    no_glow: bool,
    /// Raison de l'échec de glow.
    reason: Option<String>,
}

fn read_memory() -> Memory {
    let Some(text) = crate::settings::config_file(MEMORY_FILE)
        .and_then(|path| std::fs::read_to_string(path).ok())
    else {
        return Memory::default();
    };
    parse_memory(&text)
}

fn parse_memory(text: &str) -> Memory {
    let mut memory = Memory::default();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(reason) = line.strip_prefix('#') {
            memory.reason = Some(reason.trim().to_string());
        } else if line.eq_ignore_ascii_case(NO_GLOW) {
            memory.no_glow = true;
        } else if line.eq_ignore_ascii_case(NO_GL) {
            memory.no_gl = true;
        } else if memory.engine.is_none() {
            memory.engine = Some(line.to_lowercase());
        }
    }
    memory
}

fn write_memory(memory: &Memory) {
    let Some(path) = crate::settings::config_file(MEMORY_FILE) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut text = String::new();
    if let Some(engine) = &memory.engine {
        text.push_str(engine);
        text.push('\n');
    }
    if memory.no_gl {
        text.push_str(NO_GL);
        text.push('\n');
    }
    if memory.no_glow {
        text.push_str(NO_GLOW);
        text.push('\n');
        if let Some(reason) = &memory.reason {
            // Une seule ligne : la raison peut en compter plusieurs.
            let reason = reason.split_whitespace().collect::<Vec<_>>().join(" ");
            text.push_str(&format!("# {reason}\n"));
        }
    }
    if text.is_empty() {
        let _ = std::fs::remove_file(path);
    } else {
        let _ = std::fs::write(path, text);
    }
}

/// Applique le moteur retenu. À appeler avant tout autre fil.
pub fn prepare() {
    let auto = std::env::var_os(AUTO_VAR).is_some();
    let user_choice = !auto
        && (std::env::var_os("WGPU_BACKEND").is_some() || std::env::var_os(RENDERER_VAR).is_some());
    let memory = read_memory();
    if !user_choice && !auto {
        match memory.engine.as_deref() {
            Some(GLOW) => set_env(RENDERER_VAR, GLOW),
            Some(backend) => set_env("WGPU_BACKEND", backend),
            None => {}
        }
    }
    GLOW_ALLOWED.store(!user_choice && !memory.no_glow, Ordering::Relaxed);
    if memory.no_glow
        && let Ok(mut failure) = GLOW_FAILURE.lock()
    {
        *failure = memory.reason;
    }
}

fn set_env(name: &str, value: &str) {
    // SAFETY : appelé au tout début de `main`, aucun autre fil n'est lancé.
    unsafe { std::env::set_var(name, value) };
}

/// Moteur de rendu à utiliser.
pub fn renderer() -> eframe::Renderer {
    if on_glow() {
        eframe::Renderer::Glow
    } else {
        eframe::Renderer::Wgpu
    }
}

/// glow en cours d'utilisation.
pub fn on_glow() -> bool {
    std::env::var(RENDERER_VAR).is_ok_and(|r| r.eq_ignore_ascii_case(GLOW))
}

/// wgpu en OpenGL (choisi ou retenu).
pub fn on_gl() -> bool {
    std::env::var("WGPU_BACKEND").is_ok_and(|b| b.eq_ignore_ascii_case(GL))
}

/// Moteur de secours après un échec au démarrage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// wgpu par défaut (DirectX 12, Vulkan).
    Default,
    /// wgpu en OpenGL.
    Gl,
    Glow,
}

/// Moteur suivant celui en échec, d'après les échecs retenus : wgpu par
/// défaut → wgpu en OpenGL → glow → wgpu par défaut. `None` quand tous ont
/// échoué. Chaque échec d'OpenGL ou de glow étant retenu, la boucle se
/// termine.
fn next_engine(memory: &Memory, on_glow: bool, on_gl: bool) -> Option<Fallback> {
    let no_gl = memory.no_gl || on_gl;
    let no_glow = memory.no_glow || on_glow;
    if on_glow || on_gl {
        // Depuis OpenGL : glow s'il reste à essayer, sinon le moteur par
        // défaut, qui n'a peut-être jamais échoué (WARP).
        if !no_glow {
            Some(Fallback::Glow)
        } else {
            Some(Fallback::Default)
        }
    } else if !no_gl {
        Some(Fallback::Gl)
    } else if !no_glow {
        Some(Fallback::Glow)
    } else {
        None
    }
}

/// Moteur de secours à essayer après un échec au démarrage.
pub fn fallback_engine() -> Option<Fallback> {
    next_engine(&read_memory(), on_glow(), on_gl())
}

/// Note la carte retenue par wgpu. Vrai si l'application doit se relancer
/// avec glow (rendu logiciel, glow pas encore essayé).
pub fn set_wgpu(info: &eframe::wgpu::AdapterInfo) -> bool {
    use eframe::wgpu::{Backend, DeviceType};
    let api = match info.backend {
        Backend::Dx12 => "DirectX 12",
        Backend::Vulkan => "Vulkan",
        Backend::Metal => "Metal",
        Backend::Gl => "OpenGL",
        _ => "?",
    };
    let software = info.device_type == DeviceType::Cpu;
    store(
        Info {
            name: info.name.clone(),
            api: api.into(),
            driver: format!("{} {}", info.driver, info.driver_info)
                .trim()
                .to_string(),
        },
        software,
    );
    let switch = software && GLOW_ALLOWED.load(Ordering::Relaxed);
    SWITCH_TO_GLOW.store(switch, Ordering::Relaxed);
    switch
}

/// Note la carte retenue par glow.
pub fn set_glow(gl: &eframe::glow::Context) {
    use eframe::glow::{self, HasContext};
    // SAFETY : contexte courant, fourni par eframe à la création de l'app.
    let (name, version) = unsafe {
        (
            gl.get_parameter_string(glow::RENDERER),
            gl.get_parameter_string(glow::VERSION),
        )
    };
    let software = is_software_gl(&name);
    store(
        Info {
            name,
            api: gl_api(&version),
            driver: version,
        },
        software,
    );
}

/// « OpenGL 3.1 » ou « OpenGL ES 3.2 », d'après la chaîne `GL_VERSION`
/// (« 3.1.0 - Build 9.17.10.4459 », « OpenGL ES 3.2 Mesa 26.2.5 »).
fn gl_api(version: &str) -> String {
    let (api, rest) = match version.strip_prefix("OpenGL ES") {
        Some(rest) => ("OpenGL ES", rest),
        None => ("OpenGL", version),
    };
    match rest.split_whitespace().next() {
        Some(number) => format!("{api} {number}"),
        None => api.to_string(),
    }
}

/// Rendus OpenGL logiciels connus (Mesa, Windows sans pilote, émulateurs).
fn is_software_gl(renderer: &str) -> bool {
    let renderer = renderer.to_lowercase();
    [
        "llvmpipe",
        "softpipe",
        "swrast",
        "swiftshader",
        "mesa offscreen",
        "gdi generic",
        "basic render",
    ]
    .iter()
    .any(|s| renderer.contains(s))
}

fn store(info: Info, software: bool) {
    SOFTWARE.store(software, Ordering::Relaxed);
    if let Ok(mut slot) = INFO.lock() {
        *slot = Some(info);
    }
}

/// Rendu logiciel (sans carte graphique) : affichage allégé.
pub fn software() -> bool {
    SOFTWARE.load(Ordering::Relaxed)
}

/// Avancement d'une animation de `duration` secondes, de 0 à 1 ; aussitôt
/// terminée en rendu logiciel, où chaque image coûte cher.
pub fn progress(elapsed: f64, duration: f64) -> f64 {
    if software() || duration <= 0.0 {
        1.0
    } else {
        (elapsed / duration).clamp(0.0, 1.0)
    }
}

/// Durée d'une animation : nulle en rendu logiciel.
pub fn duration(secs: f32) -> f32 {
    if software() { 0.0 } else { secs }
}

/// Carte, moteur et pilote, pour les relevés de plantage.
pub fn summary() -> Option<String> {
    let info = INFO.lock().ok()?;
    let info = info.as_ref()?;
    let engine = if on_glow() { ", glow" } else { "" };
    let mut text = format!(
        "{} ({}{engine}, pilote {})",
        info.name, info.api, info.driver
    );
    if software() {
        text.push_str(", rendu logiciel");
        let adapters = display_adapters();
        if !adapters.is_empty() {
            text.push_str(&format!(", adaptateurs : {}", adapters.join(", ")));
        }
    }
    Some(text)
}

/// Carte et moteur, pour « À propos ».
pub fn about() -> Option<(String, String)> {
    let info = INFO.lock().ok()?;
    let info = info.as_ref()?;
    Some((info.name.clone(), info.api.clone()))
}

/// La création de l'application a été interrompue pour passer à glow.
pub fn switch_requested() -> bool {
    SWITCH_TO_GLOW.load(Ordering::Relaxed)
}

/// Retient glow et relance l'application avec lui.
pub fn switch_to_glow() {
    let card = about().map(|(name, api)| format!("{name} ({api})"));
    crate::crash::log(&format!(
        "rendu logiciel sur {} : relance avec glow",
        card.as_deref().unwrap_or("?")
    ));
    let mut memory = read_memory();
    memory.engine = Some(GLOW.into());
    write_memory(&memory);
    relaunch(&[(RENDERER_VAR, GLOW)]);
}

/// Un moteur de secours reste à essayer après un échec au démarrage.
pub fn can_fall_back() -> bool {
    fallback_engine().is_some()
}

/// Retient l'échec du moteur actuel, passe au moteur de secours et relance
/// l'application (voir [`next_engine`]). `reason` : l'échec du moteur
/// actuel, consigné dans `crash.log` (et retenu pour glow).
pub fn fall_back(reason: &str) {
    let mut memory = read_memory();
    let from = if on_glow() {
        memory.no_glow = true;
        memory.reason = Some(reason.to_string());
        "glow"
    } else if on_gl() {
        memory.no_gl = true;
        "wgpu en OpenGL"
    } else {
        "wgpu"
    };
    let Some(next) = next_engine(&memory, on_glow(), on_gl()) else {
        write_memory(&memory);
        return;
    };
    let to = match next {
        Fallback::Default => {
            memory.engine = None;
            write_memory(&memory);
            relaunch(&[(RENDERER_VAR, "wgpu")]);
            "wgpu par défaut"
        }
        Fallback::Gl => {
            memory.engine = Some(GL.into());
            write_memory(&memory);
            relaunch(&[("WGPU_BACKEND", GL)]);
            "wgpu en OpenGL"
        }
        Fallback::Glow => {
            memory.engine = Some(GLOW.into());
            write_memory(&memory);
            relaunch(&[(RENDERER_VAR, GLOW)]);
            "glow"
        }
    };
    crate::crash::log(&format!("échec de {from} : {reason}\nrelance avec {to}"));
}

/// Raison de l'échec de glow, s'il a échoué, en clair pour « À propos ».
pub fn glow_failure() -> Option<String> {
    let reason = GLOW_FAILURE.lock().ok()?.clone()?;
    // egui refuse l'OpenGL 1.1 de Windows, seul présent sans pilote de la
    // carte graphique.
    let text = if reason.contains("opengl 2.0+") {
        bp_core::tr!("version inférieure à 2.0 (pilote de la carte graphique absent ?)").to_string()
    } else if reason.contains("no glutin configs") {
        bp_core::tr!("aucun format d'affichage OpenGL compatible").to_string()
    } else {
        reason
    };
    // Le détail complet reste dans moteur-graphique.txt et crash.log.
    Some(match text.char_indices().nth(140) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text,
    })
}

/// Adaptateurs d'affichage vus par Windows (« Intel(R) HD Graphics 3000 »,
/// « Carte graphique de base Microsoft » faute de pilote).
#[cfg(windows)]
pub fn display_adapters() -> Vec<String> {
    use windows_sys::Win32::Graphics::Gdi::{DISPLAY_DEVICEW, EnumDisplayDevicesW};
    let mut names: Vec<String> = Vec::new();
    for index in 0..16 {
        let mut device = DISPLAY_DEVICEW {
            cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        // SAFETY : `device` est initialisé avec sa taille, comme l'exige
        // l'API ; un nom de périphérique nul énumère les adaptateurs.
        if unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) } == 0 {
            break;
        }
        let len = device
            .DeviceString
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(128);
        let name = String::from_utf16_lossy(&device.DeviceString[..len])
            .trim()
            .to_string();
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

#[cfg(not(windows))]
pub fn display_adapters() -> Vec<String> {
    Vec::new()
}

/// Erreur d'eframe propre à glow (contexte OpenGL introuvable ou refusé).
pub fn is_glow_error(error: &eframe::Error) -> bool {
    matches!(
        error,
        eframe::Error::Glutin(_) | eframe::Error::NoGlutinConfigs(..) | eframe::Error::OpenGL(_)
    )
}

/// Relance l'application avec `vars`, sans attendre qu'elle se termine.
fn relaunch(vars: &[(&str, &str)]) {
    if let Ok(exe) = std::env::current_exe() {
        let mut command = std::process::Command::new(exe);
        // Seulement le moteur choisi : pas celui hérité de ce lancement.
        command
            .args(std::env::args_os().skip(1))
            .env_remove("WGPU_BACKEND")
            .env_remove(RENDERER_VAR)
            .env(AUTO_VAR, "1");
        for (name, value) in vars {
            command.env(name, value);
        }
        let _ = command.spawn();
    }
}

/// Interrompt la création de l'application pour passer à glow.
#[derive(Debug)]
pub struct SwitchToGlow;

impl std::fmt::Display for SwitchToGlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("rendu logiciel : relance avec glow")
    }
}

impl std::error::Error for SwitchToGlow {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_software_gl() {
        assert!(is_software_gl("llvmpipe (LLVM 19.1.7, 256 bits)"));
        assert!(is_software_gl("GDI Generic"));
        assert!(!is_software_gl("Intel(R) HD Graphics 3000"));
        assert!(!is_software_gl("Mesa Intel(R) Graphics (LNL)"));
    }

    #[test]
    fn reads_memory() {
        let m = parse_memory("gl\nsans-glow\n# egui_glow requires opengl 2.0+. \n");
        assert_eq!(m.engine.as_deref(), Some("gl"));
        assert!(m.no_glow);
        assert_eq!(m.reason.as_deref(), Some("egui_glow requires opengl 2.0+."));
        // Format de la 9.13.0-rc.10.
        let m = parse_memory("sans-glow\n");
        assert!(m.engine.is_none() && m.no_glow && m.reason.is_none());
        let m = parse_memory("gl\nsans-gl\nsans-glow\n");
        assert!(m.no_gl && m.no_glow);
    }

    #[test]
    fn falls_back_without_dead_end() {
        let memory = |no_gl, no_glow| Memory {
            no_gl,
            no_glow,
            ..Memory::default()
        };
        // wgpu par défaut en échec : OpenGL, puis glow, puis plus rien.
        assert_eq!(
            next_engine(&memory(false, false), false, false),
            Some(Fallback::Gl)
        );
        assert_eq!(
            next_engine(&memory(true, false), false, false),
            Some(Fallback::Glow)
        );
        assert_eq!(next_engine(&memory(true, true), false, false), None);
        // wgpu en OpenGL en échec : glow, ou, glow déjà en échec, retour au
        // moteur par défaut (l'impasse du client HD Graphics 3000).
        assert_eq!(
            next_engine(&memory(false, false), false, true),
            Some(Fallback::Glow)
        );
        assert_eq!(
            next_engine(&memory(false, true), false, true),
            Some(Fallback::Default)
        );
        // glow en échec : moteur par défaut.
        assert_eq!(
            next_engine(&memory(false, false), true, false),
            Some(Fallback::Default)
        );
        assert_eq!(
            next_engine(&memory(true, true), true, false),
            Some(Fallback::Default)
        );
    }

    #[test]
    fn names_gl_version() {
        assert_eq!(gl_api("3.1.0 - Build 9.17.10.4459"), "OpenGL 3.1.0");
        assert_eq!(gl_api("4.6 (Core Profile) Mesa 26.2.5"), "OpenGL 4.6");
        assert_eq!(gl_api("OpenGL ES 3.2 Mesa 26.2.5"), "OpenGL ES 3.2");
    }
}
