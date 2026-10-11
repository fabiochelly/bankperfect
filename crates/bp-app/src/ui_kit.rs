//! Éléments d'interface communs aux fenêtres : cartes, sections, lignes
//! libellé / champ, contrôle segmenté, listes colorées des modes et des
//! catégories, pastilles.

use bp_core::model::{Document, Mode};
use egui::{Color32, RichText};
use egui_phosphor::regular as icon;

use crate::theme::palette;

/// Marge intérieure des cartes.
pub const CARD_PADDING: f32 = 18.0;

/// Largeur d'une fenêtre : une fraction de l'écran, bornée.
pub fn dialog_width(ctx: &egui::Context, fraction: f32, min: f32, max: f32) -> f32 {
    (ctx.content_rect().width() * fraction).clamp(min, max)
}

/// Hauteur disponible pour le contenu d'une fenêtre (fraction de l'écran).
pub fn dialog_height(ctx: &egui::Context, fraction: f32) -> f32 {
    ctx.content_rect().height() * fraction
}

/// Description sous l'en-tête d'une fenêtre.
pub fn subtitle(ui: &mut egui::Ui, text: impl Into<String>) {
    let pal = palette(ui);
    ui.label(RichText::new(text.into()).color(pal.fg_dark));
    ui.add_space(16.0);
}

/// Titre de section en petites capitales, avec icône.
pub fn caption(ui: &mut egui::Ui, glyph: &str, title: &str) {
    let pal = palette(ui);
    ui.label(
        RichText::new(format!("{glyph}  {}", title.to_uppercase()))
            .size(11.5)
            .family(crate::theme::semibold())
            .color(pal.muted),
    );
    ui.add_space(6.0);
}

/// Carte de largeur `outer_width` (bordure comprise).
pub fn card<R>(ui: &mut egui::Ui, outer_width: f32, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let pal = palette(ui);
    let width = outer_width - 2.0 * CARD_PADDING - 2.0;
    egui::Frame::new()
        .fill(pal.bg)
        .stroke(egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)))
        .corner_radius(12)
        .inner_margin(egui::Margin::same(CARD_PADDING as i8))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.set_max_width(width);
            content(ui)
        })
        .inner
}

/// Titre de section suivi d'une carte, puis un espace.
pub fn section<R>(
    ui: &mut egui::Ui,
    outer_width: f32,
    glyph: &str,
    title: &str,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    caption(ui, glyph, title);
    let result = card(ui, outer_width, content);
    ui.add_space(14.0);
    result
}

/// Ligne « libellé à gauche, champ à droite » à hauteur de champ.
pub fn row<R>(
    ui: &mut egui::Ui,
    label: &str,
    label_width: f32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let pal = palette(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(36.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(label_width, 36.0), egui::Sense::hover());
        ui.painter().text(
            egui::pos2(rect.left(), rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::new(14.0, crate::theme::medium()),
            pal.fg,
        );
        content(ui)
    })
    .inner
}

/// Contrôle segmenté : choix de même largeur, l'option active teintée de sa
/// couleur. `width` : largeur totale (`None` = toute la largeur disponible).
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, &str, &str, Color32)],
    height: f32,
    width: Option<f32>,
) -> bool {
    let pal = palette(ui);
    let width = width.unwrap_or_else(|| ui.available_width());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, height / 2.0 - 4.0, pal.bg_dark);
    let segment = (rect.width() - 6.0) / options.len() as f32;
    let mut changed = false;
    for (k, &(option, glyph, label, color)) in options.iter().enumerate() {
        let r = egui::Rect::from_min_size(
            rect.min + egui::vec2(3.0 + k as f32 * segment, 3.0),
            egui::vec2(segment, height - 6.0),
        );
        let response = ui
            .interact(r, ui.id().with(("segment", label)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let active = *value == option;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, label)
        });
        let enabled = ui.is_enabled();
        let radius = r.height() / 2.0 - 3.0;
        if active {
            let (fill, border) = if enabled {
                (color.gamma_multiply(0.22), color.gamma_multiply(0.7))
            } else {
                (pal.bg_highlight, pal.border)
            };
            ui.painter().rect(
                r,
                radius,
                fill,
                egui::Stroke::new(1.0, border),
                egui::StrokeKind::Inside,
            );
        } else if response.hovered() && enabled {
            ui.painter().rect(
                r,
                radius,
                pal.bg_highlight,
                egui::Stroke::new(1.0, pal.border),
                egui::StrokeKind::Inside,
            );
        }
        let text_color = match (enabled, active) {
            (false, _) => pal.muted,
            (true, true) => color,
            (true, false) => pal.fg_dark,
        };
        let family = if active {
            crate::theme::semibold()
        } else {
            crate::theme::medium()
        };
        let text = if glyph.is_empty() {
            label.to_string()
        } else {
            format!("{glyph}  {label}")
        };
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::new(13.0, family),
            text_color,
        );
        if response.clicked() && enabled && !active {
            *value = option;
            changed = true;
        }
    }
    changed
}

/// Liste déroulante des modes de paiement, chacun avec l'icône et la couleur
/// de son badge. Renvoie vrai si le mode a changé.
pub fn mode_combo(
    ui: &mut egui::Ui,
    id: &str,
    document: &Document,
    mode: &mut Mode,
    width: f32,
) -> bool {
    let pal = palette(ui);
    let before = mode.clone();
    let (glyph, color) = crate::grid::mode_style(document, mode, &pal);
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(format!("{glyph}  {}", mode.label())).color(color))
        .width(width - ui.spacing().button_padding.x)
        .height(420.0)
        .show_ui(ui, |ui| {
            for m in document.mode_list() {
                let (glyph, color) = crate::grid::mode_style(document, &m, &pal);
                let text = RichText::new(format!("{glyph}  {}", m.label())).color(color);
                ui.selectable_value(mode, m, text);
            }
        });
    *mode != before
}

/// Liste déroulante des catégories (familles en gras, icônes et couleurs).
pub fn category_combo(
    ui: &mut egui::Ui,
    id: &str,
    document: &Document,
    styles: &std::collections::HashMap<u32, crate::category_icons::CategoryStyle>,
    category: &mut Option<u32>,
    width: f32,
) -> bool {
    let pal = palette(ui);
    let before = *category;
    let style = category.and_then(|id| styles.get(&id).copied());
    let selected = match category.and_then(|id| document.category_label(id)) {
        Some(label) => {
            let color = style.map_or(pal.fg, |s| crate::theme::category_color(&pal, s.color));
            let glyph = style.map_or(icon::TAG, |s| s.icon);
            RichText::new(format!("{glyph}  {label}")).color(color)
        }
        None => RichText::new("Aucune catégorie").color(pal.muted),
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(width - ui.spacing().button_padding.x)
        .height(420.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(category, None, "Aucune catégorie");
            for c in &document.categories {
                let style = styles.get(&c.id).copied();
                let glyph = style.map_or(icon::TAG, |s| s.icon);
                let indent = if c.is_child { "      " } else { "" };
                let mut text = RichText::new(format!("{indent}{glyph}  {}", c.name));
                if !c.is_child {
                    text = text.family(crate::theme::semibold());
                }
                if let Some(s) = style {
                    text = text.color(crate::theme::category_color(&pal, s.color));
                }
                ui.selectable_value(category, Some(c.id), text);
            }
        });
    *category != before
}

/// Pastille teintée (compteur, état).
pub fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::new(12.5, crate::theme::medium()),
        color,
    );
    let size = egui::vec2(galley.size().x + 20.0, 26.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 13.0, color.gamma_multiply(0.16));
    ui.painter().galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}

/// Champ texte avec une icône devant, sur toute la largeur `width`.
pub fn icon_field(
    ui: &mut egui::Ui,
    glyph: &str,
    text: &mut String,
    hint: &str,
    width: f32,
) -> egui::Response {
    let pal = palette(ui);
    let response = ui.add(
        crate::theme::field(text)
            .hint_text(hint)
            .margin(egui::Margin {
                left: 36,
                right: 10,
                top: 9,
                bottom: 9,
            })
            .desired_width(width),
    );
    ui.painter().text(
        egui::pos2(response.rect.left() + 18.0, response.rect.center().y),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(16.0),
        if response.has_focus() {
            pal.accent
        } else {
            pal.muted
        },
    );
    response
}

/// Cellule de largeur fixe, texte aligné à gauche (`Min`) ou à droite (`Max`)
/// et coupé avec « … » s'il déborde.
pub fn cell(ui: &mut egui::Ui, width: f32, align: egui::Align, text: RichText) {
    let layout = if align == egui::Align::Max {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, 22.0), layout, |ui| {
        ui.set_min_width(width);
        ui.add(egui::Label::new(text).truncate());
    });
}

/// Texte sur une seule ligne, coupé avec « … » au-delà de `max_width`.
pub fn one_line(
    ui: &egui::Ui,
    text: String,
    font: egui::FontId,
    color: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: max_width.max(10.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}
