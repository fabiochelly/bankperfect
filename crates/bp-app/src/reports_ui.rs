//! Rapports : total des comptes, totaux par catégorie (Ctrl+T) et par tiers,
//! relevé multicompte ; chacun s'exporte vers un tableur.

use std::collections::HashSet;

use bp_core::model::{Amount, Tack};
use bp_core::reports::{self, AccountScope, Pivot, PivotGoal, PivotOptions};
use bp_core::{tr, trn};
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::spreadsheet::Cell;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReportTab {
    Accounts,
    Categories,
    Payees,
    Statement,
}

pub struct ReportsForm {
    pub tab: ReportTab,
    all_accounts: bool,
    goal: PivotGoal,
    year: Option<i32>,
    all_categories: bool,
    line_totals: bool,
    remove_numbers: bool,
    /// Comptes du relevé multicompte.
    statement_accounts: Vec<bool>,
    statement_filtered: bool,
    visible: VisibleLines,
    message: Option<String>,
}

/// Lignes retenues par les filtres de la grille, compte par compte, mises
/// en cache tant que ni le document ni les filtres ne changent.
#[derive(Default)]
pub struct VisibleLines {
    key: Option<(u64, bp_core::filter::Filters, bp_core::model::Date)>,
    lines: Vec<HashSet<usize>>,
}

impl VisibleLines {
    pub fn get(&mut self, doc: &OpenDoc) -> &[HashSet<usize>] {
        let today = bp_core::model::Date::today();
        let key = (doc.revision(), doc.filters.clone(), today);
        if self.key.as_ref() != Some(&key) {
            let compiled = doc.filters.compile(today);
            self.lines = doc
                .document
                .accounts
                .iter()
                .map(|account| {
                    let index = bp_core::filter::SearchIndex::build(&doc.document, account);
                    let order: Vec<usize> = (0..account.operations.len()).collect();
                    compiled
                        .apply(account, &index, &order)
                        .into_iter()
                        .collect()
                })
                .collect();
            self.key = Some(key);
        }
        &self.lines
    }
}

impl ReportsForm {
    pub fn new(doc: &OpenDoc, tab: ReportTab) -> Self {
        let years = reports::available_years(&doc.document, &AccountScope::All);
        let current = chrono::Datelike::year(&chrono::Local::now().date_naive());
        // Année en cours si elle a des opérations, sinon toutes années.
        let year = years.contains(&current).then_some(current);
        Self {
            tab,
            all_accounts: true,
            goal: PivotGoal::MonthlyTotals,
            year,
            all_categories: true,
            line_totals: false,
            remove_numbers: true,
            statement_accounts: doc
                .document
                .accounts
                .iter()
                .enumerate()
                .map(|(i, a)| i == doc.account || !a.counts_as_virtual())
                .collect(),
            statement_filtered: true,
            visible: VisibleLines::default(),
            message: None,
        }
    }

    fn scope(&self, doc: &OpenDoc) -> AccountScope {
        if self.all_accounts {
            AccountScope::All
        } else {
            AccountScope::One(doc.account)
        }
    }

    fn pivot_options(&self, doc: &OpenDoc) -> PivotOptions {
        PivotOptions {
            scope: self.scope(doc),
            goal: self.goal,
            year: self.year,
            all_categories: self.all_categories,
            line_totals: self.line_totals,
            remove_dates_and_numbers: self.remove_numbers,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.85, 900.0, 1700.0);
        let height = ui_kit::dialog_height(ctx, 0.85).max(560.0);
        crate::dialogs::modal(ctx, "reports").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(220.0, 36.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| crate::dialogs::header(ui, icon::TABLE, pal.accent, tr!("Rapports")),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui_kit::segmented(
                        ui,
                        &mut self.tab,
                        &[
                            (
                                ReportTab::Accounts,
                                icon::BANK,
                                tr!("Total des comptes"),
                                pal.accent,
                            ),
                            (
                                ReportTab::Categories,
                                icon::TAG,
                                tr!("Par catégorie"),
                                pal.accent,
                            ),
                            (
                                ReportTab::Payees,
                                icon::ADDRESS_BOOK,
                                tr!("Par tiers"),
                                pal.accent,
                            ),
                            (
                                ReportTab::Statement,
                                icon::LIST_BULLETS,
                                tr!("Relevé multicompte"),
                                pal.accent,
                            ),
                        ],
                        38.0,
                        Some((width - 240.0).min(780.0)),
                    );
                });
            });
            ui.add_space(14.0);
            let body = height - 36.0 - 14.0 - 100.0;
            let mut export: Option<(String, Vec<Vec<Cell>>)> = None;
            ui.allocate_ui_with_layout(
                egui::vec2(width, body),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_size(egui::vec2(width, body));
                    match self.tab {
                        ReportTab::Accounts => self.accounts(ui, doc, body, &mut export),
                        ReportTab::Categories | ReportTab::Payees => {
                            self.pivot(ui, doc, width, body, &mut export)
                        }
                        ReportTab::Statement => self.statement(ui, doc, width, body, &mut export),
                    }
                },
            );
            if let Some(message) = &self.message {
                ui.label(RichText::new(format!("{}  {message}", icon::INFO)).color(pal.accent));
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, tr!("Fermer"), true).clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    outcome = Outcome::Close;
                }
                if let Some((name, rows)) = export
                    && crate::dialogs::button(
                        ui,
                        tr!("{}  Exporter vers un tableur…", icon::MICROSOFT_EXCEL_LOGO),
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

    // ─────────────────────────── Total des comptes ───────────────────────────

    fn accounts(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut OpenDoc,
        body: f32,
        export: &mut Option<(String, Vec<Vec<Cell>>)>,
    ) {
        let pal = palette(ui);
        let totals = reports::account_totals(&doc.document);
        let mut rows = vec![vec![
            Cell::Bold(tr!("Compte").into()),
            Cell::Bold(tr!("Pointé").into()),
            Cell::Bold(tr!("À venir").into()),
            Cell::Bold(tr!("Solde").into()),
        ]];
        let mut select = None;
        egui::ScrollArea::vertical()
            .id_salt("acc_totals")
            .max_height(body)
            .show(ui, |ui| {
                const NAME: f32 = 300.0;
                const AMOUNT: f32 = 170.0;
                let table = NAME + 3.0 * AMOUNT + 24.0;
                let row = |ui: &mut egui::Ui,
                           k: usize,
                           strong: bool,
                           add: &mut dyn FnMut(&mut egui::Ui)| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(table, 40.0), egui::Sense::hover());
                    if strong {
                        ui.painter().line_segment(
                            [rect.left_top(), rect.right_top()],
                            egui::Stroke::new(1.0, pal.border),
                        );
                    } else if k % 2 == 1 {
                        ui.painter()
                            .rect_filled(rect, 6.0, pal.stripe.gamma_multiply(0.7));
                    }
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect.shrink2(egui::vec2(12.0, 0.0)))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    child.spacing_mut().item_spacing.x = 0.0;
                    add(&mut child);
                };
                row(ui, 0, false, &mut |ui| {
                    for (k, title) in [tr!("Compte"), tr!("Pointé"), tr!("À venir"), tr!("Solde")]
                        .into_iter()
                        .enumerate()
                    {
                        let text = RichText::new(title)
                            .family(crate::theme::semibold())
                            .color(pal.fg_dark);
                        if k == 0 {
                            ui_kit::cell(ui, NAME, egui::Align::Min, text);
                        } else {
                            ui_kit::cell(ui, AMOUNT, egui::Align::Max, text);
                        }
                    }
                });
                for (k, line) in totals.rows.iter().enumerate() {
                    let currency = doc.document.accounts[line.account].currency.clone();
                    row(ui, k + 1, false, &mut |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(NAME, 24.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(NAME);
                                if ui
                                    .link(
                                        RichText::new(format!("{}  {}", icon::BANK, line.name))
                                            .family(crate::theme::medium()),
                                    )
                                    .on_hover_text(tr!("Afficher ce compte"))
                                    .clicked()
                                {
                                    select = Some(line.account);
                                }
                            },
                        );
                        for value in [
                            line.totals.pointed,
                            line.totals.upcoming,
                            line.totals.balance,
                        ] {
                            amount_label(ui, AMOUNT, value, &currency, false);
                        }
                    });
                    rows.push(vec![
                        Cell::Text(line.name.clone()),
                        Cell::Money(line.totals.pointed.as_f64()),
                        Cell::Money(line.totals.upcoming.as_f64()),
                        Cell::Money(line.totals.balance.as_f64()),
                    ]);
                }
                row(ui, 0, true, &mut |ui| {
                    ui_kit::cell(
                        ui,
                        NAME,
                        egui::Align::Min,
                        RichText::new(tr!("Total")).family(crate::theme::semibold()),
                    );
                    for value in [
                        totals.total.pointed,
                        totals.total.upcoming,
                        totals.total.balance,
                    ] {
                        amount_label(ui, AMOUNT, value, doc.main_currency(), true);
                    }
                });
                rows.push(vec![
                    Cell::Bold(tr!("Total").into()),
                    Cell::BoldMoney(totals.total.pointed.as_f64()),
                    Cell::BoldMoney(totals.total.upcoming.as_f64()),
                    Cell::BoldMoney(totals.total.balance.as_f64()),
                ]);
                ui.add_space(8.0);
                ui.label(
                    RichText::new(tr!(
                        "Les comptes virtuels ne sont pas comptés dans le total."
                    ))
                    .size(12.5)
                    .color(pal.muted),
                );
            });
        if let Some(i) = select {
            doc.select_account(i);
        }
        *export = Some((tr!("Total des comptes").into(), rows));
    }

    // ─────────────────────────── Tableaux croisés ───────────────────────────

    fn pivot(
        &mut self,
        ui: &mut egui::Ui,
        doc: &OpenDoc,
        width: f32,
        body: f32,
        export: &mut Option<(String, Vec<Vec<Cell>>)>,
    ) {
        let pal = palette(ui);
        let by_payee = self.tab == ReportTab::Payees;
        // Options.
        ui.horizontal_wrapped(|ui| {
            ui_kit::segmented(
                ui,
                &mut self.all_accounts,
                &[
                    (true, "", tr!("Tous les comptes"), pal.accent),
                    (false, "", tr!("Compte courant"), pal.accent),
                ],
                32.0,
                Some(300.0),
            );
            ui.add_space(12.0);
            ui_kit::segmented(
                ui,
                &mut self.goal,
                &[
                    (PivotGoal::MonthlyTotals, "", tr!("Par mois"), pal.accent),
                    (PivotGoal::YearlyTotals, "", tr!("Par année"), pal.accent),
                    (
                        PivotGoal::YearlyAverages,
                        "",
                        tr!("Moyenne mensuelle"),
                        pal.accent,
                    ),
                ],
                32.0,
                Some(420.0),
            );
            if self.goal == PivotGoal::MonthlyTotals {
                ui.add_space(12.0);
                let years = reports::available_years(&doc.document, &self.scope(doc));
                egui::ComboBox::from_id_salt("pivot_year")
                    .selected_text(
                        self.year
                            .map_or(tr!("Toutes années").to_string(), |y| y.to_string()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.year, None, tr!("Toutes années"));
                        for y in years.iter().rev() {
                            ui.selectable_value(&mut self.year, Some(*y), y.to_string());
                        }
                    });
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if by_payee {
                crate::theme::checkbox(
                    ui,
                    &mut self.remove_numbers,
                    tr!("Regrouper les tiers sans dates ni numéros"),
                );
            } else {
                crate::theme::checkbox(
                    ui,
                    &mut self.all_categories,
                    tr!("Détailler les sous-catégories"),
                );
            }
            ui.add_space(12.0);
            crate::theme::checkbox(
                ui,
                &mut self.line_totals,
                tr!("Dépenses et recettes calculées opération par opération"),
            );
        });
        ui.add_space(10.0);
        let options = self.pivot_options(doc);
        let pivot = if by_payee {
            reports::payee_totals(&doc.document, &options)
        } else {
            reports::category_totals(&doc.document, &options)
        };
        pivot_table(ui, &pivot, width, body - 90.0, by_payee);
        let title = if by_payee {
            tr!("Totaux par tiers")
        } else {
            tr!("Totaux par catégorie")
        };
        *export = Some((
            title.into(),
            pivot_rows(
                &pivot,
                if by_payee {
                    tr!("Tiers")
                } else {
                    tr!("Catégorie")
                },
            ),
        ));
    }

    // ─────────────────────────── Relevé multicompte ───────────────────────────

    fn statement(
        &mut self,
        ui: &mut egui::Ui,
        doc: &OpenDoc,
        width: f32,
        body: f32,
        export: &mut Option<(String, Vec<Vec<Cell>>)>,
    ) {
        let pal = palette(ui);
        self.statement_accounts
            .resize(doc.document.accounts.len(), false);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr!("Comptes :")).color(pal.fg_dark));
            for &i in &doc.document.account_order {
                crate::theme::checkbox(
                    ui,
                    &mut self.statement_accounts[i],
                    doc.document.accounts[i].name.as_str(),
                );
            }
        });
        ui.add_space(4.0);
        crate::theme::checkbox(
            ui,
            &mut self.statement_filtered,
            tr!("Appliquer les filtres de la grille à chaque compte"),
        );
        ui.add_space(10.0);
        let chosen: Vec<usize> = doc
            .document
            .account_order
            .iter()
            .copied()
            .filter(|&i| self.statement_accounts.get(i).copied().unwrap_or(false))
            .collect();
        let visible = self.visible.get(doc);
        let filtered = self.statement_filtered;
        let keep = |account: usize, line: usize, _op: &bp_core::model::Operation| {
            !filtered || visible[account].contains(&line)
        };
        let statement = reports::multi_account_statement(&doc.document, &chosen, &keep);
        ui.horizontal(|ui| {
            ui_kit::pill(
                ui,
                &trn!(statement.lines.len(), "{} ligne", "{} lignes"),
                pal.accent,
            );
            ui_kit::pill(
                ui,
                &tr!(
                    "Pointé {}",
                    money(statement.totals.pointed, doc.main_currency())
                ),
                pal.pointed,
            );
            ui_kit::pill(
                ui,
                &tr!(
                    "À venir {}",
                    money(statement.totals.upcoming, doc.main_currency())
                ),
                pal.future,
            );
            ui_kit::pill(
                ui,
                &tr!(
                    "Solde {}",
                    money(statement.totals.balance, doc.main_currency())
                ),
                pal.positive,
            );
        });
        ui.add_space(8.0);
        let mut rows = vec![
            [
                tr!("Date"),
                tr!("Compte"),
                tr!("Mode"),
                tr!("Tiers"),
                tr!("Détail"),
                tr!("Montant"),
                tr!("Pointage"),
            ]
            .map(|t| Cell::Bold(t.into()))
            .into_iter()
            .collect::<Vec<_>>(),
        ];
        let payee_width = (width - 600.0).max(160.0);
        ui.horizontal(|ui| {
            for (w, align, title) in [
                (96.0, egui::Align::Min, tr!("Date")),
                (140.0, egui::Align::Min, tr!("Compte")),
                (130.0, egui::Align::Min, tr!("Mode")),
                (payee_width, egui::Align::Min, tr!("Tiers")),
                (120.0, egui::Align::Max, tr!("Montant")),
            ] {
                ui_kit::cell(
                    ui,
                    w,
                    align,
                    RichText::new(title)
                        .family(crate::theme::semibold())
                        .color(pal.fg_dark),
                );
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("statement")
            .max_height(body - 120.0)
            .auto_shrink([false, false])
            .show_rows(ui, 26.0, statement.lines.len(), |ui, range| {
                for line in &statement.lines[range] {
                    let account = &doc.document.accounts[line.account];
                    let op = &account.operations[line.line];
                    ui.horizontal(|ui| {
                        ui_kit::cell(
                            ui,
                            96.0,
                            egui::Align::Min,
                            RichText::new(crate::settings::shared::date(op.date)),
                        );
                        ui_kit::cell(
                            ui,
                            140.0,
                            egui::Align::Min,
                            RichText::new(&account.name).color(pal.fg_dark),
                        );
                        ui_kit::cell(
                            ui,
                            130.0,
                            egui::Align::Min,
                            RichText::new(op.mode_label()).color(pal.fg_dark),
                        );
                        ui_kit::cell(
                            ui,
                            payee_width,
                            egui::Align::Min,
                            RichText::new(bp_core::split::payee_label(&op.payee)),
                        );
                        let color = if op.amount.is_negative() {
                            pal.negative
                        } else {
                            pal.positive
                        };
                        ui_kit::cell(
                            ui,
                            120.0,
                            egui::Align::Max,
                            RichText::new(money(op.amount, &account.currency)).color(color),
                        );
                        let tack = match op.tack {
                            Tack::None => "",
                            Tack::Pointed => icon::CHECK,
                            Tack::Reconciled => icon::CHECKS,
                        };
                        ui.label(RichText::new(tack).color(pal.pointed));
                    });
                }
            });
        for line in &statement.lines {
            let account = &doc.document.accounts[line.account];
            let op = &account.operations[line.line];
            rows.push(vec![
                Cell::Text(op.date.to_string()),
                Cell::Text(account.name.clone()),
                Cell::Text(op.mode_label()),
                Cell::Text(bp_core::split::payee_label(&op.payee).into_owned()),
                Cell::Text(op.details.clone()),
                Cell::Money(op.amount.as_f64()),
                Cell::Text(tack_letter(op.tack).into()),
            ]);
        }
        *export = Some((tr!("Relevé multicompte").into(), rows));
    }
}

/// Pointage dans les exports : `P` (pointé), `R` (rapproché) ; `C`
/// (cleared) pour le pointé en anglais.
fn tack_letter(tack: Tack) -> &'static str {
    match tack {
        Tack::None => "",
        Tack::Pointed => tr!("P"),
        Tack::Reconciled => "R",
    }
}

fn amount_label(ui: &mut egui::Ui, width: f32, value: Amount, currency: &str, bold: bool) {
    let pal = palette(ui);
    let color = if value.is_negative() {
        pal.negative
    } else {
        pal.fg
    };
    let mut text = RichText::new(money(value, currency)).color(color);
    if bold {
        text = text.family(crate::theme::semibold());
    }
    ui_kit::cell(ui, width, egui::Align::Max, text);
}

/// Tableau croisé : libellés figés à gauche, une colonne par période (qui
/// défilent horizontalement), lignes de totaux en bas.
fn pivot_table(ui: &mut egui::Ui, pivot: &Pivot, width: f32, height: f32, by_payee: bool) {
    const ROW: f32 = 28.0;
    const COL: f32 = 100.0;
    let pal = palette(ui);
    let label_width = if by_payee { 260.0 } else { 240.0 };
    enum Line<'a> {
        Header,
        Carried(&'a [Amount]),
        Row(&'a bp_core::reports::PivotRow),
        Total(&'static str, &'a [Amount]),
    }
    let mut lines = vec![Line::Header];
    if let Some(carried) = &pivot.carried_balance {
        lines.push(Line::Carried(carried));
    }
    lines.extend(pivot.rows.iter().map(Line::Row));
    lines.push(Line::Total(tr!("Dépenses"), &pivot.debit));
    lines.push(Line::Total(tr!("Recettes"), &pivot.credit));
    lines.push(Line::Total(tr!("Total"), &pivot.total));
    let first_total = lines.len() - 3;
    let fixed = |ui: &mut egui::Ui, w: f32, align: egui::Align, text: RichText| {
        let layout = if align == egui::Align::Max {
            egui::Layout::right_to_left(egui::Align::Center)
        } else {
            egui::Layout::left_to_right(egui::Align::Center)
        };
        ui.allocate_ui_with_layout(egui::vec2(w, ROW), layout, |ui| {
            ui.set_min_size(egui::vec2(w, ROW));
            ui.add(egui::Label::new(text).truncate());
        });
    };
    let amount = |value: Amount, bold: bool| {
        if value == Amount::ZERO {
            return RichText::new("·").color(pal.muted.gamma_multiply(0.5));
        }
        let color = if value.is_negative() {
            pal.negative
        } else {
            pal.positive
        };
        let text = RichText::new(money(value, "")).color(color);
        if bold {
            text.family(crate::theme::semibold())
        } else {
            text
        }
    };
    egui::ScrollArea::vertical()
        .id_salt("pivot")
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let background = ui.painter().add(egui::Shape::Noop);
            let top = ui.cursor().top();
            let left = ui.cursor().left();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(label_width);
                    for line in &lines {
                        let text = match line {
                            Line::Header => RichText::new(if by_payee {
                                tr!("Tiers")
                            } else {
                                tr!("Catégorie")
                            })
                            .family(crate::theme::semibold())
                            .color(pal.fg_dark),
                            Line::Carried(_) => RichText::new(tr!("Solde reporté"))
                                .italics()
                                .color(pal.muted),
                            Line::Row(row) if row.is_child => {
                                RichText::new(format!("      {}", row.label)).color(pal.fg_dark)
                            }
                            Line::Row(row) if by_payee => {
                                RichText::new(bp_core::split::payee_label(&row.label))
                                    .family(crate::theme::medium())
                            }
                            Line::Row(row) => {
                                RichText::new(&row.label).family(crate::theme::medium())
                            }
                            Line::Total(label, _) => {
                                RichText::new(*label).family(crate::theme::semibold())
                            }
                        };
                        fixed(ui, label_width, egui::Align::Min, text);
                    }
                });
                egui::ScrollArea::horizontal()
                    .id_salt("pivot_values")
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            let row_size = egui::vec2(COL * pivot.columns.len() as f32, ROW);
                            let row_layout = egui::Layout::left_to_right(egui::Align::Center);
                            for line in &lines {
                                ui.allocate_ui_with_layout(row_size, row_layout, |ui| match line {
                                    Line::Header => {
                                        for title in &pivot.columns {
                                            fixed(
                                                ui,
                                                COL,
                                                egui::Align::Max,
                                                RichText::new(title)
                                                    .family(crate::theme::semibold())
                                                    .color(pal.fg_dark),
                                            );
                                        }
                                    }
                                    Line::Carried(values) => {
                                        for v in *values {
                                            fixed(
                                                ui,
                                                COL,
                                                egui::Align::Max,
                                                amount(*v, false).italics(),
                                            );
                                        }
                                    }
                                    Line::Row(row) => {
                                        for v in &row.values {
                                            fixed(ui, COL, egui::Align::Max, amount(*v, false));
                                        }
                                    }
                                    Line::Total(_, values) => {
                                        for v in *values {
                                            fixed(ui, COL, egui::Align::Max, amount(*v, true));
                                        }
                                    }
                                });
                            }
                            ui.add_space(12.0);
                        });
                    });
            });
            // Rayures et séparateurs, sous le texte.
            let right = left + width - 16.0;
            let mut shapes = Vec::new();
            for k in 0..lines.len() {
                let rect = egui::Rect::from_min_max(
                    egui::pos2(left, top + k as f32 * ROW),
                    egui::pos2(right, top + (k + 1) as f32 * ROW),
                );
                if k == 0 || k == first_total {
                    let y = if k == 0 { rect.bottom() } else { rect.top() };
                    shapes.push(egui::Shape::line_segment(
                        [egui::pos2(left, y), egui::pos2(right, y)],
                        egui::Stroke::new(1.0, pal.border),
                    ));
                } else if k < first_total && k % 2 == 0 {
                    shapes.push(egui::Shape::rect_filled(
                        rect,
                        4.0,
                        pal.stripe.gamma_multiply(0.7),
                    ));
                }
            }
            ui.painter().set(background, egui::Shape::Vec(shapes));
        });
}

fn pivot_rows(pivot: &Pivot, first: &str) -> Vec<Vec<Cell>> {
    let mut rows = Vec::new();
    let mut header = vec![Cell::Bold(first.into())];
    header.extend(pivot.columns.iter().map(|c| Cell::Bold(c.clone())));
    rows.push(header);
    if let Some(carried) = &pivot.carried_balance {
        let mut row = vec![Cell::Text(tr!("Solde reporté").into())];
        row.extend(carried.iter().map(|v| Cell::Money(v.as_f64())));
        rows.push(row);
    }
    for r in &pivot.rows {
        let label = if r.is_child {
            format!("    {}", r.label)
        } else {
            r.label.clone()
        };
        let mut row = vec![Cell::Text(label)];
        row.extend(r.values.iter().map(|v| {
            if *v == Amount::ZERO {
                Cell::Empty
            } else {
                Cell::Money(v.as_f64())
            }
        }));
        rows.push(row);
    }
    for (label, values) in [
        (tr!("Dépenses"), &pivot.debit),
        (tr!("Recettes"), &pivot.credit),
        (tr!("Total"), &pivot.total),
    ] {
        let mut row = vec![Cell::Bold(label.into())];
        row.extend(values.iter().map(|v| Cell::BoldMoney(v.as_f64())));
        rows.push(row);
    }
    rows
}

/// Lignes affichées de la grille (ordre et filtres courants), pour l'export
/// tableur du compte courant.
pub fn visible_rows(doc: &mut OpenDoc) -> Vec<Vec<Cell>> {
    doc.refresh();
    let mut rows = vec![
        [
            tr!("Date"),
            tr!("Mode"),
            tr!("Tiers"),
            tr!("Détail"),
            tr!("Catégorie"),
            tr!("Montant"),
            tr!("Solde"),
            tr!("Pointage"),
        ]
        .map(|t| Cell::Bold(t.into()))
        .into_iter()
        .collect::<Vec<_>>(),
    ];
    let visible = doc.visible();
    let account = &doc.document.accounts[doc.account];
    for &line in visible.iter() {
        let op = &account.operations[line];
        rows.push(vec![
            Cell::Text(op.date.to_string()),
            Cell::Text(op.mode_label()),
            Cell::Text(bp_core::split::payee_label(&op.payee).into_owned()),
            Cell::Text(op.details.clone()),
            Cell::Text(
                op.category
                    .and_then(|c| doc.document.category_label(c))
                    .unwrap_or_default(),
            ),
            Cell::Money(op.amount.as_f64()),
            Cell::Money(doc.balance_of(line).as_f64()),
            Cell::Text(tack_letter(op.tack).into()),
        ]);
    }
    rows
}
