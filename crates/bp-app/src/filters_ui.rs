//! Filtres prédéfinis (F7) : jeux nommés de critères de la grille, aux
//! dates éventuellement dynamiques (`1/m/a`, `dj/m-1/a`…).

use bp_core::autofilters::{AutoFilter, AutoFilters};
use bp_core::filter::TackFilter;
use bp_core::model::Date;
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

const LABEL_WIDTH: f32 = 150.0;

/// Filtres prédéfinis du document (ou ceux par défaut).
pub fn load(doc: &OpenDoc) -> AutoFilters {
    AutoFilters::load(&doc.document, &bp_core::legacy_pickle::parse_py)
        .unwrap_or_else(|_| AutoFilters::defaults())
}

/// Applique un filtre prédéfini à la grille (l'option « cacher les lignes
/// inutiles » est conservée).
pub fn apply(doc: &mut OpenDoc, filter: &AutoFilter) {
    let hide = doc.filters.hide_old_lines;
    doc.filters = filter.to_filters(Date::today());
    doc.filters.hide_old_lines = hide;
}

/// Filtre prédéfini reprenant les critères actuels de la grille.
pub fn from_current(doc: &OpenDoc, name: &str) -> AutoFilter {
    let f = &doc.filters;
    AutoFilter {
        name: name.to_string(),
        date_from: f.date_from.map(|d| d.to_string()).unwrap_or_default(),
        date_to: f.date_to.map(|d| d.to_string()).unwrap_or_default(),
        mode: f.mode.clone(),
        payee: f.payee.clone(),
        category: f.category.clone(),
        amount: f.amount.clone(),
        tack: f.tack,
    }
}

pub struct FiltersForm {
    filters: AutoFilters,
    selected: Option<String>,
    draft: AutoFilter,
    error: Option<String>,
}

impl FiltersForm {
    /// `save_current` : propose d'enregistrer les critères actuels.
    pub fn new(doc: &OpenDoc, save_current: bool) -> Self {
        let filters = load(doc);
        let mut form = Self {
            selected: None,
            draft: AutoFilter::new(""),
            error: None,
            filters,
        };
        if save_current {
            form.draft = from_current(doc, "Nouveau filtre");
        } else if let Some(first) = form.filters.names().first().map(|s| s.to_string()) {
            form.select(&first);
        }
        form
    }

    fn select(&mut self, name: &str) {
        if let Some(f) = self.filters.get(name) {
            self.draft = f.clone();
            self.selected = Some(name.to_string());
            self.error = None;
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.6, 820.0, 1100.0);
        crate::dialogs::modal(ctx, "auto_filters").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::FUNNEL, pal.accent, "Filtres prédéfinis");
            ui_kit::subtitle(
                ui,
                "Des jeux de critères à appliquer en un clic à la grille. Les dates peuvent suivre le calendrier.",
            );
            let gap = 22.0;
            let list_width = (width * 0.32).clamp(240.0, 340.0);
            let detail_width = width - list_width - gap;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.vertical(|ui| {
                    ui.set_width(list_width);
                    self.list(ui, doc, list_width);
                });
                ui.vertical(|ui| {
                    ui.set_width(detail_width);
                    // Hauteur bornée : la fenêtre tient dans l'écran.
                    egui::ScrollArea::vertical()
                        .id_salt("filter_detail")
                        .max_height(ui_kit::dialog_height(ui.ctx(), 0.62))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.set_max_width(detail_width - 16.0);
                            self.detail(ui, detail_width - 16.0);
                        });
                });
            });
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, &format!("{}  Appliquer", icon::FUNNEL), true).clicked() {
                    match self.draft.validate() {
                        Ok(()) => {
                            apply(doc, &self.draft);
                            outcome = Outcome::Done(if self.draft.name.is_empty() {
                                "Filtre appliqué".into()
                            } else {
                                format!("Filtre « {} » appliqué", self.draft.name)
                            });
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if crate::dialogs::button(ui, format!("{}  Enregistrer", icon::FLOPPY_DISK)).clicked() {
                    self.save(doc);
                }
                if crate::dialogs::button(ui, "Fermer").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            });
        });
        outcome
    }

    fn save(&mut self, doc: &mut OpenDoc) {
        let name = bp_core::autofilters::clean_name(&self.draft.name);
        if name.is_empty() {
            self.error = Some("Donnez un nom au filtre.".into());
            return;
        }
        self.draft.name = name.clone();
        if let Some(old) = self.selected.clone()
            && old != name
        {
            self.filters.remove(&old);
        }
        match self.filters.upsert(self.draft.clone()) {
            Ok(()) => {
                if self.filters.save(&mut doc.document).is_ok() {
                    doc.dirty = true;
                }
                self.selected = Some(name);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn list(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32) {
        let pal = palette(ui);
        let names: Vec<String> = self.filters.names().iter().map(|s| s.to_string()).collect();
        let mut clicked = None;
        ui_kit::card(ui, width, |ui| {
            if names.is_empty() {
                ui.label(RichText::new("Aucun filtre enregistré.").color(pal.muted));
            }
            for name in &names {
                let selected = self.selected.as_deref() == Some(name.as_str());
                if ui
                    .selectable_label(selected, format!("{}  {name}", icon::FUNNEL_SIMPLE))
                    .clicked()
                {
                    clicked = Some(name.clone());
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(format!("{}  Nouveau", icon::PLUS)).clicked() {
                    self.draft = AutoFilter::new("Nouveau filtre");
                    self.selected = None;
                }
                if ui
                    .button(format!("{}  Filtre actuel", icon::COPY))
                    .on_hover_text("Reprendre les critères de la grille")
                    .clicked()
                {
                    self.draft = from_current(doc, "Nouveau filtre");
                    self.selected = None;
                }
                if let Some(name) = self.selected.clone()
                    && ui
                        .add(egui::Button::new(
                            RichText::new(icon::TRASH).color(pal.negative),
                        ))
                        .on_hover_text("Supprimer")
                        .clicked()
                {
                    self.filters.remove(&name);
                    if self.filters.save(&mut doc.document).is_ok() {
                        doc.dirty = true;
                    }
                    self.selected = None;
                    self.draft = AutoFilter::new("");
                }
            });
        });
        if let Some(name) = clicked {
            self.select(&name);
        }
    }

    fn detail(&mut self, ui: &mut egui::Ui, width: f32) {
        let pal = palette(ui);
        let full = width - 2.0 * ui_kit::CARD_PADDING - 2.0 - LABEL_WIDTH - 8.0;
        let d = &mut self.draft;
        ui_kit::section(ui, width, icon::TAG, "Filtre", |ui| {
            ui_kit::row(ui, "Nom", LABEL_WIDTH, |ui| {
                ui.add(crate::theme::field(&mut d.name).desired_width(full));
            });
        });
        ui_kit::section(ui, width, icon::CALENDAR_BLANK, "Période", |ui| {
            let today = Date::today();
            for (label, value) in [("Du", &mut d.date_from), ("Au", &mut d.date_to)] {
                ui_kit::row(ui, label, LABEL_WIDTH, |ui| {
                    ui.add(
                        crate::theme::field(value)
                            .hint_text("vide, 15/01/2026 ou 1/m/a")
                            .desired_width(150.0),
                    );
                    match bp_core::autofilters::eval_dynamic_date(value, today) {
                        Ok(Some(date)) => {
                            ui_kit::pill(ui, &format!("aujourd'hui : {date}"), pal.accent)
                        }
                        Ok(None) => {}
                        Err(_) => ui_kit::pill(ui, "date incorrecte", pal.negative),
                    }
                });
                ui.add_space(6.0);
            }
            ui.label(
                RichText::new(
                    "Dates dynamiques jour/mois/année avec j, m, a (aujourd'hui) et dj (dernier jour du mois) : 1/m/a = début du mois, dj/m-1/a = fin du mois dernier, j-30/m/a = il y a 30 jours.",
                )
                .size(12.5)
                .color(pal.muted),
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (label, from, to) in [
                    ("Ce mois", "1/m/a", "dj/m/a"),
                    ("Mois dernier", "1/m-1/a", "dj/m-1/a"),
                    ("Cette année", "1/1/a", "31/12/a"),
                    ("30 derniers jours", "j-30/m/a", "j/m/a"),
                ] {
                    if ui.small_button(label).clicked() {
                        d.date_from = from.into();
                        d.date_to = to.into();
                    }
                }
            });
        });
        ui_kit::section(ui, width, icon::SLIDERS_HORIZONTAL, "Critères", |ui| {
            for (label, value, hint) in [
                ("Mode", &mut d.mode, "carte, chq…"),
                ("Tiers ou détail", &mut d.payee, "texte cherché"),
                ("Catégorie", &mut d.category, "nom de catégorie"),
                ("Montant", &mut d.amount, "<0, -50>50…"),
            ] {
                ui_kit::row(ui, label, LABEL_WIDTH, |ui| {
                    ui.add(
                        crate::theme::field(value)
                            .hint_text(hint)
                            .desired_width(full),
                    );
                });
                ui.add_space(6.0);
            }
            ui_kit::row(ui, "Pointage", LABEL_WIDTH, |ui| {
                egui::ComboBox::from_id_salt("filter_tack")
                    .selected_text(d.tack.label())
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for t in TackFilter::ALL {
                            ui.selectable_value(&mut d.tack, t, t.label());
                        }
                    });
            });
        });
        let description = d.description(Date::today());
        if !description.is_empty() {
            ui.label(RichText::new(format!("{}  {description}", icon::INFO)).color(pal.fg_dark));
        }
    }
}
