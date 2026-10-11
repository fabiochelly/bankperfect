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
        let theme = &mut self.theme;
        ui_kit::section(ui, width, icon::PALETTE, "Apparence", |ui| {
            ui_kit::row(ui, "Mode", LABEL_WIDTH, |ui| {
                let w = ui.available_width().min(460.0);
                let changed = ui_kit::segmented(
                    ui,
                    theme,
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
                if changed {
                    ui.ctx().set_theme(*theme);
                }
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
        ui_kit::section(ui, width, icon::SWATCHES, "Thèmes", |ui| {
            for (dark, title) in [(true, "Sombres"), (false, "Clairs")] {
                ui.label(RichText::new(title).color(pal.muted).size(12.5));
                ui.add_space(4.0);
                if let Some(id) = theme_gallery(ui, d, dark) {
                    if dark {
                        d.dark_theme = id.into();
                    } else {
                        d.light_theme = id.into();
                    }
                    // Le thème choisi s'affiche, sauf s'il l'est déjà en
                    // suivant le système.
                    let ctx = ui.ctx().clone();
                    let shown = ctx.theme()
                        == if dark {
                            egui::Theme::Dark
                        } else {
                            egui::Theme::Light
                        };
                    if !(*theme == egui::ThemePreference::System && shown) {
                        *theme = if dark {
                            egui::ThemePreference::Dark
                        } else {
                            egui::ThemePreference::Light
                        };
                    }
                    crate::theme::select(&ctx, &d.dark_theme, &d.light_theme);
                    ctx.set_theme(*theme);
                }
                ui.add_space(10.0);
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

/// Vignettes des thèmes sombres ou clairs ; renvoie le thème cliqué.
fn theme_gallery(ui: &mut egui::Ui, d: &Settings, dark: bool) -> Option<&'static str> {
    const GAP: f32 = 12.0;
    const MIN_WIDTH: f32 = 168.0;
    let selected = if dark { &d.dark_theme } else { &d.light_theme };
    let selected = crate::theme::find(selected, dark).id;
    let themes: Vec<_> = crate::theme::THEMES
        .iter()
        .filter(|t| t.dark == dark)
        .collect();
    let available = ui.available_width();
    let columns = ((available + GAP) / (MIN_WIDTH + GAP)).floor().max(1.0) as usize;
    let card_width = (available - GAP * (columns - 1) as f32) / columns as f32;
    let mut clicked = None;
    for row in themes.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for theme in row {
                if theme_card(ui, theme, theme.id == selected, card_width).clicked() {
                    clicked = Some(theme.id);
                }
            }
        });
        ui.add_space(GAP - ui.spacing().item_spacing.y);
    }
    clicked
}

/// Vignette d'un thème : fenêtre miniature (comptes, grille rayée, montants,
/// trait de changement de mois) et nom.
fn theme_card(
    ui: &mut egui::Ui,
    theme: &crate::theme::ThemeDef,
    selected: bool,
    width: f32,
) -> egui::Response {
    use egui::{Rect, Stroke, StrokeKind, pos2, vec2};
    let ui_pal = palette(ui);
    let p = &theme.palette;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 112.0), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, theme.name)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    let border = if selected {
        Stroke::new(2.0, ui_pal.accent)
    } else if response.hovered() {
        Stroke::new(1.0, ui_pal.accent.gamma_multiply(0.6))
    } else {
        Stroke::new(1.0, ui_pal.border)
    };
    painter.rect(rect, 10.0, ui_pal.bg_dark, border, StrokeKind::Inside);

    // Fenêtre miniature.
    let window = Rect::from_min_size(rect.min + vec2(7.0, 7.0), vec2(rect.width() - 14.0, 70.0));
    painter.rect_filled(window, 6.0, p.bg);
    let sidebar = Rect::from_min_max(
        window.min,
        pos2(window.left() + window.width() * 0.27, window.bottom()),
    );
    painter.rect_filled(
        sidebar,
        egui::CornerRadius {
            nw: 6,
            sw: 6,
            ne: 0,
            se: 0,
        },
        p.bg_dark,
    );
    for k in 0..4 {
        let y = sidebar.top() + 10.0 + k as f32 * 12.0;
        let w = sidebar.width() * if k == 0 { 0.6 } else { 0.45 };
        let color = if k == 0 { p.accent } else { p.muted };
        painter.rect_filled(
            Rect::from_min_size(pos2(sidebar.left() + 7.0, y), vec2(w, 4.0)),
            2.0,
            color,
        );
    }
    let grid = Rect::from_min_max(pos2(sidebar.right(), window.top() + 14.0), window.max);
    // Bouton d'accent dans la barre du haut.
    painter.rect_filled(
        Rect::from_min_size(
            pos2(window.right() - 38.0, window.top() + 4.0),
            vec2(30.0, 7.0),
        ),
        3.5,
        p.accent,
    );
    let rows = 4;
    let row_h = grid.height() / rows as f32;
    for k in 0..rows {
        let r = Rect::from_min_size(
            pos2(grid.left(), grid.top() + k as f32 * row_h),
            vec2(grid.width(), row_h),
        );
        let fill = match k {
            1 => p.selection,
            _ if k % 2 == 0 => p.stripe,
            _ => p.bg,
        };
        let corner = if k == rows - 1 {
            egui::CornerRadius {
                se: 6,
                ..Default::default()
            }
        } else {
            egui::CornerRadius::ZERO
        };
        painter.rect_filled(r, corner, fill);
        let cy = r.center().y;
        painter.rect_filled(
            Rect::from_center_size(pos2(r.left() + 8.0 + 3.0, cy), vec2(6.0, 6.0)),
            3.0,
            if k == 2 { p.future } else { p.pointed },
        );
        let text_w = r.width() * [0.38, 0.30, 0.42, 0.26][k];
        painter.rect_filled(
            Rect::from_min_size(pos2(r.left() + 20.0, cy - 2.0), vec2(text_w, 4.0)),
            2.0,
            p.fg_dark,
        );
        painter.rect_filled(
            Rect::from_min_size(pos2(r.right() - 34.0, cy - 2.0), vec2(26.0, 4.0)),
            2.0,
            if k == 3 { p.positive } else { p.negative },
        );
    }
    // Trait rouge d'un changement de mois.
    let y = grid.top() + 3.0 * row_h;
    painter.line_segment(
        [pos2(grid.left(), y), pos2(grid.right(), y)],
        Stroke::new(1.0, p.month_rule),
    );

    // Nom.
    let name_y = window.bottom() + (rect.bottom() - window.bottom()) / 2.0;
    let family = if selected {
        crate::theme::semibold()
    } else {
        crate::theme::medium()
    };
    painter.text(
        pos2(rect.left() + 12.0, name_y),
        egui::Align2::LEFT_CENTER,
        theme.name,
        egui::FontId::new(13.0, family),
        if selected { ui_pal.fg } else { ui_pal.fg_dark },
    );
    if selected {
        painter.text(
            pos2(rect.right() - 12.0, name_y),
            egui::Align2::RIGHT_CENTER,
            icon::CHECK_CIRCLE,
            egui::FontId::proportional(16.0),
            ui_pal.accent,
        );
    }
    response
}
