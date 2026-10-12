//! Fenêtre « Protection du fichier » : définir, changer ou retirer le mot de
//! passe, et régénérer la clé de secours.
//!
//! Le mot de passe actuel est redemandé avant toute modification, pour qu'une
//! session laissée ouverte ne suffise pas à changer la protection.

use bp_core::format_v6::{Protection, Unlock};
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::theme::palette;
use crate::tools::Outcome;

const MIN_LENGTH: usize = 8;

#[derive(Default)]
pub struct ProtectionForm {
    current: String,
    new: String,
    confirm: String,
    error: Option<String>,
}

enum Action {
    SetPassword,
    RemovePassword,
    NewRecoveryKey,
}

impl ProtectionForm {
    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let protected = is_protected(doc);
        crate::dialogs::modal(ctx, "tool_protection").show(ctx, |ui| {
            let pal = palette(ui);
            ui.set_width(500.0);
            crate::dialogs::header(ui, icon::SHIELD_CHECK, pal.accent, "Protection du fichier");
            let (glyph, status, color) = if protected {
                (
                    icon::LOCK_SIMPLE,
                    "Protégé par mot de passe : contenu chiffré (XChaCha20-Poly1305, Argon2id).",
                    pal.positive,
                )
            } else {
                (
                    icon::LOCK_SIMPLE_OPEN,
                    "Non protégé : n'importe qui peut ouvrir ce fichier avec BankPerfect.",
                    pal.warning,
                )
            };
            ui.label(RichText::new(format!("{glyph}  {status}")).color(color));
            ui.add_space(10.0);

            let mut action = None;
            egui::Grid::new("protection_fields")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    if protected {
                        ui.label("Mot de passe actuel");
                        ui.add(
                            crate::theme::field(&mut self.current)
                                .password(true)
                                .desired_width(260.0),
                        );
                        ui.end_row();
                    }
                    ui.label(if protected {
                        "Nouveau mot de passe"
                    } else {
                        "Mot de passe"
                    });
                    let response = ui.add(
                        crate::theme::field(&mut self.new)
                            .password(true)
                            .desired_width(260.0),
                    );
                    ui.end_row();
                    ui.label("Confirmation");
                    let confirm = ui.add(
                        crate::theme::field(&mut self.confirm)
                            .password(true)
                            .desired_width(260.0),
                    );
                    ui.end_row();
                    if (response.lost_focus() || confirm.lost_focus())
                        && ui.input(|i| i.key_pressed(Key::Enter))
                    {
                        action = Some(Action::SetPassword);
                    }
                });
            if !self.new.is_empty() {
                let (text, color) = strength(&self.new, &pal);
                ui.label(RichText::new(text).small().color(color));
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Une nouvelle clé de secours de 24 mots est créée avec le mot de passe : \
                     notez-la, c'est le seul moyen d'ouvrir le fichier si vous l'oubliez.",
                )
                .small()
                .color(pal.muted),
            );
            if let Some(error) = &self.error {
                ui.add_space(4.0);
                ui.label(RichText::new(error).color(pal.negative));
            }
            crate::dialogs::footer(ui, |ui| {
                let label = if protected {
                    "Changer le mot de passe"
                } else {
                    "Protéger"
                };
                if crate::dialogs::primary(ui, label, true).clicked() {
                    action = Some(Action::SetPassword);
                }
                if crate::dialogs::button(ui, "Fermer").clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    outcome = Outcome::Close;
                }
                if protected {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if crate::dialogs::button(ui, "Nouvelle clé de secours").clicked() {
                            action = Some(Action::NewRecoveryKey);
                        }
                        if crate::dialogs::button(ui, RichText::new("Retirer").color(pal.negative))
                            .on_hover_text("Retirer le mot de passe")
                            .clicked()
                        {
                            action = Some(Action::RemovePassword);
                        }
                    });
                }
            });
            if let Some(action) = action {
                match self.apply(action, doc, protected) {
                    Ok(done) => outcome = done,
                    Err(error) => self.error = Some(error),
                }
            }
        });
        outcome
    }

    fn apply(
        &mut self,
        action: Action,
        doc: &mut OpenDoc,
        protected: bool,
    ) -> Result<Outcome, String> {
        if protected && !check_current(doc, &self.current) {
            return Err("Mot de passe actuel incorrect.".into());
        }
        let password = match action {
            Action::SetPassword => {
                if self.new.chars().count() < MIN_LENGTH {
                    return Err(format!(
                        "Le mot de passe doit faire au moins {MIN_LENGTH} caractères."
                    ));
                }
                if self.new != self.confirm {
                    return Err("Les deux saisies ne correspondent pas.".into());
                }
                Some(self.new.clone())
            }
            Action::NewRecoveryKey => Some(self.current.clone()),
            Action::RemovePassword => None,
        };
        // Mot de passe connu de la session, pour revérifier avant d'enregistrer.
        doc.legacy_password = password.clone();
        let outcome = match password {
            Some(password) => {
                let (protection, key) =
                    Protection::with_password(&password).map_err(|e| e.to_string())?;
                doc.protection = Some(protection);
                Outcome::Protected(key.phrase())
            }
            None => {
                doc.protection = Some(Protection::none().map_err(|e| e.to_string())?);
                Outcome::Done("Mot de passe retiré — enregistrez (Ctrl+S) pour l'appliquer".into())
            }
        };
        doc.dirty = true;
        *self = Self::default();
        Ok(outcome)
    }
}

/// Protection effective du fichier ouvert (un fichier v5 avec mot de passe
/// sera chiffré au premier enregistrement).
pub fn is_protected(doc: &OpenDoc) -> bool {
    match &doc.protection {
        Some(p) => p.is_password_protected(),
        None => doc.legacy_password.is_some(),
    }
}

/// Vérifie le mot de passe actuel : celui du fichier v5 d'origine, sinon en
/// déverrouillant le fichier enregistré (coût Argon2id, ~0,5 s).
fn check_current(doc: &OpenDoc, password: &str) -> bool {
    if doc.protection.is_none() {
        return doc.legacy_password.as_deref() == Some(password);
    }
    if let Some(legacy) = &doc.legacy_password {
        return legacy == password;
    }
    std::fs::read(&doc.path)
        .ok()
        .is_some_and(|data| Protection::open(&data, Unlock::Password(password)).is_ok())
}

fn strength(password: &str, pal: &crate::theme::Palette) -> (&'static str, egui::Color32) {
    let n = password.chars().count();
    let classes = [
        password.chars().any(|c| c.is_lowercase()),
        password.chars().any(|c| c.is_uppercase()),
        password.chars().any(|c| c.is_ascii_digit()),
        password.chars().any(|c| !c.is_alphanumeric()),
    ]
    .iter()
    .filter(|&&b| b)
    .count();
    match (n, classes) {
        (0..MIN_LENGTH, _) => ("Trop court", pal.negative),
        (_, 0..=1) if n < 14 => ("Faible", pal.warning),
        (_, 2) if n < 12 => ("Moyen", pal.future),
        _ => ("Solide", pal.positive),
    }
}
