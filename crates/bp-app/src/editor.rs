//! Panneau de saisie et de modification d'une opération (à droite de la grille).
//! Remplace l'ancienne zone de saisie du bas de fenêtre.

use std::collections::BTreeSet;

use bp_core::model::{Amount, Date, Document, Mode, Operation, Tack};
use chrono::Months;
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::theme::palette;

pub struct Editor {
    /// Opération modifiée, ou `None` pour une nouvelle opération.
    pub target: Option<usize>,
    date: String,
    mode: Mode,
    payee: String,
    details: String,
    category: Option<u32>,
    amount: String,
    tack: Tack,
    /// Pointage d'origine : la date de pointage passe à aujourd'hui s'il change.
    original_tack: Option<Tack>,
    extra: Vec<(String, String)>,
    error: Option<String>,
    /// Tiers déjà utilisés dans le compte, pour l'auto-complétion.
    known_payees: Vec<String>,
    focus_requested: bool,
}

pub enum EditorAction {
    None,
    Save(Option<usize>, Operation),
    Close,
}

impl Editor {
    pub fn new_operation(document: &Document, account: usize) -> Self {
        Self {
            target: None,
            date: Date::today().to_string(),
            mode: Mode::default(),
            payee: String::new(),
            details: String::new(),
            category: None,
            amount: String::new(),
            tack: Tack::None,
            original_tack: None,
            extra: Vec::new(),
            error: None,
            known_payees: known_payees(document, account),
            focus_requested: true,
        }
    }

    pub fn edit(document: &Document, account: usize, index: usize) -> Self {
        let op = &document.accounts[account].operations[index];
        Self {
            target: Some(index),
            date: op.date.to_string(),
            mode: op.mode.clone(),
            payee: op.payee.clone(),
            details: op.details.clone(),
            category: op.category,
            amount: op.amount.format(2, ','),
            tack: op.tack,
            original_tack: Some(op.tack),
            extra: op.extra.clone(),
            error: None,
            known_payees: known_payees(document, account),
            focus_requested: true,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, document: &Document) -> EditorAction {
        let pal = palette(ui);
        let mut action = EditorAction::None;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let title = if self.target.is_some() { "Modifier l'opération" } else { "Nouvelle opération" };
            ui.label(RichText::new(title).heading());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(icon::X).on_hover_text("Fermer (Échap)").clicked() {
                    action = EditorAction::Close;
                }
            });
        });
        ui.add_space(8.0);

        egui::Grid::new("editor").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
            ui.label("Date");
            let date = ui.add(crate::theme::field(&mut self.date).hint_text("jj/mm/aaaa").desired_width(120.0));
            if self.focus_requested {
                date.request_focus();
                self.focus_requested = false;
            }
            if date.has_focus() {
                self.date_keys(ui);
            }
            ui.end_row();

            ui.label("Mode");
            egui::ComboBox::from_id_salt("mode")
                .selected_text(self.mode.label())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for mode in document.mode_list() {
                        let label = mode.label().to_string();
                        ui.selectable_value(&mut self.mode, mode, label);
                    }
                });
            ui.end_row();

            ui.label("Tiers");
            ui.vertical(|ui| {
                let response = ui.add(crate::theme::field(&mut self.payee).desired_width(f32::INFINITY));
                if response.has_focus() && !self.payee.is_empty() {
                    self.payee_suggestions(ui, document);
                }
            });
            ui.end_row();

            ui.label("Détail");
            ui.add(crate::theme::field(&mut self.details).desired_width(f32::INFINITY));
            ui.end_row();

            ui.label("Catégorie");
            let selected = self.category.and_then(|id| document.category_label(id)).unwrap_or_else(|| "(aucune)".into());
            egui::ComboBox::from_id_salt("category")
                .selected_text(selected)
                .width(ui.available_width())
                .height(400.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.category, None, "(aucune)");
                    for c in &document.categories {
                        let label = if c.is_child { format!("      {}", c.name) } else { c.name.clone() };
                        let text = if c.is_child { RichText::new(label) } else { RichText::new(label).family(crate::theme::semibold()) };
                        ui.selectable_value(&mut self.category, Some(c.id), text);
                    }
                });
            ui.end_row();

            ui.label("Montant");
            let hint = self.sign_hint(document);
            ui.add(crate::theme::field(&mut self.amount).hint_text(hint).desired_width(140.0));
            ui.end_row();

            ui.label("Pointage");
            ui.horizontal(|ui| {
                // Contrôle segmenté : trois choix de même largeur.
                ui.spacing_mut().item_spacing.x = 4.0;
                let width = (ui.available_width() - 8.0) / 3.0;
                for (tack, label) in [
                    (Tack::None, "Non".to_string()),
                    (Tack::Pointed, format!("{} Pointée", icon::CHECK)),
                    (Tack::Reconciled, format!("{} Rappr.", icon::CHECKS)),
                ] {
                    let button = egui::Button::selectable(self.tack == tack, label);
                    if ui.add_sized([width, 32.0], button).on_hover_text(tack_label(tack)).clicked() {
                        self.tack = tack;
                    }
                }
            });
            ui.end_row();
        });

        let visible_extra: Vec<_> = self.extra.iter().filter(|(k, _)| !k.starts_with('#') && k != "freeicon").collect();
        if !visible_extra.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Champs supplémentaires").small().family(crate::theme::medium()).color(pal.muted));
            for (k, v) in visible_extra {
                ui.label(format!("{k} : {v}"));
            }
        }

        if let Some(error) = &self.error {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
        }

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            let save = crate::theme::primary_button(ui, format!("{}  Enregistrer", icon::CHECK));
            if ui.add(save).on_hover_text("Ctrl+Entrée").clicked() {
                action = self.validate(document);
            }
            if ui.button("Annuler").clicked() {
                action = EditorAction::Close;
            }
        });
        let help = format!(
            "Ctrl+Entrée pour enregistrer, Échap pour fermer. Dans la date : {up}/{down} ±1 jour, Ctrl+{up}/{down} ±1 mois.",
            up = icon::ARROW_UP,
            down = icon::ARROW_DOWN
        );
        ui.label(RichText::new(help).small().color(pal.muted));

        let (enter, escape) = ui.input(|i| (i.modifiers.command && i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        if enter {
            action = self.validate(document);
        } else if escape {
            action = EditorAction::Close;
        }
        action
    }

    fn sign_hint(&self, document: &Document) -> &'static str {
        if mode_is_debit(document, &self.mode) { "dépense, ex. 12,50" } else { "recette, ex. 12,50" }
    }

    /// ↑/↓ (ou PgPréc/PgSuiv) : ±1 jour ; avec Ctrl : ±1 mois.
    fn date_keys(&mut self, ui: &egui::Ui) {
        let (up, down, ctrl) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::PageUp),
                i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::PageDown),
                i.modifiers.command,
            )
        });
        if !(up || down) {
            return;
        }
        let Some(date) = Date::parse_fr(&self.date, Date::today()) else { return };
        let naive = date.to_naive();
        let moved = match (up, ctrl) {
            (true, false) => naive.succ_opt(),
            (false, false) => naive.pred_opt(),
            (true, true) => naive.checked_add_months(Months::new(1)),
            (false, true) => naive.checked_sub_months(Months::new(1)),
        };
        if let Some(moved) = moved {
            self.date = Date::from_naive(moved).to_string();
        }
    }

    fn payee_suggestions(&mut self, ui: &mut egui::Ui, document: &Document) {
        let needle = bp_core::filter::normalize(&self.payee);
        let needle = needle.trim();
        let memorized: Vec<_> = document
            .payees
            .iter()
            .filter(|p| bp_core::filter::normalize(&p.caption).contains(needle))
            .take(5)
            .cloned()
            .collect();
        let used: Vec<String> = self
            .known_payees
            .iter()
            .filter(|p| bp_core::filter::normalize(p).contains(needle) && **p != self.payee)
            .take(6)
            .cloned()
            .collect();
        if memorized.is_empty() && used.is_empty() {
            return;
        }
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            for p in memorized {
                let text = format!("{}  {}", icon::ADDRESS_BOOK, p.caption);
                if ui.selectable_label(false, text).on_hover_text("Tiers mémorisé : remplit aussi mode, catégorie, détail et montant").clicked() {
                    self.payee = p.payee.clone();
                    self.mode = p.mode.clone();
                    if p.category.is_some() {
                        self.category = p.category;
                    }
                    if !p.details.is_empty() {
                        self.details = p.details.clone();
                    }
                    if Amount::parse(&p.amount).is_some_and(|a| a != Amount::ZERO) {
                        self.amount = p.amount.clone();
                    }
                }
            }
            for name in used {
                if ui.selectable_label(false, &name).clicked() {
                    self.payee = name;
                }
            }
        });
    }

    fn validate(&mut self, document: &Document) -> EditorAction {
        let today = Date::today();
        let Some(date) = Date::parse_fr(&self.date, today) else {
            self.error = Some("Date invalide (jj/mm/aaaa)".into());
            return EditorAction::None;
        };
        let text = self.amount.trim();
        let Some(mut amount) = Amount::parse(text) else {
            self.error = Some("Montant invalide".into());
            return EditorAction::None;
        };
        // Sans signe explicite, le signe vient du mode de paiement.
        if !text.starts_with(['-', '+']) && mode_is_debit(document, &self.mode) {
            amount = -amount;
        }
        let payee = self.payee.replace(';', " ");
        let details = self.details.replace(';', " ");
        self.error = None;
        EditorAction::Save(
            self.target,
            Operation {
                uid: 0,
                date,
                mark_date: match self.original_tack {
                    Some(original) if original != self.tack => today,
                    _ => date,
                },
                tack: self.tack,
                mode: self.mode.clone(),
                payee,
                details,
                category: self.category,
                amount,
                extra: self.extra.clone(),
            },
        )
    }
}

fn tack_label(tack: Tack) -> &'static str {
    match tack {
        Tack::None => "Non pointée",
        Tack::Pointed => "Pointée",
        Tack::Reconciled => "Rapprochée",
    }
}

fn mode_is_debit(document: &Document, mode: &Mode) -> bool {
    match mode {
        Mode::Standard(m) => m.is_debit(),
        Mode::Custom(name) => document.custom_modes.iter().find(|m| &m.name == name).is_none_or(|m| m.debit),
    }
}

fn known_payees(document: &Document, account: usize) -> Vec<String> {
    let set: BTreeSet<&str> = document.accounts[account]
        .operations
        .iter()
        .rev()
        .map(|op| op.payee.as_str())
        .filter(|p| !p.is_empty())
        .collect();
    set.into_iter().map(str::to_string).collect()
}
