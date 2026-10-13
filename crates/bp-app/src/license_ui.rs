//! Fenêtre « Licence » : coller la clé reçue à l'achat et l'activer auprès
//! du serveur de licences, ou voir et retirer la licence active.

use std::sync::{Arc, Mutex};

use egui::RichText;
use egui_phosphor::regular as icon;

use crate::dialogs;
use crate::license::License;
use crate::theme::palette;

type Activation = Arc<Mutex<Option<Result<(String, License), String>>>>;

/// Ce que la fenêtre demande à l'application.
pub enum LicenseAction {
    Keep,
    Close,
    /// Licence activée : certificat à garder dans les réglages.
    Activated(String, License),
    /// Retirer la licence de cet ordinateur.
    Remove,
}

pub struct LicenseForm {
    key: String,
    /// La clé vient de l'ancienne version de BankPerfect.
    from_legacy: bool,
    pending: Option<Activation>,
    error: Option<String>,
    /// Le champ a déjà reçu le focus (à l'ouverture).
    focused: bool,
}

impl LicenseForm {
    pub fn new() -> Self {
        let legacy = crate::license::legacy_key();
        Self {
            from_legacy: legacy.is_some(),
            key: legacy.unwrap_or_default(),
            pending: None,
            error: None,
            focused: false,
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let slot: Activation = Arc::new(Mutex::new(None));
        let thread_slot = slot.clone();
        let key = self.key.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = crate::license::activate(&key);
            if let Ok(mut guard) = thread_slot.lock() {
                *guard = Some(result);
            }
            ctx.request_repaint();
        });
        self.pending = Some(slot);
        self.error = None;
    }

    pub fn show(&mut self, ctx: &egui::Context, current: Option<&License>) -> LicenseAction {
        // Réponse du serveur.
        if let Some(slot) = &self.pending
            && let Some(result) = slot.lock().ok().and_then(|mut g| g.take())
        {
            self.pending = None;
            match result {
                Ok((certificate, license)) => {
                    return LicenseAction::Activated(certificate, license);
                }
                Err(message) => self.error = Some(message),
            }
        }
        let mut action = LicenseAction::Keep;
        dialogs::modal(ctx, "license").show(ctx, |ui| {
            ui.set_width(crate::ui_kit::dialog_width(ui.ctx(), 0.4, 460.0, 560.0));
            let pal = palette(ui);
            dialogs::header(ui, icon::KEY, pal.accent, "Licence BankPerfect");
            match current {
                Some(license) => {
                    crate::ui_kit::card(ui, ui.available_width(), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(icon::SEAL_CHECK)
                                    .size(28.0)
                                    .color(pal.positive),
                            );
                            ui.add_space(6.0);
                            ui.vertical(|ui| {
                                let kind = if license.gold {
                                    "Licence Gold"
                                } else {
                                    "Licence"
                                };
                                ui.label(
                                    RichText::new(format!("{kind} n° {}", license.number))
                                        .size(17.0)
                                        .family(crate::theme::semibold()),
                                );
                                ui.label(RichText::new(&license.name).color(pal.fg_dark));
                            });
                        });
                    });
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(
                            "Merci pour votre soutien ! Votre nom et votre numéro de licence \
                             s'affichent en bas de la fenêtre.",
                        )
                        .color(pal.fg_dark),
                    );
                    dialogs::footer(ui, |ui| {
                        if dialogs::primary(ui, "Fermer", true).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            action = LicenseAction::Close;
                        }
                        if dialogs::button(ui, "Retirer de cet ordinateur").clicked() {
                            action = LicenseAction::Remove;
                        }
                    });
                }
                None => {
                    crate::ui_kit::subtitle(
                        ui,
                        "Si vous avez acheté une licence, collez ici la clé reçue lors de \
                         l'achat. Elle est vérifiée une seule fois auprès du serveur de \
                         l'auteur ; votre nom et votre numéro de licence s'afficheront \
                         ensuite en bas de la fenêtre.",
                    );
                    let busy = self.pending.is_some();
                    let field = ui.add_enabled(
                        !busy,
                        crate::theme::field(&mut self.key)
                            .hint_text("Clé de licence")
                            .desired_width(f32::INFINITY),
                    );
                    // Prêt à coller dès l'ouverture.
                    if !self.focused {
                        field.request_focus();
                        self.focused = true;
                    }
                    if self.from_legacy {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!(
                                "{}  Clé retrouvée dans l'ancienne version de BankPerfect.",
                                icon::INFO
                            ))
                            .small()
                            .color(pal.muted),
                        );
                    }
                    if let Some(error) = &self.error {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!("{}  {error}", icon::WARNING))
                                .color(pal.negative),
                        );
                    }
                    if busy {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("Vérification de la clé…").color(pal.fg_dark));
                        });
                    }
                    let ready = !busy && !self.key.trim().is_empty();
                    dialogs::footer(ui, |ui| {
                        if dialogs::primary(ui, "Activer", ready).clicked()
                            || (ready && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            self.start(ui.ctx());
                        }
                        if dialogs::button(ui, "Annuler").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            action = LicenseAction::Close;
                        }
                    });
                }
            }
        });
        action
    }
}
