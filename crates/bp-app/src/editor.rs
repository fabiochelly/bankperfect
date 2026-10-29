//! Panneau de saisie et de modification d'une opération (à droite de la grille).
//! Remplace l'ancienne zone de saisie du bas de fenêtre.

use std::collections::BTreeSet;

use bp_core::categorize::{Query, Suggestion};
use bp_core::i18n::{self, Lang};
use bp_core::model::{Amount, Date, Document, Mode, Operation, Tack};
use bp_core::tr;
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
    /// Proposition de tiers en surbrillance (↑/↓), indice dans la liste.
    payee_pick: Option<usize>,
    /// Liste des propositions refermée (Échap ou choix fait) jusqu'à la
    /// prochaine frappe dans le tiers.
    payee_closed: bool,
    /// Emplacement de la liste à l'image précédente, pour y suivre un clic.
    payee_list: Option<egui::Rect>,
    /// Proposition retenue : focus et curseur en fin de tiers.
    payee_refocus: bool,
    focus_requested: bool,
    /// Le contenu dépasse la hauteur du panneau (barre de défilement).
    overflow: bool,
    /// Entrée tapée dans le montant de la barre du bas : validation.
    pending_submit: bool,
    /// Catégories proposées pour la saisie en cours, et texte (tiers,
    /// détail, montant) pour lequel elles ont été calculées.
    suggestion: Option<Suggestion>,
    suggestion_key: String,
    /// Catégorie remplie automatiquement (remplacée tant que l'utilisateur
    /// n'en choisit pas une autre).
    auto_category: Option<u32>,
    /// Message de la baguette magique (rien à proposer).
    category_hint: Option<String>,
}

/// Proposition de l'auto-complétion du tiers.
enum PayeeChoice {
    /// Tiers mémorisé : remplit aussi mode, catégorie, détail et montant.
    Memorized(bp_core::model::Payee),
    /// Tiers déjà utilisé dans le compte.
    Used(String),
}

/// Bouton « baguette magique » à côté de la catégorie.
const WAND_WIDTH: f32 = 36.0;

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
            payee_pick: None,
            payee_closed: false,
            payee_list: None,
            payee_refocus: false,
            focus_requested: true,
            overflow: false,
            pending_submit: false,
            suggestion: None,
            suggestion_key: String::new(),
            auto_category: None,
            category_hint: None,
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
            amount: op.amount.abs().format(2, i18n::decimal_separator()),
            tack: op.tack,
            credit: !op.amount.is_negative(),
            original_tack: Some(op.tack),
            extra: op.extra.clone(),
            error: None,
            known_payees: known_payees(document, account),
            payee_pick: None,
            payee_closed: false,
            payee_list: None,
            payee_refocus: false,
            focus_requested: true,
            overflow: false,
            pending_submit: false,
            suggestion: None,
            suggestion_key: String::new(),
            auto_category: None,
            category_hint: None,
        }
    }

    /// Place le curseur dans le champ date (F8).
    pub fn focus_date(&mut self) {
        self.focus_requested = true;
    }

    /// Valide la saisie (Maj+Entrée sur une ligne en cours de modification).
    pub fn submit(&mut self) -> EditorAction {
        self.validate()
    }

    /// Sans prise du focus clavier (barre de saisie permanente du bas).
    pub fn quiet(mut self) -> Self {
        self.focus_requested = false;
        self
    }

    /// Saisie en série : la nouvelle opération reprend la date précédente.
    pub fn with_date(mut self, date: String) -> Self {
        self.date = date;
        self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, doc: &OpenDoc) -> EditorAction {
        self.refresh_suggestion(doc);
        let document = &doc.document;
        let pal = palette(ui);
        let mut action = EditorAction::None;

        // En-tête : titre, compte, fermeture.
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let title = if self.target.is_some() {
                    tr!("Modifier l'opération")
                } else {
                    tr!("Nouvelle opération")
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
                    .on_hover_text(tr!("Fermer (Échap)"))
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
                    tr!("Quand et comment"),
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
                            let date = date.on_hover_text(tr!(
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

                            let width = cols[1].available_width();
                            self.mode_combo(&mut cols[1], doc, width);
                            let height =
                                cols[0].min_rect().height().max(cols[1].min_rect().height());
                            ui.allocate_space(egui::vec2(2.0 * half + 12.0, height));
                        });
                    },
                );

                section(ui, card_width, icon::USER, tr!("Qui et pourquoi"), |ui| {
                    let id = egui::Id::new("editor_payee");
                    self.payee_keys(ui, document, id);
                    let response = ui.add(
                        crate::theme::field(&mut self.payee)
                            .id(id)
                            .hint_text(tr!("Commerçant, organisme, personne…"))
                            .desired_width(f32::INFINITY),
                    );
                    if self.payee_list_shown(ui, &response) {
                        self.payee_suggestions(ui, document, None);
                    }
                    ui.add_space(14.0);
                    ui.add(
                        crate::theme::field(&mut self.details)
                            .hint_text(tr!("Note, référence, n° de chèque…"))
                            .desired_width(f32::INFINITY),
                    );
                });

                section(ui, card_width, icon::TAG, tr!("Classement"), |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let width = ui.available_width() - WAND_WIDTH - 8.0;
                        crate::ui_kit::category_combo(
                            ui,
                            "category",
                            document,
                            &doc.category_styles,
                            &mut self.category,
                            width,
                        );
                        self.wand_button(ui, doc);
                    });
                    self.suggestion_hints(ui, doc);
                    ui.add_space(14.0);
                    crate::ui_kit::segmented(
                        ui,
                        &mut self.tack,
                        &[
                            (Tack::None, icon::CIRCLE_DASHED, tr!("À pointer"), pal.fg),
                            (Tack::Pointed, icon::CHECK, tr!("Pointée"), pal.pointed),
                            (
                                Tack::Reconciled,
                                icon::CHECKS,
                                tr!("Rapprochée"),
                                pal.reconciled,
                            ),
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
                        tr!("Champs supplémentaires"),
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
                    let save = crate::theme::primary_button(
                        ui,
                        format!("{}  {}", icon::CHECK, tr!("Enregistrer")),
                    )
                    .min_size(egui::vec2(140.0, 38.0));
                    if ui.add(save).on_hover_text(tr!("Ctrl+Entrée")).clicked() {
                        action = self.validate();
                    }
                    if ui
                        .add(egui::Button::new(tr!("Annuler")).min_size(egui::vec2(90.0, 38.0)))
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

        let (enter, escape, load, clear, guess) = ui.input_mut(|i| {
            (
                i.modifiers.command && i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.consume_key(egui::Modifiers::NONE, Key::F10),
                i.consume_key(egui::Modifiers::COMMAND, Key::Num0),
                i.consume_key(egui::Modifiers::NONE, Key::F9),
            )
        });
        if load {
            self.load_payee(doc);
        }
        if guess {
            self.guess_category(doc);
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

    /// Liste des modes de paiement, en couleur ; un chèque émis propose le
    /// numéro du chèque suivant dans le détail d'une nouvelle opération.
    fn mode_combo(&mut self, ui: &mut egui::Ui, doc: &OpenDoc, width: f32) {
        let document = &doc.document;
        let pal = palette(ui);
        let before = self.mode.clone();
        let (glyph, color) = crate::grid::mode_style(document, &self.mode, &pal);
        egui::ComboBox::from_id_salt("mode")
            .selected_text(RichText::new(format!("{glyph}  {}", self.mode.label())).color(color))
            .width(width)
            .height(420.0)
            .show_ui(ui, |ui| {
                for mode in document.mode_list() {
                    let (glyph, color) = crate::grid::mode_style(document, &mode, &pal);
                    let text = RichText::new(format!("{glyph}  {}", mode.label())).color(color);
                    ui.selectable_value(&mut self.mode, mode, text);
                }
            });
        if self.mode != before {
            self.credit = !mode_is_debit(document, &self.mode);
            if self.target.is_none()
                && self.details.trim().is_empty()
                && self.mode == Mode::Standard(bp_core::model::StandardMode::ChequeEmis)
            {
                self.details = bp_core::edit::next_cheque_number(doc.current_account());
            }
        }
    }

    /// Barre de saisie du bas (option) : mêmes champs que le panneau de
    /// droite, sur une ou deux lignes selon la largeur.
    pub fn show_bar(&mut self, ui: &mut egui::Ui, doc: &OpenDoc) -> EditorAction {
        self.refresh_suggestion(doc);
        let document = &doc.document;
        let pal = palette(ui);
        let mut action = EditorAction::None;
        let mut focused = false;
        const GAP: f32 = 8.0;
        // En-tête : titre, erreur, boutons.
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let title = if self.target.is_some() {
                tr!("Modifier l'opération")
            } else {
                tr!("Nouvelle opération")
            };
            ui.label(RichText::new(title).family(crate::theme::semibold()));
            ui.label(
                RichText::new(format!("{}  {}", icon::BANK, doc.current_account().name))
                    .color(pal.muted),
            );
            if let Some(error) = &self.error {
                ui.add_space(12.0);
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.target.is_some() {
                    tr!("Enregistrer")
                } else {
                    tr!("Ajouter")
                };
                let save = crate::theme::primary_button(ui, format!("{}  {label}", icon::CHECK))
                    .min_size(egui::vec2(120.0, 30.0));
                if ui
                    .add(save)
                    .on_hover_text(tr!("Entrée (dans le montant) ou Ctrl+Entrée"))
                    .clicked()
                {
                    action = self.validate();
                }
                let reset = if self.target.is_some() {
                    tr!("Annuler")
                } else {
                    tr!("Effacer")
                };
                if ui
                    .add(egui::Button::new(reset).min_size(egui::vec2(80.0, 30.0)))
                    .on_hover_text(tr!("Échap"))
                    .clicked()
                {
                    action = EditorAction::Close;
                }
            });
        });
        ui.add_space(6.0);
        let width = ui.available_width();
        let wide = width >= 1250.0;
        let (date_w, mode_w, cat_w, sign_w, amount_w, tack_w) =
            (150.0, 175.0, 210.0, 34.0, 130.0, 112.0);
        let mut payee_rect = None;
        let mut payee_list = false;
        let color = if self.credit {
            pal.positive
        } else {
            pal.negative
        };
        let mut fields_a = |ui: &mut egui::Ui, this: &mut Self, focused: &mut bool| {
            let date = crate::datepicker::date_field(
                ui,
                "bar_date",
                &mut this.date,
                date_w,
                crate::datepicker::Placement::Popup,
            );
            if this.focus_requested {
                date.request_focus();
                this.focus_requested = false;
            }
            if date.has_focus() {
                *focused = true;
                this.date_keys(ui);
            }
            this.mode_combo(ui, doc, mode_w);
            // Tiers et détail se partagent la place restante, jusqu'au bord
            // droit (en large, après eux : catégorie, signe, montant, pointage).
            let after = if wide {
                cat_w + WAND_WIDTH + sign_w + amount_w + tack_w + 5.0 * GAP
            } else {
                0.0
            };
            // Largeur totale du champ, marges comprises.
            let text_w = ((ui.available_width() - after - GAP) / 2.0).max(120.0);
            let id = egui::Id::new("bar_payee");
            this.payee_keys(ui, document, id);
            let payee = ui.add(
                crate::theme::field(&mut this.payee)
                    .id(id)
                    .hint_text(tr!("Tiers"))
                    .desired_width(text_w),
            );
            *focused |= payee.has_focus();
            payee_list = this.payee_list_shown(ui, &payee);
            payee_rect = Some(payee.rect);
            let details = ui.add(
                crate::theme::field(&mut this.details)
                    .hint_text(tr!("Détail"))
                    .desired_width(text_w),
            );
            *focused |= details.has_focus();
        };
        let fields_b = |ui: &mut egui::Ui, this: &mut Self, focused: &mut bool| {
            crate::ui_kit::category_combo(
                ui,
                "bar_category",
                document,
                &doc.category_styles,
                &mut this.category,
                cat_w,
            );
            this.wand_button(ui, doc);
            let sign = if this.credit { "+" } else { "−" };
            if ui
                .add_sized(
                    egui::vec2(sign_w, 32.0),
                    egui::Button::new(RichText::new(sign).size(18.0).color(color)),
                )
                .on_hover_text(tr!("Dépense ou recette (ou tapez - ou + dans le montant)"))
                .clicked()
            {
                this.credit = !this.credit;
            }
            let amount = ui.add(
                crate::theme::field(&mut this.amount)
                    .text_color(color)
                    .horizontal_align(egui::Align::Max)
                    .hint_text(zero_hint())
                    .desired_width(amount_w),
            );
            if amount.changed() {
                if let Some(rest) = this.amount.strip_prefix('-') {
                    this.credit = false;
                    this.amount = rest.to_string();
                } else if let Some(rest) = this.amount.strip_prefix('+') {
                    this.credit = true;
                    this.amount = rest.to_string();
                }
            }
            *focused |= amount.has_focus();
            if amount.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                *focused = true;
                this.error = None;
                this.pending_submit = true;
            }
            crate::ui_kit::segmented(
                ui,
                &mut this.tack,
                &[
                    (Tack::None, icon::CIRCLE_DASHED, "", pal.fg),
                    (Tack::Pointed, icon::CHECK, "", pal.pointed),
                    (Tack::Reconciled, icon::CHECKS, "", pal.reconciled),
                ],
                32.0,
                Some(tack_w),
            );
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            fields_a(ui, self, &mut focused);
            if wide {
                fields_b(ui, self, &mut focused);
            }
        });
        if !wide {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                fields_b(ui, self, &mut focused);
            });
        }
        if payee_list {
            self.payee_suggestions(ui, document, payee_rect);
        }
        if std::mem::take(&mut self.pending_submit) {
            action = self.validate();
        }
        let (enter, escape, load, clear, guess) = ui.input_mut(|i| {
            (
                focused && i.modifiers.command && i.key_pressed(Key::Enter),
                focused && i.key_pressed(Key::Escape),
                focused && i.consume_key(egui::Modifiers::NONE, Key::F10),
                focused && i.consume_key(egui::Modifiers::COMMAND, Key::Num0),
                focused && i.consume_key(egui::Modifiers::NONE, Key::F9),
            )
        });
        if load {
            self.load_payee(doc);
        }
        if guess {
            self.guess_category(doc);
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
                        (false, icon::ARROW_UP_RIGHT, tr!("Dépense"), pal.negative),
                        (true, icon::ARROW_DOWN_LEFT, tr!("Recette"), pal.positive),
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
                            RichText::new(zero_hint())
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
            self.amount = a.abs().format(2, i18n::decimal_separator());
        }
    }

    /// Charger un tiers (F10) : d'abord parmi les tiers mémorisés, sinon la
    /// dernière opération du compte dont le tiers ou le détail contient le
    /// texte saisi (mode, détail, catégorie et montant recopiés).
    fn load_payee(&mut self, doc: &OpenDoc) {
        let needle = bp_core::filter::normalize(&self.payee);
        let needle = needle.trim();
        if needle.is_empty() {
            self.error = Some(tr!("Saisissez du texte dans le champ Tiers, puis F10").into());
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
                self.amount = op.amount.abs().format(2, i18n::decimal_separator());
                self.error = None;
            }
            None => self.error = Some(tr!("Aucun tiers ne contient ce texte").into()),
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
        self.suggestion = None;
        self.auto_category = None;
        self.category_hint = None;
        self.focus_requested = true;
    }

    /// Tiers proposés pour le texte saisi : mémorisés, puis déjà utilisés.
    fn payee_choices(&self, document: &Document) -> Vec<PayeeChoice> {
        let needle = bp_core::filter::normalize(&self.payee);
        let needle = needle.trim();
        let memorized = document
            .payees
            .iter()
            .filter(|p| bp_core::filter::normalize(&p.caption).contains(needle))
            .take(5)
            .cloned()
            .map(PayeeChoice::Memorized);
        let used = self
            .known_payees
            .iter()
            .filter(|p| bp_core::filter::normalize(p).contains(needle) && **p != self.payee)
            .take(6)
            .cloned()
            .map(PayeeChoice::Used);
        memorized.chain(used).collect()
    }

    /// Avant le champ tiers, qui les prendrait sinon : ↑/↓ parcourent les
    /// propositions, Entrée retient celle en surbrillance, Échap referme la
    /// liste.
    fn payee_keys(&mut self, ui: &egui::Ui, document: &Document, id: egui::Id) {
        if !ui.memory(|m| m.has_focus(id))
            || self.payee_closed
            || self.payee.is_empty()
            || !crate::settings::shared::autocomplete()
        {
            return;
        }
        let mut choices = self.payee_choices(document);
        let count = choices.len();
        if count == 0 {
            return;
        }
        let pick = self.payee_pick.filter(|&i| i < count);
        let (down, up, enter, escape) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                pick.is_some() && i.consume_key(egui::Modifiers::NONE, Key::Enter),
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
            )
        });
        self.payee_pick = match (down, up, pick) {
            (true, _, None) => Some(0),
            (true, _, Some(i)) => Some((i + 1) % count),
            (_, true, None) => Some(count - 1),
            (_, true, Some(i)) => Some((i + count - 1) % count),
            _ => pick,
        };
        if escape {
            self.payee_closed = true;
            self.payee_pick = None;
        } else if enter && let Some(i) = pick {
            self.pick_payee(document, choices.swap_remove(i));
        }
    }

    /// Après le champ tiers : faut-il afficher les propositions ? Le champ
    /// perd le focus dès le relâchement d'un clic sur la liste ; elle reste
    /// donc affichée tant qu'un clic y est en cours, sans quoi il serait
    /// perdu.
    fn payee_list_shown(&mut self, ui: &egui::Ui, field: &egui::Response) -> bool {
        if field.changed() {
            self.payee_closed = false;
            self.payee_pick = None;
        }
        if std::mem::take(&mut self.payee_refocus) {
            field.request_focus();
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), field.id) {
                let end = egui::text::CCursor::new(self.payee.chars().count());
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), field.id);
            }
        }
        let clicking = self.payee_list.is_some_and(|list| {
            ui.input(|i| {
                i.pointer.press_origin().is_some_and(|p| list.contains(p))
                    || (i.pointer.any_click()
                        && i.pointer.interact_pos().is_some_and(|p| list.contains(p)))
            })
        });
        let shown = (field.has_focus() || clicking)
            && !self.payee_closed
            && !self.payee.is_empty()
            && crate::settings::shared::autocomplete();
        if !shown {
            self.payee_list = None;
            self.payee_pick = None;
        }
        shown
    }

    fn payee_suggestions(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        above: Option<egui::Rect>,
    ) {
        let choices = self.payee_choices(document);
        if choices.is_empty() {
            self.payee_list = None;
            return;
        }
        let rect = match above {
            // Barre du bas : liste flottante au-dessus du champ.
            Some(anchor) => {
                egui::Area::new(ui.id().with("payee_suggestions"))
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::LEFT_BOTTOM)
                    .fixed_pos(anchor.left_top() - egui::vec2(0.0, 4.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(anchor.width());
                            self.suggestion_list(ui, document, choices);
                        });
                    })
                    .response
                    .rect
            }
            None => {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        self.suggestion_list(ui, document, choices);
                    })
                    .response
                    .rect
            }
        };
        self.payee_list = Some(rect);
    }

    fn suggestion_list(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        choices: Vec<PayeeChoice>,
    ) {
        let mut picked = None;
        for (i, choice) in choices.into_iter().enumerate() {
            let selected = self.payee_pick == Some(i);
            let response = match &choice {
                PayeeChoice::Memorized(p) => ui
                    .selectable_label(selected, format!("{}  {}", icon::ADDRESS_BOOK, p.caption))
                    .on_hover_text(tr!(
                        "Tiers mémorisé : remplit aussi mode, catégorie, détail et montant"
                    )),
                PayeeChoice::Used(name) => ui.selectable_label(selected, name),
            };
            if response.clicked() {
                picked = Some(choice);
            }
        }
        if let Some(choice) = picked {
            self.pick_payee(document, choice);
        }
    }

    /// Reporte la proposition choisie dans la saisie et referme la liste.
    fn pick_payee(&mut self, document: &Document, choice: PayeeChoice) {
        match choice {
            PayeeChoice::Memorized(p) => self.fill_from_payee(document, &p),
            PayeeChoice::Used(name) => self.payee = name,
        }
        self.payee_closed = true;
        self.payee_pick = None;
        self.payee_list = None;
        self.payee_refocus = true;
    }

    /// Montant signé saisi (zéro s'il est vide ou illisible).
    fn typed_amount(&self) -> Amount {
        let amount = parse_amount(self.amount.trim())
            .unwrap_or(Amount::ZERO)
            .abs();
        if self.credit {
            amount
        } else {
            Amount(-amount.0)
        }
    }

    fn compute_suggestion(&self, doc: &OpenDoc) -> Suggestion {
        doc.categorizer().suggest(&Query {
            payee: &self.payee,
            details: &self.details,
            amount: self.typed_amount(),
            mode: Some(&self.mode),
            merchant_code: None,
        })
    }

    /// Recalcule les suggestions quand le tiers, le détail, le montant ou le
    /// sens changent ; remplit la catégorie vide si la suggestion est sûre.
    fn refresh_suggestion(&mut self, doc: &OpenDoc) {
        if self.auto_category.is_some() && self.category != self.auto_category {
            // L'utilisateur a choisi une autre catégorie.
            self.auto_category = None;
        }
        let key = format!(
            "{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
            self.payee,
            self.details,
            self.amount,
            self.credit,
            doc.revision()
        );
        if key == self.suggestion_key {
            return;
        }
        self.suggestion_key = key;
        self.category_hint = None;
        if self.payee.trim().is_empty() && self.details.trim().is_empty() {
            self.suggestion = None;
            if self.auto_category.take().is_some() {
                self.category = None;
            }
            return;
        }
        let suggestion = self.compute_suggestion(doc);
        if crate::settings::shared::categorize_in_entry()
            && (self.category.is_none() || self.auto_category.is_some())
        {
            match suggestion.confident() {
                Some(category) => {
                    self.category = Some(category);
                    self.auto_category = Some(category);
                }
                None => {
                    if self.auto_category.take().is_some() {
                        self.category = None;
                    }
                }
            }
        }
        self.suggestion = Some(suggestion);
    }

    /// Baguette magique : devine la catégorie tout de suite, même peu sûre.
    fn guess_category(&mut self, doc: &OpenDoc) {
        let suggestion = self.compute_suggestion(doc);
        match suggestion.best() {
            Some(best) => {
                self.category = Some(best.category);
                self.auto_category = Some(best.category);
                self.category_hint = None;
            }
            None => {
                self.category_hint = Some(if self.payee.trim().is_empty() {
                    tr!("Saisissez d'abord le tiers").into()
                } else {
                    tr!("Aucune catégorie à proposer pour ce libellé").into()
                });
            }
        }
        self.suggestion = Some(suggestion);
    }

    fn wand_button(&mut self, ui: &mut egui::Ui, doc: &OpenDoc) {
        let pal = palette(ui);
        let button =
            egui::Button::new(RichText::new(icon::MAGIC_WAND).size(17.0).color(pal.accent))
                .min_size(egui::vec2(WAND_WIDTH, 32.0));
        if ui
            .add(button)
            .on_hover_text(tr!(
                "Deviner la catégorie d'après le tiers, le détail et le montant (F9)"
            ))
            .clicked()
        {
            self.guess_category(doc);
        }
    }

    /// Sous la catégorie : origine d'une catégorie devinée, ou suggestions à
    /// un clic quand aucune n'est sûre.
    fn suggestion_hints(&mut self, ui: &mut egui::Ui, doc: &OpenDoc) {
        let pal = palette(ui);
        let percent = |c: f32| (c * 100.0).round();
        if let Some(hint) = &self.category_hint {
            ui.add_space(6.0);
            ui.label(RichText::new(hint).color(pal.muted).size(12.5));
            return;
        }
        let Some(suggestion) = &self.suggestion else {
            return;
        };
        if let Some(auto) = self.auto_category.filter(|&a| self.category == Some(a))
            && let Some(c) = suggestion.candidates.iter().find(|c| c.category == auto)
        {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "{}  {}",
                    icon::SPARKLE,
                    tr!("Devinée : {} · {} %", c.reason, percent(c.confidence))
                ))
                .color(pal.muted)
                .size(12.5),
            );
            return;
        }
        if self.category.is_some() || suggestion.is_empty() {
            return;
        }
        let mut chosen = None;
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            ui.label(RichText::new("Suggestions").color(pal.muted).size(12.5));
            for c in &suggestion.candidates {
                let label = doc.document.category_label(c.category).unwrap_or_default();
                let text =
                    RichText::new(format!("{label}  {}", tr!("{} %", percent(c.confidence))))
                        .size(12.5);
                if ui
                    .add(egui::Button::new(text).small())
                    .on_hover_text(tr!("D'après : {}", c.reason))
                    .clicked()
                {
                    chosen = Some(c.category);
                }
            }
        });
        if let Some(category) = chosen {
            self.category = Some(category);
            self.auto_category = None;
        }
    }

    fn validate(&mut self) -> EditorAction {
        let today = Date::today();
        let Some(date) = Date::parse_fr(&self.date, today) else {
            self.error = Some(tr!("Date invalide (jj/mm/aaaa)").into());
            return EditorAction::None;
        };
        let text = self.amount.trim();
        let Some(mut amount) = parse_amount(text) else {
            self.error = Some(tr!("Montant invalide").into());
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

/// Indication d'un montant vide : `0,00`, `0.00` en anglais.
fn zero_hint() -> String {
    format!("0{}00", i18n::decimal_separator())
}

/// Montant tapé par l'utilisateur. En français, comme `Amount::parse` (la
/// virgule ou le point est décimal). En anglais, le point est décimal et la
/// virgule sépare les milliers (« 1,234.56 ») ; une virgule seule reste
/// décimale quand elle l'est clairement (« 12,5 », « 12,50 », « 0,125 ») :
/// suivie de trois chiffres après un entier non nul (« 1,234 »), elle
/// sépare les milliers.
pub fn parse_amount(text: &str) -> Option<Amount> {
    if i18n::lang() == Lang::En {
        let kept: String = text
            .chars()
            .filter(|c| matches!(c, '.' | ',' | '0'..='9'))
            .collect();
        if !kept.contains('.')
            && let Some((int, frac)) = kept.split_once(',')
            && !frac.contains(',')
            && frac.len() == 3
            && (1..=3).contains(&int.len())
            && int.trim_start_matches('0') == int
        {
            return Amount::parse(&text.replace(',', ""));
        }
    }
    Amount::parse(text)
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

#[cfg(test)]
mod tests {
    use super::*;
    use bp_core::i18n::with_lang;

    fn cents(text: &str) -> Option<i64> {
        parse_amount(text).map(|a| a.0 / 100)
    }

    #[test]
    fn french_amounts_unchanged() {
        with_lang(Lang::Fr, || {
            assert_eq!(cents("12,50"), Some(1250));
            assert_eq!(cents("12.5"), Some(1250));
            assert_eq!(cents("1 234,56"), Some(123_456));
            assert_eq!(cents("1.234.567"), Some(123_456_700));
            // Virgule décimale, comme avant.
            assert_eq!(cents("1,234"), Some(123));
            assert_eq!(zero_hint(), "0,00");
        });
    }

    #[test]
    fn english_amounts() {
        with_lang(Lang::En, || {
            assert_eq!(cents("1,234.56"), Some(123_456));
            assert_eq!(cents("1,234"), Some(123_400));
            assert_eq!(cents("12,345,678"), Some(1_234_567_800));
            assert_eq!(cents("12.50"), Some(1250));
            assert_eq!(cents("€1,234.50"), Some(123_450));
            // Virgule clairement décimale.
            assert_eq!(cents("12,5"), Some(1250));
            assert_eq!(cents("12,50"), Some(1250));
            assert_eq!(cents("0,125"), Some(12));
            assert_eq!(cents("1234,5678"), Some(123_456));
            assert_eq!(cents("abc"), None);
            assert_eq!(zero_hint(), "0.00");
        });
    }

    #[test]
    fn edited_amount_reads_back() {
        let account = bp_core::model::Account {
            uid: "a".into(),
            name: "Courant".into(),
            bank_name: String::new(),
            agency_name: String::new(),
            agency_address: String::new(),
            number: String::new(),
            bank_code: String::new(),
            agency_code: String::new(),
            bic: String::new(),
            iban: String::new(),
            logo: String::new(),
            currency: "€".into(),
            currency_rate: 1.0,
            secondary_currency: String::new(),
            secondary_rate: 1.0,
            is_virtual: false,
            disabled: false,
            operations: vec![Operation {
                uid: 1,
                date: Date(45000),
                mark_date: Date(45000),
                tack: Tack::None,
                mode: Mode::default(),
                payee: "Fnac".into(),
                details: String::new(),
                category: None,
                amount: Amount::from_cents(-123_456),
                extra: Vec::new(),
            }],
        };
        let document = Document {
            group_name: String::new(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: Vec::new(),
            accounts: vec![account],
            account_order: vec![0],
            default_account: 0,
            categories: Vec::new(),
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        };
        for lang in Lang::ALL {
            with_lang(lang, || {
                let mut editor = Editor::edit(&document, 0, 0);
                match editor.submit() {
                    EditorAction::Save(Some(0), op) => {
                        assert_eq!(op.amount, Amount::from_cents(-123_456), "{lang:?}")
                    }
                    _ => panic!("saisie refusée en {lang:?}"),
                }
            });
        }
    }
}
