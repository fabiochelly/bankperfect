//! Bandeau des soldes : Pointé, À venir, Solde final.
//!
//! Trois blocs à plat (sans cadre), séparés par un trait fin. Chacun suit le
//! même schéma : libellé, un nombre principal, de petits chiffres secondaires
//! avec la tendance, et un mini-graphique. Toutes les tendances portent sur
//! la même période (1M à Tout). Les graphiques filtrent la grille : glisser
//! sur une courbe sélectionne une période, cliquer une barre filtre la semaine.

use bp_core::filter::TackFilter;
use bp_core::model::{Amount, Date};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;

use crate::document::{ChartWindow, OpenDoc};
use crate::format::{money, money_compact};
use crate::theme::{Palette, palette};

const STRIP_HEIGHT: f32 = 104.0;
/// Durée de l'animation des montants quand les données changent.
const COUNT_UP_SECONDS: f64 = 0.28;

pub fn cards(ui: &mut Ui, doc: &mut OpenDoc) {
    let pal = palette(ui);
    let available = ui.available_width();
    let (strip, _) = ui.allocate_exact_size(vec2(available, STRIP_HEIGHT), Sense::hover());
    let gap = 14.0;
    let width = (strip.width() - 2.0 * gap) / 3.0;
    let blocks: Vec<Rect> = (0..3)
        .map(|k| {
            Rect::from_min_size(
                pos2(strip.left() + (width + gap) * k as f32, strip.top()),
                vec2(width, STRIP_HEIGHT),
            )
        })
        .collect();
    let colors = [pal.pointed, pal.future, pal.accent];
    for (block, color) in blocks.iter().zip(colors) {
        callout(ui, *block, color);
    }
    let series = doc.insights().series.clone();
    let window = window_dates(&series, doc.chart_window);
    let inner = |r: Rect| {
        Rect::from_min_max(
            pos2(r.left() + 22.0, r.top() + 12.0),
            pos2(r.right() - 14.0, r.bottom() - 10.0),
        )
    };
    pointed_block(ui, inner(blocks[0]), doc, window);
    pending_block(ui, inner(blocks[1]), doc);
    balance_block(ui, inner(blocks[2]), doc, window);
    ui.add_space(12.0);
}

/// Encadré : fond légèrement teinté de la couleur du bloc et bordure gauche
/// épaisse de cette couleur.
fn callout(ui: &Ui, rect: Rect, color: Color32) {
    let pal = palette(ui);
    let radius = egui::CornerRadius::same(10);
    ui.painter().rect_filled(rect, radius, pal.surface);
    ui.painter()
        .rect_filled(rect, radius, color.gamma_multiply(0.06));
    ui.painter().rect_filled(
        Rect::from_min_size(rect.min, vec2(5.0, rect.height())),
        egui::CornerRadius {
            nw: 10,
            sw: 10,
            ne: 0,
            se: 0,
        },
        color,
    );
}

/// Partie gauche d'un bloc : libellé (avec icône), nombre principal animé et
/// lignes secondaires. Renvoie la largeur occupée.
#[allow(clippy::too_many_arguments)]
fn block_text(
    ui: &Ui,
    rect: Rect,
    glyph: &str,
    label: &str,
    color: Color32,
    value: Amount,
    id: &str,
    currency: &str,
    lines: &[(String, Color32)],
    max_width: f32,
) -> f32 {
    let pal = palette(ui);
    let head = ui.painter().text(
        rect.left_top(),
        Align2::LEFT_TOP,
        glyph,
        FontId::proportional(13.0),
        color,
    );
    let title_galley = truncated(
        ui,
        label,
        FontId::new(12.0, crate::theme::medium()),
        pal.muted,
        max_width - (head.right() - rect.left()) - 6.0,
    );
    let title = Rect::from_min_size(pos2(head.right() + 6.0, rect.top()), title_galley.size());
    ui.painter().galley(title.min, title_galley, pal.muted);
    let shown = animated(ui, ui.id().with(id), value);
    let value_color = if value.is_negative() {
        pal.negative
    } else {
        pal.fg
    };
    // Montant complet, sinon abrégé (k€, M€), sinon en plus petit.
    let mut galley = ui.painter().layout_no_wrap(
        money(shown, currency),
        FontId::new(24.0, crate::theme::semibold()),
        value_color,
    );
    for size in [24.0, 20.0, 17.0] {
        if galley.size().x <= max_width {
            break;
        }
        galley = ui.painter().layout_no_wrap(
            money_compact(shown, currency),
            FontId::new(size, crate::theme::semibold()),
            value_color,
        );
    }
    let value_pos = pos2(rect.left(), title.bottom() + 4.0);
    let mut width = galley.size().x.max(title.right() - rect.left());
    let mut y = value_pos.y + galley.size().y + 3.0;
    ui.painter().galley(value_pos, galley, value_color);
    for (text, color) in lines {
        if y + 13.0 > rect.bottom() + 4.0 {
            break;
        }
        let galley = truncated(ui, text, FontId::proportional(11.5), *color, max_width);
        let size = galley.size();
        ui.painter().galley(pos2(rect.left(), y), galley, *color);
        width = width.max(size.x);
        y += size.y + 1.0;
    }
    width.min(max_width)
}

/// Texte sur une ligne, coupé avec « … » au-delà de `max_width`.
fn truncated(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: max_width.max(10.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// Largeur en dessous de laquelle le mini-graphique d'un bloc est masqué.
const CHART_MIN_BLOCK: f32 = 330.0;

/// Largeur laissée au texte : un peu plus de la moitié si le graphique est
/// affiché, tout le bloc sinon.
fn text_width(rect: Rect) -> f32 {
    if rect.width() >= CHART_MIN_BLOCK {
        rect.width() * 0.56
    } else {
        rect.width()
    }
}

/// Texte de tendance : flèche, variation abrégée et pourcentage.
fn trend_line(
    delta: Amount,
    base: Amount,
    currency: &str,
    period: &str,
    pal: &Palette,
) -> (String, Color32) {
    let (glyph, color) = if delta.is_negative() {
        (icon::TREND_DOWN, pal.negative)
    } else {
        (icon::TREND_UP, pal.positive)
    };
    let sign = if delta.is_negative() { "" } else { "+" };
    let percent = if base.0 != 0 {
        format!(
            "  {sign}{:.1} %",
            delta.0 as f64 / base.0.unsigned_abs() as f64 * 100.0
        )
        .replace('.', ",")
    } else {
        String::new()
    };
    (
        format!(
            "{glyph} {sign}{}{percent} {period}",
            money_compact(delta, currency)
        ),
        color,
    )
}

fn period_label(window: ChartWindow) -> &'static str {
    match window {
        ChartWindow::Month => "sur 1 mois",
        ChartWindow::SixMonths => "sur 6 mois",
        ChartWindow::Year => "sur 1 an",
        ChartWindow::FiveYears => "sur 5 ans",
        ChartWindow::All => "depuis l'origine",
    }
}

// ─────────────────────────────── Pointé ───────────────────────────────

fn pointed_block(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc, window: Option<(Date, Date)>) {
    let pal = palette(ui);
    let currency = doc.current_account().currency.clone();
    let totals = doc.totals();
    let series = doc.insights().pointed_series.clone();
    let mut lines = Vec::new();
    if let Some((start, end)) = window {
        let today = Date::today().min(end);
        let before = balance_at(&series, start);
        lines.push(trend_line(
            balance_at(&series, today) + -before,
            before,
            &currency,
            period_label(doc.chart_window),
            &pal,
        ));
    }
    lines.push(match totals.last_reconciled {
        Some(date) => (
            format!(
                "dont {} rapproché au {date}",
                money_compact(totals.reconciled, &currency)
            ),
            pal.muted,
        ),
        None => ("aucun rapprochement".into(), pal.muted),
    });
    let used = block_text(
        ui,
        rect,
        icon::CHECKS,
        "Pointé",
        pal.pointed,
        totals.pointed,
        "pointed_value",
        &currency,
        &lines,
        text_width(rect),
    );
    if let Some((start, end)) = window
        && rect.width() >= CHART_MIN_BLOCK
    {
        let chart = chart_rect(rect, used);
        if chart.width() > 60.0 {
            balance_chart(
                ui,
                chart,
                doc,
                &series,
                start.min(Date::today()),
                end.min(Date::today()),
                &currency,
                &pal,
                pal.pointed,
                "pointed_chart",
            );
        }
    }
}

// ─────────────────────────────── À venir ───────────────────────────────

fn pending_block(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc) {
    let pal = palette(ui);
    let currency = doc.current_account().currency.clone();
    let totals = doc.totals();
    let insights = doc.insights().clone();
    let count = match totals.pending_count {
        0 => "aucune opération à pointer".to_string(),
        1 => "1 opération non pointée".to_string(),
        n => format!("{} opérations non pointées", crate::format::count(n)),
    };
    let (future_sum, future_count) = insights.future_pending;
    let future = if future_count == 0 {
        "aucune opération future".to_string()
    } else {
        format!(
            "dont {} à venir ({})",
            money_compact(future_sum, &currency),
            crate::format::count(future_count)
        )
    };
    let lines = [(count, pal.fg_dark), (future, pal.muted)];
    let used = block_text(
        ui,
        rect,
        icon::CLOCK,
        "À venir",
        pal.future,
        totals.pending,
        "pending_value",
        &currency,
        &lines,
        text_width(rect),
    );

    // Barres par semaine : 8 passées, celle en cours, 4 à venir.
    let chart = chart_rect(rect, used);
    if rect.width() < CHART_MIN_BLOCK || chart.width() < 60.0 {
        return;
    }
    let weeks = &insights.pending_weeks;
    let max = weeks
        .iter()
        .map(|w| w.1.0.unsigned_abs())
        .max()
        .unwrap_or(0)
        .max(1) as f32;
    let today = Date::today();
    let slot = chart.width() / weeks.len().max(1) as f32;
    ui.painter().hline(
        chart.x_range(),
        chart.bottom(),
        Stroke::new(1.0, pal.border),
    );
    for (i, (monday, sum, n)) in weeks.iter().enumerate() {
        let sunday = Date(monday.0 + 6);
        let x0 = chart.left() + slot * i as f32;
        let column = Rect::from_min_max(pos2(x0, chart.top()), pos2(x0 + slot, chart.bottom()));
        let response = ui.interact(column, ui.id().with(("pending_week", i)), Sense::click());
        let active = doc.filters.tack == TackFilter::NotPointed
            && doc.filters.date_from == Some(*monday)
            && doc.filters.date_to == Some(sunday);
        let h = (chart.height() * sum.0.unsigned_abs() as f32 / max).max(if *n > 0 {
            2.0
        } else {
            0.0
        });
        let bar = Rect::from_min_max(
            pos2(x0 + slot * 0.22, chart.bottom() - h),
            pos2(x0 + slot * 0.78, chart.bottom()),
        );
        let current = (*monday..=sunday).contains(&today);
        let future = *monday > today;
        let color = if future { pal.violet } else { pal.future };
        let fill = if active || response.hovered() || current {
            color
        } else {
            color.gamma_multiply(0.55)
        };
        if response.hovered() {
            ui.painter()
                .rect_filled(column, 3.0, pal.bg_highlight.gamma_multiply(0.6));
        }
        ui.painter().rect_filled(bar, 2.0, fill);
        if response.clicked() {
            if active {
                doc.filters.tack = TackFilter::All;
                doc.filters.date_from = None;
                doc.filters.date_to = None;
            } else {
                doc.filters.tack = TackFilter::NotPointed;
                doc.filters.date_from = Some(*monday);
                doc.filters.date_to = Some(sunday);
            }
        }
        response.on_hover_text(format!(
            "Semaine du {monday} au {sunday}\n{} non pointée(s) : {}\nCliquer pour filtrer",
            crate::format::count(*n),
            money(*sum, &currency)
        ));
    }
}

// ─────────────────────────────── Solde final ───────────────────────────────

fn balance_block(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc, window: Option<(Date, Date)>) {
    let pal = palette(ui);
    let currency = doc.current_account().currency.clone();
    let totals = doc.totals();
    let series = doc.insights().series.clone();
    // Boutons de période, en haut à droite du bloc.
    let chips = Rect::from_min_max(
        pos2(rect.center().x, rect.top() - 3.0),
        pos2(rect.right(), rect.top() + 17.0),
    );
    if rect.width() >= CHART_MIN_BLOCK {
        window_chips(ui, chips, doc, &pal);
    }
    let mut lines = Vec::new();
    if let Some((start, end)) = window {
        let today = Date::today().min(end);
        let before = balance_at(&series, start);
        lines.push(trend_line(
            balance_at(&series, today) + -before,
            before,
            &currency,
            period_label(doc.chart_window),
            &pal,
        ));
        if end > Date::today() {
            lines.push((
                format!(
                    "prévu {} au {end}",
                    money_compact(balance_at(&series, end), &currency)
                ),
                pal.muted,
            ));
        }
    }
    let used = block_text(
        ui,
        rect,
        icon::WALLET,
        "Solde final",
        pal.accent,
        totals.balance,
        "balance_value",
        &currency,
        &lines,
        text_width(rect),
    );
    if let Some((start, end)) = window
        && rect.width() >= CHART_MIN_BLOCK
    {
        let chart = Rect::from_min_max(
            pos2(rect.left() + used + 20.0, rect.top() + 22.0),
            rect.right_bottom(),
        );
        if chart.width() > 60.0 {
            balance_chart(
                ui,
                chart,
                doc,
                &series,
                start,
                end,
                &currency,
                &pal,
                pal.accent,
                "balance_chart",
            );
        }
    }
}

/// Zone du mini-graphique : à droite du texte, sur la hauteur du bloc.
fn chart_rect(rect: Rect, text_width: f32) -> Rect {
    Rect::from_min_max(
        pos2(rect.left() + text_width + 20.0, rect.top() + 8.0),
        rect.right_bottom(),
    )
}

/// Montant animé : quand la valeur change, il « défile » vers la nouvelle
/// valeur en ~0,3 s, sans bloquer l'interface.
fn animated(ui: &Ui, id: egui::Id, target: Amount) -> Amount {
    let now = ui.input(|i| i.time);
    let (from, to, start) = ui
        .data(|d| d.get_temp::<(i64, i64, f64)>(id))
        .unwrap_or((target.0, target.0, 0.0));
    let (from, start) = if to != target.0 {
        // Nouvelle cible : on repart de la valeur actuellement affichée.
        let t = ((now - start) / COUNT_UP_SECONDS).clamp(0.0, 1.0);
        ((from as f64 + (to - from) as f64 * ease(t)) as i64, now)
    } else {
        (from, start)
    };
    ui.data_mut(|d| d.insert_temp(id, (from, target.0, start)));
    let t = ((now - start) / COUNT_UP_SECONDS).clamp(0.0, 1.0);
    if t < 1.0 {
        ui.ctx().request_repaint();
    }
    Amount((from as f64 + (target.0 - from) as f64 * ease(t)).round() as i64)
}

fn ease(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// Boutons de période (1M, 6M, 1A, 5A, Tout), alignés à droite du titre.
fn window_chips(ui: &mut Ui, header: Rect, doc: &mut OpenDoc, pal: &Palette) {
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
            let fill = if selected {
                pal.accent.gamma_multiply(0.22)
            } else {
                pal.bg_highlight
            };
            ui.painter().rect_filled(chip, 6.0, fill);
        }
        let color = if selected { pal.accent } else { pal.muted };
        ui.painter()
            .galley(chip.center() - galley.size() / 2.0, galley, color);
        if response.clicked() {
            doc.chart_window = *window;
        }
    }
}

/// Dates de début et de fin de la fenêtre affichée (la fin inclut les
/// opérations futures : solde prévisionnel).
fn window_dates(series: &[(Date, Amount)], window: ChartWindow) -> Option<(Date, Date)> {
    let first = series.first()?.0;
    let last = series.last()?.0;
    let end = last.max(Date::today());
    let start = match window.days() {
        Some(days) => Date((Date::today().0 - days).max(first.0 - 1)),
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
    color: Color32,
    id: &str,
) {
    let span = (end.0 - start.0).max(1) as f32;
    let samples = (rect.width() / 2.0).max(2.0) as usize;
    let dates: Vec<Date> = (0..samples)
        .map(|i| Date(start.0 + (span * i as f32 / (samples - 1) as f32).round() as i32))
        .collect();
    let values: Vec<f64> = dates
        .iter()
        .map(|d| balance_at(series, *d).as_f64())
        .collect();
    let (mut lo, mut hi) = values
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    if (hi - lo).abs() < 1.0 {
        lo -= 1.0;
        hi += 1.0;
    }
    let pad = (hi - lo) * 0.12;
    let (lo, hi) = (lo - pad, hi + pad);
    let x_of = |i: usize| rect.left() + rect.width() * i as f32 / (samples - 1) as f32;
    let y_of = |v: f64| rect.bottom() - ((v - lo) / (hi - lo)) as f32 * rect.height();
    let x_of_date =
        |d: Date| rect.left() + ((d.0 - start.0) as f32 / span).clamp(0.0, 1.0) * rect.width();
    let date_of_x = |x: f32| {
        Date(start.0 + (((x - rect.left()) / rect.width()).clamp(0.0, 1.0) * span).round() as i32)
    };
    let painter = ui.painter_at(rect.expand2(vec2(4.0, 6.0)));
    let today = Date::today();
    let split = dates.iter().position(|d| *d > today).unwrap_or(samples);

    // Période filtrée.
    let (from, to) = (doc.filters.date_from, doc.filters.date_to);
    if from.is_some() || to.is_some() {
        let a = x_of_date(from.unwrap_or(start));
        let b = x_of_date(to.unwrap_or(end));
        painter.rect_filled(
            Rect::from_x_y_ranges(a..=b, rect.y_range()),
            4.0,
            color.gamma_multiply(0.12),
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

    let points: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| pos2(x_of(i), y_of(v)))
        .collect();

    // Aire en dégradé (plus pâle dans le futur).
    let mut mesh = egui::Mesh::default();
    for (i, p) in points.iter().enumerate() {
        let strength = if i < split { 0.30 } else { 0.12 };
        mesh.colored_vertex(*p, color.gamma_multiply(strength));
        mesh.colored_vertex(pos2(p.x, rect.bottom()), color.gamma_multiply(0.0));
    }
    for i in 0..points.len().saturating_sub(1) as u32 {
        let k = i * 2;
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    painter.add(mesh);

    // Courbe passée : halo puis trait ; courbe future (prévisionnel) en pointillés.
    let past: Vec<Pos2> = points[..split.min(points.len())].to_vec();
    if past.len() > 1 {
        painter.add(egui::Shape::line(
            past.clone(),
            Stroke::new(7.0, color.gamma_multiply(0.12)),
        ));
        painter.add(egui::Shape::line(past.clone(), Stroke::new(2.0, color)));
    }
    if split < points.len() {
        let future: Vec<Pos2> = points[split.saturating_sub(1)..].to_vec();
        painter.add(egui::Shape::dashed_line(
            &future,
            Stroke::new(2.0, color.gamma_multiply(0.7)),
            5.0,
            4.0,
        ));
        if let Some(last) = past.last() {
            painter.circle(*last, 3.5, pal.surface, Stroke::new(2.0, color));
        }
    }

    // Interaction : survol (valeur), glisser (période), clic (effacer).
    let response = ui.interact(rect, ui.id().with(id), Sense::click_and_drag());
    let drag_id = ui.id().with((id, "drag"));
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
                color.gamma_multiply(0.25),
            );
        }
        if response.drag_stopped() {
            ui.data_mut(|d| d.remove::<f32>(drag_id));
            if (p.x - x0).abs() > 4.0 {
                doc.filters.date_from = Some(date_of_x(x0.min(p.x)));
                doc.filters.date_to = Some(date_of_x(x0.max(p.x)));
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
            Stroke::new(1.0, pal.fg.gamma_multiply(0.3)),
        );
        painter.circle(pos2(x, y), 4.5, color, Stroke::new(2.0, pal.surface));
        let future = if date > today {
            "  · prévisionnel"
        } else {
            ""
        };
        let text = format!("{date}  ·  {}{future}", money(value, currency));
        let galley =
            ui.painter()
                .layout_no_wrap(text, egui::TextStyle::Small.resolve(ui.style()), pal.fg);
        let mut tip = Rect::from_min_size(
            pos2(x + 10.0, rect.top() - 4.0),
            galley.size() + vec2(14.0, 8.0),
        );
        if tip.right() > rect.right() {
            tip = tip.translate(vec2(-(tip.width() + 20.0), 0.0));
        }
        ui.painter().rect_filled(tip, 6.0, pal.menu_bg);
        ui.painter()
            .galley(tip.min + vec2(7.0, 4.0), galley, pal.fg);
        response.on_hover_cursor(egui::CursorIcon::Crosshair);
    }
}
