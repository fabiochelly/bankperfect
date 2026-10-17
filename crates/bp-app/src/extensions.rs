//! Liaison entre l'application et le moteur d'extensions : dossier, services
//! du système, commandes (menus, palette, raccourcis) et fenêtre de gestion.

use std::path::PathBuf;
use std::rc::Rc;

use bp_ext::{Engine, Needs};
use egui::{Key, KeyboardShortcut, Modifiers, RichText};
use egui_phosphor::regular as icon;

use crate::ext_ui::glyph;
use crate::theme::palette;
use crate::ui_kit;

/// Adresse de la documentation des extensions.
pub const DOCS_URL: &str =
    "https://github.com/fabiochelly/bankperfect/blob/main/docs/extensions.md";

/// Exemples livrés avec l'application (installables depuis la fenêtre de
/// gestion).
pub const EXAMPLES: &[(&str, &str)] = &[
    (
        "doublons.luau",
        include_str!("../../../extensions/doublons.luau"),
    ),
    (
        "synthese-du-mois.luau",
        include_str!("../../../extensions/synthese-du-mois.luau"),
    ),
    (
        "arrondis.luau",
        include_str!("../../../extensions/arrondis.luau"),
    ),
    (
        "export-json.luau",
        include_str!("../../../extensions/export-json.luau"),
    ),
    (
        "selection.luau",
        include_str!("../../../extensions/selection.luau"),
    ),
    (
        "import-debit-credit.luau",
        include_str!("../../../extensions/import-debit-credit.luau"),
    ),
];

struct AppServices;

fn dialog(title: &str, filters: &[(String, Vec<String>)]) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    for (name, extensions) in filters {
        dialog = dialog.add_filter(name, extensions);
    }
    dialog
}

impl bp_ext::Services for AppServices {
    fn pick_open(&self, title: &str, filters: &[(String, Vec<String>)]) -> Option<PathBuf> {
        dialog(title, filters).pick_file()
    }

    fn pick_save(
        &self,
        title: &str,
        name: &str,
        filters: &[(String, Vec<String>)],
    ) -> Option<PathBuf> {
        let dir = crate::settings::shared::export_dir(std::path::Path::new("."));
        dialog(title, filters)
            .set_file_name(name)
            .set_directory(dir)
            .save_file()
    }

    fn open_url(&self, url: &str) {
        let _ = open::that_detached(url);
    }
}

/// Dossier des extensions de l'utilisateur (créé au besoin).
pub fn user_dir() -> PathBuf {
    let dir =
        crate::settings::config_file("extensions").unwrap_or_else(|| PathBuf::from("extensions"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Moteur sur un dossier ; `None` : aucun dossier (tests).
pub fn engine(dir: Option<PathBuf>, disabled: &[String]) -> Engine {
    let dir = dir.unwrap_or_default();
    let data =
        crate::settings::config_file("extensions-donnees").unwrap_or_else(|| dir.join(".donnees"));
    let mut engine = Engine::new(dir.clone(), data, Rc::new(AppServices));
    if !dir.as_os_str().is_empty() {
        engine.load_all(disabled);
    }
    engine
}

/// Commande d'extension telle que vue par les menus et la palette.
#[derive(Clone)]
pub struct Entry {
    pub ext: usize,
    pub command: usize,
    pub label: String,
    pub group: &'static str,
    pub icon: &'static str,
    pub shortcut: Option<&'static str>,
    pub keys: Option<KeyboardShortcut>,
    pub needs: Needs,
    pub keywords: &'static str,
    /// Menus où la commande apparaît (`bp_ext::MENUS`).
    pub menus: Vec<&'static str>,
}

/// Commandes d'extension (copie tenue à jour par l'application).
static ENTRIES: std::sync::Mutex<Vec<Entry>> = std::sync::Mutex::new(Vec::new());

/// Chaîne gardée pour toute la durée du programme (libellés des menus).
fn intern(text: &str) -> &'static str {
    static STRINGS: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let mut strings = STRINGS.lock().expect("verrou");
    if let Some(s) = strings.iter().find(|s| **s == text) {
        return s;
    }
    let s: &'static str = Box::leak(text.to_string().into_boxed_str());
    strings.push(s);
    s
}

/// `"Ctrl+Alt+D"` → raccourci egui (Ctrl = Cmd sous macOS).
pub fn parse_shortcut(text: &str) -> Option<KeyboardShortcut> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+').map(str::trim) {
        match part.to_lowercase().as_str() {
            "ctrl" | "cmd" | "commande" => modifiers = modifiers.plus(Modifiers::COMMAND),
            "alt" | "option" => modifiers = modifiers.plus(Modifiers::ALT),
            "maj" | "shift" => modifiers = modifiers.plus(Modifiers::SHIFT),
            _ => key = Key::from_name(part).or_else(|| Key::from_name(&part.to_uppercase())),
        }
    }
    let key = key?;
    // Une simple lettre sans Ctrl ni Alt gênerait la saisie.
    let function_key = matches!(key.name().as_bytes(), [b'F', _, ..]) && key.name().len() <= 3;
    if !function_key && !modifiers.command && !modifiers.alt {
        return None;
    }
    Some(KeyboardShortcut::new(modifiers, key))
}

/// Recopie les commandes des extensions actives (après chargement).
pub fn refresh(engine: &Engine) {
    let entries = engine
        .commands()
        .into_iter()
        .map(|(ext, command, info, name)| {
            let keys = info.shortcut.as_deref().and_then(parse_shortcut);
            Entry {
                ext,
                command,
                label: info.title.clone(),
                group: intern(&name),
                icon: glyph(info.icon.as_deref()),
                shortcut: keys.map(|_| intern(info.shortcut.as_deref().unwrap_or_default())),
                keys,
                needs: info.needs,
                menus: info.menus.iter().map(|m| intern(m)).collect(),
                keywords: intern(&format!(
                    "extension {} {}",
                    name,
                    info.description.unwrap_or_default()
                )),
            }
        })
        .collect();
    if let Ok(mut e) = ENTRIES.lock() {
        *e = entries;
    }
}

pub fn entries() -> Vec<Entry> {
    ENTRIES.lock().map(|e| e.clone()).unwrap_or_default()
}

/// Commandes placées dans un menu (rangs dans [`entries`]), dans l'ordre.
pub fn placed(menu: &str) -> Vec<(usize, Entry)> {
    entries()
        .into_iter()
        .enumerate()
        .filter(|(_, e)| e.menus.contains(&menu))
        .collect()
}

/// Libellé d'un emplacement (`"grid"` → « clic droit sur les opérations »).
pub fn menu_label(menu: &str) -> &'static str {
    bp_ext::MENUS
        .iter()
        .find(|(id, _)| *id == menu)
        .map_or("Extensions", |(_, label)| label)
}

pub fn entry(index: usize) -> Option<Entry> {
    ENTRIES.lock().ok().and_then(|e| e.get(index).cloned())
}

// ─────────────────────────── Fenêtre de gestion ───────────────────────────

#[derive(Default)]
pub struct Manager {
    selected: usize,
}

pub enum ManagerAction {
    None,
    Close,
    Toggle(usize, bool),
    Reload,
    OpenFolder,
    InstallExamples,
    Docs,
}

fn permission_label(permission: &str) -> (&'static str, String) {
    match permission {
        "network" => (icon::GLOBE, "Accès à Internet".into()),
        other => (icon::KEY, other.to_string()),
    }
}

fn event_label(event: bp_ext::Event) -> &'static str {
    match event {
        bp_ext::Event::FileOpened => "à l'ouverture du fichier",
        bp_ext::Event::OperationAdded => "à chaque opération ajoutée",
        bp_ext::Event::OperationChanged => "à chaque opération modifiée",
        bp_ext::Event::OperationDeleted => "à chaque opération supprimée",
        bp_ext::Event::BeforeSave => "avant chaque enregistrement",
        bp_ext::Event::AfterSave => "après chaque enregistrement",
        bp_ext::Event::AccountSelected => "au changement de compte",
        bp_ext::Event::SelectionChanged => "au changement de sélection",
    }
}

impl Manager {
    pub fn show(&mut self, ctx: &egui::Context, engine: &Engine) -> ManagerAction {
        let mut action = ManagerAction::None;
        let width = ui_kit::dialog_width(ctx, 0.66, 760.0, 1040.0);
        crate::dialogs::modal(ctx, "extensions_manager").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::PUZZLE_PIECE, pal.accent, "Extensions");
            ui_kit::subtitle(
                ui,
                "Des scripts Luau ajoutent des commandes, des fenêtres et des automatismes. Ils tournent en bac à sable : ils n'accèdent qu'à vos comptes et, s'ils le déclarent, à Internet.",
            );
            let height = ui_kit::dialog_height(ctx, 0.56);
            let list_width = (width * 0.36).min(340.0);
            let detail_width = width - list_width - 16.0;
            if self.selected >= engine.extensions.len() {
                self.selected = 0;
            }
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.allocate_ui_with_layout(
                    egui::vec2(list_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(list_width);
                        ui_kit::caption(ui, icon::LIST, "Installées");
                        if engine.extensions.is_empty() {
                            ui_kit::card(ui, list_width, |ui| {
                                ui.label(RichText::new("Aucune extension pour l'instant.").color(pal.fg_dark));
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Installez les exemples pour découvrir ce qu'elles permettent, ou déposez vos fichiers .luau dans le dossier des extensions.")
                                        .size(12.5)
                                        .color(pal.muted),
                                );
                                ui.add_space(12.0);
                                if crate::dialogs::primary(ui, &format!("{}  Installer les exemples", icon::DOWNLOAD_SIMPLE), true).clicked() {
                                    action = ManagerAction::InstallExamples;
                                }
                            });
                            return;
                        }
                        egui::ScrollArea::vertical()
                            .id_salt("extensions_list")
                            .max_height(height - 24.0)
                            .show(ui, |ui| {
                                for (i, ext) in engine.extensions.iter().enumerate() {
                                    let selected = i == self.selected;
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(list_width - 6.0, 58.0),
                                        egui::Sense::click(),
                                    );
                                    let fill = if selected {
                                        pal.selection
                                    } else if response.hovered() {
                                        pal.bg_highlight
                                    } else {
                                        pal.bg
                                    };
                                    ui.painter().rect(
                                        rect,
                                        10.0,
                                        fill,
                                        egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)),
                                        egui::StrokeKind::Inside,
                                    );
                                    let status = if ext.error.is_some() {
                                        (icon::WARNING_CIRCLE, pal.negative)
                                    } else if ext.enabled {
                                        let first = ext.commands().into_iter().next();
                                        let glyph = match first {
                                            None if !ext.importers().is_empty() => icon::DOWNLOAD_SIMPLE,
                                            first => glyph(first.and_then(|c| c.icon).as_deref()),
                                        };
                                        (glyph, pal.accent)
                                    } else {
                                        (icon::PROHIBIT, pal.muted)
                                    };
                                    ui.painter().text(
                                        egui::pos2(rect.left() + 22.0, rect.center().y),
                                        egui::Align2::CENTER_CENTER,
                                        status.0,
                                        egui::FontId::proportional(19.0),
                                        status.1,
                                    );
                                    let name = ui_kit::one_line(
                                        ui,
                                        ext.name(),
                                        egui::FontId::new(14.0, crate::theme::semibold()),
                                        if ext.enabled { pal.fg } else { pal.muted },
                                        rect.width() - 56.0,
                                    );
                                    ui.painter().galley(
                                        egui::pos2(rect.left() + 42.0, rect.top() + 10.0),
                                        name,
                                        pal.fg,
                                    );
                                    let detail = match (&ext.error, ext.enabled) {
                                        (Some(_), _) => "Erreur au chargement".to_string(),
                                        (None, false) => "Désactivée".to_string(),
                                        (None, true) => {
                                            let n = ext.commands().len();
                                            let version = ext.version();
                                            let commands = match n {
                                                0 if !ext.importers().is_empty() => {
                                                    "format d'import".to_string()
                                                }
                                                0 => "automatismes".to_string(),
                                                1 => "1 commande".to_string(),
                                                n => format!("{n} commandes"),
                                            };
                                            if version.is_empty() {
                                                commands
                                            } else {
                                                format!("v{version} · {commands}")
                                            }
                                        }
                                    };
                                    let detail = ui_kit::one_line(
                                        ui,
                                        detail,
                                        egui::FontId::proportional(12.5),
                                        pal.muted,
                                        rect.width() - 56.0,
                                    );
                                    ui.painter().galley(
                                        egui::pos2(rect.left() + 42.0, rect.top() + 32.0),
                                        detail,
                                        pal.muted,
                                    );
                                    if response.clicked() {
                                        self.selected = i;
                                    }
                                    ui.add_space(6.0);
                                }
                            });
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(detail_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(detail_width);
                        if let Some(ext) = engine.extensions.get(self.selected) {
                            egui::ScrollArea::vertical()
                                .id_salt("extensions_detail")
                                .max_height(height)
                                .show(ui, |ui| self.detail(ui, ext, detail_width, &mut action));
                        }
                    },
                );
            });
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, "Fermer", true).clicked() {
                    action = ManagerAction::Close;
                }
                if crate::dialogs::button(ui, format!("{}  Recharger", icon::ARROW_CLOCKWISE)).clicked() {
                    action = ManagerAction::Reload;
                }
                if crate::dialogs::button(ui, format!("{}  Ouvrir le dossier", icon::FOLDER_OPEN)).clicked() {
                    action = ManagerAction::OpenFolder;
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if crate::dialogs::button(ui, format!("{}  Documentation", icon::BOOK_OPEN)).clicked() {
                        action = ManagerAction::Docs;
                    }
                    if !engine.extensions.is_empty()
                        && crate::dialogs::button(ui, format!("{}  Exemples", icon::DOWNLOAD_SIMPLE))
                            .on_hover_text("Ajoute les extensions d'exemple qui manquent")
                            .clicked()
                    {
                        action = ManagerAction::InstallExamples;
                    }
                });
            });
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                action = ManagerAction::Close;
            }
        });
        action
    }

    fn detail(
        &self,
        ui: &mut egui::Ui,
        ext: &bp_ext::Extension,
        width: f32,
        action: &mut ManagerAction,
    ) {
        let pal = palette(ui);
        ui_kit::card(ui, width - 4.0, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(ext.name())
                            .size(17.0)
                            .family(crate::theme::semibold()),
                    );
                    let mut meta = Vec::new();
                    if !ext.version().is_empty() {
                        meta.push(format!("version {}", ext.version()));
                    }
                    if !ext.author().is_empty() {
                        meta.push(format!("par {}", ext.author()));
                    }
                    meta.push(
                        ext.path
                            .strip_prefix(
                                ext.path
                                    .parent()
                                    .and_then(|p| p.parent())
                                    .unwrap_or(&ext.path),
                            )
                            .unwrap_or(&ext.path)
                            .display()
                            .to_string(),
                    );
                    ui.label(RichText::new(meta.join(" · ")).size(12.5).color(pal.muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut enabled = ext.enabled;
                    if crate::theme::checkbox(ui, &mut enabled, "Activée").changed() {
                        *action = ManagerAction::Toggle(self.selected, enabled);
                    }
                });
            });
            let description = ext.description();
            if !description.is_empty() {
                ui.add_space(10.0);
                ui.add(egui::Label::new(RichText::new(description).color(pal.fg_dark)).wrap());
            }
        });
        ui.add_space(14.0);
        if let Some(error) = &ext.error {
            ui_kit::caption(ui, icon::WARNING, "Erreur au chargement");
            egui::Frame::new()
                .fill(pal.negative.gamma_multiply(0.12))
                .corner_radius(10)
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(width - 32.0);
                    ui.add(egui::Label::new(RichText::new(error).monospace().color(pal.fg)).wrap());
                });
            ui.add_space(14.0);
        }
        let commands = ext.commands();
        if !commands.is_empty() {
            ui_kit::section(ui, width - 4.0, icon::COMMAND, "Commandes", |ui| {
                for (i, c) in commands.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(6.0);
                    }
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(glyph(c.icon.as_deref()))
                                .size(16.0)
                                .color(pal.accent),
                        );
                        ui.label(RichText::new(&c.title).family(crate::theme::medium()));
                        if let Some(s) = &c.shortcut {
                            let ok = parse_shortcut(s).is_some();
                            ui_kit::pill(ui, s, if ok { pal.muted } else { pal.negative });
                        }
                        if c.needs == Needs::Selection {
                            ui.label(
                                RichText::new("sur la sélection")
                                    .size(12.0)
                                    .color(pal.muted),
                            );
                        }
                    });
                    if c.menus != ["extensions"] {
                        let places: Vec<&str> = c.menus.iter().map(|m| menu_label(m)).collect();
                        ui.label(
                            RichText::new(format!("{}  {}", icon::LIST, places.join(", ")))
                                .size(12.5)
                                .color(pal.muted),
                        );
                    }
                    if let Some(d) = &c.description {
                        ui.label(RichText::new(d).size(12.5).color(pal.fg_dark));
                    }
                }
            });
        }
        let events = ext.events();
        let permissions = ext.permissions();
        let importers = ext.importers();
        if !events.is_empty() || !permissions.is_empty() || !importers.is_empty() {
            ui_kit::section(
                ui,
                width - 4.0,
                icon::SHIELD_CHECK,
                "Fonctionnement",
                |ui| {
                    for (name, extensions) in &importers {
                        let suffixes: Vec<String> =
                            extensions.iter().map(|e| format!(".{e}")).collect();
                        ui.label(format!(
                            "{}  Importe « {name} » ({})",
                            icon::DOWNLOAD_SIMPLE,
                            suffixes.join(", ")
                        ));
                    }
                    for e in events {
                        ui.label(format!("{}  Réagit {}", icon::LIGHTNING, event_label(e)));
                    }
                    for p in permissions {
                        let (g, label) = permission_label(&p);
                        ui.label(RichText::new(format!("{g}  {label}")).color(pal.warning));
                    }
                },
            );
        }
        let log = ext.log();
        if !log.is_empty() {
            ui_kit::section(ui, width - 4.0, icon::TERMINAL, "Journal (print)", |ui| {
                for line in log.iter().rev().take(40).rev() {
                    ui.label(
                        RichText::new(line)
                            .monospace()
                            .size(12.0)
                            .color(pal.fg_dark),
                    );
                }
            });
        }
    }
}

/// Copie les exemples absents dans le dossier ; renvoie le nombre copié.
pub fn install_examples(dir: &std::path::Path) -> usize {
    let _ = std::fs::create_dir_all(dir);
    EXAMPLES
        .iter()
        .filter(|(name, source)| {
            let path = dir.join(name);
            !path.exists() && std::fs::write(path, source).is_ok()
        })
        .count()
}
