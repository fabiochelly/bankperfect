//! Bandeau des soldes : En banque (pointé), À venir, Solde final.
//!
//! Par défaut, une seule carte qui se lit comme une équation : « En banque ± À
//! venir = Solde final ». Le solde final, en grand, porte la courbe du solde ;
//! une barre de composition court sous les deux premiers termes et une frise
//! place les opérations non pointées autour d'aujourd'hui. Un petit bouton (ou
//! Affichage › Soldes en trois cartes) la sépare en trois cartes, chacune avec
//! son graphique.
//!
//! Les graphiques filtrent la grille : glisser sur une courbe sélectionne une
//! période, cliquer une semaine de la frise filtre ses opérations non
//! pointées. Toutes les tendances portent sur la même période (1M à Tout).

use std::sync::Arc;

use bp_core::filter::TackFilter;
use bp_core::model::{Amount, Date};
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align2, Color32, FontId, Galley, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
    pos2, vec2,
};
use egui_phosphor::regular as icon;

use crate::commands::Command;
use crate::document::{ChartWindow, OpenDoc, Totals};
use crate::format::{money, money_compact};
use crate::theme::{Palette, medium, palette, semibold};

const TILES_HEIGHT: f32 = 148.0;
const CARDS_HEIGHT: f32 = 112.0;
/// Côté du bouton qui bascule entre l'équation et les cartes.
const TOGGLE_SIZE: f32 = 22.0;
/// Durée du fondu quand on bascule de présentation.
const SWITCH_SECONDS: f64 = 0.25;
const RADIUS: u8 = 12;
/// Durée de l'animation des montants quand les données changent.
const COUNT_UP_SECONDS: f64 = 0.28;
/// Largeur en dessous de laquelle un mini-graphique est masqué (petites
/// fenêtres seulement).
const CHART_MIN_BLOCK: f32 = 200.0;
/// Largeur en dessous de laquelle les boutons de période sont masqués.
const CHIPS_MIN_BLOCK: f32 = 280.0;

/// Données du bandeau.
struct Figures {
    currency: String,
    totals: Totals,
    window: Option<(Date, Date)>,
    series: Arc<[(Date, Amount)]>,
    pointed_series: Arc<[(Date, Amount)]>,
    future_pending: (Amount, usize),
}

fn figures(doc: &OpenDoc) -> Figures {
    let insights = doc.insights();
    let series = Arc::clone(&insights.series);
    let mut window = window_dates(&series, doc.chart_window);
    // Sans solde prévisionnel : la fenêtre s'arrête aujourd'hui.
    if !doc.show_forecast
        && let Some((start, end)) = &mut window
    {
        *end = (*end).min(Date::today());
        *start = (*start).min(*end);
    }
    Figures {
        currency: doc.current_account().currency.clone(),
        totals: doc.totals(),
        window,
        series,
        pointed_series: Arc::clone(&insights.pointed_series),
        future_pending: insights.future_pending,
    }
}

/// Dessine le bandeau ; renvoie la commande de bascule si son bouton a été
/// cliqué.
pub fn cards(ui: &mut Ui, doc: &mut OpenDoc) -> Option<Command> {
    // Fondu à l'apparition d'une présentation.
    let id = ui.id().with("balance_layout");
    let now = ui.input(|i| i.time);
    let (shown, since) = ui
        .data(|d| d.get_temp::<(bool, f64)>(id))
        .unwrap_or((doc.balance_cards, f64::MIN));
    let since = if shown == doc.balance_cards {
        since
    } else {
        now
    };
    ui.data_mut(|d| d.insert_temp(id, (doc.balance_cards, since)));
    let t = ((now - since) / SWITCH_SECONDS).clamp(0.0, 1.0);
    if t < 1.0 {
        ui.ctx().request_repaint();
    }
    let toggle = ui
        .scope(|ui| {
            ui.set_opacity(ease(t) as f32);
            if doc.balance_cards {
                three_cards(ui, doc)
            } else {
                equation(ui, doc)
            }
        })
        .inner;
    ui.add_space(crate::islands::GAP);
    toggle.then_some(Command::ToggleBalanceCards)
}

/// Soldes en tuiles (îles) : « En banque », « À venir » et, en grand,
/// « Solde final » avec sa courbe ; les signes de l'équation sont posés à
/// cheval entre les tuiles.
fn equation(ui: &mut Ui, doc: &mut OpenDoc) -> bool {
    let pal = palette(ui);
    let dark = ui.visuals().dark_mode;
    let (area, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), TILES_HEIGHT), Sense::hover());
    let f = figures(doc);
    let gap = crate::islands::GAP;
    let unit = (area.width() - 2.0 * gap) / 4.1;
    let bank = Rect::from_min_size(area.min, vec2(unit, area.height()));
    let pending = bank.translate(vec2(unit + gap, 0.0));
    let hero = Rect::from_min_max(pos2(pending.right() + gap, area.top()), area.max);
    for tile in [bank, pending, hero] {
        crate::islands::paint(ui.painter(), tile, &pal, dark);
    }

    // En banque : montant, tendance, rapprochement, et l'anneau de pointage.
    let inner = bank.shrink2(vec2(16.0, 14.0));
    let head = tile_head(
        ui,
        inner.min,
        icon::CHECKS,
        "En banque",
        pal.accent,
        inner.width(),
    );
    bank_tooltip(ui, head);
    let ring_size = 48.0;
    let ring = Rect::from_min_size(
        pos2(inner.right() - ring_size, inner.top()),
        Vec2::splat(ring_size),
    );
    pointed_ring(ui, ring, doc, &pal);
    let value = animated(ui, ui.id().with("pointed_value"), f.totals.pointed);
    let galley = amount_galley(
        ui,
        value,
        &f.currency,
        23.0,
        pal.fg,
        ring.left() - inner.left() - 8.0,
    );
    let baseline = inner.top() + 62.0;
    paint_on_baseline(ui, inner.left(), baseline, galley);
    let mut lines = Vec::new();
    lines.extend(
        f.window
            .and_then(|(start, end)| pointed_trend(&f, start, end, doc.chart_window, &pal)),
    );
    lines.push(reconciled_line(&f, &pal));
    paint_lines(
        ui,
        pos2(inner.left(), baseline + 10.0),
        &lines,
        inner.width(),
        inner.bottom() + 2.0,
    );

    // À venir : montant sans signe (l'opérateur le porte), nombre de non
    // pointées, frise des semaines autour d'aujourd'hui.
    let inner = pending.shrink2(vec2(16.0, 14.0));
    tile_head(
        ui,
        inner.min,
        icon::CLOCK,
        "À venir",
        pal.future,
        inner.width(),
    );
    let value = animated(ui, ui.id().with("pending_value"), f.totals.pending);
    let galley = amount_galley(
        ui,
        Amount(value.0.abs()),
        &f.currency,
        23.0,
        pal.future,
        inner.width(),
    );
    paint_on_baseline(ui, inner.left(), baseline, galley);
    let count_galley = pending_count(ui, &f, &pal, inner.width());
    paint_on_baseline(ui, inner.left(), baseline + 26.0, count_galley);
    let frise = Rect::from_min_max(pos2(inner.left(), baseline + 38.0), inner.max);
    if frise.height() >= 8.0 {
        pending_timeline(ui, frise, doc, &f.currency, &pal);
    }

    // Solde final : le texte à gauche, la courbe remplit le reste de la
    // tuile jusqu'aux bords et apparaît en fondu juste après le texte.
    let hero_color = if f.totals.balance.is_negative() {
        pal.negative
    } else {
        pal.positive
    };
    let inner = hero.shrink2(vec2(18.0, 14.0));
    let balance = animated(ui, ui.id().with("balance_value"), f.totals.balance);
    let color = if balance.is_negative() {
        pal.negative
    } else {
        pal.fg
    };
    let text_max = inner.width() * 0.6;
    let galley = amount_galley(ui, balance, &f.currency, 34.0, color, text_max);
    let mut lines = Vec::new();
    if let Some((start, end)) = f.window {
        lines.push(balance_trend(&f, start, end, doc.chart_window, &pal));
        if end > Date::today() {
            lines.push(forecast_line(&f, end, &pal));
        }
    }
    let text_width = lines
        .iter()
        .map(|(text, color)| {
            truncated(ui, text, FontId::proportional(11.5), *color, text_max)
                .size()
                .x
        })
        .fold(galley.size().x, f32::max);
    if let Some((start, end)) = f.window {
        let fade_from = inner.left() + text_width - 10.0;
        if hero.right() - fade_from >= 120.0 {
            lush_chart(ui, hero, fade_from, doc, &f, start, end, &pal, dark);
        }
    }
    let toggle = layout_toggle(ui, toggle_rect(inner), false, &pal);
    let chips_left = if inner.width() >= CHIPS_MIN_BLOCK {
        let chips = Rect::from_min_max(
            pos2(inner.center().x, inner.top() - 3.0),
            pos2(inner.right() - TOGGLE_SIZE - 4.0, inner.top() + 15.0),
        );
        window_chips(ui, chips, doc, &pal, hero_color)
    } else {
        inner.right() - TOGGLE_SIZE - 4.0
    };
    tile_head(
        ui,
        inner.min,
        icon::WALLET,
        "Solde final",
        hero_color,
        chips_left - inner.left() - 8.0,
    );
    let hero_baseline = inner.top() + 70.0;
    paint_on_baseline(ui, inner.left(), hero_baseline, galley);
    paint_lines(
        ui,
        pos2(inner.left(), hero_baseline + 9.0),
        &lines,
        text_max,
        hero.bottom() - 4.0,
    );

    for tile in [bank, pending, hero] {
        crate::islands::outline(ui.painter(), tile, &pal, dark);
    }
    // Signes de l'équation, à cheval entre les tuiles.
    let sign = if f.totals.pending.is_negative() {
        "\u{2212}"
    } else {
        "+"
    };
    badge(
        ui,
        pos2(bank.right() + gap / 2.0, area.center().y),
        sign,
        &pal,
        dark,
    );
    badge(
        ui,
        pos2(pending.right() + gap / 2.0, area.center().y),
        "=",
        &pal,
        dark,
    );
    toggle
}

/// En-tête d'une tuile : icône dans une pastille ronde, puis le libellé.
fn tile_head(ui: &Ui, pos: Pos2, glyph: &str, text: &str, color: Color32, max_width: f32) -> Rect {
    let pal = palette(ui);
    let dark = ui.visuals().dark_mode;
    let disc = Rect::from_min_size(pos, Vec2::splat(26.0));
    ui.painter().circle_filled(
        disc.center(),
        13.0,
        if dark {
            Color32::from_white_alpha(14)
        } else {
            Color32::from_black_alpha(10)
        },
    );
    ui.painter().text(
        disc.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(13.0),
        color,
    );
    let galley = truncated(
        ui,
        text,
        FontId::new(12.5, medium()),
        pal.fg_dark,
        max_width - 34.0,
    );
    let label = Rect::from_min_size(
        pos2(disc.right() + 8.0, disc.center().y - galley.size().y / 2.0),
        galley.size(),
    );
    ui.painter().galley(label.min, galley, pal.fg_dark);
    disc.union(label)
}

/// Pastille d'un signe de l'équation.
fn badge(ui: &Ui, center: Pos2, glyph: &str, pal: &Palette, dark: bool) {
    let rect = Rect::from_center_size(center, Vec2::splat(26.0));
    let shadow = egui::Shadow {
        offset: [0, 3],
        blur: 10,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 30 }),
    };
    ui.painter().add(shadow.as_shape(rect, 13));
    let (fill, stroke) = if dark {
        (pal.surface, Color32::from_white_alpha(20))
    } else {
        (crate::islands::fill(pal, dark), pal.border)
    };
    ui.painter()
        .circle(center, 13.0, fill, Stroke::new(1.0, stroke));
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(15.0),
        pal.fg,
    );
}

/// Anneau de pointage : part des opérations du compte pointées ou
/// rapprochées.
fn pointed_ring(ui: &Ui, rect: Rect, doc: &OpenDoc, pal: &Palette) {
    let counts = doc.insights().tack_counts;
    let total: usize = counts.iter().sum();
    if total == 0 {
        return;
    }
    let done = counts[1] + counts[2];
    let ratio = done as f32 / total as f32;
    let dark = ui.visuals().dark_mode;
    let center = rect.center();
    let radius = rect.width() / 2.0 - 3.0;
    let track = if dark {
        Color32::from_white_alpha(18)
    } else {
        Color32::from_black_alpha(16)
    };
    ui.painter()
        .circle_stroke(center, radius, Stroke::new(4.5, track));
    if ratio > 0.0 {
        let n = (64.0 * ratio).ceil().max(2.0) as usize;
        let points: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::TAU * ratio * i as f32 / n as f32;
                center + vec2(a.cos(), a.sin()) * radius
            })
            .collect();
        ui.painter()
            .add(egui::Shape::line(points, Stroke::new(4.5, pal.positive)));
    }
    let percent = (ratio * 100.0).floor() as u32;
    ui.painter().text(
        center - vec2(0.0, 3.0),
        Align2::CENTER_CENTER,
        format!("{percent} %"),
        FontId::new(11.0, semibold()),
        pal.fg,
    );
    ui.painter().text(
        center + vec2(0.0, 8.0),
        Align2::CENTER_CENTER,
        "pointé",
        FontId::proportional(8.0),
        pal.muted,
    );
    ui.interact(rect, ui.id().with("pointed_ring"), Sense::hover())
        .on_hover_text(format!(
            "{} opérations pointées sur {} (dont {} rapprochées)",
            crate::format::count(done),
            crate::format::count(total),
            crate::format::count(counts[2]),
        ));
}

/// « 33 non pointées · 1,3 k€ à venir » : le nombre en grand.
fn pending_count(ui: &Ui, f: &Figures, pal: &Palette, max_width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    let small = TextFormat {
        font_id: FontId::new(12.5, medium()),
        color: pal.fg_dark,
        ..Default::default()
    };
    match f.totals.pending_count {
        0 => job.append("rien à pointer", 0.0, small.clone()),
        n => {
            job.append(
                &crate::format::count(n),
                0.0,
                TextFormat {
                    font_id: FontId::new(18.0, semibold()),
                    color: pal.fg,
                    ..Default::default()
                },
            );
            let label = if n == 1 {
                " non pointée"
            } else {
                " non pointées"
            };
            job.append(label, 0.0, small.clone());
        }
    }
    let (future_sum, future_count) = f.future_pending;
    if future_count > 0 {
        job.append(
            &format!(" · {} à venir", money_compact(future_sum, &f.currency)),
            0.0,
            TextFormat {
                color: pal.muted,
                ..small
            },
        );
    }
    job.wrap = single_line(max_width);
    ui.painter().layout_job(job)
}

/// Courbe du solde final en aire dégradée (violet vers vert, ou rouge sous
/// zéro) qui remplit la tuile à partir de `left` (avec un fondu d'entrée),
/// avec le solde pointé en transparence derrière et les mois (ou années) en
/// bas.
#[allow(clippy::too_many_arguments)]
fn lush_chart(
    ui: &mut Ui,
    tile: Rect,
    left: f32,
    doc: &mut OpenDoc,
    f: &Figures,
    start: Date,
    end: Date,
    pal: &Palette,
    dark: bool,
) {
    use crate::islands::{RADIUS, gradient, rounded_span};
    const FADE: f32 = 90.0;
    let zone = Rect::from_min_max(pos2(left, tile.top()), tile.max);
    let plot = Rect::from_min_max(
        pos2(zone.left(), tile.top() + 42.0),
        pos2(zone.right(), tile.bottom() - 24.0),
    );
    let span = (end.0 - start.0).max(1) as f32;
    let samples = (zone.width() / 2.0).max(2.0) as usize;
    let x_of = |i: usize| zone.left() + zone.width() * i as f32 / (samples - 1) as f32;
    let date_of =
        |i: usize| Date(start.0 + (span * i as f32 / (samples - 1) as f32).round() as i32);
    let x_of_date =
        |d: Date| zone.left() + ((d.0 - start.0) as f32 / span).clamp(0.0, 1.0) * zone.width();
    // Fondu d'entrée, après le texte.
    let fade = |x: f32| {
        let t = ((x - zone.left()) / FADE).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let values: Vec<f64> = (0..samples)
        .map(|i| balance_at(&f.series, date_of(i)).as_f64())
        .collect();
    let pointed: Vec<f64> = (0..samples)
        .map(|i| balance_at(&f.pointed_series, date_of(i)).as_f64())
        .collect();
    let today = Date::today();
    let split = (0..samples)
        .position(|i| date_of(i) > today)
        .unwrap_or(samples);
    let (mut lo, mut hi) = values
        .iter()
        .chain(&pointed[..split])
        .fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    if (hi - lo).abs() < 1.0 {
        lo -= 1.0;
        hi += 1.0;
    }
    let pad = (hi - lo) * 0.1;
    let (lo, hi) = (lo - pad, hi + pad);
    let y_of = move |v: f64| plot.bottom() - ((v - lo) / (hi - lo)) as f32 * plot.height();
    let painter = ui.painter().clone();

    // Une aire = une bande verticale par échantillon, du trait jusqu'au bas
    // arrondi de la tuile.
    let area = |ys: &[f32], color_at: &dyn Fn(usize, f32) -> Color32| {
        let mut mesh = egui::Mesh::default();
        for (i, &y) in ys.iter().enumerate() {
            let x = x_of(i);
            let (top, bottom) = rounded_span(tile, RADIUS, x);
            let t = (x - zone.left()) / zone.width();
            let color = color_at(i, t).gamma_multiply(fade(x));
            mesh.colored_vertex(pos2(x, y.max(top)), color);
            mesh.colored_vertex(pos2(x, bottom), color.gamma_multiply(0.75));
        }
        for i in 0..ys.len().saturating_sub(1) as u32 {
            let k = i * 2;
            mesh.add_triangle(k, k + 1, k + 2);
            mesh.add_triangle(k + 1, k + 3, k + 2);
        }
        painter.add(mesh);
    };
    let future_fade = |i: usize| if i < split { 1.0 } else { 0.55 };

    // Solde pointé (jusqu'à aujourd'hui), en transparence derrière.
    let (b, p) = (crate::theme::logo_b(pal).0, crate::theme::logo_p(pal).1);
    let back: Vec<f32> = pointed[..split].iter().map(|&v| y_of(v)).collect();
    area(&back, &|_, t| {
        gradient(
            &[(0.0, p.gamma_multiply(0.2)), (1.0, b.gamma_multiply(0.26))],
            t,
        )
    });

    // Solde : violet vers vert (ou rouge s'il finit sous zéro).
    let end_color = if f.totals.balance.is_negative() {
        pal.negative
    } else if dark {
        pal.positive.lerp_to_gamma(Color32::WHITE, 0.2)
    } else {
        pal.positive.lerp_to_gamma(Color32::WHITE, 0.3)
    };
    let alpha = if dark { 0.92 } else { 0.85 };
    let stops = [
        (
            0.0,
            Color32::from_rgb(0x6b, 0x4f, 0xd0).gamma_multiply(alpha),
        ),
        (
            0.55,
            Color32::from_rgb(0x9a, 0x7f, 0xe6).gamma_multiply(alpha),
        ),
        (1.0, end_color.gamma_multiply(alpha)),
    ];
    let front: Vec<f32> = values.iter().map(|&v| y_of(v)).collect();
    area(&front, &|i, t| {
        gradient(&stops, t).gamma_multiply(future_fade(i))
    });

    // Période filtrée.
    let (from, to) = (doc.filters.date_from, doc.filters.date_to);
    if from.is_some() || to.is_some() {
        painter.rect_filled(
            Rect::from_x_y_ranges(
                x_of_date(from.unwrap_or(start))..=x_of_date(to.unwrap_or(end)),
                plot.top() - 6.0..=plot.bottom() + 6.0,
            ),
            4.0,
            Color32::from_white_alpha(if dark { 28 } else { 70 }),
        );
    }

    // Trait du solde : plein jusqu'à aujourd'hui, pointillé ensuite ; en
    // fondu au début, comme l'aire.
    let line = Color32::from_white_alpha(if dark { 150 } else { 210 });
    let points: Vec<Pos2> = front
        .iter()
        .enumerate()
        .map(|(i, &y)| pos2(x_of(i), y))
        .collect();
    let past = &points[..split.min(points.len())];
    let faded = past
        .iter()
        .position(|p| p.x >= zone.left() + FADE)
        .unwrap_or(past.len());
    for w in past[..(faded + 1).min(past.len())].windows(2) {
        let color = line.gamma_multiply(fade((w[0].x + w[1].x) / 2.0));
        painter.line_segment([w[0], w[1]], Stroke::new(1.5, color));
    }
    if past.len() > faded + 1 {
        painter.add(egui::Shape::line(
            past[faded..].to_vec(),
            Stroke::new(1.5, line),
        ));
    }
    if split < points.len() {
        painter.add(egui::Shape::dashed_line(
            &points[split.saturating_sub(1)..],
            Stroke::new(1.5, line.gamma_multiply(0.8)),
            4.0,
            4.0,
        ));
        if let Some(last) = past.last() {
            painter.circle(
                *last,
                3.5,
                crate::islands::fill(pal, dark).to_opaque(),
                Stroke::new(2.0, line),
            );
        }
    }

    // Mois (ou semaines, ou années) en bas, une fois la courbe apparue.
    let label_color = if dark {
        Color32::from_white_alpha(150)
    } else {
        pal.fg.gamma_multiply(0.75)
    };
    let mut last_right = zone.left() + FADE * 0.5;
    for (date, text, current) in axis_labels(start, end) {
        let x = x_of_date(date);
        let font = if current {
            FontId::new(10.5, semibold())
        } else {
            FontId::proportional(10.5)
        };
        let color = if current { pal.fg } else { label_color };
        let galley = ui.painter().layout_no_wrap(text, font, color);
        let label_left = x - galley.size().x / 2.0;
        if label_left < last_right + 6.0 || label_left + galley.size().x > zone.right() - 12.0 {
            continue;
        }
        last_right = label_left + galley.size().x;
        painter.galley(
            pos2(label_left, tile.bottom() - 8.0 - galley.size().y),
            galley,
            color,
        );
    }

    // Survol, glisser pour filtrer une période, clic pour l'effacer.
    let hit = Rect::from_min_max(pos2(zone.left(), plot.top() - 14.0), zone.max);
    chart_interaction(
        ui,
        &painter,
        Rect::from_x_y_ranges(zone.x_range(), plot.y_range()),
        hit,
        doc,
        &f.series,
        start,
        end,
        &f.currency,
        pal,
        Color32::WHITE,
        "balance_chart",
        &y_of,
    );
}

/// Repères de l'axe du temps : semaines pour un mois, mois jusqu'à trois
/// ans, années au-delà. Le repère qui contient aujourd'hui est marqué.
fn axis_labels(start: Date, end: Date) -> Vec<(Date, String, bool)> {
    use chrono::Datelike;
    const MONTHS: [&str; 12] = [
        "Jan", "Fév", "Mar", "Avr", "Mai", "Juin", "Juil", "Août", "Sep", "Oct", "Nov", "Déc",
    ];
    let today = Date::today();
    let span = end.0 - start.0;
    let mut out = Vec::new();
    if span <= 62 {
        // Lundis.
        let mut d = start.0 + (7 - start.to_naive().weekday().num_days_from_monday() as i32) % 7;
        while d <= end.0 {
            let date = Date(d);
            let n = date.to_naive();
            let current = today.0 >= d && today.0 < d + 7;
            out.push((
                Date(d + 3),
                format!("{:02}/{:02}", n.day(), n.month()),
                current,
            ));
            d += 7;
        }
    } else if span <= 3 * 366 {
        let s = start.to_naive();
        let (mut y, mut m) = (s.year(), s.month());
        while let Some(first) = Date::from_ymd(y, m, 1)
            && first.0 <= end.0
        {
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            let next = Date::from_ymd(ny, nm, 1)
                .map(|d| d.0)
                .unwrap_or(first.0 + 30);
            let mid = Date((first.0.max(start.0) + next.min(end.0 + 1)) / 2);
            let current = today.0 >= first.0 && today.0 < next;
            out.push((mid, MONTHS[(m - 1) as usize].to_string(), current));
            (y, m) = (ny, nm);
        }
    } else {
        let (first_year, last_year) = (start.to_naive().year(), end.to_naive().year());
        for y in first_year..=last_year {
            let (Some(a), Some(b)) = (Date::from_ymd(y, 1, 1), Date::from_ymd(y + 1, 1, 1)) else {
                continue;
            };
            let mid = Date((a.0.max(start.0) + b.0.min(end.0 + 1)) / 2);
            out.push((mid, y.to_string(), today.0 >= a.0 && today.0 < b.0));
        }
    }
    out
}

// ─────────────────────────────── Les cartes ───────────────────────────────

fn three_cards(ui: &mut Ui, doc: &mut OpenDoc) -> bool {
    let pal = palette(ui);
    let dark = ui.visuals().dark_mode;
    let (strip, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), CARDS_HEIGHT), Sense::hover());
    let f = figures(doc);
    let gap = 14.0;
    let width = (strip.width() - 2.0 * gap) / 3.0;
    let blocks: Vec<Rect> = (0..3)
        .map(|k| {
            Rect::from_min_size(
                pos2(strip.left() + (width + gap) * k as f32, strip.top()),
                vec2(width, CARDS_HEIGHT),
            )
        })
        .collect();
    let balance_color = if f.totals.balance.is_negative() {
        pal.negative
    } else {
        pal.positive
    };
    let colors = [pal.accent, pal.future, balance_color];
    for (k, (block, color)) in blocks.iter().zip(colors).enumerate() {
        let lift = ui
            .ctx()
            .animate_bool(ui.id().with(("lift", k)), ui.rect_contains_pointer(*block));
        card(ui, *block, lift);
        glow(
            &ui.painter_at(block.shrink(1.0)),
            pos2(block.right() - block.width() * 0.12, block.top() + 6.0),
            vec2(block.width() * 0.62, block.height() * 0.95),
            color.gamma_multiply(if dark { 0.15 } else { 0.12 } + 0.08 * lift),
        );
    }
    let inner = |r: Rect| Rect::from_min_max(r.min + vec2(20.0, 14.0), r.max - vec2(16.0, 12.0));
    let text_width = |r: Rect| {
        if r.width() >= CHART_MIN_BLOCK {
            r.width() * 0.56
        } else {
            r.width()
        }
    };

    // En banque (pointé).
    let (block, r) = (blocks[0], inner(blocks[0]));
    bank_tooltip(ui, Rect::from_min_size(r.min, vec2(r.width() * 0.5, 16.0)));
    let mut lines = Vec::new();
    lines.extend(
        f.window
            .and_then(|(start, end)| pointed_trend(&f, start, end, doc.chart_window, &pal)),
    );
    lines.push(reconciled_line(&f, &pal));
    let used = card_text(
        ui,
        r,
        (icon::CHECKS, "En banque", pal.accent),
        f.totals.pointed,
        pal.fg,
        "pointed_value",
        &f.currency,
        &lines,
        text_width(r),
    );
    if let Some((start, end)) = f.window
        && r.width() >= CHART_MIN_BLOCK
    {
        let chart = card_chart(block, r, used);
        if chart.width() > 60.0 {
            balance_chart(
                ui,
                chart,
                block.shrink(1.0),
                doc,
                &f.pointed_series,
                start.min(Date::today()),
                end.min(Date::today()),
                &f.currency,
                &pal,
                pal.accent,
                "pointed_chart",
            );
        }
    }

    // À venir.
    let (block, r) = (blocks[1], inner(blocks[1]));
    let count = match f.totals.pending_count {
        0 => "aucune opération à pointer".to_string(),
        1 => "1 opération non pointée".to_string(),
        n => format!("{} opérations non pointées", crate::format::count(n)),
    };
    let (future_sum, future_count) = f.future_pending;
    let future = if future_count == 0 {
        "aucune opération future".to_string()
    } else {
        format!(
            "dont {} à venir ({})",
            money_compact(future_sum, &f.currency),
            crate::format::count(future_count)
        )
    };
    let used = card_text(
        ui,
        r,
        (icon::CLOCK, "À venir", pal.future),
        f.totals.pending,
        pal.future,
        "pending_value",
        &f.currency,
        &[(count, pal.fg_dark), (future, pal.muted)],
        text_width(r),
    );
    let chart = card_chart(block, r, used);
    if r.width() >= CHART_MIN_BLOCK && chart.width() > 60.0 {
        let frise = Rect::from_center_size(
            pos2(chart.center().x - 6.0, chart.center().y - 4.0),
            vec2(chart.width() - 12.0, 22.0),
        );
        pending_timeline(ui, frise, doc, &f.currency, &pal);
    }

    // Solde final.
    let (block, r) = (blocks[2], inner(blocks[2]));
    let toggle = layout_toggle(ui, toggle_rect(r), true, &pal);
    if r.width() >= CHIPS_MIN_BLOCK {
        let area = Rect::from_min_max(
            pos2(r.center().x, r.top() - 3.0),
            pos2(r.right() - TOGGLE_SIZE - 4.0, r.top() + 15.0),
        );
        window_chips(ui, area, doc, &pal, balance_color);
    }
    let mut lines = Vec::new();
    if let Some((start, end)) = f.window {
        lines.push(balance_trend(&f, start, end, doc.chart_window, &pal));
        if end > Date::today() {
            lines.push(forecast_line(&f, end, &pal));
        }
    }
    let used = card_text(
        ui,
        r,
        (icon::WALLET, "Solde final", balance_color),
        f.totals.balance,
        if f.totals.balance.is_negative() {
            pal.negative
        } else {
            pal.fg
        },
        "balance_value",
        &f.currency,
        &lines,
        text_width(r),
    );
    if let Some((start, end)) = f.window
        && r.width() >= CHART_MIN_BLOCK
    {
        let mut chart = card_chart(block, r, used);
        chart.min.y = chart.min.y.max(r.top() + 22.0);
        if chart.width() > 60.0 {
            balance_chart(
                ui,
                chart,
                block.shrink(1.0),
                doc,
                &f.series,
                start,
                end,
                &f.currency,
                &pal,
                balance_color,
                "balance_chart",
            );
        }
    }
    toggle
}

/// Zone de la courbe d'une carte : à droite du texte, jusqu'aux bords droit
/// et bas de la carte.
fn card_chart(block: Rect, inner: Rect, text_width: f32) -> Rect {
    Rect::from_min_max(
        pos2(inner.left() + text_width + 20.0, inner.top() + 22.0),
        block.right_bottom() - vec2(1.0, 1.0),
    )
}

/// Texte d'une carte : libellé, montant animé et lignes secondaires. Renvoie
/// la largeur occupée.
#[allow(clippy::too_many_arguments)]
fn card_text(
    ui: &Ui,
    rect: Rect,
    (glyph, label, color): (&str, &str, Color32),
    value: Amount,
    value_color: Color32,
    id: &str,
    currency: &str,
    lines: &[(String, Color32)],
    max_width: f32,
) -> f32 {
    let title = caption(ui, rect.min, glyph, label, color, max_width);
    let shown = animated(ui, ui.id().with(id), value);
    let galley = amount_galley(ui, shown, currency, 26.0, value_color, max_width);
    let baseline = title.bottom() + 5.0 + baseline_of(&galley);
    let mut width = galley.size().x.max(title.width());
    paint_on_baseline(ui, rect.left(), baseline, galley);
    let used = paint_lines(
        ui,
        pos2(rect.left(), baseline + 8.0),
        lines,
        max_width,
        rect.bottom() + 4.0,
    );
    width = width.max(used);
    width.min(max_width)
}

// ─────────────────────────────── Éléments communs ───────────────────────────────

/// Fond d'une carte : celui des îles.
fn card_fill(ui: &Ui) -> Color32 {
    crate::islands::fill(&palette(ui), ui.visuals().dark_mode)
}

/// Fond d'une carte, liseré discret et reflet clair sur le bord haut.
/// `lift` (de 0 à 1) ajoute l'ombre portée du survol.
fn card(ui: &Ui, rect: Rect, lift: f32) {
    let pal = palette(ui);
    let dark = ui.visuals().dark_mode;
    let painter = ui.painter();
    if lift > 0.0 {
        let shadow = egui::Shadow {
            offset: [0, 6],
            blur: 22,
            spread: 0,
            color: Color32::from_black_alpha((if dark { 90.0 } else { 40.0 } * lift) as u8),
        };
        painter.add(shadow.as_shape(rect, RADIUS));
    }
    painter.rect(
        rect,
        RADIUS,
        card_fill(ui),
        Stroke::new(1.0, pal.border.gamma_multiply(if dark { 0.8 } else { 1.0 })),
        StrokeKind::Inside,
    );
    let sheen = if dark {
        Color32::from_white_alpha(16)
    } else {
        Color32::from_white_alpha(170)
    };
    let r = RADIUS as f32;
    painter.hline(
        (rect.left() + r)..=(rect.right() - r),
        rect.top() + 1.5,
        Stroke::new(1.0, sheen),
    );
}

/// Infobulle du libellé « En banque » : c'est le solde pointé.
fn bank_tooltip(ui: &Ui, rect: Rect) {
    ui.interact(rect, ui.id().with("bank_caption"), Sense::hover())
        .on_hover_text(
            "Solde pointé : les opérations déjà passées sur le compte bancaire,\n\
             c'est-à-dire ce que la banque affiche aujourd'hui.",
        );
}

/// Emplacement du bouton de bascule : en haut à droite de `area`.
fn toggle_rect(area: Rect) -> Rect {
    Rect::from_min_size(
        pos2(area.right() - TOGGLE_SIZE, area.top() - 4.0),
        Vec2::splat(TOGGLE_SIZE),
    )
}

/// Petit bouton qui sépare l'équation en trois cartes, ou les regroupe.
/// Renvoie vrai s'il a été cliqué.
fn layout_toggle(ui: &Ui, rect: Rect, split: bool, pal: &Palette) -> bool {
    let response = ui.interact(rect, ui.id().with("balance_layout_toggle"), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 6.0, pal.bg_highlight);
    }
    let glyph = if split { icon::EQUALS } else { icon::COLUMNS };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(14.0),
        if hovered { pal.fg } else { pal.muted },
    );
    response
        .on_hover_text(if split {
            "Regrouper en équation"
        } else {
            "Séparer en trois cartes"
        })
        .clicked()
}

/// Halo radial en ellipse : la couleur au centre, transparent au bord.
fn glow(painter: &Painter, center: Pos2, radius: Vec2, color: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color);
    let n = 64u32;
    // Un anneau intermédiaire adoucit la décroissance.
    for (scale, strength) in [(0.45, 0.55), (1.0, 0.0)] {
        for i in 0..n {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            mesh.colored_vertex(
                center + vec2(a.cos() * radius.x, a.sin() * radius.y) * scale,
                color.gamma_multiply(strength),
            );
        }
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.add_triangle(0, 1 + i, 1 + j);
        mesh.add_triangle(1 + i, 1 + n + i, 1 + j);
        mesh.add_triangle(1 + j, 1 + n + i, 1 + n + j);
    }
    painter.add(mesh);
}

/// Libellé en petites capitales espacées, précédé d'une icône colorée.
/// Renvoie le rectangle occupé.
fn caption(ui: &Ui, pos: Pos2, glyph: &str, text: &str, color: Color32, max_width: f32) -> Rect {
    let pal = palette(ui);
    let icon_rect = ui.painter().text(
        pos,
        Align2::LEFT_TOP,
        glyph,
        FontId::proportional(13.0),
        color,
    );
    let mut job = LayoutJob::single_section(
        text.to_uppercase(),
        TextFormat {
            font_id: FontId::new(10.5, medium()),
            color: pal.muted,
            extra_letter_spacing: 0.9,
            ..Default::default()
        },
    );
    job.wrap = single_line(max_width - icon_rect.width() - 7.0);
    let galley = ui.painter().layout_job(job);
    let label = Rect::from_min_size(
        pos2(
            icon_rect.right() + 7.0,
            icon_rect.center().y - galley.size().y / 2.0,
        ),
        galley.size(),
    );
    ui.painter().galley(label.min, galley, pal.muted);
    icon_rect.union(label)
}

/// Montant mis en forme : partie entière en grand, décimales et devise plus
/// petites et atténuées. Abrégé (k€, M€) puis réduit s'il dépasse `max_width`.
fn amount_galley(
    ui: &Ui,
    amount: Amount,
    currency: &str,
    size: f32,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    let text = |compact: bool| {
        let text = if compact {
            money_compact(amount, currency)
        } else {
            money(amount, currency)
        };
        text.replacen('-', "\u{2212}", 1)
    };
    // Montant complet, réduit, puis abrégé.
    let mut galley = ui
        .painter()
        .layout_job(amount_job(&text(false), size, color));
    for (compact, scale) in [(false, 0.85), (true, 1.0), (true, 0.85), (true, 0.72)] {
        if galley.size().x <= max_width {
            break;
        }
        galley = ui
            .painter()
            .layout_job(amount_job(&text(compact), (size * scale).round(), color));
    }
    galley
}

fn amount_job(text: &str, size: f32, color: Color32) -> LayoutJob {
    let (main, unit) = text.split_once('\u{a0}').unwrap_or((text, ""));
    let (int, frac) = main.split_at(main.rfind(',').unwrap_or(main.len()));
    let big = TextFormat {
        font_id: FontId::new(size, semibold()),
        color,
        extra_letter_spacing: -0.02 * size,
        ..Default::default()
    };
    let small = TextFormat {
        font_id: FontId::new((size * 0.6).round(), semibold()),
        color: color.gamma_multiply(0.6),
        ..Default::default()
    };
    let mut job = LayoutJob::default();
    job.append(int, 0.0, big);
    job.append(frac, 0.0, small.clone());
    if !unit.is_empty() {
        job.append(unit, size * 0.14, small);
    }
    job
}

/// Distance du haut du texte à sa ligne de base (premier caractère).
fn baseline_of(galley: &Galley) -> f32 {
    galley
        .rows
        .first()
        .and_then(|row| row.row.glyphs.first().map(|g| row.pos.y + g.pos.y))
        .unwrap_or(galley.size().y * 0.8)
}

fn paint_on_baseline(ui: &Ui, x: f32, baseline: f32, galley: Arc<Galley>) {
    let pos = pos2(x, baseline - baseline_of(&galley));
    ui.painter().galley(pos, galley, Color32::PLACEHOLDER);
}

/// Petites lignes sous un montant, tant qu'elles tiennent au-dessus de
/// `bottom`. Renvoie la largeur occupée.
fn paint_lines(
    ui: &Ui,
    pos: Pos2,
    lines: &[(String, Color32)],
    max_width: f32,
    bottom: f32,
) -> f32 {
    let mut y = pos.y;
    let mut width: f32 = 0.0;
    for (text, color) in lines {
        if y + 13.0 > bottom {
            break;
        }
        let galley = truncated(ui, text, FontId::proportional(11.5), *color, max_width);
        let size = galley.size();
        ui.painter().galley(pos2(pos.x, y), galley, *color);
        width = width.max(size.x);
        y += size.y + 1.0;
    }
    width
}

fn single_line(max_width: f32) -> TextWrapping {
    TextWrapping {
        max_width: max_width.max(10.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    }
}

/// Texte sur une ligne, coupé avec « … » au-delà de `max_width`.
fn truncated(ui: &Ui, text: &str, font: FontId, color: Color32, max_width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = single_line(max_width);
    ui.painter().layout_job(job)
}

/// Texte de tendance : flèche, variation abrégée et pourcentage. Rien si la
/// variation est nulle.
fn trend_line(
    delta: Amount,
    base: Amount,
    currency: &str,
    period: &str,
    pal: &Palette,
) -> Option<(String, Color32)> {
    if delta.0 == 0 {
        return None;
    }
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
    Some((
        format!(
            "{glyph} {sign}{}{percent} {period}",
            money_compact(delta, currency)
        ),
        color,
    ))
}

fn pointed_trend(
    f: &Figures,
    start: Date,
    end: Date,
    window: ChartWindow,
    pal: &Palette,
) -> Option<(String, Color32)> {
    let today = Date::today().min(end);
    let before = balance_at(&f.pointed_series, start);
    trend_line(
        balance_at(&f.pointed_series, today) + -before,
        before,
        &f.currency,
        period_label(window),
        pal,
    )
}

/// Tendance du solde, ou « stable » si rien n'a bougé sur la période.
fn balance_trend(
    f: &Figures,
    start: Date,
    end: Date,
    window: ChartWindow,
    pal: &Palette,
) -> (String, Color32) {
    let today = Date::today().min(end);
    let before = balance_at(&f.series, start);
    trend_line(
        balance_at(&f.series, today) + -before,
        before,
        &f.currency,
        period_label(window),
        pal,
    )
    .unwrap_or_else(|| {
        (
            format!("{} stable {}", icon::ARROW_RIGHT, period_label(window)),
            pal.muted,
        )
    })
}

fn forecast_line(f: &Figures, end: Date, pal: &Palette) -> (String, Color32) {
    (
        format!(
            "prévu {} au {end}",
            money_compact(balance_at(&f.series, end), &f.currency)
        ),
        pal.muted,
    )
}

fn reconciled_line(f: &Figures, pal: &Palette) -> (String, Color32) {
    match f.totals.last_reconciled {
        Some(date) => (
            format!(
                "{} {} rapproché au {date}",
                icon::SEAL_CHECK,
                money_compact(f.totals.reconciled, &f.currency)
            ),
            pal.muted,
        ),
        None => ("aucun rapprochement".into(), pal.muted),
    }
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

/// Boutons de période (1M, 6M, 1A, 5A, Tout), alignés à droite de `area`.
/// Renvoie le bord gauche du premier bouton.
fn window_chips(ui: &mut Ui, area: Rect, doc: &mut OpenDoc, pal: &Palette, color: Color32) -> f32 {
    let mut x = area.right();
    for window in ChartWindow::ALL.iter().rev() {
        let galley = ui.painter().layout_no_wrap(
            window.label().into(),
            FontId::new(11.0, medium()),
            Color32::PLACEHOLDER,
        );
        let chip = Rect::from_min_size(
            pos2(x - galley.size().x - 12.0, area.center().y - 10.0),
            vec2(galley.size().x + 12.0, 20.0),
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
                color.gamma_multiply(0.2)
            } else {
                pal.bg_highlight
            };
            ui.painter().rect_filled(chip, 6.0, fill);
        }
        let text = if selected { color } else { pal.muted };
        ui.painter()
            .galley(chip.center() - galley.size() / 2.0, galley, text);
        if response.clicked() {
            doc.chart_window = *window;
        }
    }
    x
}

/// Frise des opérations non pointées, de 8 semaines avant à 4 semaines après
/// aujourd'hui : un trait par jour, au-dessus de l'axe pour un crédit, en
/// dessous pour un débit. Cliquer une semaine filtre ses opérations non pointées.
fn pending_timeline(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc, currency: &str, pal: &Palette) {
    let weeks = doc.insights().pending_weeks.clone();
    let days = doc.insights().pending_days.clone();
    if weeks.is_empty() || days.is_empty() {
        return;
    }
    let start = weeks[0].0;
    let span = (weeks.len() * 7) as f32;
    let x_of = |d: Date| rect.left() + ((d.0 - start.0) as f32 + 0.5) / span * rect.width();
    let today = Date::today();
    let slot = rect.width() / weeks.len() as f32;
    let column_rect = |i: usize| {
        Rect::from_min_max(
            pos2(rect.left() + slot * i as f32, rect.top() - 2.0),
            pos2(rect.left() + slot * (i + 1) as f32, rect.bottom() + 2.0),
        )
    };
    for (i, (monday, sum, n)) in weeks.iter().enumerate() {
        let sunday = Date(monday.0 + 6);
        let column = column_rect(i);
        let response = ui.interact(column, ui.id().with(("pending_week", i)), Sense::click());
        let active = doc.filters.tack == TackFilter::NotPointed
            && doc.filters.date_from == Some(*monday)
            && doc.filters.date_to == Some(sunday);
        if active {
            ui.painter()
                .rect_filled(column, 4.0, pal.future.gamma_multiply(0.2));
        } else if response.hovered() {
            ui.painter()
                .rect_filled(column, 4.0, pal.bg_highlight.gamma_multiply(0.7));
        }
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
            money(*sum, currency)
        ));
    }
    let painter = ui.painter();
    let y = rect.center().y;
    painter.hline(rect.x_range(), y, Stroke::new(1.0, pal.border));
    let max = days
        .iter()
        .map(|d| d.1.0.unsigned_abs())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    // Un trait par jour : vers le haut pour un crédit, vers le bas pour un
    // débit, d'autant plus long que la somme est importante.
    let reach = rect.height() / 2.0;
    let width = (rect.width() / span * 0.7).clamp(1.0, 2.5);
    for (date, sum, _) in &days {
        let h = 1.5 + (reach - 1.5) * (sum.0.unsigned_abs() as f32 / max).sqrt();
        let color = if *date > today {
            pal.violet
        } else {
            pal.future
        };
        let x = x_of(*date);
        let tip = if sum.is_negative() { y + h } else { y - h };
        painter.line_segment([pos2(x, y), pos2(x, tip)], Stroke::new(width, color));
    }
    // Aujourd'hui.
    painter.vline(
        x_of(today),
        (rect.top() - 2.0)..=(rect.bottom() + 2.0),
        Stroke::new(1.5, pal.fg.gamma_multiply(0.6)),
    );
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

/// Courbe du solde dans `rect`, dessinée sans déborder de `clip`.
#[allow(clippy::too_many_arguments)]
fn balance_chart(
    ui: &mut Ui,
    rect: Rect,
    clip: Rect,
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
    let flat = (hi - lo).abs() < 0.005;
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
    let painter = ui.painter_at(rect.expand2(vec2(4.0, 6.0)).intersect(clip));
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

    if flat {
        // Rien n'a bougé : un trait pointillé discret plutôt qu'un aplat.
        let y = rect.center().y + 6.0;
        painter.add(egui::Shape::dashed_line(
            &[pos2(rect.left(), y), pos2(rect.right() - 8.0, y)],
            Stroke::new(1.5, color.gamma_multiply(0.5)),
            4.0,
            4.0,
        ));
        painter.text(
            pos2(rect.center().x, y - 7.0),
            Align2::CENTER_BOTTOM,
            "aucun mouvement sur la période",
            FontId::proportional(10.5),
            pal.muted,
        );
    } else {
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
                painter.circle(*last, 3.5, card_fill(ui), Stroke::new(2.0, color));
            }
        }
    }

    chart_interaction(
        ui, &painter, rect, rect, doc, series, start, end, currency, pal, color, id, &y_of,
    );
}

/// Interaction avec une courbe de solde : survol (date et valeur), glisser
/// (sélectionne une période, qui filtre la grille), clic (efface la période).
/// `rect` porte l'échelle du temps et la hauteur de la sélection ; `hit` est
/// la zone sensible.
#[allow(clippy::too_many_arguments)]
fn chart_interaction(
    ui: &mut Ui,
    painter: &Painter,
    rect: Rect,
    hit: Rect,
    doc: &mut OpenDoc,
    series: &[(Date, Amount)],
    start: Date,
    end: Date,
    currency: &str,
    pal: &Palette,
    color: Color32,
    id: &str,
    y_of: &dyn Fn(f64) -> f32,
) {
    let span = (end.0 - start.0).max(1) as f32;
    let x_of_date =
        |d: Date| rect.left() + ((d.0 - start.0) as f32 / span).clamp(0.0, 1.0) * rect.width();
    let date_of_x = |x: f32| {
        Date(start.0 + (((x - rect.left()) / rect.width()).clamp(0.0, 1.0) * span).round() as i32)
    };
    let today = Date::today();
    let response = ui.interact(hit, ui.id().with(id), Sense::click_and_drag());
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
        painter.circle(pos2(x, y), 4.5, color, Stroke::new(2.0, card_fill(ui)));
        let future = if date > today {
            "  · prévisionnel"
        } else {
            ""
        };
        let text = format!("{date}  ·  {}{future}", money(value, currency));
        let galley =
            ui.painter()
                .layout_no_wrap(text, egui::TextStyle::Small.resolve(ui.style()), pal.fg);
        let size = galley.size() + vec2(14.0, 8.0);
        let top = (y - size.y - 12.0).max(hit.top() + 2.0);
        let mut tip = Rect::from_min_size(pos2(x + 10.0, top), size);
        if tip.right() > rect.right() {
            tip = tip.translate(vec2(-(tip.width() + 20.0), 0.0));
        }
        ui.painter().rect_filled(tip, 6.0, pal.menu_bg);
        ui.painter()
            .galley(tip.min + vec2(7.0, 4.0), galley, pal.fg);
        response.on_hover_cursor(egui::CursorIcon::Crosshair);
    }
}
