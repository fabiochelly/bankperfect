//! Fenêtre « Gestion » : comptes, catégories, tiers et modes de paiement.
//!
//! Les champs texte sont appliqués quand on les quitte (comme l'ancienne
//! version), pour ne pas reconstruire l'index de recherche à chaque frappe.

use bp_core::manage;
use bp_core::model::{Amount, Mode, Payee, StandardMode};
use egui::{Color32, Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Accounts,
    Categories,
    Payees,
    Modes,
}

pub struct ManageForm {
    pub tab: Tab,
    account: usize,
    category: Option<usize>,
    payee: Option<usize>,
    mode: Option<usize>,
    /// Tampons d'édition (appliqués à la sortie du champ).
    buffers: std::collections::HashMap<&'static str, String>,
    new_account: Option<(String, String)>,
    new_category: Option<(String, bool)>,
    with_children: bool,
    payee_search: String,
    confirm: Option<ConfirmDelete>,
    message: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum ConfirmDelete {
    Account(usize),
    Category(usize),
    Payee(usize),
    Mode(usize),
}

const LABEL_WIDTH: f32 = 190.0;
const ROW_HEIGHT: f32 = 46.0;

/// Ligne d'une liste : icône, titre, sous-titre et texte à droite ; fond
/// teinté et barre d'accent quand elle est sélectionnée.
#[allow(clippy::too_many_arguments)]
fn list_row(
    ui: &mut egui::Ui,
    selected: bool,
    indent: f32,
    glyph: &str,
    color: Color32,
    title: RichText,
    subtitle: Option<&str>,
    right: Option<RichText>,
) -> egui::Response {
    let pal = palette(ui);
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), egui::Sense::click());
    let label = title.text().to_string();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 9.0, pal.accent.gamma_multiply(0.14));
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.min + egui::vec2(0.0, 11.0),
                egui::vec2(3.0, ROW_HEIGHT - 22.0),
            ),
            2.0,
            pal.accent,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, 9.0, pal.bg_highlight.gamma_multiply(0.6));
    }
    let x = rect.left() + 14.0 + indent;
    painter.text(
        egui::pos2(x + 8.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(16.0),
        color,
    );
    let text_x = x + 28.0;
    let right_galley = right.map(|r| {
        egui::WidgetText::from(r).into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
    });
    let right_width = right_galley.as_ref().map_or(0.0, |g| g.size().x + 14.0);
    let max_text = (rect.right() - right_width - text_x - 8.0).max(20.0);
    let title_galley = egui::WidgetText::from(title).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        max_text,
        egui::TextStyle::Body,
    );
    let title_y = if subtitle.is_some() {
        rect.top() + 6.0
    } else {
        rect.center().y - title_galley.size().y / 2.0
    };
    painter.galley(egui::pos2(text_x, title_y), title_galley, pal.fg);
    if let Some(subtitle) = subtitle {
        let galley = egui::WidgetText::from(RichText::new(subtitle).size(12.0).color(pal.muted))
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Truncate),
                max_text,
                egui::TextStyle::Body,
            );
        painter.galley(
            egui::pos2(text_x, rect.bottom() - 7.0 - galley.size().y),
            galley,
            pal.muted,
        );
    }
    if let Some(galley) = right_galley {
        painter.galley(
            egui::pos2(
                rect.right() - 14.0 - galley.size().x,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            pal.fg,
        );
    }
    response
}

/// Bouton d'action d'une barre d'outils (icône et libellé).
fn tool_button(ui: &mut egui::Ui, glyph: &str, label: &str) -> egui::Response {
    ui.add(egui::Button::new(format!("{glyph}  {label}")).min_size(egui::vec2(0.0, 34.0)))
}

/// Encadré de confirmation d'une suppression ; `Some(true)` pour supprimer.
fn confirm_card(ui: &mut egui::Ui, width: f32, text: &str) -> Option<bool> {
    let pal = palette(ui);
    let mut result = None;
    egui::Frame::new()
        .fill(pal.negative.gamma_multiply(0.10))
        .stroke(egui::Stroke::new(1.0, pal.negative.gamma_multiply(0.4)))
        .corner_radius(12)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(width - 34.0);
            ui.label(RichText::new(format!("{}  {text}", icon::WARNING)).color(pal.negative));
            ui.add_space(12.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 36.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if crate::dialogs::danger(ui, "Supprimer").clicked() {
                        result = Some(true);
                    }
                    if crate::dialogs::button(ui, "Annuler").clicked() {
                        result = Some(false);
                    }
                },
            );
        });
    ui.add_space(14.0);
    result
}

impl ManageForm {
    pub fn new(doc: &OpenDoc, tab: Tab) -> Self {
        Self {
            tab,
            account: doc.account,
            category: None,
            payee: None,
            mode: None,
            buffers: Default::default(),
            new_account: None,
            new_category: None,
            with_children: true,
            payee_search: String::new(),
            confirm: None,
            message: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.8, 900.0, 1500.0);
        let height = ui_kit::dialog_height(ctx, 0.8).max(560.0);
        crate::dialogs::modal(ctx, "tool_manage").show(ctx, |ui| {
            ui.set_width(width);
            // Hauteur fixe : la fenêtre, centrée, ne bouge pas d'un onglet à l'autre.
            ui.set_height(height);
            let pal = palette(ui);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(260.0, 36.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| crate::dialogs::header(ui, icon::GEAR_SIX, pal.accent, "Gestion"),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let tabs_width = (width - 280.0).min(720.0);
                    let before = self.tab;
                    ui_kit::segmented(
                        ui,
                        &mut self.tab,
                        &[
                            (Tab::Accounts, icon::BANK, "Comptes", pal.accent),
                            (Tab::Categories, icon::TAG, "Catégories", pal.accent),
                            (Tab::Payees, icon::ADDRESS_BOOK, "Tiers", pal.accent),
                            (
                                Tab::Modes,
                                icon::CREDIT_CARD,
                                "Modes de paiement",
                                pal.accent,
                            ),
                        ],
                        38.0,
                        Some(tabs_width),
                    );
                    if self.tab != before {
                        self.buffers.clear();
                        self.confirm = None;
                        self.message = None;
                        self.new_category = None;
                        self.new_account = None;
                    }
                });
            });
            ui.add_space(18.0);
            let body_height = height - 36.0 - 18.0 - 80.0;
            let gap = 22.0;
            let list_width = (width * 0.32).clamp(280.0, 440.0);
            let detail_width = width - list_width - gap;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.vertical(|ui| {
                    ui.set_width(list_width);
                    ui.set_height(body_height);
                    match self.tab {
                        Tab::Accounts => self.account_list(ui, doc, list_width, body_height),
                        Tab::Categories => self.category_list(ui, doc, list_width, body_height),
                        Tab::Payees => self.payee_list(ui, doc, list_width, body_height),
                        Tab::Modes => self.mode_list(ui, doc, list_width, body_height),
                    }
                });
                ui.vertical(|ui| {
                    ui.set_width(detail_width);
                    ui.set_height(body_height);
                    egui::ScrollArea::vertical()
                        .id_salt(("manage_detail", self.tab))
                        .max_height(body_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let w = detail_width - 16.0;
                            ui.set_max_width(w);
                            match self.tab {
                                Tab::Accounts => self.account_detail(ui, doc, w),
                                Tab::Categories => self.category_detail(ui, doc, w),
                                Tab::Payees => self.payee_detail(ui, doc, w),
                                Tab::Modes => self.mode_detail(ui, doc, w),
                            }
                        });
                });
            });
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, "Fermer", true).clicked()
                    || (self.confirm.is_none() && ui.input(|i| i.key_pressed(Key::Escape)))
                {
                    outcome = Outcome::Close;
                }
                if let Some(message) = &self.message {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}  {message}", icon::INFO)).color(pal.warning),
                        );
                    });
                }
            });
        });
        outcome
    }

    /// Champ texte appliqué à la sortie : `apply` reçoit la valeur saisie.
    fn text_field(
        &mut self,
        ui: &mut egui::Ui,
        key: &'static str,
        current: &str,
        width: f32,
        mut apply: impl FnMut(String),
    ) -> egui::Response {
        let buffer = self
            .buffers
            .entry(key)
            .or_insert_with(|| current.to_string());
        let response = ui.add(crate::theme::field(buffer).desired_width(width));
        if response.lost_focus() && buffer != current {
            apply(buffer.clone());
        }
        if !response.has_focus() && !response.lost_focus() {
            *buffer = current.to_string();
        }
        response
    }

    /// Carte de liste avec barre d'outils en bas.
    fn list_card(
        ui: &mut egui::Ui,
        width: f32,
        height: f32,
        id: &str,
        rows: impl FnOnce(&mut egui::Ui),
        toolbar: impl FnOnce(&mut egui::Ui),
    ) {
        let pal = palette(ui);
        egui::Frame::new()
            .fill(pal.bg)
            .stroke(egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(width - 18.0);
                ui.set_height(height - 18.0);
                let list_height = height - 18.0 - 54.0;
                egui::ScrollArea::vertical()
                    .id_salt(id)
                    .max_height(list_height)
                    .min_scrolled_height(list_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        rows(ui);
                    });
                ui.add_space(6.0);
                ui.painter().hline(
                    ui.min_rect().x_range(),
                    ui.cursor().top(),
                    egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    toolbar(ui);
                });
            });
    }

    // ─────────────────────────────── Comptes ───────────────────────────────

    fn account_list(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32, height: f32) {
        let pal = palette(ui);
        if self.account >= doc.document.accounts.len() {
            self.account = 0;
        }
        let order = doc.document.account_order.clone();
        let mut action = None;
        let mut tool = None;
        Self::list_card(
            ui,
            width,
            height,
            "acc_list",
            |ui| {
                for &i in &order {
                    let a = &doc.document.accounts[i];
                    let mut title = RichText::new(if a.name.is_empty() {
                        "(sans nom)"
                    } else {
                        &a.name
                    })
                    .family(crate::theme::medium());
                    if a.disabled {
                        title = title.color(pal.muted).strikethrough();
                    } else if a.is_virtual {
                        title = title.italics();
                    }
                    let subtitle = if a.bank_name.is_empty() {
                        a.iban.clone()
                    } else {
                        a.bank_name.clone()
                    };
                    let balance = a.balance();
                    let color = if balance.is_negative() {
                        pal.negative
                    } else {
                        pal.fg_dark
                    };
                    let right = RichText::new(crate::format::money(balance, &a.currency))
                        .color(color)
                        .size(13.0);
                    let glyph = if a.is_virtual {
                        icon::WALLET
                    } else {
                        icon::BANK
                    };
                    let subtitle = (!subtitle.is_empty()).then_some(subtitle);
                    if list_row(
                        ui,
                        self.account == i,
                        0.0,
                        glyph,
                        pal.accent,
                        title,
                        subtitle.as_deref(),
                        Some(right),
                    )
                    .clicked()
                    {
                        action = Some(i);
                    }
                }
            },
            |ui| {
                if tool_button(ui, icon::PLUS, "Nouveau").clicked() {
                    tool = Some(0);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new(icon::TRASH).color(pal.negative))
                                .min_size(egui::vec2(34.0, 34.0)),
                        )
                        .on_hover_text("Supprimer le compte")
                        .clicked()
                    {
                        tool = Some(1);
                    }
                    if ui
                        .add(egui::Button::new(icon::ARROW_DOWN).min_size(egui::vec2(34.0, 34.0)))
                        .on_hover_text("Descendre")
                        .clicked()
                    {
                        tool = Some(2);
                    }
                    if ui
                        .add(egui::Button::new(icon::ARROW_UP).min_size(egui::vec2(34.0, 34.0)))
                        .on_hover_text("Monter")
                        .clicked()
                    {
                        tool = Some(3);
                    }
                });
            },
        );
        let i = self.account;
        match tool {
            Some(0) => {
                self.new_account = Some((
                    manage::unique_account_name(&doc.document, "Nouveau compte"),
                    String::new(),
                ));
                self.confirm = None;
            }
            Some(1) => self.confirm = Some(ConfirmDelete::Account(i)),
            Some(2) => {
                doc.edit_document(|d| manage::move_account(d, i, false));
            }
            Some(3) => {
                doc.edit_document(|d| manage::move_account(d, i, true));
            }
            _ => {}
        }
        if let Some(i) = action
            && i != self.account
        {
            self.account = i;
            self.buffers.clear();
            self.confirm = None;
        }
    }

    fn account_detail(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32) {
        let pal = palette(ui);
        if let Some((name, balance)) = &mut self.new_account {
            let mut create = None;
            ui_kit::section(ui, width, icon::PLUS_CIRCLE, "Nouveau compte", |ui| {
                let w = ui.available_width() - LABEL_WIDTH;
                ui_kit::row(ui, "Nom du compte", LABEL_WIDTH, |ui| {
                    let r = ui.add(crate::theme::field(name).desired_width(w));
                    if r.ctx.memory(|m| m.focused().is_none()) {
                        r.request_focus();
                    }
                });
                ui.add_space(8.0);
                ui_kit::row(ui, "Solde initial", LABEL_WIDTH, |ui| {
                    ui.add(
                        crate::theme::field(balance)
                            .hint_text("0,00")
                            .desired_width(180.0),
                    );
                });
                ui.add_space(14.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 36.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        if crate::dialogs::primary(ui, "Créer le compte", !name.trim().is_empty())
                            .clicked()
                        {
                            create = Some(true);
                        }
                        if crate::dialogs::button(ui, "Annuler").clicked() {
                            create = Some(false);
                        }
                    },
                );
            });
            match create {
                Some(true) => {
                    let name = manage::unique_account_name(&doc.document, name);
                    let balance = Amount::parse(balance).unwrap_or_default();
                    let index = doc.edit_document(|d| manage::add_account(d, &name, balance));
                    self.account = index;
                    doc.select_account(index);
                    self.buffers.clear();
                    self.new_account = None;
                }
                Some(false) => self.new_account = None,
                None => {}
            }
            return;
        }

        if let Some(ConfirmDelete::Account(i)) = self.confirm {
            let (name, count) = doc
                .document
                .accounts
                .get(i)
                .map_or((String::new(), 0), |a| (a.name.clone(), a.operations.len()));
            match confirm_card(
                ui,
                width,
                &format!("Supprimer le compte « {name} » et ses {count} opérations ?"),
            ) {
                Some(true) => {
                    match doc.edit_document(|d| manage::delete_account(d, i)) {
                        Some(next) => {
                            self.account = next;
                            doc.select_account(next);
                        }
                        None => {
                            self.message = Some("Le fichier doit garder au moins un compte.".into())
                        }
                    }
                    self.confirm = None;
                    self.buffers.clear();
                }
                Some(false) => self.confirm = None,
                None => {}
            }
        }

        let i = self.account;
        let a = doc.document.accounts[i].clone_header();
        macro_rules! field {
            ($ui:expr, $key:literal, $value:expr, $width:expr, |$d:ident, $v:ident| $body:expr) => {
                self.text_field($ui, $key, &$value, $width, |v| {
                    let $v = manage::clean(&v);
                    doc.edit_document(|$d| $body);
                })
            };
        }
        let full = width - 2.0 * ui_kit::CARD_PADDING - 2.0 - LABEL_WIDTH - 8.0;

        ui_kit::section(ui, width, icon::IDENTIFICATION_CARD, "Identité", |ui| {
            ui_kit::row(ui, "Nom du compte", LABEL_WIDTH, |ui| {
                field!(
                    ui,
                    "name",
                    a.name,
                    full,
                    |d, v| d.accounts[i].name = if v.trim().is_empty() {
                        format!("Compte {}", i + 1)
                    } else {
                        v
                    }
                );
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Banque", LABEL_WIDTH, |ui| {
                field!(ui, "bank", a.bank_name, full, |d, v| d.accounts[i]
                    .bank_name = v);
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Agence", LABEL_WIDTH, |ui| {
                field!(ui, "agency", a.agency_name, full, |d, v| d.accounts[i]
                    .agency_name =
                    v);
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Adresse", LABEL_WIDTH, |ui| {
                field!(ui, "address", a.agency_address, full, |d, v| d.accounts
                    [i]
                    .agency_address =
                    v);
            });
        });

        ui_kit::section(ui, width, icon::BANK, "Coordonnées bancaires", |ui| {
            ui_kit::row(ui, "Banque · guichet", LABEL_WIDTH, |ui| {
                field!(ui, "bank_code", a.bank_code, 110.0, |d, v| d.accounts[i]
                    .bank_code =
                    v);
                field!(ui, "agency_code", a.agency_code, 110.0, |d, v| d.accounts
                    [i]
                    .agency_code =
                    v);
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "N° de compte", LABEL_WIDTH, |ui| {
                field!(ui, "number", a.number, 240.0, |d, v| d.accounts[i].number =
                    v.to_uppercase());
                if !a.number.is_empty() {
                    match bp_core::calc::rib_key(&a.bank_code, &a.agency_code, &a.number) {
                        Some(key) => ui_kit::pill(ui, &format!("clé RIB {key}"), pal.accent),
                        None => ui_kit::pill(ui, "clé RIB incalculable", pal.warning),
                    }
                }
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "BIC", LABEL_WIDTH, |ui| {
                field!(ui, "bic", a.bic, 240.0, |d, v| d.accounts[i].bic =
                    v.to_uppercase());
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "IBAN", LABEL_WIDTH, |ui| {
                field!(ui, "iban", a.iban, (full - 130.0).max(240.0), |d, v| {
                    d.accounts[i].iban = bp_core::calc::format_iban(&v.to_uppercase())
                });
                if !a.iban.trim().is_empty() {
                    if bp_core::calc::iban_is_valid(&a.iban) {
                        ui_kit::pill(ui, &format!("{}  valide", icon::CHECK), pal.positive);
                    } else {
                        ui_kit::pill(ui, &format!("{}  invalide", icon::WARNING), pal.negative);
                    }
                }
            });
        });

        ui_kit::section(ui, width, icon::CURRENCY_CIRCLE_DOLLAR, "Devises", |ui| {
            ui_kit::row(ui, "Devise principale", LABEL_WIDTH, |ui| {
                field!(ui, "curr1", a.currency, 70.0, |d, v| d.accounts[i]
                    .currency =
                    if v.trim().is_empty() { "€".into() } else { v });
                ui.label(RichText::new("taux").color(pal.muted));
                field!(
                    ui,
                    "rate1",
                    a.currency_rate.to_string().replace('.', ","),
                    120.0,
                    |d, v| {
                        d.accounts[i].currency_rate =
                            v.replace(',', ".").trim().parse().unwrap_or(1.0)
                    }
                );
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Devise secondaire", LABEL_WIDTH, |ui| {
                field!(ui, "curr2", a.secondary_currency, 70.0, |d, v| d.accounts
                    [i]
                    .secondary_currency =
                    if v.trim().is_empty() { "F".into() } else { v });
                ui.label(RichText::new("taux").color(pal.muted));
                field!(
                    ui,
                    "rate2",
                    a.secondary_rate.to_string().replace('.', ","),
                    120.0,
                    |d, v| {
                        d.accounts[i].secondary_rate =
                            v.replace(',', ".").trim().parse().unwrap_or(6.55957)
                    }
                );
            });
        });

        ui_kit::section(ui, width, icon::TOGGLE_LEFT, "Options", |ui| {
            let mut is_virtual = a.is_virtual;
            if crate::theme::checkbox(
                ui,
                &mut is_virtual,
                "Compte virtuel : exclu des totaux et statistiques",
            )
            .changed()
            {
                doc.edit_document(|d| d.accounts[i].is_virtual = is_virtual);
            }
            ui.add_space(8.0);
            let mut disabled = a.disabled;
            if crate::theme::checkbox(
                ui,
                &mut disabled,
                "Compte inactif : masqué de la liste des comptes",
            )
            .changed()
            {
                doc.edit_document(|d| d.accounts[i].disabled = disabled);
            }
        });
    }

    // ─────────────────────────────── Catégories ───────────────────────────────

    fn category_list(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32, height: f32) {
        let pal = palette(ui);
        let mut clicked = None;
        Self::list_card(
            ui,
            width,
            height,
            "cat_list",
            |ui| {
                for (pos, c) in doc.document.categories.iter().enumerate() {
                    let style = doc.category_styles.get(&c.id).copied();
                    let glyph = style.map_or(icon::TAG, |s| s.icon);
                    let color =
                        style.map_or(pal.muted, |s| crate::theme::category_color(&pal, s.color));
                    let title = if c.is_child {
                        RichText::new(&c.name)
                    } else {
                        RichText::new(&c.name).family(crate::theme::semibold())
                    };
                    let indent = if c.is_child { 22.0 } else { 0.0 };
                    if list_row(
                        ui,
                        self.category == Some(pos),
                        indent,
                        glyph,
                        color,
                        title,
                        None,
                        None,
                    )
                    .clicked()
                    {
                        clicked = Some(pos);
                    }
                }
            },
            |ui| {
                if tool_button(ui, icon::PLUS, "Catégorie").clicked() {
                    self.new_category = Some((String::new(), false));
                }
                if tool_button(ui, icon::PLUS, "Sous-catégorie").clicked() {
                    self.new_category = Some((String::new(), true));
                }
            },
        );
        if let Some(pos) = clicked {
            self.category = Some(pos);
            self.buffers.clear();
            self.confirm = None;
        }
    }

    fn category_detail(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32) {
        let pal = palette(ui);
        if let Some((name, child)) = &mut self.new_category {
            let kind = if *child {
                "Nouvelle sous-catégorie"
            } else {
                "Nouvelle catégorie"
            };
            let mut done = None;
            ui_kit::section(ui, width, icon::PLUS_CIRCLE, kind, |ui| {
                let w = ui.available_width();
                let response = ui.add(crate::theme::field(name).hint_text("Nom").desired_width(w));
                if response.ctx.memory(|m| m.focused().is_none()) {
                    response.request_focus();
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    done = Some(true);
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Elle sera placée après la catégorie sélectionnée.")
                        .color(pal.muted)
                        .size(12.5),
                );
                ui.add_space(12.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 36.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        if crate::dialogs::primary(ui, "Ajouter", !name.trim().is_empty()).clicked()
                        {
                            done = Some(true);
                        }
                        if crate::dialogs::button(ui, "Annuler").clicked() {
                            done = Some(false);
                        }
                    },
                );
            });
            match done {
                Some(true) if !name.trim().is_empty() => {
                    let (name, child) = (name.clone(), *child);
                    // Après la catégorie sélectionnée (et ses enfants pour une mère).
                    let before = self.category.map(|p| {
                        let mut end = p + 1;
                        if !child {
                            while end < doc.document.categories.len()
                                && doc.document.categories[end].is_child
                            {
                                end += 1;
                            }
                        }
                        end
                    });
                    doc.edit_document(|d| manage::insert_category(d, &name, child, before));
                    self.category = before.or(Some(doc.document.categories.len() - 1));
                    self.new_category = None;
                }
                Some(_) => self.new_category = None,
                None => {}
            }
        }

        let Some(pos) = self.category.filter(|&p| p < doc.document.categories.len()) else {
            if self.new_category.is_none() {
                empty_state(ui, icon::TAG, "Sélectionnez une catégorie dans la liste.");
            }
            ui_kit::section(
                ui,
                width,
                icon::ARROWS_DOWN_UP,
                "Import, export et tri",
                |ui| {
                    self.category_tools(ui, doc);
                },
            );
            return;
        };
        let c = doc.document.categories[pos].clone();
        let usage = manage::category_usage(&doc.document, pos);

        if let Some(ConfirmDelete::Category(p)) = self.confirm {
            let children = if c.is_child {
                ""
            } else {
                " et ses sous-catégories"
            };
            match confirm_card(
                ui,
                width,
                &format!(
                    "Supprimer « {} »{children} ? {usage} opération(s) perdront leur catégorie.",
                    c.name
                ),
            ) {
                Some(true) => {
                    doc.edit_document(|d| manage::delete_category(d, p));
                    self.confirm = None;
                    self.category = None;
                    return;
                }
                Some(false) => self.confirm = None,
                None => {}
            }
        }

        ui_kit::section(ui, width, icon::TAG, "Catégorie", |ui| {
            let full = ui.available_width() - LABEL_WIDTH - 8.0;
            ui_kit::row(ui, "Nom", LABEL_WIDTH, |ui| {
                self.text_field(ui, "cat_name", &c.name, full, |v| {
                    doc.edit_document(|d| manage::rename_category(d, pos, &v));
                });
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Niveau", LABEL_WIDTH, |ui| {
                let mut child = c.is_child;
                ui.add_enabled_ui(pos > 0, |ui| {
                    if ui_kit::segmented(
                        ui,
                        &mut child,
                        &[
                            (false, icon::FOLDER, "Catégorie", pal.accent),
                            (
                                true,
                                icon::ARROW_ELBOW_DOWN_RIGHT,
                                "Sous-catégorie",
                                pal.accent,
                            ),
                        ],
                        34.0,
                        Some(full.min(420.0)),
                    ) {
                        doc.edit_document(|d| manage::toggle_child(d, pos));
                    }
                });
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Utilisation", LABEL_WIDTH, |ui| {
                let text = match usage {
                    0 => "aucune opération".to_string(),
                    1 => "1 opération".to_string(),
                    n => format!("{n} opérations"),
                };
                ui_kit::pill(ui, &text, if usage == 0 { pal.muted } else { pal.accent });
            });
        });

        ui_kit::section(ui, width, icon::ARROWS_DOWN_UP, "Position", |ui| {
            ui.horizontal(|ui| {
                if tool_button(ui, icon::ARROW_UP, "Monter").clicked() {
                    let with = self.with_children;
                    self.category = doc
                        .edit_document(|d| manage::move_category(d, pos, true, with))
                        .or(Some(pos));
                }
                if tool_button(ui, icon::ARROW_DOWN, "Descendre").clicked() {
                    let with = self.with_children;
                    self.category = doc
                        .edit_document(|d| manage::move_category(d, pos, false, with))
                        .or(Some(pos));
                }
                ui.add_space(12.0);
                crate::theme::checkbox(ui, &mut self.with_children, "Avec les sous-catégories");
            });
        });

        ui_kit::section(ui, width, icon::ARCHIVE, "Import, export et tri", |ui| {
            self.category_tools(ui, doc);
        });

        ui.allocate_ui_with_layout(
            egui::vec2(width, 36.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                if crate::dialogs::button(
                    ui,
                    RichText::new(format!("{}  Supprimer la catégorie", icon::TRASH))
                        .color(pal.negative),
                )
                .clicked()
                {
                    self.confirm = Some(ConfirmDelete::Category(pos));
                }
            },
        );
    }

    fn category_tools(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc) {
        ui.horizontal(|ui| {
            if tool_button(ui, icon::SORT_ASCENDING, "Trier par nom").clicked() {
                doc.edit_document(manage::sort_categories);
                self.category = None;
            }
            if tool_button(ui, icon::EXPORT, "Exporter…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_file_name("categories.txt")
                    .add_filter("Texte", &["txt"])
                    .save_file()
            {
                let text = manage::export_categories(&doc.document);
                self.message = Some(match std::fs::write(&path, text) {
                    Ok(()) => format!("Catégories exportées dans {}", path.display()),
                    Err(e) => format!("Export impossible : {e}"),
                });
            }
            if tool_button(ui, icon::DOWNLOAD_SIMPLE, "Importer…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Texte", &["txt"])
                    .pick_file()
            {
                let text = std::fs::read(&path)
                    .map(|b| encoding_rs::UTF_8.decode(&b).0.into_owned())
                    .unwrap_or_default();
                let n = doc.edit_document(|d| manage::import_categories(d, &text));
                self.message = Some(if n > 0 {
                    format!("{n} catégories importées")
                } else {
                    "Aucune catégorie lue dans ce fichier".into()
                });
                self.category = None;
            }
        });
    }

    // ─────────────────────────────── Tiers ───────────────────────────────

    fn payee_list(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32, height: f32) {
        let pal = palette(ui);
        ui_kit::icon_field(
            ui,
            icon::MAGNIFYING_GLASS,
            &mut self.payee_search,
            "Rechercher un tiers",
            width,
        );
        ui.add_space(10.0);
        let needle = bp_core::filter::normalize(&self.payee_search);
        let needle = needle.trim().to_string();
        let mut order: Vec<usize> = (0..doc.document.payees.len()).collect();
        order.sort_by_key(|&i| bp_core::filter::normalize(&doc.document.payees[i].caption));
        let mut clicked = None;
        let mut create = false;
        let mut delete = None;
        let selected = self.payee;
        Self::list_card(
            ui,
            width,
            height - 48.0,
            "payee_list",
            |ui| {
                for i in order {
                    let p = &doc.document.payees[i];
                    let hay = bp_core::filter::normalize(&format!(
                        "{} {} {}",
                        p.caption, p.payee, p.details
                    ));
                    if !needle.is_empty() && !hay.contains(&needle) {
                        continue;
                    }
                    let (glyph, color) = crate::grid::mode_style(&doc.document, &p.mode, &pal);
                    let subtitle = if p.payee.is_empty() || p.payee == p.caption {
                        None
                    } else {
                        Some(p.payee.as_str())
                    };
                    let title = RichText::new(&p.caption).family(crate::theme::medium());
                    if list_row(
                        ui,
                        self.payee == Some(i),
                        0.0,
                        glyph,
                        color,
                        title,
                        subtitle,
                        None,
                    )
                    .clicked()
                    {
                        clicked = Some(i);
                    }
                }
            },
            |ui| {
                if tool_button(ui, icon::PLUS, "Nouveau tiers").clicked() {
                    create = true;
                }
                if let Some(i) = selected {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new(icon::TRASH).color(pal.negative))
                                    .min_size(egui::vec2(34.0, 34.0)),
                            )
                            .on_hover_text("Supprimer le tiers")
                            .clicked()
                        {
                            delete = Some(i);
                        }
                    });
                }
            },
        );
        if create {
            let index = doc.edit_document(|d| {
                d.payees.push(Payee {
                    caption: "Nouveau tiers".into(),
                    mode: Mode::default(),
                    category: None,
                    payee: String::new(),
                    details: String::new(),
                    amount: String::new(),
                });
                d.payees.len() - 1
            });
            clicked = Some(index);
        }
        if let Some(i) = delete {
            self.confirm = Some(ConfirmDelete::Payee(i));
        }
        if let Some(i) = clicked {
            self.payee = Some(i);
            self.buffers.clear();
            self.confirm = None;
        }
    }

    fn payee_detail(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32) {
        let pal = palette(ui);
        let Some(i) = self.payee.filter(|&i| i < doc.document.payees.len()) else {
            empty_state(
                ui,
                icon::ADDRESS_BOOK,
                "Les tiers mémorisés remplissent la saisie : tapez le début de leur nom dans le champ Tiers d'une nouvelle opération.",
            );
            return;
        };
        if let Some(ConfirmDelete::Payee(k)) = self.confirm {
            let name = doc
                .document
                .payees
                .get(k)
                .map(|p| p.caption.clone())
                .unwrap_or_default();
            match confirm_card(
                ui,
                width,
                &format!(
                    "Supprimer le tiers mémorisé « {name} » ? Les opérations ne changent pas."
                ),
            ) {
                Some(true) => {
                    doc.edit_document(|d| {
                        if k < d.payees.len() {
                            d.payees.remove(k);
                        }
                    });
                    self.payee = None;
                    self.confirm = None;
                    return;
                }
                Some(false) => self.confirm = None,
                None => {}
            }
        }
        let p = doc.document.payees[i].clone();
        ui_kit::section(ui, width, icon::ADDRESS_BOOK, "Tiers mémorisé", |ui| {
            let full = ui.available_width() - LABEL_WIDTH - 8.0;
            ui_kit::row(ui, "Nom dans la liste", LABEL_WIDTH, |ui| {
                self.text_field(ui, "payee_caption", &p.caption, full, |v| {
                    doc.edit_document(|d| d.payees[i].caption = manage::clean(&v));
                });
            });
        });
        ui_kit::section(
            ui,
            width,
            icon::MAGIC_WAND,
            "Valeurs remplies à la saisie",
            |ui| {
                let full = ui.available_width() - LABEL_WIDTH - 8.0;
                ui_kit::row(ui, "Mode de paiement", LABEL_WIDTH, |ui| {
                    let mut mode = p.mode.clone();
                    if ui_kit::mode_combo(ui, "payee_mode", &doc.document, &mut mode, full) {
                        doc.edit_document(|d| d.payees[i].mode = mode);
                    }
                });
                ui.add_space(8.0);
                ui_kit::row(ui, "Tiers", LABEL_WIDTH, |ui| {
                    self.text_field(ui, "payee_payee", &p.payee, full, |v| {
                        doc.edit_document(|d| d.payees[i].payee = manage::clean(&v));
                    });
                });
                ui.add_space(8.0);
                ui_kit::row(ui, "Détail", LABEL_WIDTH, |ui| {
                    self.text_field(ui, "payee_details", &p.details, full, |v| {
                        doc.edit_document(|d| d.payees[i].details = manage::clean(&v));
                    });
                });
                ui.add_space(8.0);
                ui_kit::row(ui, "Montant", LABEL_WIDTH, |ui| {
                    self.text_field(ui, "payee_amount", &p.amount, 180.0, |v| {
                        doc.edit_document(|d| d.payees[i].amount = manage::clean(&v));
                    });
                });
                ui.add_space(8.0);
                ui_kit::row(ui, "Catégorie", LABEL_WIDTH, |ui| {
                    let mut category = p.category;
                    let styles = doc.category_styles.clone();
                    if ui_kit::category_combo(
                        ui,
                        "payee_cat",
                        &doc.document,
                        &styles,
                        &mut category,
                        full,
                    ) {
                        doc.edit_document(|d| d.payees[i].category = category);
                    }
                });
            },
        );
        ui.label(
            RichText::new("Les champs vides ne sont pas remplis lors de la saisie.")
                .color(pal.muted)
                .size(12.5),
        );
    }

    // ─────────────────────────────── Modes ───────────────────────────────

    fn mode_list(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32, height: f32) {
        let pal = palette(ui);
        let mut clicked = None;
        let mut add = false;
        Self::list_card(
            ui,
            width,
            height,
            "mode_list",
            |ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("   MODES STANDARD")
                        .size(11.0)
                        .color(pal.muted),
                );
                for m in StandardMode::ALL {
                    let mode = Mode::Standard(m);
                    let (glyph, color) = crate::grid::mode_style(&doc.document, &mode, &pal);
                    let sign = if m.is_debit() { "dépense" } else { "recette" };
                    let right = RichText::new(format!("{}  {sign}", icon::LOCK_SIMPLE))
                        .size(12.0)
                        .color(pal.muted);
                    list_row(
                        ui,
                        false,
                        0.0,
                        glyph,
                        color,
                        RichText::new(m.label()).color(pal.fg_dark),
                        None,
                        Some(right),
                    )
                    .on_hover_text("Mode standard, non modifiable");
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new("   MODES PERSONNALISÉS")
                        .size(11.0)
                        .color(pal.muted),
                );
                if doc.document.custom_modes.is_empty() {
                    ui.label(RichText::new("   Aucun pour l'instant.").color(pal.muted));
                }
                for (i, m) in doc.document.custom_modes.iter().enumerate() {
                    let mode = Mode::Custom(m.name.clone());
                    let (glyph, color) = crate::grid::mode_style(&doc.document, &mode, &pal);
                    let sign = if m.debit { "dépense" } else { "recette" };
                    let right = RichText::new(sign).size(12.0).color(color);
                    let title = RichText::new(&m.name).family(crate::theme::medium());
                    if list_row(
                        ui,
                        self.mode == Some(i),
                        0.0,
                        glyph,
                        color,
                        title,
                        None,
                        Some(right),
                    )
                    .clicked()
                    {
                        clicked = Some(i);
                    }
                }
            },
            |ui| {
                if tool_button(ui, icon::PLUS, "Nouveau mode").clicked() {
                    add = true;
                }
            },
        );
        if add {
            doc.edit_document(|d| manage::add_mode(d, "Nouveau mode", true));
            clicked = Some(doc.document.custom_modes.len() - 1);
        }
        if let Some(i) = clicked {
            self.mode = Some(i);
            self.buffers.clear();
            self.confirm = None;
        }
    }

    fn mode_detail(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32) {
        let pal = palette(ui);
        let Some(i) = self.mode.filter(|&i| i < doc.document.custom_modes.len()) else {
            empty_state(
                ui,
                icon::CREDIT_CARD,
                "Sélectionnez un mode personnalisé, ou créez-en un (carte d'une autre banque, chèque-restaurant…).",
            );
            return;
        };
        let m = doc.document.custom_modes[i].clone();
        let usage = manage::mode_usage(&doc.document, i);
        let replacement = if m.debit {
            StandardMode::Carte
        } else {
            StandardMode::VirementRecu
        };
        if let Some(ConfirmDelete::Mode(k)) = self.confirm {
            match confirm_card(
                ui,
                width,
                &format!(
                    "Supprimer « {} » ? {usage} opération(s) passeront en « {} ».",
                    m.name,
                    replacement.label()
                ),
            ) {
                Some(true) => {
                    doc.edit_document(|d| manage::delete_mode(d, k, Mode::Standard(replacement)));
                    self.confirm = None;
                    self.mode = None;
                    return;
                }
                Some(false) => self.confirm = None,
                None => {}
            }
        }
        ui_kit::section(ui, width, icon::CREDIT_CARD, "Mode de paiement", |ui| {
            let full = ui.available_width() - LABEL_WIDTH - 8.0;
            let mut refused = false;
            ui_kit::row(ui, "Nom", LABEL_WIDTH, |ui| {
                self.text_field(ui, "mode_name", &m.name, full, |v| {
                    refused = !doc.edit_document(|d| manage::rename_mode(d, i, &v));
                });
            });
            if refused {
                self.message =
                    Some("Nom refusé : vide, déjà utilisé ou nom d'un mode standard.".into());
            }
            ui.add_space(8.0);
            ui_kit::row(ui, "Sens", LABEL_WIDTH, |ui| {
                let mut credit = !m.debit;
                if ui_kit::segmented(
                    ui,
                    &mut credit,
                    &[
                        (false, icon::ARROW_UP_RIGHT, "Dépense", pal.negative),
                        (true, icon::ARROW_DOWN_LEFT, "Recette", pal.positive),
                    ],
                    34.0,
                    Some(full.min(360.0)),
                ) {
                    doc.edit_document(|d| d.custom_modes[i].debit = !credit);
                }
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Utilisation", LABEL_WIDTH, |ui| {
                let text = match usage {
                    0 => "aucune opération".to_string(),
                    1 => "1 opération".to_string(),
                    n => format!("{n} opérations"),
                };
                ui_kit::pill(ui, &text, if usage == 0 { pal.muted } else { pal.accent });
            });
        });
        if crate::dialogs::button(
            ui,
            RichText::new(format!("{}  Supprimer le mode", icon::TRASH)).color(pal.negative),
        )
        .clicked()
        {
            self.confirm = Some(ConfirmDelete::Mode(i));
        }
    }
}

/// Message centré quand rien n'est sélectionné.
fn empty_state(ui: &mut egui::Ui, glyph: &str, text: &str) {
    let pal = palette(ui);
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(glyph)
                .size(40.0)
                .color(pal.muted.gamma_multiply(0.7)),
        );
        ui.add_space(10.0);
        ui.set_max_width(420.0);
        ui.label(RichText::new(text).color(pal.muted));
    });
    ui.add_space(40.0);
}
