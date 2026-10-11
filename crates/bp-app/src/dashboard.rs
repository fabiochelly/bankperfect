//! Tableau de bord : cartes de synthèse avec mini-graphiques interactifs.
//!
//! - Solde : courbe du solde dans le temps ; glisser sélectionne une période
//!   (filtre de dates), un clic simple efface la période.
//! - Pointage : pointé et rapproché, barre à trois zones ; un clic sur une zone
//!   filtre la grille sur cet état.
//! - À venir : non pointées par mois ; un clic sur un mois filtre ce mois.

use bp_core::filter::TackFilter;
use bp_core::model::{Amount, Date};
use chrono::Datelike;
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;

use crate::document::{ChartWindow, OpenDoc};
use crate::format::money;
use crate::theme::{Palette, palette};

const CARD_HEIGHT: f32 = 172.0;
const CARD_MIN_WIDTH: f32 = 300.0;
const GAP: f32 = 14.0;
const MONTH_INITIALS: [&str; 12] = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];
const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

pub fn cards(ui: &mut Ui, doc: &mut OpenDoc) {
    let available = ui.available_width();
    let columns = (((available + GAP) / (CARD_MIN_WIDTH + GAP)).floor() as usize).clamp(1, 3);
    let width = (available - GAP * (columns - 1) as f32) / columns as f32;
    let cards: [fn(&mut Ui, Rect, &mut OpenDoc); 3] = [balance_card, tack_card, pending_card];
    for row in cards.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for card in row {
                let (rect, _) = ui.allocate_exact_size(vec2(width, CARD_HEIGHT), Sense::hover());
                card(ui, rect, doc);
            }
        });
        ui.add_space(GAP);
    }
}

/// Fond de carte, liseré coloré, pastille et titre. Renvoie la zone de contenu
/// et la zone libre à droite du titre.
fn card_frame(ui: &Ui, rect: Rect, color: Color32, glyph: &str, title: &str) -> (Rect, Rect) {
    let pal = palette(ui);
    let painter = ui.painter();
    painter.rect(
        rect,
        12.0,
        pal.surface,
        Stroke::new(1.0, pal.border),
        egui::StrokeKind::Inside,
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(4.0, rect.height())),
        CornerRadius {
            nw: 12,
            sw: 12,
            ne: 0,
            se: 0,
        },
        color,
    );
    let badge = Rect::from_min_size(
        pos2(rect.left() + 18.0, rect.top() + 14.0),
        vec2(26.0, 26.0),
    );
    painter.rect_filled(badge, 7.0, color.gamma_multiply(0.18));
    painter.text(
        badge.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(15.0),
        color,
    );
    let title_galley = painter.layout_no_wrap(
        title.to_string(),
        FontId::new(13.0, crate::theme::medium()),
        pal.muted,
    );
    let title_pos = pos2(
        badge.right() + 10.0,
        badge.center().y - title_galley.size().y / 2.0,
    );
    let title_right = title_pos.x + title_galley.size().x;
    painter.galley(title_pos, title_galley, pal.muted);
    let header_right = Rect::from_min_max(
        pos2(title_right + 12.0, badge.top()),
        pos2(rect.right() - 14.0, badge.bottom()),
    );
    let content = Rect::from_min_max(
        pos2(rect.left() + 18.0, badge.bottom() + 8.0),
        pos2(rect.right() - 16.0, rect.bottom() - 12.0),
    );
    (content, header_right)
}

fn value_text(ui: &Ui, pos: Pos2, amount: Amount, currency: &str, size: f32) -> Rect {
    let pal = palette(ui);
    let color = if amount.is_negative() {
        pal.negative
    } else {
        pal.fg
    };
    ui.painter().text(
        pos,
        Align2::LEFT_TOP,
        money(amount, currency),
        FontId::new(size, crate::theme::semibold()),
        color,
    )
}

// ─────────────────────────────── Solde ───────────────────────────────

fn balance_card(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc) {
    let pal = palette(ui);
    let (content, header) = card_frame(ui, rect, pal.accent, icon::WALLET, "Solde");
    let currency = doc.current_account().currency.clone();

    // Choix de la fenêtre de temps, à droite du titre.
    let mut x = header.right();
    for window in ChartWindow::ALL.iter().rev() {
        let galley = ui.painter().layout_no_wrap(
            window.label().into(),
            egui::TextStyle::Small.resolve(ui.style()),
            pal.fg,
        );
        let chip = Rect::from_min_size(
            pos2(x - galley.size().x - 14.0, header.center().y - 11.0),
            vec2(galley.size().x + 14.0, 22.0),
        );
        x = chip.left() - 2.0;
        let response = ui.interact(
            chip,
            ui.id().with(("window", window.label())),
            Sense::click(),
        );
        let selected = doc.chart_window == *window;
        if selected || response.hovered() {
            ui.painter().rect_filled(
                chip,
                6.0,
                if selected {
                    pal.selection
                } else {
                    pal.bg_highlight
                },
            );
        }
        let color = if selected { pal.fg } else { pal.muted };
        ui.painter()
            .galley(chip.center() - galley.size() / 2.0, galley, color);
        if response.clicked() {
            doc.chart_window = *window;
        }
    }

    let totals = doc.totals();
    let value = value_text(ui, content.left_top(), totals.balance, &currency, 24.0);

    let series = doc.insights().series.clone();
    let chart = Rect::from_min_max(
        pos2(content.left(), value.bottom() + 10.0),
        content.right_bottom(),
    );
    let window = window_dates(&series, doc.chart_window);

    // Variation sur la fenêtre affichée.
    if let Some((start, end)) = window {
        let delta = balance_at(&series, end) + -balance_at(&series, start);
        let (glyph, color) = if delta.is_negative() {
            (icon::TREND_DOWN, pal.negative)
        } else {
            (icon::TREND_UP, pal.positive)
        };
        let sign = if delta.is_negative() { "" } else { "+" };
        ui.painter().text(
            pos2(value.right() + 12.0, value.center().y + 2.0),
            Align2::LEFT_CENTER,
            format!("{glyph} {sign}{}", money(delta, &currency)),
            egui::TextStyle::Small.resolve(ui.style()),
            color,
        );
        balance_chart(ui, chart, doc, &series, start, end, &currency, &pal);
    }
}

/// Dates de début et de fin de la fenêtre affichée.
fn window_dates(series: &[(Date, Amount)], window: ChartWindow) -> Option<(Date, Date)> {
    let first = series.first()?.0;
    let last = series.last()?.0;
    let end = last.max(Date::today());
    let start = match window.days() {
        Some(days) => Date((end.0 - days).max(first.0 - 1)),
        None => Date(first.0 - 1),
    };
    Some((start, end))
}

/// Solde en fin de journée à une date donnée.
fn balance_at(series: &[(Date, Amount)], date: Date) -> Amount {
    let n = series.partition_point(|(d, _)| *d <= date);
    if n == 0 {
        Amount::ZERO
    } else {
        series[n - 1].1
    }
}

#[allow(clippy::too_many_arguments)]
fn balance_chart(
    ui: &mut Ui,
    rect: Rect,
    doc: &mut OpenDoc,
    series: &[(Date, Amount)],
    start: Date,
    end: Date,
    currency: &str,
    pal: &Palette,
) {
    let span = (end.0 - start.0).max(1) as f32;
    let samples = (rect.width() / 2.0).max(2.0) as usize;
    let values: Vec<f64> = (0..samples)
        .map(|i| {
            let date = Date(start.0 + (span * i as f32 / (samples - 1) as f32).round() as i32);
            balance_at(series, date).as_f64()
        })
        .collect();
    let (mut lo, mut hi) = values
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    if (hi - lo).abs() < 1.0 {
        lo -= 1.0;
        hi += 1.0;
    }
    let pad = (hi - lo) * 0.08;
    let (lo, hi) = (lo - pad, hi + pad);
    let x_of = |i: usize| rect.left() + rect.width() * i as f32 / (samples - 1) as f32;
    let y_of = |v: f64| rect.bottom() - ((v - lo) / (hi - lo)) as f32 * rect.height();
    let x_of_date =
        |d: Date| rect.left() + ((d.0 - start.0) as f32 / span).clamp(0.0, 1.0) * rect.width();
    let date_of_x = |x: f32| {
        Date(start.0 + (((x - rect.left()) / rect.width()).clamp(0.0, 1.0) * span).round() as i32)
    };
    let painter = ui.painter_at(rect.expand(2.0));

    // Période filtrée.
    let (from, to) = (doc.filters.date_from, doc.filters.date_to);
    if from.is_some() || to.is_some() {
        let a = x_of_date(from.unwrap_or(start));
        let b = x_of_date(to.unwrap_or(end));
        painter.rect_filled(
            Rect::from_x_y_ranges(a..=b, rect.y_range()),
            4.0,
            pal.accent.gamma_multiply(0.14),
        );
    }

    // Zéro.
    if lo < 0.0 && hi > 0.0 {
        let y = y_of(0.0);
        painter.add(egui::Shape::dashed_line(
            &[pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, pal.muted.gamma_multiply(0.5)),
            4.0,
            4.0,
        ));
    }

    // Aire en dégradé sous la courbe, puis la courbe.
    let points: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| pos2(x_of(i), y_of(v)))
        .collect();
    let mut mesh = egui::Mesh::default();
    let top = pal.accent.gamma_multiply(0.32);
    let bottom = pal.accent.gamma_multiply(0.0);
    for p in &points {
        mesh.colored_vertex(*p, top);
        mesh.colored_vertex(pos2(p.x, rect.bottom()), bottom);
    }
    for i in 0..points.len().saturating_sub(1) as u32 {
        let k = i * 2;
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    painter.add(mesh);
    painter.add(egui::Shape::line(
        points.clone(),
        Stroke::new(2.0, pal.accent),
    ));

    // Aujourd'hui.
    let today = Date::today();
    if today > start && today < end {
        let x = x_of_date(today);
        painter.vline(
            x,
            rect.y_range(),
            Stroke::new(1.0, pal.future.gamma_multiply(0.6)),
        );
    }

    // Interaction : survol (valeur), glisser (période), clic (effacer).
    let response = ui.interact(rect, ui.id().with("balance_chart"), Sense::click_and_drag());
    let drag_id = ui.id().with("balance_chart_drag");
    if response.drag_started()
        && let Some(p) = response.interact_pointer_pos()
    {
        ui.data_mut(|d| d.insert_temp(drag_id, p.x));
    }
    let drag_from: Option<f32> = ui.data(|d| d.get_temp(drag_id));
    if let (Some(x0), Some(p)) = (
        drag_from,
        response.interact_pointer_pos().or(response.hover_pos()),
    ) {
        if response.dragged() {
            let (a, b) = (x0.min(p.x), x0.max(p.x));
            painter.rect_filled(
                Rect::from_x_y_ranges(a..=b, rect.y_range()),
                4.0,
                pal.accent.gamma_multiply(0.25),
            );
        }
        if response.drag_stopped() {
            ui.data_mut(|d| d.remove::<f32>(drag_id));
            if (p.x - x0).abs() > 4.0 {
                let (a, b) = (date_of_x(x0.min(p.x)), date_of_x(x0.max(p.x)));
                doc.filters.date_from = Some(a);
                doc.filters.date_to = Some(b);
            }
        }
    }
    if response.clicked() {
        doc.filters.date_from = None;
        doc.filters.date_to = None;
    }
    if let Some(p) = response.hover_pos() {
        let date = date_of_x(p.x);
        let value = balance_at(series, date);
        let x = x_of_date(date);
        let y = y_of(value.as_f64());
        painter.vline(
            x,
            rect.y_range(),
            Stroke::new(1.0, pal.fg.gamma_multiply(0.35)),
        );
        painter.circle(pos2(x, y), 4.0, pal.accent, Stroke::new(2.0, pal.surface));
        let text = format!("{date}  ·  {}", money(value, currency));
        let font = egui::TextStyle::Small.resolve(ui.style());
        let galley = ui.painter().layout_no_wrap(text, font, pal.fg);
        let mut tip = Rect::from_min_size(
            pos2(x + 8.0, rect.top() - 2.0),
            galley.size() + vec2(12.0, 6.0),
        );
        if tip.right() > rect.right() {
            tip = tip.translate(vec2(-(tip.width() + 16.0), 0.0));
        }
        ui.painter().rect_filled(tip, 5.0, pal.menu_bg);
        ui.painter()
            .galley(tip.min + vec2(6.0, 3.0), galley, pal.fg);
        response.on_hover_cursor(egui::CursorIcon::Crosshair);
    }
}

// ────────────────────────────── Pointage ─────────────────────────────

fn tack_card(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc) {
    let pal = palette(ui);
    let (content, header) = card_frame(ui, rect, pal.pointed, icon::CHECKS, "Pointage");
    let currency = doc.current_account().currency.clone();
    let totals = doc.totals();
    let insights = doc.insights().clone();

    // Rapproché au …, à droite du titre.
    if let Some(date) = totals.last_reconciled {
        ui.painter().text(
            header.right_center(),
            Align2::RIGHT_CENTER,
            format!("rapproché au {date}"),
            egui::TextStyle::Small.resolve(ui.style()),
            pal.muted,
        );
    }

    // Deux montants côte à côte.
    let half = content.width() / 2.0;
    let small = egui::TextStyle::Small.resolve(ui.style());
    ui.painter().text(
        content.left_top(),
        Align2::LEFT_TOP,
        "Pointé",
        small.clone(),
        pal.muted,
    );
    value_text(
        ui,
        content.left_top() + vec2(0.0, 16.0),
        totals.pointed,
        &currency,
        20.0,
    );
    ui.painter().text(
        content.left_top() + vec2(half, 0.0),
        Align2::LEFT_TOP,
        "Rapproché",
        small.clone(),
        pal.muted,
    );
    value_text(
        ui,
        content.left_top() + vec2(half, 16.0),
        totals.reconciled,
        &currency,
        20.0,
    );

    // Barre à trois zones, proportionnelle au nombre d'opérations.
    let bar = Rect::from_min_size(
        pos2(content.left(), content.top() + 56.0),
        vec2(content.width(), 14.0),
    );
    let zones = [
        (TackFilter::NotPointed, pal.future, "non pointées"),
        (TackFilter::Pointed, pal.pointed, "pointées"),
        (TackFilter::Reconciled, pal.reconciled, "rapprochées"),
    ];
    let total: usize = insights.tack_counts.iter().sum::<usize>().max(1);
    ui.painter().rect_filled(bar, 7.0, pal.bg_highlight);
    let mut x = bar.left();
    let visible: Vec<usize> = (0..3).filter(|&k| insights.tack_counts[k] > 0).collect();
    for (pos, &k) in visible.iter().enumerate() {
        let (filter, color, label) = zones[k];
        let w = (bar.width() * insights.tack_counts[k] as f32 / total as f32).max(4.0);
        let zone = Rect::from_min_max(
            pos2(x, bar.top()),
            pos2((x + w).min(bar.right()), bar.bottom()),
        );
        x += w;
        let radius = CornerRadius {
            nw: if pos == 0 { 7 } else { 0 },
            sw: if pos == 0 { 7 } else { 0 },
            ne: if pos + 1 == visible.len() { 7 } else { 0 },
            se: if pos + 1 == visible.len() { 7 } else { 0 },
        };
        let response = ui.interact(
            zone.expand2(vec2(0.0, 4.0)),
            ui.id().with(("tack_zone", k)),
            Sense::click(),
        );
        let active = doc.filters.tack == filter;
        let dimmed = doc.filters.tack != TackFilter::All && !active;
        let fill = if dimmed {
            color.gamma_multiply(0.3)
        } else if response.hovered() {
            color
        } else {
            color.gamma_multiply(0.85)
        };
        ui.painter().rect_filled(zone, radius, fill);
        if response.clicked() {
            doc.filters.tack = if active { TackFilter::All } else { filter };
        }
        response.on_hover_text(format!(
            "{} {label}\n{}\nCliquer pour filtrer",
            crate::format::count(insights.tack_counts[k]),
            money(insights.tack_sums[k], &currency)
        ));
    }

    // Légende cliquable.
    let mut lx = content.left();
    let ly = bar.bottom() + 16.0;
    for (k, (filter, color, label)) in zones.iter().enumerate() {
        let text = format!("{} {label}", crate::format::count(insights.tack_counts[k]));
        let galley = ui
            .painter()
            .layout_no_wrap(text, small.clone(), pal.fg_dark);
        let item = Rect::from_min_size(pos2(lx, ly - 9.0), vec2(galley.size().x + 16.0, 18.0));
        let response = ui.interact(item, ui.id().with(("tack_legend", k)), Sense::click());
        let active = doc.filters.tack == *filter;
        ui.painter().circle_filled(pos2(lx + 4.0, ly), 4.0, *color);
        let text_color = if active || response.hovered() {
            pal.fg
        } else {
            pal.muted
        };
        ui.painter().galley(
            pos2(lx + 12.0, ly - galley.size().y / 2.0),
            galley.clone(),
            text_color,
        );
        if response.clicked() {
            doc.filters.tack = if active { TackFilter::All } else { *filter };
        }
        lx = item.right() + 10.0;
    }
}

// ────────────────────────────── À venir ──────────────────────────────

fn pending_card(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc) {
    let pal = palette(ui);
    let (content, header) = card_frame(ui, rect, pal.future, icon::CLOCK, "À venir");
    let currency = doc.current_account().currency.clone();
    let totals = doc.totals();
    let insights = doc.insights().clone();

    let small = egui::TextStyle::Small.resolve(ui.style());
    let count = match totals.pending_count {
        0 => "aucune opération".to_string(),
        1 => "1 opération non pointée".to_string(),
        n => format!("{} opérations non pointées", crate::format::count(n)),
    };
    ui.painter().text(
        header.right_center(),
        Align2::RIGHT_CENTER,
        count,
        small.clone(),
        pal.muted,
    );
    let value = value_text(ui, content.left_top(), totals.pending, &currency, 24.0);

    // Barres par mois (valeur absolue des non pointées).
    let chart = Rect::from_min_max(
        pos2(content.left(), value.bottom() + 10.0),
        pos2(content.right(), content.bottom() - 16.0),
    );
    let months = &insights.pending_months;
    let max = months
        .iter()
        .map(|m| m.total.0.unsigned_abs())
        .max()
        .unwrap_or(0)
        .max(1) as f32;
    let now = Date::today().to_naive();
    let slot = chart.width() / months.len().max(1) as f32;
    for (i, m) in months.iter().enumerate() {
        let x0 = chart.left() + slot * i as f32;
        let column = Rect::from_min_max(
            pos2(x0, chart.top()),
            pos2(x0 + slot, chart.bottom() + 16.0),
        );
        let response = ui.interact(column, ui.id().with(("pending_month", i)), Sense::click());
        let bounds = month_bounds(m.year, m.month);
        let active = doc.filters.tack == TackFilter::NotPointed
            && doc.filters.date_from == Some(bounds.0)
            && doc.filters.date_to == Some(bounds.1);
        let is_current = m.year == now.year() && m.month == now.month();
        let h = (chart.height() * m.total.0.unsigned_abs() as f32 / max).max(if m.count > 0 {
            3.0
        } else {
            0.0
        });
        let bar = Rect::from_min_max(
            pos2(x0 + slot * 0.18, chart.bottom() - h),
            pos2(x0 + slot * 0.82, chart.bottom()),
        );
        let future = (m.year, m.month) > (now.year(), now.month());
        let base = if future { pal.violet } else { pal.future };
        let fill = if active || response.hovered() {
            base
        } else {
            base.gamma_multiply(0.7)
        };
        if response.hovered() {
            ui.painter().rect_filled(
                column.shrink2(vec2(2.0, 0.0)),
                6.0,
                pal.bg_highlight.gamma_multiply(0.6),
            );
        }
        ui.painter().rect_filled(
            bar,
            CornerRadius {
                nw: 4,
                ne: 4,
                sw: 1,
                se: 1,
            },
            fill,
        );
        let label_color = if is_current || active {
            pal.fg
        } else {
            pal.muted
        };
        let font = if is_current {
            FontId::new(11.5, crate::theme::semibold())
        } else {
            small.clone()
        };
        ui.painter().text(
            pos2(column.center().x, chart.bottom() + 9.0),
            Align2::CENTER_CENTER,
            MONTH_INITIALS[m.month as usize - 1],
            font,
            label_color,
        );
        if response.clicked() {
            if active {
                doc.filters.tack = TackFilter::All;
                doc.filters.date_from = None;
                doc.filters.date_to = None;
            } else {
                doc.filters.tack = TackFilter::NotPointed;
                doc.filters.date_from = Some(bounds.0);
                doc.filters.date_to = Some(bounds.1);
            }
        }
        response.on_hover_text(format!(
            "{} {} : {} non pointée(s)\n{}\nCliquer pour filtrer",
            MONTHS[m.month as usize - 1],
            m.year,
            m.count,
            money(m.total, &currency)
        ));
    }
}

fn month_bounds(year: i32, month: u32) -> (Date, Date) {
    let first = Date::from_ymd(year, month, 1).expect("mois valide");
    let next = if month == 12 {
        Date::from_ymd(year + 1, 1, 1)
    } else {
        Date::from_ymd(year, month + 1, 1)
    };
    (first, Date(next.expect("mois valide").0 - 1))
}
