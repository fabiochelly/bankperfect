//! Barre latérale : le compte affiché en carte bancaire, les autres comptes
//! avec leur solde, le total, l'état de protection.

use bp_core::model::Amount;
use bp_core::tr;
use egui::text::{LayoutJob, TextWrapping};
use egui::{Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, pos2, vec2};
use egui_phosphor::regular as icon;

use crate::commands::Command;
use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::palette;

/// Renvoie la commande choisie dans le menu contextuel d'un compte.
pub fn show(ui: &mut egui::Ui, doc: &mut OpenDoc) -> Option<Command> {
    let pal = palette(ui);
    ui.add_space(2.0);
    ui.label(RichText::new(&doc.document.group_name).heading());
    ui.label(RichText::new(doc.file_name()).small().color(pal.muted));
    ui.add_space(14.0);

    let mut command = None;
    let card = bank_card(ui, doc);
    card.context_menu(|ui| command = account_menu(ui, doc));
    ui.add_space(18.0);

    let order = doc.document.account_order.clone();
    let enabled: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&i| !doc.document.accounts[i].disabled)
        .collect();
    let total = enabled
        .iter()
        .filter(|&&i| !doc.document.accounts[i].counts_as_virtual())
        .fold(Amount::ZERO, |sum, &i| sum + doc.account_balance(i));
    let others: Vec<usize> = enabled
        .iter()
        .copied()
        .filter(|&i| i != doc.account)
        .collect();
    // La liste des comptes défile quand elle ne tient pas : au-delà d'une
    // douzaine de comptes, les derniers passaient sous le total. Sa hauteur
    // laisse la place du pied (total, protection), mesuré à l'image
    // précédente.
    if !others.is_empty() {
        ui.label(
            RichText::new(tr!("AUTRES COMPTES"))
                .small()
                .family(crate::theme::medium())
                .color(pal.muted),
        );
        ui.add_space(2.0);
    }
    let footer_id = ui.id().with("sidebar_footer");
    let footer_h = ui.data(|d| d.get_temp::<f32>(footer_id)).unwrap_or(96.0);
    let mut clicked = None;
    ui.scope(|ui| {
        // Barre pleine et fine : flottante, elle masquait la fin des soldes.
        ui.spacing_mut().scroll = egui::style::ScrollStyle {
            bar_width: 6.0,
            ..egui::style::ScrollStyle::solid()
        };
        egui::ScrollArea::vertical()
            .id_salt("sidebar_accounts")
            .max_height((ui.available_height() - footer_h).max(80.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for i in others {
                    let account = &doc.document.accounts[i];
                    let balance = doc.account_balance(i);
                    let (pressed, response) = account_row(ui, account, balance, false, &pal);
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
                    egui::CollapsingHeader::new(
                        RichText::new(tr!("Comptes désactivés"))
                            .small()
                            .color(pal.muted),
                    )
                    .id_salt("Comptes désactivés")
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
            });
    });
    if let Some(i) = clicked {
        doc.select_account(i);
    }

    let footer_top = ui.cursor().top();
    ui.add_space(8.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr!("Total")).family(crate::theme::semibold()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let color = if total.is_negative() {
                pal.negative
            } else {
                ui.visuals().strong_text_color()
            };
            ui.label(
                RichText::new(money(total, doc.main_currency()))
                    .family(crate::theme::semibold())
                    .color(color),
            );
        });
    });
    ui.label(
        RichText::new(tr!("comptes réels uniquement"))
            .small()
            .color(pal.muted),
    );
    // Pied : le total ci-dessus, un écart, puis le badge de protection.
    let badge_h = ui.text_style_height(&egui::TextStyle::Small) + 2.0 * ui.spacing().item_spacing.y;
    let footer = ui.cursor().top() - footer_top + 8.0 + badge_h;
    if (footer - footer_h).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(footer_id, footer));
        ui.ctx().request_repaint();
    }

    // En bas de l'île, avec la même marge qu'à gauche (celle de l'île).
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        protection_badge(ui, doc);
    });
    command
}

/// Le compte affiché, en carte bancaire : dégradé aux couleurs du logo,
/// banque et puce, nom, solde, fin de l'IBAN et dernier rapprochement.
fn bank_card(ui: &mut egui::Ui, doc: &OpenDoc) -> egui::Response {
    let account = doc.current_account();
    let balance = doc.account_balance(doc.account);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 118.0), Sense::click());
    let painter = ui.painter();
    let radius = 14.0;

    // Bleu du « b » vers violet du « p », avec un reflet vert en haut à droite.
    let stops = [
        (0.0, Color32::from_rgb(0x3d, 0x59, 0xa1)),
        (0.52, Color32::from_rgb(0x5b, 0x4f, 0xb8)),
        (1.0, Color32::from_rgb(0x8b, 0x6f, 0xd6)),
    ];
    let shine_at = pos2(
        rect.right() + rect.width() * 0.05,
        rect.top() - rect.height() * 0.1,
    );
    let shine_reach = rect.width().max(rect.height()) * 0.6;
    let shine = Color32::from_rgb(0xb9, 0xf2, 0x8a);
    crate::islands::rounded_gradient(painter, rect, radius, |p: Pos2| {
        let t = (p.x - rect.left()) / rect.width() * 0.6 + (p.y - rect.top()) / rect.height() * 0.4;
        let base = crate::islands::gradient(&stops, t);
        let glow = (1.0 - p.distance(shine_at) / shine_reach)
            .max(0.0)
            .powf(1.5)
            * 0.55;
        crate::islands::lerp(base, shine, glow)
    });
    // Liseré fin, plus clair en haut : un léger relief, sans ombre.
    crate::islands::lit_edge(painter, rect, radius, 1.0, 64, 10);

    let white = |alpha: u8| Color32::from_white_alpha(alpha);
    let inner = rect.shrink2(vec2(14.0, 12.0));
    // Puce.
    let chip = Rect::from_min_size(pos2(inner.right() - 24.0, inner.top()), vec2(24.0, 18.0));
    painter.rect_filled(chip, 4.0, Color32::from_rgb(0xd6, 0xc4, 0x8a));
    painter.rect_stroke(
        chip.shrink(4.0),
        2.0,
        egui::Stroke::new(1.0, Color32::from_rgb(0xb8, 0xa4, 0x6a)),
        egui::StrokeKind::Inside,
    );
    let text = |text: &str,
                pos: Pos2,
                size: f32,
                family: egui::FontFamily,
                color: Color32,
                width: f32,
                align: Align2| {
        let mut job =
            LayoutJob::simple_singleline(text.to_string(), FontId::new(size, family), color);
        job.wrap = TextWrapping {
            max_width: width.max(10.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let galley = painter.layout_job(job);
        let rect = align.anchor_size(pos, galley.size());
        painter.galley(rect.min, galley, color);
    };
    // Le nom du compte, en grand, une seule fois : la banque n'apparaît
    // que si elle ajoute quelque chose (souvent, le compte porte son nom).
    let name = if account.name.trim().is_empty() {
        tr!("(sans nom)")
    } else {
        account.name.trim()
    };
    text(
        name,
        pos2(inner.left(), inner.top() - 1.0),
        16.0,
        crate::theme::semibold(),
        Color32::WHITE,
        inner.width() - 34.0,
        Align2::LEFT_TOP,
    );
    let bank = account.bank_name.trim();
    let (name_key, bank_key) = (name.to_lowercase(), bank.to_lowercase());
    if !bank.is_empty() && !name_key.contains(&bank_key) && !bank_key.contains(&name_key) {
        text(
            bank,
            pos2(inner.left(), inner.bottom() - 44.0),
            11.0,
            crate::theme::medium(),
            white(210),
            inner.width(),
            Align2::LEFT_BOTTOM,
        );
    }
    text(
        &money(balance, &account.currency).replacen('-', "\u{2212}", 1),
        pos2(inner.left(), inner.bottom() - 17.0),
        21.0,
        crate::theme::semibold(),
        Color32::WHITE,
        inner.width(),
        Align2::LEFT_BOTTOM,
    );
    let digits: String = account
        .iban
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if digits.len() >= 4 {
        let tail = &digits[digits.len() - 4..];
        text(
            &format!("•••• {tail}"),
            inner.left_bottom(),
            10.5,
            crate::theme::medium(),
            white(200),
            inner.width() * 0.5,
            Align2::LEFT_BOTTOM,
        );
    }
    // Pointage simple : pas de rapprochement à dater.
    if let Some(date) = doc.totals().last_reconciled
        && !doc.disable_reconcile
    {
        let d = date.to_string();
        text(
            &tr!("rapproché {}", &d[..5.min(d.len())]),
            inner.right_bottom(),
            10.5,
            crate::theme::medium(),
            white(200),
            inner.width() * 0.5,
            Align2::RIGHT_BOTTOM,
        );
    }
    crate::ui_kit::hover(response, &bank_details(account))
}

/// Banque et IBAN d'un compte, sur deux lignes (vide s'il n'y a ni l'un ni
/// l'autre).
fn bank_details(account: &bp_core::model::Account) -> String {
    [account.bank_name.trim(), account.iban.trim()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
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
                tr!("(sans nom)")
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
    let response = crate::ui_kit::hover(response, &bank_details(account));
    (pressed, response)
}

fn protection_badge(ui: &mut egui::Ui, doc: &OpenDoc) {
    let pal = palette(ui);
    let (glyph, text, color) = match (&doc.protection, &doc.legacy_password) {
        (Some(p), _) if p.is_password_protected() => (
            icon::SHIELD_CHECK,
            tr!("Chiffré par mot de passe"),
            pal.positive,
        ),
        (None, Some(_)) => (
            icon::LOCK_SIMPLE,
            tr!("Sera chiffré à l'enregistrement"),
            pal.pointed,
        ),
        _ => (
            icon::LOCK_SIMPLE_OPEN,
            tr!("Fichier non protégé"),
            pal.negative,
        ),
    };
    ui.label(
        RichText::new(format!("{glyph}  {text}"))
            .small()
            .color(color),
    );
}
