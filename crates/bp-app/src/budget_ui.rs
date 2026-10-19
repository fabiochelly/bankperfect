//! Budgets : budget mensuel par catégorie (Ctrl+B) et tableaux de budget
//! annuel. Les montants se modifient directement dans les tableaux.

use bp_core::budget::{
    AnnualBudgets, BudgetKind, BudgetOptions, BudgetPeriod, BudgetReport, BudgetTotal, LineInput,
    MonthCounting, MonthlyBudget, Spread, ValueKind, parse_amount_input,
};
use bp_core::i18n::{self, Lang};
use bp_core::model::{Amount, Date};
use bp_core::{tr, trc, trn};
use chrono::Datelike;
use egui::{Color32, Key, RichText, vec2};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::spreadsheet::Cell;
use crate::theme::{Palette, palette};
use crate::tools::Outcome;
use crate::ui_kit;

/// Montant saisi dans un tableau de budget ou l'échéancier. En anglais, une
/// virgule suivie de trois chiffres sépare les milliers (« 1,234.56 »,
/// « 1,500 ») ; ailleurs elle reste décimale, comme en français.
pub(crate) fn amount_input(text: &str) -> String {
    i18n::strip_thousands(text)
}

/// [`parse_amount_input`] avec les milliers à l'anglaise.
fn parse_amount(text: &str) -> Option<Amount> {
    parse_amount_input(&amount_input(text))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BudgetTab {
    Monthly,
    Annual,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PeriodKind {
    Month,
    Year,
    All,
    Filter,
}

/// Ligne du budget annuel en cours de modification (`index` : `None` pour
/// une nouvelle ligne).
struct LineEditor {
    index: Option<usize>,
    input: LineInput,
    search: String,
}

pub struct BudgetForm {
    pub tab: BudgetTab,
    // Budget mensuel.
    budget: MonthlyBudget,
    options: BudgetOptions,
    period_kind: PeriodKind,
    filter_names: Vec<String>,
    filter_index: usize,
    all_accounts: bool,
    editing: Option<(Option<u32>, String)>,
    // Budget annuel.
    tables: AnnualBudgets,
    selected: Option<usize>,
    name_draft: String,
    year_draft: i32,
    view: ValueKind,
    spread: Spread,
    cell_edit: Option<(usize, usize, String)>,
    line_editor: Option<LineEditor>,
    new_table: Option<(String, i32)>,
    confirm_delete: bool,
    error: Option<String>,
    message: Option<String>,
}

impl BudgetForm {
    pub fn new(doc: &OpenDoc, tab: BudgetTab) -> Self {
        let parse = &bp_core::legacy_pickle::parse_py;
        let today = Date::today();
        let tables = AnnualBudgets::load(&doc.document, parse).unwrap_or_default();
        let year = today.to_naive().year();
        let selected = tables
            .tables
            .iter()
            .position(|t| t.year == year)
            .or((!tables.tables.is_empty()).then_some(0));
        let mut form = Self {
            tab,
            budget: MonthlyBudget::load(&doc.document, parse).unwrap_or_default(),
            options: BudgetOptions::new(today),
            period_kind: PeriodKind::Month,
            filter_names: crate::filters_ui::load(doc)
                .names()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            filter_index: 0,
            all_accounts: true,
            editing: None,
            tables,
            selected: None,
            name_draft: String::new(),
            year_draft: year,
            view: ValueKind::Budget,
            spread: Spread::Month,
            cell_edit: None,
            line_editor: None,
            new_table: None,
            confirm_delete: false,
            error: None,
            message: None,
        };
        form.select_table(selected);
        form
    }

    fn select_table(&mut self, index: Option<usize>) {
        self.selected = index;
        if let Some(t) = index.and_then(|i| self.tables.tables.get(i)) {
            self.name_draft = t.name.clone();
            self.year_draft = t.year;
        }
        self.cell_edit = None;
        self.line_editor = None;
        self.confirm_delete = false;
        self.error = None;
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.85, 900.0, 1700.0);
        let height = ui_kit::dialog_height(ctx, 0.88).max(580.0);
        crate::dialogs::modal(ctx, "budgets").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(260.0, 36.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| crate::dialogs::header(ui, icon::PIGGY_BANK, pal.accent, tr!("Budgets")),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui_kit::segmented(
                        ui,
                        &mut self.tab,
                        &[
                            (
                                BudgetTab::Monthly,
                                icon::CALENDAR_BLANK,
                                tr!("Budget mensuel"),
                                pal.accent,
                            ),
                            (
                                BudgetTab::Annual,
                                icon::CALENDAR,
                                tr!("Budgets annuels"),
                                pal.accent,
                            ),
                        ],
                        38.0,
                        Some(420.0),
                    );
                });
            });
            ui.add_space(14.0);
            let body = height - 36.0 - 14.0 - 100.0;
            let mut export = None;
            ui.allocate_ui_with_layout(
                vec2(width, body),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_size(vec2(width, body));
                    export = match self.tab {
                        BudgetTab::Monthly => self.monthly(ui, doc, width, &pal),
                        BudgetTab::Annual => self.annual(ui, doc, width, &pal),
                    };
                },
            );
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            } else if let Some(message) = &self.message {
                ui.label(RichText::new(format!("{}  {message}", icon::INFO)).color(pal.accent));
            }
            crate::dialogs::footer(ui, |ui| {
                let escape = ui.input(|i| i.key_pressed(Key::Escape))
                    && self.editing.is_none()
                    && self.cell_edit.is_none()
                    && self.line_editor.is_none();
                if crate::dialogs::primary(ui, tr!("Fermer"), true).clicked() || escape {
                    outcome = Outcome::Close;
                }
                if let Some((name, rows)) = export
                    && crate::dialogs::button(
                        ui,
                        format!(
                            "{}  {}",
                            icon::MICROSOFT_EXCEL_LOGO,
                            tr!("Exporter vers un tableur…")
                        ),
                    )
                    .clicked()
                {
                    let dir = crate::settings::shared::export_dir(
                        doc.path.parent().unwrap_or(std::path::Path::new(".")),
                    );
                    crate::spreadsheet::export(ui.ctx(), &dir, &name, &name, rows);
                }
            });
        });
        outcome
    }

    // ─────────────────────────── Budget mensuel ───────────────────────────

    fn save_monthly(&mut self, doc: &mut OpenDoc) {
        match self.budget.save(&mut doc.document) {
            Ok(()) => doc.dirty = true,
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn period_bar(&mut self, ui: &mut egui::Ui, doc: &OpenDoc, pal: &Palette) {
        let today = Date::today();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(14.0, 10.0);
            let before = self.period_kind;
            let mut kinds = vec![
                (PeriodKind::Month, "", tr!("Un mois"), pal.accent),
                (PeriodKind::Year, "", tr!("Une année"), pal.accent),
                (PeriodKind::All, "", tr!("Tout"), pal.accent),
            ];
            if !self.filter_names.is_empty() {
                kinds.push((PeriodKind::Filter, "", tr!("Filtre"), pal.accent));
            }
            ui_kit::segmented(
                ui,
                &mut self.period_kind,
                &kinds,
                32.0,
                Some(95.0 * kinds.len() as f32),
            );
            if self.period_kind != before {
                self.options.period = match self.period_kind {
                    PeriodKind::Month => BudgetPeriod::current_month(today),
                    PeriodKind::Year => BudgetPeriod::current_year(today),
                    PeriodKind::All => BudgetPeriod::All,
                    PeriodKind::Filter => self.filter_period(doc),
                };
            }
            match &mut self.options.period {
                BudgetPeriod::Month { year, month } => {
                    if ui
                        .button(icon::CARET_LEFT)
                        .on_hover_text(tr!("Mois précédent"))
                        .clicked()
                    {
                        if *month == 1 {
                            *month = 12;
                            *year -= 1;
                        } else {
                            *month -= 1;
                        }
                    }
                    let label = format!(
                        "{} {}",
                        i18n::month_name((*month as usize).saturating_sub(1)),
                        year
                    );
                    ui.add_sized(
                        vec2(150.0, 28.0),
                        egui::Label::new(
                            RichText::new(label)
                                .family(crate::theme::medium())
                                .size(15.0),
                        ),
                    );
                    if ui
                        .button(icon::CARET_RIGHT)
                        .on_hover_text(tr!("Mois suivant"))
                        .clicked()
                    {
                        if *month == 12 {
                            *month = 1;
                            *year += 1;
                        } else {
                            *month += 1;
                        }
                    }
                }
                BudgetPeriod::Year(year) => {
                    let years = bp_core::reports::available_years(
                        &doc.document,
                        &bp_core::reports::AccountScope::All,
                    );
                    egui::ComboBox::from_id_salt("budget_year")
                        .selected_text(year.to_string())
                        .width(110.0)
                        .show_ui(ui, |ui| {
                            let current = today.to_naive().year();
                            let mut list = years.clone();
                            if !list.contains(&current) {
                                list.push(current);
                            }
                            for y in list.iter().rev() {
                                ui.selectable_value(year, *y, y.to_string());
                            }
                        });
                }
                BudgetPeriod::Filter(_) => {
                    let before = self.filter_index;
                    egui::ComboBox::from_id_salt("budget_filter")
                        .selected_text(
                            self.filter_names
                                .get(self.filter_index)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for (i, name) in self.filter_names.iter().enumerate() {
                                ui.selectable_value(&mut self.filter_index, i, name);
                            }
                        });
                    if before != self.filter_index {
                        self.options.period = self.filter_period(doc);
                    }
                }
                BudgetPeriod::All => {}
            }
            if !matches!(self.options.period, BudgetPeriod::Month { .. }) {
                ui_kit::segmented(
                    ui,
                    &mut self.options.counting,
                    &[
                        (
                            MonthCounting::MonthsWithOperations,
                            "",
                            tr!("Mois avec opérations"),
                            pal.accent,
                        ),
                        (MonthCounting::Days, "", tr!("Jours ÷ 30,4"), pal.accent),
                    ],
                    32.0,
                    Some(330.0),
                );
            }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            if ui_kit::segmented(
                ui,
                &mut self.all_accounts,
                &[
                    (true, "", tr!("Tous les comptes"), pal.accent),
                    (false, "", tr!("Compte courant"), pal.accent),
                ],
                30.0,
                Some(290.0),
            ) {
                self.options.accounts = (!self.all_accounts).then(|| vec![doc.account]);
            }
            crate::theme::checkbox(
                ui,
                &mut self.options.show_unbudgeted,
                tr!("Lignes sans budget"),
            );
            crate::theme::checkbox(ui, &mut self.options.show_zero, tr!("Lignes à zéro"));
            crate::theme::checkbox(
                ui,
                &mut self.options.show_subcategories,
                tr!("Sous-catégories"),
            );
        });
    }

    fn filter_period(&self, doc: &OpenDoc) -> BudgetPeriod {
        let filters = crate::filters_ui::load(doc);
        self.filter_names
            .get(self.filter_index)
            .and_then(|n| filters.get(n))
            .map_or(BudgetPeriod::All, |f| BudgetPeriod::Filter(f.clone()))
    }

    fn monthly(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut OpenDoc,
        width: f32,
        pal: &Palette,
    ) -> Option<(String, Vec<Vec<Cell>>)> {
        self.period_bar(ui, doc, pal);
        let report = self
            .budget
            .report(&doc.document, &self.options, Date::today());
        ui.add_space(12.0);
        // Synthèse.
        let gap = 16.0;
        let tile = (width - 2.0 * gap) / 3.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            summary_tile(
                ui,
                tile,
                tr!("Dépenses"),
                icon::ARROW_DOWN_RIGHT,
                &report.expenses,
                Some(BudgetKind::Expense),
                pal,
            );
            summary_tile(
                ui,
                tile,
                tr!("Recettes"),
                icon::ARROW_UP_RIGHT,
                &report.income,
                Some(BudgetKind::Income),
                pal,
            );
            summary_tile(
                ui,
                tile,
                tr!("Solde"),
                icon::SCALES,
                &report.total,
                None,
                pal,
            );
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new(&report.description)
                .size(12.5)
                .color(pal.muted),
        );
        ui.add_space(8.0);
        let commit = self.monthly_table(ui, &report, width, pal);
        if let Some((category, text, kind_hint)) = commit {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                self.budget.remove(category);
                self.save_monthly(doc);
                self.error = None;
            } else {
                match parse_amount(trimmed) {
                    Some(amount) => {
                        let kind = if trimmed.starts_with('+') {
                            BudgetKind::Income
                        } else if trimmed.starts_with('-') {
                            BudgetKind::Expense
                        } else {
                            kind_hint
                        };
                        self.budget.set(category, kind, Some(amount));
                        self.save_monthly(doc);
                        self.error = None;
                    }
                    None => self.error = Some(tr!("Montant incorrect : « {} »", trimmed)),
                }
            }
        }
        Some((tr!("Budget mensuel").into(), monthly_rows(&report)))
    }

    /// Tableau du budget ; renvoie une saisie validée `(catégorie, texte,
    /// nature par défaut)`.
    fn monthly_table(
        &mut self,
        ui: &mut egui::Ui,
        report: &BudgetReport,
        width: f32,
        pal: &Palette,
    ) -> Option<(Option<u32>, String, BudgetKind)> {
        const ROW: f32 = 36.0;
        let amount_w = 130.0;
        let pct_w = 70.0;
        let label_w = (width * 0.26).clamp(200.0, 320.0);
        let bar_w = (width - 16.0 - label_w - 3.0 * amount_w - pct_w - 24.0).max(80.0);
        let mut commit = None;
        let mut start_edit = None;
        let header = |ui: &mut egui::Ui, w: f32, align: egui::Align, text: &str| {
            ui_kit::cell(
                ui,
                w,
                align,
                RichText::new(text)
                    .family(crate::theme::semibold())
                    .color(pal.fg_dark),
            );
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            header(ui, label_w, egui::Align::Min, tr!("Catégorie"));
            header(ui, amount_w, egui::Align::Max, tr!("Budget"));
            header(ui, amount_w, egui::Align::Max, tr!("Réel"));
            header(ui, amount_w, egui::Align::Max, tr!("Écart"));
            ui.add_space(24.0);
            header(ui, bar_w, egui::Align::Min, tr!("Consommation"));
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt("budget_rows").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if report.rows.is_empty() {
                ui.add_space(30.0);
                ui.label(RichText::new(tr!("Aucune ligne : ajoutez des catégories ou affichez les lignes sans budget.")).color(pal.muted));
            }
            for (k, row) in report.rows.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(vec2(width - 16.0, ROW), egui::Sense::hover());
                if k % 2 == 1 {
                    ui.painter().rect_filled(rect, 6.0, pal.stripe.gamma_multiply(0.7));
                } else if response.hovered() {
                    ui.painter().rect_filled(rect, 6.0, pal.bg_highlight.gamma_multiply(0.35));
                }
                let mut child = ui.new_child(
                    egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                let ui = &mut child;
                ui.spacing_mut().item_spacing.x = 0.0;
                let label = if row.is_child {
                    RichText::new(format!("      {}", row.label)).color(pal.fg_dark)
                } else {
                    RichText::new(&row.label).family(crate::theme::medium())
                };
                ui_kit::cell(ui, label_w, egui::Align::Min, label);
                // Budget : modifiable d'un clic.
                let editing = self.editing.as_ref().is_some_and(|(c, _)| *c == row.category);
                if editing {
                    let (_, text) = self.editing.as_mut().unwrap();
                    let edit = ui.add_sized(
                        vec2(amount_w - 8.0, 28.0),
                        egui::TextEdit::singleline(text).horizontal_align(egui::Align::Max).hint_text(tr!("-150 ou +2000")),
                    );
                    ui.add_space(8.0);
                    if !edit.has_focus() && !edit.lost_focus() {
                        edit.request_focus();
                    }
                    if ui.input(|i| i.key_pressed(Key::Escape)) {
                        self.editing = None;
                    } else if edit.lost_focus() {
                        let hint = row.kind.unwrap_or(if row.real.0 > 0 { BudgetKind::Income } else { BudgetKind::Expense });
                        commit = Some((row.category, text.clone(), hint));
                        self.editing = None;
                    }
                } else {
                    let text = match row.budget {
                        Some(b) => RichText::new(money(b, "€")).color(amount_color(b, pal)),
                        None => RichText::new("—").color(pal.muted.gamma_multiply(0.6)),
                    };
                    let response = ui
                        .allocate_ui_with_layout(vec2(amount_w, ROW), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.set_min_size(vec2(amount_w, ROW));
                            ui.add(egui::Label::new(text).truncate());
                        })
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::Text)
                        .on_hover_text(tr!("Cliquer pour modifier le budget (vide = aucun)"));
                    if response.hovered() {
                        ui.painter().rect_stroke(
                            response.rect.shrink2(vec2(0.0, 4.0)),
                            6.0,
                            egui::Stroke::new(1.0, pal.accent.gamma_multiply(0.5)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if response.clicked() {
                        start_edit = Some((
                            row.category,
                            row.budget.map(|b| crate::format::money(b, "")).unwrap_or_default(),
                        ));
                    }
                }
                ui_kit::cell(ui, amount_w, egui::Align::Max, RichText::new(money(row.real, "€")).color(amount_color(row.real, pal)));
                match row.difference {
                    Some(d) => ui_kit::cell(ui, amount_w, egui::Align::Max, RichText::new(signed(d)).color(difference_color(d, row.kind, pal))),
                    None => ui_kit::cell(ui, amount_w, egui::Align::Max, RichText::new("")),
                }
                ui.add_space(24.0);
                if let (Some(budget), Some(kind)) = (row.budget, row.kind) {
                    let (ratio, color) = consumption(row.real, budget, kind, pal);
                    progress(ui, bar_w - 12.0, ratio, color, pal);
                    ui.add_space(12.0);
                    ui_kit::cell(ui, pct_w, egui::Align::Max, RichText::new(tr!("{} %", format!("{:.0}", ratio * 100.0))).color(pal.fg_dark));
                }
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new(tr!("Cliquez sur un budget pour le modifier. Les montants négatifs sont des dépenses ; préfixez par + une recette. Les calculs simples sont acceptés (1200/12)."))
                    .size(12.5)
                    .color(pal.muted),
            );
        });
        if let Some(edit) = start_edit {
            self.editing = Some(edit);
        }
        commit
    }

    // ─────────────────────────── Budgets annuels ───────────────────────────

    fn save_annual(&mut self, doc: &mut OpenDoc) {
        match self.tables.save(&mut doc.document) {
            Ok(()) => doc.dirty = true,
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn annual(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut OpenDoc,
        width: f32,
        pal: &Palette,
    ) -> Option<(String, Vec<Vec<Cell>>)> {
        let list_w = 260.0;
        let gap = 22.0;
        let detail_w = width - list_w - gap;
        let mut export = None;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.vertical(|ui| {
                ui.set_width(list_w);
                self.tables_list(ui, doc, list_w, pal);
            });
            ui.vertical(|ui| {
                ui.set_width(detail_w);
                ui.spacing_mut().item_spacing.x = 8.0;
                export = self.table_detail(ui, doc, detail_w, pal);
            });
        });
        export
    }

    fn tables_list(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32, pal: &Palette) {
        let labels = self.tables.labels();
        let mut clicked = None;
        ui_kit::card(ui, width, |ui| {
            if labels.is_empty() {
                ui.label(RichText::new(tr!("Aucun tableau.")).color(pal.muted));
            }
            for (i, label) in labels.iter().enumerate() {
                if ui
                    .selectable_label(
                        self.selected == Some(i),
                        format!("{}  {label}", icon::TABLE),
                    )
                    .clicked()
                {
                    clicked = Some(i);
                }
            }
            ui.add_space(8.0);
            if let Some((name, year)) = &mut self.new_table {
                let mut create = false;
                let mut cancel = false;
                ui.add(
                    crate::theme::field(name)
                        .hint_text(tr!("Nom du tableau"))
                        .desired_width(width - 40.0),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(year).range(1900..=9999));
                    create = ui
                        .button(format!("{}  {}", icon::CHECK, tr!("Créer")))
                        .clicked();
                    cancel = ui.button(tr!("Annuler")).clicked();
                });
                let (name, year) = (name.clone(), *year);
                if create {
                    match self.tables.create(&name, year) {
                        Ok(i) => {
                            self.new_table = None;
                            self.save_annual(doc);
                            clicked = Some(i);
                        }
                        Err(e) => self.error = Some(capitalize(&e.to_string())),
                    }
                } else if cancel {
                    self.new_table = None;
                }
            } else {
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("{}  {}", icon::PLUS, tr!("Nouveau")))
                        .clicked()
                    {
                        self.new_table = Some((String::new(), Date::today().to_naive().year()));
                    }
                    if let Some(i) = self.selected
                        && ui
                            .button(format!("{}  {}", icon::COPY, tr!("Dupliquer")))
                            .clicked()
                        && let Ok(j) = self.tables.duplicate(i)
                    {
                        self.save_annual(doc);
                        clicked = Some(j);
                    }
                    if self.selected.is_some()
                        && ui
                            .add(egui::Button::new(
                                RichText::new(icon::TRASH).color(pal.negative),
                            ))
                            .on_hover_text(tr!("Supprimer"))
                            .clicked()
                    {
                        self.confirm_delete = true;
                    }
                });
            }
        });
        if self.confirm_delete
            && let Some(i) = self.selected
        {
            ui.add_space(10.0);
            let name = self.tables.tables[i].name.clone();
            egui::Frame::new()
                .fill(pal.negative.gamma_multiply(0.10))
                .stroke(egui::Stroke::new(1.0, pal.negative.gamma_multiply(0.4)))
                .corner_radius(12)
                .inner_margin(14)
                .show(ui, |ui| {
                    ui.set_width(width - 30.0);
                    ui.label(tr!("Supprimer le tableau « {} » ?", name));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(RichText::new(tr!("Supprimer")).color(pal.negative))
                            .clicked()
                        {
                            let _ = self.tables.delete(i);
                            self.save_annual(doc);
                            clicked = Some(usize::MAX);
                        }
                        if ui.button(tr!("Annuler")).clicked() {
                            self.confirm_delete = false;
                        }
                    });
                });
        }
        match clicked {
            Some(usize::MAX) => self.select_table((!self.tables.tables.is_empty()).then_some(0)),
            Some(i) => self.select_table(Some(i)),
            None => {}
        }
    }

    fn table_detail(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut OpenDoc,
        width: f32,
        pal: &Palette,
    ) -> Option<(String, Vec<Vec<Cell>>)> {
        let Some(index) = self.selected.filter(|&i| i < self.tables.tables.len()) else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(icon::TABLE).size(40.0).color(pal.muted));
                ui.label(RichText::new(tr!("Créez un tableau pour prévoir mois par mois vos dépenses et recettes de l'année.")).color(pal.muted));
            });
            return None;
        };
        // Nom, année et comptes.
        ui.horizontal(|ui| {
            let name = ui.add(crate::theme::field(&mut self.name_draft).desired_width(260.0));
            let year = ui.add(egui::DragValue::new(&mut self.year_draft).range(1900..=9999));
            let table = &self.tables.tables[index];
            let changed = self.name_draft.trim() != table.name || self.year_draft != table.year;
            if changed && (name.lost_focus() || year.drag_stopped() || year.lost_focus()) {
                match self.tables.rename(index, &self.name_draft, self.year_draft) {
                    Ok(i) => {
                        self.selected = Some(i);
                        self.save_annual(doc);
                        self.error = None;
                    }
                    Err(e) => self.error = Some(capitalize(&e.to_string())),
                }
            }
            ui.add_space(12.0);
            let index = self.selected.unwrap_or(index);
            let chosen = self.tables.tables[index].account_indices(&doc.document);
            let all = self.tables.tables[index].accounts.is_none();
            let label = if all {
                tr!("Tous les comptes").to_string()
            } else {
                trn!(chosen.len(), "{} compte", "{} comptes")
            };
            let mut new_selection: Option<Option<Vec<usize>>> = None;
            ui.menu_button(format!("{}  {label}", icon::BANK), |ui| {
                let mut every = all;
                if crate::theme::checkbox(ui, &mut every, tr!("Tous les comptes")).changed() {
                    new_selection = Some(if every { None } else { Some(chosen.clone()) });
                }
                ui.separator();
                for &a in &doc.document.account_order {
                    let mut on = all || chosen.contains(&a);
                    if crate::theme::checkbox(ui, &mut on, doc.document.accounts[a].name.as_str())
                        .changed()
                    {
                        let mut list: Vec<usize> = if all {
                            doc.document.account_order.clone()
                        } else {
                            chosen.clone()
                        };
                        if on {
                            list.push(a);
                        } else {
                            list.retain(|&x| x != a);
                        }
                        new_selection = Some(Some(list));
                    }
                }
            });
            if let Some(selection) = new_selection {
                self.tables.tables[index].set_accounts(&doc.document, selection.as_deref());
                self.save_annual(doc);
            }
        });
        let index = self.selected.unwrap_or(index);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui_kit::segmented(
                ui,
                &mut self.view,
                &[
                    (ValueKind::Budget, "", tr!("Budget"), pal.accent),
                    (ValueKind::Real, "", tr!("Réel"), pal.accent),
                    (ValueKind::Difference, "", tr!("Écart"), pal.accent),
                ],
                32.0,
                Some(300.0),
            );
            ui.add_space(12.0);
            if self.view != ValueKind::Difference {
                ui.label(RichText::new(tr!("Saisie :")).color(pal.fg_dark));
                egui::ComboBox::from_id_salt("annual_spread")
                    .selected_text(spread_label(self.spread))
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for s in [
                            Spread::Month,
                            Spread::AllMonths,
                            Spread::EveryTwoMonths,
                            Spread::EveryThreeMonths,
                        ] {
                            ui.selectable_value(&mut self.spread, s, spread_label(s));
                        }
                    });
            }
            ui.add_space(12.0);
            if ui
                .button(format!("{}  {}", icon::PLUS, trc!("budget", "Ligne")))
                .clicked()
            {
                self.line_editor = Some(LineEditor {
                    index: None,
                    input: LineInput {
                        title: String::new(),
                        auto_real: true,
                        categories: Vec::new(),
                    },
                    search: String::new(),
                });
            }
        });
        ui.add_space(10.0);
        let report = self.tables.tables[index].report(&doc.document);
        let editor_h = if self.line_editor.is_some() {
            300.0
        } else {
            0.0
        };
        let grid_h = (ui.available_height() - editor_h - 8.0).max(160.0);
        let (commit, edit_line) = self.annual_grid(ui, &report, index, width, grid_h, pal);
        if let Some((line, month, text)) = commit {
            let value = if text.trim().is_empty() {
                Some(Amount::ZERO)
            } else {
                parse_amount(&text)
            };
            match value {
                Some(v) => {
                    let kind = self.view;
                    match self.tables.tables[index].set_value(line, kind, month, v, self.spread) {
                        Ok(()) => {
                            self.save_annual(doc);
                            self.error = None;
                        }
                        Err(e) => self.error = Some(capitalize(&e.to_string())),
                    }
                }
                None => self.error = Some(tr!("Montant incorrect : « {} »", text.trim())),
            }
        }
        if let Some(line) = edit_line {
            let l = &self.tables.tables[index].lines[line];
            self.line_editor = Some(LineEditor {
                index: Some(line),
                input: LineInput {
                    title: l.title.clone(),
                    auto_real: l.auto_real,
                    categories: l.categories.clone(),
                },
                search: String::new(),
            });
        }
        if self.line_editor.is_some() {
            ui.add_space(8.0);
            self.line_editor_card(ui, doc, index, width, pal);
        }
        let table = &self.tables.tables[index];
        Some((
            format!("{} ({})", table.name, table.year),
            annual_rows(&report),
        ))
    }

    /// Grille : titres figés à gauche, 12 mois et total. Renvoie une saisie
    /// validée `(ligne, mois, texte)` et la ligne dont le titre a été cliqué.
    #[allow(clippy::type_complexity)]
    fn annual_grid(
        &mut self,
        ui: &mut egui::Ui,
        report: &bp_core::budget::AnnualReport,
        index: usize,
        width: f32,
        height: f32,
        pal: &Palette,
    ) -> (Option<(usize, usize, String)>, Option<usize>) {
        const ROW: f32 = 32.0;
        const COL: f32 = 96.0;
        let title_w = 220.0;
        let mut commit = None;
        let mut edit_line = None;
        let mut start_edit = None;
        let view = self.view;
        let pick = |m: &bp_core::budget::MonthValues| match view {
            ValueKind::Budget => m.budget,
            ValueKind::Real => m.real,
            ValueKind::Difference => m.difference,
        };
        let amount = |v: Amount, bold: bool| {
            if v == Amount::ZERO {
                return RichText::new("·").color(pal.muted.gamma_multiply(0.5));
            }
            let color = if view == ValueKind::Difference {
                if v.is_negative() {
                    pal.negative
                } else {
                    pal.positive
                }
            } else {
                amount_color(v, pal)
            };
            let t = RichText::new(money(v, "")).color(color);
            if bold {
                t.family(crate::theme::semibold())
            } else {
                t
            }
        };
        let fixed = |ui: &mut egui::Ui, w: f32, align: egui::Align, text: RichText| {
            let layout = if align == egui::Align::Max {
                egui::Layout::right_to_left(egui::Align::Center)
            } else {
                egui::Layout::left_to_right(egui::Align::Center)
            };
            ui.allocate_ui_with_layout(vec2(w, ROW), layout, |ui| {
                ui.set_min_size(vec2(w, ROW));
                ui.add(egui::Label::new(text).truncate());
            })
            .response
        };
        let lines = report.rows.len();
        egui::ScrollArea::vertical().id_salt("annual_grid").max_height(height).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let background = ui.painter().add(egui::Shape::Noop);
            let top = ui.cursor().top();
            let left = ui.cursor().left();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(title_w);
                    fixed(ui, title_w, egui::Align::Min, RichText::new(trc!("budget", "Ligne")).family(crate::theme::semibold()).color(pal.fg_dark));
                    for (l, row) in report.rows.iter().enumerate() {
                        let glyph = if row.auto_real { icon::LIGHTNING } else { icon::PENCIL_SIMPLE };
                        let response = fixed(
                            ui,
                            title_w,
                            egui::Align::Min,
                            RichText::new(format!("{glyph}  {}", row.title)).family(crate::theme::medium()),
                        )
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(if row.auto_real {
                            tr!("Réel calculé d'après les catégories — cliquer pour modifier la ligne")
                        } else {
                            tr!("Réel saisi à la main — cliquer pour modifier la ligne")
                        });
                        if response.clicked() {
                            edit_line = Some(l);
                        }
                    }
                    fixed(ui, title_w, egui::Align::Min, RichText::new(tr!("Totaux")).family(crate::theme::semibold()));
                });
                egui::ScrollArea::horizontal().id_salt("annual_values").show(ui, |ui| {
                    ui.vertical(|ui| {
                        let size = vec2(COL * 13.0, ROW);
                        let layout = egui::Layout::left_to_right(egui::Align::Center);
                        ui.allocate_ui_with_layout(size, layout, |ui| {
                            for m in 0..12 {
                                fixed(ui, COL, egui::Align::Max, RichText::new(i18n::month_abbr(m)).family(crate::theme::semibold()).color(pal.fg_dark));
                            }
                            fixed(ui, COL, egui::Align::Max, RichText::new(tr!("Total")).family(crate::theme::semibold()).color(pal.fg_dark));
                        });
                        for (l, row) in report.rows.iter().enumerate() {
                            let editable = match view {
                                ValueKind::Budget => true,
                                ValueKind::Real => !row.auto_real,
                                ValueKind::Difference => false,
                            };
                            ui.allocate_ui_with_layout(size, layout, |ui| {
                                for (m, values) in row.months.iter().enumerate() {
                                    let editing = self.cell_edit.as_ref().is_some_and(|(cl, cm, _)| *cl == l && *cm == m);
                                    if editing {
                                        let (_, _, text) = self.cell_edit.as_mut().unwrap();
                                        let edit = ui.add_sized(vec2(COL - 6.0, ROW - 6.0), egui::TextEdit::singleline(text).horizontal_align(egui::Align::Max));
                                        ui.add_space(6.0);
                                        if !edit.has_focus() && !edit.lost_focus() {
                                            edit.request_focus();
                                        }
                                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                                            self.cell_edit = None;
                                        } else if edit.lost_focus() {
                                            commit = Some((l, m, text.clone()));
                                            self.cell_edit = None;
                                        }
                                        continue;
                                    }
                                    let value = pick(values);
                                    let response = fixed(ui, COL, egui::Align::Max, amount(value, false));
                                    if editable {
                                        let response = response.interact(egui::Sense::click()).on_hover_cursor(egui::CursorIcon::Text);
                                        if response.hovered() {
                                            ui.painter().rect_stroke(
                                                response.rect.shrink2(vec2(2.0, 3.0)),
                                                6.0,
                                                egui::Stroke::new(1.0, pal.accent.gamma_multiply(0.5)),
                                                egui::StrokeKind::Inside,
                                            );
                                        }
                                        if response.clicked() {
                                            let text = if value == Amount::ZERO { String::new() } else { money(value, "") };
                                            start_edit = Some((l, m, text));
                                        }
                                    }
                                }
                                fixed(ui, COL, egui::Align::Max, amount(pick(&row.total), true));
                            });
                        }
                        ui.allocate_ui_with_layout(size, layout, |ui| {
                            for values in &report.totals.months {
                                fixed(ui, COL, egui::Align::Max, amount(pick(values), true));
                            }
                            fixed(ui, COL, egui::Align::Max, amount(pick(&report.totals.total), true));
                        });
                        ui.add_space(12.0);
                    });
                });
            });
            let right = left + width - 16.0;
            let mut shapes = Vec::new();
            for k in 0..lines + 2 {
                let rect = egui::Rect::from_min_max(egui::pos2(left, top + k as f32 * ROW), egui::pos2(right, top + (k + 1) as f32 * ROW));
                if k == 0 || k == lines + 1 {
                    let y = if k == 0 { rect.bottom() } else { rect.top() };
                    shapes.push(egui::Shape::line_segment([egui::pos2(left, y), egui::pos2(right, y)], egui::Stroke::new(1.0, pal.border)));
                } else if k % 2 == 0 {
                    shapes.push(egui::Shape::rect_filled(rect, 4.0, pal.stripe.gamma_multiply(0.7)));
                }
            }
            ui.painter().set(background, egui::Shape::Vec(shapes));
        });
        if lines == 0 {
            ui.add_space(8.0);
            ui.label(RichText::new(tr!("Ajoutez une ligne (loyer, courses, salaire…) puis saisissez ses montants mois par mois.")).color(pal.muted));
        }
        if let Some(edit) = start_edit {
            self.cell_edit = Some(edit);
        }
        let _ = index;
        (commit, edit_line)
    }

    fn line_editor_card(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut OpenDoc,
        index: usize,
        width: f32,
        pal: &Palette,
    ) {
        let Some(editor) = &mut self.line_editor else {
            return;
        };
        let mut action = None;
        ui_kit::card(ui, width, |ui| {
            let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
            ui.horizontal(|ui| {
                ui_kit::caption(
                    ui,
                    icon::PENCIL_SIMPLE,
                    if editor.index.is_some() {
                        tr!("Modifier la ligne")
                    } else {
                        tr!("Nouvelle ligne")
                    },
                );
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(
                    crate::theme::field(&mut editor.input.title)
                        .hint_text(tr!("Titre de la ligne"))
                        .desired_width(280.0),
                );
                ui.add_space(16.0);
                crate::theme::checkbox(
                    ui,
                    &mut editor.input.auto_real,
                    tr!("Réel calculé d'après les catégories"),
                );
            });
            if editor.input.auto_real {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(
                        crate::theme::field(&mut editor.search)
                            .hint_text(format!(
                                "{}  {}",
                                icon::MAGNIFYING_GLASS,
                                tr!("Chercher une catégorie")
                            ))
                            .desired_width(280.0),
                    );
                    ui.add_space(12.0);
                    let names: Vec<String> = editor
                        .input
                        .categories
                        .iter()
                        .map(|c| {
                            c.and_then(|id| doc.document.category_label(id))
                                .unwrap_or_else(|| tr!("Sans catégorie").into())
                        })
                        .collect();
                    let summary = if names.is_empty() {
                        tr!("Aucune catégorie choisie").to_string()
                    } else {
                        names.join(", ")
                    };
                    ui.add(egui::Label::new(RichText::new(summary).color(pal.fg_dark)).truncate());
                });
                ui.add_space(6.0);
                let search = editor.search.to_lowercase();
                let mut all: Vec<(Option<u32>, String)> =
                    vec![(None, tr!("Sans catégorie").into())];
                all.extend(doc.document.categories.iter().map(|c| {
                    (
                        Some(c.id),
                        doc.document
                            .category_label(c.id)
                            .unwrap_or_else(|| c.name.clone()),
                    )
                }));
                egui::ScrollArea::vertical()
                    .id_salt("line_categories")
                    .max_height(120.0)
                    .show(ui, |ui| {
                        ui.set_width(inner);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(18.0, 8.0);
                            for (id, label) in all.iter().filter(|(_, l)| {
                                search.is_empty() || l.to_lowercase().contains(&search)
                            }) {
                                let mut on = editor.input.categories.contains(id);
                                if crate::theme::checkbox(ui, &mut on, label.as_str()).changed() {
                                    if on {
                                        editor.input.categories.push(*id);
                                    } else {
                                        editor.input.categories.retain(|c| c != id);
                                    }
                                }
                            }
                        });
                    });
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(crate::theme::primary_button(
                        ui,
                        format!("{}  {}", icon::CHECK, tr!("Enregistrer")),
                    ))
                    .clicked()
                {
                    action = Some(0);
                }
                if ui.button(tr!("Annuler")).clicked() {
                    action = Some(1);
                }
                if editor.index.is_some()
                    && ui
                        .button(
                            RichText::new(format!(
                                "{}  {}",
                                icon::TRASH,
                                tr!("Supprimer la ligne")
                            ))
                            .color(pal.negative),
                        )
                        .clicked()
                {
                    action = Some(2);
                }
            });
        });
        let table = &mut self.tables.tables[index];
        match action {
            Some(0) => {
                let result = match editor.index {
                    Some(i) => table.update_line(i, &editor.input),
                    None => table.add_line(&editor.input).map(|_| ()),
                };
                match result {
                    Ok(()) => {
                        self.line_editor = None;
                        self.error = None;
                        self.save_annual(doc);
                    }
                    Err(e) => self.error = Some(capitalize(&e.to_string())),
                }
            }
            Some(1) => {
                self.line_editor = None;
                self.error = None;
            }
            Some(2) => {
                if let Some(i) = editor.index {
                    let _ = table.remove_line(i);
                }
                self.line_editor = None;
                self.save_annual(doc);
            }
            _ => {}
        }
    }
}

// ─────────────────────────── Aides ───────────────────────────

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

fn spread_label(spread: Spread) -> &'static str {
    match spread {
        Spread::Month => tr!("Au mois choisi"),
        Spread::AllMonths => tr!("À tous les mois"),
        Spread::EveryTwoMonths => tr!("Un mois sur deux"),
        Spread::EveryThreeMonths => tr!("Un mois sur trois"),
    }
}

fn amount_color(v: Amount, pal: &Palette) -> Color32 {
    if v.is_negative() {
        pal.negative
    } else {
        pal.fg
    }
}

fn signed(v: Amount) -> String {
    if v.0 > 0 {
        format!("+{}", money(v, "€"))
    } else {
        money(v, "€")
    }
}

/// Un écart est favorable s'il augmente le solde (moins de dépense, plus de
/// recette).
fn difference_color(d: Amount, _kind: Option<BudgetKind>, pal: &Palette) -> Color32 {
    if d.0 >= 0 { pal.positive } else { pal.negative }
}

/// Part consommée (dépense) ou atteinte (recette) et sa couleur.
fn consumption(real: Amount, budget: Amount, kind: BudgetKind, pal: &Palette) -> (f32, Color32) {
    let ratio = if budget.0 == 0 || (real.0 > 0) != (budget.0 > 0) {
        0.0
    } else {
        real.0 as f32 / budget.0 as f32
    };
    let color = match kind {
        BudgetKind::Expense if ratio > 1.0 => pal.negative,
        BudgetKind::Expense if ratio > 0.85 => pal.warning,
        BudgetKind::Expense => pal.positive,
        BudgetKind::Income if ratio >= 1.0 => pal.positive,
        BudgetKind::Income => pal.accent,
    };
    (ratio, color)
}

fn progress(ui: &mut egui::Ui, width: f32, ratio: f32, color: Color32, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(width.max(20.0), 10.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 5.0, pal.bg_highlight.gamma_multiply(0.8));
    let filled = egui::Rect::from_min_size(
        rect.min,
        vec2(rect.width() * ratio.clamp(0.0, 1.0), rect.height()),
    );
    if filled.width() > 0.5 {
        ui.painter().rect_filled(filled, 5.0, color);
    }
}

fn summary_tile(
    ui: &mut egui::Ui,
    width: f32,
    title: &str,
    glyph: &str,
    total: &BudgetTotal,
    kind: Option<BudgetKind>,
    pal: &Palette,
) {
    ui_kit::card(ui, width, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(glyph).color(pal.accent));
                ui.label(
                    RichText::new(title)
                        .family(crate::theme::semibold())
                        .color(pal.fg_dark),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(tr!("prévu {}", money(total.budget, "€")))
                            .size(12.5)
                            .color(pal.muted),
                    );
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(money(total.real, "€"))
                        .size(22.0)
                        .family(crate::theme::semibold())
                        .color(amount_color(total.real, pal)),
                );
                if total.budget.0 != 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(signed(total.difference)).color(difference_color(
                                total.difference,
                                kind,
                                pal,
                            )),
                        );
                    });
                }
            });
            ui.add_space(6.0);
            let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
            match kind {
                Some(kind) if total.budget.0 != 0 => {
                    let (ratio, color) = consumption(total.real, total.budget, kind, pal);
                    progress(ui, inner, ratio, color, pal);
                }
                _ => {
                    let (rect, _) = ui.allocate_exact_size(vec2(inner, 10.0), egui::Sense::hover());
                    let _ = rect;
                }
            }
        });
    });
}

fn monthly_rows(report: &BudgetReport) -> Vec<Vec<Cell>> {
    let mut rows = vec![
        [
            tr!("Catégorie"),
            tr!("Nature"),
            tr!("Budget"),
            tr!("Réel"),
            tr!("Écart"),
            tr!("Écart %"),
        ]
        .map(|t| Cell::Bold(t.into()))
        .into_iter()
        .collect::<Vec<_>>(),
    ];
    for r in &report.rows {
        let label = if r.is_child {
            format!("    {}", r.label)
        } else {
            r.label.clone()
        };
        rows.push(vec![
            Cell::Text(label),
            Cell::Text(r.kind.map(|k| k.label().to_string()).unwrap_or_default()),
            r.budget.map_or(Cell::Empty, |b| Cell::Money(b.as_f64())),
            Cell::Money(r.real.as_f64()),
            r.difference
                .map_or(Cell::Empty, |d| Cell::Money(d.as_f64())),
            r.percent
                .map_or(Cell::Empty, |p| Cell::Number((p * 10.0).round() / 10.0)),
        ]);
    }
    for (label, t) in [
        (tr!("Dépenses"), &report.expenses),
        (tr!("Recettes"), &report.income),
        (tr!("Total"), &report.total),
    ] {
        rows.push(vec![
            Cell::Bold(label.into()),
            Cell::Empty,
            Cell::BoldMoney(t.budget.as_f64()),
            Cell::BoldMoney(t.real.as_f64()),
            Cell::BoldMoney(t.difference.as_f64()),
            t.percent
                .map_or(Cell::Empty, |p| Cell::Number((p * 10.0).round() / 10.0)),
        ]);
    }
    rows
}

fn annual_rows(report: &bp_core::budget::AnnualReport) -> Vec<Vec<Cell>> {
    let mut header = vec![Cell::Bold(trc!("budget", "Ligne").into())];
    let months = (0..12).map(i18n::month_abbr);
    for m in months.chain(std::iter::once(tr!("Total"))) {
        header.extend([
            Cell::Bold(tr!("{} budget", m)),
            Cell::Bold(tr!("{} réel", m)),
            Cell::Bold(tr!("{} écart", m)),
        ]);
    }
    let mut rows = vec![header];
    let line = |row: &bp_core::budget::AnnualRow, bold: bool| {
        let money = |v: Amount| {
            if bold {
                Cell::BoldMoney(v.as_f64())
            } else {
                Cell::Money(v.as_f64())
            }
        };
        let mut cells = vec![if bold {
            Cell::Bold(row.title.clone())
        } else {
            Cell::Text(row.title.clone())
        }];
        for m in row.months.iter().chain(std::iter::once(&row.total)) {
            cells.extend([money(m.budget), money(m.real), money(m.difference)]);
        }
        cells
    };
    rows.extend(report.rows.iter().map(|r| line(r, false)));
    rows.push(line(&report.totals, true));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_english_thousands() {
        let value = |t: &str| parse_amount(t).map(|a| a.as_f64());
        assert_eq!(value("1 234,56"), Some(1234.56));
        assert_eq!(value("1,500"), Some(1.5));
        i18n::with_lang(Lang::En, || {
            assert_eq!(value("1,234.56"), Some(1234.56));
            assert_eq!(value("-1,500"), Some(-1500.0));
            assert_eq!(value("1,234,567.89"), Some(1_234_567.89));
            assert_eq!(value("-€1,234.50"), Some(-1234.5));
            assert_eq!(value("12,50"), Some(12.5));
            assert_eq!(value("1200/12"), Some(100.0));
            assert_eq!(
                tr!("Montant incorrect : « {} »", "x"),
                "Invalid amount: “x”"
            );
        });
    }
}
