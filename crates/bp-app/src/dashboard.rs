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

const CARD_HEIGHT: f32 = 120.0;
const CARD_MIN_WIDTH: f32 = 300.0;
const GAP: f32 = 18.0;

pub fn cards(ui: &mut Ui, doc: &mut OpenDoc) {
    let available = ui.available_width();
    let columns = (((available + GAP) / (CARD_MIN_WIDTH + GAP)).floor() as usize).clamp(1, 3);
    let width = (available - GAP * (columns - 1) as f32) / columns as f32;
    let cards: [fn(&mut Ui, Rect, &mut OpenDoc); 3] = [balance_card, tack_card, weekly_card];
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

    // Décomposition en tout petit : rapproché + pointé + à venir = solde.
    let small = FontId::new(11.0, egui::FontFamily::Proportional);
    let pointed_only = totals.pointed + -totals.reconciled;
    let mut job = egui::text::LayoutJob::default();
    let mut part = |text: String, color: Color32| {
        job.append(
            &text,
            0.0,
            egui::TextFormat {
                font_id: small.clone(),
                color,
                ..Default::default()
            },
        );
    };
    part(
        format!("{} rapproché", money(totals.reconciled, &currency)),
        pal.reconciled,
    );
    part("  +  ".into(), pal.muted);
    part(
        format!("{} pointé", money(pointed_only, &currency)),
        pal.pointed,
    );
    part("  +  ".into(), pal.muted);
    part(
        format!("{} à venir", money(totals.pending, &currency)),
        pal.future,
    );
    let mut equation = ui.painter().layout_job(job);
    if equation.size().x > content.width() {
        // Trop long : montants abrégés (k€, M€).
        let mut job = egui::text::LayoutJob::default();
        let mut part = |text: String, color: Color32| {
            job.append(
                &text,
                0.0,
                egui::TextFormat {
                    font_id: small.clone(),
                    color,
                    ..Default::default()
                },
            );
        };
        let short = |a: Amount| crate::format::money_compact(a, &currency);
        part(
            format!("{} rapproché", short(totals.reconciled)),
            pal.reconciled,
        );
        part(" + ".into(), pal.muted);
        part(format!("{} pointé", short(pointed_only)), pal.pointed);
        part(" + ".into(), pal.muted);
        part(format!("{} à venir", short(totals.pending)), pal.future);
        equation = ui.painter().layout_job(job);
    }
    let equation_pos = pos2(content.left(), content.bottom() - equation.size().y);
    let equation_rect = Rect::from_min_size(equation_pos, equation.size());
    ui.painter()
        .with_clip_rect(content)
        .galley(equation_pos, equation, pal.muted);
    ui.interact(equation_rect, ui.id().with("equation"), Sense::hover())
        .on_hover_text(format!(
            "Rapproché : {}\nPointé (non rapproché) : {}\nÀ venir (non pointé) : {}\n= Solde : {}",
            money(totals.reconciled, &currency),
            money(pointed_only, &currency),
            money(totals.pending, &currency),
            money(totals.balance, &currency),
        ));

    let series = doc.insights().series.clone();
    let chart = Rect::from_min_max(
        pos2(value.right() + 28.0, content.top() + 2.0),
        pos2(content.right(), equation_rect.top() - 6.0),
    );
    let window = window_dates(&series, doc.chart_window);

    // Variation sur la fenêtre affichée, sous le montant.
    if let Some((start, end)) = window {
        let delta = balance_at(&series, end) + -balance_at(&series, start);
        let (glyph, color) = if delta.is_negative() {
            (icon::TREND_DOWN, pal.negative)
        } else {
            (icon::TREND_UP, pal.positive)
        };
        let sign = if delta.is_negative() { "" } else { "+" };
        ui.painter().text(
            pos2(value.left(), value.bottom() + 2.0),
            Align2::LEFT_TOP,
            format!("{glyph} {sign}{}", money(delta, &currency)),
            egui::TextStyle::Small.resolve(ui.style()),
            color,
        );
        if chart.width() > 60.0 {
            balance_chart(ui, chart, doc, &series, start, end, &currency, &pal);
        }
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
    let small = egui::TextStyle::Small.resolve(ui.style());

    if let Some(date) = totals.last_reconciled {
        ui.painter().text(
            header.right_center(),
            Align2::RIGHT_CENTER,
            format!("rapproché au {date}"),
            small.clone(),
            pal.muted,
        );
    }

    // Barre à trois zones, proportionnelle au nombre d'opérations.
    let bar = Rect::from_min_size(
        pos2(content.left(), content.top() + 6.0),
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
    let shown: Vec<usize> = (0..3).filter(|&k| insights.tack_counts[k] > 0).collect();
    for (pos, &k) in shown.iter().enumerate() {
        let (filter, color, label) = zones[k];
        let w = (bar.width() * insights.tack_counts[k] as f32 / total as f32).max(4.0);
        let zone = Rect::from_min_max(
            pos2(x, bar.top()),
            pos2((x + w).min(bar.right()), bar.bottom()),
        );
        x += w;
        let first = pos == 0;
        let last = pos + 1 == shown.len();
        let radius = CornerRadius {
            nw: if first { 7 } else { 0 },
            sw: if first { 7 } else { 0 },
            ne: if last { 7 } else { 0 },
            se: if last { 7 } else { 0 },
        };
        let response = ui.interact(
            zone.expand2(vec2(0.0, 6.0)),
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

    // Légende cliquable en trois colonnes égales ; la part de chaque état
    // est dans l'info-bulle.
    let column = content.width() / 3.0;
    let ly = bar.bottom() + 22.0;
    for (k, (filter, color, label)) in zones.iter().enumerate() {
        let lx = content.left() + column * k as f32;
        let item = Rect::from_min_size(pos2(lx, ly - 10.0), vec2(column - 6.0, 20.0));
        let response = ui.interact(item, ui.id().with(("tack_legend", k)), Sense::click());
        let active = doc.filters.tack == *filter;
        ui.painter().circle_filled(pos2(lx + 4.0, ly), 4.0, *color);
        let text_color = if active || response.hovered() {
            pal.fg
        } else {
            pal.muted
        };
        let text = format!("{} {label}", crate::format::count(insights.tack_counts[k]));
        let galley = ui
            .painter()
            .layout(text, small.clone(), text_color, f32::INFINITY);
        ui.painter()
            .with_clip_rect(item.shrink2(vec2(0.0, -2.0)))
            .galley(
                pos2(lx + 12.0, ly - galley.size().y / 2.0),
                galley,
                text_color,
            );
        let percent = 100.0 * insights.tack_counts[k] as f32 / total as f32;
        let share = if percent > 0.0 && percent < 1.0 {
            "moins de 1 %".to_string()
        } else {
            format!("{percent:.0} %")
        };
        if response
            .on_hover_text(format!("{share} des opérations\nCliquer pour filtrer"))
            .clicked()
        {
            doc.filters.tack = if active { TackFilter::All } else { *filter };
        }
    }
}

// ───────────────────────── Semaine par semaine ─────────────────────────

/// Variation du solde semaine par semaine (lundi → dimanche), jusqu'à la
/// semaine en cours. Un clic sur une barre filtre la semaine.
fn weekly_card(ui: &mut Ui, rect: Rect, doc: &mut OpenDoc) {
    let pal = palette(ui);
    let (content, header) =
        card_frame(ui, rect, pal.violet, icon::CHART_BAR, "Semaine par semaine");
    let currency = doc.current_account().currency.clone();
    let series = doc.insights().series.clone();
    let small = egui::TextStyle::Small.resolve(ui.style());

    // Nombre de semaines selon la place : une barre tous les 14 px environ.
    let weeks = ((content.width() * 0.6 / 14.0) as usize).clamp(6, 26);
    let today = Date::today().to_naive();
    let this_monday = today - chrono::Days::new(today.weekday().num_days_from_monday() as u64);
    let data: Vec<(Date, Date, Amount)> = (0..weeks)
        .rev()
        .map(|k| {
            let monday = Date::from_naive(this_monday - chrono::Days::new(7 * k as u64));
            let sunday = Date(monday.0 + 6);
            let delta = balance_at(&series, sunday) + -balance_at(&series, Date(monday.0 - 1));
            (monday, sunday, delta)
        })
        .collect();
    let current = data.last().map_or(Amount::ZERO, |w| w.2);
    let average = Amount(data.iter().map(|w| w.2.0).sum::<i64>() / weeks.max(1) as i64);

    let sign = |a: Amount| if a.is_negative() { "" } else { "+" };
    ui.painter().text(
        header.right_center(),
        Align2::RIGHT_CENTER,
        format!(
            "moyenne {}{} / sem.",
            sign(average),
            crate::format::money_compact(average, &currency)
        ),
        small.clone(),
        pal.muted,
    );
    let color = if current.is_negative() {
        pal.negative
    } else {
        pal.positive
    };
    let value = ui.painter().text(
        content.left_top(),
        Align2::LEFT_TOP,
        format!("{}{}", sign(current), money(current, &currency)),
        FontId::new(24.0, crate::theme::semibold()),
        color,
    );
    ui.painter().text(
        pos2(value.left(), value.bottom() + 2.0),
        Align2::LEFT_TOP,
        "cette semaine",
        small.clone(),
        pal.muted,
    );

    // Barres de part et d'autre d'une ligne zéro.
    let chart = Rect::from_min_max(
        pos2(value.right() + 28.0, content.top() + 2.0),
        content.right_bottom(),
    );
    if chart.width() < 60.0 {
        return;
    }
    let max = data
        .iter()
        .map(|w| w.2.0.unsigned_abs())
        .max()
        .unwrap_or(0)
        .max(1) as f32;
    let has_negative = data.iter().any(|w| w.2.is_negative());
    let has_positive = data.iter().any(|w| !w.2.is_negative());
    let zero_y = match (has_positive, has_negative) {
        (true, true) => chart.center().y,
        (false, true) => chart.top(),
        _ => chart.bottom(),
    };
    let up_room = zero_y - chart.top();
    let down_room = chart.bottom() - zero_y;
    ui.painter()
        .hline(chart.x_range(), zero_y, Stroke::new(1.0, pal.border));
    let slot = chart.width() / data.len() as f32;
    for (i, (monday, sunday, delta)) in data.iter().enumerate() {
        let x0 = chart.left() + slot * i as f32;
        let column = Rect::from_min_max(pos2(x0, chart.top()), pos2(x0 + slot, chart.bottom()));
        let response = ui.interact(column, ui.id().with(("week", i)), Sense::click());
        let active = doc.filters.date_from == Some(*monday) && doc.filters.date_to == Some(*sunday);
        let ratio = delta.0.unsigned_abs() as f32 / max;
        let (top, bottom) = if delta.is_negative() {
            (zero_y, zero_y + down_room * ratio)
        } else {
            (zero_y - up_room * ratio, zero_y)
        };
        let bar = Rect::from_min_max(
            pos2(x0 + slot * 0.18, top.min(bottom - 1.0)),
            pos2(x0 + slot * 0.82, bottom.max(top + 1.0)),
        );
        let base = if delta.is_negative() {
            pal.negative
        } else {
            pal.positive
        };
        let current_week = i + 1 == data.len();
        let fill = if active || response.hovered() || current_week {
            base
        } else {
            base.gamma_multiply(0.6)
        };
        if response.hovered() {
            ui.painter().rect_filled(
                column.shrink2(vec2(1.0, 0.0)),
                4.0,
                pal.bg_highlight.gamma_multiply(0.6),
            );
        }
        ui.painter().rect_filled(bar, 2.0, fill);
        if response.clicked() {
            if active {
                doc.filters.date_from = None;
                doc.filters.date_to = None;
            } else {
                doc.filters.date_from = Some(*monday);
                doc.filters.date_to = Some(*sunday);
            }
        }
        response.on_hover_text(format!(
            "Semaine du {monday} au {sunday}\n{}{}\nCliquer pour filtrer",
            sign(*delta),
            money(*delta, &currency)
        ));
    }
}
