//! Fenêtre « À propos » : identité du logiciel, licence, puis le système
//! (carte graphique, moteur de rendu, emplacement) pour le diagnostic.

use bp_core::tr;
use egui::{Color32, RichText};
use egui_phosphor::regular as icon;

use crate::dialogs;
use crate::license::License;
use crate::theme::{Palette, palette};

const WIDTH: f32 = 500.0;
/// Largeur des libellés des lignes « Système ».
const LABEL_WIDTH: f32 = 150.0;
const REPO: &str = "https://github.com/fabiochelly/bankperfect";

/// Affiche la fenêtre ; vrai quand elle doit se fermer.
pub fn show(ctx: &egui::Context, license: Option<&License>) -> bool {
    let mut close = false;
    dialogs::modal(ctx, "about").show(ctx, |ui| {
        ui.set_width(WIDTH);
        let pal = palette(ui);
        hero(ui, &pal);
        ui.add_space(22.0);
        if let Some(license) = license {
            license_card(ui, &pal, license);
            ui.add_space(14.0);
        }
        crate::ui_kit::caption(ui, icon::CPU, tr!("Système"));
        crate::ui_kit::card(ui, WIDTH, |ui| system(ui, &pal));
        dialogs::footer(ui, |ui| {
            if dialogs::primary(ui, tr!("Fermer"), true).clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter))
            {
                close = true;
            }
            if dialogs::button(ui, format!("{}  GitHub", icon::GITHUB_LOGO)).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(REPO));
            }
        });
    });
    close
}

/// Logo, nom, puis version et licence en pastilles.
fn hero(ui: &mut egui::Ui, pal: &Palette) {
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(76.0, 76.0), egui::Sense::hover());
        crate::logo::paint_symbol(
            ui.painter(),
            rect,
            crate::theme::logo_b(pal),
            crate::theme::logo_p(pal),
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new("BankPerfect")
                .size(26.0)
                .family(crate::theme::semibold())
                .color(pal.fg),
        );
        ui.add_space(8.0);
    });
    let pills = [
        (format!("Version {}", env!("CARGO_PKG_VERSION")), pal.accent),
        (
            format!(
                "{}  {}",
                icon::SEAL_CHECK,
                tr!("Logiciel libre — licence MIT")
            ),
            pal.positive,
        ),
    ];
    centered_pills(ui, &pills);
}

/// Pastilles centrées sur une ligne.
fn centered_pills(ui: &mut egui::Ui, pills: &[(String, Color32)]) {
    let gap = 8.0;
    let font = egui::FontId::new(12.5, crate::theme::medium());
    let total: f32 = pills
        .iter()
        .map(|(text, color)| {
            ui.painter()
                .layout_no_wrap(text.clone(), font.clone(), *color)
                .size()
                .x
                + 20.0
        })
        .sum::<f32>()
        + gap * (pills.len().saturating_sub(1)) as f32;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
        for (text, color) in pills {
            crate::ui_kit::pill(ui, text, *color);
        }
    });
}

/// Titulaire de la licence, dans une carte avec une icône teintée.
fn license_card(ui: &mut egui::Ui, pal: &Palette, license: &License) {
    let (glyph, color, kind) = if license.gold {
        (
            icon::CROWN,
            pal.warning,
            tr!("Licence Gold n° {}", license.number),
        )
    } else {
        (icon::KEY, pal.accent, tr!("Licence n° {}", license.number))
    };
    crate::ui_kit::card(ui, WIDTH, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            badge(ui, glyph, color);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    RichText::new(&license.name)
                        .size(15.0)
                        .family(crate::theme::semibold())
                        .color(pal.fg),
                );
                ui.label(RichText::new(kind).size(12.5).color(pal.muted));
            });
        });
    });
}

/// Icône dans une pastille ronde teintée.
fn badge(ui: &mut egui::Ui, glyph: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), egui::Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 19.0, color.gamma_multiply(0.16));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(18.0),
        color,
    );
}

/// Carte graphique, moteur, diagnostic du rendu logiciel et emplacement.
fn system(ui: &mut egui::Ui, pal: &Palette) {
    let software = crate::graphics::software();
    if software {
        warning_banner(
            ui,
            pal,
            tr!("Rendu logiciel, sans la carte graphique : affichage allégé"),
        );
        ui.add_space(10.0);
    }
    let mut rows: Vec<(&str, String, String)> = Vec::new();
    if let Some((card, api)) = crate::graphics::about() {
        rows.push((icon::GRAPHICS_CARD, tr!("Carte graphique").into(), card));
        rows.push((icon::STACK, tr!("Moteur de rendu").into(), api));
    }
    if software {
        let adapters = crate::graphics::display_adapters();
        if !adapters.is_empty() {
            rows.push((
                icon::MONITOR,
                tr!("Adaptateur Windows").into(),
                adapters.join(", "),
            ));
        }
        if let Some(reason) = crate::graphics::glow_failure() {
            rows.push((icon::WARNING, "OpenGL".into(), reason));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        rows.push((
            icon::FOLDER_OPEN,
            tr!("Emplacement").into(),
            exe.display().to_string(),
        ));
    }
    for (k, (glyph, label, value)) in rows.iter().enumerate() {
        if k > 0 {
            separator(ui, pal);
        }
        info_row(ui, pal, glyph, label, value);
    }
}

/// Ligne « icône et libellé à gauche, valeur à droite » ; la valeur passe
/// à la ligne si elle est longue.
fn info_row(ui: &mut egui::Ui, pal: &Palette, glyph: &str, label: &str, value: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(LABEL_WIDTH, 22.0), egui::Sense::hover());
        ui.painter().text(
            egui::pos2(rect.left(), rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{glyph}  {label}"),
            egui::FontId::new(13.0, crate::theme::medium()),
            pal.muted,
        );
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.add(
                egui::Label::new(RichText::new(value).size(13.0).color(pal.fg))
                    .wrap()
                    .selectable(true),
            );
        });
    });
}

fn separator(ui: &mut egui::Ui, pal: &Palette) {
    ui.add_space(6.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, pal.border.gamma_multiply(0.5)),
    );
    ui.add_space(6.0);
}

/// Bandeau d'avertissement teinté.
fn warning_banner(ui: &mut egui::Ui, pal: &Palette, text: &str) {
    egui::Frame::new()
        .fill(pal.warning.gamma_multiply(0.12))
        .stroke(egui::Stroke::new(1.0, pal.warning.gamma_multiply(0.35)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::WARNING).size(15.0).color(pal.warning));
                ui.add(egui::Label::new(RichText::new(text).size(13.0).color(pal.warning)).wrap());
            });
        });
}
