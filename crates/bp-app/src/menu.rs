//! Menus façon VS Code : entrées hautes avec gouttière d'icône ou de coche,
//! libellé à gauche, raccourci grisé à droite, séparateurs fins.

use egui::containers::menu::MenuConfig;
use egui::{Align2, Response, Sense, Style, Ui, vec2};
use egui_phosphor::regular as icon;

use crate::theme::palette;

const ITEM_HEIGHT: f32 = 30.0;
const MENU_MIN_WIDTH: f32 = 280.0;
const GUTTER: f32 = 34.0;

/// Style de la barre de menus (Fichier, Édition…).
pub fn bar_style(style: &mut Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = vec2(10.0, 5.0);
    style.spacing.item_spacing.x = 2.0;
}

/// Style des menus déroulants.
pub fn popup_style(style: &mut Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.spacing.item_spacing = vec2(0.0, 1.0);
}

pub fn config() -> MenuConfig {
    MenuConfig::new().style(popup_style)
}

/// Prépare un menu déroulant de largeur standard.
pub fn begin(ui: &mut Ui) {
    begin_with_width(ui, MENU_MIN_WIDTH);
}

/// Prépare un menu déroulant d'une largeur donnée (les entrées la remplissent).
pub fn begin_with_width(ui: &mut Ui, width: f32) {
    ui.set_width(width);
    ui.spacing_mut().item_spacing.y = 1.0;
}

/// Barre de menus : quand un menu est ouvert, survoler un autre titre l'ouvre
/// directement, sans recliquer (comme dans VS Code).
pub fn switch_on_hover(ctx: &egui::Context, buttons: &[Response]) {
    let ids: Vec<egui::Id> = buttons.iter().map(egui::Popup::default_response_id).collect();
    if !ids.iter().any(|&id| egui::Popup::is_id_open(ctx, id)) {
        return;
    }
    for (button, &id) in buttons.iter().zip(&ids) {
        if button.hovered() && !egui::Popup::is_id_open(ctx, id) {
            egui::Popup::close_all(ctx);
            egui::Popup::open_id(ctx, id);
        }
    }
}

/// Entrée de menu. Ferme le menu quand elle est cliquée.
pub fn item(ui: &mut Ui, glyph: Option<&str>, label: &str, shortcut: Option<&str>) -> Response {
    row(ui, Gutter::Icon(glyph), label, shortcut, true)
}

/// Entrée de menu désactivable.
pub fn item_enabled(ui: &mut Ui, enabled: bool, glyph: Option<&str>, label: &str, shortcut: Option<&str>) -> Response {
    row(ui, Gutter::Icon(glyph), label, shortcut, enabled)
}

/// Entrée à cocher : une coche dans la gouttière quand elle est active.
pub fn toggle(ui: &mut Ui, value: &mut bool, label: &str, shortcut: Option<&str>) -> Response {
    let response = row(ui, Gutter::Check(*value), label, shortcut, true);
    if response.clicked() {
        *value = !*value;
    }
    response
}

/// Entrée exclusive (choix parmi plusieurs) : coche si sélectionnée.
pub fn choice(ui: &mut Ui, selected: bool, label: &str) -> Response {
    row(ui, Gutter::Check(selected), label, None, true)
}

/// Titre de section, grisé, non cliquable.
pub fn heading(ui: &mut Ui, text: &str) {
    let pal = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + GUTTER, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        egui::TextStyle::Small.resolve(ui.style()),
        pal.muted,
    );
}

pub fn separator(ui: &mut Ui) {
    let pal = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(rect.x_range().shrink(8.0), rect.center().y, egui::Stroke::new(1.0, pal.border));
}

enum Gutter<'a> {
    Icon(Option<&'a str>),
    Check(bool),
}

fn row(ui: &mut Ui, gutter: Gutter<'_>, label: &str, shortcut: Option<&str>, enabled: bool) -> Response {
    let pal = palette(ui);
    let width = ui.available_width();
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(width, ITEM_HEIGHT), sense);
    let hovered = enabled && response.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 5.0, pal.selection);
    }
    let text_color = if !enabled {
        pal.muted.gamma_multiply(0.6)
    } else if hovered {
        pal.fg
    } else {
        pal.fg_dark
    };
    let body = egui::TextStyle::Body.resolve(ui.style());
    let center_y = rect.center().y;

    let gutter_glyph = match gutter {
        Gutter::Icon(glyph) => glyph,
        Gutter::Check(true) => Some(icon::CHECK),
        Gutter::Check(false) => None,
    };
    if let Some(glyph) = gutter_glyph {
        let color = match gutter {
            Gutter::Check(_) => pal.accent,
            Gutter::Icon(_) if hovered => pal.fg,
            Gutter::Icon(_) => pal.muted,
        };
        ui.painter().text(
            egui::pos2(rect.left() + GUTTER / 2.0, center_y),
            Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(16.0),
            if enabled { color } else { text_color },
        );
    }
    ui.painter().text(egui::pos2(rect.left() + GUTTER, center_y), Align2::LEFT_CENTER, label, body, text_color);
    if let Some(shortcut) = shortcut {
        ui.painter().text(
            egui::pos2(rect.right() - 12.0, center_y),
            Align2::RIGHT_CENTER,
            shortcut,
            egui::TextStyle::Small.resolve(ui.style()),
            pal.muted,
        );
    }
    if response.clicked() {
        ui.close();
    }
    response
}
