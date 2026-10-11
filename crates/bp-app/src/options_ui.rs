//! Fenêtre « Options » : saisie et calculs, fichiers et sauvegardes,
//! affichage. Les modifications ne sont appliquées qu'avec « Enregistrer ».

use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::settings::{DateFormat, Settings};
use crate::theme::palette;
use crate::ui_kit;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Entry,
    Files,
    Display,
}

pub struct OptionsForm {
    tab: Tab,
    /// Copie de travail des réglages.
    draft: Settings,
    theme: egui::ThemePreference,
}

pub enum OptionsOutcome {
    Keep,
    Cancel,
    Apply(Box<Settings>, egui::ThemePreference),
}

const LABEL_WIDTH: f32 = 230.0;

impl OptionsForm {
    pub fn new(settings: &Settings, theme: egui::ThemePreference) -> Self {
        Self {
            tab: Tab::Entry,
            draft: settings.clone(),
            theme,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> OptionsOutcome {
        let mut outcome = OptionsOutcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.55, 720.0, 900.0);
        let height = ui_kit::dialog_height(ctx, 0.75).clamp(520.0, 720.0);
        crate::dialogs::modal(ctx, "options").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(220.0, 36.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| crate::dialogs::header(ui, icon::SLIDERS, pal.accent, "Options"),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui_kit::segmented(
                        ui,
                        &mut self.tab,
                        &[
                            (
                                Tab::Entry,
                                icon::PENCIL_SIMPLE,
                                "Saisie et calculs",
                                pal.accent,
                            ),
                            (Tab::Files, icon::FOLDER_SIMPLE, "Fichiers", pal.accent),
                            (Tab::Display, icon::PAINT_BRUSH, "Affichage", pal.accent),
                        ],
                        38.0,
                        Some((width - 240.0).min(560.0)),
                    );
                });
            });
            ui.add_space(18.0);
            let body = height - 36.0 - 18.0 - 80.0;
            egui::ScrollArea::vertical()
                .max_height(body)
                .min_scrolled_height(body)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let w = width - 16.0;
                    ui.set_max_width(w);
                    match self.tab {
                        Tab::Entry => self.entry(ui, w),
                        Tab::Files => self.files(ui, w),
                        Tab::Display => self.display(ui, w),
                    }
                });
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, "Enregistrer", true).clicked() {
                    outcome = OptionsOutcome::Apply(Box::new(self.draft.clone()), self.theme);
                }
                if crate::dialogs::button(ui, "Annuler").clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    outcome = OptionsOutcome::Cancel;
                }
            });
        });
        outcome
    }

    fn entry(&mut self, ui: &mut egui::Ui, width: f32) {
        let pal = palette(ui);
        let d = &mut self.draft;
        ui_kit::section(ui, width, icon::PENCIL_SIMPLE, "Saisie", |ui| {
            ui_kit::segmented(
                ui,
                &mut d.entry_at_bottom,
                &[
                    (false, icon::SIDEBAR_SIMPLE, "Panneau à droite", pal.accent),
                    (true, icon::ROWS, "Barre en bas", pal.accent),
                ],
                34.0,
                Some(420.0),
            );
            ui.add_space(10.0);
            crate::theme::checkbox(
                ui,
                &mut d.autocomplete,
                "Proposer les tiers déjà utilisés pendant la saisie",
            );
            ui.add_space(8.0);
            crate::theme::checkbox(
                ui,
                &mut d.amounts_in_cents,
                "Montants tapés en centimes (1250 donne 12,50)",
            );
            ui.add_space(8.0);
            crate::theme::checkbox(
                ui,
                &mut d.keep_entry_open,
                "Saisie en série : le formulaire reste ouvert et vidé après chaque ajout",
            );
        });
        ui_kit::section(ui, width, icon::CALCULATOR, "Pointage et soldes", |ui| {
            crate::theme::checkbox(
                ui,
                &mut d.disable_reconcile,
                "Pointage simple, sans rapprochement (une ligne est pointée ou non)",
            );
            ui.add_space(8.0);
            crate::theme::checkbox(
                ui,
                &mut d.ignore_filter_totals,
                "Soldes calculés sur toutes les lignes, même quand un filtre est actif",
            );
        });
        ui_kit::section(
            ui,
            width,
            icon::ARROW_U_UP_LEFT,
            "Historique et tri",
            |ui| {
                ui_kit::row(ui, "Niveaux d'annulation", LABEL_WIDTH, |ui| {
                    ui.add(egui::Slider::new(&mut d.max_undo, 1..=100));
                });
                ui.add_space(8.0);
                crate::theme::checkbox(
                    ui,
                    &mut d.remember_sort,
                    "Retrouver la colonne de tri à l'ouverture",
                );
            },
        );
        let _ = pal;
    }

    fn files(&mut self, ui: &mut egui::Ui, width: f32) {
        let pal = palette(ui);
        let d = &mut self.draft;
        ui_kit::section(ui, width, icon::FLOPPY_DISK, "Enregistrement", |ui| {
            crate::theme::checkbox(
                ui,
                &mut d.autosave_on_close,
                "Enregistrer automatiquement à la fermeture, sans demander",
            );
        });
        let folder_row = |ui: &mut egui::Ui,
                          label: &str,
                          value: &mut Option<std::path::PathBuf>,
                          default: &str| {
            ui_kit::row(ui, label, LABEL_WIDTH, |ui| {
                let text = value
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| default.to_string());
                let color = if value.is_some() { pal.fg } else { pal.muted };
                let w = ui.available_width() - 150.0;
                ui.add_sized(
                    [w, 34.0],
                    egui::Label::new(RichText::new(text).color(color)).truncate(),
                );
                if ui
                    .button(format!("{}  Choisir…", icon::FOLDER_OPEN))
                    .clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder()
                {
                    *value = Some(dir);
                }
                if value.is_some()
                    && ui
                        .add(
                            egui::Button::new(RichText::new(icon::X).color(pal.muted)).frame(false),
                        )
                        .on_hover_text("Revenir au dossier par défaut")
                        .clicked()
                {
                    *value = None;
                }
            });
        };
        ui_kit::section(ui, width, icon::FOLDERS, "Dossiers", |ui| {
            folder_row(
                ui,
                "Fichiers de comptes",
                &mut d.files_dir,
                "Dernier dossier utilisé",
            );
            ui.add_space(8.0);
            folder_row(
                ui,
                "Exports (CSV, QIF…)",
                &mut d.export_dir,
                "Dossier du fichier de comptes",
            );
            ui.add_space(8.0);
            folder_row(
                ui,
                "Sauvegardes (.bp0)",
                &mut d.backup_dir,
                "Dossier de données de l'application",
            );
        });
        ui_kit::section(
            ui,
            width,
            icon::CLOCK_COUNTER_CLOCKWISE,
            "Sauvegardes",
            |ui| {
                ui_kit::row(ui, "Sauvegardes gardées par fichier", LABEL_WIDTH, |ui| {
                    ui.add(egui::Slider::new(&mut d.backup_keep, 1..=100));
                });
                ui.label(
                RichText::new("Une copie est faite avant chaque enregistrement ; les plus anciennes sont supprimées.")
                    .color(pal.muted)
                    .size(12.5),
            );
            },
        );
    }

    fn display(&mut self, ui: &mut egui::Ui, width: f32) {
        let pal = palette(ui);
        let d = &mut self.draft;
        ui_kit::section(ui, width, icon::PALETTE, "Apparence", |ui| {
            ui_kit::row(ui, "Thème", LABEL_WIDTH, |ui| {
                let w = ui.available_width().min(460.0);
                ui_kit::segmented(
                    ui,
                    &mut self.theme,
                    &[
                        (
                            egui::ThemePreference::Dark,
                            icon::MOON,
                            "Sombre",
                            pal.accent,
                        ),
                        (egui::ThemePreference::Light, icon::SUN, "Clair", pal.accent),
                        (
                            egui::ThemePreference::System,
                            icon::DESKTOP,
                            "Système",
                            pal.accent,
                        ),
                    ],
                    34.0,
                    Some(w),
                );
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Taille du texte", LABEL_WIDTH, |ui| {
                ui.add(
                    egui::Slider::new(&mut d.text_scale, 0.8..=1.5)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0))
                        .step_by(0.05),
                );
            });
            ui.add_space(8.0);
            let mut translucent = d.translucent();
            if crate::theme::checkbox(ui, &mut translucent, "Fond translucide (flou du bureau)")
                .changed()
            {
                d.translucent = Some(translucent);
            }
        });
        ui_kit::section(ui, width, icon::CALENDAR_BLANK, "Dates", |ui| {
            ui_kit::row(ui, "Format des dates", LABEL_WIDTH, |ui| {
                let sample = bp_core::model::Date::today();
                egui::ComboBox::from_id_salt("date_format")
                    .selected_text(d.date_format.format(sample))
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for format in DateFormat::ALL {
                            ui.selectable_value(&mut d.date_format, format, format.format(sample));
                        }
                    });
            });
        });
    }
}
