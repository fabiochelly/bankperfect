//! Plantages et échecs au démarrage : l'exécutable Windows n'a pas de
//! console, une panique ou une erreur d'eframe y fermait l'application sans
//! rien montrer. Elles sont désormais écrites dans `crash.log` (dossier de
//! configuration) et, sous Windows, affichées dans une boîte de message.

use bp_core::tr;

const LOG_FILE: &str = "crash.log";

/// Installe le relevé des paniques. La langue des messages est celle des
/// réglages, l'interface n'ayant peut-être pas encore démarré.
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
}

/// Signale l'échec du démarrage (fenêtre ou carte graphique).
pub fn startup_failed(error: &dyn std::fmt::Display) {
    let text = tr!("BankPerfect n'a pas pu démarrer : {}", error);
    let path = write_log(&text);
    show(&text, path);
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
    writeln!(
        file,
        "── {now} · BankPerfect {} ({}) ──\n{text}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    )
    .ok()?;
    Some(path)
}

/// Boîte de message sous Windows ; ailleurs, la sortie d'erreur suffit.
fn show(message: &str, log: Option<std::path::PathBuf>) {
    let mut text = message.to_string();
    if let Some(log) = log {
        text.push_str("\n\n");
        text.push_str(&tr!("Détails : {}", log.display()));
    }
    eprintln!("{text}");
    #[cfg(windows)]
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("BankPerfect")
        .set_description(text)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}
