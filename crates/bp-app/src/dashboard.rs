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

const EQUATION_HEIGHT: f32 = 136.0;
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
    ui.add_space(12.0);
    toggle.then_some(Command::ToggleBalanceCards)
}

fn equation(ui: &mut Ui, doc: &mut OpenDoc) -> bool {
    let pal = palette(ui);
    let dark = ui.visuals().dark_mode;
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), EQUATION_HEIGHT), Sense::hover());
    let f = figures(doc);
    card(ui, rect, 0.0);

    let inner = Rect::from_min_max(rect.min + vec2(24.0, 16.0), rect.max - vec2(18.0, 14.0));
    let op_width = (inner.width() * 0.045).clamp(26.0, 44.0);
    let hero_width = inner.width() * 0.46;
    let term_width = ((inner.width() - hero_width - 2.0 * op_width) / 2.0).max(40.0);
    let pointed = Rect::from_min_size(inner.min, vec2(term_width, inner.height()));
    let pending = pointed.translate(vec2(term_width + op_width, 0.0));
    let hero = Rect::from_min_max(pos2(pending.right() + op_width, inner.top()), inner.max);
    let hero_color = if f.totals.balance.is_negative() {
        pal.negative
    } else {
        pal.positive
    };

    // Halo derrière le solde final.
    glow(
        &ui.painter_at(rect.shrink(1.0)),
        pos2(hero.left() + hero.width() * 0.2, hero.top() + 42.0),
        vec2(hero.width() * 0.6, 78.0),
        hero_color.gamma_multiply(if dark { 0.14 } else { 0.12 }),
    );

    // Libellés.
    let label = caption(
        ui,
        pointed.min,
        icon::CHECKS,
        "En banque",
        pal.accent,
        pointed.width(),
    );
    bank_tooltip(ui, label);
    caption(
        ui,
        pending.min,
        icon::CLOCK,
        "À venir",
        pal.future,
        pending.width(),
    );
    let toggle = layout_toggle(ui, toggle_rect(hero), false, &pal);
    let chips_right = hero.right() - TOGGLE_SIZE - 4.0;
    let chips_left = if hero.width() >= CHIPS_MIN_BLOCK {
        let area = Rect::from_min_max(
            pos2(hero.center().x, hero.top() - 3.0),
            pos2(chips_right, hero.top() + 15.0),
        );
        window_chips(ui, area, doc, &pal, hero_color)
    } else {
        chips_right
    };
    caption(
        ui,
        hero.min,
        icon::WALLET,
        "Solde final",
        hero_color,
        chips_left - hero.left() - 8.0,
    );

    // Montants, posés sur une même ligne de base.
    let with_chart = f.window.is_some() && hero.width() >= CHART_MIN_BLOCK + 60.0;
    let hero_text = if with_chart {
        hero.width() * 0.55
    } else {
        hero.width()
    };
    let balance = animated(ui, ui.id().with("balance_value"), f.totals.balance);
    let balance_galley = amount_galley(
        ui,
        balance,
        &f.currency,
        36.0,
        if balance.is_negative() {
            pal.negative
        } else {
            pal.fg
        },
        hero_text,
    );
    let baseline = hero.top() + 21.0 + baseline_of(&balance_galley);
    let mut hero_used = balance_galley.size().x;
    paint_on_baseline(ui, hero.left(), baseline, balance_galley);

    let pointed_value = animated(ui, ui.id().with("pointed_value"), f.totals.pointed);
    let galley = amount_galley(
        ui,
        pointed_value,
        &f.currency,
        24.0,
        pal.fg,
        pointed.width(),
    );
    paint_on_baseline(ui, pointed.left(), baseline, galley);

    // L'à venir s'affiche sans signe : c'est l'opérateur qui le porte.
    let pending_value = animated(ui, ui.id().with("pending_value"), f.totals.pending);
    let galley = amount_galley(
        ui,
        Amount(pending_value.0.abs()),
        &f.currency,
        24.0,
        pal.future,
        pending.width(),
    );
    paint_on_baseline(ui, pending.left(), baseline, galley);

    let operator = |x: f32, glyph: &str| {
        let galley = ui.painter().layout_no_wrap(
            glyph.into(),
            FontId::proportional(28.0),
            pal.muted.gamma_multiply(0.8),
        );
        let x = x - galley.size().x / 2.0;
        paint_on_baseline(ui, x, baseline, galley);
    };
    let sign = if f.totals.pending.is_negative() {
        "\u{2212}"
    } else {
        "+"
    };
    operator(pointed.right() + op_width / 2.0, sign);
    operator(pending.right() + op_width / 2.0, "=");

    // Lignes secondaires.
    let lines_top = baseline + 9.0;
    let mut lines = Vec::new();
    lines.extend(
        f.window
            .and_then(|(start, end)| pointed_trend(&f, start, end, doc.chart_window, &pal)),
    );
    lines.push(reconciled_line(&f, &pal));
    paint_lines(
        ui,
        pos2(pointed.left(), lines_top),
        &lines,
        pointed.width(),
        inner.bottom() - 6.0,
    );

    let mut pending_line = match f.totals.pending_count {
        0 => "rien à pointer".to_string(),
        1 => "1 non pointée".to_string(),
        n => format!("{} non pointées", crate::format::count(n)),
    };
    let (future_sum, future_count) = f.future_pending;
    if future_count > 0 {
        pending_line.push_str(&format!(
            " · {} à venir",
            money_compact(future_sum, &f.currency)
        ));
    }
    paint_lines(
        ui,
        pos2(pending.left(), lines_top),
        &[(pending_line, pal.fg_dark)],
        pending.width(),
        inner.bottom(),
    );
    let frise = Rect::from_min_max(
        pos2(pending.left(), lines_top + 17.0),
        pos2(pending.right(), inner.bottom() - 9.0),
    );
    if frise.height() >= 8.0 {
        pending_timeline(ui, frise, doc, &f.currency, &pal);
    }

    // Barre de composition sous les deux premiers termes.
    composition_bar(
        ui,
        Rect::from_min_max(
            pos2(pointed.left(), inner.bottom() - 4.0),
            pos2(pending.right(), inner.bottom()),
        ),
        &f,
        &pal,
    );

    let mut lines = Vec::new();
    if let Some((start, end)) = f.window {
        lines.push(balance_trend(&f, start, end, doc.chart_window, &pal));
        if end > Date::today() {
            lines.push(forecast_line(&f, end, &pal));
        }
    }
    let width = paint_lines(
        ui,
        pos2(hero.left(), lines_top),
        &lines,
        hero_text,
        rect.bottom(),
    );
    hero_used = hero_used.max(width);

    // Courbe du solde, jusqu'aux bords droit et bas de la carte.
    if with_chart && let Some((start, end)) = f.window {
        let chart = Rect::from_min_max(
            pos2(hero.left() + hero_used + 28.0, hero.top() + 30.0),
            pos2(rect.right() - 1.0, rect.bottom() - 1.0),
        );
        if chart.width() > 60.0 {
            balance_chart(
                ui,
                chart,
                rect.shrink(1.0),
                doc,
                &f.series,
                start,
                end,
                &f.currency,
                &pal,
                hero_color,
                "balance_chart",
            );
        }
    }
    toggle
}

/// Barre de composition : la part du solde final et celle de l'à venir, à
/// l'échelle du pointé. Hachurée quand l'à venir retire de l'argent.
fn composition_bar(ui: &Ui, rect: Rect, f: &Figures, pal: &Palette) {
    let (p, b) = (f.totals.pointed.as_f64(), f.totals.balance.as_f64());
    let lo = 0f64.min(p).min(b);
    let hi = 0f64.max(p).max(b);
    if hi - lo < 0.01 {
        return;
    }
    let x = |v: f64| rect.left() + ((v - lo) / (hi - lo)) as f32 * rect.width();
    let painter = ui.painter();
    let radius = rect.height() / 2.0;
    painter.rect_filled(rect, radius, pal.border.gamma_multiply(0.5));
    let span = |a: f64, b: f64| Rect::from_x_y_ranges(x(a.min(b))..=x(a.max(b)), rect.y_range());
    let final_color = if b < 0.0 { pal.negative } else { pal.positive };
    painter.rect_filled(span(0.0, b), radius, final_color.gamma_multiply(0.85));
    let part = span(p, b);
    if part.width() >= 1.0 {
        if b < p {
            let clip = ui.painter_at(part);
            clip.rect_filled(part, radius, pal.future.gamma_multiply(0.22));
            let mut x = part.left() - part.height();
            while x < part.right() {
                clip.line_segment(
                    [pos2(x, part.bottom()), pos2(x + part.height(), part.top())],
                    Stroke::new(1.4, pal.future),
                );
                x += 4.0;
            }
        } else {
            painter.rect_filled(part, radius, pal.future);
        }
    }
    if lo < 0.0 {
        painter.vline(
            x(0.0),
            (rect.top() - 3.0)..=(rect.bottom() + 3.0),
            Stroke::new(1.0, pal.fg.gamma_multiply(0.5)),
        );
    }
    let sign = if f.totals.pending.is_negative() {
        "\u{2212}"
    } else {
        "+"
    };
    ui.interact(
        rect.expand(5.0),
        ui.id().with("composition"),
        Sense::hover(),
    )
    .on_hover_text(format!(
        "En banque {}  {sign}  à venir {}  =  solde final {}",
        money(f.totals.pointed, &f.currency),
        money(Amount(f.totals.pending.0.abs()), &f.currency),
        money(f.totals.balance, &f.currency),
    ));
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

/// Fond de la carte : plus foncé que la fenêtre en thème sombre, la surface
/// habituelle en thème clair.
fn card_fill(ui: &Ui) -> Color32 {
    let pal = palette(ui);
    if ui.visuals().dark_mode {
        pal.bg_dark.lerp_to_gamma(Color32::BLACK, 0.3)
    } else {
        pal.surface
    }
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
        let galley =
            ui.painter()
                .layout_no_wrap(window.label().into(), FontId::new(11.0, medium()), pal.fg);
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
    let date_of_x = |x: f32| {
        Date(start.0 + (((x - rect.left()) / rect.width()).clamp(0.0, 1.0) * span).round() as i32)
    };
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
