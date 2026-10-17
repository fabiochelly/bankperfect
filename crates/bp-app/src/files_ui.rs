//! Fenêtres liées aux fichiers : nouveau fichier de comptes.

use bp_core::model::Amount;
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

/// « Créer un nouveau fichier de comptes » : nom du groupe de comptes et
/// solde initial du compte courant, puis choix de l'emplacement.
#[derive(Default)]
pub struct NewFileForm {
    name: String,
    balance: String,
    error: Option<String>,
}

impl NewFileForm {
    pub fn show_standalone(
        &mut self,
        ctx: &egui::Context,
        start_dir: Option<std::path::PathBuf>,
    ) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.4, 560.0, 640.0);
        crate::dialogs::modal(ctx, "new_file").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::FILE_PLUS, pal.accent, "Nouveau fichier de comptes");
            ui_kit::subtitle(
                ui,
                "Un fichier regroupe plusieurs comptes (courant, livret…). Il est créé avec un compte « Courant » et les catégories habituelles.",
            );
            let mut submit = false;
            ui_kit::section(ui, width, icon::FOLDER_SIMPLE, "Fichier", |ui| {
                let label_width = 170.0;
                let w = ui.available_width() - label_width - 8.0;
                ui_kit::row(ui, "Nom", label_width, |ui| {
                    let r = ui.add(
                        crate::theme::field(&mut self.name)
                            .hint_text("Ex. : Comptes de la famille")
                            .desired_width(w),
                    );
                    if r.ctx.memory(|m| m.focused().is_none()) {
                        r.request_focus();
                    }
                    submit |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                });
                ui.add_space(8.0);
                ui_kit::row(ui, "Solde du compte courant", label_width, |ui| {
                    let r = ui.add(
                        crate::theme::field(&mut self.balance)
                            .hint_text("0,00 (négatif si découvert)")
                            .desired_width(220.0),
                    );
                    submit |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                });
            });
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            let valid = !self.name.trim().is_empty();
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, "Créer…", valid).clicked() {
                    submit = true;
                }
                if crate::dialogs::button(ui, "Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            });
            let pick = egui::Id::new("new_file");
            if submit && valid {
                match self.balance() {
                    Ok(_) => crate::file_picker::open(ctx, pick, crate::file_picker::Kind::Save, self.dialog(start_dir)),
                    Err(e) => self.error = Some(e),
                }
            }
            if let Some(path) = crate::file_picker::chosen(ctx, pick) {
                match self.create(path) {
                    Ok(path) => outcome = Outcome::Reopen(path, "Nouveau fichier de comptes créé".into()),
                    Err(e) => self.error = Some(e),
                }
            }
        });
        outcome
    }

    fn balance(&self) -> Result<Amount, String> {
        if self.balance.trim().is_empty() {
            Ok(Amount::ZERO)
        } else {
            Amount::parse(&self.balance).ok_or_else(|| "Solde invalide".into())
        }
    }

    /// Choix de l'emplacement du nouveau fichier.
    fn dialog(&self, start_dir: Option<std::path::PathBuf>) -> rfd::FileDialog {
        let name = bp_core::manage::clean(self.name.trim());
        let mut dialog = rfd::FileDialog::new()
            .set_title("Enregistrer le nouveau fichier de comptes")
            .set_file_name(format!("{name}.bp"))
            .add_filter("Comptes BankPerfect", &["bp"]);
        if let Some(dir) = start_dir {
            dialog = dialog.set_directory(dir);
        }
        dialog
    }

    /// Écrit le nouveau fichier à l'emplacement choisi.
    fn create(&self, mut path: std::path::PathBuf) -> Result<std::path::PathBuf, String> {
        let balance = self.balance()?;
        let name = bp_core::manage::clean(self.name.trim());
        if path.extension().is_none() {
            path.set_extension("bp");
        }
        if path.exists() {
            return Err("Ce fichier existe déjà : choisissez un autre nom.".into());
        }
        let document = bp_core::manage::new_document(&name, balance);
        let protection = bp_core::format_v6::Protection::none().map_err(|e| e.to_string())?;
        bp_core::file::save(&path, &document, &protection).map_err(|e| e.to_string())?;
        Ok(path)
    }
}
