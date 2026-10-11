//! Registre des commandes de l'application : libellé, icône, groupe,
//! raccourci et mots-clés. Les menus et la palette de commandes s'en servent,
//! l'exécution est dans `BankPerfectApp::execute`.

use egui::ThemePreference;
use egui_phosphor::regular as icon;

use crate::budget_ui::BudgetTab;
use crate::document::OpenDoc;
use crate::manage_ui::Tab;
use crate::reports_ui::ReportTab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Open,
    OpenRecent(usize),
    NewFile,
    CloseFile,
    Save,
    SaveAs,
    SaveCopy,
    DeleteFile,
    ExportCsv,
    ExportXlsx,
    Report(ReportTab),
    Charts,
    UpdateQuotes,
    Transfer,
    ConvertToTransfer,
    SplitOperation,
    GoToSister,
    Budget(BudgetTab),
    ExportQif,
    Print,
    Import,
    Protection,
    Backups,
    Quit,
    Undo,
    Redo,
    NewOperation,
    FocusEntry,
    ModifySelected,
    EditOperation,
    Delete,
    CutLines,
    CopyLines,
    PasteLines,
    Replace,
    AddToPayees,
    SetMarkDate,
    ToggleLock,
    ToggleMarkDates,
    ToggleBalancePanel,
    ToggleForecast,
    ReverseCurrencies,
    Scheduler,
    Forecast,
    Rules,
    AutoFilters,
    SaveFilter,
    ApplyFilter(usize),
    ClearFilters,
    AddToScheduler,
    ToggleHideOld,
    ToggleTranslucent,
    Theme(ThemePreference),
    NextAccount,
    PreviousAccount,
    SelectAccount(usize),
    Manage(Tab),
    Tack,
    ReconcileAll,
    ModifyGroup,
    ExtraFields,
    Notes,
    Calculator,
    Loan,
    Currencies,
    Palette,
    Options,
    About,
    Help,
}

pub struct Info {
    pub group: &'static str,
    pub label: String,
    pub icon: &'static str,
    pub shortcut: Option<&'static str>,
    /// Synonymes trouvés par la recherche, sans être affichés.
    pub keywords: &'static str,
    pub enabled: bool,
}

impl Command {
    /// Commandes proposées, dans l'ordre des menus.
    pub fn all(doc: Option<&OpenDoc>) -> Vec<Command> {
        use Command::*;
        let mut list = vec![NewFile, Open];
        list.extend((0..recent_count()).map(OpenRecent));
        if let Some(doc) = doc {
            list.extend([
                Save,
                SaveAs,
                SaveCopy,
                CloseFile,
                Import,
                ExportCsv,
                ExportQif,
                Print,
                Protection,
                Backups,
                Undo,
                Redo,
                NewOperation,
                EditOperation,
                ModifySelected,
                FocusEntry,
                Delete,
                CutLines,
                CopyLines,
                PasteLines,
                Replace,
                Tack,
                SetMarkDate,
                ReconcileAll,
                ToggleLock,
                ModifyGroup,
                ExtraFields,
                AddToPayees,
                ToggleHideOld,
                ToggleMarkDates,
                ToggleBalancePanel,
                ToggleForecast,
                ReverseCurrencies,
                Scheduler,
                Forecast,
                Rules,
                ExportXlsx,
                Report(ReportTab::Accounts),
                Report(ReportTab::Categories),
                Report(ReportTab::Payees),
                Report(ReportTab::Statement),
                Charts,
                UpdateQuotes,
                Transfer,
                ConvertToTransfer,
                SplitOperation,
                GoToSister,
                Budget(BudgetTab::Monthly),
                Budget(BudgetTab::Annual),
                AutoFilters,
                SaveFilter,
                ClearFilters,
                AddToScheduler,
                NextAccount,
                PreviousAccount,
            ]);
            list.extend(
                doc.document
                    .account_order
                    .iter()
                    .filter(|&&i| !doc.document.accounts[i].disabled)
                    .map(|&i| SelectAccount(i)),
            );
            list.extend((0..filter_count()).map(ApplyFilter));
            list.extend([
                Manage(Tab::Accounts),
                Manage(Tab::Categories),
                Manage(Tab::Payees),
                Manage(Tab::Modes),
            ]);
        }
        list.extend([
            DeleteFile,
            Notes,
            Calculator,
            Loan,
            Currencies,
            Theme(ThemePreference::Dark),
            Theme(ThemePreference::Light),
            Theme(ThemePreference::System),
            ToggleTranslucent,
            Options,
            Help,
            About,
            Quit,
        ]);
        list
    }

    pub fn info(self, doc: Option<&OpenDoc>) -> Info {
        use Command::*;
        let has_doc = doc.is_some();
        let selection = doc.map_or(0, |d| d.selection.len());
        let info = |group, label: &str, icon, shortcut, keywords| Info {
            group,
            label: label.to_string(),
            icon,
            shortcut,
            keywords,
            enabled: true,
        };
        let mut result = match self {
            OpenRecent(i) => {
                let name = recent_name(i);
                info(
                    "Fichier",
                    &format!("Rouvrir {name}"),
                    icon::CLOCK,
                    None,
                    "récent dernier",
                )
            }
            NewFile => info(
                "Fichier",
                "Nouveau fichier de comptes…",
                icon::FILE_PLUS,
                Some("Ctrl+N"),
                "créer groupe",
            ),
            CloseFile => info(
                "Fichier",
                "Fermer le fichier",
                icon::X_SQUARE,
                Some("Ctrl+W"),
                "",
            ),
            SaveAs => info(
                "Fichier",
                "Enregistrer sous…",
                icon::FLOPPY_DISK_BACK,
                Some("F12"),
                "copie renommer",
            ),
            SaveCopy => info(
                "Fichier",
                "Enregistrer une copie…",
                icon::USB,
                None,
                "disque externe clé usb sauvegarde",
            ),
            DeleteFile => info(
                "Fichier",
                "Supprimer un fichier de comptes…",
                icon::TRASH_SIMPLE,
                None,
                "corbeille effacer",
            ),
            Open => info(
                "Fichier",
                "Ouvrir un fichier…",
                icon::FOLDER_OPEN,
                Some("Ctrl+O"),
                "charger",
            ),
            Save => info(
                "Fichier",
                "Enregistrer",
                icon::FLOPPY_DISK,
                Some("Ctrl+S"),
                "sauver sauvegarder",
            ),
            Import => info(
                "Fichier",
                "Importer des opérations…",
                icon::DOWNLOAD_SIMPLE,
                Some("Ctrl+I"),
                "qif ofx csv banque relevé télécharger",
            ),
            Print => info(
                "Fichier",
                "Imprimer le relevé…",
                icon::PRINTER,
                Some("Ctrl+P"),
                "pdf aperçu relevé impression",
            ),
            ExportCsv => info(
                "Fichier",
                "Exporter en CSV…",
                icon::FILE_CSV,
                None,
                "tableur excel",
            ),
            ExportXlsx => info(
                "Fichier",
                "Exporter en tableur (.xlsx)…",
                icon::MICROSOFT_EXCEL_LOGO,
                None,
                "excel calc libreoffice classeur lignes affichées",
            ),
            Report(tab) => {
                let (label, glyph, shortcut, keywords) = match tab {
                    ReportTab::Accounts => {
                        ("Total des comptes", icon::BANK, None, "soldes tous comptes")
                    }
                    ReportTab::Categories => (
                        "Totaux par catégorie",
                        icon::TAG,
                        Some("Ctrl+T"),
                        "tableau croisé dépenses recettes mois année",
                    ),
                    ReportTab::Payees => (
                        "Totaux par tiers",
                        icon::ADDRESS_BOOK,
                        None,
                        "tableau croisé bénéficiaires",
                    ),
                    ReportTab::Statement => (
                        "Relevé multicompte",
                        icon::LIST_BULLETS,
                        None,
                        "plusieurs comptes ensemble",
                    ),
                };
                info("Rapports", label, glyph, shortcut, keywords)
            }
            UpdateQuotes => info(
                "Comptes",
                "Mettre à jour les cotations…",
                icon::TREND_UP,
                None,
                "titres bourse actions isin cours portefeuille",
            ),
            Charts => info(
                "Rapports",
                "Graphiques…",
                icon::CHART_BAR,
                Some("Ctrl+Maj+G"),
                "camembert anneau barres courbe répartition évolution",
            ),
            Budget(BudgetTab::Monthly) => info(
                "Rapports",
                "Budget mensuel…",
                icon::PIGGY_BANK,
                Some("Ctrl+B"),
                "prévu réel catégories dépenses",
            ),
            Budget(BudgetTab::Annual) => info(
                "Rapports",
                "Budgets annuels…",
                icon::CALENDAR,
                None,
                "prévisions année tableau mois",
            ),
            ExportQif => info(
                "Fichier",
                "Exporter en QIF…",
                icon::EXPORT,
                None,
                "quicken money",
            ),
            Protection => info(
                "Fichier",
                "Protection du fichier…",
                icon::SHIELD_CHECK,
                None,
                "mot de passe chiffrement sécurité clé de secours",
            ),
            Backups => info(
                "Fichier",
                "Sauvegardes…",
                icon::CLOCK_COUNTER_CLOCKWISE,
                None,
                "restaurer bp0 historique",
            ),
            Quit => info("Fichier", "Quitter", icon::SIGN_OUT, None, "fermer sortir"),
            Undo => {
                let label = doc
                    .and_then(|d| d.undo_label())
                    .map_or("Annuler".to_string(), |l| format!("Annuler {l}"));
                let mut i = info(
                    "Édition",
                    &label,
                    icon::ARROW_U_UP_LEFT,
                    Some("Ctrl+Z"),
                    "défaire",
                );
                i.enabled = doc.is_some_and(|d| d.undo_label().is_some());
                i
            }
            Redo => {
                let label = doc
                    .and_then(|d| d.redo_label())
                    .map_or("Refaire".to_string(), |l| format!("Refaire {l}"));
                let mut i = info(
                    "Édition",
                    &label,
                    icon::ARROW_U_UP_RIGHT,
                    Some("Ctrl+Y"),
                    "rétablir",
                );
                i.enabled = doc.is_some_and(|d| d.redo_label().is_some());
                i
            }
            NewOperation => info(
                "Édition",
                "Nouvelle opération",
                icon::PLUS,
                Some("Inser"),
                "ajouter saisir ligne",
            ),
            EditOperation => {
                let mut i = info(
                    "Édition",
                    "Modifier l'opération",
                    icon::PENCIL_SIMPLE,
                    Some("Entrée"),
                    "éditer ligne",
                );
                i.enabled = selection == 1;
                i
            }
            FocusEntry => info(
                "Édition",
                "Aller à la saisie",
                icon::CURSOR_TEXT,
                Some("F8"),
                "champ date formulaire saisir",
            ),
            ModifySelected => {
                let mut i = info(
                    "Édition",
                    "Modifier la ligne sélectionnée",
                    icon::PENCIL_SIMPLE_LINE,
                    Some("Maj+Entrée"),
                    "éditer enregistrer modification",
                );
                i.enabled = selection == 1;
                i
            }
            Delete => {
                let mut i = info(
                    "Édition",
                    "Supprimer les opérations",
                    icon::TRASH,
                    Some("Suppr"),
                    "effacer lignes",
                );
                i.enabled = selection > 0;
                i
            }
            CutLines => {
                let mut i = info(
                    "Édition",
                    "Couper",
                    icon::SCISSORS,
                    Some("Ctrl+X"),
                    "déplacer lignes",
                );
                i.enabled = selection > 0;
                i
            }
            CopyLines => {
                let mut i = info(
                    "Édition",
                    "Copier",
                    icon::COPY,
                    Some("Ctrl+C"),
                    "dupliquer lignes",
                );
                i.enabled = selection > 0;
                i
            }
            PasteLines => {
                let n = clipboard_len();
                let label = match n {
                    0 | 1 => "Coller".to_string(),
                    n => format!("Coller {n} lignes"),
                };
                let mut i = info(
                    "Édition",
                    &label,
                    icon::CLIPBOARD_TEXT,
                    Some("Ctrl+V"),
                    "lignes copiées",
                );
                i.enabled = n > 0;
                i
            }
            AddToPayees => {
                let mut i = info(
                    "Opérations",
                    "Ajouter aux tiers mémorisés…",
                    icon::USER_PLUS,
                    None,
                    "tiers créer mémoriser",
                );
                i.enabled = selection == 1;
                i
            }
            SetMarkDate => {
                let mut i = info(
                    "Opérations",
                    "Date de pointage…",
                    icon::CALENDAR_CHECK,
                    Some("Ctrl+Maj+P"),
                    "pointage date",
                );
                i.enabled = selection > 0;
                i
            }
            ToggleLock => {
                let locked = doc.is_some_and(|d| d.locked);
                let label = if locked {
                    "Déverrouiller les lignes rapprochées"
                } else {
                    "Verrouiller les lignes rapprochées"
                };
                info(
                    "Opérations",
                    label,
                    icon::LOCK,
                    None,
                    "verrou protéger rapprochement",
                )
            }
            ToggleMarkDates => {
                let on = doc.is_some_and(|d| d.show_mark_dates);
                let label = if on {
                    "Masquer les dates de pointage"
                } else {
                    "Afficher les dates de pointage"
                };
                info(
                    "Affichage",
                    label,
                    icon::CALENDAR_CHECK,
                    Some("F11"),
                    "pointage colonne date",
                )
            }
            ToggleBalancePanel => {
                let on = doc.is_none_or(|d| d.show_balance_panel);
                let label = if on {
                    "Masquer le panneau des soldes"
                } else {
                    "Afficher le panneau des soldes"
                };
                info(
                    "Affichage",
                    label,
                    icon::SQUARES_FOUR,
                    None,
                    "soldes blocs pointé à venir",
                )
            }
            ToggleForecast => {
                let on = doc.is_none_or(|d| d.show_forecast);
                let label = if on {
                    "Masquer le solde prévisionnel"
                } else {
                    "Afficher le solde prévisionnel"
                };
                info(
                    "Affichage",
                    label,
                    icon::CHART_LINE,
                    None,
                    "prévision futur",
                )
            }
            Rules => info(
                "Opérations",
                "Règles automatiques…",
                icon::MAGIC_WAND,
                Some("Ctrl+F"),
                "calcul automatique des champs catégoriser",
            ),
            AutoFilters => info(
                "Affichage",
                "Filtres prédéfinis…",
                icon::FUNNEL,
                Some("F7"),
                "filtrer période mois enregistrés",
            ),
            SaveFilter => info(
                "Affichage",
                "Enregistrer le filtre actuel…",
                icon::FUNNEL_SIMPLE,
                None,
                "mémoriser filtre",
            ),
            ApplyFilter(i) => {
                let name = filter_name(i);
                info(
                    "Affichage",
                    &format!("Filtrer : {name}"),
                    icon::FUNNEL_SIMPLE,
                    None,
                    "filtre prédéfini",
                )
            }
            ClearFilters => {
                let mut i = info(
                    "Affichage",
                    "Retirer tous les filtres",
                    icon::FUNNEL_X,
                    None,
                    "effacer réinitialiser",
                );
                i.enabled = doc.is_some_and(|d| d.filters.is_active());
                i
            }
            Scheduler => info(
                "Comptes",
                "Échéancier…",
                icon::CALENDAR_DOTS,
                Some("Ctrl+E"),
                "échéances prélèvements récurrents abonnements",
            ),
            Forecast => info(
                "Comptes",
                "Solde prévisionnel…",
                icon::CHART_LINE_UP,
                None,
                "prévision futur échéances",
            ),
            Transfer => {
                let mut i = info(
                    "Opérations",
                    "Transfert de compte à compte…",
                    icon::ARROWS_LEFT_RIGHT,
                    Some("Ctrl+Maj+T"),
                    "virement entre comptes épargne",
                );
                i.enabled = doc.is_some_and(|d| {
                    d.document.accounts.iter().filter(|a| !a.disabled).count() > 1
                });
                i
            }
            ConvertToTransfer => {
                let mut i = info(
                    "Opérations",
                    "Convertir en transfert…",
                    icon::LINK,
                    None,
                    "lier virement autre compte",
                );
                i.enabled = selection == 1
                    && doc.is_some_and(|d| {
                        d.selection.iter().next().is_some_and(|&k| {
                            let op = d.operation(k);
                            bp_core::split::reciprocal_mode(&op.mode).is_some()
                                && bp_core::split::transfer_direction(&op.payee).is_none()
                        }) && d.document.accounts.iter().filter(|a| !a.disabled).count() > 1
                    });
                i
            }
            SplitOperation => {
                let mut i = info(
                    "Opérations",
                    "Ventiler l'opération…",
                    icon::ARROWS_SPLIT,
                    Some("F6"),
                    "répartir plusieurs catégories parts",
                );
                i.enabled = selection == 1;
                i
            }
            GoToSister => {
                let mut i = info(
                    "Opérations",
                    "Aller à la ligne liée",
                    icon::ARROW_SQUARE_OUT,
                    None,
                    "transfert sœur autre compte",
                );
                i.enabled = selection == 1
                    && doc.is_some_and(|d| {
                        d.selection
                            .iter()
                            .next()
                            .is_some_and(|&k| d.sister_of(k).is_some())
                    });
                i
            }
            AddToScheduler => {
                let mut i = info(
                    "Opérations",
                    "Ajouter à l'échéancier…",
                    icon::CALENDAR_PLUS,
                    None,
                    "récurrent mensuel échéance",
                );
                i.enabled = selection == 1;
                i
            }
            ReverseCurrencies => info(
                "Outils",
                "Inverser les monnaies du compte…",
                icon::ARROWS_LEFT_RIGHT,
                None,
                "devise convertir euro franc",
            ),
            Replace => info(
                "Édition",
                "Remplacer…",
                icon::SWAP,
                Some("Ctrl+R"),
                "rechercher texte tiers détail",
            ),
            ToggleHideOld => {
                let hidden = doc.is_some_and(|d| d.filters.hide_old_lines);
                let label = if hidden {
                    "Afficher les lignes inutiles"
                } else {
                    "Cacher les lignes inutiles"
                };
                info(
                    "Affichage",
                    label,
                    icon::EYE_SLASH,
                    Some("Ctrl+H"),
                    "masquer anciennes rapprochées",
                )
            }
            ToggleTranslucent => {
                let label = if crate::theme::translucent() {
                    "Fond opaque"
                } else {
                    "Fond translucide (flou du bureau)"
                };
                info(
                    "Affichage",
                    label,
                    icon::DROP_HALF,
                    None,
                    "transparent vitré verre flou",
                )
            }
            Theme(ThemePreference::Dark) => info(
                "Affichage",
                "Thème : Tokyo Night",
                icon::MOON,
                None,
                "sombre nuit couleurs",
            ),
            Theme(ThemePreference::Light) => info(
                "Affichage",
                "Thème : Tokyo Night Day",
                icon::SUN,
                None,
                "clair jour couleurs",
            ),
            Theme(ThemePreference::System) => info(
                "Affichage",
                "Thème : suivre le système",
                icon::DESKTOP,
                None,
                "automatique couleurs",
            ),
            NextAccount => info(
                "Comptes",
                "Compte suivant",
                icon::ARROW_RIGHT,
                Some("Ctrl+Tab"),
                "",
            ),
            PreviousAccount => info(
                "Comptes",
                "Compte précédent",
                icon::ARROW_LEFT,
                Some("Ctrl+Maj+Tab"),
                "",
            ),
            SelectAccount(i) => {
                let name = doc
                    .and_then(|d| d.document.accounts.get(i))
                    .map_or(String::new(), |a| a.name.clone());
                info(
                    "Comptes",
                    &format!("Aller au compte {name}"),
                    icon::BANK,
                    None,
                    "ouvrir afficher",
                )
            }
            Manage(Tab::Accounts) => info(
                "Comptes",
                "Gestion des comptes…",
                icon::BANK,
                Some("Ctrl+G"),
                "créer supprimer renommer iban rib banque",
            ),
            Manage(Tab::Categories) => info(
                "Comptes",
                "Catégories…",
                icon::TAG,
                None,
                "classement créer renommer",
            ),
            Manage(Tab::Payees) => info(
                "Comptes",
                "Tiers mémorisés…",
                icon::ADDRESS_BOOK,
                Some("F5"),
                "bénéficiaires",
            ),
            Manage(Tab::Modes) => info(
                "Comptes",
                "Modes de paiement…",
                icon::CREDIT_CARD,
                None,
                "carte chèque virement",
            ),
            Tack => {
                let mut i = info(
                    "Opérations",
                    "Pointer la sélection",
                    icon::CHECK,
                    Some("Espace"),
                    "cocher marquer",
                );
                i.enabled = selection > 0;
                i
            }
            ReconcileAll => info(
                "Opérations",
                "Rapprocher toutes les lignes pointées",
                icon::CHECKS,
                None,
                "relevé banque",
            ),
            ModifyGroup => {
                let mut i = info(
                    "Opérations",
                    "Modifier le groupe…",
                    icon::PENCIL_SIMPLE_LINE,
                    Some("Ctrl+M"),
                    "plusieurs lignes sélection",
                );
                i.enabled = selection > 0;
                i
            }
            ExtraFields => {
                let mut i = info(
                    "Opérations",
                    "Champs supplémentaires…",
                    icon::LIST_PLUS,
                    Some("F3"),
                    "valeurs",
                );
                i.enabled = selection == 1;
                i
            }
            Notes => info(
                "Outils",
                "Notes",
                icon::NOTE,
                Some("Ctrl+Maj+N"),
                "pense-bête mémo",
            ),
            Calculator => info(
                "Outils",
                "Calculatrice",
                icon::CALCULATOR,
                Some("Ctrl+Maj+C"),
                "calcul",
            ),
            Loan => info(
                "Outils",
                "Simulateur de crédit…",
                icon::HOUSE_LINE,
                Some("Ctrl+L"),
                "emprunt prêt immobilier mensualité taux amortissement",
            ),
            Currencies => info(
                "Outils",
                "Convertisseur de devises…",
                icon::CURRENCY_CIRCLE_DOLLAR,
                Some("Ctrl+D"),
                "change cours bce dollar livre franc suisse",
            ),
            Options => info(
                "Outils",
                "Options…",
                icon::SLIDERS,
                Some("Ctrl+,"),
                "préférences réglages paramètres",
            ),
            About => info(
                "Aide",
                "À propos de BankPerfect",
                icon::INFO,
                None,
                "version licence",
            ),
            Help => info(
                "Aide",
                "Aide en ligne",
                icon::QUESTION,
                None,
                "manuel documentation",
            ),
            Palette => info(
                "Outils",
                "Palette de commandes",
                icon::COMMAND,
                Some("Ctrl+K"),
                "rechercher",
            ),
        };
        if !has_doc
            && !matches!(
                self,
                Open | OpenRecent(_)
                    | NewFile
                    | DeleteFile
                    | Quit
                    | Notes
                    | Calculator
                    | Theme(_)
                    | ToggleTranslucent
                    | Palette
                    | Options
                    | About
                    | Help
            )
        {
            result.enabled = false;
        }
        result
    }
}

/// Fichiers récents (copie des réglages, tenue à jour par l'application).
static RECENT: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());

pub fn set_recent(files: &[std::path::PathBuf]) {
    if let Ok(mut recent) = RECENT.lock()
        && recent.as_slice() != files
    {
        *recent = files.to_vec();
    }
}

fn recent_count() -> usize {
    RECENT.lock().map_or(0, |r| r.len().min(5))
}

fn recent_name(index: usize) -> String {
    RECENT
        .lock()
        .ok()
        .and_then(|r| {
            r.get(index)
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default()
}

/// Nombre de lignes dans le presse-papiers interne (tenu à jour par
/// l'application).
static CLIPBOARD: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn set_clipboard_len(n: usize) {
    CLIPBOARD.store(n, std::sync::atomic::Ordering::Relaxed);
}

fn clipboard_len() -> usize {
    CLIPBOARD.load(std::sync::atomic::Ordering::Relaxed)
}

/// Noms des filtres prédéfinis (tenus à jour par l'application quand la
/// palette est ouverte).
static FILTERS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub fn set_filter_names(names: Vec<String>) {
    if let Ok(mut f) = FILTERS.lock() {
        *f = names;
    }
}

fn filter_count() -> usize {
    FILTERS.lock().map_or(0, |f| f.len())
}

fn filter_name(index: usize) -> String {
    FILTERS
        .lock()
        .ok()
        .and_then(|f| f.get(index).cloned())
        .unwrap_or_default()
}
