//! Thème visuel : polices, espacements et couleurs (clair et sombre).

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
    });
    ctx.set_visuals_of(Theme::Light, visuals(false));
    ctx.set_visuals_of(Theme::Dark, visuals(true));
}

fn visuals(dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let radius = CornerRadius::same(6);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    v.window_corner_radius = CornerRadius::same(10);
    v.selection.bg_fill = if dark { Color32::from_rgb(0x1E, 0x3A, 0x8A) } else { Color32::from_rgb(0xDB, 0xE7, 0xFE) };
    v.selection.stroke = Stroke::new(1.0, if dark { Color32::from_rgb(0x93, 0xC5, 0xFD) } else { ACCENT });
    v.hyperlink_color = ACCENT;
    if dark {
        v.panel_fill = Color32::from_rgb(0x18, 0x1B, 0x20);
        v.window_fill = Color32::from_rgb(0x1F, 0x23, 0x2A);
        v.extreme_bg_color = Color32::from_rgb(0x12, 0x14, 0x18);
        v.faint_bg_color = Color32::from_rgb(0x1D, 0x21, 0x27);
    } else {
        v.panel_fill = Color32::from_rgb(0xF7, 0xF8, 0xFA);
        v.window_fill = Color32::WHITE;
        v.extreme_bg_color = Color32::WHITE;
        v.faint_bg_color = Color32::from_rgb(0xF3, 0xF5, 0xF8);
    }
    v
}

/// Couleurs sémantiques dépendant du thème courant.
pub struct Palette {
    pub negative: Color32,
    pub positive: Color32,
    pub muted: Color32,
    pub month_rule: Color32,
    pub future: Color32,
    pub pointed: Color32,
    pub reconciled: Color32,
    pub card: Color32,
    pub sidebar: Color32,
}

pub fn palette(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode {
        Palette {
            negative: Color32::from_rgb(0xF8, 0x71, 0x71),
            positive: Color32::from_rgb(0x4A, 0xDE, 0x80),
            muted: Color32::from_gray(150),
            month_rule: Color32::from_rgb(0xB9, 0x1C, 0x1C),
            future: Color32::from_rgb(0xF8, 0x71, 0x71),
            pointed: Color32::from_rgb(0x60, 0xA5, 0xFA),
            reconciled: Color32::from_rgb(0x4A, 0xDE, 0x80),
            card: Color32::from_rgb(0x22, 0x27, 0x2F),
            sidebar: Color32::from_rgb(0x14, 0x17, 0x1B),
        }
    } else {
        Palette {
            negative: Color32::from_rgb(0xC0, 0x26, 0x26),
            positive: Color32::from_rgb(0x15, 0x80, 0x3D),
            muted: Color32::from_gray(115),
            month_rule: Color32::from_rgb(0xDC, 0x26, 0x26),
            future: Color32::from_rgb(0xDC, 0x26, 0x26),
            pointed: ACCENT,
            reconciled: Color32::from_rgb(0x15, 0x80, 0x3D),
            card: Color32::WHITE,
            sidebar: Color32::from_rgb(0xEE, 0xF1, 0xF5),
        }
    }
}
