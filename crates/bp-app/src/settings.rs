//! Réglages de l'application, enregistrés en JSON dans le dossier de
//! configuration du système (`~/.config/bankperfect/reglages.json` sous Linux).

use std::path::PathBuf;

use bp_core::i18n::{self, Lang};
use serde::{Deserialize, Serialize};

const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Langue de l'interface (`fr`, `en`) ; `None` : celle du système.
    pub language: Option<String>,
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
    /// Soldes en trois cartes plutôt qu'en équation.
    pub balance_cards: bool,
    /// Ligne des filtres de colonnes ouverte sous les en-têtes de la grille.
    pub show_column_filters: bool,
    /// Courbe du solde derrière la colonne Solde de la grille.
    pub balance_river: bool,
    /// Certificat de licence signé par le serveur (voir `license`).
    pub license: Option<String>,
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
    /// Catégoriser les opérations importées que les règles laissent sans
    /// catégorie (suggestions sûres seulement).
    pub categorize_on_import: bool,
    /// Pendant la saisie, remplir la catégorie quand la suggestion est sûre.
    pub categorize_in_entry: bool,
    /// Recours à l'IA (API Claude) pour les libellés inconnus, sur demande.
    pub ai_categorize: bool,
    /// Clé d'API Anthropic de l'utilisateur.
    pub ai_api_key: String,
    /// Modèle utilisé (voir `bp_core::categorize::ai::MODELS`).
    pub ai_model: String,
    /// Réglages lus sur disque : seuls ceux-là sont réenregistrés (les tests
    /// partent de réglages par défaut et n'écrivent rien).
    #[serde(skip)]
    persist: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
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
            balance_cards: false,
            show_column_filters: false,
            balance_river: true,
            license: None,
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
            categorize_on_import: true,
            categorize_in_entry: true,
            ai_categorize: false,
            ai_api_key: String::new(),
            ai_model: bp_core::categorize::ai::DEFAULT_MODEL.into(),
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
    let path = config_file(name).ok_or(bp_core::tr!("dossier de configuration introuvable"))?;
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
            let _ = bp_core::format_v6::write_replace(&path, &json);
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

    /// Langue choisie, sinon celle du système (les réglages par défaut des
    /// tests restent en français, quel que soit le système).
    pub fn language(&self) -> Lang {
        self.language
            .as_deref()
            .and_then(Lang::from_code)
            .unwrap_or_else(|| {
                if self.persist {
                    static SYSTEM: std::sync::OnceLock<Lang> = std::sync::OnceLock::new();
                    *SYSTEM.get_or_init(|| {
                        sys_locale::get_locale().map_or(Lang::Fr, |l| Lang::from_locale(&l))
                    })
                } else {
                    Lang::Fr
                }
            })
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
        use chrono::Datelike;
        let d = date.to_naive();
        match self {
            DateFormat::Long => date.to_string(),
            DateFormat::Short => d.format("%d/%m/%y").to_string(),
            DateFormat::WithDay => format!(
                "{} {}",
                i18n::weekday_abbr(d.weekday().num_days_from_monday() as usize),
                d.format("%d/%m/%y")
            ),
            // « 31 janv. 2026 », « Jan 31, 2026 ».
            DateFormat::Text if i18n::lang() == Lang::En => format!(
                "{} {}, {}",
                i18n::month_abbr_in_date(d.month0() as usize),
                d.day(),
                d.year()
            ),
            DateFormat::Text => format!(
                "{} {} {}",
                d.day(),
                i18n::month_abbr_in_date(d.month0() as usize),
                d.year()
            ),
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
    static CATEGORIZE_IMPORT: AtomicBool = AtomicBool::new(true);
    static CATEGORIZE_ENTRY: AtomicBool = AtomicBool::new(true);

    pub fn set(settings: &super::Settings) {
        bp_core::i18n::set_lang(settings.language());
        if let Ok(mut state) = STATE.lock()
            && (state.0 != settings.export_dir || state.1 != settings.date_format)
        {
            *state = (settings.export_dir.clone(), settings.date_format);
        }
        AUTOCOMPLETE.store(settings.autocomplete, Ordering::Relaxed);
        CENTS.store(settings.amounts_in_cents, Ordering::Relaxed);
        CATEGORIZE_IMPORT.store(settings.categorize_on_import, Ordering::Relaxed);
        CATEGORIZE_ENTRY.store(settings.categorize_in_entry, Ordering::Relaxed);
    }

    /// Catégoriser les opérations importées.
    pub fn categorize_on_import() -> bool {
        CATEGORIZE_IMPORT.load(Ordering::Relaxed)
    }

    /// Remplir la catégorie pendant la saisie quand la suggestion est sûre.
    pub fn categorize_in_entry() -> bool {
        CATEGORIZE_ENTRY.load(Ordering::Relaxed)
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
