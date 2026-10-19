//! Thème visuel : police Inter et galerie de thèmes (Tokyo Night par défaut,
//! Catppuccin, Dracula, Nord… et « BankPerfect classic », repris des couleurs
//! de l'ancienne version). Un thème sombre et un thème clair sont actifs à la
//! fois : le mode (sombre, clair ou système) choisit lequel s'affiche.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use egui::epaint::text::{FontTweak, VariationCoords};
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke,
    TextStyle, Theme, ThemePreference, Visuals,
};

const INTER: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");

const MEDIUM: &str = "inter-medium";
/// Défilement par cran de molette, en points (environ 6 lignes de la grille).
pub const WHEEL_LINE_SPEED: f32 = 180.0;
/// Largeur de la barre de défilement au survol.
pub const SCROLLBAR_WIDTH: f32 = 14.0;
/// Multiplicateur des défilements exprimés en points (molettes sous Wayland).
pub const WHEEL_POINT_FACTOR: f32 = 3.0;
const SEMIBOLD: &str = "inter-semibold";

/// Couleurs d'un thème.
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
    pub violet: Color32,
    /// Fond des menus et fenêtres flottantes : plus sombre que la fenêtre,
    /// opaque.
    pub menu_bg: Color32,
    /// Élément survolé dans un menu : encore plus sombre.
    pub menu_hover: Color32,
    pub teal: Color32,
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
    violet: Color32::from_rgb(0xbb, 0x9a, 0xf7),
    menu_bg: Color32::from_rgb(0x13, 0x13, 0x1a),
    menu_hover: Color32::from_rgb(0x0b, 0x0b, 0x10),
    teal: Color32::from_rgb(0x73, 0xda, 0xca),
};

/// Tokyo Night Day.
pub const DAY: Palette = Palette {
    bg: Color32::from_rgb(0xe1, 0xe2, 0xe7),
    bg_dark: Color32::from_rgb(0xd0, 0xd5, 0xe3),
    bg_highlight: Color32::from_rgb(0xc4, 0xc8, 0xda),
    surface: Color32::from_rgb(0xec, 0xed, 0xf1),
    border: Color32::from_rgb(0xb7, 0xbc, 0xd3),
    fg: Color32::from_rgb(0x34, 0x3b, 0x58),
    fg_dark: Color32::from_rgb(0x4c, 0x54, 0x76),
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
    violet: Color32::from_rgb(0x98, 0x54, 0xf1),
    menu_bg: Color32::from_rgb(0xd0, 0xd4, 0xe0),
    menu_hover: Color32::from_rgb(0xb4, 0xb9, 0xcd),
    teal: Color32::from_rgb(0x11, 0x8c, 0x74),
};

/// BankPerfect classic : couleurs par défaut de l'ancienne version (grille
/// blanche et bleu-gris, sélection acier, panneaux bleu clair, montants
/// négatifs rouges, trait rouge des changements de mois, coches sarcelle),
/// sur des fenêtres grises façon Windows.
pub const CLASSIC: Palette = Palette {
    bg: rgb(0xffffff),
    bg_dark: rgb(0xeef1f5),
    bg_highlight: rgb(0xe1ebf5),
    surface: rgb(0xcadceb),
    border: rgb(0xa9b8c8),
    fg: rgb(0x000000),
    fg_dark: rgb(0x2b2b2b),
    muted: rgb(0x6e6e6e),
    accent: rgb(0x2f5f98),
    on_accent: rgb(0xffffff),
    selection: rgb(0x9fb3cc),
    negative: rgb(0xe00000),
    positive: rgb(0x2e7d32),
    warning: rgb(0xa86400),
    month_rule: rgb(0xcc0000),
    future: rgb(0xcc5500),
    pointed: rgb(0x408080),
    reconciled: rgb(0x1f1f1f),
    stripe: rgb(0xdce8ef),
    violet: rgb(0x7b3fa0),
    menu_bg: rgb(0xf0f0f0),
    menu_hover: rgb(0xd5e3f0),
    teal: rgb(0x008080),
};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Mélange `pct` % de `b` dans `a`.
const fn mix(a: u32, b: u32, pct: u32) -> Color32 {
    const fn channel(a: u32, b: u32, shift: u32, pct: u32) -> u8 {
        let a = (a >> shift) & 0xff;
        let b = (b >> shift) & 0xff;
        ((a * (100 - pct) + b * pct + 50) / 100) as u8
    }
    Color32::from_rgb(
        channel(a, b, 16, pct),
        channel(a, b, 8, pct),
        channel(a, b, 0, pct),
    )
}

/// Couleurs d'origine d'un thème publié ; les autres en sont déduites.
struct Base {
    dark: bool,
    bg: u32,
    /// Plus sombre que `bg` : panneau des comptes, champs de saisie.
    bg_dark: u32,
    surface: u32,
    highlight: u32,
    border: u32,
    fg: u32,
    fg_dark: u32,
    muted: u32,
    accent: u32,
    on_accent: u32,
    red: u32,
    green: u32,
    yellow: u32,
    orange: u32,
    cyan: u32,
    violet: u32,
    teal: u32,
}

const fn derive(b: Base) -> Palette {
    Palette {
        bg: rgb(b.bg),
        bg_dark: rgb(b.bg_dark),
        bg_highlight: rgb(b.highlight),
        surface: rgb(b.surface),
        border: rgb(b.border),
        fg: rgb(b.fg),
        fg_dark: rgb(b.fg_dark),
        muted: rgb(b.muted),
        accent: rgb(b.accent),
        on_accent: rgb(b.on_accent),
        selection: mix(b.bg, b.accent, if b.dark { 22 } else { 26 }),
        negative: rgb(b.red),
        positive: rgb(b.green),
        // Le jaune des thèmes clairs est trop pâle pour du texte.
        warning: if b.dark {
            rgb(b.yellow)
        } else {
            mix(b.yellow, 0, 25)
        },
        month_rule: rgb(b.red),
        future: rgb(b.orange),
        pointed: rgb(b.cyan),
        reconciled: rgb(b.green),
        stripe: if b.dark {
            mix(b.bg, b.surface, 35)
        } else {
            mix(b.bg, b.bg_dark, 45)
        },
        violet: rgb(b.violet),
        menu_bg: if b.dark {
            mix(b.bg_dark, 0, 15)
        } else {
            rgb(b.bg_dark)
        },
        menu_hover: if b.dark {
            mix(b.bg_dark, 0, 50)
        } else {
            mix(b.bg_dark, b.fg, 15)
        },
        teal: rgb(b.teal),
    }
}

/// Un thème de la galerie.
pub struct ThemeDef {
    /// Identifiant enregistré dans les réglages.
    pub id: &'static str,
    pub name: &'static str,
    pub dark: bool,
    pub palette: Palette,
}

pub const DEFAULT_DARK: &str = "tokyo-night";
pub const DEFAULT_LIGHT: &str = "tokyo-night-day";

/// Thèmes proposés : les sombres, puis les clairs.
pub const THEMES: &[ThemeDef] = &[
    ThemeDef {
        id: DEFAULT_DARK,
        name: "Tokyo Night",
        dark: true,
        palette: NIGHT,
    },
    ThemeDef {
        id: "catppuccin-mocha",
        name: "Catppuccin Mocha",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x1e1e2e,
            bg_dark: 0x181825,
            surface: 0x313244,
            highlight: 0x3a3c51,
            border: 0x45475a,
            fg: 0xcdd6f4,
            fg_dark: 0xbac2de,
            muted: 0x7f849c,
            accent: 0xcba6f7,
            on_accent: 0x11111b,
            red: 0xf38ba8,
            green: 0xa6e3a1,
            yellow: 0xf9e2af,
            orange: 0xfab387,
            cyan: 0x89dceb,
            violet: 0x89b4fa,
            teal: 0x94e2d5,
        }),
    },
    ThemeDef {
        id: "catppuccin-macchiato",
        name: "Catppuccin Macchiato",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x24273a,
            bg_dark: 0x1e2030,
            surface: 0x363a4f,
            highlight: 0x3f4359,
            border: 0x494d64,
            fg: 0xcad3f5,
            fg_dark: 0xb8c0e0,
            muted: 0x8087a2,
            accent: 0xc6a0f6,
            on_accent: 0x181926,
            red: 0xed8796,
            green: 0xa6da95,
            yellow: 0xeed49f,
            orange: 0xf5a97f,
            cyan: 0x91d7e3,
            violet: 0x8aadf4,
            teal: 0x8bd5ca,
        }),
    },
    ThemeDef {
        id: "catppuccin-frappe",
        name: "Catppuccin Frappé",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x303446,
            bg_dark: 0x292c3c,
            surface: 0x414559,
            highlight: 0x494e63,
            border: 0x51576d,
            fg: 0xc6d0f5,
            fg_dark: 0xb5bfe2,
            muted: 0x8c93ad,
            accent: 0xca9ee6,
            on_accent: 0x232634,
            red: 0xe78284,
            green: 0xa6d189,
            yellow: 0xe5c890,
            orange: 0xef9f76,
            cyan: 0x99d1db,
            violet: 0x8caaee,
            teal: 0x81c8be,
        }),
    },
    ThemeDef {
        id: "dracula",
        name: "Dracula",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x282a36,
            bg_dark: 0x21222c,
            surface: 0x343746,
            highlight: 0x3c3f51,
            border: 0x44475a,
            fg: 0xf8f8f2,
            fg_dark: 0xdcdcd5,
            muted: 0x7b88b8,
            accent: 0xbd93f9,
            on_accent: 0x21222c,
            red: 0xff5555,
            green: 0x50fa7b,
            yellow: 0xf1fa8c,
            orange: 0xffb86c,
            cyan: 0x8be9fd,
            violet: 0xff79c6,
            teal: 0x6df1bc,
        }),
    },
    ThemeDef {
        id: "nord",
        name: "Nord",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x2e3440,
            bg_dark: 0x272c36,
            surface: 0x3b4252,
            highlight: 0x434c5e,
            border: 0x4c566a,
            fg: 0xe5e9f0,
            fg_dark: 0xd8dee9,
            muted: 0x8890a3,
            accent: 0x88c0d0,
            on_accent: 0x2e3440,
            red: 0xbf616a,
            green: 0xa3be8c,
            yellow: 0xebcb8b,
            orange: 0xd08770,
            cyan: 0x81a1c1,
            violet: 0xb48ead,
            teal: 0x8fbcbb,
        }),
    },
    ThemeDef {
        id: "gruvbox",
        name: "Gruvbox",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x282828,
            bg_dark: 0x1d2021,
            surface: 0x32302f,
            highlight: 0x3c3836,
            border: 0x504945,
            fg: 0xebdbb2,
            fg_dark: 0xd5c4a1,
            muted: 0xa89984,
            accent: 0xfe8019,
            on_accent: 0x1d2021,
            red: 0xfb4934,
            green: 0xb8bb26,
            yellow: 0xfabd2f,
            orange: 0xfabd2f,
            cyan: 0x83a598,
            violet: 0xd3869b,
            teal: 0x8ec07c,
        }),
    },
    ThemeDef {
        id: "rose-pine",
        name: "Rosé Pine",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x191724,
            bg_dark: 0x14121e,
            surface: 0x1f1d2e,
            highlight: 0x26233a,
            border: 0x403d52,
            fg: 0xe0def4,
            fg_dark: 0xc4c1db,
            muted: 0x908caa,
            accent: 0xebbcba,
            on_accent: 0x191724,
            red: 0xeb6f92,
            green: 0x9ccfd8,
            yellow: 0xf6c177,
            orange: 0xf6c177,
            cyan: 0xc4a7e7,
            violet: 0xc4a7e7,
            teal: 0x3e8fb0,
        }),
    },
    ThemeDef {
        id: "one-dark",
        name: "One Dark",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x282c34,
            bg_dark: 0x21252b,
            surface: 0x2c313a,
            highlight: 0x333842,
            border: 0x3e4451,
            fg: 0xc8ccd4,
            fg_dark: 0xabb2bf,
            muted: 0x7f848e,
            accent: 0x61afef,
            on_accent: 0x21252b,
            red: 0xe06c75,
            green: 0x98c379,
            yellow: 0xe5c07b,
            orange: 0xd19a66,
            cyan: 0x56b6c2,
            violet: 0xc678dd,
            teal: 0x56b6c2,
        }),
    },
    ThemeDef {
        id: "everforest",
        name: "Everforest",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x2d353b,
            bg_dark: 0x232a2e,
            surface: 0x343f44,
            highlight: 0x3d484d,
            border: 0x475258,
            fg: 0xd3c6aa,
            fg_dark: 0xc0b597,
            muted: 0x9da9a0,
            accent: 0xa7c080,
            on_accent: 0x232a2e,
            red: 0xe67e80,
            green: 0x83c092,
            yellow: 0xdbbc7f,
            orange: 0xe69875,
            cyan: 0x7fbbb3,
            violet: 0xd699b6,
            teal: 0x7fbbb3,
        }),
    },
    ThemeDef {
        id: "kanagawa",
        name: "Kanagawa",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x1f1f28,
            bg_dark: 0x16161d,
            surface: 0x2a2a37,
            highlight: 0x2f2f3e,
            border: 0x363646,
            fg: 0xdcd7ba,
            fg_dark: 0xc8c093,
            muted: 0x8a8980,
            accent: 0x7e9cd8,
            on_accent: 0x16161d,
            red: 0xe46876,
            green: 0x98bb6c,
            yellow: 0xe6c384,
            orange: 0xffa066,
            cyan: 0x7fb4ca,
            violet: 0x957fb8,
            teal: 0x7aa89f,
        }),
    },
    ThemeDef {
        id: "solarized-dark",
        name: "Solarized",
        dark: true,
        palette: derive(Base {
            dark: true,
            bg: 0x002b36,
            bg_dark: 0x00212b,
            surface: 0x073642,
            highlight: 0x0b3e4b,
            border: 0x174652,
            fg: 0x93a1a1,
            fg_dark: 0x839496,
            muted: 0x657b83,
            accent: 0x268bd2,
            on_accent: 0x002b36,
            red: 0xdc322f,
            green: 0x859900,
            yellow: 0xb58900,
            orange: 0xcb4b16,
            cyan: 0x2aa198,
            violet: 0x6c71c4,
            teal: 0x2aa198,
        }),
    },
    ThemeDef {
        id: DEFAULT_LIGHT,
        name: "Tokyo Night Day",
        dark: false,
        palette: DAY,
    },
    ThemeDef {
        id: "bankperfect-classic",
        name: "BankPerfect classic",
        dark: false,
        palette: CLASSIC,
    },
    ThemeDef {
        id: "catppuccin-latte",
        name: "Catppuccin Latte",
        dark: false,
        palette: derive(Base {
            dark: false,
            bg: 0xe6e9ef,
            bg_dark: 0xdce0e8,
            surface: 0xeff1f5,
            highlight: 0xccd0da,
            border: 0xbcc0cc,
            fg: 0x4c4f69,
            fg_dark: 0x5c5f77,
            muted: 0x6c6f85,
            accent: 0x8839ef,
            on_accent: 0xffffff,
            red: 0xd20f39,
            green: 0x40a02b,
            yellow: 0xdf8e1d,
            orange: 0xfe640b,
            cyan: 0x209fb5,
            violet: 0x1e66f5,
            teal: 0x179299,
        }),
    },
    ThemeDef {
        id: "rose-pine-dawn",
        name: "Rosé Pine Dawn",
        dark: false,
        palette: derive(Base {
            dark: false,
            bg: 0xfaf4ed,
            bg_dark: 0xf2e9e1,
            surface: 0xfffaf3,
            highlight: 0xe9e3df,
            border: 0xcecacd,
            fg: 0x575279,
            fg_dark: 0x67627f,
            muted: 0x797593,
            accent: 0x907aa9,
            on_accent: 0xffffff,
            red: 0xb4637a,
            green: 0x286983,
            yellow: 0xea9d34,
            orange: 0xd7827e,
            cyan: 0x56949f,
            violet: 0x907aa9,
            teal: 0x56949f,
        }),
    },
    ThemeDef {
        id: "gruvbox-light",
        name: bp_core::tr_mark!("Gruvbox clair"),
        dark: false,
        palette: derive(Base {
            dark: false,
            bg: 0xf2e5bc,
            bg_dark: 0xebdbb2,
            surface: 0xfbf1c7,
            highlight: 0xe0cfa5,
            border: 0xd5c4a1,
            fg: 0x3c3836,
            fg_dark: 0x504945,
            muted: 0x7c6f64,
            accent: 0xaf3a03,
            on_accent: 0xfbf1c7,
            red: 0x9d0006,
            green: 0x79740e,
            yellow: 0xd79921,
            orange: 0xb57614,
            cyan: 0x076678,
            violet: 0x8f3f71,
            teal: 0x427b58,
        }),
    },
    ThemeDef {
        id: "everforest-light",
        name: bp_core::tr_mark!("Everforest clair"),
        dark: false,
        palette: derive(Base {
            dark: false,
            bg: 0xf4f0d9,
            bg_dark: 0xe6e2cc,
            surface: 0xfdf6e3,
            highlight: 0xe0dcc7,
            border: 0xbdc3af,
            fg: 0x5c6a72,
            fg_dark: 0x6b7a83,
            muted: 0x829181,
            accent: 0x35a77c,
            on_accent: 0xffffff,
            red: 0xf85552,
            green: 0x6f8a00,
            yellow: 0xdfa000,
            orange: 0xf57d26,
            cyan: 0x3a94c5,
            violet: 0xdf69ba,
            teal: 0x35a77c,
        }),
    },
    ThemeDef {
        id: "solarized-light",
        name: bp_core::tr_mark!("Solarized clair"),
        dark: false,
        palette: derive(Base {
            dark: false,
            bg: 0xfdf6e3,
            bg_dark: 0xeee8d5,
            surface: 0xfffbef,
            highlight: 0xe6dfc8,
            border: 0xd9d2bc,
            fg: 0x586e75,
            fg_dark: 0x657b83,
            muted: 0x7a8b8c,
            accent: 0x268bd2,
            on_accent: 0xffffff,
            red: 0xdc322f,
            green: 0x859900,
            yellow: 0xb58900,
            orange: 0xcb4b16,
            cyan: 0x2aa198,
            violet: 0x6c71c4,
            teal: 0x2aa198,
        }),
    },
];

/// Thème d'identifiant `id`.
pub fn by_id(id: &str) -> Option<&'static ThemeDef> {
    THEMES.iter().find(|t| t.id == id)
}

/// Thème d'identifiant `id`, ou le thème par défaut de ce mode.
pub fn find(id: &str, dark: bool) -> &'static ThemeDef {
    by_id(id).filter(|t| t.dark == dark).unwrap_or_else(|| {
        let default = if dark { DEFAULT_DARK } else { DEFAULT_LIGHT };
        THEMES.iter().find(|t| t.id == default).unwrap()
    })
}

/// Palettes actives : thème sombre, thème clair.
static ACTIVE: RwLock<(Palette, Palette)> = RwLock::new((NIGHT, DAY));

fn active() -> (Palette, Palette) {
    *ACTIVE.read().unwrap_or_else(|e| e.into_inner())
}

/// Choisit le thème sombre et le thème clair, et met à jour les couleurs.
pub fn select(ctx: &egui::Context, dark_id: &str, light_id: &str) {
    *ACTIVE.write().unwrap_or_else(|e| e.into_inner()) =
        (find(dark_id, true).palette, find(light_id, false).palette);
    apply_visuals(ctx);
}

fn apply_visuals(ctx: &egui::Context) {
    let (dark, light) = active();
    ctx.set_visuals_of(Theme::Dark, visuals(&dark, true));
    ctx.set_visuals_of(Theme::Light, visuals(&light, false));
}

/// Fond de fenêtre translucide (le compositeur floute ce qu'il y a derrière,
/// comme Hyprland). Ne vaut que si la surface a été créée transparente.
static TRANSLUCENT: AtomicBool = AtomicBool::new(false);
/// La fenêtre a été créée avec une surface transparente.
static SURFACE_TRANSPARENT: AtomicBool = AtomicBool::new(false);

/// Opacité du fond de la fenêtre (grille, menus, barre d'état) : le thème
/// sombre est plus couvrant pour rester lisible sur un bureau très clair.
fn backdrop_alpha(dark: bool) -> f32 {
    if dark { 0.88 } else { 0.80 }
}

/// Opacité du panneau des comptes, un peu plus vitré que le reste.
pub fn sidebar_alpha(dark: bool) -> f32 {
    if dark { 0.82 } else { 0.70 }
}

pub fn set_surface_transparent(on: bool) {
    SURFACE_TRANSPARENT.store(on, Ordering::Relaxed);
}

pub fn surface_transparent() -> bool {
    SURFACE_TRANSPARENT.load(Ordering::Relaxed)
}

/// Active ou non le fond translucide, et met à jour les couleurs.
pub fn set_translucent(ctx: &egui::Context, on: bool) {
    TRANSLUCENT.store(on && surface_transparent(), Ordering::Relaxed);
    apply_visuals(ctx);
}

pub fn translucent() -> bool {
    TRANSLUCENT.load(Ordering::Relaxed)
}

/// Couleur de fond, rendue translucide si le fond vitré est actif.
pub fn backdrop(color: Color32, alpha: f32) -> Color32 {
    if translucent() {
        color.gamma_multiply(alpha)
    } else {
        color
    }
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(11.5, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (
                TextStyle::Monospace,
                FontId::new(13.5, FontFamily::Monospace),
            ),
            (
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(20.0, FontFamily::Name(SEMIBOLD.into())),
            ),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 34.0;
        style.spacing.combo_height = 320.0;
        style.spacing.menu_margin = egui::Margin::same(8);
        // Barre flottante au-dessus d'une marge réservée par la grille
        // (voir grid::SCROLLBAR_GUTTER) : rail discret, poignée arrondie
        // semi-transparente, plus large et plus opaque au survol.
        style.spacing.scroll = egui::style::ScrollStyle {
            floating: true,
            bar_width: SCROLLBAR_WIDTH,
            floating_width: 10.0,
            floating_allocated_width: 0.0,
            handle_min_length: 56.0,
            bar_inner_margin: 3.0,
            bar_outer_margin: 3.0,
            foreground_color: true,
            dormant_background_opacity: 0.25,
            active_background_opacity: 0.45,
            interact_background_opacity: 0.6,
            dormant_handle_opacity: 0.4,
            active_handle_opacity: 0.6,
            interact_handle_opacity: 0.8,
            ..egui::style::ScrollStyle::floating()
        };
        // Les libellés ne capturent pas la souris : cliquer sur le texte d'une
        // ligne ou d'un compte sélectionne bien la ligne ou le compte.
        style.interaction.selectable_labels = false;
        // Réactivité : survols et défilements programmés instantanés.
        style.animation_time = 0.0;
        style.scroll_animation = egui::style::ScrollAnimation::none();
    });
    apply_visuals(ctx);
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
    fonts
        .families
        .insert(FontFamily::Proportional, family("inter"));
    fonts
        .families
        .insert(FontFamily::Name(MEDIUM.into()), family(MEDIUM));
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), family(SEMIBOLD));
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
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.override_text_color = None;
    v.panel_fill = backdrop(p.bg, backdrop_alpha(dark));
    // Fenêtres et listes déroulantes opaques.
    v.window_fill = p.menu_bg.to_opaque();
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 110 } else { 40 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 30 }),
    };
    v.menu_corner_radius = CornerRadius::same(8);
    v.extreme_bg_color = p.bg_dark;
    // Bandes de la grille : un voile clair (sombre en thème clair) plutôt
    // qu'une couleur, pour rester visibles sur les îles, même translucides.
    v.faint_bg_color = if dark {
        Color32::from_white_alpha(9)
    } else {
        Color32::from_black_alpha(10)
    };
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
    for state in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    v
}

/// Couleur d'une famille de catégories, parmi les teintes du thème.
pub fn category_color(p: &Palette, index: usize) -> Color32 {
    let colors = [
        p.accent, p.teal, p.positive, p.violet, p.future, p.warning, p.negative, p.pointed,
    ];
    colors[index % colors.len()]
}

/// Couleur de la série ou de la part `index` d'un graphique.
pub fn chart_color(p: &Palette, index: usize) -> Color32 {
    let colors = [
        p.accent,
        p.violet,
        p.positive,
        p.warning,
        p.negative,
        p.pointed,
        p.future,
        p.teal,
        p.violet.lerp_to_gamma(p.negative, 0.5),
        p.pointed.lerp_to_gamma(p.accent, 0.5),
    ];
    colors[index % colors.len()]
}

/// Palette du thème courant.
pub fn palette(ui: &egui::Ui) -> Palette {
    let (dark, light) = active();
    if ui.visuals().dark_mode { dark } else { light }
}

/// Palette du thème courant, hors d'un `Ui`.
pub fn palette_of(ctx: &egui::Context) -> Palette {
    let (dark, light) = active();
    if ctx.theme() == Theme::Dark {
        dark
    } else {
        light
    }
}

/// Champ de saisie d'une ligne, plus haut et plus aéré que celui d'egui.
pub fn field(text: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .margin(egui::Margin::symmetric(10, 8))
        .vertical_align(egui::Align::Center)
}

/// Champ de mot de passe avec un œil pour l'afficher en clair (`visible`).
pub fn password_field(
    ui: &mut egui::Ui,
    id_salt: &str,
    text: &mut String,
    visible: &mut bool,
    width: f32,
) -> egui::Response {
    let id = ui.make_persistent_id(id_salt);
    // Affiché en clair, le champ reste un champ mot de passe pour egui :
    // seul le rendu change. La saisie passe ainsi par le même chemin dans
    // les deux modes (un champ ordinaire active la méthode de saisie, et
    // fcitx5 sous Wayland y perdait ou répétait des touches), et la copie
    // reste interdite.
    let font = egui::FontSelection::default().resolve(ui.style());
    let line_height = ui.fonts_mut(|f| f.row_height(&font)) + ui.spacing().extra_text_line_spacing;
    let color = ui
        .visuals()
        .override_text_color
        .unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
    let mut plain = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap_width: f32| {
        let mut job =
            egui::text::LayoutJob::simple_singleline(buf.as_str().to_owned(), font.clone(), color);
        job.keep_trailing_whitespace = true;
        for section in &mut job.sections {
            section.format.line_height = Some(line_height);
        }
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let mut edit = egui::TextEdit::singleline(text)
        .id(id)
        .margin(egui::Margin {
            left: 10,
            right: 38,
            top: 8,
            bottom: 8,
        })
        .vertical_align(egui::Align::Center)
        .desired_width(width)
        .password(true);
    if *visible {
        edit = edit.layouter(&mut plain);
    }
    let response = edit.show(ui).response.response;

    let (glyph, tip) = if *visible {
        (
            egui_phosphor::regular::EYE_SLASH,
            bp_core::tr!("Masquer le mot de passe"),
        )
    } else {
        (
            egui_phosphor::regular::EYE,
            bp_core::tr!("Afficher le mot de passe"),
        )
    };
    // L'œil capte aussi le glisser : sinon le champ dessous, qui le capte,
    // resterait survolé et garderait son curseur texte.
    let eye_rect = egui::Rect::from_center_size(
        egui::pos2(response.rect.right() - 19.0, response.rect.center().y),
        egui::vec2(28.0, 28.0),
    );
    let eye = ui
        .interact(eye_rect, id.with("eye"), egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip);
    eye.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    let pal = palette(ui);
    let hovered = eye.hovered() || eye.is_pointer_button_down_on();
    if hovered {
        ui.painter()
            .rect_filled(eye_rect, CornerRadius::same(6), pal.bg_highlight);
    }
    let color = match (*visible, hovered) {
        (true, _) => pal.accent,
        (false, true) => pal.fg,
        (false, false) => pal.muted,
    };
    ui.painter().text(
        eye_rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(20.0),
        color,
    );
    if eye.clicked() {
        *visible = !*visible;
        response.request_focus();
    }
    response
}

/// Bouton principal (fond couleur d'accent).
pub fn primary_button(ui: &egui::Ui, text: impl Into<String>) -> egui::Button<'static> {
    let p = palette(ui);
    egui::Button::new(
        egui::RichText::new(text.into())
            .family(medium())
            .color(p.on_accent),
    )
    .fill(p.accent)
}

/// Case à cocher façon Bootstrap : carré à coins arrondis, fond d'accent et
/// coche blanche une fois cochée ; le libellé est cliquable.
pub fn checkbox(
    ui: &mut egui::Ui,
    checked: &mut bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let p = palette(ui);
    let text: egui::WidgetText = text.into();
    const BOX: f32 = 17.0;
    const GAP: f32 = 8.0;
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let size = egui::vec2(BOX + GAP + galley.size().x, galley.size().y.max(BOX + 2.0));
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *checked,
            galley.text(),
        )
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let enabled = ui.is_enabled();
    let box_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + BOX / 2.0, rect.center().y),
        egui::vec2(BOX, BOX),
    );
    let painter = ui.painter();
    if response.has_focus() {
        painter.rect_filled(box_rect.expand(3.0), 6.0, p.accent.gamma_multiply(0.3));
    }
    if *checked {
        let fill = if response.hovered() {
            p.accent.gamma_multiply(0.85)
        } else {
            p.accent
        };
        painter.rect_filled(box_rect, 4.0, fill);
        let c = box_rect.center();
        let s = BOX / 17.0;
        painter.line(
            vec![
                c + egui::vec2(-4.2 * s, 0.2 * s),
                c + egui::vec2(-1.3 * s, 3.1 * s),
                c + egui::vec2(4.3 * s, -3.0 * s),
            ],
            Stroke::new(2.0, p.on_accent),
        );
    } else {
        let border = if response.hovered() {
            p.accent
        } else {
            p.muted
        };
        painter.rect(
            box_rect,
            4.0,
            p.bg_dark,
            Stroke::new(1.5, border),
            egui::StrokeKind::Inside,
        );
    }
    let color = if enabled { p.fg } else { p.muted };
    painter.galley(
        egui::pos2(
            box_rect.right() + GAP,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    response
}

/// Dégradés des deux lettres du symbole (proposition « A1 ») : le « b » en
/// bleus, le « p » en violets, issus des couleurs de l'ancien logo. Les mêmes
/// dans les deux thèmes.
pub fn logo_b(_p: &Palette) -> (Color32, Color32) {
    (
        Color32::from_rgb(0x7a, 0xa2, 0xf7),
        Color32::from_rgb(0x5a, 0x7f, 0xe0),
    )
}

pub fn logo_p(_p: &Palette) -> (Color32, Color32) {
    (
        Color32::from_rgb(0x8b, 0x6f, 0xd6),
        Color32::from_rgb(0xbb, 0x9a, 0xf7),
    )
}
