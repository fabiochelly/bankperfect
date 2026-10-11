//! Transferts de compte à compte (Ctrl+Maj+T), conversion d'une ligne en
//! transfert et ventilation d'une opération (F6).

use bp_core::model::{Amount, Date};
use bp_core::split::{self, SplitPart, TransferKind, TransferRequest};
use egui::{Key, RichText, vec2};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::{Palette, palette};
use crate::tools::Outcome;
use crate::ui_kit;

/// Derniers paramètres d'un transfert, gardés dans le fichier.
const LAST_KEY: &str = "transfer.v6";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LastTransfer {
    kind: usize,
    category: Option<u32>,
    details: String,
    from: String,
    to: String,
}

fn account_index(doc: &OpenDoc, uid: &str) -> Option<usize> {
    doc.document
        .accounts
        .iter()
        .position(|a| a.uid == uid && !a.disabled)
}

/// Liste déroulante des comptes actifs.
fn account_combo(
    ui: &mut egui::Ui,
    id: &str,
    doc: &OpenDoc,
    value: &mut usize,
    width: f32,
    except: Option<usize>,
) {
    let name = doc
        .document
        .accounts
        .get(*value)
        .map(|a| a.name.clone())
        .unwrap_or_default();
    egui::ComboBox::from_id_salt(id)
        .selected_text(name)
        .width(width)
        .show_ui(ui, |ui| {
            for &i in &doc.document.account_order {
                let account = &doc.document.accounts[i];
                if account.disabled || Some(i) == except {
                    continue;
                }
                ui.selectable_value(value, i, format!("{}  {}", icon::BANK, account.name));
            }
        });
}

// ─────────────────────────── Transfert ───────────────────────────

pub struct TransferForm {
    date: String,
    kind: TransferKind,
    from: usize,
    to: usize,
    amount: String,
    details: String,
    category: Option<u32>,
    error: Option<String>,
}

impl TransferForm {
    pub fn new(doc: &OpenDoc) -> Self {
        let last: LastTransfer = bp_core::feature_store::get(&doc.document, LAST_KEY)
            .and_then(Result::ok)
            .unwrap_or_default();
        let from = account_index(doc, &last.from).unwrap_or(doc.account);
        let to = account_index(doc, &last.to)
            .filter(|&t| t != from)
            .or_else(|| {
                doc.document
                    .account_order
                    .iter()
                    .copied()
                    .find(|&i| i != from && !doc.document.accounts[i].disabled)
            })
            .unwrap_or(from);
        Self {
            date: Date::today().to_string(),
            kind: TransferKind::ALL
                .get(last.kind)
                .copied()
                .unwrap_or(TransferKind::Transfer),
            from,
            to,
            amount: String::new(),
            details: last.details,
            category: last.category,
            error: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.5, 640.0, 820.0);
        let mut submit = false;
        crate::dialogs::modal(ctx, "transfer").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::ARROWS_LEFT_RIGHT, pal.accent, "Transfert de compte à compte");
            ui_kit::subtitle(ui, "Deux lignes liées sont créées : une sortie dans le compte débité, une entrée dans le compte crédité.");
            let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
            ui_kit::section(ui, width, icon::BANK, "Comptes", |ui| {
                let half = (inner - 60.0) / 2.0;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Débité").color(pal.negative));
                        account_combo(ui, "transfer_from", doc, &mut self.from, half, None);
                    });
                    ui.vertical(|ui| {
                        ui.add_space(20.0);
                        if ui.add_sized(vec2(40.0, 32.0), egui::Button::new(icon::ARROWS_LEFT_RIGHT)).on_hover_text("Inverser").clicked() {
                            std::mem::swap(&mut self.from, &mut self.to);
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Crédité").color(pal.positive));
                        account_combo(ui, "transfer_to", doc, &mut self.to, half, Some(self.from));
                    });
                });
            });
            ui_kit::section(ui, width, icon::CURRENCY_EUR, "Transfert", |ui| {
                ui.horizontal(|ui| {
                    crate::datepicker::date_field(ui, "transfer_date", &mut self.date, 200.0, crate::datepicker::Placement::Popup);
                    ui.add_space(12.0);
                    let amount = ui.add(crate::theme::field(&mut self.amount).hint_text("Montant").desired_width(inner - 200.0 - 12.0 - 8.0));
                    if amount.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        submit = true;
                    }
                    if self.amount.is_empty() && !amount.has_focus() && !amount.lost_focus() && ui.ctx().memory(|m| m.focused().is_none()) {
                        amount.request_focus();
                    }
                });
                ui.add_space(10.0);
                let options: Vec<(TransferKind, &str, &str, egui::Color32)> = TransferKind::ALL
                    .iter()
                    .map(|&k| {
                        let glyph = match k {
                            TransferKind::Transfer => icon::ARROWS_LEFT_RIGHT,
                            TransferKind::Cheque => icon::NOTE_PENCIL,
                            TransferKind::Cash => icon::MONEY,
                        };
                        (k, glyph, k.label(), pal.accent)
                    })
                    .collect();
                ui_kit::segmented(ui, &mut self.kind, &options, 34.0, Some(inner));
                ui.add_space(4.0);
                let (out_mode, in_mode) = self.kind.modes();
                ui.label(
                    RichText::new(format!(
                        "Modes : « {} » dans le compte débité, « {} » dans le compte crédité",
                        bp_core::model::Mode::Standard(out_mode).label(),
                        bp_core::model::Mode::Standard(in_mode).label()
                    ))
                    .size(12.5)
                    .color(pal.muted),
                );
            });
            ui_kit::section(ui, width, icon::TAG, "Détail et catégorie", |ui| {
                ui.add(crate::theme::field(&mut self.details).hint_text("Détail (facultatif)").desired_width(inner));
                ui.add_space(8.0);
                ui_kit::category_combo(ui, "transfer_category", &doc.document, &doc.category_styles, &mut self.category, inner);
            });
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, &format!("{}  Créer le transfert", icon::CHECK), true).clicked() {
                    submit = true;
                }
                if crate::dialogs::button(ui, "Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            });
        });
        if submit {
            outcome = self.submit(doc).unwrap_or_else(|e| {
                self.error = Some(e);
                Outcome::Keep
            });
        }
        outcome
    }

    fn submit(&mut self, doc: &mut OpenDoc) -> Result<Outcome, String> {
        let date = Date::parse_fr(&self.date, Date::today()).ok_or("Date invalide")?;
        let amount = Amount::parse(&self.amount)
            .filter(|a| a.0 != 0)
            .ok_or("Montant invalide")?;
        let request = TransferRequest {
            date,
            kind: self.kind,
            details: self.details.trim().to_string(),
            category: self.category,
            amount: amount.abs(),
            from: self.from,
            to: self.to,
        };
        let lines = doc
            .edit_accounts("transfert", &[self.from, self.to], |d| {
                split::create_transfer(d, &request)
            })
            .map_err(|e| capitalize(&e.to_string()))?;
        let last = LastTransfer {
            kind: TransferKind::ALL
                .iter()
                .position(|&k| k == self.kind)
                .unwrap_or(0),
            category: self.category,
            details: request.details.clone(),
            from: doc.document.accounts[self.from].uid.clone(),
            to: doc.document.accounts[self.to].uid.clone(),
        };
        let _ = bp_core::feature_store::set(&mut doc.document, LAST_KEY, &last);
        for (account, line) in [lines.first, lines.second] {
            if account == doc.account {
                doc.select_lines(&[line]);
            }
        }
        Ok(Outcome::Done(format!(
            "Transfert de {} de « {} » vers « {} »",
            money(amount.abs(), &doc.document.accounts[self.from].currency),
            doc.document.accounts[self.from].name,
            doc.document.accounts[self.to].name
        )))
    }
}

// ─────────────────────────── Conversion ───────────────────────────

pub struct ConvertForm {
    line: usize,
    other: usize,
    error: Option<String>,
}

impl ConvertForm {
    pub fn new(doc: &OpenDoc, line: usize) -> Self {
        let other = doc
            .document
            .account_order
            .iter()
            .copied()
            .find(|&i| i != doc.account && !doc.document.accounts[i].disabled)
            .unwrap_or(doc.account);
        Self {
            line,
            other,
            error: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.45, 600.0, 760.0);
        let mut submit = false;
        let op = doc.operation(self.line).clone();
        crate::dialogs::modal(ctx, "convert_transfer").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::ARROWS_LEFT_RIGHT, pal.accent, "Convertir en transfert");
            ui_kit::subtitle(ui, "La ligne devient un transfert lié : sa contrepartie est créée dans le compte choisi.");
            let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
            ui_kit::section(ui, width, icon::LIST_BULLETS, "Ligne", |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(op.date.to_string()).color(pal.fg_dark));
                    ui.add_space(8.0);
                    ui.label(RichText::new(&op.payee).family(crate::theme::medium()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let color = if op.amount.is_negative() { pal.negative } else { pal.positive };
                        ui.label(RichText::new(money(op.amount, &doc.current_account().currency)).color(color));
                    });
                });
            });
            let label = if op.amount.is_negative() { "Compte crédité" } else { "Compte débité" };
            ui_kit::section(ui, width, icon::BANK, label, |ui| {
                account_combo(ui, "convert_other", doc, &mut self.other, inner, Some(doc.account));
                ui.add_space(4.0);
                if let Some(mode) = split::reciprocal_mode(&op.mode) {
                    ui.label(
                        RichText::new(format!("Mode de la contrepartie : « {} »", bp_core::model::Mode::Standard(mode).label()))
                            .size(12.5)
                            .color(pal.muted),
                    );
                }
            });
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, &format!("{}  Convertir", icon::CHECK), true).clicked() {
                    submit = true;
                }
                if crate::dialogs::button(ui, "Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            });
        });
        if submit {
            let (account, line, other) = (doc.account, self.line, self.other);
            match doc.edit_accounts("conversion en transfert", &[account, other], |d| {
                split::convert_to_transfer(d, account, line, other)
            }) {
                Ok(_) => {
                    doc.select_lines(&[line]);
                    outcome = Outcome::Done(format!(
                        "Ligne liée au compte « {} »",
                        doc.document.accounts[other].name
                    ));
                }
                Err(e) => self.error = Some(capitalize(&e.to_string())),
            }
        }
        outcome
    }
}

// ─────────────────────────── Ventilation ───────────────────────────

struct PartDraft {
    details: String,
    category: Option<u32>,
    amount: String,
}

pub struct SplitForm {
    line: usize,
    parts: Vec<PartDraft>,
    error: Option<String>,
}

impl SplitForm {
    pub fn new(doc: &OpenDoc, line: usize) -> Self {
        let op = doc.operation(line);
        let halves = split::split_evenly(op.amount.abs(), 2);
        let parts = halves
            .iter()
            .enumerate()
            .map(|(k, a)| PartDraft {
                details: if k == 0 {
                    op.details.clone()
                } else {
                    String::new()
                },
                category: if k == 0 { op.category } else { None },
                amount: money(*a, ""),
            })
            .collect();
        Self {
            line,
            parts,
            error: None,
        }
    }

    fn parsed(&self) -> Vec<Option<Amount>> {
        self.parts
            .iter()
            .map(|p| Amount::parse(&p.amount).map(Amount::abs))
            .collect()
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.6, 760.0, 1000.0);
        let mut submit = false;
        let op = doc.operation(self.line).clone();
        let currency = doc.current_account().currency.clone();
        crate::dialogs::modal(ctx, "split").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::ARROWS_SPLIT, pal.accent, "Ventiler l'opération");
            ui_kit::subtitle(ui, "L'opération est remplacée par une ligne par part, chacune avec son détail, sa catégorie et son montant.");
            ui_kit::card(ui, width, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(op.date.to_string()).color(pal.fg_dark));
                    ui.add_space(8.0);
                    ui.label(RichText::new(&op.payee).family(crate::theme::medium()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let color = if op.amount.is_negative() { pal.negative } else { pal.positive };
                        ui.label(RichText::new(money(op.amount, &currency)).size(18.0).family(crate::theme::semibold()).color(color));
                    });
                });
            });
            ui.add_space(12.0);
            self.parts_table(ui, doc, width, &pal);
            let amounts = self.parsed();
            let total: Amount = amounts.iter().flatten().copied().sum();
            let remaining = Amount(op.amount.0.abs() - total.0);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(format!("{}  Ajouter une part", icon::PLUS)).clicked() {
                    self.parts.push(PartDraft {
                        details: String::new(),
                        category: None,
                        amount: if remaining.0 > 0 { money(remaining, "") } else { String::new() },
                    });
                }
                if ui.button(format!("{}  Répartir également", icon::EQUALS)).on_hover_text("Même montant pour chaque part").clicked() {
                    let shares = split::split_evenly(op.amount.abs(), self.parts.len());
                    for (p, a) in self.parts.iter_mut().zip(shares) {
                        p.amount = money(a, "");
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (text, color) = match remaining.0.cmp(&0) {
                        std::cmp::Ordering::Equal => (format!("{}  Tout est ventilé", icon::CHECK_CIRCLE), pal.positive),
                        std::cmp::Ordering::Greater => (format!("Reste à ventiler : {}", money(remaining, &currency)), pal.warning),
                        std::cmp::Ordering::Less => (format!("Dépassement : {}", money(Amount(-remaining.0), &currency)), pal.negative),
                    };
                    ui.label(RichText::new(text).family(crate::theme::medium()).color(color));
                });
            });
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            crate::dialogs::footer(ui, |ui| {
                let ready = remaining.0 == 0 && amounts.iter().all(|a| a.is_some_and(|a| a.0 > 0)) && self.parts.len() >= 2;
                if crate::dialogs::primary(ui, &format!("{}  Ventiler en {} lignes", icon::CHECK, self.parts.len()), ready).clicked() {
                    submit = true;
                }
                if crate::dialogs::button(ui, "Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            });
        });
        if submit {
            let parts: Vec<SplitPart> = self
                .parts
                .iter()
                .zip(self.parsed())
                .map(|(p, a)| SplitPart {
                    details: p.details.trim().to_string(),
                    category: p.category,
                    amount: a.unwrap_or_default(),
                })
                .collect();
            let (account, line) = (doc.account, self.line);
            match doc.edit_accounts("ventilation", &[account], |d| {
                split::split_operation(d, account, line, &parts)
            }) {
                Ok(lines) => {
                    doc.select_lines(&lines);
                    outcome =
                        Outcome::Done(format!("Opération ventilée en {} lignes", lines.len()));
                }
                Err(e) => self.error = Some(capitalize(&e.to_string())),
            }
        }
        outcome
    }

    fn parts_table(&mut self, ui: &mut egui::Ui, doc: &OpenDoc, width: f32, pal: &Palette) {
        let amount_w = 150.0;
        let remove_w = 36.0;
        let gap = 10.0;
        let rest = width - amount_w - remove_w - 40.0 - 3.0 * gap;
        let details_w = rest * 0.5;
        let category_w = rest - details_w;
        let mut remove = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui_kit::cell(
                ui,
                30.0,
                egui::Align::Max,
                RichText::new("N°").color(pal.fg_dark),
            );
            ui_kit::cell(
                ui,
                details_w,
                egui::Align::Min,
                RichText::new("Détail")
                    .family(crate::theme::semibold())
                    .color(pal.fg_dark),
            );
            ui_kit::cell(
                ui,
                category_w,
                egui::Align::Min,
                RichText::new("Catégorie")
                    .family(crate::theme::semibold())
                    .color(pal.fg_dark),
            );
            ui_kit::cell(
                ui,
                amount_w,
                egui::Align::Max,
                RichText::new("Montant")
                    .family(crate::theme::semibold())
                    .color(pal.fg_dark),
            );
        });
        egui::ScrollArea::vertical()
            .id_salt("split_parts")
            .max_height(ui_kit::dialog_height(ui.ctx(), 0.45))
            .show(ui, |ui| {
                let can_remove = self.parts.len() > 2;
                for (k, part) in self.parts.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        ui_kit::cell(
                            ui,
                            30.0,
                            egui::Align::Max,
                            RichText::new(format!("{}", k + 1)).color(pal.muted),
                        );
                        ui.add(
                            crate::theme::field(&mut part.details)
                                .hint_text("Détail")
                                .desired_width(details_w),
                        );
                        ui_kit::category_combo(
                            ui,
                            &format!("split_cat_{k}"),
                            &doc.document,
                            &doc.category_styles,
                            &mut part.category,
                            category_w,
                        );
                        ui.add(
                            crate::theme::field(&mut part.amount)
                                .horizontal_align(egui::Align::Max)
                                .hint_text("0,00")
                                .desired_width(amount_w),
                        );
                        if can_remove
                            && ui
                                .add(
                                    egui::Button::new(RichText::new(icon::TRASH).color(pal.muted))
                                        .frame(false),
                                )
                                .on_hover_text("Retirer cette part")
                                .clicked()
                        {
                            remove = Some(k);
                        }
                    });
                    ui.add_space(6.0);
                }
            });
        if let Some(k) = remove {
            self.parts.remove(k);
        }
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}
