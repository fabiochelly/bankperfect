//! Bandeau des soldes : « Pointé + À venir = Solde final ».
//!
//! Trois chiffres seulement, reliés par leur relation, lisibles d'un coup
//! d'œil. Chacun porte son contexte (barre rapproché/pointé, nombre
//! d'opérations, variation et courbe du solde) et filtre la grille au clic.
//! La courbe du solde montre le prévisionnel en pointillés après aujourd'hui ;
//! glisser dessus sélectionne une période.

use std::f32::consts::TAU;

use bp_core::filter::TackFilter;
use bp_core::model::{Amount, Date};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use egui_phosphor::regular as icon;

use crate::document::{ChartWindow, OpenDoc};
use crate::format::{money, money_compact};
use crate::theme::{Palette, palette};

const STRIP_HEIGHT: f32 = 98.0;
const OPERATOR_WIDTH: f32 = 34.0;
/// Durée de l'animation des montants quand les données changent.
const COUNT_UP_SECONDS: f64 = 0.28;

pub fn cards(ui: &mut Ui, doc: &mut OpenDoc) {
    let available = ui.available_width();
    let (strip, _) = ui.allocate_exact_size(vec2(available, STRIP_HEIGHT), Sense::hover());
    let solde_width = (available * 0.46)
        .clamp(320.0, 760.0)
        .min(available - 2.0 * OPERATOR_WIDTH - 2.0 * 180.0);
    let small_width = (available - solde_width - 2.0 * OPERATOR_WIDTH) / 2.0;
    let pointed = Rect::from_min_size(strip.min, vec2(small_width, STRIP_HEIGHT));
    let pending = Rect::from_min_size(
        pos2(pointed.right() + OPERATOR_WIDTH, strip.top()),
        vec2(small_width, STRIP_HEIGHT),
    );
    let solde = Rect::from_min_max(
        pos2(pending.right() + OPERATOR_WIDTH, strip.top()),
        strip.max,
    );
    let pal = palette(ui);
    for (x, sign) in [(pointed.right(), "+"), (pending.right(), "=")] {
        ui.painter().text(
            pos2(x + OPERATOR_WIDTH / 2.0, strip.center().y),
            Align2::CENTER_CENTER,
            sign,
            FontId::new(26.0, egui::FontFamily::Proportional),
            pal.muted.gamma_multiply(0.8),
        );
    }
    pointed_tile(ui, pointed, doc);
    pending_tile(ui, pending, doc);
    balance_tile(ui, solde, doc);
    ui.add_space(14.0);
}

/// Fond d'une tuile ; `hero` ajoute un halo et une bordure d'accent.
fn tile(ui: &Ui, rect: Rect, color: Color32, hero: bool) {
    let pal = palette(ui);
    let stroke = if hero {
        Stroke::new(1.0, color.gamma_multiply(0.45))
    } else {
        Stroke::new(1.0, pal.border)
    };
    ui.painter()
        .rect(rect, 14.0, pal.surface, stroke, egui::StrokeKind::Inside);
    let glow = ui.painter_at(rect.shrink(1.0));
    let (radius, strength) = if hero {
        (rect.height() * 2.2, 0.20)
    } else {
        (rect.height() * 1.4, 0.10)
    };
    radial_glow(
        &glow,
        rect.left_top() + vec2(28.0, 20.0),
        radius,
        color.gamma_multiply(strength),
    );
    ui.painter().hline(
        rect.x_range().shrink(14.0),
        rect.top() + 0.5,
        Stroke::new(1.0, Color32::from_white_alpha(14)),
    );
}

/// Libellé de tuile avec son icône colorée ; renvoie la position sous le libellé.
fn tile_label(ui: &Ui, pos: Pos2, glyph: &str, text: &str, color: Color32) -> f32 {
    let pal = palette(ui);
    let icon_rect = ui.painter().text(
        pos,
        Align2::LEFT_TOP,
        glyph,
        FontId::proportional(14.0),
        color,
    );
    let label = ui.painter().text(
        pos2(icon_rect.right() + 6.0, pos.y),
        Align2::LEFT_TOP,
        text,
        FontId::new(12.5, crate::theme::medium()),
        pal.fg_dark,
    );
    label.bottom().max(icon_rect.bottom())
}

fn tile_value(
    ui: &Ui,
    pos: Pos2,
    amount: Amount,
    currency: &str,
    size: f32,
    max_width: f32,
) -> Rect {
    let pal = palette(ui);
    let color = if amount.is_negative() {
        pal.negative
    } else {
        pal.fg
    };
    let full = money(amount, currency);
    let font = FontId::new(size, crate::theme::semibold());
    let galley = ui.painter().layout_no_wrap(full, font.clone(), color);
    // Trop large : montant abrégé plutôt que tronqué.
    let galley = if galley.size().x > max_width {
        ui.painter()
            .layout_no_wrap(money_compact(amount, currency), font, color)
    } else {
        galley
    };
    let rect = Rect::from_min_size(pos, galley.size());
    ui.painter().galley(pos, galley, color);
    rect
}

// ─────────────────────────────── Pointé ───────────────────────────────

fn pointed_tile(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc) {
    let pal = palette(ui);
    tile(ui, rect, pal.pointed, false);
    let currency = doc.current_account().currency.clone();
    let totals = doc.totals();
    let insights = doc.insights().clone();
    let inner = rect.shrink2(vec2(16.0, 12.0));
    let y = tile_label(ui, inner.left_top(), icon::CHECKS, "Pointé", pal.pointed);
    let shown = animated(ui, ui.id().with("pointed_value"), totals.pointed);
    let value = tile_value(
        ui,
        pos2(inner.left(), y + 4.0),
        shown,
        &currency,
        23.0,
        inner.width(),
    );

    // Barre rapproché / pointé (en nombre d'opérations), cliquable.
    let bar = Rect::from_min_size(
        pos2(inner.left(), value.bottom() + 8.0),
        vec2(inner.width(), 6.0),
    );
    let reconciled = insights.tack_counts[2];
    let pointed_only = insights.tack_counts[1];
    let total = (reconciled + pointed_only).max(1);
    let split = bar.left() + bar.width() * reconciled as f32 / total as f32;
    let zones = [
        (
            Rect::from_min_max(bar.min, pos2(split, bar.bottom())),
            TackFilter::Reconciled,
            pal.reconciled,
            "rapprochées",
            reconciled,
            insights.tack_sums[2],
        ),
        (
            Rect::from_min_max(pos2(split, bar.top()), bar.max),
            TackFilter::Pointed,
            pal.pointed,
            "pointées (non rapprochées)",
            pointed_only,
            insights.tack_sums[1],
        ),
    ];
    ui.painter().rect_filled(bar, 3.0, pal.bg_highlight);
    for (k, (zone, filter, color, label, count, sum)) in zones.into_iter().enumerate() {
        if zone.width() <= 0.0 {
            continue;
        }
        let response = ui.interact(
            zone.expand2(vec2(0.0, 6.0)),
            ui.id().with(("pointed_zone", k)),
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
        ui.painter().rect_filled(zone, 3.0, fill);
        if response.clicked() {
            doc.filters.tack = if active { TackFilter::All } else { filter };
        }
        response.on_hover_text(format!(
            "{} {label}\n{}\nCliquer pour filtrer",
            crate::format::count(count),
            money(sum, &currency)
        ));
    }
    let detail = match totals.last_reconciled {
        Some(date) => format!(
            "dont {} rapproché au {date}",
            money_compact(totals.reconciled, &currency)
        ),
        None => "aucun rapprochement".into(),
    };
    ui.painter().text(
        pos2(inner.left(), bar.bottom() + 5.0),
        Align2::LEFT_TOP,
        detail,
        FontId::proportional(11.0),
        pal.muted,
    );
}

// ─────────────────────────────── À venir ───────────────────────────────

fn pending_tile(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc) {
    let pal = palette(ui);
    let active = doc.filters.tack == TackFilter::NotPointed;
    let response = ui.interact(rect, ui.id().with("pending_tile"), Sense::click());
    tile(ui, rect, pal.future, active);
    if response.hovered() {
        ui.painter().rect_stroke(
            rect,
            14.0,
            Stroke::new(1.0, pal.future.gamma_multiply(0.6)),
            egui::StrokeKind::Inside,
        );
    }
    let currency = doc.current_account().currency.clone();
    let totals = doc.totals();
    let inner = rect.shrink2(vec2(16.0, 12.0));
    let y = tile_label(ui, inner.left_top(), icon::CLOCK, "À venir", pal.future);
    let shown = animated(ui, ui.id().with("pending_value"), totals.pending);
    let value = tile_value(
        ui,
        pos2(inner.left(), y + 4.0),
        shown,
        &currency,
        23.0,
        inner.width(),
    );
    let count = match totals.pending_count {
        0 => "aucune opération à pointer".to_string(),
        1 => "1 opération non pointée".to_string(),
        n => format!("{} opérations non pointées", crate::format::count(n)),
    };
    ui.painter().text(
        pos2(inner.left(), value.bottom() + 8.0),
        Align2::LEFT_TOP,
        count,
        FontId::proportional(11.5),
        pal.fg_dark,
    );
    let hint = if active {
        "Cliquer pour tout afficher"
    } else {
        "Cliquer pour n'afficher que les opérations non pointées"
    };
    let response = response.on_hover_text(hint);
    if response.clicked() {
        doc.filters.tack = if active {
            TackFilter::All
        } else {
            TackFilter::NotPointed
        };
    }
}

// ─────────────────────────────── Solde final ───────────────────────────────

fn balance_tile(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc) {
    let pal = palette(ui);
    tile(ui, rect, pal.accent, true);
    let currency = doc.current_account().currency.clone();
    let inner = rect.shrink2(vec2(16.0, 12.0));
    let y = tile_label(
        ui,
        inner.left_top(),
        icon::WALLET,
        "Solde final",
        pal.accent,
    );
    let chips = Rect::from_min_max(
        pos2(inner.center().x, inner.top() - 4.0),
        pos2(inner.right(), inner.top() + 18.0),
    );
    window_chips(ui, chips, doc, &pal);

    let totals = doc.totals();
    let shown = animated(ui, ui.id().with("balance_value"), totals.balance);
    let value = tile_value(
        ui,
        pos2(inner.left(), y + 4.0),
        shown,
        &currency,
        27.0,
        inner.width() * 0.5,
    );

    let series = doc.insights().series.clone();
    let Some((start, end)) = window_dates(&series, doc.chart_window) else {
        return;
    };
    let today = Date::today().min(end);
    let before = balance_at(&series, start);
    let delta = balance_at(&series, today) + -before;
    let percent = (before.0 != 0).then(|| delta.0 as f64 / before.0.unsigned_abs() as f64 * 100.0);
    let label = match doc.chart_window {
        ChartWindow::Month => "sur 1 mois",
        ChartWindow::SixMonths => "sur 6 mois",
        ChartWindow::Year => "sur 1 an",
        ChartWindow::FiveYears => "sur 5 ans",
        ChartWindow::All => "depuis l'origine",
    };
    let badge = delta_badge(
        ui,
        pos2(inner.left(), value.bottom() + 6.0),
        delta,
        percent,
        &currency,
        label,
    );

    let chart_left = value.right().max(badge.right()) + 20.0;
    let chart = Rect::from_min_max(pos2(chart_left, inner.top() + 24.0), inner.right_bottom());
    if chart.width() > 80.0 {
        balance_chart(ui, chart, doc, &series, start, end, &currency, &pal);
    }
}

/// Disque dégradé (opaque au centre, transparent au bord).
fn radial_glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color);
    let steps = 48;
    for i in 0..=steps {
        let a = TAU * i as f32 / steps as f32;
        mesh.colored_vertex(
            center + vec2(a.cos(), a.sin()) * radius,
            Color32::TRANSPARENT,
        );
    }
    for i in 1..=steps as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(mesh);
}

/// Pastille de variation : flèche, montant (et pourcentage), couleur selon le
/// signe. Dessinée à partir de `pos` (coin haut gauche) ; renvoie son rectangle.
fn delta_badge(
    ui: &Ui,
    pos: Pos2,
    delta: Amount,
    percent: Option<f64>,
    currency: &str,
    suffix: &str,
) -> Rect {
    let pal = palette(ui);
    let (glyph, color) = if delta.is_negative() {
        (icon::TREND_DOWN, pal.negative)
    } else {
        (icon::TREND_UP, pal.positive)
    };
    let sign = if delta.is_negative() { "" } else { "+" };
    let mut text = format!("{glyph} {sign}{}", money_compact(delta, currency));
    if let Some(p) = percent.filter(|p| p.is_finite()) {
        text.push_str(&format!("  {sign}{p:.1} %").replace('.', ","));
    }
    if !suffix.is_empty() {
        text.push_str("  ");
        text.push_str(suffix);
    }
    let galley =
        ui.painter()
            .layout_no_wrap(text, FontId::new(11.5, crate::theme::medium()), color);
    let rect = Rect::from_min_size(pos, galley.size() + vec2(14.0, 6.0));
    ui.painter()
        .rect_filled(rect, rect.height() / 2.0, color.gamma_multiply(0.15));
    ui.painter()
        .galley(rect.min + vec2(7.0, 3.0), galley, color);
    rect
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
            pal.accent.gamma_multiply(0.12),
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
        mesh.colored_vertex(*p, pal.accent.gamma_multiply(strength));
        mesh.colored_vertex(pos2(p.x, rect.bottom()), pal.accent.gamma_multiply(0.0));
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
            Stroke::new(7.0, pal.accent.gamma_multiply(0.12)),
        ));
        painter.add(egui::Shape::line(
            past.clone(),
            Stroke::new(2.0, pal.accent),
        ));
    }
    if split < points.len() {
        let future: Vec<Pos2> = points[split.saturating_sub(1)..].to_vec();
        painter.add(egui::Shape::dashed_line(
            &future,
            Stroke::new(2.0, pal.accent.gamma_multiply(0.7)),
            5.0,
            4.0,
        ));
        if let Some(last) = past.last() {
            painter.circle(*last, 3.5, pal.surface, Stroke::new(2.0, pal.accent));
        }
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
        painter.circle(pos2(x, y), 4.5, pal.accent, Stroke::new(2.0, pal.surface));
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
