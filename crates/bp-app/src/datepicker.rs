//! Champ de date avec icône calendrier intégrée et sélecteur de date.
//!
//! La saisie est libre : `15` donne le 15 du mois en cours, `15/3` (ou
//! `15.3`, `15 3`, `1503`…) le 15 mars de l'année en cours. Pendant la
//! frappe, la date comprise s'affiche en clair dans le champ ; en le
//! quittant, le texte devient la date complète.

use bp_core::i18n;
use bp_core::model::Date;
use bp_core::tr;
use chrono::{Datelike, NaiveDate};
use egui::{Align2, Color32, CornerRadius, FontId, Id, Key, Rect, Response, Sense, Ui, vec2};
use egui_phosphor::regular as icon;

use crate::theme::palette;

/// Case d'un jour.
const CELL: f32 = 36.0;
/// Case d'un mois (vue des mois : 3 colonnes).
const MONTH_CELL: egui::Vec2 = egui::Vec2::new(CELL * 7.0 / 3.0, 46.0);

/// Où s'affiche le calendrier.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Fenêtre flottante sous le champ.
    Popup,
    /// Sous le champ, dans le flux (à utiliser dans un menu déjà flottant).
    Inline,
}

/// Champ de date de largeur `width`, icône calendrier comprise.
/// Renvoie la réponse du champ texte ; `changed()` est vrai aussi quand une
/// date est choisie dans le calendrier ou que la saisie est complétée.
pub fn date_field(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    text: &mut String,
    width: f32,
    placement: Placement,
) -> Response {
    let id = ui.id().with(id_salt);
    let open_id = id.with("open");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
    let today = Date::today();

    let mut field = ui.add(
        egui::TextEdit::singleline(text)
            .margin(egui::Margin {
                left: 10,
                right: crate::theme::FIELD_ICON_MARGIN,
                top: 8,
                bottom: 8,
            })
            .vertical_align(egui::Align::Center)
            .desired_width(width),
    );
    let parsed = Date::parse_fr(text, today);
    if field.has_focus() {
        typing_hint(ui, &field, text, parsed);
    }

    let icon_button = crate::theme::field_icon_button(
        ui,
        &field,
        id.with("icon"),
        icon::CALENDAR_DOTS,
        open,
        tr!("Choisir dans le calendrier"),
    );
    if icon_button.clicked() {
        open = !open;
        if open {
            show_month(ui, id, parsed.unwrap_or(today));
        }
        // On peut continuer à taper : le calendrier suit la saisie.
        field.request_focus();
    }
    if open
        && field.changed()
        && let Some(date) = parsed
    {
        show_month(ui, id, date);
    }

    // En quittant le champ, la saisie devient la date complète.
    if field.lost_focus() {
        if let Some(date) = parsed {
            let full = date.to_string();
            if *text != full {
                *text = full;
                field.mark_changed();
            }
        }
        if ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Tab)) {
            open = false;
        }
    }

    let mut picked = None;
    if open {
        let selected = Date::parse_fr(text, today);
        match placement {
            Placement::Popup => {
                let area = egui::Area::new(id.with("popup"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(field.rect.left_bottom() + vec2(0.0, 6.0))
                    .show(ui.ctx(), |ui| {
                        let pal = palette(ui);
                        egui::Frame::new()
                            .fill(pal.menu_bg)
                            .stroke(egui::Stroke::new(1.0, pal.border))
                            .corner_radius(14)
                            .shadow(egui::Shadow {
                                offset: [0, 10],
                                blur: 28,
                                spread: 0,
                                color: Color32::from_black_alpha(90),
                            })
                            .inner_margin(14)
                            .show(ui, |ui| calendar(ui, id, selected))
                            .inner
                    });
                picked = area.inner;
                let clicked_outside = ui.input(|i| i.pointer.primary_pressed())
                    && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
                        !area.response.rect.contains(p) && !field.rect.contains(p)
                    });
                if clicked_outside || ui.input(|i| i.key_pressed(Key::Escape)) {
                    open = false;
                }
            }
            Placement::Inline => {
                ui.add_space(6.0);
                picked = calendar(ui, id, selected);
            }
        }
    }
    if let Some(date) = picked {
        *text = date.to_string();
        field.mark_changed();
        open = false;
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
    field
}

/// Pendant la frappe, la date comprise en clair à droite du texte (ou un
/// avertissement si elle ne se lit pas), quand la place le permet.
fn typing_hint(ui: &Ui, field: &Response, text: &str, parsed: Option<Date>) {
    let typed = text.trim();
    if typed.is_empty() {
        return;
    }
    let pal = palette(ui);
    let (hint, color) = match parsed {
        Some(date) if date.to_string() == typed => return,
        Some(date) => (long_date(date), pal.muted),
        None => (tr!("date invalide").to_string(), pal.negative),
    };
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = FontId::proportional(12.5);
    let typed_width = ui.fonts_mut(|f| {
        f.layout_no_wrap(text.to_owned(), body, Color32::PLACEHOLDER)
            .size()
            .x
    });
    let hint = ui.fonts_mut(|f| f.layout_no_wrap(hint, small, color));
    let right = field.rect.right() - crate::theme::FIELD_ICON_MARGIN as f32;
    if field.rect.left() + 10.0 + typed_width + 14.0 + hint.size().x > right {
        return;
    }
    let pos = egui::pos2(
        right - hint.size().x,
        field.rect.center().y - hint.size().y / 2.0,
    );
    ui.painter().galley(pos, hint, color);
}

/// « jeu. 15 janv. 2026 », « Thu Jan 15, 2026 ».
fn long_date(date: Date) -> String {
    let weekday = date.to_naive().weekday().num_days_from_monday() as usize;
    format!(
        "{} {}",
        i18n::weekday_abbr(weekday),
        crate::settings::DateFormat::Text.format(date)
    )
}

/// Affiche dans le calendrier le mois de `date` (vue des jours).
fn show_month(ui: &Ui, id: Id, date: Date) {
    let d = date.to_naive();
    ui.data_mut(|data| {
        data.insert_temp(id.with("month"), (d.year(), d.month()));
        data.insert_temp(id.with("months_view"), false);
    });
}

/// Calendrier mensuel (semaine du lundi au dimanche). Un clic sur le mois
/// passe à la vue des douze mois de l'année. Renvoie la date cliquée.
pub fn calendar(ui: &mut Ui, id: Id, selected: Option<Date>) -> Option<Date> {
    let today = Date::today().to_naive();
    let month_id = id.with("month");
    let view_id = id.with("months_view");
    let (mut year, mut month) = ui
        .data(|d| d.get_temp::<(i32, u32)>(month_id))
        .unwrap_or_else(|| {
            let start = selected.map(Date::to_naive).unwrap_or(today);
            (start.year(), start.month())
        });
    let mut months_view = ui.data(|d| d.get_temp::<bool>(view_id).unwrap_or(false));
    let mut picked = None;

    ui.set_width(CELL * 7.0);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    // Page précédente/suivante au clavier, le champ gardant le focus.
    let (page_up, page_down) =
        ui.input(|i| (i.key_pressed(Key::PageUp), i.key_pressed(Key::PageDown)));

    // En-tête : titre cliquable (mois ou année), flèches de navigation.
    let pal = palette(ui);
    let mut step = 0i32;
    ui.horizontal(|ui| {
        let title = if months_view {
            year.to_string()
        } else {
            format!("{} {year}", i18n::month_title(month as usize - 1))
        };
        let tip = if months_view {
            tr!("Revenir aux jours")
        } else {
            tr!("Choisir le mois")
        };
        if header_title(ui, &title, months_view)
            .on_hover_text(tip)
            .clicked()
        {
            months_view = !months_view;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (next, previous) = if months_view {
                (tr!("Année suivante"), tr!("Année précédente"))
            } else {
                (tr!("Mois suivant"), tr!("Mois précédent"))
            };
            if round_button(ui, icon::CARET_RIGHT, next).clicked() || page_down {
                step = 1;
            }
            ui.add_space(2.0);
            if round_button(ui, icon::CARET_LEFT, previous).clicked() || page_up {
                step = -1;
            }
        });
    });
    if months_view {
        year += step;
    } else if step != 0 {
        let index = year * 12 + month as i32 - 1 + step;
        (year, month) = (index.div_euclid(12), index.rem_euclid(12) as u32 + 1);
    }
    ui.add_space(10.0);

    if months_view {
        for row in 0..4 {
            ui.horizontal(|ui| {
                for col in 0..3 {
                    let m = row * 3 + col + 1;
                    let (rect, response) = ui.allocate_exact_size(MONTH_CELL, Sense::click());
                    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    let is_shown = m == month;
                    let is_current = year == today.year() && m == today.month();
                    cell_background(ui, rect.shrink(3.0), 10, is_shown, &response);
                    let text_color = if is_shown {
                        pal.on_accent
                    } else if is_current {
                        pal.accent
                    } else {
                        pal.fg
                    };
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        i18n::month_abbr(m as usize - 1),
                        FontId::new(14.0, crate::theme::medium()),
                        text_color,
                    );
                    if response.clicked() {
                        month = m;
                        months_view = false;
                    }
                }
            });
        }
    } else {
        // Jours de la semaine.
        ui.horizontal(|ui| {
            for i in 0..7 {
                let (rect, _) = ui.allocate_exact_size(vec2(CELL, 24.0), Sense::hover());
                let color = if i >= 5 {
                    pal.muted.gamma_multiply(0.7)
                } else {
                    pal.muted
                };
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    i18n::weekday_letter(i),
                    FontId::new(11.5, crate::theme::medium()),
                    color,
                );
            }
        });
        ui.add_space(2.0);

        // Grille de 6 semaines commençant le lundi précédant le 1er du mois.
        let first = NaiveDate::from_ymd_opt(year, month, 1).expect("mois valide");
        let start = first - chrono::Days::new(first.weekday().num_days_from_monday() as u64);
        let selected = selected.map(Date::to_naive);
        for week in 0..6 {
            ui.horizontal(|ui| {
                for weekday in 0..7 {
                    let day = start + chrono::Days::new(week * 7 + weekday);
                    let (rect, response) = ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
                    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    let in_month = day.month() == month;
                    let is_selected = selected == Some(day);
                    let is_today = day == today;
                    let circle = Rect::from_center_size(rect.center(), vec2(32.0, 32.0));
                    cell_background(ui, circle, 16, is_selected, &response);
                    let color = if is_selected {
                        pal.on_accent
                    } else if is_today {
                        pal.accent
                    } else if !in_month {
                        pal.muted.gamma_multiply(0.45)
                    } else if weekday >= 5 {
                        pal.fg_dark
                    } else {
                        pal.fg
                    };
                    let font = if is_selected || is_today {
                        FontId::new(14.0, crate::theme::semibold())
                    } else {
                        FontId::proportional(14.0)
                    };
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        day.day().to_string(),
                        font,
                        color,
                    );
                    if is_today && !is_selected {
                        ui.painter().circle_filled(
                            rect.center() + vec2(0.0, 11.0),
                            2.0,
                            pal.accent,
                        );
                    }
                    if response.clicked() {
                        picked = Some(Date::from_naive(day));
                    }
                }
            });
        }
    }

    // Pied : retour à aujourd'hui.
    ui.add_space(8.0);
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(line, 0.0, pal.border.gamma_multiply(0.6));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let today_button = ui
            .add(
                egui::Button::new(
                    egui::RichText::new(format!(
                        "{}  {}",
                        icon::CALENDAR_CHECK,
                        tr!("Aujourd'hui")
                    ))
                    .color(pal.accent),
                )
                .frame(false)
                .min_size(vec2(0.0, 30.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if today_button.clicked() {
            picked = Some(Date::from_naive(today));
        }
    });

    ui.data_mut(|d| {
        d.insert_temp(month_id, (year, month));
        d.insert_temp(view_id, months_view);
    });
    picked
}

/// Titre de l'en-tête, cliquable (fond au survol, chevron).
fn header_title(ui: &mut Ui, title: &str, months_view: bool) -> Response {
    let pal = palette(ui);
    let chevron = if months_view {
        icon::CARET_UP
    } else {
        icon::CARET_DOWN
    };
    let galley = ui.fonts_mut(|f| {
        f.layout_no_wrap(
            format!("{title}  {chevron}"),
            FontId::new(15.0, crate::theme::semibold()),
            pal.fg,
        )
    });
    let size = galley.size() + vec2(16.0, 10.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    if response.hovered() {
        ui.painter().rect_filled(rect, 8.0, pal.bg_highlight);
    }
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, pal.fg);
    response
}

/// Bouton rond de navigation (flèche), fond au survol.
fn round_button(ui: &mut Ui, glyph: &str, tip: &str) -> Response {
    let pal = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    let response = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip);
    let hovered = response.hovered();
    if hovered {
        ui.painter()
            .circle_filled(rect.center(), 15.0, pal.bg_highlight);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(16.0),
        if hovered { pal.fg } else { pal.muted },
    );
    response
}

/// Fond d'une case : couleur d'accent si choisie, discrète au survol.
fn cell_background(ui: &Ui, rect: Rect, radius: u8, selected: bool, response: &Response) {
    let pal = palette(ui);
    let fill = if selected {
        pal.accent
    } else if response.hovered() {
        pal.bg_highlight
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(radius), fill);
}
