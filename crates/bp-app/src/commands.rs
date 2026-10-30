//! Registre des commandes de l'application : libellé, icône, groupe,
//! raccourci et mots-clés. Les menus et la palette de commandes s'en servent,
//! l'exécution est dans `BankPerfectApp::execute`.

use bp_core::i18n::{self, Lang};
use bp_core::{tr, tr_mark, trc};
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
    BankSync,
    Transfer,
    ConvertToTransfer,
    SplitOperation,
    GoToSister,
    Budget(BudgetTab),
    ExportQif,
    ExportJson,
    Print,
    Import,
    ImportJson,
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
    ToggleBalanceCards,
    ToggleColumnFilters,
    ToggleBalanceRiver,
    ReverseCurrencies,
    Scheduler,
    Forecast,
    Rules,
    Categorize,
    AutoFilters,
    SaveFilter,
    ApplyFilter(usize),
    ClearFilters,
    AddToScheduler,
    ToggleHideOld,
    ToggleTranslucent,
    Theme(ThemePreference),
    /// Thème de la galerie, par identifiant.
    UseTheme(&'static str),
    /// Langue de l'interface.
    Language(Lang),
    NextAccount,
    PreviousAccount,
    SelectAccount(usize),
    Manage(Tab),
    Tack,
    ReconcileAll,
    SmartTack,
    ModifyGroup,
    ExtraFields,
    Notes,
    Calculator,
    Loan,
    Currencies,
    Palette,
    Options,
    About,
    License,
    Help,
    /// Commande d'extension (rang dans `extensions::entries`).
    Extension(usize),
    ManageExtensions,
    ExtensionsFolder,
    ExtensionsDocs,
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

// Groupes des commandes, traduits par `Command::info`.
const FILE: &str = tr_mark!("Fichier");
const EDIT: &str = tr_mark!("Édition");
const OPERATIONS: &str = tr_mark!("Opérations");
const VIEW: &str = tr_mark!("Affichage");
const ACCOUNTS: &str = tr_mark!("Comptes");
const REPORTS: &str = tr_mark!("Rapports");
const TOOLS: &str = tr_mark!("Outils");
const HELP: &str = tr_mark!("Aide");
const EXTENSIONS: &str = tr_mark!("Extensions");

impl Command {
    /// Commandes proposées, dans l'ordre des menus.
    pub fn all(doc: Option<&OpenDoc>) -> Vec<Command> {
        use Command::*;
        let mut list = vec![NewFile, Open];
        list.extend((0..recent_count()).map(OpenRecent));
        list.push(ImportJson);
        if let Some(doc) = doc {
            list.extend([
                Save,
                SaveAs,
                SaveCopy,
                CloseFile,
                Import,
                ExportCsv,
                ExportQif,
                ExportJson,
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
                SmartTack,
                ToggleLock,
                ModifyGroup,
                ExtraFields,
                AddToPayees,
                ToggleHideOld,
                ToggleMarkDates,
                ToggleBalancePanel,
                ToggleForecast,
                ToggleBalanceCards,
                ToggleColumnFilters,
                ToggleBalanceRiver,
                ReverseCurrencies,
                Scheduler,
                Forecast,
                Rules,
                Categorize,
                ExportXlsx,
                Report(ReportTab::Accounts),
                Report(ReportTab::Categories),
                Report(ReportTab::Payees),
                Report(ReportTab::Statement),
                Charts,
                UpdateQuotes,
                BankSync,
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
        ]);
        list.extend(crate::theme::THEMES.iter().map(|t| UseTheme(t.id)));
        list.extend(Lang::ALL.map(Language));
        list.extend(
            crate::extensions::entries()
                .iter()
                .enumerate()
                .filter(|(_, e)| doc.is_some() || e.needs == bp_ext::Needs::Nothing)
                .map(|(i, _)| Extension(i)),
        );
        list.extend([ManageExtensions, ExtensionsFolder, ExtensionsDocs]);
        list.extend([ToggleTranslucent, Options, Help, License, About, Quit]);
        list
    }

    pub fn info(self, doc: Option<&OpenDoc>) -> Info {
        use Command::*;
        let has_doc = doc.is_some();
        let selection = doc.map_or(0, |d| d.selection.len());
        // Groupe, raccourci et mots-clés sont traduits ici (`tr_mark!`).
        let info = |group: &'static str,
                    label: &str,
                    icon,
                    shortcut: Option<&'static str>,
                    keywords: &'static str| Info {
            group: i18n::tr(group),
            label: label.to_string(),
            icon,
            shortcut: shortcut.map(i18n::tr),
            keywords: i18n::tr(keywords),
            enabled: true,
        };
        let mut result = match self {
            OpenRecent(i) => {
                let name = recent_name(i);
                info(
                    FILE,
                    &tr!("Rouvrir {}", name),
                    icon::CLOCK,
                    None,
                    tr_mark!("récent dernier"),
                )
            }
            NewFile => info(
                FILE,
                tr!("Nouveau fichier de comptes…"),
                icon::FILE_PLUS,
                Some("Ctrl+N"),
                tr_mark!("créer groupe"),
            ),
            CloseFile => info(
                FILE,
                tr!("Fermer le fichier"),
                icon::X_SQUARE,
                Some("Ctrl+W"),
                "",
            ),
            SaveAs => info(
                FILE,
                tr!("Enregistrer sous…"),
                icon::FLOPPY_DISK_BACK,
                Some("F12"),
                tr_mark!("copie renommer"),
            ),
            SaveCopy => info(
                FILE,
                tr!("Enregistrer une copie…"),
                icon::USB,
                None,
                tr_mark!("disque externe clé usb sauvegarde"),
            ),
            DeleteFile => info(
                FILE,
                tr!("Supprimer un fichier de comptes…"),
                icon::TRASH_SIMPLE,
                None,
                tr_mark!("corbeille effacer"),
            ),
            Open => info(
                FILE,
                tr!("Ouvrir un fichier…"),
                icon::FOLDER_OPEN,
                Some("Ctrl+O"),
                tr_mark!("charger"),
            ),
            Save => info(
                FILE,
                tr!("Enregistrer"),
                icon::FLOPPY_DISK,
                Some("Ctrl+S"),
                tr_mark!("sauver sauvegarder"),
            ),
            Import => info(
                FILE,
                tr!("Importer des opérations…"),
                icon::DOWNLOAD_SIMPLE,
                Some("Ctrl+I"),
                tr_mark!("qif ofx csv banque relevé télécharger"),
            ),
            Print => info(
                FILE,
                tr!("Imprimer le relevé…"),
                icon::PRINTER,
                Some("Ctrl+P"),
                tr_mark!("pdf aperçu relevé impression"),
            ),
            ImportJson => info(
                FILE,
                tr!("Importer un export JSON…"),
                icon::FILE_ARROW_UP,
                None,
                tr_mark!("json restaurer fichier complet format ouvert"),
            ),
            ExportJson => info(
                FILE,
                tr!("Exporter le fichier complet en JSON…"),
                icon::BRACKETS_CURLY,
                None,
                tr_mark!("json format ouvert sauvegarde complète développeur"),
            ),
            ExportCsv => info(
                FILE,
                tr!("Exporter en CSV…"),
                icon::FILE_CSV,
                None,
                tr_mark!("tableur excel"),
            ),
            ExportXlsx => info(
                FILE,
                tr!("Exporter en tableur (.xlsx)…"),
                icon::MICROSOFT_EXCEL_LOGO,
                None,
                tr_mark!("excel calc libreoffice classeur lignes affichées"),
            ),
            Report(tab) => {
                let (label, glyph, shortcut, keywords) = match tab {
                    ReportTab::Accounts => (
                        tr!("Total des comptes"),
                        icon::BANK,
                        None,
                        tr_mark!("soldes tous comptes"),
                    ),
                    ReportTab::Categories => (
                        tr!("Totaux par catégorie"),
                        icon::TAG,
                        Some("Ctrl+T"),
                        tr_mark!("tableau croisé dépenses recettes mois année"),
                    ),
                    ReportTab::Payees => (
                        tr!("Totaux par tiers"),
                        icon::ADDRESS_BOOK,
                        None,
                        tr_mark!("tableau croisé bénéficiaires"),
                    ),
                    ReportTab::Statement => (
                        tr!("Relevé multicompte"),
                        icon::LIST_BULLETS,
                        None,
                        tr_mark!("plusieurs comptes ensemble"),
                    ),
                };
                info(REPORTS, label, glyph, shortcut, keywords)
            }
            BankSync => info(
                FILE,
                tr!("Synchroniser avec la banque…"),
                icon::ARROWS_CLOCKWISE,
                None,
                tr_mark!("import automatique banque dsp2 enable banking agrégateur opérations"),
            ),
            UpdateQuotes => info(
                ACCOUNTS,
                tr!("Mettre à jour les cotations…"),
                icon::TREND_UP,
                None,
                tr_mark!("titres bourse actions isin cours portefeuille"),
            ),
            Charts => info(
                REPORTS,
                tr!("Graphiques…"),
                icon::CHART_BAR,
                Some(tr_mark!("Ctrl+Maj+G")),
                tr_mark!("camembert anneau barres courbe répartition évolution"),
            ),
            Budget(BudgetTab::Monthly) => info(
                REPORTS,
                tr!("Budget mensuel…"),
                icon::PIGGY_BANK,
                Some("Ctrl+B"),
                tr_mark!("prévu réel catégories dépenses"),
            ),
            Budget(BudgetTab::Annual) => info(
                REPORTS,
                tr!("Budgets annuels…"),
                icon::CALENDAR,
                None,
                tr_mark!("prévisions année tableau mois"),
            ),
            ExportQif => info(
                FILE,
                tr!("Exporter en QIF…"),
                icon::EXPORT,
                None,
                tr_mark!("quicken money"),
            ),
            Protection => info(
                FILE,
                tr!("Protection du fichier…"),
                icon::SHIELD_CHECK,
                None,
                tr_mark!("mot de passe chiffrement sécurité clé de secours"),
            ),
            Backups => info(
                FILE,
                tr!("Sauvegardes…"),
                icon::CLOCK_COUNTER_CLOCKWISE,
                None,
                tr_mark!("restaurer bp0 historique"),
            ),
            Quit => info(
                FILE,
                tr!("Quitter"),
                icon::SIGN_OUT,
                None,
                tr_mark!("fermer sortir"),
            ),
            Undo => {
                let label = doc
                    .and_then(|d| d.undo_label())
                    .map_or(trc!("undo", "Annuler").to_string(), |l| {
                        tr!("Annuler {}", l)
                    });
                let mut i = info(
                    EDIT,
                    &label,
                    icon::ARROW_U_UP_LEFT,
                    Some("Ctrl+Z"),
                    tr_mark!("défaire"),
                );
                i.enabled = doc.is_some_and(|d| d.undo_label().is_some());
                i
            }
            Redo => {
                let label = doc
                    .and_then(|d| d.redo_label())
                    .map_or(tr!("Refaire").to_string(), |l| tr!("Refaire {}", l));
                let mut i = info(
                    EDIT,
                    &label,
                    icon::ARROW_U_UP_RIGHT,
                    Some("Ctrl+Y"),
                    tr_mark!("rétablir"),
                );
                i.enabled = doc.is_some_and(|d| d.redo_label().is_some());
                i
            }
            NewOperation => info(
                EDIT,
                tr!("Nouvelle opération"),
                icon::PLUS,
                Some(tr_mark!("Inser")),
                tr_mark!("ajouter saisir ligne"),
            ),
            EditOperation => {
                let mut i = info(
                    EDIT,
                    tr!("Modifier l'opération"),
                    icon::PENCIL_SIMPLE,
                    Some(tr_mark!("Entrée")),
                    tr_mark!("éditer ligne"),
                );
                i.enabled = selection == 1;
                i
            }
            FocusEntry => info(
                EDIT,
                tr!("Aller à la saisie"),
                icon::CURSOR_TEXT,
                Some("F8"),
                tr_mark!("champ date formulaire saisir"),
            ),
            ModifySelected => {
                let mut i = info(
                    EDIT,
                    tr!("Modifier la ligne sélectionnée"),
                    icon::PENCIL_SIMPLE_LINE,
                    Some(tr_mark!("Maj+Entrée")),
                    tr_mark!("éditer enregistrer modification"),
                );
                i.enabled = selection == 1;
                i
            }
            Delete => {
                let mut i = info(
                    EDIT,
                    tr!("Supprimer les opérations"),
                    icon::TRASH,
                    Some(tr_mark!("Suppr")),
                    tr_mark!("effacer lignes"),
                );
                i.enabled = selection > 0;
                i
            }
            CutLines => {
                let mut i = info(
                    EDIT,
                    tr!("Couper"),
                    icon::SCISSORS,
                    Some("Ctrl+X"),
                    tr_mark!("déplacer lignes"),
                );
                i.enabled = selection > 0;
                i
            }
            CopyLines => {
                let mut i = info(
                    EDIT,
                    tr!("Copier"),
                    icon::COPY,
                    Some("Ctrl+C"),
                    tr_mark!("dupliquer lignes"),
                );
                i.enabled = selection > 0;
                i
            }
            PasteLines => {
                let n = clipboard_len();
                let label = match n {
                    0 | 1 => tr!("Coller").to_string(),
                    n => tr!("Coller {} lignes", n),
                };
                let mut i = info(
                    EDIT,
                    &label,
                    icon::CLIPBOARD_TEXT,
                    Some("Ctrl+V"),
                    tr_mark!("lignes copiées"),
                );
                i.enabled = n > 0;
                i
            }
            AddToPayees => {
                let mut i = info(
                    OPERATIONS,
                    tr!("Ajouter aux tiers mémorisés…"),
                    icon::USER_PLUS,
                    None,
                    tr_mark!("tiers créer mémoriser"),
                );
                i.enabled = selection == 1;
                i
            }
            SetMarkDate => {
                let mut i = info(
                    OPERATIONS,
                    tr!("Date de pointage…"),
                    icon::CALENDAR_CHECK,
                    Some(tr_mark!("Ctrl+Maj+P")),
                    tr_mark!("pointage date"),
                );
                i.enabled = selection > 0;
                i
            }
            ToggleLock => {
                let locked = doc.is_some_and(|d| d.locked);
                let label = if locked {
                    tr!("Déverrouiller les lignes rapprochées")
                } else {
                    tr!("Verrouiller les lignes rapprochées")
                };
                info(
                    OPERATIONS,
                    label,
                    icon::LOCK,
                    None,
                    tr_mark!("verrou protéger rapprochement"),
                )
            }
            ToggleMarkDates => {
                let on = doc.is_some_and(|d| d.show_mark_dates);
                let label = if on {
                    tr!("Masquer les dates de pointage")
                } else {
                    tr!("Afficher les dates de pointage")
                };
                info(
                    VIEW,
                    label,
                    icon::CALENDAR_CHECK,
                    Some("F11"),
                    tr_mark!("pointage colonne date"),
                )
            }
            ToggleBalancePanel => {
                let on = doc.is_none_or(|d| d.show_balance_panel);
                let label = if on {
                    tr!("Masquer le panneau des soldes")
                } else {
                    tr!("Afficher le panneau des soldes")
                };
                info(
                    VIEW,
                    label,
                    icon::SQUARES_FOUR,
                    None,
                    tr_mark!("soldes blocs pointé à venir"),
                )
            }
            ToggleBalanceCards => {
                let label = if doc.is_some_and(|d| d.balance_cards) {
                    tr!("Soldes en équation")
                } else {
                    tr!("Soldes en trois cartes")
                };
                info(
                    VIEW,
                    label,
                    icon::COLUMNS,
                    None,
                    tr_mark!("soldes cartes équation panneau bascule"),
                )
            }
            ToggleColumnFilters => {
                let label = if doc.is_some_and(|d| d.show_column_filters) {
                    tr!("Masquer les filtres de colonnes")
                } else {
                    tr!("Afficher les filtres de colonnes")
                };
                info(
                    VIEW,
                    label,
                    icon::MAGNIFYING_GLASS,
                    None,
                    tr_mark!("filtres colonnes recherche loupe tiers montant"),
                )
            }
            ToggleBalanceRiver => {
                let label = if doc.is_none_or(|d| d.show_balance_river) {
                    tr!("Masquer la courbe du solde")
                } else {
                    tr!("Afficher la courbe du solde")
                };
                info(
                    VIEW,
                    label,
                    icon::CHART_LINE,
                    None,
                    tr_mark!("courbe solde graphique colonne évolution"),
                )
            }
            ToggleForecast => {
                let on = doc.is_none_or(|d| d.show_forecast);
                let label = if on {
                    tr!("Masquer le solde prévisionnel")
                } else {
                    tr!("Afficher le solde prévisionnel")
                };
                info(
                    VIEW,
                    label,
                    icon::CHART_LINE,
                    None,
                    tr_mark!("prévision futur"),
                )
            }
            Rules => info(
                OPERATIONS,
                tr!("Règles automatiques…"),
                icon::MAGIC_WAND,
                Some("Ctrl+F"),
                tr_mark!("calcul automatique des champs catégoriser"),
            ),
            Categorize => info(
                OPERATIONS,
                tr!("Catégoriser les opérations…"),
                icon::SPARKLE,
                Some("Ctrl+J"),
                tr_mark!("deviner catégories manquantes automatique historique commerçants ia"),
            ),
            AutoFilters => info(
                VIEW,
                tr!("Filtres prédéfinis…"),
                icon::FUNNEL,
                Some("F7"),
                tr_mark!("filtrer période mois enregistrés"),
            ),
            SaveFilter => info(
                VIEW,
                tr!("Enregistrer le filtre actuel…"),
                icon::FUNNEL_SIMPLE,
                None,
                tr_mark!("mémoriser filtre"),
            ),
            ApplyFilter(i) => {
                let name = filter_name(i);
                info(
                    VIEW,
                    &tr!("Filtrer : {}", name),
                    icon::FUNNEL_SIMPLE,
                    None,
                    tr_mark!("filtre prédéfini"),
                )
            }
            ClearFilters => {
                let mut i = info(
                    VIEW,
                    tr!("Retirer tous les filtres"),
                    icon::FUNNEL_X,
                    None,
                    tr_mark!("effacer réinitialiser"),
                );
                i.enabled = doc.is_some_and(|d| d.filters.is_active());
                i
            }
            Scheduler => info(
                ACCOUNTS,
                tr!("Échéancier…"),
                icon::CALENDAR_DOTS,
                Some("Ctrl+E"),
                tr_mark!("échéances prélèvements récurrents abonnements"),
            ),
            Forecast => info(
                ACCOUNTS,
                tr!("Solde prévisionnel…"),
                icon::CHART_LINE_UP,
                None,
                tr_mark!("prévision futur échéances"),
            ),
            Transfer => {
                let mut i = info(
                    OPERATIONS,
                    tr!("Transfert de compte à compte…"),
                    icon::ARROWS_LEFT_RIGHT,
                    Some(tr_mark!("Ctrl+Maj+T")),
                    tr_mark!("virement entre comptes épargne"),
                );
                i.enabled = doc.is_some_and(|d| {
                    d.document.accounts.iter().filter(|a| !a.disabled).count() > 1
                });
                i
            }
            ConvertToTransfer => {
                let mut i = info(
                    OPERATIONS,
                    tr!("Convertir en transfert…"),
                    icon::LINK,
                    None,
                    tr_mark!("lier virement autre compte"),
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
                    OPERATIONS,
                    tr!("Ventiler l'opération…"),
                    icon::ARROWS_SPLIT,
                    Some("F6"),
                    tr_mark!("répartir plusieurs catégories parts"),
                );
                i.enabled = selection == 1;
                i
            }
            GoToSister => {
                let mut i = info(
                    OPERATIONS,
                    tr!("Aller à la ligne liée"),
                    icon::ARROW_SQUARE_OUT,
                    None,
                    tr_mark!("transfert sœur autre compte"),
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
                    OPERATIONS,
                    tr!("Ajouter à l'échéancier…"),
                    icon::CALENDAR_PLUS,
                    None,
                    tr_mark!("récurrent mensuel échéance"),
                );
                i.enabled = selection == 1;
                i
            }
            ReverseCurrencies => info(
                TOOLS,
                tr!("Inverser les monnaies du compte…"),
                icon::ARROWS_LEFT_RIGHT,
                None,
                tr_mark!("devise convertir euro franc"),
            ),
            Replace => info(
                EDIT,
                tr!("Remplacer…"),
                icon::SWAP,
                Some("Ctrl+R"),
                tr_mark!("rechercher texte tiers détail"),
            ),
            ToggleHideOld => {
                let hidden = doc.is_some_and(|d| d.filters.hide_old_lines);
                let label = if hidden {
                    tr!("Afficher les lignes inutiles")
                } else {
                    tr!("Cacher les lignes inutiles")
                };
                info(
                    VIEW,
                    label,
                    icon::EYE_SLASH,
                    Some("Ctrl+H"),
                    tr_mark!("masquer anciennes rapprochées archiver archivage"),
                )
            }
            ToggleTranslucent => {
                let label = if crate::theme::translucent() {
                    tr!("Fond opaque")
                } else {
                    tr!("Fond translucide (flou du bureau)")
                };
                info(
                    VIEW,
                    label,
                    icon::DROP_HALF,
                    None,
                    tr_mark!("transparent vitré verre flou"),
                )
            }
            Theme(ThemePreference::Dark) => info(
                VIEW,
                tr!("Mode sombre"),
                icon::MOON,
                None,
                tr_mark!("thème nuit couleurs"),
            ),
            Theme(ThemePreference::Light) => info(
                VIEW,
                tr!("Mode clair"),
                icon::SUN,
                None,
                tr_mark!("thème jour couleurs"),
            ),
            Theme(ThemePreference::System) => info(
                VIEW,
                tr!("Mode : suivre le système"),
                icon::DESKTOP,
                None,
                tr_mark!("thème automatique couleurs"),
            ),
            UseTheme(id) => {
                let theme = crate::theme::by_id(id).unwrap_or(&crate::theme::THEMES[0]);
                info(
                    VIEW,
                    &tr!("Thème : {}", bp_core::i18n::tr(theme.name)),
                    icon::PALETTE,
                    None,
                    if theme.dark {
                        tr_mark!("couleurs apparence sombre")
                    } else {
                        tr_mark!("couleurs apparence clair")
                    },
                )
            }
            Language(lang) => info(
                VIEW,
                &tr!("Langue : {} (language)", lang.native_name()),
                icon::TRANSLATE,
                None,
                tr_mark!("langue language traduction translation english anglais français french"),
            ),
            NextAccount => info(
                ACCOUNTS,
                tr!("Compte suivant"),
                icon::ARROW_RIGHT,
                Some("Ctrl+Tab"),
                "",
            ),
            PreviousAccount => info(
                ACCOUNTS,
                tr!("Compte précédent"),
                icon::ARROW_LEFT,
                Some(tr_mark!("Ctrl+Maj+Tab")),
                "",
            ),
            SelectAccount(i) => {
                let name = doc
                    .and_then(|d| d.document.accounts.get(i))
                    .map_or(String::new(), |a| a.name.clone());
                info(
                    ACCOUNTS,
                    &tr!("Aller au compte {}", name),
                    icon::BANK,
                    None,
                    tr_mark!("ouvrir afficher"),
                )
            }
            Manage(Tab::Accounts) => info(
                ACCOUNTS,
                tr!("Gestion des comptes…"),
                icon::BANK,
                Some("Ctrl+G"),
                tr_mark!("créer supprimer renommer iban rib banque"),
            ),
            Manage(Tab::Categories) => info(
                ACCOUNTS,
                tr!("Catégories…"),
                icon::TAG,
                None,
                tr_mark!("classement créer renommer"),
            ),
            Manage(Tab::Payees) => info(
                ACCOUNTS,
                tr!("Tiers mémorisés…"),
                icon::ADDRESS_BOOK,
                Some("F5"),
                tr_mark!("bénéficiaires"),
            ),
            Manage(Tab::Modes) => info(
                ACCOUNTS,
                tr!("Modes de paiement…"),
                icon::CREDIT_CARD,
                None,
                tr_mark!("carte chèque virement"),
            ),
            Tack => {
                let mut i = info(
                    OPERATIONS,
                    tr!("Pointer la sélection"),
                    icon::CHECK,
                    Some(tr_mark!("Espace")),
                    tr_mark!("cocher marquer"),
                );
                i.enabled = selection > 0;
                i
            }
            ReconcileAll => info(
                OPERATIONS,
                tr!("Rapprocher toutes les lignes pointées"),
                icon::CHECKS,
                None,
                tr_mark!("relevé banque"),
            ),
            SmartTack => info(
                OPERATIONS,
                tr!("Pointage intelligent…"),
                icon::SEAL_CHECK,
                None,
                tr_mark!("pointer automatiquement solde banque relevé deviner"),
            ),
            ModifyGroup => {
                let mut i = info(
                    OPERATIONS,
                    tr!("Modifier le groupe…"),
                    icon::PENCIL_SIMPLE_LINE,
                    Some("Ctrl+M"),
                    tr_mark!("plusieurs lignes sélection"),
                );
                i.enabled = selection > 0;
                i
            }
            ExtraFields => {
                let mut i = info(
                    OPERATIONS,
                    tr!("Champs supplémentaires…"),
                    icon::LIST_PLUS,
                    Some("F3"),
                    tr_mark!("valeurs"),
                );
                i.enabled = selection == 1;
                i
            }
            Notes => info(
                TOOLS,
                tr!("Notes"),
                icon::NOTE,
                Some(tr_mark!("Ctrl+Maj+N")),
                tr_mark!("pense-bête mémo"),
            ),
            Calculator => info(
                TOOLS,
                tr!("Calculatrice"),
                icon::CALCULATOR,
                Some(tr_mark!("Ctrl+Maj+C")),
                tr_mark!("calcul"),
            ),
            Loan => info(
                TOOLS,
                tr!("Simulateur de crédit…"),
                icon::HOUSE_LINE,
                Some("Ctrl+L"),
                tr_mark!("emprunt prêt immobilier mensualité taux amortissement"),
            ),
            Currencies => info(
                TOOLS,
                tr!("Convertisseur de devises…"),
                icon::CURRENCY_CIRCLE_DOLLAR,
                Some("Ctrl+D"),
                tr_mark!("change cours bce dollar livre franc suisse"),
            ),
            Options => info(
                TOOLS,
                tr!("Options…"),
                icon::SLIDERS,
                Some("Ctrl+,"),
                tr_mark!("préférences réglages paramètres"),
            ),
            License => info(
                HELP,
                tr!("Licence…"),
                icon::KEY,
                None,
                tr_mark!("licence clé enregistrer activer achat"),
            ),
            About => info(
                HELP,
                tr!("À propos de BankPerfect"),
                icon::INFO,
                None,
                tr_mark!("version licence"),
            ),
            Help => info(
                HELP,
                tr!("Aide en ligne"),
                icon::QUESTION,
                None,
                tr_mark!("manuel documentation"),
            ),
            Palette => info(
                TOOLS,
                tr!("Palette de commandes"),
                icon::COMMAND,
                Some("Ctrl+K"),
                tr_mark!("rechercher"),
            ),
            Extension(index) => match crate::extensions::entry(index) {
                Some(entry) => Info {
                    group: entry.group,
                    label: entry.label.clone(),
                    icon: entry.icon,
                    shortcut: entry.shortcut,
                    keywords: entry.keywords,
                    enabled: match entry.needs {
                        bp_ext::Needs::Nothing => true,
                        bp_ext::Needs::File => has_doc,
                        bp_ext::Needs::Selection => selection > 0,
                    },
                },
                None => Info {
                    group: i18n::tr(EXTENSIONS),
                    label: String::new(),
                    icon: icon::PUZZLE_PIECE,
                    shortcut: None,
                    keywords: "",
                    enabled: false,
                },
            },
            ManageExtensions => info(
                EXTENSIONS,
                tr!("Gérer les extensions…"),
                icon::PUZZLE_PIECE,
                None,
                tr_mark!("plugins scripts luau modules"),
            ),
            ExtensionsFolder => info(
                EXTENSIONS,
                tr!("Ouvrir le dossier des extensions"),
                icon::FOLDER_OPEN,
                None,
                tr_mark!("plugins scripts luau"),
            ),
            ExtensionsDocs => info(
                EXTENSIONS,
                tr!("Documentation des extensions"),
                icon::BOOK_OPEN,
                None,
                tr_mark!("api luau développer créer plugin"),
            ),
        };
        if !has_doc
            && !matches!(
                self,
                Open | OpenRecent(_)
                    | NewFile
                    | ImportJson
                    | DeleteFile
                    | Quit
                    | Notes
                    | Calculator
                    | Theme(_)
                    | UseTheme(_)
                    | Language(_)
                    | ToggleTranslucent
                    | Palette
                    | Options
                    | License
                    | About
                    | Help
                    | Extension(_)
                    | ManageExtensions
                    | ExtensionsFolder
                    | ExtensionsDocs
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

#[cfg(test)]
mod tests {
    use super::*;
    use bp_core::i18n::with_lang;

    #[test]
    fn commands_in_english() {
        with_lang(Lang::En, || {
            let save = Command::Save.info(None);
            assert_eq!(save.label, "Save");
            assert_eq!(save.group, "File");
            let charts = Command::Charts.info(None);
            assert_eq!(charts.shortcut, Some("Ctrl+Shift+G"));
            assert!(charts.keywords.contains("pie"));
            assert_eq!(Command::Undo.info(None).label, "Undo");
            assert_eq!(
                Command::Language(Lang::Fr).info(None).label,
                "Language: Français"
            );
        });
        with_lang(Lang::Fr, || {
            let charts = Command::Charts.info(None);
            assert_eq!(charts.group, "Rapports");
            assert_eq!(charts.shortcut, Some("Ctrl+Maj+G"));
            assert_eq!(Command::Undo.info(None).label, "Annuler");
        });
    }
}
