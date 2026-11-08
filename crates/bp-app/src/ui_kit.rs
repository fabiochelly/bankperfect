//! Éléments d'interface communs aux fenêtres : cartes, sections, lignes
//! libellé / champ, contrôle segmenté, listes colorées des modes et des
//! catégories, pastilles.

use bp_core::model::{Document, Mode};
use bp_core::tr;
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

/// [`egui::ComboBox`] dont la liste reste au-dessus de la fenêtre qui l'ouvre.
///
/// Fenêtres modales et listes déroulantes partagent la couche « premier
/// plan » d'egui : une liste rouverte après un clic dans la fenêtre pouvait
/// s'afficher derrière elle (choix de catégorie de « Catégoriser les
/// opérations »). La liste devient une sous-couche de la fenêtre, toujours
/// dessinée juste au-dessus.
pub struct Combo(egui::ComboBox);

impl Combo {
    pub fn new(id_salt: impl egui::AsIdSalt) -> Self {
        Self(egui::ComboBox::from_id_salt(id_salt))
    }

    pub fn selected_text(self, text: impl Into<egui::WidgetText>) -> Self {
        Self(self.0.selected_text(text))
    }

    pub fn width(self, width: f32) -> Self {
        Self(self.0.width(width))
    }

    pub fn height(self, height: f32) -> Self {
        Self(self.0.height(height))
    }

    pub fn truncate(self) -> Self {
        Self(self.0.truncate())
    }

    pub fn show_ui<R>(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let shown = self.0.show_ui(ui, add_contents);
        let layer = ui.layer_id();
        let ctx = ui.ctx();
        if layer.order == egui::Order::Foreground
            && egui::ComboBox::is_open(ctx, shown.response.id)
            && ctx.memory(|m| m.areas().parent_layer(layer).is_none())
        {
            let popup =
                egui::LayerId::new(egui::Order::Foreground, shown.response.id.with("popup"));
            ctx.set_sublayer(layer, popup);
        }
        shown
    }
}

/// Infobulle, seulement si son texte n'est pas vide (pas de bulle vide).
pub fn hover(response: egui::Response, text: &str) -> egui::Response {
    if text.trim().is_empty() {
        response
    } else {
        response.on_hover_text(text.trim().to_string())
    }
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
            .interact(r, ui.id().with(("segment", k, label)), egui::Sense::click())
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
    Combo::new(id)
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
    category_combo_impl(ui, id, document, styles, category, width, false)
}

/// Comme [`category_combo`], de largeur fixe : un libellé trop long est
/// coupé avec « … » (colonnes alignées d'une liste).
pub fn category_combo_fixed(
    ui: &mut egui::Ui,
    id: &str,
    document: &Document,
    styles: &std::collections::HashMap<u32, crate::category_icons::CategoryStyle>,
    category: &mut Option<u32>,
    width: f32,
) -> bool {
    category_combo_impl(ui, id, document, styles, category, width, true)
}

fn category_combo_impl(
    ui: &mut egui::Ui,
    id: &str,
    document: &Document,
    styles: &std::collections::HashMap<u32, crate::category_icons::CategoryStyle>,
    category: &mut Option<u32>,
    width: f32,
    fixed: bool,
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
        None => RichText::new(tr!("Aucune catégorie")).color(pal.muted),
    };
    let mut combo = Combo::new(id)
        .selected_text(selected)
        .width(width - ui.spacing().button_padding.x)
        .height(420.0);
    if fixed {
        combo = combo.truncate();
    }
    combo.show_ui(ui, |ui| {
        ui.selectable_value(category, None, tr!("Aucune catégorie"));
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
    pill_shape(ui, text, color, egui::Sense::hover(), false);
}

/// [`pill`] cliquable ; `selected` l'entoure d'un trait.
pub fn pill_button(
    ui: &mut egui::Ui,
    text: &str,
    color: Color32,
    selected: bool,
) -> egui::Response {
    pill_shape(ui, text, color, egui::Sense::click(), selected)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn pill_shape(
    ui: &mut egui::Ui,
    text: &str,
    color: Color32,
    sense: egui::Sense,
    selected: bool,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::new(12.5, crate::theme::medium()),
        color,
    );
    let size = egui::vec2(galley.size().x + 20.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = if response.hovered() { 0.28 } else { 0.16 };
    ui.painter()
        .rect_filled(rect, 13.0, color.gamma_multiply(fill));
    if selected {
        ui.painter().rect_stroke(
            rect,
            13.0,
            egui::Stroke::new(1.5, color),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    response
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
    cell_response(ui, width, align, text);
}

/// Comme [`cell`], en renvoyant la zone (pour une infobulle).
pub fn cell_response(
    ui: &mut egui::Ui,
    width: f32,
    align: egui::Align,
    text: RichText,
) -> egui::Response {
    let layout = if align == egui::Align::Max {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, 22.0), layout, |ui| {
        ui.set_min_width(width);
        ui.add(egui::Label::new(text).truncate());
    })
    .response
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

/// Nombre entier réglé au clavier ou avec les boutons − et +. Remplace le
/// `DragValue` d'egui, qui changeait de valeur au moindre mouvement de la
/// souris pendant un clic. Renvoie vrai quand la valeur change (bouton, ou
/// nombre valide tapé).
pub fn stepper<T>(
    ui: &mut egui::Ui,
    id_salt: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    suffix: Option<&str>,
) -> bool
where
    T: Copy + Into<i64> + TryFrom<i64>,
{
    let (lo, hi) = ((*range.start()).into(), (*range.end()).into());
    let mut current: i64 = (*value).into();
    let id = ui.id().with(("stepper", id_salt));
    let digits = hi.max(lo.abs()).to_string().len() as f32;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let size = egui::vec2(30.0, 30.0);
        if ui
            .add_enabled(current > lo, egui::Button::new(icon::MINUS).min_size(size))
            .clicked()
        {
            current -= 1;
            changed = true;
        }
        // Texte en cours de frappe gardé tel quel tant que le champ a le
        // focus ; sinon, la valeur.
        let mut text = ui
            .data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| current.to_string());
        let response = ui.add(
            crate::theme::field(&mut text)
                .desired_width(14.0 + 9.0 * digits)
                .horizontal_align(egui::Align::Center),
        );
        if response.changed()
            && let Ok(typed) = text.trim().parse::<i64>()
            && (lo..=hi).contains(&typed)
            && typed != current
        {
            current = typed;
            changed = true;
        }
        if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
        if ui
            .add_enabled(current < hi, egui::Button::new(icon::PLUS).min_size(size))
            .clicked()
        {
            current += 1;
            changed = true;
        }
        if let Some(suffix) = suffix {
            ui.label(suffix);
        }
    });
    if changed && let Ok(v) = T::try_from(current.clamp(lo, hi)) {
        *value = v;
    }
    changed
}
