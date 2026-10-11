//! Registre des commandes de l'application : libellé, icône, groupe,
//! raccourci et mots-clés. Les menus et la palette de commandes s'en servent,
//! l'exécution est dans `BankPerfectApp::execute`.

use egui::ThemePreference;
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::manage_ui::Tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Open,
    Save,
    ExportCsv,
    ExportQif,
    Protection,
    Backups,
    Quit,
    Undo,
    Redo,
    NewOperation,
    EditOperation,
    Delete,
    Replace,
    ToggleHideOld,
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
    Palette,
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
        let mut list = vec![Open];
        if let Some(doc) = doc {
            list.extend([
                Save,
                ExportCsv,
                ExportQif,
                Protection,
                Backups,
                Undo,
                Redo,
                NewOperation,
                EditOperation,
                Delete,
                Replace,
                Tack,
                ReconcileAll,
                ModifyGroup,
                ExtraFields,
                ToggleHideOld,
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
            list.extend([
                Manage(Tab::Accounts),
                Manage(Tab::Categories),
                Manage(Tab::Payees),
                Manage(Tab::Modes),
            ]);
        }
        list.extend([
            Notes,
            Calculator,
            Theme(ThemePreference::Dark),
            Theme(ThemePreference::Light),
            Theme(ThemePreference::System),
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
            ExportCsv => info(
                "Fichier",
                "Exporter en CSV…",
                icon::FILE_CSV,
                None,
                "tableur excel",
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
            Palette => info(
                "Outils",
                "Palette de commandes",
                icon::COMMAND,
                Some("Ctrl+K"),
                "rechercher",
            ),
        };
        if !has_doc && !matches!(self, Open | Quit | Notes | Calculator | Theme(_) | Palette) {
            result.enabled = false;
        }
        result
    }
}
