//! Fenêtres modales.

use std::path::PathBuf;

use egui::{Id, Key, Modal, RichText};
use egui_phosphor::regular as icon;

use crate::theme::palette;

/// Demande du mot de passe (ou de la clé de secours) à l'ouverture.
pub struct PasswordPrompt {
    pub path: PathBuf,
    pub input: String,
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
        Modal::new(Id::new("password")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(RichText::new(format!("{}  Fichier protégé", icon::LOCK_SIMPLE)).heading());
            ui.label(
                self.path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            );
            ui.add_space(8.0);
            let response = if self.use_recovery_key {
                ui.label("Clé de secours (24 mots) :");
                ui.add(
                    egui::TextEdit::multiline(&mut self.input)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
            } else {
                ui.label("Mot de passe :");
                ui.add(
                    crate::theme::field(&mut self.input)
                        .password(true)
                        .desired_width(f32::INFINITY),
                )
            };
            response.request_focus();
            if let Some(error) = &self.error {
                ui.label(RichText::new(error).color(palette(ui).negative));
            }
            if self.can_use_recovery_key {
                let label = if self.use_recovery_key {
                    "Utiliser le mot de passe"
                } else {
                    "Mot de passe oublié ? Utiliser la clé de secours"
                };
                if ui.link(label).clicked() {
                    self.use_recovery_key = !self.use_recovery_key;
                    self.input.clear();
                    self.error = None;
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Ouvrir").clicked()
                    || (!self.use_recovery_key && ui.input(|i| i.key_pressed(Key::Enter)))
                {
                    result = PromptResult::Submit;
                }
                if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
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
    Modal::new(Id::new("recovery")).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.label(RichText::new(format!("{}  Votre clé de secours", icon::SHIELD_CHECK)).heading());
        ui.add_space(6.0);
        ui.label("Votre fichier est maintenant chiffré. Si vous oubliez votre mot de passe, cette clé est le seul moyen de le rouvrir. Notez-la ou imprimez-la, et rangez-la en lieu sûr : elle ne sera plus jamais affichée.");
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
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(format!("{}  Copier", icon::COPY)).clicked() {
                ui.ctx().copy_text(phrase.to_string());
            }
            if ui.button("J'ai noté ma clé").clicked() {
                close = true;
            }
        });
    });
    close
}

pub enum Choice {
    Pending,
    Yes,
    No,
    Cancel,
}

/// Question à trois réponses (Oui / Non / Annuler).
pub fn confirm(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    message: &str,
    yes: &str,
    no: Option<&str>,
) -> Choice {
    let mut choice = Choice::Pending;
    Modal::new(Id::new(id)).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(RichText::new(title).heading());
        ui.add_space(6.0);
        ui.label(message);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new(yes).family(crate::theme::semibold()))
                .clicked()
            {
                choice = Choice::Yes;
            }
            if let Some(no) = no
                && ui.button(no).clicked()
            {
                choice = Choice::No;
            }
            if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                choice = Choice::Cancel;
            }
        });
    });
    choice
}

/// Message d'erreur avec un seul bouton.
pub fn alert(ctx: &egui::Context, title: &str, message: &str) -> bool {
    let mut close = false;
    Modal::new(Id::new("alert")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(RichText::new(format!("{}  {title}", icon::WARNING)).heading());
        ui.add_space(6.0);
        ui.label(message);
        ui.add_space(10.0);
        if ui.button("OK").clicked()
            || ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter))
        {
            close = true;
        }
    });
    close
}

/// Liste des anomalies corrigées à l'ouverture.
pub fn warnings(ctx: &egui::Context, warnings: &[String]) -> bool {
    let mut close = false;
    Modal::new(Id::new("warnings")).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.label(
            RichText::new(format!(
                "{}  Anomalies corrigées à l'ouverture",
                icon::WARNING
            ))
            .heading(),
        );
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for w in warnings {
                    ui.label(format!("• {w}"));
                }
            });
        ui.add_space(8.0);
        if ui.button("Fermer").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
    });
    close
}
