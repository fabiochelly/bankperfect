//! Fenêtre principale : menus, mise en page, ouverture/enregistrement.

use std::path::{Path, PathBuf};

use bp_core::FileFormat;
use bp_core::file;
use bp_core::format_v6::{Protection, Unlock};
use bp_core::model::{Amount, Tack};
use bp_core::{tr, trn};
use egui::{Align, Key, KeyboardShortcut, Layout, Modifiers, RichText};
use egui_phosphor::regular as icon;

use crate::budget_ui::BudgetTab;
use crate::commands::Command;
use crate::dialogs::{self, Choice, PasswordPrompt, PromptResult};
use crate::document::{CursorTarget, OpenDoc};
use crate::editor::{Editor, EditorAction};
use crate::format::money;
use crate::grid::{self, GridAction};
use crate::reports_ui::ReportTab;
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

/// Symbole du logiciel en haut à droite : côté et marge au bord de la fenêtre.
const LOGO_SIZE: f32 = 34.0;
const LOGO_MARGIN: f32 = 6.0;

/// Haut des îles (comptes, soldes) : juste sous le symbole, qui déborde de
/// la barre de menus.
fn islands_top(ctx: &egui::Context) -> f32 {
    ctx.content_rect().top() + LOGO_MARGIN + LOGO_SIZE + 4.0
}
const SHORTCUT_NOTES: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::N);
const SHORTCUT_RULES: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
const SHORTCUT_CATEGORIZE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::J);
const SHORTCUT_CATEGORY_TOTALS: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND, Key::T);
const SHORTCUT_CHARTS: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::G);
const SHORTCUT_BUDGET: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);
const SHORTCUT_CURRENCIES: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
const SHORTCUT_LOAN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::L);
const SHORTCUT_TRANSFER: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::T);
const SHORTCUT_SPLIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F6);
const SHORTCUT_FILTERS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F7);
const SHORTCUT_SCHEDULER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
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
/// Message des lignes rapprochées verrouillées.
fn locked_message() -> String {
    tr!("Les lignes rapprochées sont verrouillées (menu Opérations)").to_string()
}
const SHORTCUT_CALC: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::C);

/// Raccourcis de l'application : une extension ne peut pas les prendre.
const BUILTIN_SHORTCUTS: &[KeyboardShortcut] = &[
    SHORTCUT_OPEN,
    SHORTCUT_SAVE,
    SHORTCUT_NEXT_ACCOUNT,
    SHORTCUT_PREV_ACCOUNT,
    SHORTCUT_HIDE_OLD,
    SHORTCUT_UNDO,
    SHORTCUT_REDO,
    SHORTCUT_REDO_ALT,
    SHORTCUT_REPLACE,
    SHORTCUT_GROUP,
    SHORTCUT_EXTRA,
    SHORTCUT_ACCOUNTS,
    SHORTCUT_PAYEES,
    SHORTCUT_NOTES,
    SHORTCUT_RULES,
    SHORTCUT_CATEGORIZE,
    SHORTCUT_CATEGORY_TOTALS,
    SHORTCUT_CHARTS,
    SHORTCUT_BUDGET,
    SHORTCUT_CURRENCIES,
    SHORTCUT_LOAN,
    SHORTCUT_TRANSFER,
    SHORTCUT_SPLIT,
    SHORTCUT_FILTERS,
    SHORTCUT_SCHEDULER,
    SHORTCUT_IMPORT,
    SHORTCUT_PRINT,
    SHORTCUT_OPTIONS,
    SHORTCUT_NEW,
    SHORTCUT_SAVE_AS,
    SHORTCUT_CLOSE,
    SHORTCUT_PALETTE,
    SHORTCUT_MARK_DATE,
    SHORTCUT_MARK_DATES,
    SHORTCUT_CUT,
    SHORTCUT_COPY,
    SHORTCUT_PASTE,
    SHORTCUT_CALC,
];

/// Action différée en attendant la réponse à « Enregistrer les modifications ? ».
enum Pending {
    Open(Option<PathBuf>),
    NewFile,
    /// Importer un export JSON dans un nouveau fichier.
    ImportJson,
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
    /// Modification d'une ligne de transfert en attente : quelles lignes ?
    confirm_linked_edit: Option<(usize, bp_core::model::Operation)>,
    /// Fichier de comptes à mettre à la corbeille (confirmation).
    confirm_trash: Option<PathBuf>,
    /// Presse-papiers interne des lignes (survit au changement de compte et
    /// de fichier).
    clipboard: Vec<bp_core::model::Operation>,
    /// Ligne dont on veut faire un tiers mémorisé (confirmation).
    confirm_payee: Option<usize>,
    /// Confirmation de l'inversion des monnaies.
    confirm_reverse: bool,
    /// Export JSON (non chiffré) d'un fichier protégé : à confirmer.
    confirm_json_export: bool,
    /// Résumé des échéances insérées automatiquement à l'ouverture.
    scheduler_notice: Option<String>,
    options: Option<crate::options_ui::OptionsForm>,
    about: bool,
    /// Licence active (certificat des réglages, vérifié au démarrage).
    license: Option<crate::license::License>,
    license_form: Option<crate::license_ui::LicenseForm>,
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
    /// Une composition est en cours dans la méthode de saisie (pré-saisie
    /// non vide), voir `raw_input_hook`.
    ime_composing: bool,
    settings: crate::settings::Settings,
    /// Le thème des réglages reste à appliquer (première image).
    theme_pending: bool,
    /// Vraie application (pas un test) : notes affichées à l'ouverture.
    interactive: bool,
    /// Première image pas encore peinte.
    first_frame: bool,
    /// Extensions Luau chargées.
    extensions: bp_ext::Engine,
    ext_manager: Option<crate::extensions::Manager>,
    /// Erreurs des extensions à montrer.
    ext_errors: Vec<String>,
    /// Fichier, compte affiché et sélection vus par les extensions
    /// (évènements `account_selected` et `selection_changed`).
    ext_watch: Option<(u64, usize, std::collections::BTreeSet<usize>)>,
    /// Contexte egui, pour ouvrir un sélecteur de fichiers hors d'une image.
    egui_ctx: egui::Context,
    /// Suite d'une commande qui attend le choix d'un fichier (voir `ask`).
    pick_then: Option<PickThen>,
}

/// Suite d'une commande, appelée avec le fichier choisi.
type PickThen = Box<dyn FnOnce(&mut BankPerfectApp, PathBuf)>;

const DRAWER_WIDTH: f32 = 460.0;
const DRAWER_SLIDE_SECONDS: f64 = 0.15;

impl BankPerfectApp {
    /// Application avec les réglages enregistrés de l'utilisateur.
    pub fn new(initial_file: Option<PathBuf>) -> Self {
        let mut app = Self::with_settings(None, crate::settings::Settings::load());
        app.theme_pending = true;
        app.interactive = true;
        app.load_extensions(crate::extensions::user_dir());
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
        crate::settings::shared::set(&settings);
        let mut app = Self {
            doc: None,
            editor: None,
            prompt: None,
            recovery_phrase: None,
            show_warnings: false,
            pending: None,
            confirm_delete: false,
            confirm_linked_edit: None,
            confirm_trash: None,
            clipboard: Vec::new(),
            confirm_payee: None,
            confirm_reverse: false,
            confirm_json_export: false,
            scheduler_notice: None,
            options: None,
            about: false,
            license: settings.license.as_deref().and_then(crate::license::verify),
            license_form: None,
            tool: None,
            command_palette: None,
            recent_commands: Vec::new(),
            error: None,
            unreadable: None,
            toast: None,
            allow_close: false,
            egui_ctx: egui::Context::default(),
            pick_then: None,
            title: String::new(),
            drawer_open: false,
            drawer_opened_at: 0.0,
            menus: menu::MenuBar::default(),
            profiler: crate::profiler::Profiler::from_env(),
            ime_composing: false,
            settings,
            theme_pending: false,
            interactive: false,
            first_frame: true,
            extensions: crate::extensions::engine(None, &[]),
            ext_manager: None,
            ext_errors: Vec::new(),
            ext_watch: None,
        };
        if let Some(path) = initial_file {
            app.open_path(path);
        }
        app
    }

    /// Ouvre un fichier de comptes (tests).
    pub fn open_file(&mut self, path: PathBuf) {
        self.open_path(path);
    }

    /// Charge les extensions d'un dossier (application : dossier de
    /// l'utilisateur ; tests : exemples).
    /// Applique la langue des réglages ; les extensions sont rechargées pour
    /// que `bp.language` et les noms qu'elles en tirent suivent.
    fn language_changed(&mut self) {
        let before = bp_core::i18n::lang();
        crate::settings::shared::set(&self.settings);
        if bp_core::i18n::lang() != before {
            self.extensions.load_all(&self.settings.disabled_extensions);
            crate::extensions::refresh(&self.extensions);
        }
    }

    pub fn load_extensions(&mut self, dir: PathBuf) {
        self.extensions = crate::extensions::engine(Some(dir), &self.settings.disabled_extensions);
        crate::extensions::refresh(&self.extensions);
    }

    /// Lance une commande d'extension.
    fn run_extension(&mut self, ctx: &egui::Context, index: usize) {
        let Some(entry) = crate::extensions::entry(index) else {
            return;
        };
        let now = ctx.input(|i| i.time);
        if entry.needs == bp_ext::Needs::Selection
            && self.doc.as_ref().is_none_or(|d| d.selection.is_empty())
        {
            self.toast = Some((tr!("Sélectionnez d'abord des lignes").into(), now + 3.0));
            return;
        }
        let mut context = self.doc.as_mut().map(|d| d.extension_context(None));
        let fx = self
            .extensions
            .run_command(entry.ext, entry.command, context.as_mut());
        drop(context);
        self.apply_effects(fx, now);
    }

    /// Prévient les extensions d'un évènement ; `merge` : leurs modifications
    /// rejoignent la dernière étape d'annulation (l'action qui a déclenché).
    fn fire_extensions(
        &mut self,
        event: bp_ext::Event,
        lines: &[bp_ext::EventItem],
        merge: bool,
        now: f64,
    ) {
        if !self.extensions.has_hooks(event) {
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        let tag = if merge { doc.last_undo_tag() } else { None };
        let fx = self
            .extensions
            .fire(event, lines, doc.extension_context(tag));
        self.apply_effects(fx, now);
    }

    /// Ouvre l'aperçu d'import d'un fichier : lu par le format d'une
    /// extension qui le reconnaît, sinon par l'import de BankPerfect.
    fn open_import(&mut self, path: PathBuf) {
        let Some(doc) = &mut self.doc else { return };
        if !self.extensions.importers().is_empty() {
            let data = match std::fs::read(&path) {
                Ok(data) => data,
                Err(e) => {
                    self.error = Some(tr!("Lecture impossible : {}", e));
                    return;
                }
            };
            let read = self
                .extensions
                .import_file(&path, &data, &mut doc.extension_context(None));
            match read {
                Some((name, Ok(parsed))) => {
                    let form =
                        crate::import_ui::ImportForm::from_extension(path, name, parsed, doc);
                    self.tool = Some(crate::tools::Tool::Import(Box::new(form)));
                    return;
                }
                Some((_, Err(e))) => {
                    self.ext_errors.push(e);
                    return;
                }
                None => {}
            }
        }
        match crate::import_ui::ImportForm::new(path) {
            Ok(form) => self.tool = Some(crate::tools::Tool::Import(Box::new(form))),
            Err(e) => self.error = Some(e),
        }
    }

    /// Évènements des extensions en attente : ceux mis de côté par le
    /// document, opérations touchées par les actions de l'utilisateur,
    /// changement de compte et de sélection.
    fn extension_events(&mut self, now: f64) {
        let Some(doc) = &mut self.doc else {
            self.ext_watch = None;
            return;
        };
        let mut events = std::mem::take(&mut doc.ext_events);
        let wanted = [
            bp_ext::Event::OperationAdded,
            bp_ext::Event::OperationChanged,
            bp_ext::Event::OperationDeleted,
        ]
        .into_iter()
        .any(|e| self.extensions.has_hooks(e));
        events.extend(doc.take_operation_events(wanted));
        let selection = if self.extensions.has_hooks(bp_ext::Event::SelectionChanged) {
            doc.selection.clone()
        } else {
            Default::default()
        };
        let watch = (doc.file_id, doc.account, selection);
        match &self.ext_watch {
            Some((file, account, selection)) if *file == watch.0 => {
                if *account != watch.1 {
                    events.push((
                        bp_ext::Event::AccountSelected,
                        vec![bp_ext::EventItem::line(watch.1, 0)],
                        false,
                    ));
                }
                if *selection != watch.2 && !(selection.is_empty() && watch.2.is_empty()) {
                    events.push((bp_ext::Event::SelectionChanged, Vec::new(), false));
                }
            }
            _ => {}
        }
        self.ext_watch = Some(watch);
        for (event, items, merge) in events {
            self.fire_extensions(event, &items, merge, now);
        }
    }

    fn apply_effects(&mut self, fx: bp_ext::Effects, now: f64) {
        if let Some(doc) = &mut self.doc
            && (fx.modified() || fx.select.is_some() || fx.filters.is_some())
        {
            doc.apply_extension(&fx);
        }
        if let Some(toast) = fx.toasts.last() {
            self.toast = Some((toast.clone(), now + 4.0));
        }
        self.ext_errors.extend(fx.errors);
    }

    /// Fenêtres des extensions, fenêtre de gestion et erreurs.
    fn extension_modals(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.extensions.has_frames() {
            let frames: Vec<bp_ext::FrameMut<'_>> = self.extensions.frames_mut().collect();
            let reply = crate::ext_ui::show(ctx, frames, self.doc.as_ref());
            if let Some(reply) = reply {
                let mut context = self.doc.as_mut().map(|d| d.extension_context(None));
                let fx = match reply {
                    crate::ext_ui::Reply::Answer(a) => self.extensions.answer(a, context.as_mut()),
                    crate::ext_ui::Reply::Event(e) => {
                        self.extensions.window_event(e, context.as_mut())
                    }
                };
                drop(context);
                self.apply_effects(fx, now);
                ctx.request_repaint();
            }
        }
        if let Some(manager) = &mut self.ext_manager {
            use crate::extensions::ManagerAction;
            match manager.show(ctx, &self.extensions) {
                ManagerAction::None => {}
                ManagerAction::Close => self.ext_manager = None,
                ManagerAction::Toggle(index, enabled) => {
                    self.extensions.set_enabled(index, enabled);
                    let id = self.extensions.extensions[index].id.clone();
                    self.settings.disabled_extensions.retain(|d| *d != id);
                    if !enabled {
                        self.settings.disabled_extensions.push(id);
                    }
                    self.settings.save();
                    crate::extensions::refresh(&self.extensions);
                }
                ManagerAction::Reload => {
                    self.extensions.load_all(&self.settings.disabled_extensions);
                    crate::extensions::refresh(&self.extensions);
                    self.toast = Some((tr!("Extensions rechargées").into(), now + 3.0));
                }
                ManagerAction::OpenFolder => {
                    let _ = std::fs::create_dir_all(self.extensions.dir());
                    let _ = open::that_detached(self.extensions.dir());
                }
                ManagerAction::InstallExamples => {
                    let n = crate::extensions::install_examples(self.extensions.dir());
                    self.extensions.load_all(&self.settings.disabled_extensions);
                    crate::extensions::refresh(&self.extensions);
                    self.toast = Some((
                        match n {
                            0 => tr!("Les exemples sont déjà installés").to_string(),
                            n => trn!(n, "{} exemple installé", "{} exemples installés"),
                        },
                        now + 4.0,
                    ));
                }
                ManagerAction::Docs => {
                    ctx.open_url(egui::OpenUrl::new_tab(crate::extensions::docs_url()));
                }
            }
        }
        if !self.ext_errors.is_empty()
            && dialogs::alert(
                ctx,
                tr!("Erreur dans une extension"),
                &self.ext_errors.join("\n\n"),
            )
        {
            self.ext_errors.clear();
        }
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
                    .set_title(tr!("Ouvrir un fichier BankPerfect"))
                    .add_filter(tr!("Comptes BankPerfect"), &["bp", "bp0"]);
                if let Some(dir) = &self.settings.files_dir {
                    dialog = dialog.set_directory(dir);
                }
                self.ask(crate::file_picker::Kind::File, dialog, Self::open_path);
            }
            Pending::NewFile => {
                self.tool = Some(crate::tools::Tool::NewFile(Default::default()));
            }
            Pending::ImportJson => self.import_json(),
            Pending::Close => {
                self.doc = None;
                self.editor = None;
                self.tool = None;
                self.extensions.abandon();
            }
            Pending::Quit => self.allow_close = true,
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        let data = match std::fs::read(&path) {
            Ok(data) => data,
            Err(e) => {
                return self.error = Some(tr!("Impossible de lire {} : {}", path.display(), e));
            }
        };
        // Ouverture directe : un fichier protégé répond `PasswordRequired`
        // (et un fichier v5 n'est lu qu'une fois).
        self.finish_open(path, &data, Unlock::None);
    }

    fn finish_open(&mut self, path: PathBuf, data: &[u8], unlock: Unlock<'_>) {
        match file::open_bytes(data, unlock) {
            Ok(mut opened) => {
                // Échéances à insertion automatique arrivées à terme.
                let inserted = bp_core::scheduler::auto_insert(
                    &mut opened.document,
                    bp_core::model::Date::today(),
                )
                .ok()
                .filter(|r| !r.inserted.is_empty())
                .map(|r| {
                    let schedules = bp_core::scheduler::load(&opened.document)
                        .map(|l| l.schedules)
                        .unwrap_or_default();
                    let lines: Vec<(usize, u64)> =
                        r.inserted.iter().map(|l| (l.account, l.uid)).collect();
                    (r.summary(&opened.document, &schedules), lines)
                });
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
                if let Some((summary, lines)) = inserted {
                    // Règles automatiques sur les échéances insérées.
                    let changes: Vec<(usize, usize)> = lines
                        .iter()
                        .filter_map(|&(account, uid)| {
                            let ops = &doc.document.accounts.get(account)?.operations;
                            Some((account, ops.iter().position(|op| op.uid == uid)?))
                        })
                        .collect();
                    doc.apply_auto_rules(&changes);
                    doc.dirty = true;
                    self.scheduler_notice = Some(summary);
                }
                doc.ext_events
                    .push((bp_ext::Event::FileOpened, Vec::new(), false));
                self.extensions.abandon();
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
            Err(bp_core::Error::PasswordRequired) => {
                self.prompt = Some(PasswordPrompt {
                    can_use_recovery_key: bp_core::format_v6::is_v6(data),
                    path,
                    input: String::new(),
                    show_password: false,
                    use_recovery_key: false,
                    error: None,
                })
            }
            Err(bp_core::Error::WrongPassword) => {
                if let Some(prompt) = &mut self.prompt {
                    prompt.error = Some(tr!("Mot de passe ou clé de secours incorrect").into());
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
        self.fire_extensions(bp_ext::Event::BeforeSave, &[], false, now);
        let Some(doc) = &mut self.doc else { return };
        if let Err(e) = bp_core::backup::backup_before_save(
            &doc.path,
            &self.settings.backup_dir(),
            self.settings.backup_keep,
        ) {
            self.error = Some(tr!(
                "Sauvegarde .bp0 impossible : {}. Fichier non enregistré.",
                e
            ));
            return;
        }
        match save_document(doc) {
            Ok(recovery) => {
                self.recovery_phrase = recovery;
                self.toast = Some((tr!("{} enregistré", doc.file_name()), now + 3.0));
                doc.ext_events
                    .push((bp_ext::Event::AfterSave, Vec::new(), false));
            }
            Err(e) => self.error = Some(tr!("Enregistrement impossible : {}", e)),
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
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_CURRENCIES)) {
            return self.execute(ctx, Command::Currencies);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_LOAN)) {
            return self.execute(ctx, Command::Loan);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_CALC)) {
            self.tool = Some(crate::tools::Tool::Calculator(crate::tools::CalcForm::new()));
        }
        if self.doc.is_some() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F8)) {
            return self.execute(ctx, Command::FocusEntry);
        }
        if self.doc.is_some() && ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Enter)) {
            return self.execute(ctx, Command::ModifySelected);
        }
        for (index, entry) in crate::extensions::entries().iter().enumerate() {
            if let Some(keys) = entry.keys
                && !BUILTIN_SHORTCUTS.contains(&keys)
                && ctx.input_mut(|i| i.consume_shortcut(&keys))
            {
                return self.execute(ctx, Command::Extension(index));
            }
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
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_CHARTS)) {
            return self.execute(ctx, Command::Charts);
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
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_TRANSFER)) {
            return self.execute(ctx, Command::Transfer);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_SPLIT)) {
            return self.execute(ctx, Command::SplitOperation);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_EXTRA))
            && let Some(&index) = doc.selection.iter().next()
        {
            self.tool = Some(crate::tools::Tool::ExtraValues(
                crate::tools::ExtraForm::new(doc, index),
            ));
        }

        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_RULES)) {
            return self.execute(ctx, Command::Rules);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_CATEGORIZE)) {
            return self.execute(ctx, Command::Categorize);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_BUDGET)) {
            return self.execute(ctx, Command::Budget(BudgetTab::Monthly));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_CATEGORY_TOTALS)) {
            return self.execute(ctx, Command::Report(ReportTab::Categories));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_FILTERS)) {
            return self.execute(ctx, Command::AutoFilters);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_SCHEDULER)) {
            return self.execute(ctx, Command::Scheduler);
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
            Command::ImportJson => return self.request(Pending::ImportJson),
            Command::ExportJson => {
                if self
                    .doc
                    .as_ref()
                    .is_some_and(crate::protection_ui::is_protected)
                {
                    self.confirm_json_export = true;
                } else {
                    self.export_json();
                }
                return;
            }
            Command::Options => {
                let theme = ctx.options(|o| o.theme_preference);
                self.options = Some(crate::options_ui::OptionsForm::new(&self.settings, theme));
                return;
            }
            Command::About => return self.about = true,
            Command::License => {
                self.license_form = Some(crate::license_ui::LicenseForm::new());
                return;
            }
            Command::Extension(index) => return self.run_extension(ctx, index),
            Command::ManageExtensions => {
                self.ext_manager = Some(Default::default());
                return;
            }
            Command::ExtensionsFolder => {
                let _ = std::fs::create_dir_all(self.extensions.dir());
                let _ = open::that_detached(self.extensions.dir());
                return;
            }
            Command::ExtensionsDocs => {
                ctx.open_url(egui::OpenUrl::new_tab(crate::extensions::docs_url()));
                return;
            }
            Command::Help => {
                ctx.open_url(egui::OpenUrl::new_tab(
                    "https://github.com/fabiochelly/bankperfect#readme",
                ));
                return;
            }
            Command::DeleteFile => return self.delete_file(),
            Command::Quit => return ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Command::Notes => return self.tool = Some(Tool::Notes(crate::tools::NotesForm::new())),
            Command::Calculator => {
                return self.tool = Some(Tool::Calculator(crate::tools::CalcForm::new()));
            }
            Command::Currencies => {
                return self.tool = Some(Tool::Currencies(Box::new(
                    crate::currency_ui::CurrencyForm::new(),
                )));
            }
            Command::Loan => {
                let currency = self.doc.as_ref().map_or("€", |d| d.main_currency());
                let form = crate::loan_ui::LoanForm::new(currency);
                return self.tool = Some(Tool::Loan(Box::new(form)));
            }
            Command::Palette => {
                self.menus = menu::MenuBar::default();
                return self.command_palette = Some(Default::default());
            }
            Command::Theme(preference) => {
                ctx.set_theme(preference);
                return self.settings.set_theme(preference);
            }
            Command::UseTheme(id) => {
                let Some(theme) = crate::theme::by_id(id) else {
                    return;
                };
                if theme.dark {
                    self.settings.dark_theme = id.into();
                } else {
                    self.settings.light_theme = id.into();
                }
                crate::theme::select(ctx, &self.settings.dark_theme, &self.settings.light_theme);
                // Le thème choisi s'affiche, sauf s'il l'est déjà en suivant
                // le système.
                let mode = if theme.dark {
                    egui::ThemePreference::Dark
                } else {
                    egui::ThemePreference::Light
                };
                let shown = ctx.theme()
                    == if theme.dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    };
                let follows_system =
                    ctx.options(|o| o.theme_preference) == egui::ThemePreference::System;
                if !(follows_system && shown) {
                    ctx.set_theme(mode);
                }
                return self.settings.set_theme(ctx.options(|o| o.theme_preference));
            }
            Command::Language(lang) => {
                self.settings.language = Some(lang.code().into());
                self.settings.save();
                return self.language_changed();
            }
            Command::ToggleTranslucent => {
                let on = !self.settings.translucent();
                self.settings.translucent = Some(on);
                self.settings.save();
                crate::theme::set_translucent(ctx, on);
                if on && !crate::theme::surface_transparent() {
                    self.toast = Some((
                        tr!("Le fond translucide sera actif au prochain démarrage").into(),
                        now + 5.0,
                    ));
                }
                return;
            }
            Command::Save => return self.save(now),
            Command::SaveAs => return self.save_as(),
            Command::SaveCopy => return self.save_copy(),
            Command::CloseFile => return self.request(Pending::Close),
            _ => {}
        }
        let Some(doc) = &mut self.doc else { return };
        match command {
            Command::ExportCsv => self.tool = Some(Tool::ExportCsv(crate::tools::CsvForm::new())),
            Command::ExportXlsx => {
                let rows = crate::reports_ui::visible_rows(doc);
                let dir = crate::settings::shared::export_dir(
                    doc.path.parent().unwrap_or(std::path::Path::new(".")),
                );
                let name = doc.current_account().name.clone();
                crate::spreadsheet::export(&self.egui_ctx, &dir, &name, &name, rows);
            }
            Command::Budget(tab) => {
                self.tool = Some(Tool::Budget(Box::new(crate::budget_ui::BudgetForm::new(
                    doc, tab,
                ))))
            }
            Command::BankSync => {
                self.tool = Some(Tool::BankSync(Box::new(
                    crate::bank_sync_ui::BankSyncForm::new(doc),
                )))
            }
            Command::UpdateQuotes => {
                self.tool = Some(Tool::Quotes(Box::new(crate::quotes_ui::QuotesForm::new(
                    doc,
                ))))
            }
            Command::Charts => {
                self.tool = Some(Tool::Charts(Box::new(crate::charts_ui::ChartsForm::new())))
            }
            Command::Report(tab) => {
                self.tool = Some(Tool::Reports(Box::new(
                    crate::reports_ui::ReportsForm::new(doc, tab),
                )))
            }
            Command::Print => self.tool = Some(Tool::Print(Default::default())),
            Command::Import => {
                let mut statements: Vec<String> = ["ofx", "qfx", "qif", "csv", "tsv", "txt"]
                    .map(String::from)
                    .to_vec();
                for importer in self.extensions.importers() {
                    for e in importer.extensions {
                        if !statements.contains(&e) {
                            statements.push(e);
                        }
                    }
                }
                let mut dialog = rfd::FileDialog::new()
                    .set_title(tr!("Fichier d'opérations à importer"))
                    .add_filter(tr!("Relevés bancaires"), &statements);
                if let Some(dir) = &self.settings.export_dir {
                    dialog = dialog.set_directory(dir);
                }
                // BP_IMPORT_FILE : fichier imposé (captures d'écran automatiques).
                match std::env::var_os("BP_IMPORT_FILE") {
                    Some(path) => self.open_import(PathBuf::from(path)),
                    None => self.ask(crate::file_picker::Kind::File, dialog, |app, path| {
                        app.open_import(path)
                    }),
                }
            }
            Command::ExportQif => {
                let dialog = crate::tools::qif_dialog(doc);
                self.ask(crate::file_picker::Kind::Save, dialog, |app, path| {
                    let Some(doc) = &app.doc else { return };
                    let message = crate::tools::export_qif(doc, &path);
                    app.toast = Some((message, app.egui_ctx.input(|i| i.time) + 4.0));
                });
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
            Command::FocusEntry => match &mut self.editor {
                Some(editor) => editor.focus_date(),
                None => self.editor = Some(Editor::new_operation(&doc.document, doc.account)),
            },
            Command::ModifySelected => {
                if let Some(editor) = self.editor.as_mut().filter(|e| e.target.is_some()) {
                    // Ligne en cours de modification : enregistrement.
                    let action = editor.submit();
                    self.apply_editor_action(action, now);
                } else if doc.selection.len() == 1
                    && let Some(&index) = doc.selection.iter().next()
                {
                    self.editor = Some(Editor::edit(&doc.document, doc.account, index));
                }
            }
            Command::Delete => {
                if doc.selection_locked() {
                    self.toast = Some((locked_message(), now + 4.0));
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
                    self.toast = Some((locked_message(), now + 4.0));
                } else if !doc.selection.is_empty() {
                    self.clipboard = doc.copy_selection();
                    doc.delete_selection();
                    self.editor = None;
                    self.toast = Some((
                        trn!(self.clipboard.len(), "{} ligne coupée", "{} lignes coupées"),
                        now + 3.0,
                    ));
                }
            }
            Command::CopyLines => {
                if !doc.selection.is_empty() {
                    self.clipboard = doc.copy_selection();
                    self.toast = Some((
                        trn!(self.clipboard.len(), "{} ligne copiée", "{} lignes copiées"),
                        now + 3.0,
                    ));
                }
            }
            Command::PasteLines => {
                let n = doc.paste(&self.clipboard);
                if n > 0 {
                    self.editor = None;
                    self.toast = Some((trn!(n, "{} ligne collée", "{} lignes collées"), now + 3.0));
                }
            }
            Command::AddToPayees => {
                if let Some(&index) = doc.selection.iter().next() {
                    self.confirm_payee = Some(index);
                }
            }
            Command::SetMarkDate => {
                if doc.selection_locked() {
                    self.toast = Some((locked_message(), now + 4.0));
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
            Command::ToggleBalanceCards => {
                self.settings.balance_cards = !self.settings.balance_cards;
                self.settings.save();
            }
            Command::ToggleColumnFilters => {
                self.settings.show_column_filters = !self.settings.show_column_filters;
                self.settings.save();
            }
            Command::ToggleBalanceRiver => {
                self.settings.balance_river = !self.settings.balance_river;
                self.settings.save();
            }
            Command::ReverseCurrencies => self.confirm_reverse = true,
            Command::Rules => {
                self.tool = Some(Tool::Rules(Box::new(crate::rules_ui::RulesForm::new(doc))))
            }
            Command::Categorize => {
                let ai = crate::categorize_ui::AiSettings {
                    enabled: self.settings.ai_categorize,
                    api_key: self.settings.ai_api_key.clone(),
                    model: self.settings.ai_model.clone(),
                };
                self.tool = Some(Tool::Categorize(Box::new(
                    crate::categorize_ui::CategorizeForm::new(doc, ai),
                )))
            }
            Command::AutoFilters => {
                self.tool = Some(Tool::Filters(Box::new(
                    crate::filters_ui::FiltersForm::new(doc, false),
                )));
            }
            Command::SaveFilter => {
                self.tool = Some(Tool::Filters(Box::new(
                    crate::filters_ui::FiltersForm::new(doc, true),
                )));
            }
            Command::ApplyFilter(i) => {
                let filters = crate::filters_ui::load(doc);
                if let Some(name) = filters.names().get(i).map(|s| s.to_string())
                    && let Some(f) = filters.get(&name)
                {
                    crate::filters_ui::apply(doc, f);
                }
            }
            Command::ClearFilters => {
                let hide = doc.filters.hide_old_lines;
                doc.filters = Default::default();
                doc.filters.hide_old_lines = hide;
            }
            Command::Scheduler => {
                self.tool = Some(Tool::Scheduler(Box::new(
                    crate::scheduler_ui::SchedulerForm::new(doc, None),
                )));
            }
            Command::Forecast => {
                self.tool = Some(Tool::Forecast(crate::scheduler_ui::ForecastForm::new()))
            }
            Command::AddToScheduler => {
                if let Some(&index) = doc.selection.iter().next() {
                    let op = doc.operation(index).clone();
                    if let Some(schedule) =
                        bp_core::scheduler::schedule_from_operation(&doc.document, doc.account, &op)
                    {
                        let mut schedules = bp_core::scheduler::load(&doc.document)
                            .map(|l| l.schedules)
                            .unwrap_or_default();
                        schedules.push(schedule);
                        if bp_core::scheduler::save(&mut doc.document, &schedules).is_ok() {
                            doc.dirty = true;
                        }
                        let last = schedules.len() - 1;
                        self.tool = Some(Tool::Scheduler(Box::new(
                            crate::scheduler_ui::SchedulerForm::new(doc, Some(last)),
                        )));
                    }
                }
            }
            Command::ReconcileAll => doc.reconcile_all_pointed(),
            Command::SmartTack => {
                self.tool = Some(Tool::SmartTack(Box::new(
                    crate::smart_tack_ui::SmartTackForm::new(doc),
                )))
            }
            Command::ModifyGroup => {
                self.tool = Some(Tool::ModifyGroup(crate::tools::GroupForm::new()))
            }
            Command::Transfer => {
                self.tool = Some(Tool::Transfer(Box::new(
                    crate::transfer_ui::TransferForm::new(doc),
                )))
            }
            Command::ConvertToTransfer => {
                if let Some(&index) = doc.selection.iter().next() {
                    self.tool = Some(Tool::Convert(crate::transfer_ui::ConvertForm::new(
                        doc, index,
                    )));
                }
            }
            Command::SplitOperation => {
                if let Some(&index) = doc.selection.iter().next() {
                    if doc.locked && doc.operation(index).tack == Tack::Reconciled {
                        self.toast = Some((locked_message(), ctx.input(|i| i.time) + 4.0));
                    } else {
                        self.tool = Some(Tool::Split(Box::new(
                            crate::transfer_ui::SplitForm::new(doc, index),
                        )));
                    }
                }
            }
            Command::GoToSister => {
                if let Some(&index) = doc.selection.iter().next() {
                    doc.go_to_sister(index);
                }
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
            | Command::Loan
            | Command::Currencies
            | Command::Palette
            | Command::Theme(_)
            | Command::UseTheme(_)
            | Command::Language(_)
            | Command::ToggleTranslucent
            | Command::OpenRecent(_)
            | Command::Options
            | Command::License
            | Command::About
            | Command::Help
            | Command::Extension(_)
            | Command::ManageExtensions
            | Command::ExtensionsFolder
            | Command::ExtensionsDocs
            | Command::NewFile
            | Command::ImportJson
            | Command::ExportJson
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

    /// Commandes d'extension placées dans un menu (`menu = …`), à la fin,
    /// après un séparateur.
    fn extension_items(&mut self, ui: &mut egui::Ui, place: &str) {
        let has_doc = self.doc.is_some();
        let placed: Vec<usize> = crate::extensions::placed(place)
            .into_iter()
            .filter(|(_, e)| has_doc || e.needs == bp_ext::Needs::Nothing)
            .map(|(i, _)| i)
            .collect();
        if placed.is_empty() {
            return;
        }
        menu::separator(ui);
        for i in placed {
            self.menu_command(ui, Command::Extension(i));
        }
    }

    /// Enregistrer sous (F12) : le fichier enregistré devient le fichier courant.
    fn save_as(&mut self) {
        let Some(doc) = &self.doc else { return };
        let dialog = rfd::FileDialog::new()
            .set_title(tr!("Enregistrer le fichier de comptes sous…"))
            .set_directory(doc.path.parent().unwrap_or(Path::new(".")))
            .set_file_name(doc.file_name())
            .add_filter(tr!("Comptes BankPerfect"), &["bp"]);
        self.ask(crate::file_picker::Kind::Save, dialog, Self::save_as_to);
    }

    fn save_as_to(&mut self, mut path: PathBuf) {
        let now = self.egui_ctx.input(|i| i.time);
        let Some(doc) = &mut self.doc else { return };
        if path.extension().is_none() {
            path.set_extension("bp");
        }
        // L'original v5 est gardé à côté de l'ancien emplacement.
        if doc.format == FileFormat::V5
            && let Err(e) = backup_v5_original(&doc.path)
        {
            self.error = Some(tr!("Copie de l'original impossible : {}", e));
            return;
        }
        doc.format = FileFormat::V6;
        doc.path = path.clone();
        self.save(now);
        self.settings.remember(&path);
    }

    /// Enregistre une copie (clé USB, disque externe…) sans changer de
    /// fichier courant : `<nom>_AAAAMMJJ.bp` dans le dossier choisi.
    fn save_copy(&mut self) {
        let dialog = rfd::FileDialog::new().set_title(tr!("Dossier où enregistrer une copie"));
        self.ask(crate::file_picker::Kind::Folder, dialog, Self::save_copy_to);
    }

    fn save_copy_to(&mut self, folder: PathBuf) {
        let now = self.egui_ctx.input(|i| i.time);
        let Some(doc) = &mut self.doc else { return };
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
                self.toast = Some((tr!("Copie enregistrée : {}", target.display()), now + 5.0));
            }
            Err(e) => self.error = Some(tr!("Copie impossible : {}", e)),
        }
    }

    /// Exporte le fichier complet (comptes, référentiels, données des
    /// fonctionnalités) en JSON lisible, réimportable sans perte.
    fn export_json(&mut self) {
        let Some(doc) = &self.doc else { return };
        let stem = doc
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dialog = rfd::FileDialog::new()
            .set_title(tr!("Exporter le fichier complet en JSON"))
            .set_directory(crate::settings::shared::export_dir(
                doc.path.parent().unwrap_or(Path::new(".")),
            ))
            .set_file_name(format!("{stem}.json"))
            .add_filter("JSON", &["json"]);
        self.ask(crate::file_picker::Kind::Save, dialog, Self::export_json_to);
    }

    fn export_json_to(&mut self, mut path: PathBuf) {
        let now = self.egui_ctx.input(|i| i.time);
        let Some(doc) = &self.doc else { return };
        if path.extension().is_none() {
            path.set_extension("json");
        }
        let result = bp_core::json::to_json(&doc.document)
            .and_then(|bytes| bp_core::format_v6::write_atomic(&path, &bytes));
        match result {
            Ok(()) => {
                self.toast = Some((tr!("Fichier {} créé", path.display()), now + 4.0));
            }
            Err(e) => self.error = Some(tr!("Export JSON impossible : {}", e)),
        }
    }

    /// Importe un export JSON : crée un nouveau fichier de comptes (sans mot
    /// de passe) à l'emplacement choisi, puis l'ouvre.
    fn import_json(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title(tr!("Export JSON à importer"))
            .add_filter("JSON", &["json"]);
        if let Some(dir) = &self.settings.export_dir {
            dialog = dialog.set_directory(dir);
        }
        self.ask(
            crate::file_picker::Kind::File,
            dialog,
            Self::import_json_from,
        );
    }

    fn import_json_from(&mut self, source: PathBuf) {
        let document = match std::fs::read(&source)
            .map_err(bp_core::Error::from)
            .and_then(|data| bp_core::json::from_json(&data))
        {
            Ok(document) => document,
            Err(e) => return self.error = Some(tr!("Import impossible : {}", e)),
        };
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut dialog = rfd::FileDialog::new()
            .set_title(tr!("Enregistrer le fichier de comptes importé"))
            .set_file_name(format!("{stem}.bp"))
            .add_filter(tr!("Comptes BankPerfect"), &["bp"]);
        if let Some(dir) = &self.settings.files_dir {
            dialog = dialog.set_directory(dir);
        }
        self.ask(crate::file_picker::Kind::Save, dialog, move |app, path| {
            app.import_json_to(path, document)
        });
    }

    fn import_json_to(&mut self, mut path: PathBuf, document: bp_core::model::Document) {
        if path.extension().is_none() {
            path.set_extension("bp");
        }
        if path.exists() {
            self.error = Some(tr!("Ce fichier existe déjà : choisissez un autre nom.").into());
            return;
        }
        if let Err(e) = Protection::none().and_then(|p| file::save(&path, &document, &p)) {
            self.error = Some(tr!("Enregistrement impossible : {}", e));
            return;
        }
        self.open_path(path);
        if let Some(doc) = &mut self.doc {
            doc.notice = Some(
                tr!("Export JSON importé. Le fichier n'a pas de mot de passe (Fichier › Protection du fichier)")
                    .into(),
            );
        }
    }

    /// Supprimer un fichier de comptes : il part à la corbeille du système.
    fn delete_file(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title(tr!("Fichier de comptes à mettre à la corbeille"))
            .add_filter(tr!("Comptes BankPerfect"), &["bp"]);
        if let Some(dir) = self.settings.files_dir.clone().or_else(|| {
            self.doc
                .as_ref()
                .and_then(|d| d.path.parent().map(Path::to_path_buf))
        }) {
            dialog = dialog.set_directory(dir);
        }
        self.ask(crate::file_picker::Kind::File, dialog, Self::trash_file);
    }

    fn trash_file(&mut self, path: PathBuf) {
        let same = |a: &Path, b: &Path| a.canonicalize().ok() == b.canonicalize().ok();
        if self.doc.as_ref().is_some_and(|d| same(&d.path, &path)) {
            self.error =
                Some(tr!("Ce fichier est actuellement ouvert : fermez-le d'abord.").into());
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
                tr!("Fichier"),
                tr!("Édition"),
                tr!("Affichage"),
                tr!("Comptes"),
                tr!("Opérations"),
                tr!("Rapports"),
                tr!("Outils"),
                tr!("Extensions"),
                tr!("Aide"),
            ]
        } else {
            &[
                tr!("Fichier"),
                tr!("Outils"),
                tr!("Extensions"),
                tr!("Aide"),
            ]
        };
        let (opened, command) = ui
            .horizontal(|ui| {
                let opened = self.menus.titles(ui, titles);
                // Symbole du logiciel en haut à droite, dessiné par-dessus la
                // barre (il déborde un peu sous elle sans la grandir).
                let pal = palette(ui);
                let size = LOGO_SIZE;
                // Même marge en haut et à droite de la fenêtre.
                let margin = LOGO_MARGIN;
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
                // Filtres prédéfinis à gauche du symbole : la place gagnée
                // évite une ligne d'en-tête au-dessus des soldes.
                let mut command = None;
                if let Some(doc) = self.doc.as_mut() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space((ui.max_rect().right() - rect.left() + 16.0).max(0.0));
                        command = Self::filters_menu(ui, doc);
                    });
                }
                (opened, command)
            })
            .inner;
        if let Some((index, anchor)) = opened {
            let ctx = ui.ctx().clone();
            let mut menus = std::mem::take(&mut self.menus);
            menus.popup(&ctx, index, anchor, |ui| self.menu_content(ui, index));
            self.menus = menus;
        }
        if let Some(command) = command {
            let ctx = ui.ctx().clone();
            self.execute(&ctx, command);
        }
    }

    fn menu_content(&mut self, ui: &mut egui::Ui, index: usize) {
        use Command::*;
        let has_doc = self.doc.is_some();
        let tools_index = if has_doc { 6 } else { 1 };
        if has_doc && index == 5 {
            menu::begin_with_width(ui, 320.0);
            self.menu_command(ui, Report(ReportTab::Accounts));
            self.menu_command(ui, Report(ReportTab::Categories));
            self.menu_command(ui, Report(ReportTab::Payees));
            self.menu_command(ui, Report(ReportTab::Statement));
            menu::separator(ui);
            self.menu_command(ui, Charts);
            menu::separator(ui);
            self.menu_command(ui, Budget(BudgetTab::Monthly));
            self.menu_command(ui, Budget(BudgetTab::Annual));
            menu::separator(ui);
            self.menu_command(ui, ExportXlsx);
            self.extension_items(ui, "reports");
            return;
        }
        if index == tools_index + 1 {
            menu::begin_with_width(ui, 340.0);
            // Seulement les commandes qui ne sont pas placées ailleurs.
            let entries: Vec<(usize, crate::extensions::Entry)> =
                crate::extensions::placed("extensions")
                    .into_iter()
                    .filter(|(_, e)| has_doc || e.needs == bp_ext::Needs::Nothing)
                    .collect();
            // Titre d'extension seulement pour celles qui ont plusieurs
            // commandes ; séparateur entre deux extensions.
            let count = |group: &str| entries.iter().filter(|(_, e)| e.group == group).count();
            let mut last_group = None;
            let mut shown = false;
            for (i, entry) in entries.iter() {
                let i = *i;
                if last_group != Some(entry.group) {
                    let several = count(entry.group) > 1;
                    let previous_several = last_group.is_some_and(|g| count(g) > 1);
                    if last_group.is_some() && (several || previous_several) {
                        menu::separator(ui);
                    }
                    if several {
                        menu::heading(ui, entry.group);
                    }
                    last_group = Some(entry.group);
                }
                self.menu_command(ui, Extension(i));
                shown = true;
            }
            if shown {
                menu::separator(ui);
            }
            self.menu_command(ui, ManageExtensions);
            self.menu_command(ui, ExtensionsFolder);
            self.menu_command(ui, ExtensionsDocs);
            return;
        }
        if index == tools_index + 2 {
            menu::begin(ui);
            self.menu_command(ui, Palette);
            self.menu_command(ui, Help);
            menu::separator(ui);
            self.menu_command(ui, License);
            self.menu_command(ui, About);
            return;
        }
        if index == 0 {
            menu::begin_with_width(ui, 360.0);
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
                let mut open_recent = None;
                menu::submenu(
                    ui,
                    Some(icon::CLOCK_COUNTER_CLOCKWISE),
                    tr!("Fichiers récents"),
                    |ui| {
                        for (i, name) in recent.iter().enumerate() {
                            if menu::item(ui, Some(icon::FILE), name, None).clicked() {
                                open_recent = Some(i);
                            }
                        }
                    },
                );
                if let Some(i) = open_recent {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, OpenRecent(i));
                }
            }
            self.menu_command(ui, ImportJson);
            menu::separator(ui);
            self.menu_command(ui, Save);
            if has_doc {
                self.menu_command(ui, SaveAs);
                self.menu_command(ui, SaveCopy);
                menu::separator(ui);
                self.menu_command(ui, Import);
                self.menu_command(ui, BankSync);
                menu::submenu(ui, Some(icon::EXPORT), tr!("Exporter"), |ui| {
                    self.menu_command(ui, ExportXlsx);
                    self.menu_command(ui, ExportCsv);
                    self.menu_command(ui, ExportQif);
                    menu::separator(ui);
                    self.menu_command(ui, ExportJson);
                    self.extension_items(ui, "export");
                });
                self.menu_command(ui, Print);
                menu::separator(ui);
                self.menu_command(ui, Protection);
                self.menu_command(ui, Backups);
                menu::separator(ui);
                self.menu_command(ui, CloseFile);
            }
            self.menu_command(ui, DeleteFile);
            self.extension_items(ui, "file");
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
            self.menu_command(ui, Loan);
            self.menu_command(ui, Currencies);
            if has_doc {
                menu::separator(ui);
                self.menu_command(ui, ReverseCurrencies);
            }
            self.extension_items(ui, "tools");
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
                self.menu_command(ui, FocusEntry);
                menu::separator(ui);
                self.menu_command(ui, CutLines);
                self.menu_command(ui, CopyLines);
                self.menu_command(ui, PasteLines);
                menu::separator(ui);
                self.menu_command(ui, Delete);
                menu::separator(ui);
                self.menu_command(ui, Replace);
                self.extension_items(ui, "edit");
            }
            2 => {
                menu::begin(ui);
                if let Some(doc) = &mut self.doc {
                    menu::toggle(
                        ui,
                        &mut doc.filters.hide_old_lines,
                        tr!("Cacher les lignes inutiles"),
                        Some("Ctrl+H"),
                    );
                    let mut marks = doc.show_mark_dates;
                    if menu::toggle(
                        ui,
                        &mut marks,
                        tr!("Afficher les dates de pointage"),
                        Some("F11"),
                    )
                    .clicked()
                    {
                        doc.set_show_mark_dates(marks);
                    }
                }
                if menu::toggle(
                    ui,
                    &mut self.settings.entry_at_bottom,
                    tr!("Saisie en bas de la fenêtre"),
                    None,
                )
                .clicked()
                {
                    self.editor = None;
                    self.settings.save();
                }
                menu::separator(ui);
                self.menu_command(ui, AutoFilters);
                self.menu_command(ui, SaveFilter);
                self.menu_command(ui, ClearFilters);
                menu::separator(ui);
                let mut panel = self.settings.show_balance_panel;
                if menu::toggle(ui, &mut panel, tr!("Panneau des soldes"), None).clicked() {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleBalancePanel);
                }
                let mut forecast = self.settings.show_forecast;
                if menu::toggle(ui, &mut forecast, tr!("Solde prévisionnel"), None).clicked() {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleForecast);
                }
                let mut split = self.settings.balance_cards;
                if menu::toggle(ui, &mut split, tr!("Soldes en trois cartes"), None).clicked() {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleBalanceCards);
                }
                let mut column_filters = self.settings.show_column_filters;
                if menu::toggle(ui, &mut column_filters, tr!("Filtres de colonnes"), None).clicked()
                {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleColumnFilters);
                }
                let mut river = self.settings.balance_river;
                if menu::toggle(ui, &mut river, tr!("Courbe du solde"), None).clicked() {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleBalanceRiver);
                }
                menu::separator(ui);
                menu::heading(ui, tr!("Thème"));
                let current = ui.ctx().options(|o| o.theme_preference);
                for (preference, label) in [
                    (egui::ThemePreference::Dark, tr!("Sombre")),
                    (egui::ThemePreference::Light, tr!("Clair")),
                    (egui::ThemePreference::System, tr!("Suivre le système")),
                ] {
                    if menu::choice(ui, current == preference, label).clicked() {
                        let ctx = ui.ctx().clone();
                        self.execute(&ctx, Theme(preference));
                    }
                }
                for (dark, label) in [(true, tr!("Thèmes sombres")), (false, tr!("Thèmes clairs"))]
                {
                    let selected = if dark {
                        self.settings.dark_theme.clone()
                    } else {
                        self.settings.light_theme.clone()
                    };
                    let glyph = if dark { icon::MOON } else { icon::SUN };
                    let mut chosen = None;
                    menu::submenu(ui, Some(glyph), label, |ui| {
                        for theme in crate::theme::THEMES.iter().filter(|t| t.dark == dark) {
                            if menu::choice(
                                ui,
                                crate::theme::find(&selected, dark).id == theme.id,
                                bp_core::i18n::tr(theme.name),
                            )
                            .clicked()
                            {
                                chosen = Some(theme.id);
                            }
                        }
                    });
                    if let Some(id) = chosen {
                        let ctx = ui.ctx().clone();
                        self.execute(&ctx, UseTheme(id));
                    }
                }
                menu::separator(ui);
                let mut translucent = self.settings.translucent();
                if menu::toggle(
                    ui,
                    &mut translucent,
                    tr!("Fond translucide (flou du bureau)"),
                    None,
                )
                .clicked()
                {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleTranslucent);
                }
                let current = self.settings.language();
                let mut chosen = None;
                menu::submenu(ui, Some(icon::TRANSLATE), tr!("Langue"), |ui| {
                    for lang in bp_core::i18n::Lang::ALL {
                        if menu::choice(ui, current == lang, lang.native_name()).clicked() {
                            chosen = Some(lang);
                        }
                    }
                });
                if let Some(lang) = chosen {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, Command::Language(lang));
                }
                self.extension_items(ui, "view");
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
                self.menu_command(ui, Scheduler);
                self.menu_command(ui, Forecast);
                menu::separator(ui);
                for tab in [
                    crate::manage_ui::Tab::Accounts,
                    crate::manage_ui::Tab::Categories,
                    crate::manage_ui::Tab::Payees,
                    crate::manage_ui::Tab::Modes,
                ] {
                    self.menu_command(ui, Manage(tab));
                }
                menu::separator(ui);
                self.menu_command(ui, UpdateQuotes);
                self.extension_items(ui, "accounts");
            }
            _ => {
                menu::begin_with_width(ui, 340.0);
                self.menu_command(ui, Tack);
                self.menu_command(ui, SetMarkDate);
                self.menu_command(ui, ReconcileAll);
                self.menu_command(ui, SmartTack);
                let mut locked = self.settings.lock_reconciled;
                if menu::toggle(
                    ui,
                    &mut locked,
                    tr!("Verrouiller les lignes rapprochées"),
                    None,
                )
                .clicked()
                {
                    let ctx = ui.ctx().clone();
                    self.execute(&ctx, ToggleLock);
                }
                menu::separator(ui);
                self.menu_command(ui, ModifyGroup);
                self.menu_command(ui, ExtraFields);
                self.menu_command(ui, SplitOperation);
                menu::separator(ui);
                self.menu_command(ui, Transfer);
                self.menu_command(ui, ConvertToTransfer);
                self.menu_command(ui, GoToSister);
                menu::separator(ui);
                self.menu_command(ui, Rules);
                self.menu_command(ui, Categorize);
                self.menu_command(ui, AddToPayees);
                self.menu_command(ui, AddToScheduler);
                self.extension_items(ui, "operations");
            }
        }
    }

    fn balance_strip(ui: &mut egui::Ui, doc: &mut OpenDoc, compact: bool) -> Option<Command> {
        let mut command = None;
        if compact {
            // Sans panneau des comptes : le nom du compte sert de sélecteur,
            // avec la banque et l'IBAN à côté.
            let pal = palette(ui);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let title = RichText::new(&doc.current_account().name)
                    .size(18.0)
                    .family(crate::theme::semibold());
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
                let account = doc.current_account();
                let mut sub = account.bank_name.clone();
                if !account.iban.trim().is_empty() {
                    sub = if sub.is_empty() {
                        account.iban.trim().to_string()
                    } else {
                        format!("{sub}  ·  {}", account.iban.trim())
                    };
                }
                ui.add(egui::Label::new(RichText::new(sub).small().color(pal.muted)).truncate());
            });
        }
        // Le nom du compte est déjà dans le panneau latéral (la banque et
        // l'IBAN en infobulle) : les soldes remontent juste sous les menus.
        if compact {
            ui.add_space(8.0);
        } else {
            // Le symbole déborde sous la barre de menus : les tuiles
            // commencent sous lui, à la même hauteur que l'île des comptes.
            ui.add_space((islands_top(ui.ctx()) - ui.cursor().top()).max(8.0));
        }
        if doc.show_balance_panel
            && let Some(toggle) = crate::dashboard::cards(ui, doc)
        {
            command = Some(toggle);
        }
        command
    }

    /// Menu des filtres prédéfinis (appliquer, enregistrer, gérer), dans la
    /// barre de menus, en disposition de droite à gauche. Le bouton prend la
    /// couleur des lignes pointées quand un filtre est actif ; si les soldes
    /// portent sur les lignes filtrées, une mention le dit à sa gauche quand
    /// il y a la place, sinon l'infobulle du bouton.
    fn filters_menu(ui: &mut egui::Ui, doc: &mut OpenDoc) -> Option<Command> {
        let notice_text = tr!("soldes calculés sur les lignes filtrées");
        let mut command = None;
        let pal = palette(ui);
        let active = doc.filters.is_active();
        let filtered = active && !doc.ignore_filter_totals();
        let color = if active { pal.pointed } else { pal.fg_dark };
        let measure = |ui: &egui::Ui, text: &str, style: egui::TextStyle| {
            let font = style.resolve(ui.style());
            ui.painter()
                .layout_no_wrap(text.into(), font, color)
                .size()
                .x
        };
        let full = format!("{}  {}  {}", icon::FUNNEL, tr!("Filtres"), icon::CARET_DOWN);
        let button = measure(ui, &full, egui::TextStyle::Body) + 24.0;
        let notice = measure(
            ui,
            &format!("{}  {notice_text}", icon::FUNNEL),
            egui::TextStyle::Small,
        ) + 16.0;
        let room = ui.available_width();
        // Icône seule quand les menus laissent trop peu de place.
        let label = if room > button {
            full
        } else {
            format!("{} {}", icon::FUNNEL, icon::CARET_DOWN)
        };
        let show_notice = filtered && room > button + notice;
        let hover = if filtered && !show_notice {
            format!(
                "{}\n{}  {notice_text}",
                tr!("Filtres prédéfinis"),
                icon::FUNNEL
            )
        } else {
            tr!("Filtres prédéfinis").to_string()
        };
        let response = ui
            .add(egui::Button::new(RichText::new(label).color(color)).frame(false))
            .on_hover_text(hover);
        egui::Popup::menu(&response).show(|ui| {
            crate::menu::popup_style(ui.style_mut());
            crate::menu::begin(ui);
            let filters = crate::filters_ui::load(doc);
            let names: Vec<String> = filters.names().iter().map(|s| s.to_string()).collect();
            for name in &names {
                if crate::menu::item(ui, Some(icon::FUNNEL_SIMPLE), name, None).clicked()
                    && let Some(f) = filters.get(name)
                {
                    crate::filters_ui::apply(doc, f);
                }
            }
            if !names.is_empty() {
                crate::menu::separator(ui);
            }
            if crate::menu::item_enabled(
                ui,
                active,
                Some(icon::FUNNEL_X),
                tr!("Retirer les filtres"),
                None,
            )
            .clicked()
            {
                command = Some(Command::ClearFilters);
            }
            if crate::menu::item(
                ui,
                Some(icon::FLOPPY_DISK),
                tr!("Enregistrer le filtre actuel…"),
                None,
            )
            .clicked()
            {
                command = Some(Command::SaveFilter);
            }
            if crate::menu::item(
                ui,
                Some(icon::GEAR_SIX),
                tr!("Gérer les filtres…"),
                Some("F7"),
            )
            .clicked()
            {
                command = Some(Command::AutoFilters);
            }
        });
        // Dans la barre plutôt qu'au-dessus de la grille : rien ne bouge quand
        // un filtre s'active.
        if show_notice {
            ui.label(
                RichText::new(format!("{}  {notice_text}", icon::FUNNEL))
                    .small()
                    .color(pal.pointed),
            );
        }
        command
    }

    /// Nombre d'opérations du compte (affichées / total si un filtre est
    /// actif).
    fn operation_count(ui: &mut egui::Ui, doc: &OpenDoc) {
        let total = crate::format::count(doc.total_count());
        if doc.filters.is_active() || doc.filters.hide_old_lines {
            let shown = crate::format::count(doc.visible_len());
            ui.label(
                RichText::new(format!(
                    "{}  {}",
                    icon::FUNNEL,
                    trn!(
                        doc.total_count(),
                        "{1} / {2} opération",
                        "{1} / {2} opérations",
                        shown,
                        total
                    )
                ))
                .color(palette(ui).pointed),
            );
        } else {
            ui.label(trn!(
                doc.total_count(),
                "{1} opération",
                "{1} opérations",
                total
            ));
        }
    }

    /// Barre d'état, dans son île. Une seule taille de texte partout, entre
    /// le texte courant et le petit texte.
    fn status_bar(
        ui: &mut egui::Ui,
        doc: &mut OpenDoc,
        toast: Option<&str>,
        license: Option<&str>,
    ) {
        let pal = palette(ui);
        let font = egui::FontId::proportional(STATUS_FONT_SIZE);
        for style in [egui::TextStyle::Body, egui::TextStyle::Small] {
            ui.style_mut().text_styles.insert(style, font.clone());
        }
        ui.horizontal_centered(|ui| {
            Self::operation_count(ui, doc);
            if doc.filters.is_active() || doc.filters.hide_old_lines {
                filter_chips(ui, doc);
            }
            if !doc.selection.is_empty() {
                ui.separator();
            }
            selection_summary(ui, doc);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if doc.dirty {
                    ui.label(
                        RichText::new(format!(
                            "{}  {}",
                            icon::CIRCLE,
                            tr!("Modifications non enregistrées")
                        ))
                        .small()
                        .color(pal.warning),
                    );
                } else {
                    ui.label(
                        RichText::new(format!("{}  {}", icon::CHECK, tr!("Enregistré")))
                            .small()
                            .color(pal.muted),
                    );
                }
                // Licence : à gauche de l'état d'enregistrement.
                if let Some(license) = license {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("{}  {license}", icon::SEAL_CHECK))
                            .small()
                            .color(pal.muted),
                    );
                }
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
                    RichText::new(format!(
                        "{}  {}",
                        icon::FOLDER_OPEN,
                        tr!("Ouvrir un fichier…")
                    ))
                    .size(15.0),
                ))
                .clicked()
            {
                self.request(Pending::Open(None));
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr!("ou déposez un fichier .bp dans la fenêtre"))
                    .color(palette(ui).muted),
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
                    RichText::new(tr!("FICHIERS RÉCENTS"))
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
                    if crate::ui_kit::hover(ui.add(button), &folder).clicked() {
                        self.request(Pending::Open(Some(path)));
                    }
                }
            }
        });
    }

    /// Tiroir de saisie qui glisse par-dessus la grille, collé au bord droit.
    fn editor_drawer(&mut self, ctx: &egui::Context, content: egui::Rect, now: f64) {
        if self.settings.entry_at_bottom {
            return;
        }
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
        self.apply_editor_action(action, now);
    }

    /// Suite d'une action du formulaire de saisie (panneau ou barre du bas).
    fn apply_editor_action(&mut self, action: EditorAction, now: f64) {
        let Some(doc) = &mut self.doc else { return };
        let bottom = self.settings.entry_at_bottom;
        match action {
            EditorAction::None => {}
            // Barre du bas : elle reste affichée, vidée.
            EditorAction::Close if bottom => {
                self.editor = Some(Editor::new_operation(&doc.document, doc.account).quiet())
            }
            EditorAction::Close => self.editor = None,
            EditorAction::Save(target, op) => {
                let locked =
                    doc.locked && target.is_some_and(|i| doc.operation(i).tack == Tack::Reconciled);
                if locked {
                    self.toast = Some((locked_message(), now + 4.0));
                } else if let Some(index) = target.filter(|&i| doc.sister_of(i).is_some()) {
                    self.confirm_linked_edit = Some((index, op));
                } else {
                    let date = op.date.to_string();
                    doc.upsert_operation(target, op);
                    // Saisie en série : formulaire vidé, même date.
                    self.editor = if bottom {
                        let editor =
                            Editor::new_operation(&doc.document, doc.account).with_date(date);
                        Some(if target.is_some() {
                            editor.quiet()
                        } else {
                            editor
                        })
                    } else {
                        (target.is_none() && self.settings.keep_entry_open).then(|| {
                            Editor::new_operation(&doc.document, doc.account).with_date(date)
                        })
                    };
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
                OptionsOutcome::Cancel => {
                    self.options = None;
                    // Annule l'aperçu des thèmes.
                    crate::theme::select(
                        ctx,
                        &self.settings.dark_theme,
                        &self.settings.light_theme,
                    );
                    ctx.set_theme(self.settings.theme_preference());
                }
                OptionsOutcome::Apply(settings, theme) => {
                    self.options = None;
                    let translucent_before = self.settings.translucent();
                    self.settings = *settings;
                    self.settings.save();
                    self.language_changed();
                    crate::theme::select(
                        ctx,
                        &self.settings.dark_theme,
                        &self.settings.light_theme,
                    );
                    ctx.set_theme(theme);
                    self.settings.set_theme(theme);
                    if self.settings.translucent() != translucent_before {
                        crate::theme::set_translucent(ctx, self.settings.translucent());
                        if self.settings.translucent() && !crate::theme::surface_transparent() {
                            self.toast = Some((
                                tr!("Le fond translucide sera actif au prochain démarrage").into(),
                                now + 5.0,
                            ));
                        }
                    }
                    if self.toast.is_none() {
                        self.toast = Some((tr!("Options enregistrées").into(), now + 3.0));
                    }
                }
            }
        }
        if let Some(summary) = self.scheduler_notice.clone() {
            let mut close = false;
            dialogs::modal(ctx, "scheduler_notice").show(ctx, |ui| {
                ui.set_width(560.0);
                let pal = palette(ui);
                dialogs::header(
                    ui,
                    icon::CALENDAR_CHECK,
                    pal.accent,
                    tr!("Échéances insérées"),
                );
                ui.label(
                    RichText::new(tr!(
                        "Les échéances arrivées à terme ont été ajoutées à vos comptes :"
                    ))
                    .color(pal.fg_dark),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for line in summary.lines().skip(1).filter(|l| !l.trim().is_empty()) {
                            let refused = line == tr!("Lignes refusées :") || line.contains("...");
                            let color = if refused { pal.warning } else { pal.fg };
                            ui.label(RichText::new(line).color(color));
                        }
                    });
                dialogs::footer(ui, |ui| {
                    if dialogs::primary(ui, "OK", true).clicked()
                        || ui.input(|i| {
                            i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)
                        })
                    {
                        close = true;
                    }
                });
            });
            if close {
                self.scheduler_notice = None;
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
                        RichText::new(format!(
                            "{}  {}",
                            icon::SEAL_CHECK,
                            tr!("Logiciel libre — licence MIT")
                        ))
                        .color(pal.positive),
                    );
                    if let Some(license) = &self.license {
                        ui.label(
                            RichText::new(format!("{}  {}", icon::KEY, license.label()))
                                .color(pal.fg_dark),
                        );
                    }
                    ui.hyperlink_to(
                        format!("{}  github.com/fabiochelly/bankperfect", icon::GITHUB_LOGO),
                        "https://github.com/fabiochelly/bankperfect",
                    );
                    ui.add_space(10.0);
                    if let Ok(exe) = std::env::current_exe() {
                        ui.label(
                            RichText::new(tr!("Emplacement : {}", exe.display()))
                                .size(12.0)
                                .color(pal.muted),
                        );
                    }
                });
                dialogs::footer(ui, |ui| {
                    if dialogs::primary(ui, tr!("Fermer"), true).clicked()
                        || ui.input(|i| {
                            i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter)
                        })
                    {
                        self.about = false;
                    }
                });
            });
        }
        if let Some(form) = &mut self.license_form {
            use crate::license_ui::LicenseAction;
            match form.show(ctx, self.license.as_ref()) {
                LicenseAction::Keep => {}
                LicenseAction::Close => self.license_form = None,
                // La fenêtre reste ouverte et montre la licence activée.
                LicenseAction::Activated(certificate, license) => {
                    self.settings.license = Some(certificate);
                    self.settings.save();
                    self.license = Some(license);
                }
                LicenseAction::Remove => {
                    self.settings.license = None;
                    self.settings.save();
                    self.license = None;
                    self.license_form = None;
                }
            }
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
                (_, crate::tools::Tool::Loan(form)) => Some(form.show_standalone(ctx)),
                (_, crate::tools::Tool::Currencies(form)) => Some(form.show_standalone(ctx)),
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
                        tr!("Mot de passe défini — enregistrez (Ctrl+S) pour l'appliquer").into(),
                        now + 6.0,
                    ));
                }
                crate::tools::Outcome::Switch(tool) => self.tool = Some(*tool),
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
        if let Some((index, _)) = &self.confirm_linked_edit
            && let Some(doc) = &mut self.doc
        {
            let index = *index;
            match dialogs::confirm(
                ctx,
                "linked_edit",
                icon::ARROWS_LEFT_RIGHT,
                tr!("Ligne liée"),
                tr!(
                    "La ligne modifiée est un transfert lié à une ligne d'un autre compte. Quelles lignes faut-il modifier ? La date, le détail, la catégorie et le montant seront recopiés dans l'autre ligne."
                ),
                tr!("Les 2 lignes"),
                Some(tr!("Uniquement cette ligne")),
                false,
            ) {
                Choice::Pending => {}
                choice @ (Choice::Yes | Choice::No) => {
                    if let Some((_, op)) = self.confirm_linked_edit.take() {
                        doc.update_linked(index, op, choice == Choice::Yes);
                    }
                    self.editor = None;
                }
                Choice::Cancel => self.confirm_linked_edit = None,
            }
        }
        let linked_delete = self.confirm_delete
            && self.doc.as_ref().is_some_and(|d| {
                d.selection.len() == 1
                    && d.selection
                        .iter()
                        .next()
                        .is_some_and(|&i| d.sister_of(i).is_some())
            });
        if linked_delete
            && let Some(doc) = &mut self.doc
            && let Some(&index) = doc.selection.iter().next()
        {
            match dialogs::confirm(
                ctx,
                "delete_linked",
                icon::TRASH,
                tr!("Supprimer"),
                tr!(
                    "La ligne sélectionnée est un transfert lié à une ligne d'un autre compte. Faut-il supprimer les 2 lignes ? Sinon, l'autre ligne devient un simple virement."
                ),
                tr!("Les 2 lignes"),
                Some(tr!("Uniquement celle-ci")),
                true,
            ) {
                Choice::Pending => {}
                Choice::Yes => {
                    doc.delete_linked(index, true);
                    self.editor = None;
                    self.confirm_delete = false;
                }
                Choice::No => {
                    doc.delete_linked(index, false);
                    self.editor = None;
                    self.confirm_delete = false;
                }
                Choice::Cancel => self.confirm_delete = false,
            }
        }
        if self.confirm_delete
            && !linked_delete
            && let Some(doc) = &mut self.doc
        {
            let n = doc.selection.len();
            let message = trn!(
                n,
                "Supprimer l'opération sélectionnée ?",
                "Supprimer les {} opérations sélectionnées ?"
            );
            match dialogs::confirm(
                ctx,
                "delete",
                icon::TRASH,
                tr!("Supprimer"),
                &message,
                tr!("Supprimer"),
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
            let message = tr!(
                "Créer le tiers mémorisé « {} » ? La saisie le proposera avec ce mode, ce détail et cette catégorie.",
                op.payee
            );
            match dialogs::confirm(
                ctx,
                "add_payee",
                icon::USER_PLUS,
                tr!("Ajouter aux tiers mémorisés"),
                &message,
                tr!("Créer le tiers"),
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
            let decimal = bp_core::i18n::decimal_separator().to_string();
            let message = tr!(
                "Toutes les opérations du compte « {} » vont être converties de {} en {} (taux {} → {}). L'historique des annulations sera vidé.",
                account.name,
                account.currency,
                account.secondary_currency,
                account.currency_rate.to_string().replace('.', &decimal),
                account.secondary_rate.to_string().replace('.', &decimal)
            );
            match dialogs::confirm(
                ctx,
                "reverse_currencies",
                icon::ARROWS_LEFT_RIGHT,
                tr!("Inverser les monnaies"),
                &message,
                tr!("Convertir"),
                None,
                false,
            ) {
                Choice::Pending => {}
                Choice::Yes => {
                    self.confirm_reverse = false;
                    let index = doc.account;
                    doc.edit_document(|d| bp_core::manage::reverse_currencies(d, index));
                    let currency = doc.current_account().currency.clone();
                    self.toast = Some((tr!("Compte converti en {}", currency), now + 4.0));
                }
                Choice::No | Choice::Cancel => self.confirm_reverse = false,
            }
        }
        if self.confirm_json_export {
            let name = self.doc.as_ref().map(|d| d.file_name()).unwrap_or_default();
            let message = tr!(
                "« {name} » est protégé par un mot de passe, mais l'export JSON n'est pas chiffré : toute personne qui accède au fichier exporté pourra lire vos comptes. Conservez-le en lieu sûr ou supprimez-le après usage.",
                name = name
            );
            match dialogs::confirm(
                ctx,
                "json_export",
                icon::LOCK_OPEN,
                tr!("Export non chiffré"),
                &message,
                tr!("Exporter quand même"),
                None,
                true,
            ) {
                Choice::Pending => {}
                Choice::Yes => {
                    self.confirm_json_export = false;
                    self.export_json();
                }
                Choice::No | Choice::Cancel => self.confirm_json_export = false,
            }
        }
        if let Some(path) = self.confirm_trash.clone() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let message = tr!(
                "Mettre « {name} » à la corbeille ? Vous pourrez le récupérer depuis la corbeille du système.",
                name = name
            );
            match dialogs::confirm(
                ctx,
                "trash",
                icon::TRASH,
                tr!("Supprimer un fichier de comptes"),
                &message,
                tr!("Mettre à la corbeille"),
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
                            self.toast = Some((tr!("{} mis à la corbeille", name), now + 4.0));
                        }
                        Err(e) => self.error = Some(tr!("Suppression impossible : {}", e)),
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
                dialogs::header(ui, icon::WARNING, pal.negative, tr!("Fichier illisible"));
                ui.label(
                    RichText::new(tr!(
                        "« {} » ne peut pas être ouvert : il semble endommagé.",
                        name
                    ))
                        .color(pal.fg_dark),
                );
                ui.label(RichText::new(reason).small().color(pal.muted));
                if !backups.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(trn!(
                            backups.len(),
                            "{} sauvegarde de ce fichier est disponible ; elle date du {1}.",
                            "{} sauvegardes de ce fichier sont disponibles ; la plus récente date du {1}.",
                            chrono::DateTime::<chrono::Local>::from(backups[0].modified)
                                .format(tr!("%d/%m/%Y à %H:%M"))
                        ))
                        .color(pal.fg_dark),
                    );
                }
                dialogs::footer(ui, |ui| {
                    if !backups.is_empty() && dialogs::primary(ui, tr!("Voir les sauvegardes"), true).clicked() {
                        self.tool = Some(crate::tools::Tool::Backups(crate::tools::BackupsForm::new(
                            path.clone(),
                            self.settings.backup_dir(),
                            self.settings.backup_keep,
                        )));
                        close = true;
                    }
                    if dialogs::button(ui, tr!("Fermer")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
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
            let message = tr!(
                "« {name} » contient des modifications non enregistrées. Voulez-vous les enregistrer avant de continuer ?",
                name = name
            );
            match dialogs::confirm(
                ctx,
                "unsaved",
                icon::FLOPPY_DISK,
                tr!("Enregistrer les modifications ?"),
                &message,
                tr!("Enregistrer"),
                Some(tr!("Ne pas enregistrer")),
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
            && dialogs::alert(ctx, tr!("Erreur"), error)
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
    ///
    /// Les pré-saisies vides hors composition sont écartées : egui-winit
    /// renvoie la zone du curseur à la méthode de saisie après toute image
    /// qui a reçu un événement, et fcitx5 y répond par une pré-saisie vide,
    /// d'où une boucle qui redessine sans arrêt tant qu'un champ a le focus.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.events.retain(|event| match event {
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                let keep = self.ime_composing || !text.is_empty();
                self.ime_composing = !text.is_empty();
                keep
            }
            egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                self.ime_composing = false;
                true
            }
            _ => true,
        });
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
    /// Ouvre un sélecteur de fichiers sans bloquer l'interface ; `then` est
    /// appelée avec le fichier choisi (rien si l'utilisateur annule).
    fn ask(
        &mut self,
        kind: crate::file_picker::Kind,
        dialog: rfd::FileDialog,
        then: impl FnOnce(&mut Self, PathBuf) + 'static,
    ) {
        if crate::file_picker::is_open(&self.egui_ctx) {
            return;
        }
        crate::file_picker::open(&self.egui_ctx, egui::Id::new("app_pick"), kind, dialog);
        self.pick_then = Some(Box::new(then));
    }

    /// Suite de la commande en attente, une fois le fichier choisi.
    fn poll_pick(&mut self) {
        if self.pick_then.is_none() {
            return;
        }
        match crate::file_picker::finished(&self.egui_ctx, egui::Id::new("app_pick")) {
            Some(Some(path)) => {
                if let Some(then) = self.pick_then.take() {
                    then(self, path);
                }
            }
            Some(None) => self.pick_then = None,
            None => {}
        }
    }

    fn frame_ui(&mut self, ui: &mut egui::Ui) {
        self.egui_ctx = ui.ctx().clone();
        self.poll_pick();
        // Exports vers un tableur, d'où qu'ils viennent.
        if let Some(message) = crate::spreadsheet::finished(ui.ctx()) {
            self.toast = Some((message, ui.ctx().input(|i| i.time) + 5.0));
        }
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.theme_pending) {
            crate::theme::select(&ctx, &self.settings.dark_theme, &self.settings.light_theme);
            ctx.set_theme(self.settings.theme_preference());
        }
        crate::theme::adapt_to_display(&ctx);
        // Sous Wayland, la première image part avant que le compositeur ait
        // donné l'échelle de l'écran (1 au lieu de 2 en HiDPI) : tout y
        // serait mis en page et rastérisé pour rien. Le fond seul suffit.
        if std::mem::take(&mut self.first_frame)
            && self.interactive
            && std::env::var_os("WAYLAND_DISPLAY").is_some()
        {
            let canvas = crate::islands::canvas(&palette(ui), ui.visuals().dark_mode);
            egui::CentralPanel::default_margins()
                .frame(egui::Frame::new().fill(canvas))
                .show(ui, |_| {});
            ctx.request_repaint();
            return;
        }

        if let Some(path) = ctx.input(|i| {
            i.raw
                .dropped_files
                .first()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
        }) {
            let suffix = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase)
                .unwrap_or_default();
            let statement = matches!(suffix.as_str(), "ofx" | "qfx" | "qif" | "csv" | "tsv")
                || self
                    .extensions
                    .importers()
                    .iter()
                    .any(|i| i.extensions.contains(&suffix));
            if statement && self.doc.is_some() {
                // Relevé bancaire déposé : import dans le fichier ouvert.
                self.open_import(path);
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
            || self.confirm_linked_edit.is_some()
            || self.options.is_some()
            || self.about
            || self.tool.is_some()
            || self.extensions.has_frames()
            || self.ext_manager.is_some()
            || !self.ext_errors.is_empty();
        if !modal_open {
            self.handle_shortcuts(&ctx);
        }
        crate::commands::set_recent(&self.settings.recent_files);
        crate::settings::shared::set(&self.settings);
        if (ctx.zoom_factor() - self.settings.text_scale).abs() > 0.001 {
            ctx.set_zoom_factor(self.settings.text_scale);
        }
        let now = ctx.input(|i| i.time);
        self.extension_events(now);
        if self.toast.as_ref().is_some_and(|(_, until)| now > *until) {
            self.toast = None;
        }

        // Mise en page en îles : les panneaux sont posés sur le canevas, plus
        // sombre ; les comptes, les soldes et le tableau y forment des îles.
        let pal = palette(ui);
        let dark = ui.visuals().dark_mode;
        let canvas = crate::islands::canvas(&pal, dark);
        let gap = crate::islands::GAP as i8;
        egui::Panel::top("menu")
            .show_separator_line(false)
            .frame(egui::Frame::side_top_panel(ui.style()).fill(canvas))
            .show(ui, |ui| self.menu_bar(ui));

        if self.doc.is_none() {
            egui::CentralPanel::default_margins()
                .frame(egui::Frame::central_panel(ui.style()).fill(canvas))
                .show(ui, |ui| self.welcome(ui));
            self.modals(&ctx);
            self.extension_modals(&ctx);
            self.update_title(&ctx);
            open_requested_urls(&ctx);
            return;
        }

        let toast = self.toast.as_ref().map(|(t, _)| t.clone());
        let license = self.license.as_ref().map(|l| l.label());
        let doc = self.doc.as_mut().expect("document ouvert");
        // Fenêtre étroite (tuile) : pas de panneau des comptes, le nom du
        // compte devient un sélecteur.
        let compact = ui.available_width() < 860.0;
        // Barre d'état : une île sur toute la largeur, sous les comptes et le
        // tableau.
        egui::Panel::bottom("status")
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(canvas).inner_margin(egui::Margin {
                left: gap,
                right: gap,
                top: 0,
                bottom: gap,
            }))
            .show(ui, |ui| {
                status_island(ui, &pal, dark, |ui| {
                    Self::status_bar(ui, doc, toast.as_deref(), license.as_deref())
                });
            });
        // Panneau des comptes plus étroit sur les petits écrans.
        let sidebar_width = (ui.available_width() * 0.2).clamp(190.0, 260.0);
        let mut sidebar_command = None;
        if !compact {
            sidebar_command = egui::Panel::left("accounts")
                .resizable(true)
                .show_separator_line(false)
                .default_size(sidebar_width)
                .min_size(170.0)
                .frame(egui::Frame::new().fill(canvas).inner_margin(egui::Margin {
                    left: gap,
                    right: 0,
                    top: 0,
                    bottom: gap,
                }))
                .show(ui, |ui| {
                    ui.add_space((islands_top(ui.ctx()) - ui.cursor().top()).max(8.0));
                    crate::islands::frame(&pal, dark)
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.set_min_size(ui.available_size());
                            sidebar::show(ui, doc)
                        })
                        .inner
                })
                .inner;
        }
        // Zone à droite de la barre latérale : le tiroir de saisie s'y superpose.
        let content_rect = ui.available_rect_before_wrap();

        // Réglages d'affichage et de verrou repris à chaque image.
        doc.locked = self.settings.lock_reconciled;
        doc.show_balance_panel = self.settings.show_balance_panel;
        doc.show_forecast = self.settings.show_forecast;
        doc.balance_cards = self.settings.balance_cards;
        doc.show_column_filters = self.settings.show_column_filters;
        doc.show_balance_river = self.settings.balance_river;
        if let Some(notice) = doc.notice.take() {
            self.toast = Some((notice, now + 4.0));
        }
        crate::commands::set_clipboard_len(self.clipboard.len());
        if self.command_palette.is_some() {
            let names = crate::filters_ui::load(doc)
                .names()
                .iter()
                .map(|s| s.to_string())
                .collect();
            crate::commands::set_filter_names(names);
        }
        doc.refresh();
        let mut grid_action = GridAction::None;
        let mut deferred = sidebar_command;
        let header_command = egui::Panel::top("balances")
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(canvas).inner_margin(egui::Margin {
                left: gap,
                right: gap,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| Self::balance_strip(ui, doc, compact))
            .inner;
        if let Some(command) = header_command {
            deferred = Some(command);
        }

        // Saisie en bas (option) : barre permanente sous la grille.
        let mut bar_action = None;
        if self.settings.entry_at_bottom {
            let editor = self
                .editor
                .get_or_insert_with(|| Editor::new_operation(&doc.document, doc.account).quiet());
            // Même espace que sous la barre des comptes : le bas de l'île
            // s'aligne sur elle, et la grille garde son espace au-dessus.
            bar_action = egui::Panel::bottom("entry_bar")
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(canvas).inner_margin(egui::Margin {
                    left: gap,
                    right: gap,
                    top: 0,
                    bottom: gap,
                }))
                .show(ui, |ui| {
                    crate::islands::frame(&pal, dark)
                        .inner_margin(egui::Margin::symmetric(18, 6))
                        .show(ui, |ui| editor.show_bar(ui, doc))
                        .inner
                })
                .inner
                .into();
        }

        egui::CentralPanel::default_margins()
            .frame(egui::Frame::new().fill(canvas).inner_margin(egui::Margin {
                left: gap,
                right: gap,
                top: 0,
                bottom: gap,
            }))
            .show(ui, |ui| {
                crate::islands::frame(&pal, dark)
                    .inner_margin(grid::ISLAND_MARGIN)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        let action = grid::show(ui, doc);
                        if !matches!(action, GridAction::None) {
                            grid_action = action;
                        }
                    });
            });
        if self.editor.is_none()
            && !self.settings.entry_at_bottom
            && floating_add_button(&ctx, content_rect)
        {
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

        if let Some(action) = bar_action {
            self.apply_editor_action(action, now);
        }
        self.editor_drawer(&ctx, content_rect, now);
        if let Some(command) = deferred {
            self.execute(&ctx, command);
        }
        self.modals(&ctx);
        self.extension_modals(&ctx);
        self.update_title(&ctx);
        open_requested_urls(&ctx);
    }
}

/// Liens demandés pendant l'image (`hyperlink`, `open_url`) : eframe est
/// compilé sans sa fonction « links », on les ouvre donc nous-mêmes dans le
/// navigateur.
fn open_requested_urls(ctx: &egui::Context) {
    let urls: Vec<String> = ctx.output_mut(|o| {
        let mut urls = Vec::new();
        o.commands.retain(|command| match command {
            egui::OutputCommand::OpenUrl(open) => {
                urls.push(open.url.clone());
                false
            }
            _ => true,
        });
        urls
    });
    for url in urls {
        let _ = open::that_detached(&url);
    }
}

/// Retire un filtre.
type ClearFilter = fn(&mut bp_core::filter::Filters);

/// Un badge par filtre actif ; un clic le retire.
/// Hauteur intérieure de l'île de la barre d'état.
const STATUS_HEIGHT: f32 = 22.0;
/// Taille du texte de la barre d'état.
const STATUS_FONT_SIZE: f32 = 12.5;

/// Île de la barre d'état, de hauteur fixe.
fn status_island(
    ui: &mut egui::Ui,
    pal: &crate::theme::Palette,
    dark: bool,
    content: impl FnOnce(&mut egui::Ui),
) {
    crate::islands::frame(pal, dark)
        .inner_margin(egui::Margin::symmetric(12, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(STATUS_HEIGHT);
            content(ui);
        });
}

fn filter_chips(ui: &mut egui::Ui, doc: &mut OpenDoc) {
    let f = &doc.filters;
    let mut chips: Vec<(String, ClearFilter)> = Vec::new();
    if !f.mode.is_empty() {
        chips.push((tr!("Mode : {}", f.mode), |f| f.mode.clear()));
    }
    if !f.payee.is_empty() {
        chips.push((tr!("Tiers : {}", f.payee), |f| f.payee.clear()));
    }
    if !f.category.is_empty() {
        chips.push((tr!("Catégorie : {}", f.category), |f| f.category.clear()));
    }
    if !f.amount.is_empty() {
        chips.push((tr!("Montant : {}", f.amount), |f| f.amount.clear()));
    }
    if f.tack != bp_core::filter::TackFilter::All {
        chips.push((tr!("Pointage : {}", f.tack.label().to_lowercase()), |f| {
            f.tack = bp_core::filter::TackFilter::All
        }));
    }
    let period = match (f.date_from, f.date_to) {
        (Some(a), Some(b)) => Some(tr!("Période : du {} au {}", a, b)),
        (Some(a), None) => Some(tr!("Période : à partir du {}", a)),
        (None, Some(b)) => Some(tr!("Période : jusqu'au {}", b)),
        (None, None) => None,
    };
    if let Some(period) = period {
        chips.push((period, |f| {
            f.date_from = None;
            f.date_to = None;
        }));
    }
    if f.hide_old_lines {
        chips.push((tr!("Lignes rapprochées anciennes masquées").into(), |f| {
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
        if response.on_hover_text(tr!("Retirer ce filtre")).clicked() {
            remove = Some(index);
        }
    }
    if chips.len() > 1
        && ui
            .link(RichText::new(tr!("Tout effacer")).small())
            .clicked()
    {
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
                .on_hover_text(tr!("Nouvelle opération (Inser)"));
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr!("Nouvelle opération"))
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
            let line = tr!("Ligne {} : {}", op.uid, money(op.amount, &currency));
            let tack = match op.tack {
                Tack::None => None,
                Tack::Pointed => Some(tr!("pointée")),
                Tack::Reconciled => Some(tr!("rapprochée")),
            };
            ui.label(match tack {
                Some(tack) => format!("{line} · {tack}"),
                None => line,
            });
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
            let mut text = RichText::new(tr!("{} lignes : {}", n, money(total, &currency)))
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

/// Garde l'original d'un fichier v5 avant sa conversion : `<nom>.version8.bp`
/// à côté, que BankPerfect 8 ouvre directement (jamais écrasé).
fn backup_v5_original(path: &Path) -> std::io::Result<()> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let backup = path.with_file_name(format!("{stem}.version8.bp"));
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
