//! Simulateur de crédit (Ctrl+L) : trois valeurs saisies parmi capital,
//! taux, durée et mensualité, la quatrième calculée ; coût, graphique du
//! restant dû et tableau d'amortissement ; profils nommés.

use bp_core::calc::{AmortizationRow, Loan, LoanUnknown};
use bp_core::i18n;
use bp_core::{tr, trc, trn};
use chrono::Datelike;
use egui::{Color32, Key, RichText, Stroke, pos2, vec2};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

use crate::spreadsheet::Cell;
use crate::theme::{Palette, palette};
use crate::tools::Outcome;
use crate::ui_kit;

const PROFILES_FILE: &str = "credits.json";

/// Nom du mois `month` (de 1 à 12) dans la langue de l'interface.
fn month_name(month: u32) -> &'static str {
    i18n::month_name((month as usize).saturating_sub(1) % 12)
}

/// Profil enregistré.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Profile {
    name: String,
    capital: f64,
    annual_rate: f64,
    months: f64,
    payment: f64,
    #[serde(default)]
    residual: f64,
    #[serde(default)]
    start_of_period: bool,
    start_year: i32,
    start_month: u32,
}

fn load_profiles() -> Vec<Profile> {
    crate::settings::config_file(PROFILES_FILE)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Vec<Profile>>(&b).ok())
        .unwrap_or_default()
}

/// Champs du formulaire, dans l'ordre d'affichage.
const FIELDS: [LoanUnknown; 4] = [
    LoanUnknown::Capital,
    LoanUnknown::Rate,
    LoanUnknown::Months,
    LoanUnknown::Payment,
];

pub struct LoanForm {
    /// Textes saisis : capital, taux, durée, mensualité.
    texts: [String; 4],
    residual: String,
    start_of_period: bool,
    start_year: i32,
    start_month: u32,
    /// Dernière valeur calculée, recalculée après chaque « + » / « − ».
    unknown: LoanUnknown,
    loan: Option<Loan>,
    profiles: Vec<Profile>,
    profile_name: String,
    selected: Option<usize>,
    error: Option<String>,
    message: Option<String>,
}

impl LoanForm {
    pub fn new() -> Self {
        let today = chrono::Local::now().date_naive();
        let (year, month) = if today.month() == 12 {
            (today.year() + 1, 1)
        } else {
            (today.year(), today.month() + 1)
        };
        let mut form = Self {
            texts: [
                crate::format::count(150_000),
                format!("3{}5", i18n::decimal_separator()),
                "240".into(),
                String::new(),
            ],
            residual: String::new(),
            start_of_period: false,
            start_year: year,
            start_month: month,
            unknown: LoanUnknown::Payment,
            loan: None,
            profiles: load_profiles(),
            profile_name: String::new(),
            selected: None,
            error: None,
            message: None,
        };
        form.solve(LoanUnknown::Payment);
        form
    }

    fn value(&self, field: LoanUnknown) -> Option<f64> {
        parse(&self.texts[index(field)])
    }

    fn inputs(&self) -> Loan {
        Loan {
            capital: self.value(LoanUnknown::Capital).unwrap_or(0.0),
            annual_rate: self.value(LoanUnknown::Rate).unwrap_or(0.0),
            months: self.value(LoanUnknown::Months).unwrap_or(0.0),
            payment: self.value(LoanUnknown::Payment).unwrap_or(0.0),
            residual: parse(&self.residual).unwrap_or(0.0),
            start_of_period: self.start_of_period,
        }
    }

    /// Calcule `unknown` et met son champ à jour.
    fn solve(&mut self, unknown: LoanUnknown) {
        self.unknown = unknown;
        let missing: Vec<&str> = FIELDS
            .iter()
            .filter(|&&f| f != unknown && self.value(f).is_none())
            .map(|&f| label(f))
            .collect();
        if !missing.is_empty() {
            self.error = Some(tr!("Valeur à saisir : {}", missing.join(", ")));
            self.loan = None;
            return;
        }
        match self.inputs().solve(unknown) {
            Ok(loan) => {
                let value = match unknown {
                    LoanUnknown::Capital => loan.capital,
                    LoanUnknown::Rate => loan.annual_rate,
                    LoanUnknown::Months => loan.months,
                    LoanUnknown::Payment => loan.payment,
                };
                self.texts[index(unknown)] = format_value(unknown, value);
                self.loan = Some(loan);
                self.error = None;
            }
            Err(e) => {
                self.error = Some(capitalize(&e.to_string()));
                self.loan = None;
            }
        }
    }

    /// « + » / « − » : fait varier un champ puis relance le dernier calcul.
    fn step(&mut self, field: LoanUnknown, up: bool) {
        let Some(v) = self.value(field) else { return };
        let delta = match field {
            LoanUnknown::Capital => 1000.0,
            LoanUnknown::Rate => 0.1,
            LoanUnknown::Months => 12.0,
            LoanUnknown::Payment => 10.0,
        };
        let v = (if up { v + delta } else { v - delta }).max(0.0);
        self.texts[index(field)] = format_value(field, v);
        let unknown = if field == self.unknown {
            LoanUnknown::Payment
        } else {
            self.unknown
        };
        if unknown != field {
            self.solve(unknown);
        }
    }

    fn apply_profile(&mut self, p: &Profile) {
        self.texts = [
            format_value(LoanUnknown::Capital, p.capital),
            format_value(LoanUnknown::Rate, p.annual_rate),
            format_value(LoanUnknown::Months, p.months),
            format_value(LoanUnknown::Payment, p.payment),
        ];
        self.residual = if p.residual == 0.0 {
            String::new()
        } else {
            format_value(LoanUnknown::Capital, p.residual)
        };
        self.start_of_period = p.start_of_period;
        self.start_year = p.start_year;
        self.start_month = p.start_month.clamp(1, 12);
        self.profile_name = p.name.clone();
        self.solve(self.unknown);
    }

    fn save_profiles(&mut self) -> bool {
        match serde_json::to_vec_pretty(&self.profiles)
            .map_err(|e| e.to_string())
            .and_then(|json| crate::settings::write_config_file(PROFILES_FILE, &json))
        {
            Ok(()) => true,
            Err(e) => {
                self.error = Some(tr!("Enregistrement impossible : {}", e));
                false
            }
        }
    }

    pub fn show_standalone(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.85, 900.0, 1600.0);
        let height = ui_kit::dialog_height(ctx, 0.88).max(600.0);
        crate::dialogs::modal(ctx, "loan").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::HOUSE_LINE, pal.accent, tr!("Simulateur de crédit"));
            ui_kit::subtitle(ui, tr!("Saisissez trois valeurs : la quatrième se calcule. Les boutons + et − font varier une valeur et relancent le dernier calcul."));
            let body = height - 36.0 - 40.0 - 100.0;
            let gap = 22.0;
            let left_w = (width * 0.4).clamp(440.0, 540.0);
            let right_w = width - left_w - gap;
            let mut restart = None;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.allocate_ui_with_layout(vec2(left_w, body), egui::Layout::top_down(egui::Align::Min), |ui| {
                    egui::ScrollArea::vertical().id_salt("loan_inputs").max_height(body).show(ui, |ui| {
                        ui.set_width(left_w - 12.0);
                        self.inputs_card(ui, left_w - 12.0, &pal);
                        self.profiles_card(ui, left_w - 12.0, &pal);
                    });
                });
                ui.allocate_ui_with_layout(vec2(right_w, body), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_min_size(vec2(right_w, body));
                    restart = self.results(ui, right_w, body, &pal);
                });
            });
            if let Some(row) = restart
                && let Some(loan) = self.loan
            {
                let next = loan.restart_after(&row);
                self.texts = [
                    format_value(LoanUnknown::Capital, next.capital),
                    format_value(LoanUnknown::Rate, next.annual_rate),
                    format_value(LoanUnknown::Months, next.months),
                    format_value(LoanUnknown::Payment, next.payment),
                ];
                let (y, m) = if row.month == 12 { (row.year + 1, 1) } else { (row.year, row.month + 1) };
                self.start_year = y;
                self.start_month = m;
                self.loan = Some(next);
                self.message = Some(tr!("Reprise après l'échéance n° {} : {} restant dû", row.period, money(row.remaining)));
            }
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            } else if let Some(message) = &self.message {
                ui.label(RichText::new(format!("{}  {message}", icon::INFO)).color(pal.accent));
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, tr!("Fermer"), true).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
                if let Some(loan) = self.loan
                    && crate::dialogs::button(ui, tr!("{}  Exporter vers un tableur…", icon::MICROSOFT_EXCEL_LOGO)).clicked()
                {
                    let rows = export_rows(&loan, self.start_year, self.start_month);
                    let dir = crate::settings::shared::export_dir(&dirs_home());
                    crate::spreadsheet::export(ui.ctx(), &dir, tr!("Simulation de crédit"), tr!("Amortissement"), rows);
                }
            });
        });
        outcome
    }

    fn inputs_card(&mut self, ui: &mut egui::Ui, width: f32, pal: &Palette) {
        let mut solve = None;
        let mut step = None;
        ui_kit::section(ui, width, icon::CALCULATOR, tr!("Crédit"), |ui| {
            let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
            const LABEL: f32 = 118.0;
            let field_w = inner - LABEL - 32.0 - 32.0 - 40.0 - 4.0 * 6.0;
            for field in FIELDS {
                let computed = field == self.unknown && self.loan.is_some();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let color = if computed { pal.accent } else { pal.fg_dark };
                    ui_kit::cell(
                        ui,
                        LABEL,
                        egui::Align::Min,
                        RichText::new(label(field)).color(color),
                    );
                    let text = &mut self.texts[index(field)];
                    let mut edit = crate::theme::field(text).desired_width(field_w);
                    if computed {
                        edit = edit.text_color(pal.accent);
                    }
                    let response = ui.add(edit.hint_text(hint(field)));
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        solve = Some(self.unknown);
                    }
                    if ui
                        .add_sized(vec2(32.0, 32.0), egui::Button::new(icon::MINUS))
                        .on_hover_text(tr!("Diminuer"))
                        .clicked()
                    {
                        step = Some((field, false));
                    }
                    if ui
                        .add_sized(vec2(32.0, 32.0), egui::Button::new(icon::PLUS))
                        .on_hover_text(tr!("Augmenter"))
                        .clicked()
                    {
                        step = Some((field, true));
                    }
                    let calc = egui::Button::new(RichText::new(icon::EQUALS).color(if computed {
                        pal.on_accent
                    } else {
                        pal.accent
                    }))
                    .fill(if computed {
                        pal.accent
                    } else {
                        pal.accent.gamma_multiply(0.12)
                    });
                    if ui
                        .add_sized(vec2(40.0, 32.0), calc)
                        .on_hover_text(tr!("Calculer : {}", label(field).to_lowercase()))
                        .clicked()
                    {
                        solve = Some(field);
                    }
                });
                ui.add_space(8.0);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui_kit::cell(ui, LABEL, egui::Align::Min, RichText::new(tr!("Résiduel")).color(pal.fg_dark));
                let r = ui
                    .add(crate::theme::field(&mut self.residual).hint_text("0").desired_width(field_w))
                    .on_hover_text(tr!("Valeur restant due après la dernière mensualité (location avec option d'achat, crédit ballon)"));
                if r.lost_focus() {
                    solve = Some(self.unknown);
                }
            });
            ui.add_space(8.0);
            if crate::theme::checkbox(
                ui,
                &mut self.start_of_period,
                tr!("Mensualités versées en début de période"),
            )
            .changed()
            {
                solve = Some(self.unknown);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr!("Première échéance")).color(pal.fg_dark));
                egui::ComboBox::from_id_salt("loan_start_month")
                    .selected_text(month_name(self.start_month))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for m in 1..=12 {
                            ui.selectable_value(&mut self.start_month, m, month_name(m));
                        }
                    });
                ui.add(egui::DragValue::new(&mut self.start_year).range(1950..=2200));
            });
        });
        if let Some((field, up)) = step {
            self.step(field, up);
        }
        if let Some(field) = solve {
            self.message = None;
            self.solve(field);
        }
    }

    fn profiles_card(&mut self, ui: &mut egui::Ui, width: f32, pal: &Palette) {
        let mut load = None;
        let mut save = false;
        let mut delete = None;
        ui_kit::section(ui, width, icon::BOOKMARK_SIMPLE, tr!("Profils"), |ui| {
            let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
            if self.profiles.is_empty() {
                ui.label(
                    RichText::new(tr!(
                        "Enregistrez une simulation pour la retrouver plus tard."
                    ))
                    .color(pal.muted),
                );
            }
            for (k, p) in self.profiles.iter().enumerate() {
                ui.horizontal(|ui| {
                    let text = format!("{}  {}", icon::BOOKMARK_SIMPLE, p.name);
                    if ui
                        .add_sized(
                            vec2(inner - 44.0, 28.0),
                            egui::Button::selectable(self.selected == Some(k), text),
                        )
                        .clicked()
                    {
                        load = Some(k);
                    }
                    if ui
                        .add(egui::Button::new(
                            RichText::new(icon::TRASH).color(pal.negative),
                        ))
                        .on_hover_text(tr!("Supprimer le profil"))
                        .clicked()
                    {
                        delete = Some(k);
                    }
                });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(
                    crate::theme::field(&mut self.profile_name)
                        .hint_text(tr!("Nom du profil"))
                        .desired_width(inner - 140.0),
                );
                save = ui
                    .add_enabled(
                        self.loan.is_some() && !self.profile_name.trim().is_empty(),
                        egui::Button::new(tr!("{}  Enregistrer", icon::FLOPPY_DISK)),
                    )
                    .clicked();
            });
        });
        if let Some(k) = load {
            self.selected = Some(k);
            let p = self.profiles[k].clone();
            self.apply_profile(&p);
            self.message = Some(tr!("Profil « {} » chargé", p.name));
        }
        if save && let Some(loan) = self.loan {
            let name = self.profile_name.trim().to_string();
            let profile = Profile {
                name: name.clone(),
                capital: loan.capital,
                annual_rate: loan.annual_rate,
                months: loan.months,
                payment: loan.payment,
                residual: loan.residual,
                start_of_period: loan.start_of_period,
                start_year: self.start_year,
                start_month: self.start_month,
            };
            match self.profiles.iter().position(|p| p.name == name) {
                Some(k) => self.profiles[k] = profile,
                None => self.profiles.push(profile),
            }
            self.profiles.sort_by_key(|p| p.name.to_lowercase());
            self.selected = self.profiles.iter().position(|p| p.name == name);
            if self.save_profiles() {
                self.message = Some(tr!("Profil « {} » enregistré", name));
            }
        }
        if let Some(k) = delete {
            let removed = self.profiles.remove(k);
            self.selected = None;
            if self.save_profiles() {
                self.message = Some(tr!("Profil « {} » supprimé", removed.name));
            }
        }
    }

    /// Coût, graphique et tableau ; renvoie l'échéance double-cliquée.
    fn results(
        &mut self,
        ui: &mut egui::Ui,
        width: f32,
        height: f32,
        pal: &Palette,
    ) -> Option<AmortizationRow> {
        let Some(loan) = self.loan else {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(icon::HOUSE_LINE).size(44.0).color(pal.muted));
                ui.label(
                    RichText::new(tr!(
                        "Complétez trois valeurs puis cliquez sur = en face de celle à calculer."
                    ))
                    .color(pal.muted),
                );
            });
            return None;
        };
        let rows = loan.amortization(self.start_year, self.start_month);
        // Synthèse.
        let gap = 14.0;
        let tile = (width - 3.0 * gap) / 4.0;
        let cost = loan.cost();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            stat(
                ui,
                tile,
                tr!("Mensualité"),
                &money(loan.payment),
                pal.accent,
                pal,
            );
            stat(ui, tile, tr!("Durée"), &duration(loan.months), pal.fg, pal);
            stat(
                ui,
                tile,
                tr!("Coût total"),
                &cost.map(|c| money(c.total_paid)).unwrap_or_default(),
                pal.fg,
                pal,
            );
            stat(
                ui,
                tile,
                tr!("Intérêts"),
                &cost.map(|c| money(c.interest)).unwrap_or_default(),
                pal.warning,
                pal,
            );
        });
        ui.add_space(10.0);
        if let Some(c) = cost {
            // Part du capital et des intérêts.
            let (rect, _) = ui.allocate_exact_size(vec2(width, 10.0), egui::Sense::hover());
            let split =
                rect.left() + rect.width() * (c.capital_percent / 100.0).clamp(0.0, 1.0) as f32;
            ui.painter().rect_filled(rect, 5.0, pal.warning);
            ui.painter().rect_filled(
                egui::Rect::from_min_max(rect.min, pos2(split, rect.bottom())),
                5.0,
                pal.accent,
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr!("●  Capital {} %", percent(c.capital_percent)))
                        .color(pal.accent),
                );
                ui.add_space(16.0);
                ui.label(
                    RichText::new(tr!("●  Intérêts {} %", percent(c.interest_percent)))
                        .color(pal.warning),
                );
                if let Some(last) = rows.last() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(tr!(
                                "dernière échéance : {} {}",
                                month_name(last.month),
                                last.year
                            ))
                            .color(pal.muted),
                        );
                    });
                }
            });
        }
        ui.add_space(10.0);
        let chart_h = (height * 0.28).clamp(140.0, 240.0);
        let (chart, _) = ui.allocate_exact_size(vec2(width, chart_h), egui::Sense::hover());
        remaining_chart(ui, chart, &loan, &rows, pal);
        ui.add_space(12.0);
        amortization_table(ui, &rows, width, pal)
    }
}

/// Aires empilées du capital et des intérêts restant à payer.
fn remaining_chart(
    ui: &egui::Ui,
    rect: egui::Rect,
    loan: &Loan,
    rows: &[AmortizationRow],
    pal: &Palette,
) {
    if rows.is_empty() {
        return;
    }
    let painter = ui.painter_at(rect);
    let total_interest: f64 = rows.iter().map(|r| r.interest).sum();
    let mut interest_left = total_interest;
    let mut points = vec![(loan.capital, total_interest)];
    for r in rows {
        interest_left -= r.interest;
        points.push((r.remaining.max(0.0), interest_left.max(0.0)));
    }
    let max = points
        .iter()
        .map(|(c, i)| c + i)
        .fold(0.0, f64::max)
        .max(1.0);
    let plot = egui::Rect::from_min_max(
        pos2(rect.left() + 56.0, rect.top() + 6.0),
        pos2(rect.right() - 6.0, rect.bottom() - 22.0),
    );
    let n = points.len() - 1;
    let x = |k: usize| plot.left() + plot.width() * k as f32 / n.max(1) as f32;
    let y = |v: f64| plot.bottom() - (v / max) as f32 * plot.height();
    for k in 0..=4 {
        let v = max * k as f64 / 4.0;
        painter.hline(
            plot.x_range(),
            y(v),
            Stroke::new(1.0, pal.border.gamma_multiply(0.5)),
        );
        painter.text(
            pos2(plot.left() - 8.0, y(v)),
            egui::Align2::RIGHT_CENTER,
            short(v),
            egui::FontId::proportional(11.5),
            pal.muted,
        );
    }
    let area = |lower: &dyn Fn(usize) -> f64, upper: &dyn Fn(usize) -> f64, color: Color32| {
        let mut mesh = egui::Mesh::default();
        for k in 0..=n {
            mesh.colored_vertex(pos2(x(k), y(upper(k))), color);
            mesh.colored_vertex(pos2(x(k), y(lower(k))), color.gamma_multiply(0.55));
        }
        for k in 0..n as u32 {
            let i = k * 2;
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i + 1, i + 3, i + 2);
        }
        painter.add(mesh);
    };
    area(&|_| 0.0, &|k| points[k].0, pal.accent.gamma_multiply(0.55));
    area(
        &|k| points[k].0,
        &|k| points[k].0 + points[k].1,
        pal.warning.gamma_multiply(0.55),
    );
    // Traits anticrénelés sur les bords des aires (un maillage ne l'est pas).
    let capital_line: Vec<egui::Pos2> = (0..=n).map(|k| pos2(x(k), y(points[k].0))).collect();
    painter.add(egui::Shape::line(
        capital_line,
        Stroke::new(1.5, pal.accent),
    ));
    let line: Vec<egui::Pos2> = (0..=n)
        .map(|k| pos2(x(k), y(points[k].0 + points[k].1)))
        .collect();
    painter.add(egui::Shape::line(line, Stroke::new(1.5, pal.warning)));
    // Années sous l'axe.
    let years: Vec<(usize, i32)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.month == 1)
        .map(|(k, r)| (k + 1, r.year))
        .collect();
    let every = (years.len() / 8).max(1);
    for (k, year) in years.iter().step_by(every) {
        painter.text(
            pos2(x(*k), plot.bottom() + 6.0),
            egui::Align2::CENTER_TOP,
            year.to_string(),
            egui::FontId::proportional(11.5),
            pal.muted,
        );
    }
    // Survol.
    if let Some(p) = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| plot.contains(*p))
    {
        let k = (((p.x - plot.left()) / plot.width()) * n as f32)
            .round()
            .clamp(0.0, n as f32) as usize;
        painter.vline(
            x(k),
            plot.y_range(),
            Stroke::new(1.0, pal.muted.gamma_multiply(0.6)),
        );
        let (c, i) = points[k];
        let text = if k == 0 {
            tr!("Au départ : capital {} · intérêts {}", money(c), money(i))
        } else {
            tr!(
                "Après l'échéance {} : capital {} · intérêts {}",
                k,
                money(c),
                money(i)
            )
        };
        painter.text(
            pos2(plot.left() + 8.0, plot.top() + 4.0),
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::proportional(12.5),
            pal.fg,
        );
    }
}

fn amortization_table(
    ui: &mut egui::Ui,
    rows: &[AmortizationRow],
    width: f32,
    pal: &Palette,
) -> Option<AmortizationRow> {
    const ROW: f32 = 28.0;
    let cols = [70.0, 150.0];
    let amount_w = ((width - 16.0 - cols[0] - cols[1]) / 4.0).max(90.0);
    let header = |ui: &mut egui::Ui, w: f32, align: egui::Align, t: &str| {
        ui_kit::cell(
            ui,
            w,
            align,
            RichText::new(t)
                .family(crate::theme::semibold())
                .color(pal.fg_dark),
        );
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        header(ui, cols[0], egui::Align::Max, tr!("N°"));
        ui.add_space(16.0);
        header(
            ui,
            cols[1] - 16.0,
            egui::Align::Min,
            trc!("crédit", "Échéance"),
        );
        for t in [
            tr!("Mensualité"),
            tr!("Capital"),
            tr!("Intérêts"),
            tr!("Restant dû"),
        ] {
            header(ui, amount_w, egui::Align::Max, t);
        }
    });
    ui.separator();
    let mut picked = None;
    egui::ScrollArea::vertical()
        .id_salt("amortization")
        .auto_shrink([false, false])
        .show_rows(ui, ROW, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for k in range {
                let r = &rows[k];
                let (rect, response) =
                    ui.allocate_exact_size(vec2(width - 16.0, ROW), egui::Sense::click());
                let response = response.on_hover_text(tr!(
                    "Double-clic : repartir de cette échéance (capital restant, durée restante)"
                ));
                if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, 6.0, pal.bg_highlight.gamma_multiply(0.5));
                } else if k % 2 == 1 {
                    ui.painter()
                        .rect_filled(rect, 6.0, pal.stripe.gamma_multiply(0.7));
                }
                if r.month == 1 && k > 0 {
                    ui.painter().hline(
                        rect.x_range(),
                        rect.top(),
                        Stroke::new(1.0, pal.month_rule.gamma_multiply(0.5)),
                    );
                }
                if response.double_clicked() {
                    picked = Some(*r);
                }
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                let ui = &mut child;
                ui.spacing_mut().item_spacing.x = 0.0;
                ui_kit::cell(
                    ui,
                    cols[0],
                    egui::Align::Max,
                    RichText::new(r.period.to_string()).color(pal.muted),
                );
                ui.add_space(16.0);
                ui_kit::cell(
                    ui,
                    cols[1] - 16.0,
                    egui::Align::Min,
                    RichText::new(format!("{} {}", month_name(r.month), r.year)),
                );
                ui_kit::cell(
                    ui,
                    amount_w,
                    egui::Align::Max,
                    RichText::new(money(r.payment)),
                );
                ui_kit::cell(
                    ui,
                    amount_w,
                    egui::Align::Max,
                    RichText::new(money(r.principal)).color(pal.accent),
                );
                ui_kit::cell(
                    ui,
                    amount_w,
                    egui::Align::Max,
                    RichText::new(money(r.interest)).color(pal.warning),
                );
                ui_kit::cell(
                    ui,
                    amount_w,
                    egui::Align::Max,
                    RichText::new(money(r.remaining)).family(crate::theme::medium()),
                );
            }
        });
    picked
}

fn stat(ui: &mut egui::Ui, width: f32, title: &str, value: &str, color: Color32, pal: &Palette) {
    ui_kit::card(ui, width, |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).color(pal.muted));
            ui.add(
                egui::Label::new(
                    RichText::new(value)
                        .size(20.0)
                        .family(crate::theme::semibold())
                        .color(color),
                )
                .truncate(),
            );
        });
    });
}

fn export_rows(loan: &Loan, year: i32, month: u32) -> Vec<Vec<Cell>> {
    let mut rows = vec![
        vec![
            Cell::Bold(trc!("montant emprunté", "Capital").into()),
            Cell::Money(loan.capital),
        ],
        vec![
            Cell::Bold(tr!("Taux annuel (%)").into()),
            Cell::Number(loan.annual_rate),
        ],
        vec![
            Cell::Bold(tr!("Mensualités").into()),
            Cell::Number(loan.months),
        ],
        vec![
            Cell::Bold(tr!("Mensualité").into()),
            Cell::Money(loan.payment),
        ],
        vec![
            Cell::Bold(tr!("Valeur résiduelle").into()),
            Cell::Money(loan.residual),
        ],
    ];
    if let Some(c) = loan.cost() {
        rows.push(vec![
            Cell::Bold(tr!("Coût total").into()),
            Cell::Money(c.total_paid),
        ]);
        rows.push(vec![
            Cell::Bold(tr!("Intérêts").into()),
            Cell::Money(c.interest),
        ]);
    }
    rows.push(vec![Cell::Empty]);
    rows.push(
        [
            tr!("N°"),
            trc!("crédit", "Échéance"),
            tr!("Mensualité"),
            tr!("Capital"),
            tr!("Intérêts"),
            tr!("Restant dû"),
        ]
        .map(|t| Cell::Bold(t.into()))
        .into_iter()
        .collect(),
    );
    for r in loan.amortization(year, month) {
        rows.push(vec![
            Cell::Number(r.period as f64),
            Cell::Text(format!("{:02}/{}", r.month, r.year)),
            Cell::Money(r.payment),
            Cell::Money(r.principal),
            Cell::Money(r.interest),
            Cell::Money(r.remaining),
        ]);
    }
    rows
}

// ─────────────────────────── Aides ───────────────────────────

fn index(field: LoanUnknown) -> usize {
    match field {
        LoanUnknown::Capital => 0,
        LoanUnknown::Rate => 1,
        LoanUnknown::Months => 2,
        LoanUnknown::Payment => 3,
    }
}

fn label(field: LoanUnknown) -> &'static str {
    match field {
        LoanUnknown::Capital => trc!("montant emprunté", "Capital"),
        LoanUnknown::Rate => tr!("Taux annuel %"),
        LoanUnknown::Months => tr!("Mensualités"),
        LoanUnknown::Payment => tr!("Mensualité"),
    }
}

fn hint(field: LoanUnknown) -> String {
    match field {
        LoanUnknown::Capital => crate::format::count(150_000),
        LoanUnknown::Rate => format!("3{}5", i18n::decimal_separator()),
        LoanUnknown::Months => tr!("nombre : 240 = 20 ans").to_string(),
        LoanUnknown::Payment => "870".to_string(),
    }
}

/// Nombre saisi : espaces, `€` et `%` ignorés, virgule décimale (point en
/// anglais, virgule des milliers acceptée), petits calculs (`20*12`).
fn parse(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '€' | '%' | '\u{202f}' | '\u{a0}'))
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    // La calculatrice lit la virgule et le point selon la langue.
    cleaned
        .parse::<f64>()
        .ok()
        .or_else(|| crate::calculator::evaluate(&cleaned).ok().map(|(v, _)| v))
        .filter(|v| v.is_finite())
}

fn format_value(field: LoanUnknown, v: f64) -> String {
    match field {
        LoanUnknown::Capital | LoanUnknown::Payment => crate::format::money(
            bp_core::model::Amount::from_cents((v * 100.0).round() as i64),
            "",
        ),
        LoanUnknown::Rate => trim_decimals(&format!("{v:.4}")),
        LoanUnknown::Months => trim_decimals(&format!("{v:.2}")),
    }
}

/// `3.5000` → `3,5` (`3.5` en anglais) ; `240.00` → `240`.
fn trim_decimals(text: &str) -> String {
    let t = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    };
    t.replace('.', &i18n::decimal_separator().to_string())
}

/// Pourcentage à une décimale, sans le signe : `72,4`.
fn percent(v: f64) -> String {
    format!("{v:.1}").replace('.', &i18n::decimal_separator().to_string())
}

fn money(v: f64) -> String {
    crate::format::money(
        bp_core::model::Amount::from_cents((v * 100.0).round() as i64),
        "€",
    )
}

fn short(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("{:.1} M", v / 1_000_000.0).replace('.', &i18n::decimal_separator().to_string())
    } else if v >= 1_000.0 {
        format!("{:.0} k", v / 1_000.0)
    } else {
        format!("{v:.0}")
    }
}

fn duration(months: f64) -> String {
    let whole = months.ceil() as i64;
    let (y, m) = (whole / 12, whole % 12);
    let years = match y {
        0 => String::new(),
        _ => trn!(y, "{} an", "{} ans"),
    };
    // « mois » est invariable en français : le singulier anglais a sa clé.
    let months = if m == 1 {
        trc!("singulier", "{} mois", m)
    } else {
        tr!("{} mois", m)
    };
    match (years.is_empty(), m) {
        (true, _) => months,
        (false, 0) => years,
        (false, _) => format!("{years} {months}"),
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

fn dirs_home() -> std::path::PathBuf {
    directories::UserDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_french_numbers() {
        assert_eq!(parse("150 000"), Some(150000.0));
        assert_eq!(parse("3,5 %"), Some(3.5));
        assert_eq!(parse("20*12"), Some(240.0));
        assert_eq!(parse(""), None);
        assert_eq!(trim_decimals("3.5000"), "3,5");
        assert_eq!(trim_decimals("240.00"), "240");
        assert_eq!(duration(240.0), "20 ans");
        assert_eq!(duration(13.0), "1 an 1 mois");
    }

    #[test]
    fn parses_english_numbers() {
        i18n::with_lang(i18n::Lang::En, || {
            assert_eq!(parse("150,000"), Some(150000.0));
            assert_eq!(parse("1,234.56"), Some(1234.56));
            assert_eq!(parse("3.5 %"), Some(3.5));
            assert_eq!(parse("3,5"), Some(3.5));
            assert_eq!(trim_decimals("3.5000"), "3.5");
            assert_eq!(duration(240.0), "20 years");
            assert_eq!(duration(13.0), "1 year 1 month");
            assert_eq!(duration(26.0), "2 years 2 months");
            let form = LoanForm::new();
            assert_eq!(form.value(LoanUnknown::Capital), Some(150000.0));
            assert_eq!(form.value(LoanUnknown::Rate), Some(3.5));
            let loan = form.loan.expect("mensualité calculée");
            assert!((loan.payment - 869.94).abs() < 0.01, "{}", loan.payment);
            assert_eq!(form.value(LoanUnknown::Payment), Some(869.94));
        });
    }

    #[test]
    fn default_simulation_computes_the_payment() {
        let form = LoanForm::new();
        let loan = form.loan.expect("mensualité calculée");
        assert!((loan.payment - 869.94).abs() < 0.01, "{}", loan.payment);
    }
}
