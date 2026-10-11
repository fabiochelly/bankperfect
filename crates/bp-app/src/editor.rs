//! Panneau de saisie et de modification d'une opération (à droite de la grille).
//! Remplace l'ancienne zone de saisie du bas de fenêtre.

use std::collections::BTreeSet;

use bp_core::model::{Amount, Date, Document, Mode, Operation, Tack};
use chrono::Months;
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
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
    /// Sens de l'opération : recette (positif) ou dépense.
    credit: bool,
    /// Pointage d'origine : la date de pointage passe à aujourd'hui s'il change.
    original_tack: Option<Tack>,
    extra: Vec<(String, String)>,
    error: Option<String>,
    /// Tiers déjà utilisés dans le compte, pour l'auto-complétion.
    known_payees: Vec<String>,
    focus_requested: bool,
    /// Le contenu dépasse la hauteur du panneau (barre de défilement).
    overflow: bool,
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
            credit: !mode_is_debit(document, &Mode::default()),
            original_tack: None,
            extra: Vec::new(),
            error: None,
            known_payees: known_payees(document, account),
            focus_requested: true,
            overflow: false,
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
            amount: op.amount.abs().format(2, ','),
            tack: op.tack,
            credit: !op.amount.is_negative(),
            original_tack: Some(op.tack),
            extra: op.extra.clone(),
            error: None,
            known_payees: known_payees(document, account),
            focus_requested: true,
            overflow: false,
        }
    }

    /// Saisie en série : la nouvelle opération reprend la date précédente.
    pub fn with_date(mut self, date: String) -> Self {
        self.date = date;
        self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, doc: &OpenDoc) -> EditorAction {
        let document = &doc.document;
        let pal = palette(ui);
        let mut action = EditorAction::None;

        // En-tête : titre, compte, fermeture.
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let title = if self.target.is_some() {
                    "Modifier l'opération"
                } else {
                    "Nouvelle opération"
                };
                ui.label(
                    RichText::new(title)
                        .size(20.0)
                        .family(crate::theme::semibold()),
                );
                ui.label(
                    RichText::new(format!("{}  {}", icon::BANK, doc.current_account().name))
                        .color(pal.muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui
                    .add(egui::Button::new(RichText::new(icon::X).size(16.0)).frame(false))
                    .on_hover_text("Fermer (Échap)")
                    .clicked()
                {
                    action = EditorAction::Close;
                }
            });
        });
        ui.add_space(10.0);

        let footer_height = 64.0;
        // Gouttière à droite pour la barre de défilement, seulement si le
        // contenu dépassait à l'image précédente.
        let card_width = ui.available_width() - if self.overflow { 14.0 } else { 0.0 };
        let scroll = egui::ScrollArea::vertical()
            .max_height((ui.available_height() - footer_height).max(120.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.set_max_width(card_width);
                self.amount_card(ui, doc, card_width);
                ui.add_space(12.0);

                section(
                    ui,
                    card_width,
                    icon::CALENDAR_BLANK,
                    "Quand et comment",
                    |ui| {
                        let half = (ui.available_width() - 12.0) / 2.0;
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let mut cols = [
                                ui.new_child(
                                    egui::UiBuilder::new()
                                        .layout(egui::Layout::top_down(egui::Align::Min))
                                        .max_rect(egui::Rect::from_min_size(
                                            ui.cursor().min,
                                            egui::vec2(half, 80.0),
                                        )),
                                ),
                                ui.new_child(
                                    egui::UiBuilder::new()
                                        .layout(egui::Layout::top_down(egui::Align::Min))
                                        .max_rect(egui::Rect::from_min_size(
                                            ui.cursor().min + egui::vec2(half + 12.0, 0.0),
                                            egui::vec2(half, 80.0),
                                        )),
                                ),
                            ];
                            let width = cols[0].available_width();
                            let date = crate::datepicker::date_field(
                                &mut cols[0],
                                "date",
                                &mut self.date,
                                width,
                                crate::datepicker::Placement::Popup,
                            );
                            let date = date.on_hover_text(format!(
                                "{up}/{down} : ±1 jour   Ctrl+{up}/{down} : ±1 mois",
                                up = icon::ARROW_UP,
                                down = icon::ARROW_DOWN
                            ));
                            if self.focus_requested {
                                date.request_focus();
                                self.focus_requested = false;
                            }
                            if date.has_focus() {
                                self.date_keys(&cols[0]);
                            }

                            let before = self.mode.clone();
                            let (glyph, color) =
                                crate::grid::mode_style(document, &self.mode, &pal);
                            let width = cols[1].available_width();
                            egui::ComboBox::from_id_salt("mode")
                                .selected_text(
                                    RichText::new(format!("{glyph}  {}", self.mode.label()))
                                        .color(color),
                                )
                                .width(width)
                                .height(420.0)
                                .show_ui(&mut cols[1], |ui| {
                                    for mode in document.mode_list() {
                                        let (glyph, color) =
                                            crate::grid::mode_style(document, &mode, &pal);
                                        let text =
                                            RichText::new(format!("{glyph}  {}", mode.label()))
                                                .color(color);
                                        ui.selectable_value(&mut self.mode, mode, text);
                                    }
                                });
                            if self.mode != before {
                                self.credit = !mode_is_debit(document, &self.mode);
                                // Chèque émis sur une nouvelle opération : numéro
                                // du chèque suivant proposé dans le détail.
                                if self.target.is_none()
                                    && self.details.trim().is_empty()
                                    && self.mode
                                        == Mode::Standard(bp_core::model::StandardMode::ChequeEmis)
                                {
                                    self.details =
                                        bp_core::edit::next_cheque_number(doc.current_account());
                                }
                            }
                            let height =
                                cols[0].min_rect().height().max(cols[1].min_rect().height());
                            ui.allocate_space(egui::vec2(2.0 * half + 12.0, height));
                        });
                    },
                );

                section(ui, card_width, icon::USER, "Qui et pourquoi", |ui| {
                    let response = ui.add(
                        crate::theme::field(&mut self.payee)
                            .hint_text("Commerçant, organisme, personne…")
                            .desired_width(f32::INFINITY),
                    );
                    if response.has_focus()
                        && !self.payee.is_empty()
                        && crate::settings::shared::autocomplete()
                    {
                        self.payee_suggestions(ui, document);
                    }
                    ui.add_space(14.0);
                    ui.add(
                        crate::theme::field(&mut self.details)
                            .hint_text("Note, référence, n° de chèque…")
                            .desired_width(f32::INFINITY),
                    );
                });

                section(ui, card_width, icon::TAG, "Classement", |ui| {
                    let style = self
                        .category
                        .and_then(|id| doc.category_styles.get(&id).copied());
                    let selected = match self.category.and_then(|id| document.category_label(id)) {
                        Some(label) => {
                            let color = style
                                .map_or(pal.fg, |s| crate::theme::category_color(&pal, s.color));
                            let glyph = style.map_or(icon::TAG, |s| s.icon);
                            RichText::new(format!("{glyph}  {label}")).color(color)
                        }
                        None => RichText::new("Aucune catégorie").color(pal.muted),
                    };
                    egui::ComboBox::from_id_salt("category")
                        .selected_text(selected)
                        .width(ui.available_width())
                        .height(420.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.category, None, "Aucune catégorie");
                            for c in &document.categories {
                                let style = doc.category_styles.get(&c.id).copied();
                                let glyph = style.map_or(icon::TAG, |s| s.icon);
                                let indent = if c.is_child { "      " } else { "" };
                                let mut text =
                                    RichText::new(format!("{indent}{glyph}  {}", c.name));
                                if !c.is_child {
                                    text = text.family(crate::theme::semibold());
                                }
                                if let Some(s) = style {
                                    text = text.color(crate::theme::category_color(&pal, s.color));
                                }
                                ui.selectable_value(&mut self.category, Some(c.id), text);
                            }
                        });
                    ui.add_space(14.0);
                    crate::ui_kit::segmented(
                        ui,
                        &mut self.tack,
                        &[
                            (Tack::None, icon::CIRCLE_DASHED, "À pointer", pal.fg),
                            (Tack::Pointed, icon::CHECK, "Pointée", pal.pointed),
                            (Tack::Reconciled, icon::CHECKS, "Rapprochée", pal.reconciled),
                        ],
                        32.0,
                        None,
                    );
                });

                let visible_extra: Vec<_> = self
                    .extra
                    .iter()
                    .filter(|(k, _)| !k.starts_with('#') && k != "freeicon")
                    .collect();
                if !visible_extra.is_empty() {
                    section(
                        ui,
                        card_width,
                        icon::LIST_PLUS,
                        "Champs supplémentaires",
                        |ui| {
                            egui::Grid::new("editor_extra")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    for (k, v) in visible_extra {
                                        ui.label(RichText::new(k).color(pal.muted));
                                        ui.label(v);
                                        ui.end_row();
                                    }
                                });
                        },
                    );
                }
            });
        self.overflow = scroll.content_size.y > scroll.inner_rect.height() + 1.0;

        // Pied : erreur et boutons, toujours visibles en bas du panneau.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save =
                        crate::theme::primary_button(ui, format!("{}  Enregistrer", icon::CHECK))
                            .min_size(egui::vec2(140.0, 38.0));
                    if ui.add(save).on_hover_text("Ctrl+Entrée").clicked() {
                        action = self.validate();
                    }
                    if ui
                        .add(egui::Button::new("Annuler").min_size(egui::vec2(90.0, 38.0)))
                        .clicked()
                    {
                        action = EditorAction::Close;
                    }
                    if let Some(error) = &self.error {
                        ui.label(
                            RichText::new(format!("{}  {error}", icon::WARNING))
                                .color(pal.negative),
                        );
                    }
                });
            });
            ui.add_space(4.0);
            ui.separator();
        });

        let (enter, escape, load, clear) = ui.input_mut(|i| {
            (
                i.modifiers.command && i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.consume_key(egui::Modifiers::NONE, Key::F10),
                i.consume_key(egui::Modifiers::COMMAND, Key::Num0),
            )
        });
        if load {
            self.load_payee(doc);
        }
        if clear {
            self.clear(document);
        }
        if enter {
            action = self.validate();
        } else if escape {
            action = EditorAction::Close;
        }
        action
    }

    /// Carte du montant : sens (dépense / recette) et grand champ de saisie,
    /// teintée de rouge ou de vert.
    fn amount_card(&mut self, ui: &mut egui::Ui, doc: &OpenDoc, outer_width: f32) {
        let pal = palette(ui);
        let color = if self.credit {
            pal.positive
        } else {
            pal.negative
        };
        let width = outer_width - 2.0 * 20.0 - 2.0;
        egui::Frame::new()
            .fill(color.gamma_multiply(0.08))
            .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.35)))
            .corner_radius(14)
            .inner_margin(egui::Margin::symmetric(20, 16))
            .show(ui, |ui| {
                ui.set_width(width);
                ui.set_max_width(width);
                crate::ui_kit::segmented(
                    ui,
                    &mut self.credit,
                    &[
                        (false, icon::ARROW_UP_RIGHT, "Dépense", pal.negative),
                        (true, icon::ARROW_DOWN_LEFT, "Recette", pal.positive),
                    ],
                    28.0,
                    None,
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let sign = if self.credit { "+" } else { "−" };
                    ui.label(
                        RichText::new(sign)
                            .size(28.0)
                            .family(crate::theme::semibold())
                            .color(color),
                    );
                    let currency = doc.current_account().currency.clone();
                    let currency_width = 30.0;
                    let edit = egui::TextEdit::singleline(&mut self.amount)
                        .font(egui::FontId::new(30.0, crate::theme::semibold()))
                        .text_color(color)
                        .hint_text(
                            RichText::new("0,00")
                                .size(30.0)
                                .color(color.gamma_multiply(0.35)),
                        )
                        .frame(egui::Frame::NONE)
                        .desired_width(ui.available_width() - currency_width);
                    let response = ui.add(edit);
                    if response.changed() {
                        // Un signe tapé choisit le sens et disparaît du champ.
                        if let Some(rest) = self.amount.strip_prefix('-') {
                            self.credit = false;
                            self.amount = rest.to_string();
                        } else if let Some(rest) = self.amount.strip_prefix('+') {
                            self.credit = true;
                            self.amount = rest.to_string();
                        }
                    }
                    ui.label(
                        RichText::new(currency)
                            .size(22.0)
                            .color(color.gamma_multiply(0.8)),
                    );
                });
            });
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
        let Some(date) = Date::parse_fr(&self.date, Date::today()) else {
            return;
        };
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

    /// Remplit la saisie depuis un tiers mémorisé (champs non vides).
    fn fill_from_payee(&mut self, document: &Document, p: &bp_core::model::Payee) {
        self.payee = if p.payee.is_empty() {
            p.caption.clone()
        } else {
            p.payee.clone()
        };
        self.mode = p.mode.clone();
        self.credit = !mode_is_debit(document, &self.mode);
        if p.category.is_some() {
            self.category = p.category;
        }
        if !p.details.is_empty() {
            self.details = p.details.clone();
        }
        if let Some(a) = Amount::parse(&p.amount).filter(|&a| a != Amount::ZERO) {
            self.amount = a.abs().format(2, ',');
        }
    }

    /// Charger un tiers (F10) : d'abord parmi les tiers mémorisés, sinon la
    /// dernière opération du compte dont le tiers ou le détail contient le
    /// texte saisi (mode, détail, catégorie et montant recopiés).
    fn load_payee(&mut self, doc: &OpenDoc) {
        let needle = bp_core::filter::normalize(&self.payee);
        let needle = needle.trim();
        if needle.is_empty() {
            self.error = Some("Saisissez du texte dans le champ Tiers, puis F10".into());
            return;
        }
        if let Some(p) = doc
            .document
            .payees
            .iter()
            .find(|p| bp_core::filter::normalize(&p.caption).contains(needle))
            .cloned()
        {
            self.fill_from_payee(&doc.document, &p);
            self.error = None;
            return;
        }
        let found = doc
            .current_account()
            .operations
            .iter()
            .filter(|op| {
                bp_core::filter::normalize(&format!("{} {}", op.payee, op.details)).contains(needle)
            })
            .max_by_key(|op| (op.date, op.uid))
            .cloned();
        match found {
            Some(op) => {
                self.payee = op.payee;
                self.mode = op.mode;
                self.details = op.details;
                self.category = op.category;
                self.credit = !op.amount.is_negative();
                self.amount = op.amount.abs().format(2, ',');
                self.error = None;
            }
            None => self.error = Some("Aucun tiers ne contient ce texte".into()),
        }
    }

    /// Vider la saisie (Ctrl+0) : date du jour, mode Carte, champs vides.
    fn clear(&mut self, document: &Document) {
        self.date = Date::today().to_string();
        self.mode = Mode::default();
        self.credit = !mode_is_debit(document, &self.mode);
        self.payee.clear();
        self.details.clear();
        self.category = None;
        self.amount.clear();
        self.tack = Tack::None;
        self.error = None;
        self.focus_requested = true;
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
                if ui
                    .selectable_label(false, text)
                    .on_hover_text(
                        "Tiers mémorisé : remplit aussi mode, catégorie, détail et montant",
                    )
                    .clicked()
                {
                    self.fill_from_payee(document, &p);
                }
            }
            for name in used {
                if ui.selectable_label(false, &name).clicked() {
                    self.payee = name;
                }
            }
        });
    }

    fn validate(&mut self) -> EditorAction {
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
        // Option « montants en centimes » : 1250 tapé donne 12,50.
        if crate::settings::shared::amounts_in_cents() && !text.contains([',', '.']) {
            amount = Amount(amount.0 / 100);
        }
        let amount = if self.credit {
            amount.abs()
        } else {
            -amount.abs()
        };
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

fn mode_is_debit(document: &Document, mode: &Mode) -> bool {
    match mode {
        Mode::Standard(m) => m.is_debit(),
        Mode::Custom(name) => document
            .custom_modes
            .iter()
            .find(|m| &m.name == name)
            .is_none_or(|m| m.debit),
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

/// Titre de section (icône et petites capitales) suivi d'une carte.
fn section(
    ui: &mut egui::Ui,
    outer_width: f32,
    glyph: &str,
    title: &str,
    content: impl FnOnce(&mut egui::Ui),
) {
    let pal = palette(ui);
    ui.label(
        RichText::new(format!("{glyph}  {}", title.to_uppercase()))
            .size(11.5)
            .family(crate::theme::semibold())
            .color(pal.muted),
    );
    ui.add_space(4.0);
    let width = outer_width - 2.0 * 18.0 - 2.0;
    egui::Frame::new()
        .fill(pal.bg)
        .stroke(egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)))
        .corner_radius(12)
        .inner_margin(egui::Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.set_max_width(width);
            content(ui);
        });
    ui.add_space(12.0);
}
