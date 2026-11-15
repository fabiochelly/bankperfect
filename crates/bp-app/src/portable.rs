//! Mode portable : réglages, licence, extensions et sauvegardes rangés dans
//! un dossier `donnees` à côté du programme (clé USB), au lieu du dossier de
//! configuration du système (`%APPDATA%\chelly\bankperfect` sous Windows).
//!
//! Comme dans l'ancienne version, le mode est actif quand un fichier nommé
//! `portable` se trouve à côté du programme ; l'option l'y crée. Le repère ne
//! peut pas être un réglage : il faudrait lire le dossier du système pour
//! savoir qu'il ne faut pas s'en servir.

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use bp_core::tr;

/// Fichier repère, à côté du programme (même nom que dans l'ancienne version).
const MARKER: &str = "portable";
/// Dossier des données, à côté du programme.
const DATA: &str = "donnees";
/// Sous-dossier des sauvegardes (dans `donnees` comme dans le dossier de
/// données du système).
pub const BACKUPS: &str = "sauvegardes";

/// Mode portable sur ce système : sous macOS, le programme est dans un
/// paquet `.app` signé, où rien ne doit être écrit.
pub const AVAILABLE: bool = !cfg!(target_os = "macos");

/// Dossier du programme : celui de l'AppImage sous Linux (le programme
/// lui-même tourne dans une image montée en lecture seule), sinon celui de
/// l'exécutable.
pub fn app_dir() -> Option<PathBuf> {
    if let Some(image) = std::env::var_os("APPIMAGE") {
        return Path::new(&image).parent().map(Path::to_path_buf);
    }
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// Le repère est-il présent ?
pub fn is_active() -> bool {
    AVAILABLE && app_dir().is_some_and(|d| d.join(MARKER).is_file())
}

/// Mode au démarrage : les extensions et leurs données ne changent de
/// dossier qu'au redémarrage.
static AT_START: OnceLock<bool> = OnceLock::new();

/// À appeler au tout début du programme.
pub fn init() {
    AT_START.get_or_init(is_active);
}

pub fn active_at_start() -> bool {
    *AT_START.get_or_init(is_active)
}

/// Dossier des données en mode portable (`None` hors mode portable).
pub fn data_dir() -> Option<PathBuf> {
    if is_active() {
        app_dir().map(|d| d.join(DATA))
    } else {
        None
    }
}

/// Dossiers du système (configuration, données) ; le second contient les
/// sauvegardes par défaut.
fn system_dirs() -> Option<(PathBuf, PathBuf)> {
    crate::settings::system_dirs()
        .map(|d| (d.config_dir().to_path_buf(), d.data_dir().to_path_buf()))
}

/// Dossier des réglages en usage hors mode portable (pour l'affichage).
pub fn system_config_dir() -> Option<PathBuf> {
    system_dirs().map(|(config, _)| config)
}

/// Des réglages ou des sauvegardes restent dans les dossiers du système.
pub fn system_traces() -> bool {
    system_dirs().is_some_and(|(config, data)| config.exists() || data.exists())
}

/// Passe en mode portable : copie les réglages (et les sauvegardes du
/// dossier par défaut) dans `donnees`, puis crée le repère. Les fichiers du
/// système restent en place : [`wipe_system`] les efface sur demande.
pub fn enable(copy_backups: bool) -> Result<PathBuf, String> {
    let app = app_dir().ok_or(tr!("dossier du programme introuvable"))?;
    let target = app.join(DATA);
    if let Err(e) = std::fs::create_dir_all(&target) {
        return Err(not_writable(&app, &e));
    }
    if let Some((config, data)) = system_dirs() {
        copy_tree(&config, &target)?;
        if copy_backups {
            copy_tree(&data.join(BACKUPS), &target.join(BACKUPS))?;
        }
    }
    std::fs::write(app.join(MARKER), b"").map_err(|e| not_writable(&app, &e))?;
    // Réenregistrés depuis leur nouvel emplacement : chemins relatifs au
    // programme, qui suivent la clé quelle que soit sa lettre de lecteur.
    crate::settings::Settings::load().save();
    Ok(target)
}

/// Quitte le mode portable : recopie `donnees` dans les dossiers du
/// système, puis retire le repère. `donnees` reste sur la clé.
pub fn disable() -> Result<(), String> {
    let app = app_dir().ok_or(tr!("dossier du programme introuvable"))?;
    let source = app.join(DATA);
    let (config, data) = system_dirs().ok_or(tr!("dossier de configuration introuvable"))?;
    // Chemins absolus avant de quitter le dossier du programme.
    let settings = crate::settings::Settings::load();
    copy_tree_except(&source, &config, BACKUPS)?;
    copy_tree(&source.join(BACKUPS), &data.join(BACKUPS))?;
    std::fs::remove_file(app.join(MARKER)).map_err(|e| e.to_string())?;
    settings.save();
    Ok(())
}

/// Efface les réglages et sauvegardes laissés dans les dossiers du système
/// (après passage en mode portable et redémarrage).
pub fn wipe_system() -> Result<(), String> {
    if !is_active() {
        return Err(tr!("Le mode portable n'est pas actif").into());
    }
    let (config, data) = system_dirs().ok_or(tr!("dossier de configuration introuvable"))?;
    for dir in [&config, &data] {
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        // Parents vides (`chelly\bankperfect` sous Windows).
        let mut parent = dir.parent();
        for _ in 0..2 {
            let Some(p) = parent else { break };
            if std::fs::remove_dir(p).is_err() {
                break;
            }
            parent = p.parent();
        }
    }
    Ok(())
}

fn not_writable(app: &Path, error: &std::io::Error) -> String {
    if app.components().any(|c| c.as_os_str() == "WindowsApps") {
        return tr!(
            "La version Microsoft Store ne peut pas être portable : utilisez la version .zip du site, copiée sur la clé."
        )
        .into();
    }
    tr!(
        "Impossible d'écrire à côté du programme ({}) : {}. Copiez bankperfect.exe sur la clé USB et lancez-le depuis la clé.",
        app.display(),
        error
    )
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    copy_tree_except(from, to, "")
}

/// Copie récursive de `from` dans `to` (fichiers remplacés), sauf l'entrée
/// de premier niveau `skip`. Rien à faire si `from` n'existe pas.
fn copy_tree_except(from: &Path, to: &Path, skip: &str) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(from) else {
        return Ok(());
    };
    let error = |p: &Path, e: std::io::Error| format!("{} : {e}", p.display());
    std::fs::create_dir_all(to).map_err(|e| error(to, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| error(from, e))?;
        let name = entry.file_name();
        if !skip.is_empty() && name == skip {
            continue;
        }
        let (source, target) = (entry.path(), to.join(&name));
        if entry.file_type().map_err(|e| error(&source, e))?.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            std::fs::copy(&source, &target).map_err(|e| error(&source, e))?;
        }
    }
    Ok(())
}

/// Chemin enregistré dans les réglages portables : relatif au dossier du
/// programme quand il est sur le même lecteur (la clé change de lettre d'un
/// ordinateur à l'autre), absolu sinon.
pub fn stored_path(path: &Path) -> PathBuf {
    match app_dir() {
        Some(base) if is_active() && same_volume(&base, path) => {
            relative_to(&base, path).unwrap_or_else(|| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    }
}

/// Sous Unix, la racine `/` est commune à tous les disques : un chemin n'est
/// rendu relatif que s'il est sur le même système de fichiers que le
/// programme (la clé). Sous Windows, la lettre de lecteur suffit.
#[cfg(unix)]
fn same_volume(base: &Path, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let device = |p: &Path| {
        p.ancestors()
            .find_map(|a| std::fs::metadata(a).ok())
            .map(|m| m.dev())
    };
    device(base).is_some() && device(base) == device(path)
}

#[cfg(not(unix))]
fn same_volume(_: &Path, _: &Path) -> bool {
    true
}

/// Inverse de [`stored_path`] : un chemin relatif part du dossier du programme.
pub fn loaded_path(path: &Path) -> PathBuf {
    match app_dir() {
        Some(base) if path.is_relative() && is_active() => normalize(&base.join(path)),
        _ => path.to_path_buf(),
    }
}

/// Composants comparables : `\\?\E:` (chemin canonique Windows) vaut `E:`,
/// la casse est ignorée sous Windows.
fn key(component: Component) -> String {
    use std::path::Prefix;
    let text = match component {
        Component::Prefix(p) => match p.kind() {
            Prefix::VerbatimDisk(d) | Prefix::Disk(d) => format!("{}:", d as char),
            _ => p.as_os_str().to_string_lossy().into_owned(),
        },
        c => c.as_os_str().to_string_lossy().into_owned(),
    };
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// `path` relatif à `base`, s'ils ont la même racine (même lecteur).
fn relative_to(base: &Path, path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let base: Vec<Component> = base.components().collect();
    let path: Vec<Component> = path.components().collect();
    // Racine (lecteur) différente : chemin absolu gardé.
    let roots = |c: &[Component]| {
        c.iter()
            .take_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
            .map(|c| key(*c))
            .collect::<Vec<_>>()
    };
    if roots(&base) != roots(&path) {
        return None;
    }
    let common = base
        .iter()
        .zip(&path)
        .take_while(|(a, b)| key(**a) == key(**b))
        .count();
    let mut relative = PathBuf::new();
    for _ in common..base.len() {
        relative.push("..");
    }
    for c in &path[common..] {
        relative.push(c.as_os_str());
    }
    Some(relative)
}

/// Retire les `.` et `..` d'un chemin sans toucher au disque.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            c => out.push(c.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn relative_paths_follow_the_program_folder() {
        let base = Path::new("/media/cle/BankPerfect");
        let file = Path::new("/media/cle/Comptes/perso.bp");
        let rel = relative_to(base, file).unwrap();
        assert_eq!(rel, Path::new("../Comptes/perso.bp"));
        assert_eq!(normalize(&base.join(&rel)), file);
        assert_eq!(
            relative_to(
                base,
                Path::new("/media/cle/BankPerfect/donnees/sauvegardes")
            )
            .unwrap(),
            Path::new("donnees/sauvegardes")
        );
        assert_eq!(relative_to(base, Path::new("relatif.bp")), None);
    }

    #[cfg(windows)]
    #[test]
    fn relative_paths_stay_on_the_same_drive() {
        let base = Path::new(r"E:\BankPerfect");
        assert_eq!(
            relative_to(base, Path::new(r"\\?\e:\Comptes\perso.bp")).unwrap(),
            Path::new(r"..\Comptes\perso.bp")
        );
        assert_eq!(relative_to(base, Path::new(r"C:\Users\x\perso.bp")), None);
    }

    #[test]
    fn copies_a_tree_except_one_entry() {
        let root = std::env::temp_dir().join(format!("bp-portable-{}", std::process::id()));
        let (from, to) = (root.join("de"), root.join("vers"));
        std::fs::create_dir_all(from.join("extensions/demo")).unwrap();
        std::fs::create_dir_all(from.join(BACKUPS)).unwrap();
        std::fs::write(from.join("reglages.json"), b"{}").unwrap();
        std::fs::write(from.join("extensions/demo/main.luau"), b"--").unwrap();
        std::fs::write(from.join(BACKUPS).join("a.bp0"), b"x").unwrap();
        copy_tree_except(&from, &to, BACKUPS).unwrap();
        assert!(to.join("reglages.json").is_file());
        assert!(to.join("extensions/demo/main.luau").is_file());
        assert!(!to.join(BACKUPS).exists());
        copy_tree(&root.join("absent"), &root.join("rien")).unwrap();
        assert!(!root.join("rien").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
