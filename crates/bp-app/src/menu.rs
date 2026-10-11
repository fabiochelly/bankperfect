//! Menus façon VS Code : entrées hautes avec gouttière d'icône ou de coche,
//! libellé à gauche, raccourci grisé à droite, séparateurs fins.

use egui::{Align2, Response, Sense, Style, Ui, vec2};
use egui_phosphor::regular as icon;

use crate::theme::palette;

const ITEM_HEIGHT: f32 = 30.0;
const MENU_MIN_WIDTH: f32 = 280.0;
const GUTTER: f32 = 34.0;

/// Style des menus déroulants.
pub fn popup_style(style: &mut Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.spacing.item_spacing = vec2(0.0, 1.0);
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

/// Barre de menus pilotée à la main, pour la réactivité de VS Code :
/// un menu s'ouvre dès l'appui (egui attend le relâchement), survoler un
/// autre titre l'ouvre directement, un appui ailleurs ou Échap le ferme.
#[derive(Default)]
pub struct MenuBar {
    open: Option<usize>,
    /// Rectangle du menu déroulant affiché à l'image précédente.
    popup_rect: Option<egui::Rect>,
}

const CLOSE_REQUEST: &str = "bp_menu_close_request";

impl MenuBar {
    /// Dessine les titres et renvoie le menu ouvert avec la position de son titre.
    pub fn titles(&mut self, ui: &mut Ui, titles: &[&str]) -> Option<(usize, egui::Rect)> {
        let pal = palette(ui);
        if self.open.is_some_and(|i| i >= titles.len()) {
            self.open = None;
        }
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let mut bar_rect = egui::Rect::NOTHING;
        let mut anchor = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (index, title) in titles.iter().enumerate() {
                let font = egui::TextStyle::Body.resolve(ui.style());
                let galley = ui.painter().layout_no_wrap(title.to_string(), font, pal.fg);
                let size = galley.size() + vec2(20.0, 10.0);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, *title)
                });
                bar_rect = bar_rect.union(rect);
                let is_open = self.open == Some(index);
                if response.hovered() && pressed {
                    self.open = if is_open { None } else { Some(index) };
                } else if response.hovered() && self.open.is_some() && !is_open {
                    self.open = Some(index);
                }
                let is_open = self.open == Some(index);
                if is_open || response.hovered() {
                    ui.painter().rect_filled(
                        rect,
                        5.0,
                        if is_open { pal.menu_hover } else { pal.menu_bg },
                    );
                }
                let color = if is_open || response.hovered() {
                    pal.fg
                } else {
                    pal.fg_dark
                };
                ui.painter()
                    .galley(rect.center() - galley.size() / 2.0, galley, color);
                if is_open {
                    anchor = Some((index, rect));
                }
            }
        });

        // Appui hors de la barre et du menu, ou Échap : fermeture.
        let outside = |p: egui::Pos2| {
            !bar_rect.contains(p) && !self.popup_rect.is_some_and(|r| r.contains(p))
        };
        if self.open.is_some()
            && ((pressed && pointer.is_some_and(outside))
                || ui.input(|i| i.key_pressed(egui::Key::Escape)))
        {
            self.open = None;
            anchor = None;
        }
        anchor
    }

    /// Affiche le menu déroulant sous son titre. `content` dessine les entrées.
    pub fn popup(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        anchor: egui::Rect,
        content: impl FnOnce(&mut Ui),
    ) {
        let pal = crate::theme::palette_of(ctx);
        let response = egui::Area::new(egui::Id::new(("bp_menu", index)))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor.left_bottom() + vec2(0.0, 4.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(pal.menu_bg)
                    .corner_radius(8)
                    .shadow(egui::Shadow {
                        offset: [0, 6],
                        blur: 18,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(100),
                    })
                    .inner_margin(6)
                    .show(ui, |ui| {
                        popup_style(ui.style_mut());
                        content(ui);
                    });
            })
            .response;
        self.popup_rect = Some(response.rect);
        if ctx
            .data_mut(|d| d.remove_temp::<bool>(egui::Id::new(CLOSE_REQUEST)))
            .unwrap_or(false)
        {
            self.open = None;
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

/// Entrée de menu. Ferme le menu quand elle est cliquée.
pub fn item(ui: &mut Ui, glyph: Option<&str>, label: &str, shortcut: Option<&str>) -> Response {
    row(ui, Gutter::Icon(glyph), label, shortcut, true)
}

/// Entrée de menu désactivable.
pub fn item_enabled(
    ui: &mut Ui,
    enabled: bool,
    glyph: Option<&str>,
    label: &str,
    shortcut: Option<&str>,
) -> Response {
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
    ui.painter().hline(
        rect.x_range().shrink(8.0),
        rect.center().y,
        egui::Stroke::new(1.0, pal.border),
    );
}

enum Gutter<'a> {
    Icon(Option<&'a str>),
    Check(bool),
}

fn row(
    ui: &mut Ui,
    gutter: Gutter<'_>,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> Response {
    let pal = palette(ui);
    let width = ui.available_width();
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, ITEM_HEIGHT), sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let hovered = enabled && response.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 5.0, pal.menu_hover);
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
    ui.painter().text(
        egui::pos2(rect.left() + GUTTER, center_y),
        Align2::LEFT_CENTER,
        label,
        body,
        text_color,
    );
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
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new(CLOSE_REQUEST), true));
    }
    response
}
