//! Fenêtre « Gestion » : comptes, catégories, tiers et modes de paiement.
//!
//! Les champs texte sont appliqués quand on les quitte (comme l'ancienne
//! version), pour ne pas reconstruire l'index de recherche à chaque frappe.

use bp_core::manage;
use bp_core::model::{Amount, Mode, Payee, StandardMode};
use egui::{Id, Key, Modal, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::theme::palette;
use crate::tools::Outcome;

#[derive(Clone, Copy, PartialEq, Eq)]
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

enum ConfirmDelete {
    Account(usize),
    Category(usize),
    Mode(usize),
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

        Modal::new(Id::new("tool_manage")).show(ctx, |ui| {
            ui.set_width(860.0);
            ui.set_height(560.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}  Gestion", icon::GEAR_SIX)).heading());
                ui.add_space(20.0);
                for (tab, label, glyph) in [
                    (Tab::Accounts, "Comptes", icon::BANK),
                    (Tab::Categories, "Catégories", icon::TAG),
                    (Tab::Payees, "Tiers", icon::ADDRESS_BOOK),
                    (Tab::Modes, "Modes de paiement", icon::CREDIT_CARD),
                ] {
                    if ui
                        .selectable_label(self.tab == tab, format!("{glyph}  {label}"))
                        .clicked()
                        && self.tab != tab
                    {
                        self.tab = tab;
                        self.buffers.clear();
                        self.confirm = None;
                        self.message = None;
                    }
                }
            });
            ui.separator();
            ui.add_space(4.0);
            match self.tab {
                Tab::Accounts => self.accounts(ui, doc),
                Tab::Categories => self.categories(ui, doc),
                Tab::Payees => self.payees(ui, doc),
                Tab::Modes => self.modes(ui, doc),
            }
            if let Some(message) = &self.message {
                ui.label(RichText::new(message).color(palette(ui).warning));
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                if ui.add(crate::theme::primary_button(ui, "Fermer")).clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    outcome = Outcome::Close;
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
    ) {
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
    }

    // ─────────────────────────────── Comptes ───────────────────────────────

    fn accounts(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc) {
        let pal = palette(ui);
        if self.account >= doc.document.accounts.len() {
            self.account = 0;
        }
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(250.0);
                egui::ScrollArea::vertical()
                    .id_salt("acc_list")
                    .max_height(330.0)
                    .show(ui, |ui| {
                        for &i in &doc.document.account_order.clone() {
                            let a = &doc.document.accounts[i];
                            let mut label = RichText::new(if a.name.is_empty() {
                                "(sans nom)"
                            } else {
                                &a.name
                            });
                            if a.disabled {
                                label = label.color(pal.muted).strikethrough();
                            } else if a.is_virtual {
                                label = label.italics();
                            }
                            if ui.selectable_label(self.account == i, label).clicked()
                                && self.account != i
                            {
                                self.account = i;
                                self.buffers.clear();
                            }
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button(icon::ARROW_UP).on_hover_text("Monter").clicked() {
                        let i = self.account;
                        doc.edit_document(|d| manage::move_account(d, i, true));
                    }
                    if ui
                        .button(icon::ARROW_DOWN)
                        .on_hover_text("Descendre")
                        .clicked()
                    {
                        let i = self.account;
                        doc.edit_document(|d| manage::move_account(d, i, false));
                    }
                    if ui.button(format!("{}  Ajouter", icon::PLUS)).clicked() {
                        self.new_account = Some((
                            manage::unique_account_name(&doc.document, "Nouveau compte"),
                            "0".into(),
                        ));
                    }
                    if ui
                        .button(icon::TRASH)
                        .on_hover_text("Supprimer le compte")
                        .clicked()
                    {
                        self.confirm = Some(ConfirmDelete::Account(self.account));
                    }
                });
                if let Some((name, balance)) = &mut self.new_account {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Nouveau compte").family(crate::theme::medium()));
                    ui.add(
                        crate::theme::field(name)
                            .hint_text("Nom")
                            .desired_width(230.0),
                    );
                    ui.add(
                        crate::theme::field(balance)
                            .hint_text("Solde initial")
                            .desired_width(230.0),
                    );
                    let mut create = None;
                    ui.horizontal(|ui| {
                        if ui.add(crate::theme::primary_button(ui, "Créer")).clicked() {
                            create =
                                Some((name.clone(), Amount::parse(balance).unwrap_or_default()));
                        }
                        if ui.button("Annuler").clicked() {
                            create = Some((String::new(), Amount::ZERO));
                        }
                    });
                    if let Some((name, balance)) = create {
                        if !name.is_empty() {
                            let name = manage::unique_account_name(&doc.document, &name);
                            let index =
                                doc.edit_document(|d| manage::add_account(d, &name, balance));
                            self.account = index;
                            doc.select_account(index);
                            self.buffers.clear();
                        }
                        self.new_account = None;
                    }
                }
                if let Some(ConfirmDelete::Account(i)) = self.confirm {
                    let name = doc
                        .document
                        .accounts
                        .get(i)
                        .map(|a| a.name.clone())
                        .unwrap_or_default();
                    let count = doc
                        .document
                        .accounts
                        .get(i)
                        .map_or(0, |a| a.operations.len());
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("Supprimer « {name} » et ses {count} opérations ?"))
                            .color(pal.negative),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Supprimer").clicked() {
                            match doc.edit_document(|d| manage::delete_account(d, i)) {
                                Some(next) => {
                                    self.account = next;
                                    doc.select_account(next);
                                }
                                None => {
                                    self.message =
                                        Some("Le fichier doit garder au moins un compte.".into())
                                }
                            }
                            self.confirm = None;
                            self.buffers.clear();
                        }
                        if ui.button("Annuler").clicked() {
                            self.confirm = None;
                        }
                    });
                }
            });
            ui.separator();
            ui.vertical(|ui| self.account_fields(ui, doc));
        });
    }

    fn account_fields(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc) {
        let i = self.account;
        let a = doc.document.accounts[i].clone_header();
        macro_rules! field {
            ($ui:expr, $key:literal, $value:expr, $width:expr, |$d:ident, $v:ident| $body:expr) => {
                self.text_field($ui, $key, &$value, $width, |v| {
                    let $v = manage::clean(&v);
                    doc.edit_document(|$d| $body);
                });
            };
        }
        egui::Grid::new("acc_fields")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Nom du compte");
                field!(
                    ui,
                    "name",
                    a.name,
                    360.0,
                    |d, v| d.accounts[i].name = if v.trim().is_empty() {
                        format!("Compte {}", i + 1)
                    } else {
                        v
                    }
                );
                ui.end_row();
                ui.label("Banque");
                field!(ui, "bank", a.bank_name, 360.0, |d, v| d.accounts[i]
                    .bank_name = v);
                ui.end_row();
                ui.label("Agence");
                field!(ui, "agency", a.agency_name, 360.0, |d, v| d.accounts[i]
                    .agency_name =
                    v);
                ui.end_row();
                ui.label("Adresse");
                field!(ui, "address", a.agency_address, 360.0, |d, v| d.accounts
                    [i]
                    .agency_address =
                    v);
                ui.end_row();
                ui.label("Code banque / guichet");
                ui.horizontal(|ui| {
                    field!(ui, "bank_code", a.bank_code, 90.0, |d, v| d.accounts[i]
                        .bank_code =
                        v);
                    field!(ui, "agency_code", a.agency_code, 90.0, |d, v| d.accounts
                        [i]
                        .agency_code =
                        v);
                });
                ui.end_row();
                ui.label("N° de compte");
                field!(ui, "number", a.number, 200.0, |d, v| d.accounts[i].number =
                    v.to_uppercase());
                ui.end_row();
                ui.label("BIC");
                field!(ui, "bic", a.bic, 200.0, |d, v| d.accounts[i].bic =
                    v.to_uppercase());
                ui.end_row();
                ui.label("IBAN");
                field!(ui, "iban", a.iban, 360.0, |d, v| d.accounts[i].iban =
                    v.to_uppercase());
                ui.end_row();
                ui.label("Devise principale");
                ui.horizontal(|ui| {
                    field!(ui, "curr1", a.currency, 50.0, |d, v| d.accounts[i]
                        .currency =
                        if v.trim().is_empty() { "€".into() } else { v });
                    ui.label("taux");
                    field!(
                        ui,
                        "rate1",
                        a.currency_rate.to_string().replace('.', ","),
                        100.0,
                        |d, v| {
                            d.accounts[i].currency_rate =
                                v.replace(',', ".").trim().parse().unwrap_or(1.0)
                        }
                    );
                });
                ui.end_row();
                ui.label("Devise secondaire");
                ui.horizontal(|ui| {
                    field!(ui, "curr2", a.secondary_currency, 50.0, |d, v| d.accounts
                        [i]
                        .secondary_currency =
                        if v.trim().is_empty() { "F".into() } else { v });
                    ui.label("taux");
                    field!(
                        ui,
                        "rate2",
                        a.secondary_rate.to_string().replace('.', ","),
                        100.0,
                        |d, v| {
                            d.accounts[i].secondary_rate =
                                v.replace(',', ".").trim().parse().unwrap_or(6.55957)
                        }
                    );
                });
                ui.end_row();
            });
        ui.add_space(6.0);
        let mut is_virtual = a.is_virtual;
        if ui
            .checkbox(
                &mut is_virtual,
                "Ne pas inclure ce compte dans les totaux et statistiques (compte virtuel)",
            )
            .changed()
        {
            doc.edit_document(|d| d.accounts[i].is_virtual = is_virtual);
        }
        let mut disabled = a.disabled;
        if ui
            .checkbox(
                &mut disabled,
                "Compte inactif (masqué de la liste des comptes)",
            )
            .changed()
        {
            doc.edit_document(|d| d.accounts[i].disabled = disabled);
        }
    }

    // ─────────────────────────────── Catégories ───────────────────────────────

    fn categories(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc) {
        let pal = palette(ui);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(380.0);
                egui::ScrollArea::vertical().id_salt("cat_list").max_height(400.0).show(ui, |ui| {
                    for (pos, c) in doc.document.categories.iter().enumerate() {
                        let style = doc.category_styles.get(&c.id).copied();
                        let glyph = style.map_or(icon::TAG, |s| s.icon);
                        let color = style.map_or(pal.muted, |s| crate::theme::category_color(&pal, s.color));
                        let indent = if c.is_child { "        " } else { "" };
                        let text = RichText::new(format!("{indent}{glyph}  {}", c.name)).color(if self.category == Some(pos) { pal.fg } else { color });
                        let text = if c.is_child { text } else { text.family(crate::theme::medium()) };
                        if ui.selectable_label(self.category == Some(pos), text).clicked() {
                            self.category = Some(pos);
                            self.buffers.clear();
                        }
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui.button(format!("{}  Catégorie", icon::PLUS)).clicked() {
                        self.new_category = Some((String::new(), false));
                    }
                    if ui.button(format!("{}  Sous-catégorie", icon::PLUS)).clicked() {
                        self.new_category = Some((String::new(), true));
                    }
                });
                if let Some((name, child)) = &mut self.new_category {
                    let kind = if *child { "Nouvelle sous-catégorie" } else { "Nouvelle catégorie" };
                    ui.label(RichText::new(kind).family(crate::theme::medium()));
                    let response = ui.add(crate::theme::field(name).hint_text("Nom").desired_width(300.0));
                    response.request_focus();
                    let mut done = None;
                    ui.horizontal(|ui| {
                        if ui.add(crate::theme::primary_button(ui, "Ajouter")).clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                            done = Some(true);
                        }
                        if ui.button("Annuler").clicked() {
                            done = Some(false);
                        }
                    });
                    match done {
                        Some(true) if !name.trim().is_empty() => {
                            // Après la catégorie sélectionnée (et ses enfants pour une mère).
                            let (name, child) = (name.clone(), *child);
                            let before = self.category.map(|p| {
                                let mut end = p + 1;
                                if !child {
                                    while end < doc.document.categories.len() && doc.document.categories[end].is_child {
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
                ui.add_space(8.0);
                let Some(pos) = self.category.filter(|&p| p < doc.document.categories.len()) else {
                    ui.label(RichText::new("Sélectionnez une catégorie.").color(pal.muted));
                    return;
                };
                let c = doc.document.categories[pos].clone();
                ui.label("Nom");
                self.text_field(ui, "cat_name", &c.name, 300.0, |v| {
                    doc.edit_document(|d| manage::rename_category(d, pos, &v));
                });
                ui.add_space(6.0);
                let toggle = if c.is_child { "Changer en catégorie" } else { "Changer en sous-catégorie" };
                if ui.add_enabled(pos > 0, egui::Button::new(toggle)).clicked() {
                    doc.edit_document(|d| manage::toggle_child(d, pos));
                }
                ui.horizontal(|ui| {
                    if ui.button(format!("{}  Monter", icon::ARROW_UP)).clicked() {
                        let with = self.with_children;
                        self.category = doc.edit_document(|d| manage::move_category(d, pos, true, with)).or(Some(pos));
                    }
                    if ui.button(format!("{}  Descendre", icon::ARROW_DOWN)).clicked() {
                        let with = self.with_children;
                        self.category = doc.edit_document(|d| manage::move_category(d, pos, false, with)).or(Some(pos));
                    }
                });
                ui.checkbox(&mut self.with_children, "Déplacer avec les sous-catégories");
                ui.add_space(6.0);
                if ui.button(format!("{}  Trier par nom", icon::SORT_ASCENDING)).clicked() {
                    doc.edit_document(manage::sort_categories);
                    self.category = None;
                }
                ui.horizontal(|ui| {
                    if ui.button(format!("{}  Exporter…", icon::EXPORT)).clicked()
                        && let Some(path) = rfd::FileDialog::new().set_file_name("categories.txt").add_filter("Texte", &["txt"]).save_file()
                    {
                        let text = manage::export_categories(&doc.document);
                        self.message = Some(match std::fs::write(&path, text) {
                            Ok(()) => format!("Catégories exportées dans {}", path.display()),
                            Err(e) => format!("Export impossible : {e}"),
                        });
                    }
                    if ui.button(format!("{}  Importer…", icon::DOWNLOAD_SIMPLE)).clicked()
                        && let Some(path) = rfd::FileDialog::new().add_filter("Texte", &["txt"]).pick_file()
                    {
                        let text = std::fs::read(&path).map(|b| encoding_rs::UTF_8.decode(&b).0.into_owned()).unwrap_or_default();
                        let n = doc.edit_document(|d| manage::import_categories(d, &text));
                        self.message = Some(if n > 0 { format!("{n} catégories importées") } else { "Aucune catégorie lue dans ce fichier".into() });
                        self.category = None;
                    }
                });
                ui.add_space(6.0);
                if ui.button(RichText::new(format!("{}  Supprimer", icon::TRASH)).color(pal.negative)).clicked() {
                    self.confirm = Some(ConfirmDelete::Category(pos));
                }
                if let Some(ConfirmDelete::Category(p)) = self.confirm {
                    let usage = manage::category_usage(&doc.document, p);
                    let children = if c.is_child { "" } else { " et ses sous-catégories" };
                    ui.label(RichText::new(format!("Supprimer « {} »{children} ? {usage} opération(s) perdront leur catégorie.", c.name)).color(pal.negative));
                    ui.horizontal(|ui| {
                        if ui.button("Supprimer").clicked() {
                            doc.edit_document(|d| manage::delete_category(d, p));
                            self.confirm = None;
                            self.category = None;
                        }
                        if ui.button("Annuler").clicked() {
                            self.confirm = None;
                        }
                    });
                }
            });
        });
    }

    // ─────────────────────────────── Tiers ───────────────────────────────

    fn payees(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc) {
        let pal = palette(ui);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(280.0);
                ui.add(crate::theme::field(&mut self.payee_search).hint_text(format!("{}  Rechercher", icon::MAGNIFYING_GLASS)).desired_width(270.0));
                let needle = bp_core::filter::normalize(&self.payee_search);
                let needle = needle.trim();
                let mut order: Vec<usize> = (0..doc.document.payees.len()).collect();
                order.sort_by_key(|&i| bp_core::filter::normalize(&doc.document.payees[i].caption));
                egui::ScrollArea::vertical().id_salt("payee_list").max_height(360.0).show(ui, |ui| {
                    for i in order {
                        let p = &doc.document.payees[i];
                        let hay = bp_core::filter::normalize(&format!("{} {} {}", p.caption, p.payee, p.details));
                        if !needle.is_empty() && !hay.contains(needle) {
                            continue;
                        }
                        if ui.selectable_label(self.payee == Some(i), &p.caption).clicked() {
                            self.payee = Some(i);
                            self.buffers.clear();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(format!("{}  Nouveau tiers", icon::PLUS)).clicked() {
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
                        self.payee = Some(index);
                        self.buffers.clear();
                    }
                    if let Some(i) = self.payee
                        && ui.button(icon::TRASH).on_hover_text("Supprimer le tiers").clicked()
                    {
                        doc.edit_document(|d| {
                            if i < d.payees.len() {
                                d.payees.remove(i);
                            }
                        });
                        self.payee = None;
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let Some(i) = self.payee.filter(|&i| i < doc.document.payees.len()) else {
                    ui.label(RichText::new("Les tiers mémorisés remplissent la saisie : tapez le début du nom dans le champ Tiers.").color(pal.muted));
                    return;
                };
                let p = doc.document.payees[i].clone();
                egui::Grid::new("payee_fields").striped(false).num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Nom");
                    self.text_field(ui, "payee_caption", &p.caption, 320.0, |v| {
                        doc.edit_document(|d| d.payees[i].caption = manage::clean(&v));
                    });
                    ui.end_row();
                    ui.label("Mode");
                    let mut mode = p.mode.clone();
                    egui::ComboBox::from_id_salt("payee_mode").selected_text(mode.label().to_string()).width(320.0).show_ui(ui, |ui| {
                        for m in doc.document.mode_list() {
                            let label = m.label().to_string();
                            ui.selectable_value(&mut mode, m, label);
                        }
                    });
                    if mode != p.mode {
                        doc.edit_document(|d| d.payees[i].mode = mode);
                    }
                    ui.end_row();
                    ui.label("Tiers");
                    self.text_field(ui, "payee_payee", &p.payee, 320.0, |v| {
                        doc.edit_document(|d| d.payees[i].payee = manage::clean(&v));
                    });
                    ui.end_row();
                    ui.label("Détail");
                    self.text_field(ui, "payee_details", &p.details, 320.0, |v| {
                        doc.edit_document(|d| d.payees[i].details = manage::clean(&v));
                    });
                    ui.end_row();
                    ui.label("Montant");
                    self.text_field(ui, "payee_amount", &p.amount, 160.0, |v| {
                        doc.edit_document(|d| d.payees[i].amount = manage::clean(&v));
                    });
                    ui.end_row();
                    ui.label("Catégorie");
                    let mut category = p.category;
                    let selected = category.and_then(|id| doc.document.category_label(id)).unwrap_or_else(|| "(aucune)".into());
                    egui::ComboBox::from_id_salt("payee_cat").selected_text(selected).width(320.0).height(360.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut category, None, "(aucune)");
                        for c in &doc.document.categories {
                            let label = if c.is_child { format!("      {}", c.name) } else { c.name.clone() };
                            ui.selectable_value(&mut category, Some(c.id), label);
                        }
                    });
                    if category != p.category {
                        doc.edit_document(|d| d.payees[i].category = category);
                    }
                    ui.end_row();
                });
            });
        });
    }

    // ─────────────────────────────── Modes ───────────────────────────────

    fn modes(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc) {
        let pal = palette(ui);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(300.0);
                ui.label(RichText::new("Modes standard").small().color(pal.muted));
                for m in StandardMode::ALL {
                    let sign = if m.is_debit() { "dépense" } else { "recette" };
                    ui.label(RichText::new(format!("{}  ({sign})", m.label())).color(pal.muted));
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Modes personnalisés")
                        .small()
                        .color(pal.muted),
                );
                for (i, m) in doc.document.custom_modes.iter().enumerate() {
                    let sign = if m.debit { "dépense" } else { "recette" };
                    if ui
                        .selectable_label(self.mode == Some(i), format!("{}  ({sign})", m.name))
                        .clicked()
                    {
                        self.mode = Some(i);
                        self.buffers.clear();
                    }
                }
                if ui.button(format!("{}  Nouveau mode", icon::PLUS)).clicked() {
                    doc.edit_document(|d| manage::add_mode(d, "Nouveau mode", true));
                    self.mode = Some(doc.document.custom_modes.len() - 1);
                    self.buffers.clear();
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                let Some(i) = self.mode.filter(|&i| i < doc.document.custom_modes.len()) else {
                    ui.label(RichText::new("Sélectionnez un mode personnalisé.").color(pal.muted));
                    return;
                };
                let m = doc.document.custom_modes[i].clone();
                ui.label("Nom");
                let mut refused = false;
                self.text_field(ui, "mode_name", &m.name, 300.0, |v| {
                    refused = !doc.edit_document(|d| manage::rename_mode(d, i, &v));
                });
                if refused {
                    self.message =
                        Some("Nom refusé : vide, déjà utilisé ou nom d'un mode standard.".into());
                }
                let mut credit = !m.debit;
                if ui
                    .checkbox(&mut credit, "Mode de recette (montants positifs)")
                    .changed()
                {
                    doc.edit_document(|d| d.custom_modes[i].debit = !credit);
                }
                ui.add_space(6.0);
                if ui
                    .button(
                        RichText::new(format!("{}  Supprimer", icon::TRASH)).color(pal.negative),
                    )
                    .clicked()
                {
                    self.confirm = Some(ConfirmDelete::Mode(i));
                }
                if let Some(ConfirmDelete::Mode(k)) = self.confirm {
                    let usage = manage::mode_usage(&doc.document, k);
                    let replacement = if m.debit {
                        StandardMode::Carte
                    } else {
                        StandardMode::VirementRecu
                    };
                    ui.label(
                        RichText::new(format!(
                            "Supprimer « {} » ? {usage} opération(s) passeront en « {} ».",
                            m.name,
                            replacement.label()
                        ))
                        .color(pal.negative),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Supprimer").clicked() {
                            doc.edit_document(|d| {
                                manage::delete_mode(d, k, Mode::Standard(replacement))
                            });
                            self.confirm = None;
                            self.mode = None;
                        }
                        if ui.button("Annuler").clicked() {
                            self.confirm = None;
                        }
                    });
                }
            });
        });
    }
}
