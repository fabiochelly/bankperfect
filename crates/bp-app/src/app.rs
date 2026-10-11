//! Fenêtre principale : menus, mise en page, ouverture/enregistrement.

use std::path::{Path, PathBuf};

use bp_core::FileFormat;
use bp_core::file;
use bp_core::format_v6::{Protection, Unlock};
use bp_core::model::{Amount, Tack};
use egui::{Align, Key, KeyboardShortcut, Layout, Modifiers, RichText};
use egui_phosphor::regular as icon;

use crate::commands::Command;
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
const SHORTCUT_IMPORT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);
const SHORTCUT_PRINT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::P);
const SHORTCUT_OPTIONS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma);
const SHORTCUT_NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const SHORTCUT_SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F12);
const SHORTCUT_CLOSE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);
const SHORTCUT_PALETTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);
const SHORTCUT_MARK_DATE: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::P);
const SHORTCUT_MARK_DATES: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F11);
const SHORTCUT_CUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::X);
const SHORTCUT_COPY: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::C);
const SHORTCUT_PASTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::V);
const LOCKED_MESSAGE: &str = "Les lignes rapprochées sont verrouillées (menu Opérations)";
const SHORTCUT_CALC: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::C);

/// Action différée en attendant la réponse à « Enregistrer les modifications ? ».
enum Pending {
    Open(Option<PathBuf>),
    NewFile,
    Close,
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
    /// Fichier de comptes à mettre à la corbeille (confirmation).
    confirm_trash: Option<PathBuf>,
    /// Presse-papiers interne des lignes (survit au changement de compte et
    /// de fichier).
    clipboard: Vec<bp_core::model::Operation>,
    /// Ligne dont on veut faire un tiers mémorisé (confirmation).
    confirm_payee: Option<usize>,
    /// Confirmation de l'inversion des monnaies.
    confirm_reverse: bool,
    options: Option<crate::options_ui::OptionsForm>,
    about: bool,
    tool: Option<crate::tools::Tool>,
    command_palette: Option<crate::command_palette::CommandPalette>,
    /// Commandes lancées récemment, la plus récente en premier.
    recent_commands: Vec<Command>,
    error: Option<String>,
    /// Fichier illisible (corrompu) : proposer ses sauvegardes.
    unreadable: Option<(PathBuf, String)>,
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
            confirm_trash: None,
            clipboard: Vec::new(),
            confirm_payee: None,
            confirm_reverse: false,
            options: None,
            about: false,
            tool: None,
            command_palette: None,
            recent_commands: Vec::new(),
            error: None,
            unreadable: None,
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
                let mut dialog = rfd::FileDialog::new()
                    .set_title("Ouvrir un fichier BankPerfect")
                    .add_filter("Comptes BankPerfect", &["bp", "bp0"]);
                if let Some(dir) = &self.settings.files_dir {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(path) = dialog.pick_file() {
                    self.open_path(path);
                }
            }
            Pending::NewFile => {
                self.tool = Some(crate::tools::Tool::NewFile(Default::default()));
            }
            Pending::Close => {
                self.doc = None;
                self.editor = None;
                self.tool = None;
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
            Err(e) => self.unreadable = Some((path, e.to_string())),
        }
    }

    fn finish_open(&mut self, path: PathBuf, data: &[u8], unlock: Unlock<'_>) {
        match file::open_bytes(data, unlock) {
            Ok(opened) => {
                self.show_warnings = !opened.warnings.is_empty();
                self.settings.remember(&path);
                let mut doc = OpenDoc::new(path, opened);
                if self.settings.remember_sort
                    && let Some((column, ascending)) = &self.settings.last_sort
                    && let Some(sort) = sort_from_name(column)
                {
                    doc.sort = sort;
                    doc.ascending = *ascending;
                }
                self.doc = Some(doc);
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
                self.unreadable = Some((path, e.to_string()));
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
        if ctx.input_mut(|i| {
            i.consume_shortcut(&SHORTCUT_PALETTE) || i.consume_key(Modifiers::NONE, Key::F1)
        }) {
            self.execute(ctx, Command::Palette);
            return;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_OPEN)) {
            self.request(Pending::Open(None));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_OPTIONS)) {
            return self.execute(ctx, Command::Options);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_NEW)) {
            return self.execute(ctx, Command::NewFile);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_SAVE_AS)) {
            return self.execute(ctx, Command::SaveAs);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_CLOSE)) {
            return self.execute(ctx, Command::CloseFile);
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

        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_IMPORT)) {
            return self.execute(ctx, Command::Import);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_PRINT)) {
            return self.execute(ctx, Command::Print);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_MARK_DATE)) {
            return self.execute(ctx, Command::SetMarkDate);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_MARK_DATES)) {
            return self.execute(ctx, Command::ToggleMarkDates);
        }
        // Raccourcis de la grille, actifs quand aucun champ n'a le focus.
        if ctx.memory(|m| m.focused().is_some()) || self.menus.is_open() {
            return;
        }
        // Couper / copier / coller des lignes. egui transforme Ctrl+X/C/V en
        // événements presse-papiers : on les intercepte hors des champs.
        let clipboard_event = ctx.input_mut(|i| {
            let mut found = None;
            i.events.retain(|e| match e {
                egui::Event::Cut => {
                    found = Some(Command::CutLines);
                    false
                }
                egui::Event::Copy => {
                    found = Some(Command::CopyLines);
                    false
                }
                egui::Event::Paste(_) => {
                    found = Some(Command::PasteLines);
                    false
                }
                _ => true,
            });
            found
        });
        let shortcut = ctx.input_mut(|i| {
            if i.consume_shortcut(&SHORTCUT_CUT) {
                Some(Command::CutLines)
            } else if i.consume_shortcut(&SHORTCUT_COPY) {
                Some(Command::CopyLines)
            } else if i.consume_shortcut(&SHORTCUT_PASTE) {
                Some(Command::PasteLines)
            } else {
                None
            }
        });
        if let Some(command) = clipboard_event.or(shortcut) {
            return self.execute(ctx, command);
        }
        let Some(doc) = &mut self.doc else { return };
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

    /// Exécute une commande (menus, palette).
    fn execute(&mut self, ctx: &egui::Context, command: Command) {
        use crate::tools::Tool;
        let now = ctx.input(|i| i.time);
        if command != Command::Palette {
            self.recent_commands.retain(|&c| c != command);
            self.recent_commands.insert(0, command);
            self.recent_commands.truncate(5);
        }
        match command {
            Command::Open => return self.request(Pending::Open(None)),
            Command::OpenRecent(i) => {
                if let Some(path) = self.settings.recent_files.get(i).cloned() {
                    self.request(Pending::Open(Some(path)));
                }
                return;
            }
            Command::NewFile => return self.request(Pending::NewFile),
            Command::Options => {
                let theme = ctx.options(|o| o.theme_preference);
                self.options = Some(crate::options_ui::OptionsForm::new(&self.settings, theme));
                return;
            }
            Command::About => return self.about = true,
            Command::Help => {
                ctx.open_url(egui::OpenUrl::new_tab(
                    "https://www.chelly.net/wiki/wiki/manuel",
                ));
                return;
            }
            Command::DeleteFile => return self.delete_file(),
            Command::Quit => return ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Command::Notes => return self.tool = Some(Tool::Notes(crate::tools::NotesForm::new())),
            Command::Calculator => {
                return self.tool = Some(Tool::Calculator(crate::tools::CalcForm::new()));
            }
            Command::Palette => {
                self.menus = menu::MenuBar::default();
                return self.command_palette = Some(Default::default());
            }
            Command::Theme(preference) => {
                ctx.set_theme(preference);
                return self.settings.set_theme(preference);
            }
            Command::ToggleTranslucent => {
                let on = !self.settings.translucent();
                self.settings.translucent = Some(on);
                self.settings.save();
                crate::theme::set_translucent(ctx, on);
                if on && !crate::theme::surface_transparent() {
                    self.toast = Some((
                        "Le fond translucide sera actif au prochain démarrage".into(),
                        now + 5.0,
                    ));
                }
                return;
            }
            Command::Save => return self.save(now),
            Command::SaveAs => return self.save_as(now),
            Command::SaveCopy => return self.save_copy(now),
            Command::CloseFile => return self.request(Pending::Close),
            _ => {}
        }
        let Some(doc) = &mut self.doc else { return };
        match command {
            Command::ExportCsv => self.tool = Some(Tool::ExportCsv(crate::tools::CsvForm::new())),
            Command::Print => self.tool = Some(Tool::Print(Default::default())),
            Command::Import => {
                let mut dialog = rfd::FileDialog::new()
                    .set_title("Fichier d'opérations à importer")
                    .add_filter(
                        "Relevés bancaires",
                        &["ofx", "qfx", "qif", "csv", "tsv", "txt"],
                    );
                if let Some(dir) = &self.settings.export_dir {
                    dialog = dialog.set_directory(dir);
                }
                // BP_IMPORT_FILE : fichier imposé (captures d'écran automatiques).
                let path = std::env::var_os("BP_IMPORT_FILE")
                    .map(PathBuf::from)
                    .or_else(|| dialog.pick_file());
                if let Some(path) = path {
                    match crate::import_ui::ImportForm::new(path) {
                        Ok(form) => self.tool = Some(Tool::Import(Box::new(form))),
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            Command::ExportQif => {
                if let Some(message) = crate::tools::export_qif(doc) {
                    self.toast = Some((message, now + 4.0));
                }
            }
            Command::Protection => self.tool = Some(Tool::Protection(Default::default())),
            Command::Backups => {
                self.tool = Some(Tool::Backups(crate::tools::BackupsForm::new(
                    doc.path.clone(),
                    self.settings.backup_dir(),
                    self.settings.backup_keep,
                )));
            }
            Command::Undo => {
                doc.undo();
                self.editor = None;
            }
            Command::Redo => {
                doc.redo();
                self.editor = None;
            }
            Command::NewOperation => {
                self.editor = Some(Editor::new_operation(&doc.document, doc.account));
            }
            Command::EditOperation => {
                if let Some(&index) = doc.selection.iter().next() {
                    self.editor = Some(Editor::edit(&doc.document, doc.account, index));
                }
            }
            Command::Delete => {
                if doc.selection_locked() {
                    self.toast = Some((LOCKED_MESSAGE.into(), now + 4.0));
                } else {
                    self.confirm_delete = !doc.selection.is_empty();
                }
            }
            Command::Replace => self.tool = Some(Tool::Replace(Default::default())),
            Command::ToggleHideOld => doc.filters.hide_old_lines = !doc.filters.hide_old_lines,
            Command::NextAccount | Command::PreviousAccount => {
                doc.cycle_account(command == Command::NextAccount);
                self.editor = None;
            }
            Command::SelectAccount(i) => {
                doc.select_account(i);
                self.editor = None;
            }
            Command::Manage(tab) => {
                self.tool = Some(Tool::Manage(crate::manage_ui::ManageForm::new(doc, tab)));
            }
            Command::Tack => {
                doc.cycle_tack();
            }
            Command::CutLines => {
                if doc.selection_locked() {
                    self.toast = Some((LOCKED_MESSAGE.into(), now + 4.0));
                } else if !doc.selection.is_empty() {
                    self.clipboard = doc.copy_selection();
                    doc.delete_selection();
                    self.editor = None;
                    self.toast = Some((
                        format!("{} ligne(s) coupée(s)", self.clipboard.len()),
                        now + 3.0,
                    ));
                }
            }
            Command::CopyLines => {
                if !doc.selection.is_empty() {
                    self.clipboard = doc.copy_selection();
                    self.toast = Some((
                        format!("{} ligne(s) copiée(s)", self.clipboard.len()),
                        now + 3.0,
                    ));
                }
            }
            Command::PasteLines => {
                let n = doc.paste(&self.clipboard);
                if n > 0 {
                    self.editor = None;
                    self.toast = Some((format!("{n} ligne(s) collée(s)"), now + 3.0));
                }
            }
            Command::AddToPayees => {
                if let Some(&index) = doc.selection.iter().next() {
                    self.confirm_payee = Some(index);
                }
            }
            Command::SetMarkDate => {
                if doc.selection_locked() {
                    self.toast = Some((LOCKED_MESSAGE.into(), now + 4.0));
                } else if !doc.selection.is_empty() {
                    self.tool = Some(Tool::MarkDate(crate::tools::MarkDateForm::new(doc)));
                }
            }
            Command::ToggleLock => {
                self.settings.lock_reconciled = !self.settings.lock_reconciled;
                self.settings.save();
                doc.locked = self.settings.lock_reconciled;
                doc.max_undo = self.settings.max_undo;
                doc.disable_reconcile = self.settings.disable_reconcile;
                doc.set_ignore_filter_totals(self.settings.ignore_filter_totals);
                if self.settings.remember_sort {
                    let current = (sort_name(doc.sort).to_string(), doc.ascending);
                    if self.settings.last_sort.as_ref() != Some(&current) {
                        self.settings.last_sort = Some(current);
                        self.settings.save();
                    }
                }
            }
            Command::ToggleMarkDates => doc.set_show_mark_dates(!doc.show_mark_dates),
            Command::ToggleBalancePanel => {
                self.settings.show_balance_panel = !self.settings.show_balance_panel;
                self.settings.save();
            }
            Command::ToggleForecast => {
                self.settings.show_forecast = !self.settings.show_forecast;
                self.settings.save();
            }
            Command::ReverseCurrencies => self.confirm_reverse = true,
            Command::ReconcileAll => doc.reconcile_all_pointed(),
            Command::ModifyGroup => {
                self.tool = Some(Tool::ModifyGroup(crate::tools::GroupForm::new()))
            }
            Command::ExtraFields => {
                if let Some(&index) = doc.selection.iter().next() {
                    self.tool = Some(Tool::ExtraValues(crate::tools::ExtraForm::new(doc, index)));
                }
            }
            Command::Open
            | Command::Quit
            | Command::Notes
            | Command::Calculator
            | Command::Palette
            | Command::Theme(_)
            | Command::ToggleTranslucent
            | Command::OpenRecent(_)
            | Command::Options
            | Command::About
            | Command::Help
            | Command::NewFile
            | Command::DeleteFile
            | Command::SaveAs
            | Command::SaveCopy
            | Command::CloseFile
            | Command::Save => {}
        }
    }

    /// Entrée de menu tirée du registre des commandes.
    fn menu_command(&mut self, ui: &mut egui::Ui, command: Command) {
        let info = command.info(self.doc.as_ref());
        if menu::item_enabled(
            ui,
            info.enabled,
            Some(info.icon),
            &info.label,
            info.shortcut,
        )
        .clicked()
        {
            let ctx = ui.ctx().clone();
            self.execute(&ctx, command);
        }
    }

    /// Enregistrer sous (F12) : le fichier enregistré devient le fichier courant.
    fn save_as(&mut self, now: f64) {
        let Some(doc) = &mut self.doc else { return };
        let Some(mut path) = rfd::FileDialog::new()
            .set_title("Enregistrer le fichier de comptes sous…")
            .set_directory(doc.path.parent().unwrap_or(Path::new(".")))
            .set_file_name(doc.file_name())
            .add_filter("Comptes BankPerfect", &["bp"])
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("bp");
        }
        // L'original v5 est gardé à côté de l'ancien emplacement.
        if doc.format == FileFormat::V5
            && let Err(e) = backup_v5_original(&doc.path)
        {
            self.error = Some(format!("Copie de l'original impossible : {e}"));
            return;
        }
        doc.format = FileFormat::V6;
        doc.path = path.clone();
        self.save(now);
        self.settings.remember(&path);
    }

    /// Enregistre une copie (clé USB, disque externe…) sans changer de
    /// fichier courant : `<nom>_AAAAMMJJ.bp` dans le dossier choisi.
    fn save_copy(&mut self, now: f64) {
        let Some(doc) = &mut self.doc else { return };
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Dossier où enregistrer une copie")
            .pick_folder()
        else {
            return;
        };
        let stem = doc
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let target = folder.join(format!(
            "{stem}_{}.bp",
            chrono::Local::now().format("%Y%m%d")
        ));
        let result = ensure_protection(doc).and_then(|recovery| {
            let bytes =
                file::to_bytes(&doc.document, doc.protection.as_ref().expect("protection"))?;
            bp_core::format_v6::write_atomic(&target, &bytes)?;
            Ok(recovery)
        });
        match result {
            Ok(recovery) => {
                if recovery.is_some() {
                    self.recovery_phrase = recovery;
                }
                self.toast = Some((
                    format!("Copie enregistrée : {}", target.display()),
                    now + 5.0,
                ));
            }
            Err(e) => self.error = Some(format!("Copie impossible : {e}")),
        }
    }

    /// Supprimer un fichier de comptes : il part à la corbeille du système.
    fn delete_file(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Fichier de comptes à mettre à la corbeille")
            .add_filter("Comptes BankPerfect", &["bp"]);
        if let Some(dir) = self.settings.files_dir.clone().or_else(|| {
            self.doc
                .as_ref()
                .and_then(|d| d.path.parent().map(Path::to_path_buf))
        }) {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.pick_file() else {
            return;
        };
        let same = |a: &Path, b: &Path| a.canonicalize().ok() == b.canonicalize().ok();
        if self.doc.as_ref().is_some_and(|d| same(&d.path, &path)) {
            self.error = Some("Ce fichier est actuellement ouvert : fermez-le d'abord.".into());
            return;
        }
        self.confirm_trash = Some(path);
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
                "Aide",
            ]
        } else {
            &["Fichier", "Outils", "Aide"]
        };
        let opened = ui
            .horizontal(|ui| {
                let opened = self.menus.titles(ui, titles);
                // Symbole du logiciel en haut à droite, dessiné par-dessus la
                // barre (il déborde un peu sous elle sans la grandir).
                let pal = palette(ui);
                let size = 40.0;
                // Même marge en haut et à droite de la fenêtre.
                let margin = 8.0;
                let window = ui.ctx().content_rect();
                let rect = egui::Rect::from_min_size(
                    egui::pos2(window.right() - margin - size, window.top() + margin),
                    egui::vec2(size, size),
                );
                let painter = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("logo"),
                ));
                crate::logo::paint_symbol(
                    &painter,
                    rect,
                    crate::theme::logo_b(&pal),
                    crate::theme::logo_p(&pal),
                );
                // Mention discrète du nom, à gauche du symbole.
                painter.text(
                    egui::pos2(rect.left() - 4.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "BankPerfect",
                    egui::FontId::new(12.5, crate::theme::medium()),
                    pal.muted,
                );
                opened
            })
            .inner;
        if let Some((index, anchor)) = opened {
            let ctx = ui.ctx().clone();
            let mut menus = std::mem::take(&mut self.menus);
            menus.popup(&ctx, index, anchor, |ui| self.menu_content(ui, index));
            self.menus = menus;
        }
    }

    fn menu_content(&mut self, ui: &mut egui::Ui, index: usize) {
        use Command::*;
        let has_doc = self.doc.is_some();
        let tools_index = if has_doc { 5 } else { 1 };
        if index == tools_index + 1 {
            menu::begin(ui);
            self.menu_command(ui, Palette);
            self.menu_command(ui, Help);
            menu::separator(ui);
            self.menu_command(ui, About);
            return;
        }
        if index == 0 {
            menu::begin(ui);
            self.menu_command(ui, NewFile);
            self.menu_command(ui, Open);
            let recent: Vec<String> = self
                .settings
                .recent_files
                .iter()
                .take(5)
                .map(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                })
                .collect();
            if !recent.is_empty() {
                menu::separator(ui);
                menu::heading(ui, "Fichiers récents");
                for (i, name) in recent.iter().enumerate() {
                    if menu::item(ui, Some(icon::CLOCK), name, None).clicked() {
                        let ctx = ui.ctx().clone();
                        self.execute(&ctx, OpenRecent(i));
                    }
                }
            }
            menu::separator(ui);
            self.menu_command(ui, Save);
            if has_doc {
                self.menu_command(ui, SaveAs);
                self.menu_command(ui, SaveCopy);
                menu::separator(ui);
                self.menu_command(ui, Import);
                self.menu_command(ui, Print);
                self.menu_command(ui, ExportCsv);
                self.menu_command(ui, ExportQif);
                menu::separator(ui);
                self.menu_command(ui, Protection);
                self.menu_command(ui, Backups);
                menu::separator(ui);
                self.menu_command(ui, CloseFile);
            }
            self.menu_command(ui, DeleteFile);
            menu::separator(ui);
            self.menu_command(ui, Quit);
            return;
        }
        if index == tools_index {
            menu::begin(ui);
            self.menu_command(ui, Palette);
            menu::separator(ui);
            self.menu_command(ui, Notes);
            self.menu_command(ui, Calculator);
            if has_doc {
                menu::separator(ui);
                self.menu_command(ui, ReverseCurrencies);
            }
            menu::separator(ui);
            self.menu_command(ui, Options);
            return;
        }
        match index {
            1 => {
                menu::begin(ui);
                self.menu_command(ui, Undo);
                self.menu_command(ui, Redo);
                menu::separator(ui);
                self.menu_command(ui, NewOperation);
                self.menu_command(ui, EditOperation);
                menu::separator(ui);
                self.menu_command(ui, CutLines);
                self.menu_command(ui, CopyLines);
                self.menu_command(ui, PasteLines);
                menu::separator(ui);
                self.menu_command(ui, Delete);
                menu::separator(ui);
                self.menu_command(ui, Replace);
            }
            2 => {
                menu::begin(ui);
                if let Some(doc) = &mut self.doc {
                    menu::toggle(
                        ui,
                        &mut doc.filters.hide_old_lines,
                        "Cacher les lignes inutiles",
                        Some("Ctrl+H"),
                    );
                    let mut marks = doc.show_mark_dates;
                    if menu::toggle(
                        ui,
                        &mut marks,
                        "Afficher les dates de pointage",
                        Some("F11"),
                    )
                    .clicked()
                    {
                        doc.set_show_mark_dates(marks);
                    }
                }
                let mut panel = self.settings.show_balance_panel;
                if menu::toggle(ui, &mut panel, "Panneau des soldes", None).clicked() {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleBalancePanel);
                }
                let mut forecast = self.settings.show_forecast;
                if menu::toggle(ui, &mut forecast, "Solde prévisionnel", None).clicked() {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleForecast);
                }
                menu::separator(ui);
                menu::heading(ui, "Thème");
                let current = ui.ctx().options(|o| o.theme_preference);
                for (preference, label) in [
                    (egui::ThemePreference::Dark, "Tokyo Night"),
                    (egui::ThemePreference::Light, "Tokyo Night Day"),
                    (egui::ThemePreference::System, "Suivre le système"),
                ] {
                    if menu::choice(ui, current == preference, label).clicked() {
                        let ctx = ui.ctx().clone();
                        self.execute(&ctx, Theme(preference));
                    }
                }
                menu::separator(ui);
                let mut translucent = self.settings.translucent();
                if menu::toggle(
                    ui,
                    &mut translucent,
                    "Fond translucide (flou du bureau)",
                    None,
                )
                .clicked()
                {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleTranslucent);
                }
            }
            3 => {
                menu::begin(ui);
                let Some(doc) = &self.doc else { return };
                let accounts: Vec<(usize, String)> = doc
                    .document
                    .account_order
                    .iter()
                    .filter(|&&i| !doc.document.accounts[i].disabled)
                    .map(|&i| (i, doc.document.accounts[i].name.clone()))
                    .collect();
                let current = doc.account;
                for (i, name) in accounts {
                    if menu::choice(ui, i == current, &name).clicked() {
                        let ctx = ui.ctx().clone();
                        self.execute(&ctx, SelectAccount(i));
                    }
                }
                menu::separator(ui);
                self.menu_command(ui, NextAccount);
                self.menu_command(ui, PreviousAccount);
                menu::separator(ui);
                for tab in [
                    crate::manage_ui::Tab::Accounts,
                    crate::manage_ui::Tab::Categories,
                    crate::manage_ui::Tab::Payees,
                    crate::manage_ui::Tab::Modes,
                ] {
                    self.menu_command(ui, Manage(tab));
                }
            }
            _ => {
                menu::begin_with_width(ui, 340.0);
                self.menu_command(ui, Tack);
                self.menu_command(ui, SetMarkDate);
                self.menu_command(ui, ReconcileAll);
                let mut locked = self.settings.lock_reconciled;
                if menu::toggle(ui, &mut locked, "Verrouiller les lignes rapprochées", None)
                    .clicked()
                {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleLock);
                }
                menu::separator(ui);
                self.menu_command(ui, ModifyGroup);
                self.menu_command(ui, ExtraFields);
                self.menu_command(ui, AddToPayees);
            }
        }
    }

    fn balance_strip(ui: &mut egui::Ui, doc: &mut OpenDoc, compact: bool) {
        let pal = palette(ui);
        let filtered = doc.filters.is_active() && !doc.ignore_filter_totals();
        // En-tête compact sur une ligne : nom du compte, banque et IBAN.
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let title = RichText::new(&doc.current_account().name)
                .size(18.0)
                .family(crate::theme::semibold());
            if compact {
                // Sans panneau des comptes : le nom sert de sélecteur.
                let mut selected = None;
                egui::ComboBox::from_id_salt("account_switcher")
                    .selected_text(title)
                    .height(420.0)
                    .show_ui(ui, |ui| {
                        for &i in &doc.document.account_order {
                            let a = &doc.document.accounts[i];
                            if a.disabled {
                                continue;
                            }
                            let label = format!(
                                "{}    {}",
                                a.name,
                                money(doc.account_balance(i), &a.currency)
                            );
                            if ui.selectable_label(i == doc.account, label).clicked() {
                                selected = Some(i);
                            }
                        }
                    });
                if let Some(i) = selected {
                    doc.select_account(i);
                }
            } else {
                ui.label(title);
            }
            let account = doc.current_account();
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
                ui.add_space(4.0);
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
        if doc.show_balance_panel {
            crate::dashboard::cards(ui, doc);
        }
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
                ui.label(RichText::new("Enregistré").small().color(pal.muted));
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
                let locked =
                    doc.locked && target.is_some_and(|i| doc.operation(i).tack == Tack::Reconciled);
                if locked {
                    self.toast = Some((LOCKED_MESSAGE.into(), now + 4.0));
                } else {
                    let date = op.date.to_string();
                    doc.upsert_operation(target, op);
                    // Saisie en série : formulaire vidé, même date.
                    self.editor = (target.is_none() && self.settings.keep_entry_open)
                        .then(|| Editor::new_operation(&doc.document, doc.account).with_date(date));
                }
            }
        }
    }

    fn modals(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if let Some(options) = &mut self.options {
            use crate::options_ui::OptionsOutcome;
            match options.show(ctx) {
                OptionsOutcome::Keep => {}
                OptionsOutcome::Cancel => self.options = None,
                OptionsOutcome::Apply(settings, theme) => {
                    self.options = None;
                    let translucent_before = self.settings.translucent();
                    self.settings = *settings;
                    self.settings.save();
                    ctx.set_theme(theme);
                    self.settings.set_theme(theme);
                    if self.settings.translucent() != translucent_before {
                        crate::theme::set_translucent(ctx, self.settings.translucent());
                        if self.settings.translucent() && !crate::theme::surface_transparent() {
                            self.toast = Some((
                                "Le fond translucide sera actif au prochain démarrage".into(),
                                now + 5.0,
                            ));
                        }
                    }
                    if self.toast.is_none() {
                        self.toast = Some(("Options enregistrées".into(), now + 3.0));
                    }
                }
            }
        }
        if self.about {
            dialogs::modal(ctx, "about").show(ctx, |ui| {
                ui.set_width(440.0);
                let pal = palette(ui);
                ui.vertical_centered(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
                    crate::logo::paint_symbol(
                        ui.painter(),
                        rect,
                        crate::theme::logo_b(&pal),
                        crate::theme::logo_p(&pal),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("BankPerfect")
                            .size(26.0)
                            .family(crate::theme::semibold()),
                    );
                    ui.label(
                        RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                            .color(pal.muted),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!("{}  Enregistré", icon::SEAL_CHECK))
                            .color(pal.positive),
                    );
                    ui.add_space(10.0);
                    if let Ok(exe) = std::env::current_exe() {
                        ui.label(
                            RichText::new(format!("Emplacement : {}", exe.display()))
                                .size(12.0)
                                .color(pal.muted),
                        );
                    }
                });
                dialogs::footer(ui, |ui| {
                    if dialogs::primary(ui, "Fermer", true).clicked()
                        || ui.input(|i| {
                            i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter)
                        })
                    {
                        self.about = false;
                    }
                });
            });
        }
        if let Some(palette) = &mut self.command_palette {
            use crate::command_palette::PaletteResult;
            match palette.show(ctx, self.doc.as_ref(), &self.recent_commands) {
                PaletteResult::Keep => {}
                PaletteResult::Close => self.command_palette = None,
                PaletteResult::Run(command) => {
                    self.command_palette = None;
                    self.execute(ctx, command);
                }
            }
        }
        if let Some(tool) = &mut self.tool {
            let result = match (&mut self.doc, tool) {
                (_, crate::tools::Tool::Notes(form)) => Some(form.show_standalone(ctx)),
                (_, crate::tools::Tool::Calculator(form)) => Some(form.show_standalone(ctx)),
                (doc, crate::tools::Tool::NewFile(form)) => Some(form.show_standalone(
                    ctx,
                    self.settings.files_dir.clone().or_else(|| {
                        doc.as_ref()
                            .and_then(|d| d.path.parent().map(Path::to_path_buf))
                    }),
                )),
                (doc, crate::tools::Tool::Backups(form)) => {
                    Some(form.show_standalone(ctx, doc.as_ref().is_some_and(|d| d.dirty)))
                }
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
            match dialogs::confirm(
                ctx,
                "delete",
                icon::TRASH,
                "Supprimer",
                &message,
                "Supprimer",
                None,
                true,
            ) {
                Choice::Pending => {}
                Choice::Yes => {
                    doc.delete_selection();
                    self.editor = None;
                    self.confirm_delete = false;
                }
                Choice::No | Choice::Cancel => self.confirm_delete = false,
            }
        }
        if let Some(index) = self.confirm_payee
            && let Some(doc) = &mut self.doc
        {
            let op = doc.operation(index).clone();
            let message = format!(
                "Créer le tiers mémorisé « {} » ? La saisie le proposera avec ce mode, ce détail et cette catégorie.",
                op.payee
            );
            match dialogs::confirm(
                ctx,
                "add_payee",
                icon::USER_PLUS,
                "Ajouter aux tiers mémorisés",
                &message,
                "Créer le tiers",
                None,
                false,
            ) {
                Choice::Pending => {}
                Choice::Yes => {
                    self.confirm_payee = None;
                    let payee = bp_core::model::Payee {
                        caption: op.payee.clone(),
                        mode: op.mode.clone(),
                        category: op.category,
                        payee: op.payee.clone(),
                        details: op.details.clone(),
                        amount: String::new(),
                    };
                    let index = doc.edit_document(|d| {
                        d.payees.push(payee);
                        d.payees.len() - 1
                    });
                    self.tool = Some(crate::tools::Tool::Manage(
                        crate::manage_ui::ManageForm::new(doc, crate::manage_ui::Tab::Payees)
                            .with_payee(index),
                    ));
                }
                Choice::No | Choice::Cancel => self.confirm_payee = None,
            }
        }
        if self.confirm_reverse
            && let Some(doc) = &mut self.doc
        {
            let account = doc.current_account();
            let message = format!(
                "Toutes les opérations du compte « {} » vont être converties de {} en {} (taux {} → {}). L'historique des annulations sera vidé.",
                account.name,
                account.currency,
                account.secondary_currency,
                account.currency_rate.to_string().replace('.', ","),
                account.secondary_rate.to_string().replace('.', ",")
            );
            match dialogs::confirm(
                ctx,
                "reverse_currencies",
                icon::ARROWS_LEFT_RIGHT,
                "Inverser les monnaies",
                &message,
                "Convertir",
                None,
                false,
            ) {
                Choice::Pending => {}
                Choice::Yes => {
                    self.confirm_reverse = false;
                    let index = doc.account;
                    doc.edit_document(|d| bp_core::manage::reverse_currencies(d, index));
                    let currency = doc.current_account().currency.clone();
                    self.toast = Some((format!("Compte converti en {currency}"), now + 4.0));
                }
                Choice::No | Choice::Cancel => self.confirm_reverse = false,
            }
        }
        if let Some(path) = self.confirm_trash.clone() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let message = format!(
                "Mettre « {name} » à la corbeille ? Vous pourrez le récupérer depuis la corbeille du système."
            );
            match dialogs::confirm(
                ctx,
                "trash",
                icon::TRASH,
                "Supprimer un fichier de comptes",
                &message,
                "Mettre à la corbeille",
                None,
                true,
            ) {
                Choice::Pending => {}
                Choice::Yes => {
                    self.confirm_trash = None;
                    match trash::delete(&path) {
                        Ok(()) => {
                            self.settings.recent_files.retain(|f| f != &path);
                            self.settings.save();
                            self.toast = Some((format!("{name} mis à la corbeille"), now + 4.0));
                        }
                        Err(e) => self.error = Some(format!("Suppression impossible : {e}")),
                    }
                }
                Choice::No | Choice::Cancel => self.confirm_trash = None,
            }
        }
        if let Some((path, reason)) = self.unreadable.clone() {
            let backups = bp_core::backup::list(&path, &self.settings.backup_dir());
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut close = false;
            dialogs::modal(ctx, "unreadable").show(ctx, |ui| {
                ui.set_width(520.0);
                let pal = palette(ui);
                dialogs::header(ui, icon::WARNING, pal.negative, "Fichier illisible");
                ui.label(
                    RichText::new(format!("« {name} » ne peut pas être ouvert : il semble endommagé."))
                        .color(pal.fg_dark),
                );
                ui.label(RichText::new(reason).small().color(pal.muted));
                if !backups.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{} sauvegarde(s) de ce fichier sont disponibles ; la plus récente date du {}.",
                            backups.len(),
                            chrono::DateTime::<chrono::Local>::from(backups[0].modified).format("%d/%m/%Y à %H:%M")
                        ))
                        .color(pal.fg_dark),
                    );
                }
                dialogs::footer(ui, |ui| {
                    if !backups.is_empty() && dialogs::primary(ui, "Voir les sauvegardes", true).clicked() {
                        self.tool = Some(crate::tools::Tool::Backups(crate::tools::BackupsForm::new(
                            path.clone(),
                            self.settings.backup_dir(),
                            self.settings.backup_keep,
                        )));
                        close = true;
                    }
                    if dialogs::button(ui, "Fermer").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close = true;
                    }
                });
            });
            if close {
                self.unreadable = None;
            }
        }
        if self.pending.is_some() {
            let name = self.doc.as_ref().map(|d| d.file_name()).unwrap_or_default();
            let message = format!(
                "« {name} » contient des modifications non enregistrées. Voulez-vous les enregistrer avant de continuer ?"
            );
            match dialogs::confirm(
                ctx,
                "unsaved",
                icon::FLOPPY_DISK,
                "Enregistrer les modifications ?",
                &message,
                "Enregistrer",
                Some("Ne pas enregistrer"),
                false,
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
    /// Fond transparent si la fenêtre l'est (les panneaux peignent le fond
    /// translucide par-dessus), sinon la couleur du thème.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if crate::theme::surface_transparent() {
            [0.0; 4]
        } else {
            visuals.panel_fill.to_normalized_gamma_f32()
        }
    }

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
            let statement = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase)
                .is_some_and(|e| matches!(e.as_str(), "ofx" | "qfx" | "qif" | "csv" | "tsv"));
            if statement && self.doc.is_some() {
                // Relevé bancaire déposé : import dans le fichier ouvert.
                match crate::import_ui::ImportForm::new(path) {
                    Ok(form) => self.tool = Some(crate::tools::Tool::Import(Box::new(form))),
                    Err(e) => self.error = Some(e),
                }
            } else {
                self.request(Pending::Open(Some(path)));
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.doc.as_ref().is_some_and(|d| d.dirty)
        {
            if self.settings.autosave_on_close {
                // Option « enregistrer à la fermeture » : sans question.
                let now = ctx.input(|i| i.time);
                self.save(now);
            }
            if self.doc.as_ref().is_some_and(|d| d.dirty) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.pending = Some(Pending::Quit);
            }
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
        crate::commands::set_recent(&self.settings.recent_files);
        crate::settings::shared::set(&self.settings);
        if (ctx.zoom_factor() - self.settings.text_scale).abs() > 0.001 {
            ctx.set_zoom_factor(self.settings.text_scale);
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

        let sidebar_fill = crate::theme::backdrop(
            palette(ui).bg_dark,
            crate::theme::sidebar_alpha(ui.visuals().dark_mode),
        );
        // Fenêtre étroite (tuile) : pas de panneau des comptes, le nom du
        // compte devient un sélecteur.
        let compact = ui.available_width() < 860.0;
        // Panneau des comptes plus étroit sur les petits écrans.
        let sidebar_width = (ui.available_width() * 0.2).clamp(190.0, 260.0);
        if !compact {
            egui::Panel::left("accounts")
                .resizable(true)
                .default_size(sidebar_width)
                .min_size(170.0)
                .frame(
                    egui::Frame::side_top_panel(ui.style())
                        .fill(sidebar_fill)
                        .inner_margin(12),
                )
                .show(ui, |ui| sidebar::show(ui, doc));
        }

        // Zone à droite de la barre latérale : le tiroir de saisie s'y superpose.
        let content_rect = ui.available_rect_before_wrap();

        // Réglages d'affichage et de verrou repris à chaque image.
        doc.locked = self.settings.lock_reconciled;
        doc.show_balance_panel = self.settings.show_balance_panel;
        doc.show_forecast = self.settings.show_forecast;
        if let Some(notice) = doc.notice.take() {
            self.toast = Some((notice, now + 4.0));
        }
        crate::commands::set_clipboard_len(self.clipboard.len());
        doc.refresh();
        let mut grid_action = GridAction::None;
        let mut deferred = None;
        egui::Panel::top("balances")
            .frame(
                egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::symmetric(18, 0)),
            )
            .show(ui, |ui| Self::balance_strip(ui, doc, compact));

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
            GridAction::Command(command) => deferred = Some(command),
        }

        self.editor_drawer(&ctx, content_rect, now);
        if let Some(command) = deferred {
            self.execute(&ctx, command);
        }
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

/// Bouton rond « Nouvelle opération », flottant en bas à droite de la
/// grille : effet givré plat, sombre et semi-transparent, croix blanche.
fn floating_add_button(ctx: &egui::Context, content: egui::Rect) -> bool {
    const SIZE: f32 = 54.0;
    // En bas à droite de la grille, au-dessus des lignes (bouton flottant),
    // à côté de la barre de défilement.
    let center = egui::pos2(content.right() - 62.0, content.bottom() - 52.0);
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
                offset: [0, 6],
                blur: 22,
                spread: 0,
                color: egui::Color32::from_black_alpha(90),
            };
            painter.add(shadow.as_shape(rect, egui::CornerRadius::same(radius as u8)));
            painter.circle_filled(
                rect.center(),
                radius,
                egui::Color32::from_rgba_unmultiplied(0x16, 0x17, 0x20, 228),
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
                egui::FontId::proportional(25.0),
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
    let recovery = ensure_protection(doc)?;
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

/// Choisit la protection v6 au premier enregistrement : chiffrée avec le mot
/// de passe du fichier v5 s'il en avait un (la clé de secours est renvoyée).
fn ensure_protection(doc: &mut OpenDoc) -> bp_core::Result<Option<String>> {
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

fn sort_name(sort: bp_core::view::SortColumn) -> &'static str {
    use bp_core::view::SortColumn::*;
    match sort {
        Date => "date",
        MarkDate => "pointage",
        Mode => "mode",
        Payee => "tiers",
        Category => "categorie",
        Amount => "montant",
    }
}

fn sort_from_name(name: &str) -> Option<bp_core::view::SortColumn> {
    use bp_core::view::SortColumn::*;
    Some(match name {
        "date" => Date,
        "pointage" => MarkDate,
        "mode" => Mode,
        "tiers" => Payee,
        "categorie" => Category,
        "montant" => Amount,
        _ => return None,
    })
}
