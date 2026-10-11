//! Thème visuel : police Inter, thèmes Tokyo Night (sombre, par défaut) et
//! Tokyo Night Day (clair).

use std::sync::Arc;

use egui::epaint::text::{FontTweak, VariationCoords};
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke, TextStyle, Theme,
    ThemePreference, Visuals,
};

const INTER: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");

const MEDIUM: &str = "inter-medium";
const WHEEL_LINE_SPEED: f32 = 120.0;
/// Multiplicateur des défilements exprimés en points (molettes sous Wayland).
pub const WHEEL_POINT_FACTOR: f32 = 2.0;
const SEMIBOLD: &str = "inter-semibold";

/// Couleurs d'une variante de Tokyo Night.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub bg_dark: Color32,
    pub bg_highlight: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub fg: Color32,
    pub fg_dark: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub selection: Color32,
    pub negative: Color32,
    pub positive: Color32,
    pub warning: Color32,
    pub month_rule: Color32,
    pub future: Color32,
    pub pointed: Color32,
    pub reconciled: Color32,
    pub stripe: Color32,
}

/// Tokyo Night (« night »).
pub const NIGHT: Palette = Palette {
    bg: Color32::from_rgb(0x1a, 0x1b, 0x26),
    bg_dark: Color32::from_rgb(0x16, 0x16, 0x1e),
    bg_highlight: Color32::from_rgb(0x29, 0x2e, 0x42),
    surface: Color32::from_rgb(0x24, 0x28, 0x3b),
    border: Color32::from_rgb(0x2f, 0x33, 0x4d),
    fg: Color32::from_rgb(0xc0, 0xca, 0xf5),
    fg_dark: Color32::from_rgb(0xa9, 0xb1, 0xd6),
    muted: Color32::from_rgb(0x73, 0x7a, 0xa2),
    accent: Color32::from_rgb(0x7a, 0xa2, 0xf7),
    on_accent: Color32::from_rgb(0x16, 0x16, 0x1e),
    selection: Color32::from_rgb(0x28, 0x34, 0x57),
    negative: Color32::from_rgb(0xf7, 0x76, 0x8e),
    positive: Color32::from_rgb(0x9e, 0xce, 0x6a),
    warning: Color32::from_rgb(0xe0, 0xaf, 0x68),
    month_rule: Color32::from_rgb(0xdb, 0x4b, 0x4b),
    future: Color32::from_rgb(0xff, 0x9e, 0x64),
    pointed: Color32::from_rgb(0x7d, 0xcf, 0xff),
    reconciled: Color32::from_rgb(0x9e, 0xce, 0x6a),
    stripe: Color32::from_rgb(0x1e, 0x20, 0x30),
};

/// Tokyo Night Day.
pub const DAY: Palette = Palette {
    bg: Color32::from_rgb(0xe1, 0xe2, 0xe7),
    bg_dark: Color32::from_rgb(0xd0, 0xd5, 0xe3),
    bg_highlight: Color32::from_rgb(0xc4, 0xc8, 0xda),
    surface: Color32::from_rgb(0xec, 0xed, 0xf1),
    border: Color32::from_rgb(0xb7, 0xbc, 0xd3),
    fg: Color32::from_rgb(0x34, 0x3b, 0x58),
    fg_dark: Color32::from_rgb(0x37, 0x60, 0xbf),
    muted: Color32::from_rgb(0x68, 0x70, 0x9a),
    accent: Color32::from_rgb(0x2e, 0x7d, 0xe9),
    on_accent: Color32::WHITE,
    selection: Color32::from_rgb(0xb7, 0xc1, 0xe3),
    negative: Color32::from_rgb(0xc6, 0x23, 0x4e),
    positive: Color32::from_rgb(0x38, 0x7a, 0x1f),
    warning: Color32::from_rgb(0x8c, 0x6c, 0x3e),
    month_rule: Color32::from_rgb(0xf5, 0x2a, 0x65),
    future: Color32::from_rgb(0xb1, 0x5c, 0x00),
    pointed: Color32::from_rgb(0x00, 0x71, 0x97),
    reconciled: Color32::from_rgb(0x38, 0x7a, 0x1f),
    stripe: Color32::from_rgb(0xd9, 0xdb, 0xe3),
};

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Name(SEMIBOLD.into()))),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 34.0;
        style.spacing.combo_height = 320.0;
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.scroll = egui::style::ScrollStyle {
            floating: true,
            bar_width: 12.0,
            floating_width: 8.0,
            floating_allocated_width: 0.0,
            handle_min_length: 48.0,
            bar_inner_margin: 2.0,
            bar_outer_margin: 2.0,
            foreground_color: true,
            dormant_background_opacity: 0.0,
            active_background_opacity: 0.15,
            interact_background_opacity: 0.25,
            dormant_handle_opacity: 0.35,
            active_handle_opacity: 0.55,
            interact_handle_opacity: 0.75,
            ..egui::style::ScrollStyle::floating()
        };
        // Les libellés ne capturent pas la souris : cliquer sur le texte d'une
        // ligne ou d'un compte sélectionne bien la ligne ou le compte.
        style.interaction.selectable_labels = false;
    });
    // Molette : environ 4 lignes de la grille par cran (egui : 40 points).
    ctx.options_mut(|o| o.input_options.line_scroll_speed = WHEEL_LINE_SPEED);
    ctx.set_visuals_of(Theme::Dark, visuals(&NIGHT, true));
    ctx.set_visuals_of(Theme::Light, visuals(&DAY, false));
    ctx.set_theme(ThemePreference::Dark);
}

/// Le hinting rend le texte net sur les écrans standard ; sur un écran haute
/// densité, on le désactive pour des formes plus fidèles et plus lisses.
pub fn adapt_to_display(ctx: &egui::Context) {
    let hinting = ctx.pixels_per_point() < 1.5;
    if ctx.style_of(ctx.theme()).visuals.text_options.font_hinting != hinting {
        ctx.all_styles_mut(|style| style.visuals.text_options.font_hinting = hinting);
    }
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let weight = |w: f32| {
        Arc::new(FontData {
            tweak: FontTweak {
                coords: VariationCoords::new([(b"wght", w)]),
                thin_space_width: 0.75,
                ..Default::default()
            },
            ..FontData::from_static(INTER)
        })
    };
    fonts.font_data.insert("inter".into(), weight(400.0));
    fonts.font_data.insert(MEDIUM.into(), weight(500.0));
    fonts.font_data.insert(SEMIBOLD.into(), weight(600.0));
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    // Inter d'abord, puis les icônes Phosphor (zone d'usage privé, retirée
    // d'Inter, voir assets/fonts/README.md) et les polices de secours d'egui.
    let fallbacks: Vec<String> = fonts.families[&FontFamily::Proportional].clone();
    let family = |main: &str| {
        let mut list = vec![main.to_string()];
        list.extend(fallbacks.iter().filter(|f| *f != "inter").cloned());
        list
    };
    fonts.families.insert(FontFamily::Proportional, family("inter"));
    fonts.families.insert(FontFamily::Name(MEDIUM.into()), family(MEDIUM));
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), family(SEMIBOLD));
    fonts
}

/// Inter Medium (500).
pub fn medium() -> FontFamily {
    FontFamily::Name(MEDIUM.into())
}

/// Inter SemiBold (600).
pub fn semibold() -> FontFamily {
    FontFamily::Name(SEMIBOLD.into())
}

fn visuals(p: &Palette, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.override_text_color = None;
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(if dark { 110 } else { 40 }) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(if dark { 90 } else { 30 }) };
    v.menu_corner_radius = CornerRadius::same(8);
    v.extreme_bg_color = p.bg_dark;
    v.faint_bg_color = p.stripe;
    v.code_bg_color = p.bg_dark;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.negative;
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.text_cursor.stroke = Stroke::new(2.0, p.accent);
    v.striped = true;

    let radius = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.bg;
    w.noninteractive.weak_bg_fill = p.bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.fg);
    w.inactive.bg_fill = p.surface;
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.fg);
    w.hovered.bg_fill = p.bg_highlight;
    w.hovered.weak_bg_fill = p.bg_highlight;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.6));
    w.hovered.fg_stroke = Stroke::new(1.5, p.fg);
    w.active.bg_fill = p.selection;
    w.active.weak_bg_fill = p.selection;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.5, p.fg);
    w.open.bg_fill = p.bg_highlight;
    w.open.weak_bg_fill = p.bg_highlight;
    w.open.bg_stroke = Stroke::new(1.0, p.border);
    w.open.fg_stroke = Stroke::new(1.0, p.fg);
    for state in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    v
}

/// Palette du thème courant.
pub fn palette(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode { NIGHT } else { DAY }
}

/// Champ de saisie d'une ligne, plus haut et plus aéré que celui d'egui.
pub fn field(text: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text).margin(egui::Margin::symmetric(10, 8)).vertical_align(egui::Align::Center)
}

/// Bouton principal (fond couleur d'accent).
pub fn primary_button(ui: &egui::Ui, text: impl Into<String>) -> egui::Button<'static> {
    let p = palette(ui);
    egui::Button::new(egui::RichText::new(text.into()).family(medium()).color(p.on_accent)).fill(p.accent)
}
