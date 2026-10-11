//! Réglages de l'application, enregistrés en JSON dans le dossier de
//! configuration du système (`~/.config/bankperfect/reglages.json` sous Linux).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// « sombre », « clair » ou « système ».
    pub theme: String,
    /// Dossier des sauvegardes `.bp0` (défaut : dossier de données du système).
    pub backup_dir: Option<PathBuf>,
    /// Nombre de sauvegardes gardées par fichier.
    pub backup_keep: usize,
    /// Fichiers ouverts récemment, le plus récent en premier.
    pub recent_files: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "sombre".into(),
            backup_dir: None,
            backup_keep: bp_core::backup::DEFAULT_KEEP,
            recent_files: Vec::new(),
        }
    }
}

fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("net", "chelly", "bankperfect")
}

fn settings_path() -> Option<PathBuf> {
    project_dirs().map(|d| d.config_dir().join("reglages.json"))
}

impl Settings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Enregistre les réglages ; une erreur n'empêche pas de continuer.
    pub fn save(&self) {
        let Some(path) = settings_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let _ = bp_core::format_v6::write_atomic(&path, &json);
        }
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.backup_dir
            .clone()
            .or_else(|| project_dirs().map(|d| d.data_dir().join("sauvegardes")))
            .unwrap_or_else(|| PathBuf::from("sauvegardes"))
    }

    pub fn remember(&mut self, file: &std::path::Path) {
        let file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        self.recent_files.retain(|f| f != &file);
        self.recent_files.insert(0, file);
        self.recent_files.truncate(MAX_RECENT);
        self.save();
    }

    pub fn theme_preference(&self) -> egui::ThemePreference {
        match self.theme.as_str() {
            "clair" => egui::ThemePreference::Light,
            "système" => egui::ThemePreference::System,
            _ => egui::ThemePreference::Dark,
        }
    }

    pub fn set_theme(&mut self, preference: egui::ThemePreference) {
        self.theme = match preference {
            egui::ThemePreference::Light => "clair",
            egui::ThemePreference::System => "système",
            egui::ThemePreference::Dark => "sombre",
        }
        .into();
        self.save();
    }
}
