//! Graphiques : soldes des comptes, répartitions (catégories, mois, modes,
//! tranches de montant, jour du mois) et évolution du solde. Dessinés au
//! pinceau : colonnes, barres horizontales, anneau et courbe, avec survol.

use bp_core::reports::{self, AccountScope, BalanceStep, ChartData, ChartUnit};
use egui::{Color32, Pos2, Rect, RichText, Stroke, pos2, vec2};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::reports_ui::VisibleLines;
use crate::spreadsheet::Cell;
use crate::theme::{Palette, palette};
use crate::tools::Outcome;
use crate::ui_kit;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChartKind {
    Balances,
    BalanceOverTime,
    Categories,
    Months,
    Modes,
    DayOfMonth,
    AmountLevels,
}

impl ChartKind {
    const ALL: [ChartKind; 7] = [
        ChartKind::Balances,
        ChartKind::BalanceOverTime,
        ChartKind::Categories,
        ChartKind::Months,
        ChartKind::Modes,
        ChartKind::DayOfMonth,
        ChartKind::AmountLevels,
    ];

    fn label(self) -> &'static str {
        match self {
            ChartKind::Balances => "Soldes des comptes",
            ChartKind::BalanceOverTime => "Évolution du solde",
            ChartKind::Categories => "Par catégorie",
            ChartKind::Months => "Par mois de l'année",
            ChartKind::Modes => "Par mode de paiement",
            ChartKind::DayOfMonth => "Par jour du mois",
            ChartKind::AmountLevels => "Par tranche de montant",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            ChartKind::Balances => "Pointé, à venir et solde de chaque compte",
            ChartKind::BalanceOverTime => "Solde cumulé au fil du temps",
            ChartKind::Categories => "Où va l'argent, d'où il vient",
            ChartKind::Months => "Toutes années confondues",
            ChartKind::Modes => "Carte, chèque, prélèvement…",
            ChartKind::DayOfMonth => "Du 1er au 31",
            ChartKind::AmountLevels => "Nombre d'opérations par montant",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            ChartKind::Balances => icon::BANK,
            ChartKind::BalanceOverTime => icon::CHART_LINE,
            ChartKind::Categories => icon::TAG,
            ChartKind::Months => icon::CALENDAR_BLANK,
            ChartKind::Modes => icon::CREDIT_CARD,
            ChartKind::DayOfMonth => icon::CALENDAR_DOTS,
            ChartKind::AmountLevels => icon::STAIRS,
        }
    }

    /// Le graphique dépend-il des comptes choisis et des filtres ?
    fn uses_lines(self) -> bool {
        self != ChartKind::Balances
    }

    /// Peut-il s'afficher en anneau (répartition) ?
    fn can_donut(self) -> bool {
        matches!(self, ChartKind::Categories | ChartKind::Modes)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Style {
    Bars,
    Donut,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sign {
    Expenses,
    Income,
}

pub struct ChartsForm {
    kind: ChartKind,
    all_accounts: bool,
    filtered: bool,
    zeros: bool,
    detailed: bool,
    all_categories: bool,
    monthly: bool,
    style: Style,
    sign: Sign,
    visible: VisibleLines,
    /// Instant d'apparition du graphique courant (animation).
    shown_at: Option<f64>,
    message: Option<String>,
}

impl ChartsForm {
    pub fn new() -> Self {
        Self {
            kind: ChartKind::Categories,
            all_accounts: true,
            filtered: false,
            zeros: false,
            detailed: true,
            all_categories: false,
            monthly: true,
            style: Style::Donut,
            sign: Sign::Expenses,
            visible: VisibleLines::default(),
            shown_at: None,
            message: None,
        }
    }

    fn data(&mut self, doc: &OpenDoc) -> ChartData {
        let scope = if self.all_accounts {
            AccountScope::All
        } else {
            AccountScope::One(doc.account)
        };
        let filtered = self.filtered;
        let visible = if filtered { self.visible.get(doc) } else { &[] };
        let keep = |account: usize, line: usize, _op: &bp_core::model::Operation| {
            !filtered || visible.get(account).is_some_and(|v| v.contains(&line))
        };
        let d = &doc.document;
        let data = match self.kind {
            ChartKind::Balances => reports::chart_account_balances(d, self.detailed),
            ChartKind::BalanceOverTime => reports::chart_balance_over_time(
                d,
                &scope,
                &keep,
                if self.monthly {
                    BalanceStep::Month
                } else {
                    BalanceStep::Day
                },
            ),
            ChartKind::Categories => {
                reports::chart_categories(d, &scope, &keep, self.all_categories)
            }
            ChartKind::Months => reports::chart_months(d, &scope, &keep),
            ChartKind::Modes => reports::chart_modes(d, &scope, &keep),
            ChartKind::DayOfMonth => reports::chart_day_of_month(d, &scope, &keep),
            ChartKind::AmountLevels => reports::chart_amount_levels(d, &scope, &keep),
        };
        let donut = self.kind.can_donut() && self.style == Style::Donut;
        if (self.zeros && !donut) || self.kind == ChartKind::BalanceOverTime {
            data
        } else {
            data.without_zero_points()
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.85, 900.0, 1700.0);
        let height = ui_kit::dialog_height(ctx, 0.85).max(560.0);
        let now = ctx.input(|i| i.time);
        let t = {
            let start = *self.shown_at.get_or_insert(now);
            let t = ((now - start) / 0.28).clamp(0.0, 1.0) as f32;
            if t < 1.0 {
                ctx.request_repaint();
            }
            1.0 - (1.0 - t).powi(3)
        };
        let before = (
            self.kind,
            self.style,
            self.sign,
            self.all_accounts,
            self.filtered,
            self.detailed,
            self.all_categories,
            self.monthly,
        );
        crate::dialogs::modal(ctx, "charts").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::CHART_BAR, pal.accent, "Graphiques");
            ui.add_space(10.0);
            let body = height - 36.0 - 10.0 - 96.0;
            let list_width = 270.0;
            let gap = 24.0;
            let mut export = None;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.allocate_ui_with_layout(
                    vec2(list_width, body),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_size(vec2(list_width, body));
                        self.kinds(ui, &pal, list_width);
                    },
                );
                let chart_width = width - list_width - gap;
                ui.allocate_ui_with_layout(
                    vec2(chart_width, body),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_size(vec2(chart_width, body));
                        self.options(ui, &pal);
                        ui.add_space(12.0);
                        let data = self.data(doc);
                        let currency = if self.all_accounts {
                            "€".to_string()
                        } else {
                            doc.current_account().currency.clone()
                        };
                        let rect = Rect::from_min_size(
                            ui.cursor().min,
                            vec2(chart_width, ui.available_height().max(200.0)),
                        );
                        self.chart(ui, rect, &data, &currency, &pal, t);
                        export = Some(rows(&data));
                    },
                );
            });
            if let Some(message) = &self.message {
                ui.label(RichText::new(format!("{}  {message}", icon::INFO)).color(pal.accent));
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, "Fermer", true).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    outcome = Outcome::Close;
                }
                if let Some(rows) = export
                    && crate::dialogs::button(
                        ui,
                        format!("{}  Exporter les données…", icon::MICROSOFT_EXCEL_LOGO),
                    )
                    .clicked()
                {
                    let dir = crate::settings::shared::export_dir(
                        doc.path.parent().unwrap_or(std::path::Path::new(".")),
                    );
                    let name = format!("Graphique - {}", self.kind.label());
                    if let Some(message) =
                        crate::spreadsheet::save_dialog(&dir, &name, self.kind.label(), &rows)
                    {
                        self.message = Some(message);
                    }
                }
            });
        });
        let after = (
            self.kind,
            self.style,
            self.sign,
            self.all_accounts,
            self.filtered,
            self.detailed,
            self.all_categories,
            self.monthly,
        );
        if before != after {
            self.shown_at = None;
        }
        outcome
    }

    fn kinds(&mut self, ui: &mut egui::Ui, pal: &Palette, width: f32) {
        for kind in ChartKind::ALL {
            let selected = self.kind == kind;
            let (rect, response) = ui.allocate_exact_size(vec2(width, 58.0), egui::Sense::click());
            let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    selected,
                    kind.label(),
                )
            });
            let fill = if selected {
                pal.accent.gamma_multiply(0.16)
            } else if response.hovered() {
                pal.bg_highlight.gamma_multiply(0.6)
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, 10.0, fill);
            if selected {
                ui.painter().rect_stroke(
                    rect,
                    10.0,
                    Stroke::new(1.0, pal.accent.gamma_multiply(0.6)),
                    egui::StrokeKind::Inside,
                );
            }
            let color = if selected { pal.accent } else { pal.fg_dark };
            ui.painter().text(
                pos2(rect.left() + 22.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                kind.glyph(),
                egui::FontId::proportional(18.0),
                color,
            );
            let text_left = rect.left() + 44.0;
            let title = ui_kit::one_line(
                ui,
                kind.label().to_string(),
                egui::FontId::new(14.5, crate::theme::medium()),
                if selected { pal.fg } else { pal.fg_dark },
                width - 54.0,
            );
            ui.painter()
                .galley(pos2(text_left, rect.top() + 10.0), title, pal.fg);
            let hint = ui_kit::one_line(
                ui,
                kind.hint().to_string(),
                egui::FontId::proportional(12.0),
                pal.muted,
                width - 54.0,
            );
            ui.painter()
                .galley(pos2(text_left, rect.top() + 32.0), hint, pal.muted);
            if response.clicked() {
                self.kind = kind;
            }
            ui.add_space(4.0);
        }
    }

    fn options(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(14.0, 10.0);
            if self.kind.uses_lines() {
                ui_kit::segmented(
                    ui,
                    &mut self.all_accounts,
                    &[
                        (true, "", "Tous les comptes", pal.accent),
                        (false, "", "Compte courant", pal.accent),
                    ],
                    32.0,
                    Some(290.0),
                );
            }
            if self.kind.can_donut() {
                ui_kit::segmented(
                    ui,
                    &mut self.style,
                    &[
                        (Style::Donut, icon::CHART_DONUT, "Anneau", pal.accent),
                        (
                            Style::Bars,
                            icon::CHART_BAR_HORIZONTAL,
                            "Barres",
                            pal.accent,
                        ),
                    ],
                    32.0,
                    Some(230.0),
                );
                if self.style == Style::Donut {
                    ui_kit::segmented(
                        ui,
                        &mut self.sign,
                        &[
                            (Sign::Expenses, "", "Dépenses", pal.negative),
                            (Sign::Income, "", "Recettes", pal.positive),
                        ],
                        32.0,
                        Some(230.0),
                    );
                }
            }
            if self.kind == ChartKind::BalanceOverTime {
                ui_kit::segmented(
                    ui,
                    &mut self.monthly,
                    &[
                        (true, "", "Fin de mois", pal.accent),
                        (false, "", "Chaque jour", pal.accent),
                    ],
                    32.0,
                    Some(250.0),
                );
            }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            if self.kind.uses_lines() {
                crate::theme::checkbox(ui, &mut self.filtered, "Seulement les lignes filtrées");
            }
            match self.kind {
                ChartKind::Balances => {
                    crate::theme::checkbox(ui, &mut self.detailed, "Détailler pointé et à venir");
                }
                ChartKind::Categories => {
                    crate::theme::checkbox(ui, &mut self.all_categories, "Sous-catégories");
                }
                _ => {}
            }
            let donut = self.kind.can_donut() && self.style == Style::Donut;
            if !donut && self.kind != ChartKind::BalanceOverTime {
                crate::theme::checkbox(ui, &mut self.zeros, "Valeurs nulles");
            }
        });
    }

    fn chart(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        data: &ChartData,
        currency: &str,
        pal: &Palette,
        t: f32,
    ) {
        let empty = data.series.iter().all(|s| s.points.is_empty());
        if empty {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Aucune opération à représenter",
                egui::FontId::proportional(15.0),
                pal.muted,
            );
            return;
        }
        let format = |v: f64| match data.unit {
            ChartUnit::Amount => crate::format::money(
                bp_core::model::Amount::from_cents((v * 100.0).round() as i64),
                currency,
            ),
            ChartUnit::Count => format!(
                "{} opération{}",
                v as i64,
                if v.abs() >= 2.0 { "s" } else { "" }
            ),
        };
        match self.kind {
            ChartKind::BalanceOverTime => curve(ui, rect, data, &format, pal, t),
            ChartKind::Categories | ChartKind::Modes if self.style == Style::Donut => {
                let sign = if self.sign == Sign::Expenses {
                    -1.0
                } else {
                    1.0
                };
                let points: Vec<(String, f64)> = data.series[0]
                    .points
                    .iter()
                    .filter(|p| p.value * sign > 0.0)
                    .map(|p| (p.label.clone(), p.value.abs()))
                    .collect();
                donut(ui, rect, &points, &format, pal, t);
            }
            ChartKind::Categories | ChartKind::Modes => {
                horizontal_bars(ui, rect, data, &format, pal, t)
            }
            _ => columns(ui, rect, data, &format, pal, t),
        }
    }
}

// ─────────────────────────── Dessin ───────────────────────────

/// Graduations « rondes » couvrant `[lo, hi]`.
fn ticks(lo: f64, hi: f64, count: usize) -> Vec<f64> {
    let span = (hi - lo).max(1e-9);
    let raw = span / count.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let first = (lo / step).floor() * step;
    let mut out = vec![first];
    // Jusqu'à la première graduation au moins égale au maximum.
    while *out.last().unwrap() < hi - step * 1e-9 {
        let next = first + step * out.len() as f64;
        out.push(next);
    }
    out
}

/// Libellé abrégé d'une graduation : `1,5 k`, `12 k`, `2 M`.
fn short(v: f64) -> String {
    let a = v.abs();
    let text = if a >= 1_000_000.0 {
        format!("{:.1} M", v / 1_000_000.0)
    } else if a >= 1_000.0 {
        format!("{:.1} k", v / 1_000.0)
    } else {
        format!("{v:.0}")
    };
    text.replace(".0 ", " ").replace('.', ",")
}

/// Bulle d'information près du pointeur.
fn tooltip(ui: &egui::Ui, at: Pos2, title: &str, lines: &[(Color32, String)], pal: &Palette) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("chart_tip"),
    ));
    let title = painter.layout_no_wrap(
        title.to_string(),
        egui::FontId::new(13.0, crate::theme::semibold()),
        pal.fg,
    );
    let galleys: Vec<_> = lines
        .iter()
        .map(|(c, text)| {
            (
                *c,
                painter.layout_no_wrap(text.clone(), egui::FontId::proportional(13.0), pal.fg_dark),
            )
        })
        .collect();
    let w = galleys
        .iter()
        .map(|(_, g)| g.size().x + 18.0)
        .fold(title.size().x, f32::max)
        + 24.0;
    let h =
        12.0 + title.size().y + galleys.iter().map(|(_, g)| g.size().y + 4.0).sum::<f32>() + 12.0;
    let screen = ui.ctx().content_rect();
    let mut min = at + vec2(16.0, 16.0);
    if min.x + w > screen.right() - 8.0 {
        min.x = at.x - w - 16.0;
    }
    if min.y + h > screen.bottom() - 8.0 {
        min.y = at.y - h - 16.0;
    }
    let rect = Rect::from_min_size(min, vec2(w, h));
    painter.add(egui::epaint::RectShape::filled(
        rect.translate(vec2(0.0, 3.0)),
        10.0,
        Color32::from_black_alpha(40),
    ));
    painter.rect(
        rect,
        10.0,
        pal.menu_bg,
        Stroke::new(1.0, pal.border),
        egui::StrokeKind::Inside,
    );
    let mut y = rect.top() + 12.0;
    let th = title.size().y;
    painter.galley(pos2(rect.left() + 12.0, y), title, pal.fg);
    y += th + 4.0;
    for (color, galley) in galleys {
        let gh = galley.size().y;
        painter.circle_filled(pos2(rect.left() + 17.0, y + gh / 2.0), 4.0, color);
        painter.galley(pos2(rect.left() + 30.0, y), galley, pal.fg_dark);
        y += gh + 4.0;
    }
}

/// Légende des séries, en haut à droite.
fn legend(ui: &egui::Ui, rect: Rect, data: &ChartData, pal: &Palette) -> f32 {
    if data.series.len() < 2 {
        return 0.0;
    }
    let mut x = rect.right();
    for (k, s) in data.series.iter().enumerate().rev() {
        let g = ui.painter().layout_no_wrap(
            s.name.clone(),
            egui::FontId::proportional(13.0),
            pal.fg_dark,
        );
        x -= g.size().x;
        ui.painter().galley(pos2(x, rect.top()), g, pal.fg_dark);
        x -= 18.0;
        ui.painter().rect_filled(
            Rect::from_center_size(pos2(x + 6.0, rect.top() + 9.0), vec2(10.0, 10.0)),
            3.0,
            series_color(k, data.series.len(), pal),
        );
        x -= 18.0;
    }
    28.0
}

fn series_color(k: usize, count: usize, pal: &Palette) -> Color32 {
    if count == 3 {
        // Pointé, à venir, solde : mêmes couleurs que les encadrés de solde.
        [pal.pointed, pal.future, pal.positive][k]
    } else {
        crate::theme::chart_color(pal, k)
    }
}

/// Colonnes groupées (une couleur par série), axe des valeurs à gauche.
pub(crate) fn columns(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &ChartData,
    format: &dyn Fn(f64) -> String,
    pal: &Palette,
    t: f32,
) {
    let top = legend(ui, rect, data, pal);
    let n = data
        .series
        .iter()
        .map(|s| s.points.len())
        .max()
        .unwrap_or(0);
    let values = data
        .series
        .iter()
        .flat_map(|s| s.points.iter().map(|p| p.value));
    let (lo, hi) = values.fold((0.0f64, 0.0f64), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let (lo, hi) = if hi - lo < 1e-9 {
        (lo, lo + 1.0)
    } else {
        (lo, hi)
    };
    let marks = ticks(lo, hi, 5);
    let (lo, hi) = (
        marks.first().copied().unwrap_or(lo).min(lo),
        marks.last().copied().unwrap_or(hi).max(hi),
    );
    let axis = 64.0;
    // Libellés du bas : inclinés s'ils ne tiennent pas.
    let labels: Vec<&str> = data.series[0]
        .points
        .iter()
        .map(|p| p.label.as_str())
        .collect();
    let slot_guess = (rect.width() - axis) / n.max(1) as f32;
    let widest = labels
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap(l.to_string(), egui::FontId::proportional(12.5), pal.muted)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    let tilted = widest > slot_guess - 6.0;
    let bottom_space = if tilted {
        (widest * 0.6 + 16.0).min(110.0)
    } else {
        26.0
    };
    let plot = Rect::from_min_max(
        pos2(rect.left() + axis, rect.top() + top + 8.0),
        pos2(rect.right() - 8.0, rect.bottom() - bottom_space),
    );
    let y_of = |v: f64| plot.bottom() - ((v - lo) / (hi - lo)) as f32 * plot.height();
    let painter = ui.painter_at(rect);
    for m in &marks {
        let y = y_of(*m);
        let stroke = if m.abs() < 1e-9 {
            Stroke::new(1.0, pal.muted.gamma_multiply(0.6))
        } else {
            Stroke::new(1.0, pal.border.gamma_multiply(0.6))
        };
        painter.hline(plot.x_range(), y, stroke);
        painter.text(
            pos2(plot.left() - 10.0, y),
            egui::Align2::RIGHT_CENTER,
            short(*m),
            egui::FontId::proportional(12.0),
            pal.muted,
        );
    }
    let slot = plot.width() / n.max(1) as f32;
    let series_count = data.series.len();
    let group = (slot * 0.72).min(26.0 * series_count as f32 + 8.0);
    let bar = group / series_count as f32;
    let zero = y_of(0.0);
    let hover = ui
        .interact(plot, ui.id().with("columns"), egui::Sense::hover())
        .hover_pos();
    let hovered =
        hover.map(|p| (((p.x - plot.left()) / slot).floor() as usize).min(n.saturating_sub(1)));
    let step = if tilted {
        1
    } else {
        ((widest + 12.0) / slot).ceil().max(1.0) as usize
    };
    for i in 0..n {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        if hovered == Some(i) {
            painter.rect_filled(
                Rect::from_center_size(pos2(cx, plot.center().y), vec2(slot, plot.height())),
                6.0,
                pal.bg_highlight.gamma_multiply(0.5),
            );
        }
        for (k, s) in data.series.iter().enumerate() {
            let Some(p) = s.points.get(i) else { continue };
            let x = cx - group / 2.0 + bar * k as f32;
            let y = zero + (y_of(p.value) - zero) * t;
            let r = Rect::from_x_y_ranges(x + 1.5..=x + bar - 1.5, y.min(zero)..=y.max(zero));
            let color = series_color(k, series_count, pal);
            let radius = (bar * 0.25).min(5.0) as u8;
            let corners = if p.value >= 0.0 {
                egui::CornerRadius {
                    nw: radius,
                    ne: radius,
                    sw: 0,
                    se: 0,
                }
            } else {
                egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: radius,
                    se: radius,
                }
            };
            painter.rect_filled(
                r,
                corners,
                if hovered.is_some() && hovered != Some(i) {
                    color.gamma_multiply(0.55)
                } else {
                    color
                },
            );
        }
        if i % step == 0 {
            let label = labels.get(i).copied().unwrap_or("");
            let galley = painter.layout_no_wrap(
                label.to_string(),
                egui::FontId::proportional(12.5),
                pal.muted,
            );
            if tilted {
                let angle = -0.6f32;
                let w = galley.size().x;
                let pos = pos2(
                    cx - w * angle.cos() + 4.0,
                    plot.bottom() + 8.0 - w * angle.sin(),
                );
                painter.add(egui::epaint::TextShape::new(pos, galley, pal.muted).with_angle(angle));
            } else {
                painter.galley(
                    pos2(cx - galley.size().x / 2.0, plot.bottom() + 8.0),
                    galley,
                    pal.muted,
                );
            }
        }
    }
    if let (Some(i), Some(at)) = (hovered, hover) {
        let lines: Vec<(Color32, String)> = data
            .series
            .iter()
            .enumerate()
            .filter_map(|(k, s)| {
                s.points.get(i).map(|p| {
                    let text = if series_count > 1 {
                        format!("{} : {}", s.name, format(p.value))
                    } else {
                        format(p.value)
                    };
                    (series_color(k, series_count, pal), text)
                })
            })
            .collect();
        tooltip(ui, at, labels.get(i).copied().unwrap_or(""), &lines, pal);
    }
}

/// Barres horizontales de part et d'autre de zéro (libellés longs).
pub(crate) fn horizontal_bars(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &ChartData,
    format: &dyn Fn(f64) -> String,
    pal: &Palette,
    t: f32,
) {
    const ROW: f32 = 30.0;
    let points = &data.series[0].points;
    let mut order: Vec<usize> = (0..points.len()).collect();
    order.sort_by(|&a, &b| {
        points[a]
            .value
            .partial_cmp(&points[b].value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (lo, hi) = points.iter().fold((0.0f64, 0.0f64), |(lo, hi), p| {
        (lo.min(p.value), hi.max(p.value))
    });
    let span = (hi - lo).max(1e-9);
    let label_width = (rect.width() * 0.26).clamp(150.0, 260.0);
    let value_width = 110.0;
    let ui_rect = rect;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(ui_rect));
    egui::ScrollArea::vertical()
        .id_salt("hbars")
        .max_height(rect.height())
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            let (area, _) = ui.allocate_exact_size(
                vec2(rect.width() - 16.0, ROW * order.len() as f32),
                egui::Sense::hover(),
            );
            let plot = Rect::from_min_max(
                pos2(area.left() + label_width + value_width, area.top()),
                pos2(area.right() - value_width, area.bottom()),
            );
            let x_of = |v: f64| plot.left() + ((v - lo) / span) as f32 * plot.width();
            let zero = x_of(0.0);
            let painter = ui.painter().clone();
            painter.vline(
                zero,
                plot.y_range(),
                Stroke::new(1.0, pal.muted.gamma_multiply(0.5)),
            );
            let hover = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| area.contains(*p) && ui.clip_rect().contains(*p));
            for (row, &i) in order.iter().enumerate() {
                let p = &points[i];
                let r = Rect::from_min_size(
                    pos2(area.left(), area.top() + row as f32 * ROW),
                    vec2(area.width(), ROW),
                );
                let hovered = hover.is_some_and(|h| r.contains(h));
                if hovered {
                    painter.rect_filled(r, 6.0, pal.bg_highlight.gamma_multiply(0.5));
                }
                let label = ui_kit::one_line(
                    ui,
                    p.label.clone(),
                    egui::FontId::proportional(13.5),
                    pal.fg,
                    label_width - 12.0,
                );
                painter.galley(
                    pos2(r.left() + 4.0, r.center().y - label.size().y / 2.0),
                    label,
                    pal.fg,
                );
                let color = if p.value < 0.0 {
                    pal.negative
                } else {
                    pal.positive
                };
                let x = zero + (x_of(p.value) - zero) * t;
                let bar = Rect::from_x_y_ranges(
                    x.min(zero)..=x.max(zero),
                    r.center().y - 9.0..=r.center().y + 9.0,
                );
                painter.rect_filled(
                    bar,
                    5.0,
                    color.gamma_multiply(if hovered { 1.0 } else { 0.8 }),
                );
                let text = painter.layout_no_wrap(
                    format(p.value),
                    egui::FontId::proportional(12.5),
                    pal.fg_dark,
                );
                let tx = if p.value < 0.0 {
                    bar.left() - 8.0 - text.size().x
                } else {
                    bar.right() + 8.0
                };
                painter.galley(
                    pos2(tx, r.center().y - text.size().y / 2.0),
                    text,
                    pal.fg_dark,
                );
            }
        });
}

/// Anneau de répartition et légende (part, montant, pourcentage).
pub(crate) fn donut(
    ui: &mut egui::Ui,
    rect: Rect,
    points: &[(String, f64)],
    format: &dyn Fn(f64) -> String,
    pal: &Palette,
    t: f32,
) {
    let painter = ui.painter_at(rect);
    if points.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Aucune opération de ce sens",
            egui::FontId::proportional(15.0),
            pal.muted,
        );
        return;
    }
    let mut sorted: Vec<(String, f64)> = points.to_vec();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    // Au-delà de 9 parts, le reste est regroupé.
    if sorted.len() > 10 {
        let rest: f64 = sorted[9..].iter().map(|p| p.1).sum();
        let count = sorted.len() - 9;
        sorted.truncate(9);
        sorted.push((format!("Autres ({count})"), rest));
    }
    let total: f64 = sorted.iter().map(|p| p.1).sum();
    let legend_width = (rect.width() * 0.42).clamp(260.0, 460.0);
    let chart = Rect::from_min_max(rect.min, pos2(rect.right() - legend_width, rect.bottom()));
    let radius = (chart.width().min(chart.height()) / 2.0 - 16.0).max(40.0);
    let inner = radius * 0.62;
    let center = chart.center();
    let hover = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| rect.contains(*p));
    // Part survolée : sur l'anneau ou sur la légende.
    let row_h = 34.0;
    let legend_top = rect.center().y - row_h * sorted.len() as f32 / 2.0;
    let legend_left = rect.right() - legend_width + 16.0;
    let mut hovered = hover.and_then(|p| {
        let d = p - center;
        let dist = d.length();
        if dist < inner || dist > radius + 8.0 {
            return None;
        }
        let mut angle = d.y.atan2(d.x) + std::f32::consts::FRAC_PI_2;
        if angle < 0.0 {
            angle += std::f32::consts::TAU;
        }
        let mut acc = 0.0;
        sorted.iter().position(|(_, v)| {
            acc += (*v / total) as f32 * std::f32::consts::TAU;
            angle <= acc
        })
    });
    if hovered.is_none()
        && let Some(p) = hover
        && p.x >= legend_left - 8.0
    {
        let k = ((p.y - legend_top) / row_h).floor();
        if k >= 0.0 && (k as usize) < sorted.len() {
            hovered = Some(k as usize);
        }
    }
    let sweep = t * std::f32::consts::TAU;
    let mut start = -std::f32::consts::FRAC_PI_2;
    let end_limit = start + sweep;
    for (k, (_, v)) in sorted.iter().enumerate() {
        let angle = (*v / total) as f32 * std::f32::consts::TAU;
        let a0 = start;
        let a1 = (start + angle).min(end_limit);
        start += angle;
        if a1 <= a0 {
            continue;
        }
        let color = crate::theme::chart_color(pal, k);
        let (r_out, r_in) = if hovered == Some(k) {
            (radius + 6.0, inner - 2.0)
        } else {
            (radius, inner)
        };
        let color = if hovered.is_some() && hovered != Some(k) {
            color.gamma_multiply(0.45)
        } else {
            color
        };
        let gap = if sorted.len() > 1 { 0.012 } else { 0.0 };
        let feather = 1.0 / ui.ctx().pixels_per_point();
        let mut mesh = egui::Mesh::default();
        ring_segment(
            &mut mesh,
            center,
            r_in,
            r_out,
            a0 + gap,
            (a1 - gap).max(a0 + gap),
            color,
            feather,
        );
        painter.add(mesh);
    }
    // Centre : total, ou la part survolée.
    let (title, value, share) = match hovered {
        Some(k) => (sorted[k].0.clone(), sorted[k].1, Some(sorted[k].1 / total)),
        None => ("Total".to_string(), total, None),
    };
    let title = ui_kit::one_line(
        ui,
        title,
        egui::FontId::proportional(13.0),
        pal.muted,
        inner * 1.6,
    );
    let value = painter.layout_no_wrap(
        format(value),
        egui::FontId::new(20.0, crate::theme::semibold()),
        pal.fg,
    );
    let vh = value.size().y;
    painter.galley(
        pos2(
            center.x - title.size().x / 2.0,
            center.y - vh / 2.0 - title.size().y - 2.0,
        ),
        title,
        pal.muted,
    );
    painter.galley(
        pos2(center.x - value.size().x / 2.0, center.y - vh / 2.0),
        value,
        pal.fg,
    );
    if let Some(share) = share {
        painter.text(
            pos2(center.x, center.y + vh / 2.0 + 4.0),
            egui::Align2::CENTER_TOP,
            format!("{:.1} %", share * 100.0).replace('.', ","),
            egui::FontId::proportional(13.0),
            pal.muted,
        );
    }
    // Légende.
    for (k, (label, v)) in sorted.iter().enumerate() {
        let y = legend_top + k as f32 * row_h;
        let r = Rect::from_min_size(pos2(legend_left - 8.0, y), vec2(legend_width - 16.0, row_h));
        if hovered == Some(k) {
            painter.rect_filled(r, 6.0, pal.bg_highlight.gamma_multiply(0.5));
        }
        let cy = y + row_h / 2.0;
        painter.rect_filled(
            Rect::from_center_size(pos2(legend_left + 6.0, cy), vec2(12.0, 12.0)),
            4.0,
            crate::theme::chart_color(pal, k),
        );
        let pct = painter.layout_no_wrap(
            format!("{:.0} %", v / total * 100.0),
            egui::FontId::proportional(12.5),
            pal.muted,
        );
        let amount = painter.layout_no_wrap(
            format(*v),
            egui::FontId::new(13.5, crate::theme::medium()),
            pal.fg,
        );
        let right = r.right() - 8.0;
        let pw = pct.size().x;
        let aw = amount.size().x;
        painter.galley(
            pos2(right - 44.0 + (44.0 - pw), cy - pct.size().y / 2.0),
            pct,
            pal.muted,
        );
        painter.galley(
            pos2(right - 52.0 - aw, cy - amount.size().y / 2.0),
            amount,
            pal.fg,
        );
        let name = ui_kit::one_line(
            ui,
            label.clone(),
            egui::FontId::proportional(13.5),
            pal.fg_dark,
            (right - 64.0 - aw) - (legend_left + 22.0),
        );
        painter.galley(
            pos2(legend_left + 22.0, cy - name.size().y / 2.0),
            name,
            pal.fg_dark,
        );
    }
}

/// Part d'anneau entre les angles `a0` et `a1`, bords adoucis : une bande
/// transparente de `feather` (un pixel physique) borde chaque côté, comme
/// pour les formes d'egui (un maillage brut n'est pas anticrénelé).
#[allow(clippy::too_many_arguments)]
fn ring_segment(
    mesh: &mut egui::Mesh,
    center: Pos2,
    r_in: f32,
    r_out: f32,
    a0: f32,
    a1: f32,
    color: Color32,
    feather: f32,
) {
    let clear = Color32::TRANSPARENT;
    let dir = |a: f32| vec2(a.cos(), a.sin());
    // Un segment tous les ~2 points sur l'arc extérieur.
    let steps = ((a1 - a0) * r_out / 2.0).ceil().max(2.0) as u32;
    let base = mesh.vertices.len() as u32;
    for s in 0..=steps {
        let d = dir(a0 + (a1 - a0) * s as f32 / steps as f32);
        mesh.colored_vertex(center + d * (r_in - feather), clear);
        mesh.colored_vertex(center + d * r_in, color);
        mesh.colored_vertex(center + d * r_out, color);
        mesh.colored_vertex(center + d * (r_out + feather), clear);
    }
    for s in 0..steps {
        let i = base + s * 4;
        let j = i + 4;
        for k in 0..3 {
            mesh.add_triangle(i + k, i + k + 1, j + k);
            mesh.add_triangle(i + k + 1, j + k + 1, j + k);
        }
    }
    // Bords radiaux.
    for (a, side) in [(a0, -1.0), (a1, 1.0)] {
        let d = dir(a);
        let t = vec2(-d.y, d.x) * side * feather;
        let b = mesh.vertices.len() as u32;
        mesh.colored_vertex(center + d * r_in, color);
        mesh.colored_vertex(center + d * r_out, color);
        mesh.colored_vertex(center + d * r_in + t, clear);
        mesh.colored_vertex(center + d * r_out + t, clear);
        mesh.add_triangle(b, b + 1, b + 2);
        mesh.add_triangle(b + 1, b + 3, b + 2);
    }
}

/// Courbe du solde avec aire en dégradé et curseur au survol.
pub(crate) fn curve(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &ChartData,
    format: &dyn Fn(f64) -> String,
    pal: &Palette,
    t: f32,
) {
    let points = &data.series[0].points;
    let n = points.len();
    let (mut lo, mut hi) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.value), hi.max(p.value))
    });
    if hi - lo < 1.0 {
        lo -= 1.0;
        hi += 1.0;
    }
    let marks = ticks(lo, hi, 5);
    let (lo, hi) = (
        marks.first().copied().unwrap_or(lo).min(lo),
        marks.last().copied().unwrap_or(hi).max(hi),
    );
    let plot = Rect::from_min_max(
        pos2(rect.left() + 64.0, rect.top() + 8.0),
        pos2(rect.right() - 12.0, rect.bottom() - 28.0),
    );
    let painter = ui.painter_at(rect);
    let y_of = |v: f64| plot.bottom() - ((v - lo) / (hi - lo)) as f32 * plot.height();
    let x_of = |i: usize| {
        if n < 2 {
            plot.center().x
        } else {
            plot.left() + plot.width() * i as f32 / (n - 1) as f32
        }
    };
    for m in &marks {
        let y = y_of(*m);
        let stroke = if m.abs() < 1e-9 {
            Stroke::new(1.0, pal.muted.gamma_multiply(0.6))
        } else {
            Stroke::new(1.0, pal.border.gamma_multiply(0.6))
        };
        painter.hline(plot.x_range(), y, stroke);
        painter.text(
            pos2(plot.left() - 10.0, y),
            egui::Align2::RIGHT_CENTER,
            short(*m),
            egui::FontId::proportional(12.0),
            pal.muted,
        );
    }
    // Libellés de dates, une demi-douzaine au plus.
    let every = (n / 6).max(1);
    for i in (0..n).step_by(every) {
        painter.text(
            pos2(x_of(i), plot.bottom() + 8.0),
            egui::Align2::CENTER_TOP,
            &points[i].label,
            egui::FontId::proportional(12.0),
            pal.muted,
        );
    }
    let shown = ((n as f32 * t).ceil() as usize).clamp(1, n);
    let line: Vec<Pos2> = (0..shown)
        .map(|i| pos2(x_of(i), y_of(points[i].value)))
        .collect();
    let color = pal.accent;
    let base = y_of(lo.max(0.0).min(hi));
    let mut mesh = egui::Mesh::default();
    for p in &line {
        mesh.colored_vertex(*p, color.gamma_multiply(0.28));
        mesh.colored_vertex(pos2(p.x, base), color.gamma_multiply(0.0));
    }
    for i in 0..line.len().saturating_sub(1) as u32 {
        let k = i * 2;
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    painter.add(mesh);
    if line.len() > 1 {
        painter.add(egui::Shape::line(
            line.clone(),
            Stroke::new(7.0, color.gamma_multiply(0.12)),
        ));
        painter.add(egui::Shape::line(line.clone(), Stroke::new(2.0, color)));
    }
    let hover = ui
        .interact(plot, ui.id().with("curve"), egui::Sense::hover())
        .hover_pos();
    if let Some(at) = hover
        && n > 0
    {
        let i = if n < 2 {
            0
        } else {
            (((at.x - plot.left()) / plot.width()) * (n - 1) as f32)
                .round()
                .clamp(0.0, (n - 1) as f32) as usize
        };
        let p = pos2(x_of(i), y_of(points[i].value));
        painter.vline(
            p.x,
            plot.y_range(),
            Stroke::new(1.0, pal.muted.gamma_multiply(0.5)),
        );
        painter.circle(p, 5.0, pal.surface, Stroke::new(2.5, color));
        tooltip(
            ui,
            at,
            &points[i].label,
            &[(color, format(points[i].value))],
            pal,
        );
    }
}

/// Données du graphique pour l'export tableur.
fn rows(data: &ChartData) -> Vec<Vec<Cell>> {
    let mut header = vec![Cell::Bold(String::new())];
    header.extend(data.series.iter().map(|s| Cell::Bold(s.name.clone())));
    let mut rows = vec![header];
    let n = data
        .series
        .iter()
        .map(|s| s.points.len())
        .max()
        .unwrap_or(0);
    for i in 0..n {
        let label = data
            .series
            .iter()
            .find_map(|s| s.points.get(i))
            .map(|p| p.label.clone())
            .unwrap_or_default();
        let mut row = vec![Cell::Text(label)];
        row.extend(
            data.series
                .iter()
                .map(|s| match (s.points.get(i), data.unit) {
                    (Some(p), ChartUnit::Amount) => Cell::Money(p.value),
                    (Some(p), ChartUnit::Count) => Cell::Number(p.value),
                    (None, _) => Cell::Empty,
                }),
        );
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_ticks() {
        assert_eq!(
            ticks(0.0, 100.0, 5),
            vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]
        );
        let t = ticks(-330.0, 1260.0, 5);
        assert!(t.first().unwrap() <= &-330.0 && t.last().unwrap() >= &1260.0);
        assert!(t.contains(&0.0));
        assert_eq!(ticks(0.0, 6376.0, 5).last(), Some(&8000.0));
        assert_eq!(short(1500.0), "1,5 k");
        assert_eq!(short(12000.0), "12 k");
        assert_eq!(short(-250.0), "-250");
    }
}
