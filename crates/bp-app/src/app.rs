//! Fenêtre principale : menus, mise en page, ouverture/enregistrement.

use std::path::{Path, PathBuf};

use bp_core::FileFormat;
use bp_core::file;
use bp_core::format_v6::{Protection, Unlock};
use bp_core::model::{Amount, Tack};
use egui::{Align, Key, KeyboardShortcut, Layout, Modifiers, RichText};
use egui_phosphor::regular as icon;

use crate::dialogs::{self, Choice, PasswordPrompt, PromptResult};
use crate::document::{CursorTarget, OpenDoc};
use crate::editor::{Editor, EditorAction};
use crate::format::money;
use crate::grid::{self, GridAction};
use crate::theme::palette;
use crate::{menu, sidebar};

const SHORTCUT_OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SHORTCUT_SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SHORTCUT_NEXT_ACCOUNT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::Tab);
const SHORTCUT_PREV_ACCOUNT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::Tab);
const SHORTCUT_HIDE_OLD: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::H);
const SHORTCUT_UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const SHORTCUT_REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const SHORTCUT_REDO_ALT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const SHORTCUT_REPLACE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::R);
const SHORTCUT_GROUP: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::M);
const SHORTCUT_EXTRA: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F3);
const SHORTCUT_ACCOUNTS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::G);
const SHORTCUT_PAYEES: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F5);
const SHORTCUT_NOTES: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::N);
const SHORTCUT_CALC: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::C);

/// Action différée en attendant la réponse à « Enregistrer les modifications ? ».
enum Pending {
    Open(Option<PathBuf>),
    Quit,
}

pub struct BankPerfectApp {
    doc: Option<OpenDoc>,
    editor: Option<Editor>,
    prompt: Option<PasswordPrompt>,
    recovery_phrase: Option<String>,
    show_warnings: bool,
    pending: Option<Pending>,
    confirm_delete: bool,
    tool: Option<crate::tools::Tool>,
    error: Option<String>,
    toast: Option<(String, f64)>,
    allow_close: bool,
    title: String,
    drawer_open: bool,
    drawer_opened_at: f64,
    menus: menu::MenuBar,
    profiler: crate::profiler::Profiler,
    settings: crate::settings::Settings,
    /// Le thème des réglages reste à appliquer (première image).
    theme_pending: bool,
    /// Vraie application (pas un test) : notes affichées à l'ouverture.
    interactive: bool,
}

const DRAWER_WIDTH: f32 = 460.0;
const DRAWER_SLIDE_SECONDS: f64 = 0.15;

impl BankPerfectApp {
    /// Application avec les réglages enregistrés de l'utilisateur.
    pub fn new(initial_file: Option<PathBuf>) -> Self {
        let mut app = Self::with_settings(None, crate::settings::Settings::load());
        app.theme_pending = true;
        app.interactive = true;
        if let Some(path) = initial_file {
            app.open_path(path);
        }
        app
    }

    /// Application avec des réglages donnés (tests : thème laissé tel quel).
    pub fn with_settings(
        initial_file: Option<PathBuf>,
        settings: crate::settings::Settings,
    ) -> Self {
        let mut app = Self {
            doc: None,
            editor: None,
            prompt: None,
            recovery_phrase: None,
            show_warnings: false,
            pending: None,
            confirm_delete: false,
            tool: None,
            error: None,
            toast: None,
            allow_close: false,
            title: String::new(),
            drawer_open: false,
            drawer_opened_at: 0.0,
            menus: menu::MenuBar::default(),
            profiler: crate::profiler::Profiler::from_env(),
            settings,
            theme_pending: false,
            interactive: false,
        };
        if let Some(path) = initial_file {
            app.open_path(path);
        }
        app
    }

    fn request(&mut self, action: Pending) {
        if self.doc.as_ref().is_some_and(|d| d.dirty) {
            self.pending = Some(action);
        } else {
            self.run(action);
        }
    }

    fn run(&mut self, action: Pending) {
        match action {
            Pending::Open(Some(path)) => self.open_path(path),
            Pending::Open(None) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Ouvrir un fichier BankPerfect")
                    .add_filter("Comptes BankPerfect", &["bp", "bp0"])
                    .pick_file()
                {
                    self.open_path(path);
                }
            }
            Pending::Quit => self.allow_close = true,
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        let data = match std::fs::read(&path) {
            Ok(data) => data,
            Err(e) => {
                return self.error = Some(format!("Impossible de lire {} : {e}", path.display()));
            }
        };
        match file::requires_password(&data) {
            Ok(true) => {
                self.prompt = Some(PasswordPrompt {
                    can_use_recovery_key: bp_core::format_v6::is_v6(&data),
                    path,
                    input: String::new(),
                    use_recovery_key: false,
                    error: None,
                })
            }
            Ok(false) => self.finish_open(path, &data, Unlock::None),
            Err(e) => self.error = Some(format!("{} : {e}", path.display())),
        }
    }

    fn finish_open(&mut self, path: PathBuf, data: &[u8], unlock: Unlock<'_>) {
        match file::open_bytes(data, unlock) {
            Ok(opened) => {
                self.show_warnings = !opened.warnings.is_empty();
                self.settings.remember(&path);
                self.doc = Some(OpenDoc::new(path, opened));
                if self.interactive
                    && self.tool.is_none()
                    && let Some(notes) = crate::tools::NotesForm::on_open()
                {
                    self.tool = Some(crate::tools::Tool::Notes(notes));
                }
                self.editor = None;
                self.prompt = None;
            }
            Err(bp_core::Error::WrongPassword) => {
                if let Some(prompt) = &mut self.prompt {
                    prompt.error = Some("Mot de passe ou clé de secours incorrect".into());
                    prompt.input.clear();
                }
            }
            Err(e) => {
                self.prompt = None;
                self.error = Some(format!("{} : {e}", path.display()));
            }
        }
    }

    fn save(&mut self, now: f64) {
        let Some(doc) = &mut self.doc else { return };
        if let Err(e) = bp_core::backup::backup_before_save(
            &doc.path,
            &self.settings.backup_dir(),
            self.settings.backup_keep,
        ) {
            self.error = Some(format!(
                "Sauvegarde .bp0 impossible : {e}. Fichier non enregistré."
            ));
            return;
        }
        match save_document(doc) {
            Ok(recovery) => {
                self.recovery_phrase = recovery;
                self.toast = Some((format!("{} enregistré", doc.file_name()), now + 3.0));
            }
            Err(e) => self.error = Some(format!("Enregistrement impossible : {e}")),
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_OPEN)) {
            self.request(Pending::Open(None));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_NOTES)) {
            self.tool = Some(crate::tools::Tool::Notes(crate::tools::NotesForm::new()));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_CALC)) {
            self.tool = Some(crate::tools::Tool::Calculator(crate::tools::CalcForm::new()));
        }
        let Some(doc) = &mut self.doc else { return };
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_SAVE)) {
            self.save(now);
            return;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_PREV_ACCOUNT)) {
            doc.cycle_account(false);
            self.editor = None;
        } else if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_NEXT_ACCOUNT)) {
            doc.cycle_account(true);
            self.editor = None;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_HIDE_OLD)) {
            doc.filters.hide_old_lines = !doc.filters.hide_old_lines;
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&SHORTCUT_REDO_ALT) || i.consume_shortcut(&SHORTCUT_REDO)
        }) {
            doc.redo();
            self.editor = None;
        } else if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_UNDO)) {
            doc.undo();
            self.editor = None;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_REPLACE)) {
            self.tool = Some(crate::tools::Tool::Replace(Default::default()));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_GROUP)) {
            self.tool = Some(crate::tools::Tool::ModifyGroup(
                crate::tools::GroupForm::new(),
            ));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_ACCOUNTS)) {
            self.tool = Some(crate::tools::Tool::Manage(
                crate::manage_ui::ManageForm::new(doc, crate::manage_ui::Tab::Accounts),
            ));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_PAYEES)) {
            self.tool = Some(crate::tools::Tool::Manage(
                crate::manage_ui::ManageForm::new(doc, crate::manage_ui::Tab::Payees),
            ));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_EXTRA))
            && let Some(&index) = doc.selection.iter().next()
        {
            self.tool = Some(crate::tools::Tool::ExtraValues(
                crate::tools::ExtraForm::new(doc, index),
            ));
        }

        // Raccourcis de la grille, actifs quand aucun champ n'a le focus.
        if ctx.memory(|m| m.focused().is_some()) || self.menus.is_open() {
            return;
        }
        let pressed = |key| ctx.input(|i| i.key_pressed(key));
        let (ctrl, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        // Déplacements : flèches, pages, Début/Fin (écran), Ctrl+Début/Fin
        // (liste) ; avec Maj, la sélection s'étend.
        let target = [
            (Key::ArrowUp, CursorTarget::Up),
            (Key::ArrowDown, CursorTarget::Down),
            (Key::PageUp, CursorTarget::PageUp),
            (Key::PageDown, CursorTarget::PageDown),
            (
                Key::Home,
                if ctrl {
                    CursorTarget::First
                } else {
                    CursorTarget::ScreenTop
                },
            ),
            (
                Key::End,
                if ctrl {
                    CursorTarget::Last
                } else {
                    CursorTarget::ScreenBottom
                },
            ),
        ]
        .into_iter()
        .find(|(key, _)| pressed(*key))
        .map(|(_, target)| target);
        if let Some(target) = target {
            doc.move_cursor(target, shift);
        } else if pressed(Key::Space) || pressed(Key::P) {
            doc.cycle_tack();
        } else if pressed(Key::Insert) || pressed(Key::Plus) {
            self.editor = Some(Editor::new_operation(&doc.document, doc.account));
        } else if pressed(Key::Enter)
            && let Some(&index) = doc.selection.iter().next()
        {
            self.editor = Some(Editor::edit(&doc.document, doc.account, index));
        } else if pressed(Key::Delete) && !doc.selection.is_empty() {
            self.confirm_delete = true;
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match &self.doc {
            Some(doc) => format!(
                "{}{} — BankPerfect",
                doc.file_name(),
                if doc.dirty { " •" } else { "" }
            ),
            None => "BankPerfect".to_string(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let titles: &[&str] = if self.doc.is_some() {
            &[
                "Fichier",
                "Édition",
                "Affichage",
                "Comptes",
                "Opérations",
                "Outils",
            ]
        } else {
            &["Fichier", "Outils"]
        };
        if let Some((index, anchor)) = self.menus.titles(ui, titles) {
            let ctx = ui.ctx().clone();
            let mut menus = std::mem::take(&mut self.menus);
            menus.popup(&ctx, index, anchor, |ui| self.menu_content(ui, index));
            self.menus = menus;
        }
    }

    fn menu_content(&mut self, ui: &mut egui::Ui, index: usize) {
        let now = ui.input(|i| i.time);
        if index == 0 {
            menu::begin(ui);
            if menu::item(ui, Some(icon::FOLDER_OPEN), "Ouvrir…", Some("Ctrl+O")).clicked() {
                self.request(Pending::Open(None));
            }
            if menu::item_enabled(
                ui,
                self.doc.is_some(),
                Some(icon::FLOPPY_DISK),
                "Enregistrer",
                Some("Ctrl+S"),
            )
            .clicked()
            {
                self.save(now);
            }
            if self.doc.is_some() {
                menu::separator(ui);
                if menu::item(ui, Some(icon::FILE_CSV), "Exporter en CSV…", None).clicked() {
                    self.tool = Some(crate::tools::Tool::ExportCsv(crate::tools::CsvForm::new()));
                }
                if menu::item(ui, Some(icon::EXPORT), "Exporter en QIF…", None).clicked()
                    && let Some(doc) = &self.doc
                    && let Some(message) = crate::tools::export_qif(doc)
                {
                    let now = ui.input(|i| i.time);
                    self.toast = Some((message, now + 4.0));
                }
            }
            if self.doc.is_some()
                && menu::item(ui, Some(icon::SHIELD_CHECK), "Protection…", None).clicked()
            {
                self.tool = Some(crate::tools::Tool::Protection(Default::default()));
            }
            if self.doc.is_some()
                && menu::item(
                    ui,
                    Some(icon::CLOCK_COUNTER_CLOCKWISE),
                    "Sauvegardes…",
                    None,
                )
                .clicked()
            {
                self.tool = Some(crate::tools::Tool::Backups(crate::tools::BackupsForm::new(
                    self.settings.backup_dir(),
                    self.settings.backup_keep,
                )));
            }
            menu::separator(ui);
            if menu::item(ui, Some(icon::SIGN_OUT), "Quitter", None).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }

            return;
        }
        let tools_index = if self.doc.is_some() { 5 } else { 1 };
        if index == tools_index {
            menu::begin(ui);
            if menu::item(ui, Some(icon::NOTE), "Notes", Some("Ctrl+Maj+N")).clicked() {
                self.tool = Some(crate::tools::Tool::Notes(crate::tools::NotesForm::new()));
            }
            if menu::item(
                ui,
                Some(icon::CALCULATOR),
                "Calculatrice",
                Some("Ctrl+Maj+C"),
            )
            .clicked()
            {
                self.tool = Some(crate::tools::Tool::Calculator(crate::tools::CalcForm::new()));
            }
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        match index {
            1 => {
                menu::begin(ui);
                let undo = doc
                    .undo_label()
                    .map(|l| format!("Annuler {l}"))
                    .unwrap_or_else(|| "Annuler".into());
                if menu::item_enabled(
                    ui,
                    doc.undo_label().is_some(),
                    Some(icon::ARROW_U_UP_LEFT),
                    &undo,
                    Some("Ctrl+Z"),
                )
                .clicked()
                {
                    doc.undo();
                    self.editor = None;
                }
                let redo = doc
                    .redo_label()
                    .map(|l| format!("Refaire {l}"))
                    .unwrap_or_else(|| "Refaire".into());
                if menu::item_enabled(
                    ui,
                    doc.redo_label().is_some(),
                    Some(icon::ARROW_U_UP_RIGHT),
                    &redo,
                    Some("Ctrl+Y"),
                )
                .clicked()
                {
                    doc.redo();
                    self.editor = None;
                }
                menu::separator(ui);
                if menu::item(ui, Some(icon::PLUS), "Nouvelle opération", Some("Inser")).clicked()
                {
                    self.editor = Some(Editor::new_operation(&doc.document, doc.account));
                }
                let single = doc.selection.iter().next().copied();
                if menu::item_enabled(
                    ui,
                    single.is_some(),
                    Some(icon::PENCIL_SIMPLE),
                    "Modifier l'opération",
                    Some("Entrée"),
                )
                .clicked()
                    && let Some(index) = single
                {
                    self.editor = Some(Editor::edit(&doc.document, doc.account, index));
                }
                menu::separator(ui);
                if menu::item_enabled(
                    ui,
                    !doc.selection.is_empty(),
                    Some(icon::TRASH),
                    "Supprimer",
                    Some("Suppr"),
                )
                .clicked()
                {
                    self.confirm_delete = true;
                }
                menu::separator(ui);
                if menu::item(ui, Some(icon::SWAP), "Remplacer…", Some("Ctrl+R")).clicked() {
                    self.tool = Some(crate::tools::Tool::Replace(Default::default()));
                }
            }
            2 => {
                menu::begin(ui);
                menu::toggle(
                    ui,
                    &mut doc.filters.hide_old_lines,
                    "Cacher les lignes inutiles",
                    Some("Ctrl+H"),
                );
                menu::separator(ui);
                menu::heading(ui, "Thème");
                let current = ui.ctx().options(|o| o.theme_preference);
                for (preference, label) in [
                    (egui::ThemePreference::Dark, "Tokyo Night"),
                    (egui::ThemePreference::Light, "Tokyo Night Day"),
                    (egui::ThemePreference::System, "Suivre le système"),
                ] {
                    if menu::choice(ui, current == preference, label).clicked() {
                        ui.ctx().set_theme(preference);
                        self.settings.set_theme(preference);
                    }
                }
            }
            3 => {
                menu::begin(ui);
                for &i in &doc.document.account_order.clone() {
                    let account = &doc.document.accounts[i];
                    if account.disabled {
                        continue;
                    }
                    if menu::choice(ui, i == doc.account, &account.name).clicked() {
                        doc.select_account(i);
                        self.editor = None;
                    }
                }
                menu::separator(ui);
                if menu::item(
                    ui,
                    Some(icon::ARROW_RIGHT),
                    "Compte suivant",
                    Some("Ctrl+Tab"),
                )
                .clicked()
                {
                    doc.cycle_account(true);
                }
                if menu::item(
                    ui,
                    Some(icon::ARROW_LEFT),
                    "Compte précédent",
                    Some("Ctrl+Maj+Tab"),
                )
                .clicked()
                {
                    doc.cycle_account(false);
                }
                menu::separator(ui);
                for (tab, label, glyph, shortcut) in [
                    (
                        crate::manage_ui::Tab::Accounts,
                        "Gestion des comptes…",
                        icon::BANK,
                        Some("Ctrl+G"),
                    ),
                    (
                        crate::manage_ui::Tab::Categories,
                        "Catégories…",
                        icon::TAG,
                        None,
                    ),
                    (
                        crate::manage_ui::Tab::Payees,
                        "Tiers…",
                        icon::ADDRESS_BOOK,
                        Some("F5"),
                    ),
                    (
                        crate::manage_ui::Tab::Modes,
                        "Modes de paiement…",
                        icon::CREDIT_CARD,
                        None,
                    ),
                ] {
                    if menu::item(ui, Some(glyph), label, shortcut).clicked() {
                        self.tool = Some(crate::tools::Tool::Manage(
                            crate::manage_ui::ManageForm::new(doc, tab),
                        ));
                    }
                }
            }
            _ => {
                menu::begin_with_width(ui, 340.0);
                if menu::item_enabled(
                    ui,
                    !doc.selection.is_empty(),
                    Some(icon::CHECK),
                    "Pointer",
                    Some("Espace"),
                )
                .clicked()
                {
                    doc.cycle_tack();
                }
                if menu::item(
                    ui,
                    Some(icon::CHECKS),
                    "Rapprocher toutes les lignes pointées",
                    None,
                )
                .clicked()
                {
                    doc.reconcile_all_pointed();
                }
                menu::separator(ui);
                if menu::item_enabled(
                    ui,
                    !doc.selection.is_empty(),
                    Some(icon::PENCIL_SIMPLE_LINE),
                    "Modifier le groupe…",
                    Some("Ctrl+M"),
                )
                .clicked()
                {
                    self.tool = Some(crate::tools::Tool::ModifyGroup(
                        crate::tools::GroupForm::new(),
                    ));
                }
                if menu::item_enabled(
                    ui,
                    doc.selection.len() == 1,
                    Some(icon::LIST_PLUS),
                    "Champs supplémentaires…",
                    Some("F3"),
                )
                .clicked()
                    && let Some(&index) = doc.selection.iter().next()
                {
                    self.tool = Some(crate::tools::Tool::ExtraValues(
                        crate::tools::ExtraForm::new(doc, index),
                    ));
                }
            }
        }
    }

    fn balance_strip(ui: &mut egui::Ui, doc: &mut OpenDoc) {
        let pal = palette(ui);
        let account = doc.current_account();
        let filtered = doc.filters.is_active();
        // En-tête compact sur une ligne : nom du compte, banque et IBAN.
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&account.name)
                    .size(18.0)
                    .family(crate::theme::semibold()),
            );
            let mut sub = account.bank_name.clone();
            if !account.iban.trim().is_empty() {
                sub = if sub.is_empty() {
                    account.iban.trim().to_string()
                } else {
                    format!("{sub}  ·  {}", account.iban.trim())
                };
            }
            ui.label(RichText::new(sub).small().color(pal.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Place pour le bouton + qui flotte au-dessus, à cheval sur les menus.
                ui.add_space(64.0);
                // Dans l'en-tête plutôt qu'en dessous : la grille ne bouge pas
                // quand un filtre s'active.
                if filtered {
                    ui.label(
                        RichText::new(format!(
                            "{}  soldes calculés sur les lignes filtrées",
                            icon::FUNNEL
                        ))
                        .small()
                        .color(pal.pointed),
                    );
                }
            });
        });
        ui.add_space(8.0);
        crate::dashboard::cards(ui, doc);
        ui.add_space(-4.0);
    }

    fn status_bar(ui: &mut egui::Ui, doc: &mut OpenDoc, toast: Option<&str>) {
        let pal = palette(ui);
        ui.horizontal(|ui| {
            let shown = doc.visible_len();
            let total = doc.total_count();
            if doc.filters.is_active() || doc.filters.hide_old_lines {
                ui.label(
                    RichText::new(format!(
                        "{}  {} / {} opérations",
                        icon::FUNNEL,
                        crate::format::count(shown),
                        crate::format::count(total)
                    ))
                    .color(pal.pointed),
                );
                filter_chips(ui, doc);
            } else {
                ui.label(format!("{} opérations", crate::format::count(total)));
            }
            ui.separator();
            selection_summary(ui, doc);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("BankPerfect · Enregistré")
                        .small()
                        .color(pal.muted),
                );
                if let Some(text) = toast {
                    ui.label(RichText::new(format!("{}  {text}", icon::CHECK)).color(pal.positive));
                }
            });
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.28);
            ui.label(
                RichText::new(icon::PIGGY_BANK)
                    .size(56.0)
                    .color(palette(ui).accent),
            );
            ui.label(
                RichText::new("BankPerfect")
                    .size(30.0)
                    .family(crate::theme::semibold()),
            );
            ui.add_space(18.0);
            if ui
                .add(egui::Button::new(
                    RichText::new(format!("{}  Ouvrir un fichier…", icon::FOLDER_OPEN)).size(15.0),
                ))
                .clicked()
            {
                self.request(Pending::Open(None));
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new("ou déposez un fichier .bp dans la fenêtre").color(palette(ui).muted),
            );
            let recent: Vec<PathBuf> = self
                .settings
                .recent_files
                .iter()
                .filter(|p| p.exists())
                .cloned()
                .collect();
            if !recent.is_empty() {
                ui.add_space(28.0);
                ui.label(
                    RichText::new("FICHIERS RÉCENTS")
                        .small()
                        .family(crate::theme::medium())
                        .color(palette(ui).muted),
                );
                ui.add_space(4.0);
                for path in recent {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let folder = path
                        .parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    let button =
                        egui::Button::new(RichText::new(format!("{}  {name}", icon::FILE)))
                            .frame(false);
                    if ui.add(button).on_hover_text(&folder).clicked() {
                        self.request(Pending::Open(Some(path)));
                    }
                }
            }
        });
    }

    /// Tiroir de saisie qui glisse par-dessus la grille, collé au bord droit.
    fn editor_drawer(&mut self, ctx: &egui::Context, content: egui::Rect, now: f64) {
        let open = self.editor.is_some();
        if open && !self.drawer_open {
            self.drawer_opened_at = now;
        }
        self.drawer_open = open;
        let (Some(editor), Some(doc)) = (&mut self.editor, &mut self.doc) else {
            return;
        };

        // Glissement depuis la droite (150 ms, décélération).
        let t = (((now - self.drawer_opened_at) / DRAWER_SLIDE_SECONDS) as f32).clamp(0.0, 1.0);
        if t < 1.0 {
            ctx.request_repaint();
        }
        let eased = 1.0 - (1.0 - t).powi(3);
        let width = DRAWER_WIDTH.min(content.width() - 40.0);
        let x = content.right() - width + (1.0 - eased) * width;

        let pal = crate::theme::palette_of(ctx);
        let action = egui::Area::new(egui::Id::new("editor_drawer"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(x, content.top()))
            .constrain(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(pal.surface)
                    .stroke(egui::Stroke::new(1.0, pal.border))
                    .corner_radius(egui::CornerRadius {
                        nw: 14,
                        sw: 14,
                        ne: 0,
                        se: 0,
                    })
                    .shadow(egui::Shadow {
                        offset: [-6, 0],
                        blur: 28,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(90),
                    })
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 8,
                        bottom: 16,
                    })
                    .show(ui, |ui| {
                        ui.set_width(width - 40.0);
                        ui.set_min_height(content.height() - 24.0);
                        ui.set_max_height(content.height() - 24.0);
                        editor.show(ui, doc)
                    })
                    .inner
            })
            .inner;
        match action {
            EditorAction::None => {}
            EditorAction::Close => self.editor = None,
            EditorAction::Save(target, op) => {
                doc.upsert_operation(target, op);
                self.editor = None;
            }
        }
    }

    fn modals(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if let Some(tool) = &mut self.tool {
            let result = match (&mut self.doc, tool) {
                (_, crate::tools::Tool::Notes(form)) => Some(form.show_standalone(ctx)),
                (_, crate::tools::Tool::Calculator(form)) => Some(form.show_standalone(ctx)),
                (Some(doc), tool) => Some(tool.show(ctx, doc)),
                (None, _) => None,
            };
            let Some(result) = result else {
                self.tool = None;
                return;
            };
            match result {
                crate::tools::Outcome::Keep => {}
                crate::tools::Outcome::Close => self.tool = None,
                crate::tools::Outcome::Done(message) => {
                    self.tool = None;
                    self.toast = Some((message, now + 4.0));
                }
                crate::tools::Outcome::Protected(phrase) => {
                    self.tool = None;
                    self.recovery_phrase = Some(phrase);
                    self.toast = Some((
                        "Mot de passe défini — enregistrez (Ctrl+S) pour l'appliquer".into(),
                        now + 6.0,
                    ));
                }
                crate::tools::Outcome::Reopen(path, message) => {
                    self.tool = None;
                    if let Some(doc) = &mut self.doc {
                        doc.dirty = false;
                    }
                    self.open_path(path);
                    self.toast = Some((message, now + 4.0));
                }
            }
        }
        if let Some(prompt) = &mut self.prompt {
            match prompt.show(ctx) {
                PromptResult::Pending => {}
                PromptResult::Cancel => self.prompt = None,
                PromptResult::Submit => {
                    let path = prompt.path.clone();
                    let input = prompt.input.clone();
                    let recovery = prompt.use_recovery_key;
                    match std::fs::read(&path) {
                        Ok(data) => {
                            let unlock = if recovery {
                                Unlock::RecoveryKey(&input)
                            } else {
                                Unlock::Password(&input)
                            };
                            self.finish_open(path, &data, unlock);
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            }
        }
        if let Some(phrase) = &self.recovery_phrase
            && dialogs::recovery_key(ctx, phrase)
        {
            self.recovery_phrase = None;
        }
        if self.show_warnings
            && let Some(doc) = &self.doc
            && dialogs::warnings(ctx, &doc.warnings)
        {
            self.show_warnings = false;
        }
        if self.confirm_delete
            && let Some(doc) = &mut self.doc
        {
            let n = doc.selection.len();
            let message = if n == 1 {
                "Supprimer l'opération sélectionnée ?".to_string()
            } else {
                format!("Supprimer les {n} opérations sélectionnées ?")
            };
            match dialogs::confirm(ctx, "delete", "Supprimer", &message, "Supprimer", None) {
                Choice::Pending => {}
                Choice::Yes => {
                    doc.delete_selection();
                    self.editor = None;
                    self.confirm_delete = false;
                }
                Choice::No | Choice::Cancel => self.confirm_delete = false,
            }
        }
        if self.pending.is_some() {
            match dialogs::confirm(
                ctx,
                "unsaved",
                "Modifications non enregistrées",
                "Voulez-vous enregistrer les modifications avant de continuer ?",
                "Enregistrer",
                Some("Ne pas enregistrer"),
            ) {
                Choice::Pending => {}
                Choice::Yes => {
                    self.save(now);
                    if self.doc.as_ref().is_some_and(|d| !d.dirty) {
                        let action = self.pending.take().expect("action en attente");
                        self.run(action);
                    }
                }
                Choice::No => {
                    if let Some(doc) = &mut self.doc {
                        doc.dirty = false;
                    }
                    let action = self.pending.take().expect("action en attente");
                    self.run(action);
                }
                Choice::Cancel => self.pending = None,
            }
        }
        if let Some(error) = &self.error
            && dialogs::alert(ctx, "Erreur", error)
        {
            self.error = None;
        }
    }
}

impl eframe::App for BankPerfectApp {
    /// Molette sans lissage : egui étale chaque cran sur ~0,1 s, ce qui donne
    /// une impression de mollesse. On convertit les crans en points et on
    /// passe par le chemin « tactile » d'egui, appliqué immédiatement.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.profiler.note_events(&raw_input.events);
        let mut events = Vec::with_capacity(raw_input.events.len() + 2);
        for event in raw_input.events.drain(..) {
            match event {
                egui::Event::MouseWheel {
                    unit,
                    delta,
                    phase: egui::TouchPhase::Move,
                    modifiers,
                } => {
                    let delta = match unit {
                        egui::MouseWheelUnit::Line => delta * crate::theme::WHEEL_LINE_SPEED,
                        egui::MouseWheelUnit::Point => delta * crate::theme::WHEEL_POINT_FACTOR,
                        egui::MouseWheelUnit::Page => delta * 600.0,
                    };
                    let wheel = |delta, phase| egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta,
                        phase,
                        modifiers,
                    };
                    events.push(wheel(egui::Vec2::ZERO, egui::TouchPhase::Start));
                    events.push(wheel(delta, egui::TouchPhase::Move));
                }
                other => events.push(other),
            }
        }
        raw_input.events = events;
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        let cpu_previous = frame.info().cpu_usage;
        self.frame_ui(ui);
        self.profiler.overlay(ui.ctx(), cpu_previous);
        self.profiler.end_frame(started, cpu_previous);
    }
}

impl BankPerfectApp {
    fn frame_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.theme_pending) {
            ctx.set_theme(self.settings.theme_preference());
        }
        crate::theme::adapt_to_display(&ctx);

        if let Some(path) = ctx.input(|i| {
            i.raw
                .dropped_files
                .first()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
        }) {
            self.request(Pending::Open(Some(path)));
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.doc.as_ref().is_some_and(|d| d.dirty)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        let modal_open = self.prompt.is_some()
            || self.recovery_phrase.is_some()
            || self.pending.is_some()
            || self.confirm_delete
            || self.error.is_some()
            || self.show_warnings
            || self.tool.is_some();
        if !modal_open {
            self.handle_shortcuts(&ctx);
        }
        let now = ctx.input(|i| i.time);
        if self.toast.as_ref().is_some_and(|(_, until)| now > *until) {
            self.toast = None;
        }

        egui::Panel::top("menu")
            .show_separator_line(false)
            .show(ui, |ui| self.menu_bar(ui));

        if self.doc.is_none() {
            egui::CentralPanel::default_margins().show(ui, |ui| self.welcome(ui));
            self.modals(&ctx);
            self.update_title(&ctx);
            return;
        }

        let toast = self.toast.as_ref().map(|(t, _)| t.clone());
        let doc = self.doc.as_mut().expect("document ouvert");
        egui::Panel::bottom("status").show(ui, |ui| Self::status_bar(ui, doc, toast.as_deref()));

        let sidebar_fill = palette(ui).bg_dark;
        egui::Panel::left("accounts")
            .resizable(true)
            .default_size(260.0)
            .min_size(200.0)
            .frame(
                egui::Frame::side_top_panel(ui.style())
                    .fill(sidebar_fill)
                    .inner_margin(12),
            )
            .show(ui, |ui| sidebar::show(ui, doc));

        // Zone à droite de la barre latérale : le tiroir de saisie s'y superpose.
        let content_rect = ui.available_rect_before_wrap();

        doc.refresh();
        let mut grid_action = GridAction::None;
        egui::Panel::top("balances")
            .frame(
                egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::symmetric(18, 0)),
            )
            .show(ui, |ui| Self::balance_strip(ui, doc));

        egui::CentralPanel::default_margins().show(ui, |ui| {
            let action = grid::show(ui, doc);
            if !matches!(action, GridAction::None) {
                grid_action = action;
            }
        });
        if self.editor.is_none() && floating_add_button(&ctx, content_rect) {
            grid_action = GridAction::New;
        }
        match grid_action {
            GridAction::None => {}
            GridAction::Edit(index) => {
                self.editor = Some(Editor::edit(&doc.document, doc.account, index))
            }
            GridAction::New => {
                self.editor = Some(Editor::new_operation(&doc.document, doc.account))
            }
            GridAction::Extra(index) => {
                self.tool = Some(crate::tools::Tool::ExtraValues(
                    crate::tools::ExtraForm::new(doc, index),
                ))
            }
        }

        self.editor_drawer(&ctx, content_rect, now);
        self.modals(&ctx);
        self.update_title(&ctx);
    }
}

/// Retire un filtre.
type ClearFilter = fn(&mut bp_core::filter::Filters);

/// Un badge par filtre actif ; un clic le retire.
fn filter_chips(ui: &mut egui::Ui, doc: &mut OpenDoc) {
    let f = &doc.filters;
    let mut chips: Vec<(String, ClearFilter)> = Vec::new();
    if !f.mode.is_empty() {
        chips.push((format!("Mode : {}", f.mode), |f| f.mode.clear()));
    }
    if !f.payee.is_empty() {
        chips.push((format!("Tiers : {}", f.payee), |f| f.payee.clear()));
    }
    if !f.category.is_empty() {
        chips.push((format!("Catégorie : {}", f.category), |f| {
            f.category.clear()
        }));
    }
    if !f.amount.is_empty() {
        chips.push((format!("Montant : {}", f.amount), |f| f.amount.clear()));
    }
    if f.tack != bp_core::filter::TackFilter::All {
        chips.push((
            format!("Pointage : {}", f.tack.label().to_lowercase()),
            |f| f.tack = bp_core::filter::TackFilter::All,
        ));
    }
    let period = match (f.date_from, f.date_to) {
        (Some(a), Some(b)) => Some(format!("Période : du {a} au {b}")),
        (Some(a), None) => Some(format!("Période : à partir du {a}")),
        (None, Some(b)) => Some(format!("Période : jusqu'au {b}")),
        (None, None) => None,
    };
    if let Some(period) = period {
        chips.push((period, |f| {
            f.date_from = None;
            f.date_to = None;
        }));
    }
    if f.hide_old_lines {
        chips.push(("Lignes rapprochées anciennes masquées".into(), |f| {
            f.hide_old_lines = false
        }));
    }

    let pal = palette(ui);
    let font = egui::TextStyle::Small.resolve(ui.style());
    let mut remove = None;
    for (index, (label, _)) in chips.iter().enumerate() {
        let text = ui
            .painter()
            .layout_no_wrap(label.clone(), font.clone(), pal.fg);
        let cross = ui
            .painter()
            .layout_no_wrap(icon::X.to_string(), font.clone(), pal.fg);
        let size = egui::vec2(text.size().x + cross.size().x + 30.0, text.size().y + 8.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let hovered = response.hovered();
        let fill = if hovered {
            pal.accent.gamma_multiply(0.28)
        } else {
            pal.accent.gamma_multiply(0.18)
        };
        ui.painter().rect_filled(rect, rect.height() / 2.0, fill);
        ui.painter().galley(
            egui::pos2(rect.left() + 10.0, rect.center().y - text.size().y / 2.0),
            text,
            pal.fg,
        );
        // La croix passe au rouge au survol : le clic retire le filtre.
        let cross_center = egui::pos2(rect.right() - 10.0 - cross.size().x / 2.0, rect.center().y);
        if hovered {
            ui.painter()
                .circle_filled(cross_center, 8.0, pal.negative.gamma_multiply(0.22));
        }
        ui.painter().text(
            cross_center,
            egui::Align2::CENTER_CENTER,
            icon::X,
            font.clone(),
            if hovered { pal.negative } else { pal.muted },
        );
        if response.on_hover_text("Retirer ce filtre").clicked() {
            remove = Some(index);
        }
    }
    if chips.len() > 1 && ui.link(RichText::new("Tout effacer").small()).clicked() {
        doc.filters = Default::default();
    }
    if let Some(index) = remove {
        (chips[index].1)(&mut doc.filters);
    }
}

/// Bouton rond « Nouvelle opération », à cheval sur la barre de menus en
/// haut à droite : effet givré plat, sombre et semi-transparent, croix blanche.
fn floating_add_button(ctx: &egui::Context, content: egui::Rect) -> bool {
    const SIZE: f32 = 44.0;
    let center = egui::pos2(content.right() - 46.0, content.top() + 2.0);
    let rect = egui::Rect::from_center_size(center, egui::vec2(SIZE, SIZE));
    egui::Area::new(egui::Id::new("floating_add"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min - egui::vec2(12.0, 12.0))
        .show(ctx, |ui| {
            let (outer, _) =
                ui.allocate_exact_size(egui::vec2(SIZE + 24.0, SIZE + 24.0), egui::Sense::hover());
            let rect = egui::Rect::from_center_size(outer.center(), egui::vec2(SIZE, SIZE));
            let response = ui
                .interact(
                    rect,
                    egui::Id::new("floating_add_button"),
                    egui::Sense::click(),
                )
                .on_hover_text("Nouvelle opération (Inser)");
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Nouvelle opération")
            });
            let hovered = response.hovered();
            let painter = ui.painter();
            let radius = SIZE / 2.0;
            // Givre plat : disque sombre translucide, voile clair uniforme et
            // liseré fin, sans reflet ni relief.
            let shadow = egui::Shadow {
                offset: [0, 2],
                blur: 12,
                spread: 0,
                color: egui::Color32::from_black_alpha(45),
            };
            painter.add(shadow.as_shape(rect, egui::CornerRadius::same(radius as u8)));
            painter.circle_filled(
                rect.center(),
                radius,
                egui::Color32::from_rgba_unmultiplied(0x16, 0x17, 0x20, 165),
            );
            painter.circle_filled(
                rect.center(),
                radius,
                egui::Color32::from_white_alpha(if hovered { 26 } else { 12 }),
            );
            painter.circle_stroke(
                rect.center(),
                radius - 0.5,
                egui::Stroke::new(
                    1.0,
                    egui::Color32::from_white_alpha(if hovered { 60 } else { 34 }),
                ),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                icon::PLUS,
                egui::FontId::proportional(22.0),
                egui::Color32::WHITE,
            );
            response.clicked()
        })
        .inner
}

/// Total de la sélection (BAL-07).
fn selection_summary(ui: &mut egui::Ui, doc: &OpenDoc) {
    let pal = palette(ui);
    let currency = doc.current_account().currency.clone();
    let ops: Vec<_> = doc.selection.iter().map(|&i| doc.operation(i)).collect();
    match ops.len() {
        0 => {}
        1 => {
            let op = ops[0];
            let tack = match op.tack {
                Tack::None => "",
                Tack::Pointed => " · pointée",
                Tack::Reconciled => " · rapprochée",
            };
            ui.label(format!(
                "Ligne {} : {}{tack}",
                op.uid,
                money(op.amount, &currency)
            ));
        }
        n => {
            let total: Amount = ops.iter().map(|op| op.amount).sum();
            let credits: Amount = ops
                .iter()
                .filter(|op| !op.amount.is_negative())
                .map(|op| op.amount)
                .sum();
            let debits: Amount = ops
                .iter()
                .filter(|op| op.amount.is_negative())
                .map(|op| op.amount)
                .sum();
            let mut text = RichText::new(format!("{n} lignes : {}", money(total, &currency)))
                .family(crate::theme::semibold());
            if total.is_negative() {
                text = text.color(pal.negative);
            }
            ui.label(text);
            if credits != Amount::ZERO && debits != Amount::ZERO {
                ui.label(
                    RichText::new(format!(
                        "+{}  {}",
                        money(credits, &currency),
                        money(debits, &currency)
                    ))
                    .color(pal.muted),
                );
            }
        }
    }
}

/// Enregistre au format v6. Au premier enregistrement d'un fichier v5,
/// l'original est conservé à côté (`<nom>_v5.bp0`) et, s'il avait un mot de
/// passe, le fichier est chiffré avec : la clé de secours est renvoyée.
fn save_document(doc: &mut OpenDoc) -> bp_core::Result<Option<String>> {
    let mut recovery = None;
    if doc.protection.is_none() {
        let protection = match &doc.legacy_password {
            Some(password) => {
                let (protection, key) = Protection::with_password(password)?;
                recovery = Some(key.phrase());
                protection
            }
            None => Protection::none()?,
        };
        doc.protection = Some(protection);
    }
    if doc.format == FileFormat::V5 {
        backup_v5_original(&doc.path)?;
    }
    file::save(
        &doc.path,
        &doc.document,
        doc.protection.as_ref().expect("protection choisie"),
    )?;
    doc.format = FileFormat::V6;
    doc.dirty = false;
    Ok(recovery)
}

fn backup_v5_original(path: &Path) -> std::io::Result<()> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let backup = path.with_file_name(format!("{stem}_v5.bp0"));
    if !backup.exists() {
        std::fs::copy(path, backup)?;
    }
    Ok(())
}
