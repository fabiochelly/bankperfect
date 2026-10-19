//! Fenêtres modales.

use std::path::PathBuf;

use bp_core::tr;
use egui::{Id, Key, Modal, RichText};
use egui_phosphor::regular as icon;

use crate::theme::palette;

/// Fenêtre modale au cadre aéré, commune à toute l'application.
pub fn modal(ctx: &egui::Context, id: &str) -> Modal {
    let pal = crate::theme::palette_of(ctx);
    let frame = egui::Frame::new()
        .fill(pal.menu_bg)
        .stroke(egui::Stroke::new(1.0, pal.border))
        .corner_radius(14)
        .inner_margin(egui::Margin::same(24))
        .shadow(egui::Shadow {
            offset: [0, 12],
            blur: 40,
            spread: 0,
            color: egui::Color32::from_black_alpha(110),
        });
    Modal::new(Id::new(id)).frame(frame)
}

/// En-tête de fenêtre : icône dans une pastille teintée et titre.
pub fn header(ui: &mut egui::Ui, glyph: &str, color: egui::Color32, title: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 10.0, color.gamma_multiply(0.16));
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(19.0),
            color,
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(title)
                .size(18.0)
                .family(crate::theme::semibold()),
        );
    });
    ui.add_space(14.0);
}

/// Pied de fenêtre : de l'espace, puis les boutons alignés à droite. Le
/// contenu est disposé de droite à gauche (bouton principal en premier).
pub fn footer<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.add_space(22.0);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 36.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            add(ui)
        },
    )
    .inner
}

/// Bouton de pied de fenêtre (taille commune).
pub fn button(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Button::new(text).min_size(egui::vec2(100.0, 36.0)))
}

/// Bouton principal de pied de fenêtre.
pub fn primary(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    let button = crate::theme::primary_button(ui, text).min_size(egui::vec2(110.0, 36.0));
    ui.add_enabled(enabled, button)
}

/// Bouton principal rouge (action destructrice).
pub fn danger(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let pal = palette(ui);
    let button = egui::Button::new(
        RichText::new(text)
            .family(crate::theme::medium())
            .color(egui::Color32::WHITE),
    )
    .fill(pal.negative)
    .min_size(egui::vec2(110.0, 36.0));
    ui.add(button)
}

/// Demande du mot de passe (ou de la clé de secours) à l'ouverture.
pub struct PasswordPrompt {
    pub path: PathBuf,
    pub input: String,
    pub show_password: bool,
    pub use_recovery_key: bool,
    pub can_use_recovery_key: bool,
    pub error: Option<String>,
}

pub enum PromptResult {
    Pending,
    Submit,
    Cancel,
}

impl PasswordPrompt {
    pub fn show(&mut self, ctx: &egui::Context) -> PromptResult {
        let mut result = PromptResult::Pending;
        modal(ctx, "password").show(ctx, |ui| {
            ui.set_width(440.0);
            let pal = palette(ui);
            header(ui, icon::LOCK_SIMPLE, pal.accent, tr!("Fichier protégé"));
            ui.label(
                RichText::new(
                    self.path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
                .color(pal.muted),
            );
            ui.add_space(10.0);
            let response = if self.use_recovery_key {
                ui.label(tr!("Clé de secours (24 mots) :"));
                ui.add(
                    egui::TextEdit::multiline(&mut self.input)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
            } else {
                ui.label(tr!("Mot de passe :"));
                crate::theme::password_field(
                    ui,
                    "open_password",
                    &mut self.input,
                    &mut self.show_password,
                    f32::INFINITY,
                )
            };
            response.request_focus();
            if let Some(error) = &self.error {
                ui.label(RichText::new(error).color(palette(ui).negative));
            }
            if self.can_use_recovery_key {
                let label = if self.use_recovery_key {
                    tr!("Utiliser le mot de passe")
                } else {
                    tr!("Mot de passe oublié ? Utiliser la clé de secours")
                };
                if ui.link(label).clicked() {
                    self.use_recovery_key = !self.use_recovery_key;
                    self.input.clear();
                    self.error = None;
                }
            }
            footer(ui, |ui| {
                if primary(ui, tr!("Ouvrir"), true).clicked()
                    || (!self.use_recovery_key && ui.input(|i| i.key_pressed(Key::Enter)))
                {
                    result = PromptResult::Submit;
                }
                if button(ui, tr!("Annuler")).clicked() || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    result = PromptResult::Cancel;
                }
            });
        });
        result
    }
}

/// Affiche la clé de secours générée au premier enregistrement chiffré.
pub fn recovery_key(ctx: &egui::Context, phrase: &str) -> bool {
    let mut close = false;
    modal(ctx, "recovery").show(ctx, |ui| {
        ui.set_width(540.0);
        header(ui, icon::SHIELD_CHECK, palette(ui).positive, tr!("Votre clé de secours"));
        ui.label(tr!("Votre fichier est maintenant chiffré. Si vous oubliez votre mot de passe, cette clé est le seul moyen de le rouvrir. Notez-la ou imprimez-la, et rangez-la en lieu sûr : elle ne sera plus jamais affichée."));
        ui.add_space(10.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::Grid::new("words").num_columns(4).spacing([18.0, 6.0]).show(ui, |ui| {
                for (i, word) in phrase.split(' ').enumerate() {
                    ui.label(RichText::new(format!("{:>2}. {word}", i + 1)).monospace());
                    if i % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
        });
        footer(ui, |ui| {
            if primary(ui, tr!("J'ai noté ma clé"), true).clicked() {
                close = true;
            }
            if button(ui, format!("{}  {}", icon::COPY, tr!("Copier"))).clicked() {
                ui.ctx().copy_text(phrase.to_string());
            }
        });
    });
    close
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Pending,
    Yes,
    No,
    Cancel,
}

/// Question à trois réponses (Oui / Non / Annuler). `danger` : le bouton
/// principal est rouge (suppression).
#[allow(clippy::too_many_arguments)]
pub fn confirm(
    ctx: &egui::Context,
    id: &str,
    glyph: &str,
    title: &str,
    message: &str,
    yes: &str,
    no: Option<&str>,
    danger_action: bool,
) -> Choice {
    let mut choice = Choice::Pending;
    modal(ctx, id).show(ctx, |ui| {
        ui.set_width(460.0);
        let pal = palette(ui);
        let color = if danger_action {
            pal.negative
        } else {
            pal.warning
        };
        header(ui, glyph, color, title);
        ui.label(RichText::new(message).color(pal.fg_dark));
        footer(ui, |ui| {
            let yes_clicked = if danger_action {
                danger(ui, yes).clicked()
            } else {
                primary(ui, yes, true).clicked()
            };
            if yes_clicked || ui.input(|i| i.key_pressed(Key::Enter)) {
                choice = Choice::Yes;
            }
            if button(ui, tr!("Annuler")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                choice = Choice::Cancel;
            }
            if let Some(no) = no {
                // « Ne pas enregistrer » à l'écart, à gauche.
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if button(ui, RichText::new(no).color(pal.negative)).clicked() {
                        choice = Choice::No;
                    }
                });
            }
        });
    });
    choice
}

/// Message d'erreur avec un seul bouton.
pub fn alert(ctx: &egui::Context, title: &str, message: &str) -> bool {
    let mut close = false;
    modal(ctx, "alert").show(ctx, |ui| {
        ui.set_width(460.0);
        let pal = palette(ui);
        header(ui, icon::WARNING, pal.negative, title);
        ui.label(RichText::new(message).color(pal.fg_dark));
        footer(ui, |ui| {
            if primary(ui, tr!("OK"), true).clicked()
                || ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter))
            {
                close = true;
            }
        });
    });
    close
}

/// Liste des anomalies corrigées à l'ouverture.
pub fn warnings(ctx: &egui::Context, warnings: &[String]) -> bool {
    let mut close = false;
    modal(ctx, "warnings").show(ctx, |ui| {
        ui.set_width(540.0);
        header(
            ui,
            icon::WARNING,
            palette(ui).warning,
            tr!("Anomalies corrigées à l'ouverture"),
        );
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for w in warnings {
                    ui.label(format!("• {w}"));
                }
            });
        footer(ui, |ui| {
            if primary(ui, tr!("Fermer"), true).clicked()
                || ui.input(|i| i.key_pressed(Key::Escape))
            {
                close = true;
            }
        });
    });
    close
}
