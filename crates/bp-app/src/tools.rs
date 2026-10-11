//! Fenêtres des outils (ex-scripts) : Remplacer, Modifier le groupe, export CSV.

use bp_core::edit::GroupEdit;
use bp_core::export::{CsvField, CsvOptions};
use bp_core::model::{Amount, Date, Mode, Tack};
use egui::{Id, Key, Modal, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::theme::palette;

pub enum Tool {
    Backups(BackupsForm),
    ExtraValues(ExtraForm),
    Notes(NotesForm),
    Replace(ReplaceForm),
    ModifyGroup(GroupForm),
    ExportCsv(CsvForm),
}

/// Ce qu'il faut faire après l'affichage d'un outil.
pub enum Outcome {
    Keep,
    Close,
    /// Fermer et afficher un message de confirmation.
    Done(String),
    /// Fermer, rouvrir ce fichier et afficher un message.
    Reopen(std::path::PathBuf, String),
}

impl Tool {
    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        match self {
            Tool::Backups(form) => form.show(ctx, doc),
            Tool::ExtraValues(form) => form.show(ctx, doc),
            Tool::Notes(form) => form.show_standalone(ctx),
            Tool::Replace(form) => form.show(ctx, doc),
            Tool::ModifyGroup(form) => form.show(ctx, doc),
            Tool::ExportCsv(form) => form.show(ctx, doc),
        }
    }
}

fn title(ui: &mut egui::Ui, glyph: &str, text: &str) {
    ui.label(RichText::new(format!("{glyph}  {text}")).heading());
    ui.add_space(8.0);
}

/// Boutons Valider / Annuler ; Entrée valide, Échap annule.
fn buttons(ui: &mut egui::Ui, ok: &str, enabled: bool) -> Option<bool> {
    ui.add_space(12.0);
    let mut result = None;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(enabled, crate::theme::primary_button(ui, ok))
            .clicked()
        {
            result = Some(true);
        }
        if ui.button("Annuler").clicked() {
            result = Some(false);
        }
    });
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        result = Some(false);
    }
    if enabled && ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command) {
        result = Some(true);
    }
    result
}

// ─────────────────────────────── Remplacer ───────────────────────────────

#[derive(Default)]
pub struct ReplaceForm {
    find: String,
    replacement: String,
}

impl ReplaceForm {
    fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        Modal::new(Id::new("tool_replace")).show(ctx, |ui| {
            ui.set_width(420.0);
            title(ui, icon::SWAP, "Remplacer");
            ui.label(
                RichText::new(
                    "Dans le tiers et le détail des lignes affichées (respecte la casse).",
                )
                .small()
                .color(palette(ui).muted),
            );
            ui.add_space(8.0);
            egui::Grid::new("replace")
                .num_columns(2)
                .spacing([10.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Rechercher");
                    let find = ui.add(crate::theme::field(&mut self.find).desired_width(280.0));
                    if find.ctx.memory(|m| m.focused().is_none()) {
                        find.request_focus();
                    }
                    ui.end_row();
                    ui.label("Remplacer par");
                    ui.add(crate::theme::field(&mut self.replacement).desired_width(280.0));
                    ui.end_row();
                });
            let count =
                bp_core::edit::count_matches(doc.current_account(), &doc.visible(), &self.find);
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("{count} occurrence(s) trouvée(s)"))
                    .small()
                    .color(palette(ui).muted),
            );
            match buttons(ui, &format!("{}  Remplacer", icon::SWAP), count > 0) {
                Some(true) => {
                    let n = doc.replace_in_visible(&self.find, &self.replacement);
                    outcome = Outcome::Done(format!("{n} remplacement(s) effectué(s)"));
                }
                Some(false) => outcome = Outcome::Close,
                None => {}
            }
        });
        outcome
    }
}

// ─────────────────────────── Modifier le groupe ───────────────────────────

pub struct GroupForm {
    date: (bool, String),
    mode: (bool, Mode),
    payee: (bool, String),
    details: (bool, String),
    category: (bool, Option<u32>),
    amount: (bool, String),
    tack: (bool, Tack),
    error: Option<String>,
}

impl GroupForm {
    pub fn new() -> Self {
        Self {
            date: (false, Date::today().to_string()),
            mode: (false, Mode::default()),
            payee: (false, String::new()),
            details: (false, String::new()),
            category: (false, None),
            amount: (false, String::new()),
            tack: (false, Tack::None),
            error: None,
        }
    }

    fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let lines = doc.selected_visible().len();
        Modal::new(Id::new("tool_group")).show(ctx, |ui| {
            ui.set_width(480.0);
            title(
                ui,
                icon::PENCIL_SIMPLE_LINE,
                "Modifier le groupe d'opérations",
            );
            ui.label(
                RichText::new(format!(
                    "{lines} ligne(s) sélectionnée(s). Cochez les champs à modifier."
                ))
                .small()
                .color(palette(ui).muted),
            );
            ui.add_space(8.0);
            let document = &doc.document;
            egui::Grid::new("group")
                .num_columns(2)
                .spacing([12.0, 10.0])
                .show(ui, |ui| {
                    ui.checkbox(&mut self.date.0, "Date");
                    ui.add_enabled_ui(self.date.0, |ui| {
                        crate::datepicker::date_field(
                            ui,
                            "group_date",
                            &mut self.date.1,
                            200.0,
                            crate::datepicker::Placement::Popup,
                        )
                    });
                    ui.end_row();

                    ui.checkbox(&mut self.mode.0, "Mode");
                    ui.add_enabled_ui(self.mode.0, |ui| {
                        egui::ComboBox::from_id_salt("group_mode")
                            .selected_text(self.mode.1.label().to_string())
                            .width(260.0)
                            .show_ui(ui, |ui| {
                                for mode in document.mode_list() {
                                    let label = mode.label().to_string();
                                    ui.selectable_value(&mut self.mode.1, mode, label);
                                }
                            });
                    });
                    ui.end_row();

                    ui.checkbox(&mut self.payee.0, "Tiers");
                    ui.add_enabled(
                        self.payee.0,
                        crate::theme::field(&mut self.payee.1).desired_width(260.0),
                    );
                    ui.end_row();

                    ui.checkbox(&mut self.details.0, "Détail");
                    ui.add_enabled(
                        self.details.0,
                        crate::theme::field(&mut self.details.1).desired_width(260.0),
                    );
                    ui.end_row();

                    ui.checkbox(&mut self.category.0, "Catégorie");
                    ui.add_enabled_ui(self.category.0, |ui| {
                        let selected = self
                            .category
                            .1
                            .and_then(|id| document.category_label(id))
                            .unwrap_or_else(|| "(aucune)".into());
                        egui::ComboBox::from_id_salt("group_category")
                            .selected_text(selected)
                            .width(260.0)
                            .height(360.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.category.1, None, "(aucune)");
                                for c in &document.categories {
                                    let label = if c.is_child {
                                        format!("      {}", c.name)
                                    } else {
                                        c.name.clone()
                                    };
                                    ui.selectable_value(&mut self.category.1, Some(c.id), label);
                                }
                            });
                    });
                    ui.end_row();

                    ui.checkbox(&mut self.amount.0, "Montant");
                    ui.add_enabled(
                        self.amount.0,
                        crate::theme::field(&mut self.amount.1)
                            .hint_text("signe selon le mode")
                            .desired_width(160.0),
                    );
                    ui.end_row();

                    ui.checkbox(&mut self.tack.0, "Pointage");
                    ui.add_enabled_ui(self.tack.0, |ui| {
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.tack.1, Tack::None, "Non pointée");
                            ui.selectable_value(&mut self.tack.1, Tack::Pointed, "Pointée");
                            ui.selectable_value(&mut self.tack.1, Tack::Reconciled, "Rapprochée");
                        });
                    });
                    ui.end_row();
                });
            if let Some(error) = &self.error {
                ui.label(RichText::new(error).color(palette(ui).negative));
            }
            let any = self.date.0
                || self.mode.0
                || self.payee.0
                || self.details.0
                || self.category.0
                || self.amount.0
                || self.tack.0;
            match buttons(ui, &format!("{}  Appliquer", icon::CHECK), any && lines > 0) {
                Some(true) => match self.edit() {
                    Ok(edit) => {
                        let n = doc.apply_group_edit(&edit);
                        outcome = Outcome::Done(format!("{n} ligne(s) modifiée(s)"));
                    }
                    Err(e) => self.error = Some(e),
                },
                Some(false) => outcome = Outcome::Close,
                None => {}
            }
        });
        outcome
    }

    fn edit(&self) -> Result<GroupEdit, String> {
        let date = if self.date.0 {
            Some(Date::parse_fr(&self.date.1, Date::today()).ok_or("Date invalide")?)
        } else {
            None
        };
        let amount = if self.amount.0 {
            Some(Amount::parse(&self.amount.1).ok_or("Montant invalide")?)
        } else {
            None
        };
        Ok(GroupEdit {
            date,
            mode: self.mode.0.then(|| self.mode.1.clone()),
            payee: self.payee.0.then(|| self.payee.1.clone()),
            details: self.details.0.then(|| self.details.1.clone()),
            category: self.category.0.then_some(self.category.1),
            amount,
            tack: self.tack.0.then_some(self.tack.1),
        })
    }
}

// ─────────────────────────────── Export CSV ───────────────────────────────

pub struct CsvForm {
    /// Colonnes dans l'ordre, avec leur case « exporter ».
    fields: Vec<(CsvField, bool)>,
    separator: usize,
    all_accounts: bool,
    header: bool,
    cheque_as_mode: bool,
    group: bool,
    group_separator: String,
    tack_symbols: [String; 3],
}

const SEPARATORS: [(char, &str); 3] = [
    (';', "Point-virgule (;)"),
    (',', "Virgule (,)"),
    ('\t', "Tabulation"),
];

impl CsvForm {
    pub fn new() -> Self {
        Self {
            fields: CsvField::ALL.iter().map(|f| (*f, true)).collect(),
            separator: 0,
            all_accounts: false,
            header: true,
            cheque_as_mode: false,
            group: false,
            group_separator: " > ".into(),
            tack_symbols: ["-".into(), "P".into(), "R".into()],
        }
    }

    fn options(&self) -> CsvOptions {
        CsvOptions {
            fields: self
                .fields
                .iter()
                .filter(|(_, on)| *on)
                .map(|(f, _)| *f)
                .collect(),
            separator: SEPARATORS[self.separator].0,
            header: self.header,
            cheque_as_mode: self.cheque_as_mode,
            group_categories: self.group.then(|| self.group_separator.clone()),
            tack_symbols: self.tack_symbols.clone(),
        }
    }

    fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        Modal::new(Id::new("tool_csv")).show(ctx, |ui| {
            ui.set_width(560.0);
            title(ui, icon::FILE_CSV, "Exporter vers un fichier CSV");
            ui.columns(2, |cols| {
                let ui = &mut cols[0];
                ui.label(RichText::new("Colonnes (dans l'ordre)").family(crate::theme::medium()));
                let mut swap = None;
                let count = self.fields.len();
                for (i, (field, on)) in self.fields.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(on, field.label());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled(
                                    i + 1 < count,
                                    egui::Button::new(icon::ARROW_DOWN).frame(false),
                                )
                                .clicked()
                            {
                                swap = Some((i, i + 1));
                            }
                            if ui
                                .add_enabled(i > 0, egui::Button::new(icon::ARROW_UP).frame(false))
                                .clicked()
                            {
                                swap = Some((i - 1, i));
                            }
                        });
                    });
                }
                if let Some((a, b)) = swap {
                    self.fields.swap(a, b);
                }

                let ui = &mut cols[1];
                ui.label(RichText::new("Options").family(crate::theme::medium()));
                egui::ComboBox::from_label("Séparateur")
                    .selected_text(SEPARATORS[self.separator].1)
                    .show_ui(ui, |ui| {
                        for (i, (_, label)) in SEPARATORS.iter().enumerate() {
                            ui.selectable_value(&mut self.separator, i, *label);
                        }
                    });
                ui.checkbox(
                    &mut self.all_accounts,
                    "Exporter tous les comptes (un fichier par compte)",
                );
                ui.checkbox(&mut self.header, "Inclure la ligne d'en-tête");
                ui.checkbox(
                    &mut self.cheque_as_mode,
                    "Remplacer « Chq XXX » par « Chèque émis »",
                );
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.group, "Grouper catégorie et sous-catégorie");
                    ui.add_enabled(
                        self.group,
                        egui::TextEdit::singleline(&mut self.group_separator).desired_width(40.0),
                    );
                });
                ui.label("Symboles de pointage (non / pointée / rapprochée) :");
                ui.horizontal(|ui| {
                    for symbol in &mut self.tack_symbols {
                        ui.add(egui::TextEdit::singleline(symbol).desired_width(32.0));
                    }
                });
                ui.label(
                    RichText::new(if self.all_accounts {
                        "Toutes les opérations de chaque compte."
                    } else {
                        "Seules les lignes affichées du compte courant (filtres appliqués)."
                    })
                    .small()
                    .color(palette(ui).muted),
                );
            });
            let any = self.fields.iter().any(|(_, on)| *on);
            match buttons(ui, &format!("{}  Exporter", icon::EXPORT), any) {
                Some(true) => {
                    if let Some(message) = self.export(doc) {
                        outcome = Outcome::Done(message);
                    }
                }
                Some(false) => outcome = Outcome::Close,
                None => {}
            }
        });
        outcome
    }

    /// Écrit le ou les fichiers ; `None` si l'utilisateur a annulé le choix.
    fn export(&self, doc: &OpenDoc) -> Option<String> {
        let options = self.options();
        let stem = doc
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let start_dir = doc
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let write = |path: &std::path::Path, text: String| {
            // UTF-8 avec BOM : reconnu par Excel et LibreOffice.
            std::fs::write(path, [b"\xEF\xBB\xBF".as_slice(), text.as_bytes()].concat())
        };
        if self.all_accounts {
            let folder = rfd::FileDialog::new()
                .set_title("Dossier des fichiers CSV")
                .set_directory(&start_dir)
                .pick_folder()?;
            let mut written = 0;
            for account in &doc.document.accounts {
                let lines: Vec<usize> = (0..account.operations.len()).collect();
                let path = folder.join(bp_core::export::suggested_file_name(&stem, account, "csv"));
                if write(
                    &path,
                    bp_core::export::csv(&doc.document, account, &lines, &options),
                )
                .is_ok()
                {
                    written += 1;
                }
            }
            Some(format!(
                "{written} fichier(s) CSV créé(s) dans {}",
                folder.display()
            ))
        } else {
            let account = doc.current_account();
            let path = rfd::FileDialog::new()
                .set_title("Exporter vers un fichier CSV")
                .set_directory(&start_dir)
                .set_file_name(bp_core::export::suggested_file_name(&stem, account, "csv"))
                .add_filter("CSV", &["csv"])
                .save_file()?;
            let text = bp_core::export::csv(&doc.document, account, &doc.visible(), &options);
            Some(match write(&path, text) {
                Ok(()) => format!("Fichier {} créé", path.display()),
                Err(e) => format!("Impossible de créer {} : {e}", path.display()),
            })
        }
    }
}

/// Export QIF des lignes affichées du compte courant (pas d'options).
pub fn export_qif(doc: &OpenDoc) -> Option<String> {
    let stem = doc
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let account = doc.current_account();
    let path = rfd::FileDialog::new()
        .set_title("Exporter vers un fichier QIF")
        .set_directory(doc.path.parent().unwrap_or(std::path::Path::new(".")))
        .set_file_name(bp_core::export::suggested_file_name(&stem, account, "qif"))
        .add_filter("QIF", &["qif"])
        .save_file()?;
    let text = bp_core::export::qif(&doc.document, account, &doc.visible());
    // Les logiciels qui lisent le QIF attendent de l'ANSI (Windows-1252).
    let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(&text);
    Some(match std::fs::write(&path, bytes) {
        Ok(()) => format!("Fichier {} créé", path.display()),
        Err(e) => format!("Impossible de créer {} : {e}", path.display()),
    })
}

// ─────────────────────────────── Sauvegardes ───────────────────────────────

/// Liste des sauvegardes `.bp0` du fichier ouvert, avec restauration.
pub struct BackupsForm {
    dir: std::path::PathBuf,
    keep: usize,
    confirm: Option<std::path::PathBuf>,
    error: Option<String>,
}

impl BackupsForm {
    pub fn new(dir: std::path::PathBuf, keep: usize) -> Self {
        Self {
            dir,
            keep,
            confirm: None,
            error: None,
        }
    }

    fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let backups = bp_core::backup::list(&doc.path, &self.dir);
        Modal::new(Id::new("tool_backups")).show(ctx, |ui| {
            ui.set_width(560.0);
            title(ui, icon::CLOCK_COUNTER_CLOCKWISE, "Sauvegardes");
            let pal = palette(ui);
            ui.label(RichText::new(format!("Dossier : {}", self.dir.display())).small().color(pal.muted));
            ui.label(
                RichText::new(format!("Une sauvegarde est créée à chaque enregistrement ; les {} plus récentes sont gardées.", self.keep))
                    .small()
                    .color(pal.muted),
            );
            ui.add_space(8.0);
            if backups.is_empty() {
                ui.label("Aucune sauvegarde pour ce fichier pour l'instant.");
            }
            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                for backup in &backups {
                    ui.horizontal(|ui| {
                        let when: chrono::DateTime<chrono::Local> = backup.modified.into();
                        ui.label(RichText::new(icon::FILE).color(pal.muted));
                        ui.label(when.format("%d/%m/%Y à %H:%M:%S").to_string());
                        ui.label(RichText::new(format!("{} Ko", backup.size.div_ceil(1024))).small().color(pal.muted));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(format!("{}  Restaurer", icon::ARROW_COUNTER_CLOCKWISE)).clicked() {
                                self.confirm = Some(backup.path.clone());
                            }
                        });
                    });
                }
            });
            if let Some(error) = &self.error {
                ui.label(RichText::new(error).color(pal.negative));
            }
            if let Some(path) = self.confirm.clone() {
                ui.add_space(8.0);
                let unsaved = if doc.dirty { " Les modifications non enregistrées seront perdues." } else { "" };
                ui.label(
                    RichText::new(format!("Restaurer cette sauvegarde ? La version actuelle du fichier sera elle-même sauvegardée.{unsaved}"))
                        .color(pal.warning),
                );
                ui.horizontal(|ui| {
                    if ui.add(crate::theme::primary_button(ui, "Restaurer")).clicked() {
                        match bp_core::backup::restore(&path, &doc.path, &self.dir, self.keep) {
                            Ok(()) => outcome = Outcome::Reopen(doc.path.clone(), "Sauvegarde restaurée".into()),
                            Err(e) => self.error = Some(format!("Restauration impossible : {e}")),
                        }
                    }
                    if ui.button("Non").clicked() {
                        self.confirm = None;
                    }
                });
            }
            ui.add_space(10.0);
            if ui.button("Fermer").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                outcome = Outcome::Close;
            }
        });
        outcome
    }
}

// ─────────────────────────── Champs supplémentaires ───────────────────────────

/// Champs supplémentaires d'une opération : modifier, ajouter, supprimer.
/// Les champs internes (`#…`, `freeicon`) sont affichés sans être modifiables.
pub struct ExtraForm {
    index: usize,
    entries: Vec<(String, String)>,
    new_key: String,
    new_value: String,
    error: Option<String>,
}

fn is_internal(key: &str) -> bool {
    key.starts_with('#') || key == "freeicon"
}

impl ExtraForm {
    pub fn new(doc: &OpenDoc, index: usize) -> Self {
        Self {
            index,
            entries: doc.current_account().operations[index].extra.clone(),
            new_key: String::new(),
            new_value: String::new(),
            error: None,
        }
    }

    fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let op = doc.current_account().operations.get(self.index).cloned();
        let Some(op) = op else { return Outcome::Close };
        Modal::new(Id::new("tool_extra")).show(ctx, |ui| {
            ui.set_width(520.0);
            title(ui, icon::LIST_PLUS, "Champs supplémentaires");
            let pal = palette(ui);
            ui.label(
                RichText::new(format!("{} · {} · ligne {}", op.date, op.payee, op.uid))
                    .small()
                    .color(pal.muted),
            );
            ui.add_space(8.0);
            let mut remove = None;
            for (i, (key, value)) in self.entries.iter_mut().enumerate() {
                let internal = is_internal(key);
                ui.horizontal(|ui| {
                    let color = if internal { pal.muted } else { pal.fg };
                    ui.add_sized(
                        [130.0, 32.0],
                        egui::Label::new(RichText::new(key.as_str()).color(color)).truncate(),
                    );
                    ui.add_enabled_ui(!internal, |ui| {
                        ui.add_sized([300.0, 32.0], crate::theme::field(value))
                    });
                    if !internal
                        && ui
                            .button(icon::TRASH)
                            .on_hover_text("Supprimer ce champ")
                            .clicked()
                    {
                        remove = Some(i);
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add_sized(
                    [130.0, 32.0],
                    crate::theme::field(&mut self.new_key).hint_text("nouveau nom"),
                );
                ui.add_sized(
                    [300.0, 32.0],
                    crate::theme::field(&mut self.new_value).hint_text("valeur"),
                );
                if ui.button(icon::PLUS).on_hover_text("Ajouter").clicked() {
                    self.add();
                }
            });
            if let Some(i) = remove {
                self.entries.remove(i);
            }
            if let Some(error) = &self.error {
                ui.label(RichText::new(error).color(pal.negative));
            }
            match buttons(ui, &format!("{}  Enregistrer", icon::CHECK), true) {
                Some(true) => {
                    if !self.new_key.trim().is_empty() {
                        self.add();
                    }
                    if self.error.is_none() {
                        let entries: Vec<(String, String)> = self
                            .entries
                            .iter()
                            .filter(|(_, v)| !v.is_empty())
                            .cloned()
                            .collect();
                        doc.set_extra(self.index, entries);
                        outcome = Outcome::Done("Champs supplémentaires enregistrés".into());
                    }
                }
                Some(false) => outcome = Outcome::Close,
                None => {}
            }
        });
        outcome
    }

    fn add(&mut self) {
        let key = self.new_key.trim().replace([';', '='], " ");
        if key.is_empty() || key == "UId" || is_internal(&key) {
            self.error = Some("Nom de champ invalide ou réservé".into());
            return;
        }
        match self
            .entries
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        {
            Some(entry) => entry.1 = self.new_value.clone(),
            None => self.entries.push((key, self.new_value.clone())),
        }
        self.new_key.clear();
        self.new_value.clear();
        self.error = None;
    }
}

// ─────────────────────────────── Notes ───────────────────────────────

pub struct NotesForm {
    notes: Vec<crate::notes::Note>,
    current: Option<usize>,
    /// Seulement les notes à afficher à l'ouverture (lecture).
    on_open: bool,
}

impl NotesForm {
    pub fn new() -> Self {
        let notes = crate::notes::load();
        let current = (!notes.is_empty()).then_some(0);
        Self {
            notes,
            current,
            on_open: false,
        }
    }

    /// Notes marquées « afficher à l'ouverture », ou `None` s'il n'y en a pas.
    pub fn on_open() -> Option<Self> {
        let notes: Vec<_> = crate::notes::load()
            .into_iter()
            .filter(|n| n.show_on_open)
            .collect();
        (!notes.is_empty()).then_some(Self {
            notes,
            current: Some(0),
            on_open: true,
        })
    }

    /// Les notes ne dépendent d'aucun fichier ouvert.
    pub fn show_standalone(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Keep;
        Modal::new(Id::new("tool_notes")).show(ctx, |ui| {
            ui.set_width(680.0);
            title(
                ui,
                icon::NOTE,
                if self.on_open {
                    "Notes à l'ouverture"
                } else {
                    "Notes"
                },
            );
            let pal = palette(ui);
            // Hauteur fixe : sans elle, le séparateur vertical étire la fenêtre.
            ui.allocate_ui_with_layout(
                egui::vec2(680.0, 380.0),
                egui::Layout::left_to_right(egui::Align::Min),
                |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(200.0);
                        egui::ScrollArea::vertical()
                            .id_salt("notes_list")
                            .max_height(320.0)
                            .show(ui, |ui| {
                                for (i, note) in self.notes.iter().enumerate() {
                                    let label = if note.title.is_empty() {
                                        "(sans titre)"
                                    } else {
                                        note.title.as_str()
                                    };
                                    if ui
                                        .selectable_label(self.current == Some(i), label)
                                        .clicked()
                                    {
                                        self.current = Some(i);
                                    }
                                }
                            });
                        if !self.on_open {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui.button(format!("{}  Nouvelle", icon::PLUS)).clicked() {
                                    self.notes.push(crate::notes::Note {
                                        title: "Nouvelle note".into(),
                                        ..Default::default()
                                    });
                                    self.current = Some(self.notes.len() - 1);
                                }
                                if let Some(i) = self.current
                                    && ui
                                        .button(icon::TRASH)
                                        .on_hover_text("Supprimer la note")
                                        .clicked()
                                {
                                    self.notes.remove(i);
                                    self.current = if self.notes.is_empty() {
                                        None
                                    } else {
                                        Some(i.min(self.notes.len() - 1))
                                    };
                                }
                            });
                        }
                    });
                    ui.separator();
                    ui.vertical(
                        |ui| match self.current.and_then(|i| self.notes.get_mut(i)) {
                            Some(note) if self.on_open => {
                                ui.label(
                                    RichText::new(&note.title).family(crate::theme::semibold()),
                                );
                                ui.label(&note.text);
                            }
                            Some(note) => {
                                ui.add(
                                    crate::theme::field(&mut note.title)
                                        .hint_text("Titre")
                                        .desired_width(f32::INFINITY),
                                );
                                ui.add(
                                    egui::TextEdit::multiline(&mut note.text)
                                        .desired_rows(12)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.checkbox(
                                    &mut note.show_on_open,
                                    "Afficher à l'ouverture d'un fichier",
                                );
                            }
                            None => {
                                ui.label(
                                    RichText::new("Aucune note. Créez-en une avec « Nouvelle ».")
                                        .color(pal.muted),
                                );
                            }
                        },
                    );
                },
            );
            ui.add_space(10.0);
            if ui
                .add(crate::theme::primary_button(
                    ui,
                    if self.on_open {
                        "Fermer"
                    } else {
                        "Enregistrer et fermer"
                    },
                ))
                .clicked()
                || ui.input(|i| i.key_pressed(Key::Escape))
            {
                if self.on_open {
                    outcome = Outcome::Close;
                } else {
                    outcome = match crate::notes::save(&self.notes) {
                        Ok(()) => Outcome::Done("Notes enregistrées".into()),
                        Err(e) => Outcome::Done(format!("Notes non enregistrées : {e}")),
                    };
                }
            }
        });
        outcome
    }
}
