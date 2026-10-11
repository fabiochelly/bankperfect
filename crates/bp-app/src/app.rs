//! Fenêtre principale : menus, mise en page, ouverture/enregistrement.

use std::path::{Path, PathBuf};

use bp_core::FileFormat;
use bp_core::file;
use bp_core::format_v6::{Protection, Unlock};
use bp_core::model::{Amount, Tack};
use egui::{Align, Key, KeyboardShortcut, Layout, Modifiers, RichText};
use egui_phosphor::regular as icon;

use crate::dialogs::{self, Choice, PasswordPrompt, PromptResult};
use crate::document::OpenDoc;
use crate::editor::{Editor, EditorAction};
use crate::format::money;
use crate::grid::{self, GridAction};
use crate::sidebar;
use crate::theme::palette;

const SHORTCUT_OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SHORTCUT_SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SHORTCUT_NEXT_ACCOUNT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::Tab);
const SHORTCUT_PREV_ACCOUNT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::Tab);
const SHORTCUT_HIDE_OLD: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::H);
const SHORTCUT_TWO_LINES: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F4);

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
    error: Option<String>,
    toast: Option<(String, f64)>,
    allow_close: bool,
    title: String,
}

impl BankPerfectApp {
    pub fn new(initial_file: Option<PathBuf>) -> Self {
        let mut app = Self {
            doc: None,
            editor: None,
            prompt: None,
            recovery_phrase: None,
            show_warnings: false,
            pending: None,
            confirm_delete: false,
            error: None,
            toast: None,
            allow_close: false,
            title: String::new(),
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
            Err(e) => return self.error = Some(format!("Impossible de lire {} : {e}", path.display())),
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
                self.doc = Some(OpenDoc::new(path, opened));
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
        if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT_TWO_LINES)) {
            doc.two_lines = !doc.two_lines;
        }

        // Raccourcis de la grille, actifs quand aucun champ n'a le focus.
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let pressed = |key| ctx.input(|i| i.key_pressed(key));
        if pressed(Key::ArrowDown) {
            doc.move_selection(1);
        } else if pressed(Key::ArrowUp) {
            doc.move_selection(-1);
        } else if pressed(Key::Space) || pressed(Key::P) {
            doc.cycle_tack();
        } else if pressed(Key::Insert) || pressed(Key::Plus) {
            self.editor = Some(Editor::new_operation(&doc.document, doc.account));
        } else if pressed(Key::Enter) && let Some(&index) = doc.selection.iter().next() {
            self.editor = Some(Editor::edit(&doc.document, doc.account, index));
        } else if pressed(Key::Delete) && !doc.selection.is_empty() {
            self.confirm_delete = true;
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match &self.doc {
            Some(doc) => format!("{}{} — BankPerfect", doc.file_name(), if doc.dirty { " •" } else { "" }),
            None => "BankPerfect".to_string(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Fichier", |ui| {
                if ui.add(egui::Button::new(format!("{}  Ouvrir…", icon::FOLDER_OPEN)).shortcut_text("Ctrl+O")).clicked() {
                    self.request(Pending::Open(None));
                }
                let can_save = self.doc.is_some();
                if ui
                    .add_enabled(can_save, egui::Button::new(format!("{}  Enregistrer", icon::FLOPPY_DISK)).shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    self.save(now);
                }
                ui.separator();
                if ui.button("Quitter").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            let Some(doc) = &mut self.doc else { return };
            ui.menu_button("Édition", |ui| {
                if ui.add(egui::Button::new(format!("{}  Nouvelle opération", icon::PLUS)).shortcut_text("Inser")).clicked() {
                    self.editor = Some(Editor::new_operation(&doc.document, doc.account));
                }
                let single = doc.selection.iter().next().copied();
                if ui.add_enabled(single.is_some(), egui::Button::new("Modifier").shortcut_text("Entrée")).clicked()
                    && let Some(index) = single
                {
                    self.editor = Some(Editor::edit(&doc.document, doc.account, index));
                }
                if ui
                    .add_enabled(!doc.selection.is_empty(), egui::Button::new(format!("{}  Supprimer", icon::TRASH)).shortcut_text("Suppr"))
                    .clicked()
                {
                    self.confirm_delete = true;
                }
            });
            ui.menu_button("Affichage", |ui| {
                ui.add(egui::Checkbox::new(&mut doc.two_lines, "Affichage sur 2 lignes (F4)"));
                ui.add(egui::Checkbox::new(&mut doc.filters.hide_old_lines, "Cacher les lignes inutiles (Ctrl+H)"));
                ui.separator();
                ui.label(RichText::new("Thème").small().color(palette(ui).muted));
                let current = ui.ctx().options(|o| o.theme_preference);
                for (preference, label) in [
                    (egui::ThemePreference::Dark, "Tokyo Night (sombre)"),
                    (egui::ThemePreference::Light, "Tokyo Night Day (clair)"),
                    (egui::ThemePreference::System, "Suivre le système"),
                ] {
                    if ui.radio(current == preference, label).clicked() {
                        ui.ctx().set_theme(preference);
                    }
                }
            });
            ui.menu_button("Comptes", |ui| {
                for &i in &doc.document.account_order.clone() {
                    let account = &doc.document.accounts[i];
                    if account.disabled {
                        continue;
                    }
                    if ui.selectable_label(i == doc.account, &account.name).clicked() {
                        doc.select_account(i);
                        self.editor = None;
                        ui.close();
                    }
                }
                ui.separator();
                if ui.add(egui::Button::new("Compte suivant").shortcut_text("Ctrl+Tab")).clicked() {
                    doc.cycle_account(true);
                }
                if ui.add(egui::Button::new("Compte précédent").shortcut_text("Ctrl+Maj+Tab")).clicked() {
                    doc.cycle_account(false);
                }
            });
            ui.menu_button("Opérations", |ui| {
                if ui.add_enabled(!doc.selection.is_empty(), egui::Button::new("Pointer").shortcut_text("Espace")).clicked() {
                    doc.cycle_tack();
                }
                if ui.button("Rapprocher toutes les lignes pointées").clicked() {
                    doc.reconcile_all_pointed();
                }
            });
        });
    }

    fn balance_strip(ui: &mut egui::Ui, doc: &OpenDoc) -> bool {
        let pal = palette(ui);
        let account = doc.current_account();
        let totals = doc.totals();
        let currency = &account.currency;
        let mut new_operation = false;
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(&account.name).size(22.0).family(crate::theme::semibold()));
                let mut sub = account.bank_name.clone();
                if !account.iban.trim().is_empty() {
                    sub = format!("{sub}  ·  {}", account.iban.trim());
                }
                ui.label(RichText::new(sub).small().color(pal.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let button = crate::theme::primary_button(ui, format!("{}  Nouvelle opération", icon::PLUS));
                if ui.add(button).on_hover_text("Inser").clicked() {
                    new_operation = true;
                }
            });
        });
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            card(ui, &pal, "Solde", totals.balance, currency, true);
            card(ui, &pal, "Pointé", totals.pointed, currency, false);
            card(ui, &pal, "À venir", totals.pending, currency, false);
            if let Some(date) = totals.last_reconciled {
                card(ui, &pal, &format!("Rapproché au {date}"), totals.reconciled, currency, false);
            }
            if doc.filters.is_active() {
                ui.label(RichText::new(format!("{}  soldes des lignes filtrées", icon::FUNNEL)).small().color(pal.pointed));
            }
        });
        ui.add_space(10.0);
        new_operation
    }

    fn status_bar(ui: &mut egui::Ui, doc: &mut OpenDoc, toast: Option<&str>) {
        let pal = palette(ui);
        ui.horizontal(|ui| {
            let shown = doc.visible().len();
            let total = doc.total_count();
            if doc.filters.is_active() {
                ui.label(RichText::new(format!("{}  {shown} / {total} opérations", icon::FUNNEL)).color(pal.pointed));
                if ui.small_button(format!("{}  Effacer les filtres", icon::FUNNEL_X)).clicked() {
                    let hide = doc.filters.hide_old_lines;
                    doc.filters = Default::default();
                    doc.filters.hide_old_lines = hide;
                }
            } else {
                ui.label(format!("{total} opérations"));
            }
            ui.separator();
            selection_summary(ui, doc);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("BankPerfect · Enregistré").small().color(pal.muted));
                if let Some(text) = toast {
                    ui.label(RichText::new(format!("{}  {text}", icon::CHECK)).color(pal.positive));
                }
            });
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.28);
            ui.label(RichText::new(icon::PIGGY_BANK).size(56.0).color(palette(ui).accent));
            ui.label(RichText::new("BankPerfect").size(30.0).family(crate::theme::semibold()));
            ui.add_space(18.0);
            if ui.add(egui::Button::new(RichText::new(format!("{}  Ouvrir un fichier…", icon::FOLDER_OPEN)).size(15.0))).clicked() {
                self.request(Pending::Open(None));
            }
            ui.add_space(8.0);
            ui.label(RichText::new("ou déposez un fichier .bp dans la fenêtre").color(palette(ui).muted));
        });
    }

    fn modals(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
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
                            let unlock = if recovery { Unlock::RecoveryKey(&input) } else { Unlock::Password(&input) };
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
            let message = if n == 1 { "Supprimer l'opération sélectionnée ?".to_string() } else { format!("Supprimer les {n} opérations sélectionnées ?") };
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
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        for event in &mut raw_input.events {
            if let egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, .. } = event {
                *delta *= crate::theme::WHEEL_POINT_FACTOR;
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        crate::theme::adapt_to_display(&ctx);

        if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty())) {
            self.request(Pending::Open(Some(path)));
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.doc.as_ref().is_some_and(|d| d.dirty) {
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
            || self.show_warnings;
        if !modal_open {
            self.handle_shortcuts(&ctx);
        }
        let now = ctx.input(|i| i.time);
        if self.toast.as_ref().is_some_and(|(_, until)| now > *until) {
            self.toast = None;
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));

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
            .frame(egui::Frame::side_top_panel(ui.style()).fill(sidebar_fill).inner_margin(12))
            .show(ui, |ui| sidebar::show(ui, doc));

        if let Some(editor) = &mut self.editor {
            let action = egui::Panel::right("editor")
                .resizable(true)
                .default_size(360.0)
                .min_size(300.0)
                .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(14))
                .show(ui, |ui| editor.show(ui, &doc.document))
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

        doc.refresh();
        let mut grid_action = GridAction::None;
        let new_operation = egui::Panel::top("balances")
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::symmetric(18, 0)))
            .show(ui, |ui| Self::balance_strip(ui, doc))
            .inner;
        if new_operation {
            grid_action = GridAction::New;
        }

        egui::CentralPanel::default_margins().show(ui, |ui| {
            let action = grid::show(ui, doc);
            if !matches!(action, GridAction::None) {
                grid_action = action;
            }
        });
        match grid_action {
            GridAction::None => {}
            GridAction::Edit(index) => self.editor = Some(Editor::edit(&doc.document, doc.account, index)),
            GridAction::New => self.editor = Some(Editor::new_operation(&doc.document, doc.account)),
        }

        self.modals(&ctx);
        self.update_title(&ctx);
    }
}

fn card(ui: &mut egui::Ui, pal: &crate::theme::Palette, label: &str, amount: Amount, currency: &str, primary: bool) {
    egui::Frame::new()
        .fill(pal.surface)
        .corner_radius(8)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(egui::Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.set_min_width(130.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(label).small().color(pal.muted));
                let mut text = RichText::new(money(amount, currency)).size(if primary { 18.0 } else { 15.0 }).family(crate::theme::semibold());
                if amount.is_negative() {
                    text = text.color(pal.negative);
                }
                ui.label(text);
            });
        });
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
            ui.label(format!("Ligne {} : {}{tack}", op.uid, money(op.amount, &currency)));
        }
        n => {
            let total: Amount = ops.iter().map(|op| op.amount).sum();
            let credits: Amount = ops.iter().filter(|op| !op.amount.is_negative()).map(|op| op.amount).sum();
            let debits: Amount = ops.iter().filter(|op| op.amount.is_negative()).map(|op| op.amount).sum();
            let mut text = RichText::new(format!("{n} lignes : {}", money(total, &currency))).family(crate::theme::semibold());
            if total.is_negative() {
                text = text.color(pal.negative);
            }
            ui.label(text);
            if credits != Amount::ZERO && debits != Amount::ZERO {
                ui.label(RichText::new(format!("+{}  {}", money(credits, &currency), money(debits, &currency))).color(pal.muted));
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
    file::save(&doc.path, &doc.document, doc.protection.as_ref().expect("protection choisie"))?;
    doc.format = FileFormat::V6;
    doc.dirty = false;
    Ok(recovery)
}

fn backup_v5_original(path: &Path) -> std::io::Result<()> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let backup = path.with_file_name(format!("{stem}_v5.bp0"));
    if !backup.exists() {
        std::fs::copy(path, backup)?;
    }
    Ok(())
}
