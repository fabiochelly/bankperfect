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
    /// Thème utilisé en mode sombre (identifiant, voir `theme::THEMES`).
    pub dark_theme: String,
    /// Thème utilisé en mode clair.
    pub light_theme: String,
    /// Dossier des sauvegardes `.bp0` (défaut : dossier de données du système).
    pub backup_dir: Option<PathBuf>,
    /// Nombre de sauvegardes gardées par fichier.
    pub backup_keep: usize,
    /// Fichiers ouverts récemment, le plus récent en premier.
    pub recent_files: Vec<PathBuf>,
    /// Fond de fenêtre translucide et flouté par le compositeur (`None` :
    /// actif par défaut sous Linux seulement).
    pub translucent: Option<bool>,
    /// Verrouiller les lignes rapprochées.
    pub lock_reconciled: bool,
    pub show_balance_panel: bool,
    pub show_forecast: bool,
    /// Niveaux d'annulation.
    pub max_undo: usize,
    /// Pointage sans rapprochement : une ligne est pointée ou non.
    pub disable_reconcile: bool,
    /// Soldes calculés sur toutes les lignes, même filtrées.
    pub ignore_filter_totals: bool,
    /// Suggestions de tiers pendant la saisie.
    pub autocomplete: bool,
    /// Un montant tapé sans virgule est en centimes (1250 → 12,50).
    pub amounts_in_cents: bool,
    /// Après l'ajout d'une opération, le formulaire reste ouvert et vidé.
    pub keep_entry_open: bool,
    /// Saisie dans une barre permanente sous la grille (comme l'ancienne
    /// version) au lieu du panneau de droite.
    pub entry_at_bottom: bool,
    /// Mémoriser la colonne de tri entre deux sessions.
    pub remember_sort: bool,
    pub last_sort: Option<(String, bool)>,
    /// Enregistrer automatiquement à la fermeture, sans demander.
    pub autosave_on_close: bool,
    /// Dossier proposé pour ouvrir et créer les fichiers de comptes.
    pub files_dir: Option<PathBuf>,
    /// Dossier proposé pour les exports (CSV, QIF…).
    pub export_dir: Option<PathBuf>,
    /// Format des dates dans la grille.
    pub date_format: DateFormat,
    /// Taille du texte (zoom de l'interface).
    pub text_scale: f32,
    /// Extensions désactivées (identifiants).
    pub disabled_extensions: Vec<String>,
    /// Réglages lus sur disque : seuls ceux-là sont réenregistrés (les tests
    /// partent de réglages par défaut et n'écrivent rien).
    #[serde(skip)]
    persist: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "sombre".into(),
            dark_theme: crate::theme::DEFAULT_DARK.into(),
            light_theme: crate::theme::DEFAULT_LIGHT.into(),
            backup_dir: None,
            backup_keep: bp_core::backup::DEFAULT_KEEP,
            recent_files: Vec::new(),
            translucent: None,
            lock_reconciled: false,
            show_balance_panel: true,
            show_forecast: true,
            max_undo: 30,
            disable_reconcile: false,
            ignore_filter_totals: false,
            autocomplete: true,
            amounts_in_cents: false,
            keep_entry_open: false,
            entry_at_bottom: false,
            remember_sort: false,
            last_sort: None,
            autosave_on_close: false,
            files_dir: None,
            export_dir: None,
            date_format: DateFormat::default(),
            text_scale: 1.0,
            disabled_extensions: Vec::new(),
            persist: false,
        }
    }
}

fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("net", "chelly", "bankperfect")
}

/// Fichier `name` du dossier de configuration de l'application (profils
/// de crédit, devises…).
pub fn config_file(name: &str) -> Option<PathBuf> {
    project_dirs().map(|d| d.config_dir().join(name))
}

/// Écrit `json` dans le fichier de configuration `name`.
pub fn write_config_file(name: &str, json: &[u8]) -> Result<(), String> {
    let path = config_file(name).ok_or("dossier de configuration introuvable")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    bp_core::format_v6::write_atomic(&path, json).map_err(|e| e.to_string())
}

fn settings_path() -> Option<PathBuf> {
    project_dirs().map(|d| d.config_dir().join("reglages.json"))
}

impl Settings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
            .map(|s| Settings { persist: true, ..s })
            .unwrap_or_else(|| Settings {
                persist: true,
                ..Default::default()
            })
    }

    /// Enregistre les réglages ; une erreur n'empêche pas de continuer.
    pub fn save(&self) {
        if !self.persist {
            return;
        }
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

    pub fn translucent(&self) -> bool {
        self.translucent.unwrap_or(cfg!(target_os = "linux"))
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

/// Formats de date proposés pour la grille.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DateFormat {
    /// 31/01/2026
    #[default]
    Long,
    /// 31/01/26
    Short,
    /// sam. 31/01/26
    WithDay,
    /// 31 janv. 2026
    Text,
}

impl DateFormat {
    pub const ALL: [DateFormat; 4] = [
        DateFormat::Long,
        DateFormat::Short,
        DateFormat::WithDay,
        DateFormat::Text,
    ];

    pub fn format(self, date: bp_core::model::Date) -> String {
        const DAYS: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
        const MONTHS: [&str; 12] = [
            "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.",
            "nov.", "déc.",
        ];
        use chrono::Datelike;
        let d = date.to_naive();
        match self {
            DateFormat::Long => date.to_string(),
            DateFormat::Short => d.format("%d/%m/%y").to_string(),
            DateFormat::WithDay => format!(
                "{} {}",
                DAYS[d.weekday().num_days_from_monday() as usize],
                d.format("%d/%m/%y")
            ),
            DateFormat::Text => format!("{} {} {}", d.day(), MONTHS[d.month0() as usize], d.year()),
        }
    }
}

/// Réglages consultés partout sans les faire circuler (dossier d'export,
/// format des dates) ; recopiés depuis les réglages à chaque image.
pub mod shared {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use std::sync::atomic::{AtomicBool, Ordering};

    static STATE: Mutex<(Option<PathBuf>, super::DateFormat)> =
        Mutex::new((None, super::DateFormat::Long));
    static AUTOCOMPLETE: AtomicBool = AtomicBool::new(true);
    static CENTS: AtomicBool = AtomicBool::new(false);

    pub fn set(settings: &super::Settings) {
        if let Ok(mut state) = STATE.lock()
            && (state.0 != settings.export_dir || state.1 != settings.date_format)
        {
            *state = (settings.export_dir.clone(), settings.date_format);
        }
        AUTOCOMPLETE.store(settings.autocomplete, Ordering::Relaxed);
        CENTS.store(settings.amounts_in_cents, Ordering::Relaxed);
    }

    /// Suggestions de tiers pendant la saisie.
    pub fn autocomplete() -> bool {
        AUTOCOMPLETE.load(Ordering::Relaxed)
    }

    /// Montants tapés sans virgule lus en centimes.
    pub fn amounts_in_cents() -> bool {
        CENTS.load(Ordering::Relaxed)
    }

    /// Dossier d'export choisi, sinon celui du fichier de comptes.
    pub fn export_dir(fallback: &std::path::Path) -> PathBuf {
        STATE
            .lock()
            .ok()
            .and_then(|s| s.0.clone())
            .unwrap_or_else(|| fallback.to_path_buf())
    }

    pub fn date(date: bp_core::model::Date) -> String {
        STATE.lock().map(|s| s.1).unwrap_or_default().format(date)
    }
}
