//! Bloc-notes global (ex-scripts `notes.py` et `notes_auto.py`), indépendant
//! des fichiers de comptes, enregistré dans le dossier de configuration.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub title: String,
    pub text: String,
    /// Afficher la note à l'ouverture d'un fichier.
    pub show_on_open: bool,
}

fn path() -> Option<PathBuf> {
    directories::ProjectDirs::from("net", "chelly", "bankperfect")
        .map(|d| d.config_dir().join("notes.json"))
}

pub fn load() -> Vec<Note> {
    path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save(notes: &[Note]) -> std::io::Result<()> {
    let Some(path) = path() else { return Ok(()) };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(notes).map_err(std::io::Error::other)?;
    bp_core::format_v6::write_atomic(&path, &json).map_err(|e| std::io::Error::other(e.to_string()))
}
