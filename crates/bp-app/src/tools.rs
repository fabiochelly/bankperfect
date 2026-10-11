//! Fenêtres des outils (ex-scripts) : Remplacer, Modifier le groupe, export CSV.

use bp_core::edit::GroupEdit;
use bp_core::export::{CsvField, CsvOptions};
use bp_core::model::{Amount, Date, Mode, Tack};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::theme::palette;
use crate::ui_kit;

pub enum Tool {
    NewFile(crate::files_ui::NewFileForm),
    Rules(Box<crate::rules_ui::RulesForm>),
    Reports(Box<crate::reports_ui::ReportsForm>),
    Charts(Box<crate::charts_ui::ChartsForm>),
    Budget(Box<crate::budget_ui::BudgetForm>),
    Filters(Box<crate::filters_ui::FiltersForm>),
    Scheduler(Box<crate::scheduler_ui::SchedulerForm>),
    Forecast(crate::scheduler_ui::ForecastForm),
    Import(Box<crate::import_ui::ImportForm>),
    Print(PrintForm),
    MarkDate(MarkDateForm),
    Manage(crate::manage_ui::ManageForm),
    Protection(crate::protection_ui::ProtectionForm),
    Backups(BackupsForm),
    Calculator(CalcForm),
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
    /// Fermer et montrer la nouvelle clé de secours.
    Protected(String),
    /// Remplacer cette fenêtre par une autre.
    Switch(Box<Tool>),
}

impl Tool {
    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        match self {
            Tool::NewFile(form) => {
                form.show_standalone(ctx, doc.path.parent().map(std::path::Path::to_path_buf))
            }
            Tool::Manage(form) => form.show(ctx, doc),
            Tool::Rules(form) => form.show(ctx, doc),
            Tool::Reports(form) => form.show(ctx, doc),
            Tool::Charts(form) => form.show(ctx, doc),
            Tool::Budget(form) => form.show(ctx, doc),
            Tool::Filters(form) => form.show(ctx, doc),
            Tool::Scheduler(form) => form.show(ctx, doc),
            Tool::Forecast(form) => form.show(ctx, doc),
            Tool::Import(form) => form.show(ctx, doc),
            Tool::Print(form) => form.show(ctx, doc),
            Tool::MarkDate(form) => form.show(ctx, doc),
            Tool::Protection(form) => form.show(ctx, doc),
            Tool::Backups(form) => form.show_standalone(ctx, doc.dirty),
            Tool::Calculator(form) => form.show_standalone(ctx),
            Tool::ExtraValues(form) => form.show(ctx, doc),
            Tool::Notes(form) => form.show_standalone(ctx),
            Tool::Replace(form) => form.show(ctx, doc),
            Tool::ModifyGroup(form) => form.show(ctx, doc),
            Tool::ExportCsv(form) => form.show(ctx, doc),
        }
    }
}

fn title(ui: &mut egui::Ui, glyph: &str, text: &str) {
    let color = palette(ui).accent;
    crate::dialogs::header(ui, glyph, color, text);
}

/// Boutons Valider / Annuler ; Entrée valide, Échap annule.
fn buttons(ui: &mut egui::Ui, ok: &str, enabled: bool) -> Option<bool> {
    let mut result = None;
    crate::dialogs::footer(ui, |ui| {
        if crate::dialogs::primary(ui, ok, enabled).clicked() {
            result = Some(true);
        }
        if crate::dialogs::button(ui, "Annuler").clicked() {
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

// ─────────────────────────────── Impression ───────────────────────────────

/// « Imprimer le relevé » : PDF des lignes affichées, ouvert dans la
/// visionneuse du système (aperçu et impression) ou enregistré.
pub struct PrintForm {
    black_and_white: bool,
    details: bool,
    error: Option<String>,
}

impl Default for PrintForm {
    fn default() -> Self {
        Self {
            black_and_white: false,
            details: true,
            error: None,
        }
    }
}

impl PrintForm {
    fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.4, 560.0, 640.0);
        crate::dialogs::modal(ctx, "tool_print").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            title(ui, icon::PRINTER, "Imprimer le relevé");
            let n = doc.visible_len();
            ui_kit::subtitle(
                ui,
                format!(
                    "Relevé du compte « {} » : {n} ligne(s) affichée(s), dans l'ordre et avec les filtres de la grille.",
                    doc.current_account().name
                ),
            );
            ui_kit::section(ui, width, icon::SLIDERS_HORIZONTAL, "Présentation", |ui| {
                crate::theme::checkbox(ui, &mut self.details, "Afficher le détail sous le tiers");
                ui.add_space(8.0);
                crate::theme::checkbox(ui, &mut self.black_and_white, "Noir et blanc");
            });
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            let options = crate::statement_pdf::StatementOptions {
                black_and_white: self.black_and_white,
                details: self.details,
            };
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, &format!("{}  Ouvrir pour imprimer", icon::PRINTER), n > 0).clicked() {
                    match open_statement(doc, options) {
                        Ok(message) => outcome = Outcome::Done(message),
                        Err(e) => self.error = Some(e),
                    }
                }
                if crate::dialogs::button(ui, format!("{}  Enregistrer en PDF…", icon::FILE_PDF)).clicked() {
                    match save_statement(doc, options) {
                        Ok(Some(message)) => outcome = Outcome::Done(message),
                        Ok(None) => {}
                        Err(e) => self.error = Some(e),
                    }
                }
                if crate::dialogs::button(ui, "Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            });
        });
        outcome
    }
}

fn statement_name(doc: &OpenDoc) -> String {
    format!(
        "Relevé {} {}.pdf",
        bp_core::manage::clean(&doc.current_account().name).replace(['/', '\\'], "-"),
        chrono::Local::now().format("%Y-%m-%d")
    )
}

/// Écrit le relevé dans le dossier temporaire et l'ouvre.
fn open_statement(
    doc: &OpenDoc,
    options: crate::statement_pdf::StatementOptions,
) -> Result<String, String> {
    let bytes = crate::statement_pdf::build(doc, options)?;
    let dir = std::env::temp_dir().join("bankperfect");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(statement_name(doc));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    open::that_detached(&path).map_err(|e| format!("Impossible d'ouvrir le PDF : {e}"))?;
    Ok("Relevé ouvert dans la visionneuse PDF".into())
}

/// Enregistre le relevé à l'emplacement choisi ; `None` si annulé.
fn save_statement(
    doc: &OpenDoc,
    options: crate::statement_pdf::StatementOptions,
) -> Result<Option<String>, String> {
    let dir =
        crate::settings::shared::export_dir(doc.path.parent().unwrap_or(std::path::Path::new(".")));
    let Some(path) = rfd::FileDialog::new()
        .set_title("Enregistrer le relevé en PDF")
        .set_directory(dir)
        .set_file_name(statement_name(doc))
        .add_filter("PDF", &["pdf"])
        .save_file()
    else {
        return Ok(None);
    };
    let bytes = crate::statement_pdf::build(doc, options)?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(Some(format!("Relevé enregistré : {}", path.display())))
}

// ─────────────────────────────── Date de pointage ───────────────────────────────

/// Date de pointage manuelle des lignes sélectionnées (MARK-05).
pub struct MarkDateForm {
    date: String,
    lines: usize,
    error: Option<String>,
}

impl MarkDateForm {
    pub fn new(doc: &OpenDoc) -> Self {
        let first = doc
            .selection
            .iter()
            .next()
            .map(|&i| doc.operation(i).mark_date);
        Self {
            date: first.unwrap_or_else(Date::today).to_string(),
            lines: doc.selection.len(),
            error: None,
        }
    }

    fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        crate::dialogs::modal(ctx, "tool_mark_date").show(ctx, |ui| {
            ui.set_width(460.0);
            let pal = palette(ui);
            title(ui, icon::CALENDAR_CHECK, "Date de pointage");
            let text = if self.lines == 1 {
                "Date à laquelle la ligne sélectionnée a été pointée.".to_string()
            } else {
                format!(
                    "Date à laquelle les {} lignes sélectionnées ont été pointées.",
                    self.lines
                )
            };
            ui_kit::subtitle(ui, text);
            let field = crate::datepicker::date_field(
                ui,
                "mark_date",
                &mut self.date,
                240.0,
                crate::datepicker::Placement::Popup,
            );
            if field.ctx.memory(|m| m.focused().is_none()) {
                field.request_focus();
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            let mut result = buttons(ui, &format!("{}  Appliquer", icon::CHECK), true);
            if enter {
                result = Some(true);
            }
            match result {
                Some(true) => match Date::parse_fr(&self.date, Date::today()) {
                    Some(date) => {
                        doc.set_mark_date(date);
                        outcome = Outcome::Done(format!("Date de pointage : {date}"));
                    }
                    None => self.error = Some("Date invalide (jj/mm/aaaa)".into()),
                },
                Some(false) => outcome = Outcome::Close,
                None => {}
            }
        });
        outcome
    }
}

// ─────────────────────────────── Remplacer ───────────────────────────────

#[derive(Default)]
pub struct ReplaceForm {
    find: String,
    replacement: String,
}

/// Texte avec les occurrences de `find` mises en valeur.
fn highlighted(text: &str, find: &str, base: TextFormat, mark: TextFormat) -> LayoutJob {
    let mut job = LayoutJob::default();
    if find.is_empty() {
        job.append(text, 0.0, base);
        return job;
    }
    let mut rest = text;
    while let Some(pos) = rest.find(find) {
        job.append(&rest[..pos], 0.0, base.clone());
        job.append(&rest[pos..pos + find.len()], 0.0, mark.clone());
        rest = &rest[pos + find.len()..];
    }
    job.append(rest, 0.0, base);
    job
}

impl ReplaceForm {
    fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.45, 600.0, 720.0);
        crate::dialogs::modal(ctx, "tool_replace").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            title(ui, icon::SWAP, "Remplacer");
            ui_kit::subtitle(
                ui,
                "Remplace un texte dans le tiers et le détail des lignes affichées. La casse est respectée.",
            );
            ui_kit::section(ui, width, icon::TEXT_AA, "Texte", |ui| {
                let field_width = ui.available_width();
                let find = ui_kit::icon_field(
                    ui,
                    icon::MAGNIFYING_GLASS,
                    &mut self.find,
                    "Texte à rechercher",
                    field_width,
                );
                if find.ctx.memory(|m| m.focused().is_none()) {
                    find.request_focus();
                }
                ui.add_space(10.0);
                ui_kit::icon_field(
                    ui,
                    icon::ARROW_BEND_DOWN_RIGHT,
                    &mut self.replacement,
                    "Remplacer par (vide pour effacer)",
                    field_width,
                );
            });

            let account = doc.current_account();
            let visible = doc.visible();
            let count = bp_core::edit::count_matches(account, &visible, &self.find);
            let examples: Vec<&bp_core::model::Operation> = if self.find.is_empty() {
                Vec::new()
            } else {
                visible
                    .iter()
                    .map(|&i| &account.operations[i])
                    .filter(|op| op.payee.contains(&self.find) || op.details.contains(&self.find))
                    .take(4)
                    .collect()
            };
            ui_kit::caption(ui, icon::EYE, "Aperçu");
            ui_kit::card(ui, width, |ui| {
                ui.horizontal(|ui| {
                    if self.find.is_empty() {
                        ui.label(RichText::new("Tapez le texte à rechercher.").color(pal.muted));
                    } else if count == 0 {
                        ui_kit::pill(ui, "Aucune occurrence", pal.muted);
                    } else {
                        let text = if count == 1 {
                            "1 occurrence".to_string()
                        } else {
                            format!("{count} occurrences")
                        };
                        ui_kit::pill(ui, &text, pal.accent);
                    }
                });
                if !examples.is_empty() {
                    ui.add_space(10.0);
                    let font = egui::FontId::new(13.5, crate::theme::medium());
                    let base = TextFormat::simple(font.clone(), pal.fg_dark);
                    let mut removed = TextFormat::simple(font.clone(), pal.negative);
                    removed.background = pal.negative.gamma_multiply(0.15);
                    removed.strikethrough = egui::Stroke::new(1.0, pal.negative);
                    let mut added = TextFormat::simple(font.clone(), pal.positive);
                    added.background = pal.positive.gamma_multiply(0.15);
                    for op in examples {
                        let text = if op.payee.contains(&self.find) { &op.payee } else { &op.details };
                        let after = text.replace(&self.find, &self.replacement);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(op.date.to_string()).color(pal.muted).size(12.5));
                            ui.add_space(6.0);
                            ui.label(highlighted(text, &self.find, base.clone(), removed.clone()));
                            ui.label(RichText::new(icon::ARROW_RIGHT).color(pal.muted));
                            ui.label(highlighted(&after, &self.replacement, base.clone(), added.clone()));
                        });
                    }
                }
            });
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

/// Ligne « case à cocher + champ » de la modification de groupe ; le champ
/// n'est actif que si la case est cochée, et la ligne est alors teintée.
fn toggle_row(
    ui: &mut egui::Ui,
    enabled: &mut bool,
    label: &str,
    glyph: &str,
    field: impl FnOnce(&mut egui::Ui, f32),
) {
    let pal = palette(ui);
    let full = ui.available_width();
    let frame = egui::Frame::new()
        .fill(if *enabled {
            pal.accent.gamma_multiply(0.07)
        } else {
            Color32::TRANSPARENT
        })
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(10, 6));
    frame.show(ui, |ui| {
        ui.set_width(full - 20.0);
        ui.horizontal(|ui| {
            ui.set_min_height(38.0);
            let label_width = 160.0;
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 38.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(label_width);
                    crate::theme::checkbox(ui, enabled, format!("{glyph}  {label}"));
                },
            );
            let field_width = ui.available_width();
            ui.add_enabled_ui(*enabled, |ui| field(ui, field_width));
        });
    });
    ui.add_space(4.0);
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
        let width = ui_kit::dialog_width(ctx, 0.5, 640.0, 780.0);
        crate::dialogs::modal(ctx, "tool_group").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            title(
                ui,
                icon::PENCIL_SIMPLE_LINE,
                "Modifier le groupe d'opérations",
            );
            ui.horizontal(|ui| {
                let text = if lines == 1 {
                    "1 ligne sélectionnée".to_string()
                } else {
                    format!("{lines} lignes sélectionnées")
                };
                ui_kit::pill(ui, &text, pal.accent);
                ui.label(
                    RichText::new("Cochez les champs à modifier ; les autres restent inchangés.")
                        .color(pal.fg_dark),
                );
            });
            ui.add_space(16.0);
            let document = &doc.document;
            let styles = &doc.category_styles;
            ui_kit::section(
                ui,
                width,
                icon::SLIDERS_HORIZONTAL,
                "Champs à modifier",
                |ui| {
                    toggle_row(
                        ui,
                        &mut self.date.0,
                        "Date",
                        icon::CALENDAR_BLANK,
                        |ui, w| {
                            crate::datepicker::date_field(
                                ui,
                                "group_date",
                                &mut self.date.1,
                                w.min(240.0),
                                crate::datepicker::Placement::Popup,
                            );
                        },
                    );
                    toggle_row(ui, &mut self.mode.0, "Mode", icon::CREDIT_CARD, |ui, w| {
                        ui_kit::mode_combo(ui, "group_mode", document, &mut self.mode.1, w);
                    });
                    toggle_row(ui, &mut self.payee.0, "Tiers", icon::USER, |ui, w| {
                        ui.add(crate::theme::field(&mut self.payee.1).desired_width(w));
                    });
                    toggle_row(
                        ui,
                        &mut self.details.0,
                        "Détail",
                        icon::NOTE_PENCIL,
                        |ui, w| {
                            ui.add(crate::theme::field(&mut self.details.1).desired_width(w));
                        },
                    );
                    toggle_row(
                        ui,
                        &mut self.category.0,
                        "Catégorie",
                        icon::TAG,
                        |ui, w| {
                            ui_kit::category_combo(
                                ui,
                                "group_category",
                                document,
                                styles,
                                &mut self.category.1,
                                w,
                            );
                        },
                    );
                    toggle_row(
                        ui,
                        &mut self.amount.0,
                        "Montant",
                        icon::CURRENCY_EUR,
                        |ui, w| {
                            ui.add(
                                crate::theme::field(&mut self.amount.1)
                                    .hint_text("0,00 — signe selon le mode")
                                    .desired_width(w.min(240.0)),
                            );
                        },
                    );
                    toggle_row(
                        ui,
                        &mut self.tack.0,
                        "Pointage",
                        icon::CHECK_CIRCLE,
                        |ui, w| {
                            ui_kit::segmented(
                                ui,
                                &mut self.tack.1,
                                &[
                                    (Tack::None, icon::CIRCLE_DASHED, "À pointer", pal.fg),
                                    (Tack::Pointed, icon::CHECK, "Pointée", pal.pointed),
                                    (Tack::Reconciled, icon::CHECKS, "Rapprochée", pal.reconciled),
                                ],
                                34.0,
                                Some(w),
                            );
                        },
                    );
                },
            );
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
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
    (';', "Point-virgule"),
    (',', "Virgule"),
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
        let width = ui_kit::dialog_width(ctx, 0.7, 800.0, 1100.0);
        crate::dialogs::modal(ctx, "tool_csv").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            title(ui, icon::FILE_CSV, "Exporter vers un fichier CSV");
            ui_kit::subtitle(
                ui,
                "Fichier lisible par Excel, LibreOffice ou Numbers. Choisissez les colonnes, leur ordre et le format.",
            );
            let gap = 18.0;
            let left = (width - gap) * 0.38;
            let right = width - gap - left;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.vertical(|ui| {
                    ui.set_width(left);
                    ui_kit::section(ui, left, icon::COLUMNS, "Colonnes, dans l'ordre", |ui| {
                        let mut swap = None;
                        let count = self.fields.len();
                        for (i, (field, on)) in self.fields.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.set_min_height(30.0);
                                crate::theme::checkbox(ui, on, field.label());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let down = egui::Button::new(RichText::new(icon::CARET_DOWN).color(pal.muted)).frame(false);
                                    if ui.add_enabled(i + 1 < count, down).on_hover_text("Descendre").clicked() {
                                        swap = Some((i, i + 1));
                                    }
                                    let up = egui::Button::new(RichText::new(icon::CARET_UP).color(pal.muted)).frame(false);
                                    if ui.add_enabled(i > 0, up).on_hover_text("Monter").clicked() {
                                        swap = Some((i - 1, i));
                                    }
                                });
                            });
                        }
                        if let Some((a, b)) = swap {
                            self.fields.swap(a, b);
                        }
                    });
                });
                ui.vertical(|ui| {
                    ui.set_width(right);
                    ui_kit::section(ui, right, icon::STACK, "Contenu", |ui| {
                        ui_kit::segmented(
                            ui,
                            &mut self.all_accounts,
                            &[
                                (false, icon::FUNNEL, "Lignes affichées", pal.accent),
                                (true, icon::BANK, "Tous les comptes", pal.accent),
                            ],
                            34.0,
                            None,
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(if self.all_accounts {
                                "Toutes les opérations de chaque compte, un fichier par compte."
                            } else {
                                "Les lignes affichées du compte courant, filtres appliqués."
                            })
                            .color(pal.muted),
                        );
                    });
                    ui_kit::section(ui, right, icon::GEAR_SIX, "Format", |ui| {
                        let label_width = 170.0;
                        ui_kit::row(ui, "Séparateur", label_width, |ui| {
                            let w = ui.available_width();
                            ui_kit::segmented(
                                ui,
                                &mut self.separator,
                                &[
                                    (0, "", SEPARATORS[0].1, pal.accent),
                                    (1, "", SEPARATORS[1].1, pal.accent),
                                    (2, "", SEPARATORS[2].1, pal.accent),
                                ],
                                32.0,
                                Some(w),
                            );
                        });
                        ui.add_space(6.0);
                        ui_kit::row(ui, "Symboles de pointage", label_width, |ui| {
                            for (symbol, hint) in self.tack_symbols.iter_mut().zip(["non", "pointée", "rapprochée"]) {
                                ui.add(crate::theme::field(symbol).desired_width(46.0).horizontal_align(egui::Align::Center))
                                    .on_hover_text(hint);
                            }
                            ui.label(RichText::new("non · pointée · rapprochée").color(pal.muted).size(12.5));
                        });
                        ui.add_space(10.0);
                        crate::theme::checkbox(ui, &mut self.header, "Inclure la ligne d'en-tête");
                        ui.add_space(6.0);
                        crate::theme::checkbox(ui, &mut self.cheque_as_mode, "Remplacer « Chq XXX » par « Chèque émis »");
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            crate::theme::checkbox(ui, &mut self.group, "Catégorie et sous-catégorie dans une colonne, séparées par");
                            ui.add_enabled(self.group, crate::theme::field(&mut self.group_separator).desired_width(50.0));
                        });
                    });
                });
            });

            // Aperçu : les premières lignes du fichier.
            let options = self.options();
            let account = doc.current_account();
            let visible = doc.visible();
            let sample: Vec<usize> = visible.iter().take(4).copied().collect();
            let text = if options.fields.is_empty() {
                String::new()
            } else {
                bp_core::export::csv(&doc.document, account, &sample, &options)
            };
            ui_kit::section(ui, width, icon::EYE, "Aperçu", |ui| {
                if text.is_empty() {
                    ui.label(RichText::new("Aucune colonne choisie.").color(pal.muted));
                } else {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        for line in text.lines().take(5) {
                            ui.label(
                                RichText::new(line.replace('\t', "   ⇥   "))
                                    .monospace()
                                    .size(12.5)
                                    .color(pal.fg_dark),
                            );
                        }
                    });
                }
            });
            let any = self.fields.iter().any(|(_, on)| *on);
            match buttons(ui, &format!("{}  Exporter…", icon::EXPORT), any) {
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
        let start_dir = crate::settings::shared::export_dir(
            doc.path.parent().unwrap_or(std::path::Path::new(".")),
        );
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
        .set_directory(crate::settings::shared::export_dir(
            doc.path.parent().unwrap_or(std::path::Path::new(".")),
        ))
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
    /// Fichier dont on liste les sauvegardes (ouvert, ou illisible).
    path: std::path::PathBuf,
    dir: std::path::PathBuf,
    keep: usize,
    confirm: Option<std::path::PathBuf>,
    error: Option<String>,
}

/// « il y a 5 min », « il y a 3 h », « il y a 2 jours ».
fn ago(time: std::time::SystemTime) -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(time)
        .map_or(0, |d| d.as_secs());
    match seconds {
        0..60 => "à l'instant".into(),
        60..3600 => format!("il y a {} min", seconds / 60),
        3600..86_400 => format!("il y a {} h", seconds / 3600),
        86_400..172_800 => "hier".into(),
        _ => format!("il y a {} jours", seconds / 86_400),
    }
}

impl BackupsForm {
    pub fn new(path: std::path::PathBuf, dir: std::path::PathBuf, keep: usize) -> Self {
        Self {
            path,
            dir,
            keep,
            confirm: None,
            error: None,
        }
    }

    /// `dirty` : le fichier ouvert a des modifications non enregistrées.
    pub fn show_standalone(&mut self, ctx: &egui::Context, dirty: bool) -> Outcome {
        let mut outcome = Outcome::Keep;
        let backups = bp_core::backup::list(&self.path, &self.dir);
        let width = ui_kit::dialog_width(ctx, 0.5, 640.0, 780.0);
        crate::dialogs::modal(ctx, "tool_backups").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            title(ui, icon::CLOCK_COUNTER_CLOCKWISE, "Sauvegardes");
            ui_kit::subtitle(
                ui,
                format!(
                    "Une copie du fichier est faite avant chaque enregistrement ; les {} plus récentes sont gardées.",
                    self.keep
                ),
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::FOLDER).color(pal.muted));
                ui.label(RichText::new(self.dir.display().to_string()).color(pal.muted).size(12.5));
            });
            ui.add_space(12.0);
            ui_kit::caption(ui, icon::LIST, &format!("{} sauvegarde(s)", backups.len()));
            ui_kit::card(ui, width, |ui| {
                if backups.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(icon::CLOCK_COUNTER_CLOCKWISE).size(32.0).color(pal.muted));
                        ui.add_space(6.0);
                        ui.label(RichText::new("Aucune sauvegarde pour ce fichier pour l'instant.").color(pal.muted));
                    });
                    ui.add_space(20.0);
                    return;
                }
                egui::ScrollArea::vertical().max_height(ui_kit::dialog_height(ui.ctx(), 0.45)).show(ui, |ui| {
                    for (k, backup) in backups.iter().enumerate() {
                        let when: chrono::DateTime<chrono::Local> = backup.modified.into();
                        let selected = self.confirm.as_ref() == Some(&backup.path);
                        let frame = egui::Frame::new()
                            .fill(if selected { pal.accent.gamma_multiply(0.08) } else { Color32::TRANSPARENT })
                            .corner_radius(10)
                            .inner_margin(egui::Margin::symmetric(10, 8));
                        frame.show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
                                ui.painter().rect_filled(rect, 9.0, pal.accent.gamma_multiply(0.14));
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    icon::FILE,
                                    egui::FontId::proportional(17.0),
                                    pal.accent,
                                );
                                ui.add_space(4.0);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(when.format("%d/%m/%Y à %H:%M").to_string())
                                                .family(crate::theme::semibold()),
                                        );
                                        if k == 0 {
                                            ui_kit::pill(ui, "la plus récente", pal.positive);
                                        }
                                    });
                                    ui.label(
                                        RichText::new(format!("{} · {} Ko", ago(backup.modified), backup.size.div_ceil(1024)))
                                            .color(pal.muted)
                                            .size(12.5),
                                    );
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if crate::dialogs::button(ui, format!("{}  Restaurer", icon::ARROW_COUNTER_CLOCKWISE)).clicked() {
                                        self.confirm = Some(backup.path.clone());
                                    }
                                });
                            });
                        });
                    }
                });
            });
            if let Some(error) = &self.error {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            if let Some(path) = self.confirm.clone() {
                ui.add_space(14.0);
                let unsaved = if dirty { " Les modifications non enregistrées seront perdues." } else { "" };
                egui::Frame::new()
                    .fill(pal.warning.gamma_multiply(0.10))
                    .stroke(egui::Stroke::new(1.0, pal.warning.gamma_multiply(0.4)))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width(width - 34.0);
                        ui.label(
                            RichText::new(format!(
                                "{}  Restaurer cette sauvegarde ? La version actuelle sera elle-même sauvegardée.{unsaved}",
                                icon::WARNING
                            ))
                            .color(pal.warning),
                        );
                        crate::dialogs::footer(ui, |ui| {
                            if crate::dialogs::primary(ui, "Restaurer", true).clicked() {
                                match bp_core::backup::restore(&path, &self.path, &self.dir, self.keep) {
                                    Ok(()) => outcome = Outcome::Reopen(self.path.clone(), "Sauvegarde restaurée".into()),
                                    Err(e) => self.error = Some(format!("Restauration impossible : {e}")),
                                }
                            }
                            if crate::dialogs::button(ui, "Annuler").clicked() {
                                self.confirm = None;
                            }
                        });
                    });
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, "Fermer", true).clicked()
                    || (self.confirm.is_none() && ui.input(|i| i.key_pressed(Key::Escape)))
                {
                    outcome = Outcome::Close;
                }
            });
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
        let currency = doc.current_account().currency.clone();
        let width = ui_kit::dialog_width(ctx, 0.45, 600.0, 720.0);
        crate::dialogs::modal(ctx, "tool_extra").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            title(ui, icon::LIST_PLUS, "Champs supplémentaires");
            // L'opération concernée.
            ui.horizontal(|ui| {
                ui.label(RichText::new(op.date.to_string()).color(pal.muted));
                ui.label(RichText::new(&op.payee).family(crate::theme::semibold()));
                let color = if op.amount.is_negative() {
                    pal.negative
                } else {
                    pal.positive
                };
                ui_kit::pill(ui, &crate::format::money(op.amount, &currency), color);
            });
            ui.add_space(16.0);
            let label_width = 170.0;
            let mut remove = None;
            ui_kit::section(ui, width, icon::LIST_BULLETS, "Champs", |ui| {
                if self.entries.is_empty() {
                    ui.label(
                        RichText::new("Aucun champ supplémentaire pour cette opération.")
                            .color(pal.muted),
                    );
                }
                for (i, (key, value)) in self.entries.iter_mut().enumerate() {
                    let internal = is_internal(key);
                    ui.horizontal(|ui| {
                        ui.set_min_height(38.0);
                        let shown = if internal {
                            format!("{}  {key}", icon::LOCK_SIMPLE)
                        } else {
                            key.clone()
                        };
                        ui.add_sized(
                            [label_width, 36.0],
                            egui::Label::new(RichText::new(shown).color(if internal {
                                pal.muted
                            } else {
                                pal.fg_dark
                            }))
                            .truncate(),
                        );
                        let w = ui.available_width() - if internal { 0.0 } else { 44.0 };
                        ui.add_enabled(!internal, crate::theme::field(value).desired_width(w));
                        if !internal
                            && ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(icon::TRASH).color(pal.negative),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("Supprimer ce champ")
                                .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                }
            });
            if let Some(i) = remove {
                self.entries.remove(i);
            }
            ui_kit::section(ui, width, icon::PLUS_CIRCLE, "Ajouter un champ", |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        crate::theme::field(&mut self.new_key)
                            .hint_text("Nom")
                            .desired_width(label_width - 8.0),
                    );
                    let w = ui.available_width() - 110.0;
                    ui.add(
                        crate::theme::field(&mut self.new_value)
                            .hint_text("Valeur")
                            .desired_width(w),
                    );
                    if crate::dialogs::button(ui, format!("{}  Ajouter", icon::PLUS)).clicked() {
                        self.add();
                    }
                });
            });
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
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
        crate::dialogs::modal(ctx, "tool_notes").show(ctx, |ui| {
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
                                crate::theme::checkbox(
                                    ui,
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
            let label = if self.on_open {
                "Fermer"
            } else {
                "Enregistrer et fermer"
            };
            let close =
                crate::dialogs::footer(ui, |ui| crate::dialogs::primary(ui, label, true).clicked());
            if close || ui.input(|i| i.key_pressed(Key::Escape)) {
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

// ─────────────────────────────── Calculatrice ───────────────────────────────

#[derive(Default)]
pub struct CalcForm {
    expression: String,
    /// Dernier résultat validé (Entrée ou =).
    result: Option<String>,
    focus: bool,
}

/// Touches du pavé, dans la disposition de l'original (8 colonnes).
const KEYS: [&str; 40] = [
    "AC", "⌫", "√", "DMS", "EE", "cos", "acos", "cosh", //
    "7", "8", "9", "/", "%", "sin", "asin", "sinh", //
    "4", "5", "6", "*", "^", "tan", "atan", "tanh", //
    "1", "2", "3", "-", "(", "deg", "rad", "π", //
    ",", "0", "=", "+", ")", "ln", "log", "e",
];

impl CalcForm {
    pub fn new() -> Self {
        Self {
            focus: true,
            ..Default::default()
        }
    }

    fn press(&mut self, key: &str) {
        match key {
            "AC" => {
                self.expression.clear();
                self.result = None;
            }
            "⌫" => {
                self.expression.pop();
            }
            "=" => self.validate(),
            "DMS" => {
                // Ajoute l'unité suivante : °, puis ', puis ".
                let next = if self.expression.contains('\'') {
                    "\""
                } else if self.expression.contains('°') {
                    "'"
                } else {
                    "°"
                };
                self.expression.push_str(next);
            }
            "EE" => self.expression.push('E'),
            "√" => self.expression.push_str("sqrt("),
            "π" => self.expression.push_str("pi"),
            k if k.len() > 1 && k.chars().all(|c| c.is_ascii_alphabetic()) && k != "pi" => {
                self.expression.push_str(k);
                self.expression.push('(');
            }
            k => self.expression.push_str(k),
        }
        self.focus = true;
    }

    fn validate(&mut self) {
        if let Ok((value, dms)) = crate::calculator::evaluate(&self.expression) {
            let text = crate::calculator::format(value, dms);
            self.result = Some(format!("{} =", self.expression));
            self.expression = text;
        }
    }

    pub fn show_standalone(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Keep;
        crate::dialogs::modal(ctx, "tool_calc").show(ctx, |ui| {
            ui.set_width(600.0);
            title(ui, icon::CALCULATOR, "Calculatrice");
            let pal = palette(ui);
            if let Some(previous) = &self.result {
                ui.label(RichText::new(previous).small().color(pal.muted));
            }
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.expression)
                    .font(egui::FontId::new(22.0, crate::theme::medium()))
                    .margin(egui::Margin::symmetric(12, 10))
                    .desired_width(f32::INFINITY),
            );
            if std::mem::take(&mut self.focus) {
                field.request_focus();
                // Curseur en fin de saisie après un clic sur une touche du pavé.
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), field.id) {
                    let end = egui::text::CCursor::new(self.expression.chars().count());
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(end)));
                    state.store(ui.ctx(), field.id);
                }
            }
            if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.validate();
                self.focus = true;
            }
            let preview = match crate::calculator::evaluate(&self.expression) {
                Ok((value, dms)) if !self.expression.is_empty() => {
                    RichText::new(format!("= {}", crate::calculator::format(value, dms)))
                        .color(pal.accent)
                }
                Err(e) if !self.expression.is_empty() => RichText::new(e).color(pal.muted),
                _ => RichText::new(" "),
            };
            ui.label(preview);
            ui.add_space(6.0);
            let mut pressed = None;
            // Pavé sur toute la largeur de la fenêtre.
            let key_width = (ui.available_width() - 7.0 * 8.0) / 8.0;
            egui::Grid::new("calc_keys")
                .striped(false)
                .num_columns(8)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    for (i, key) in KEYS.iter().enumerate() {
                        let color = match *key {
                            "AC" | "⌫" => pal.negative,
                            "+" | "-" | "*" | "/" | "^" | "%" | "(" | ")" | "√" | "DMS" | "EE" => {
                                pal.accent
                            }
                            k if k.chars().all(|c| c.is_ascii_digit()) || k == "," => pal.fg,
                            _ => pal.teal,
                        };
                        let button = if *key == "=" {
                            egui::Button::new(
                                RichText::new(*key)
                                    .size(17.0)
                                    .color(pal.on_accent)
                                    .family(crate::theme::semibold()),
                            )
                            .fill(pal.accent)
                        } else {
                            egui::Button::new(
                                RichText::new(*key)
                                    .size(15.0)
                                    .color(color)
                                    .family(crate::theme::medium()),
                            )
                        };
                        if ui.add_sized([key_width, 44.0], button).clicked() {
                            pressed = Some(*key);
                        }
                        if i % 8 == 7 {
                            ui.end_row();
                        }
                    }
                });
            if let Some(key) = pressed {
                self.press(key);
            }
            let close = crate::dialogs::footer(ui, |ui| {
                crate::dialogs::primary(ui, "Fermer", true).clicked()
            });
            if close || ui.input(|i| i.key_pressed(Key::Escape)) {
                outcome = Outcome::Close;
            }
        });
        outcome
    }
}
