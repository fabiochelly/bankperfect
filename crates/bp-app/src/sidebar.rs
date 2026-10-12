//! Barre latérale : comptes avec leur solde, total, état de protection.

use bp_core::model::Amount;
use egui::{Align, Layout, RichText, Sense};
use egui_phosphor::regular as icon;

use crate::commands::Command;
use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::palette;

/// Renvoie la commande choisie dans le menu contextuel d'un compte.
pub fn show(ui: &mut egui::Ui, doc: &mut OpenDoc) -> Option<Command> {
    let pal = palette(ui);
    ui.add_space(10.0);
    ui.label(RichText::new(&doc.document.group_name).heading());
    ui.label(RichText::new(doc.file_name()).small().color(pal.muted));
    ui.add_space(12.0);
    ui.label(
        RichText::new("COMPTES")
            .small()
            .family(crate::theme::medium())
            .color(pal.muted),
    );
    ui.add_space(2.0);

    let order = doc.document.account_order.clone();
    let mut clicked = None;
    let mut command = None;
    let mut total = Amount::ZERO;
    for &i in order
        .iter()
        .filter(|&&i| !doc.document.accounts[i].disabled)
    {
        let account = &doc.document.accounts[i];
        let balance = doc.account_balance(i);
        if !account.counts_as_virtual() {
            total = total + balance;
        }
        let (pressed, response) = account_row(ui, account, balance, i == doc.account, &pal);
        if pressed || response.secondary_clicked() {
            clicked = Some(i);
        }
        response.context_menu(|ui| command = account_menu(ui, doc));
    }
    let disabled: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&i| doc.document.accounts[i].disabled)
        .collect();
    if !disabled.is_empty() {
        ui.add_space(4.0);
        egui::CollapsingHeader::new(RichText::new("Comptes désactivés").small().color(pal.muted))
            .default_open(false)
            .show(ui, |ui| {
                for i in disabled {
                    let account = &doc.document.accounts[i];
                    let balance = doc.account_balance(i);
                    let (pressed, response) =
                        account_row(ui, account, balance, i == doc.account, &pal);
                    if pressed || response.secondary_clicked() {
                        clicked = Some(i);
                    }
                    response.context_menu(|ui| command = account_menu(ui, doc));
                }
            });
    }
    if let Some(i) = clicked {
        doc.select_account(i);
    }

    ui.add_space(8.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Total").family(crate::theme::semibold()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let color = if total.is_negative() {
                pal.negative
            } else {
                ui.visuals().strong_text_color()
            };
            ui.label(
                RichText::new(money(total, "€"))
                    .family(crate::theme::semibold())
                    .color(color),
            );
        });
    });
    ui.label(
        RichText::new("comptes réels uniquement")
            .small()
            .color(pal.muted),
    );

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        ui.add_space(8.0);
        protection_badge(ui, doc);
    });
    command
}

/// Menu contextuel d'un compte (le compte est affiché dès le clic droit).
fn account_menu(ui: &mut egui::Ui, doc: &OpenDoc) -> Option<Command> {
    use crate::commands::Command::*;
    crate::menu::popup_style(ui.style_mut());
    crate::menu::begin(ui);
    let mut chosen = None;
    let mut entry = |ui: &mut egui::Ui, command: Command| {
        let info = command.info(Some(doc));
        if crate::menu::item_enabled(
            ui,
            info.enabled,
            Some(info.icon),
            &info.label,
            info.shortcut,
        )
        .clicked()
        {
            chosen = Some(command);
        }
    };
    entry(ui, Manage(crate::manage_ui::Tab::Accounts));
    crate::menu::separator(ui);
    entry(ui, Report(crate::reports_ui::ReportTab::Statement));
    entry(ui, Charts);
    entry(ui, Scheduler);
    entry(ui, ExportXlsx);
    // Commandes d'extension placées ici (`menu = "account"`).
    let placed = crate::extensions::placed("account");
    if !placed.is_empty() {
        crate::menu::separator(ui);
        for (i, _) in placed {
            entry(ui, Extension(i));
        }
    }
    chosen
}

fn account_row(
    ui: &mut egui::Ui,
    account: &bp_core::model::Account,
    balance: Amount,
    selected: bool,
    pal: &crate::theme::Palette,
) -> (bool, egui::Response) {
    let height = 34.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click());
    let visuals = ui.visuals();
    if selected {
        ui.painter()
            .rect_filled(rect, 6.0, visuals.selection.bg_fill);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 6.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 0.0))));
    // Le solde est placé en premier (à droite) pour que le nom soit tronqué
    // plutôt que de le chevaucher.
    child.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let mut text = RichText::new(money(balance, &account.currency));
        if balance.is_negative() {
            text = text.color(pal.negative);
        }
        ui.label(text);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let glyph = if account.counts_as_virtual() {
                icon::WALLET
            } else {
                icon::BANK
            };
            ui.label(RichText::new(glyph).color(pal.muted));
            let mut name = RichText::new(if account.name.is_empty() {
                "(sans nom)"
            } else {
                &account.name
            });
            if account.counts_as_virtual() {
                name = name.italics();
            }
            ui.add(egui::Label::new(name).truncate());
        });
    });
    // Sélection dès l'appui, comme une liste native.
    let pressed = response.hovered() && ui.input(|i| i.pointer.primary_pressed());
    let response = response.on_hover_text(format!("{}\n{}", account.bank_name, account.iban));
    (pressed, response)
}

fn protection_badge(ui: &mut egui::Ui, doc: &OpenDoc) {
    let pal = palette(ui);
    let (glyph, text, color) = match (&doc.protection, &doc.legacy_password) {
        (Some(p), _) if p.is_password_protected() => {
            (icon::SHIELD_CHECK, "Chiffré par mot de passe", pal.positive)
        }
        (None, Some(_)) => (
            icon::LOCK_SIMPLE,
            "Sera chiffré à l'enregistrement",
            pal.pointed,
        ),
        _ => (icon::LOCK_SIMPLE_OPEN, "Fichier non protégé", pal.negative),
    };
    ui.label(
        RichText::new(format!("{glyph}  {text}"))
            .small()
            .color(color),
    );
}
