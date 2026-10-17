//! Champ de date avec bouton calendrier et sélecteur de date.

use bp_core::model::Date;
use chrono::{Datelike, NaiveDate};
use egui::{Align2, Id, Response, Sense, Ui, vec2};
use egui_phosphor::regular as icon;

use crate::theme::palette;

pub(crate) const MONTHS: [&str; 12] = [
    "Janvier",
    "Février",
    "Mars",
    "Avril",
    "Mai",
    "Juin",
    "Juillet",
    "Août",
    "Septembre",
    "Octobre",
    "Novembre",
    "Décembre",
];
const WEEKDAYS: [&str; 7] = ["L", "M", "M", "J", "V", "S", "D"];
const CELL: egui::Vec2 = egui::Vec2::new(38.0, 34.0);

/// Où s'affiche le calendrier.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Fenêtre flottante sous le champ.
    Popup,
    /// Sous le champ, dans le flux (à utiliser dans un menu déjà flottant).
    Inline,
}

/// Champ texte `jj/mm/aaaa` suivi d'un bouton qui ouvre le calendrier.
/// Renvoie la réponse du champ texte ; `changed()` est vrai aussi quand une
/// date est choisie dans le calendrier.
pub fn date_field(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    text: &mut String,
    width: f32,
    placement: Placement,
) -> Response {
    let id = ui.id().with(id_salt);
    let mut picked = None;
    let inner = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let button_width = 34.0;
        let mut field = ui.add(
            crate::theme::field(text)
                .hint_text("jj/mm/aaaa")
                .desired_width(width - button_width - 4.0),
        );
        let open_id = id.with("open");
        let open = ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
        let button = ui
            .add_sized(
                [button_width, field.rect.height()],
                egui::Button::new(icon::CALENDAR_DOTS).selected(open),
            )
            .on_hover_text("Choisir dans le calendrier");
        if button.clicked() {
            ui.data_mut(|d| d.insert_temp(open_id, !open));
            if !open {
                let start = Date::parse_fr(text, Date::today())
                    .unwrap_or_else(Date::today)
                    .to_naive();
                ui.data_mut(|d| d.insert_temp(id.with("month"), (start.year(), start.month())));
            }
        }
        let current = Date::parse_fr(text, Date::today());
        let open = ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
        if open {
            match placement {
                Placement::Popup => {
                    let anchor = field.rect.union(button.rect);
                    let area = egui::Area::new(id.with("popup"))
                        .order(egui::Order::Foreground)
                        .fixed_pos(anchor.left_bottom() + vec2(0.0, 6.0))
                        .show(ui.ctx(), |ui| {
                            let pal = palette(ui);
                            egui::Frame::new()
                                .fill(pal.menu_bg)
                                .corner_radius(10)
                                .shadow(egui::Shadow {
                                    offset: [0, 8],
                                    blur: 22,
                                    spread: 0,
                                    color: egui::Color32::from_black_alpha(110),
                                })
                                .inner_margin(12)
                                .show(ui, |ui| calendar(ui, id, current))
                                .inner
                        });
                    picked = area.inner;
                    let clicked_outside = ui.input(|i| i.pointer.primary_pressed())
                        && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
                            !area.response.rect.contains(p) && !button.rect.contains(p)
                        });
                    if picked.is_some()
                        || clicked_outside
                        || ui.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        ui.data_mut(|d| d.insert_temp(open_id, false));
                    }
                }
                Placement::Inline => {}
            }
        }
        if let Some(date) = picked {
            *text = date.to_string();
            field.mark_changed();
        }
        field
    });
    let mut field = inner.inner;
    if placement == Placement::Inline
        && ui.data(|d| d.get_temp::<bool>(id.with("open")).unwrap_or(false))
    {
        let current = Date::parse_fr(text, Date::today());
        if let Some(date) = calendar(ui, id, current) {
            *text = date.to_string();
            field.mark_changed();
            ui.data_mut(|d| d.insert_temp(id.with("open"), false));
        }
    }
    field
}

/// Calendrier mensuel (semaine du lundi au dimanche). Renvoie la date cliquée.
pub fn calendar(ui: &mut Ui, id: Id, selected: Option<Date>) -> Option<Date> {
    let pal = palette(ui);
    let today = Date::today().to_naive();
    let month_id = id.with("month");
    let (mut year, mut month) = ui
        .data(|d| d.get_temp::<(i32, u32)>(month_id))
        .unwrap_or_else(|| {
            let start = selected.map(Date::to_naive).unwrap_or(today);
            (start.year(), start.month())
        });
    let mut picked = None;

    ui.set_width(CELL.x * 7.0 + 12.0);
    ui.spacing_mut().item_spacing = vec2(0.0, 2.0);

    // En-tête : mois et année, navigation par mois et par année.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.label(
            egui::RichText::new(format!("{} {year}", MONTHS[month as usize - 1]))
                .size(15.0)
                .family(crate::theme::semibold())
                .color(pal.fg),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let nav = |ui: &mut Ui, glyph: &str, tip: &str| {
                ui.add(
                    egui::Button::new(glyph)
                        .frame(false)
                        .min_size(vec2(28.0, 28.0)),
                )
                .on_hover_text(tip)
                .clicked()
            };
            if nav(ui, icon::CARET_DOUBLE_RIGHT, "Année suivante") {
                year += 1;
            }
            if nav(ui, icon::CARET_RIGHT, "Mois suivant") {
                (year, month) = if month == 12 {
                    (year + 1, 1)
                } else {
                    (year, month + 1)
                };
            }
            if nav(ui, icon::CARET_LEFT, "Mois précédent") {
                (year, month) = if month == 1 {
                    (year - 1, 12)
                } else {
                    (year, month - 1)
                };
            }
            if nav(ui, icon::CARET_DOUBLE_LEFT, "Année précédente") {
                year -= 1;
            }
        });
    });
    ui.add_space(6.0);

    // Jours de la semaine.
    ui.horizontal(|ui| {
        for (i, day) in WEEKDAYS.iter().enumerate() {
            let (rect, _) = ui.allocate_exact_size(vec2(CELL.x, 22.0), Sense::hover());
            let color = if i >= 5 {
                pal.muted.gamma_multiply(0.8)
            } else {
                pal.muted
            };
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                *day,
                egui::TextStyle::Small.resolve(ui.style()),
                color,
            );
        }
    });

    // Grille de 6 semaines commençant le lundi précédant le 1er du mois.
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("mois valide");
    let start = first - chrono::Days::new(first.weekday().num_days_from_monday() as u64);
    let selected = selected.map(Date::to_naive);
    for week in 0..6 {
        ui.horizontal(|ui| {
            for weekday in 0..7 {
                let day = start + chrono::Days::new(week * 7 + weekday);
                let (rect, response) = ui.allocate_exact_size(CELL, Sense::click());
                let cell = rect.shrink(2.0);
                let in_month = day.month() == month;
                let is_selected = selected == Some(day);
                let is_today = day == today;
                if is_selected {
                    ui.painter().rect_filled(cell, 8.0, pal.accent);
                } else if response.hovered() {
                    ui.painter().rect_filled(cell, 8.0, pal.bg_highlight);
                }
                if is_today && !is_selected {
                    ui.painter().rect_stroke(
                        cell,
                        8.0,
                        egui::Stroke::new(1.5, pal.accent),
                        egui::StrokeKind::Inside,
                    );
                }
                let color = if is_selected {
                    pal.on_accent
                } else if !in_month {
                    pal.muted.gamma_multiply(0.5)
                } else if weekday >= 5 {
                    pal.fg_dark
                } else {
                    pal.fg
                };
                let font = if is_selected || is_today {
                    egui::FontId::new(14.0, crate::theme::semibold())
                } else {
                    egui::TextStyle::Body.resolve(ui.style())
                };
                ui.painter().text(
                    cell.center(),
                    Align2::CENTER_CENTER,
                    day.day().to_string(),
                    font,
                    color,
                );
                if response.clicked() {
                    picked = Some(Date::from_naive(day));
                }
            }
        });
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(format!("{}  Aujourd'hui", icon::CALENDAR_CHECK)).frame(false))
            .clicked()
        {
            picked = Some(Date::from_naive(today));
        }
    });

    ui.data_mut(|d| d.insert_temp(month_id, (year, month)));
    picked
}
