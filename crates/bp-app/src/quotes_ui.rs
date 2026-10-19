//! Mise à jour des cotations : valeur des lignes « Titre » du compte courant
//! d'après le cours du jour (code ISIN ou symbole dans le détail, quantité
//! en tête du tiers).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bp_core::currencies::Rates;
use bp_core::i18n;
use bp_core::model::Amount;
use bp_core::quotes::{self, Quote, StockLine};
use bp_core::{tr, tr_mark, trc, trn};
use egui::{Key, RichText, vec2};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::{Palette, palette};
use crate::tools::Outcome;
use crate::ui_kit;

/// Avancement du téléchargement, partagé avec le fil de travail.
#[derive(Default)]
struct Progress {
    done: usize,
    quotes: HashMap<String, Result<Quote, String>>,
    rates: Option<Rates>,
    finished: bool,
}

pub struct QuotesForm {
    lines: Vec<StockLine>,
    progress: Option<Arc<Mutex<Progress>>>,
    quotes: HashMap<String, Result<Quote, String>>,
    rates: Rates,
    downloaded: bool,
    message: Option<String>,
}

impl QuotesForm {
    pub fn new(doc: &OpenDoc) -> Self {
        Self {
            lines: quotes::stock_lines(doc.current_account()),
            progress: None,
            quotes: HashMap::new(),
            rates: crate::currency_ui::saved_rates(),
            downloaded: false,
            message: None,
        }
    }

    fn start(&mut self, ctx: &egui::Context, doc: &OpenDoc) {
        let mut identifiers: Vec<String> =
            self.lines.iter().map(|l| l.identifier.clone()).collect();
        identifiers.sort();
        identifiers.dedup();
        let progress = Arc::new(Mutex::new(Progress::default()));
        let shared = progress.clone();
        let ctx = ctx.clone();
        let account_currency = quotes::iso_currency(&doc.current_account().currency);
        let secondary = quotes::iso_currency(&doc.current_account().secondary_currency);
        let have_rates = !self.rates.per_euro.is_empty();
        std::thread::spawn(move || {
            let mut need_rates = false;
            for id in identifiers {
                let result = fetch_quote(&id);
                if let Ok(q) = &result
                    && q.currency != account_currency
                    && q.currency != secondary
                {
                    need_rates = true;
                }
                if let Ok(mut p) = shared.lock() {
                    p.done += 1;
                    p.quotes.insert(id, result);
                }
                ctx.request_repaint();
            }
            // Cours de la BCE pour les titres cotés dans une autre devise.
            let rates = (need_rates && !have_rates)
                .then(crate::currency_ui::fetch_ecb)
                .and_then(Result::ok);
            if let Ok(mut p) = shared.lock() {
                p.rates = rates;
                p.finished = true;
            }
            ctx.request_repaint();
        });
        self.progress = Some(progress);
        self.quotes.clear();
        self.message = None;
    }

    fn poll(&mut self) -> Option<(usize, usize)> {
        let progress = self.progress.as_ref()?;
        let mut p = progress.lock().ok()?;
        for (id, q) in p.quotes.drain() {
            self.quotes.insert(id, q);
        }
        if p.finished {
            if let Some(rates) = p.rates.take() {
                self.rates = rates;
            }
            drop(p);
            self.progress = None;
            self.downloaded = true;
            return None;
        }
        Some((
            p.done,
            self.lines
                .iter()
                .map(|l| &l.identifier)
                .collect::<std::collections::HashSet<_>>()
                .len(),
        ))
    }

    /// Nouvelle valeur d'une ligne, ou la raison de son absence.
    fn new_value(&self, line: &StockLine, doc: &OpenDoc) -> Option<Result<(f64, Amount), String>> {
        let quote = self.quotes.get(&line.identifier)?;
        Some(quote.clone().and_then(|q| {
            let quantity = line
                .quantity
                .ok_or(tr!("quantité illisible en tête du tiers"))?;
            let rates = (!self.rates.per_euro.is_empty()).then_some(&self.rates);
            let price = quotes::price_in_account_currency(
                q.price,
                &q.currency,
                doc.current_account(),
                rates,
            )?;
            Ok((
                price,
                Amount::from_cents((price * quantity * 100.0).round() as i64),
            ))
        }))
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let running = self.poll();
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.75, 860.0, 1300.0);
        let mut apply = false;
        crate::dialogs::modal(ctx, "quotes").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::TREND_UP, pal.accent, tr!("Cotations des titres"));
            ui_kit::subtitle(
                ui,
                tr!(
                    "Compte « {} » : la valeur de chaque ligne « Titre » devient le cours du jour multiplié par la quantité.",
                    doc.current_account().name
                ),
            );
            if self.lines.is_empty() {
                empty_state(ui, &pal);
            } else {
                self.table(ui, doc, width, &pal);
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr!(
                    "{}  Cours de Yahoo Finance (service public non officiel, souvent différé), à titre indicatif.",
                    icon::INFO
                ))
                .size(12.5)
                .color(pal.muted),
            );
            if let Some(message) = &self.message {
                ui.label(RichText::new(format!("{}  {message}", icon::CHECK_CIRCLE)).color(pal.positive));
            }
            crate::dialogs::footer(ui, |ui| {
                let changes = self.changes(doc);
                if self.downloaded {
                    let label = tr!("{}  Appliquer ({})", icon::CHECK, changes.len());
                    if crate::dialogs::primary(ui, &label, !changes.is_empty()).clicked() {
                        apply = true;
                    }
                } else if let Some((done, total)) = running {
                    ui.add(egui::Spinner::new());
                    ui.label(RichText::new(tr!("Téléchargement {}/{}…", done, total)).color(pal.muted));
                } else {
                    let label = tr!("{}  Télécharger les cours", icon::DOWNLOAD_SIMPLE);
                    if crate::dialogs::primary(ui, &label, !self.lines.is_empty()).clicked() {
                        self.start(ctx, doc);
                    }
                }
                if crate::dialogs::button(ui, tr!("Fermer")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            });
        });
        if apply {
            let changes = self.changes(doc);
            let n = doc.set_amounts(&changes, tr_mark!("mise à jour des cotations"));
            outcome = Outcome::Done(match n {
                0 => tr!("Aucune valeur modifiée").into(),
                n => trn!(
                    n,
                    "{} ligne « Titre » mise à jour",
                    "{} lignes « Titre » mises à jour"
                ),
            });
        }
        outcome
    }

    fn changes(&self, doc: &OpenDoc) -> Vec<(usize, Amount)> {
        self.lines
            .iter()
            .filter_map(|l| match self.new_value(l, doc) {
                Some(Ok((_, amount))) => Some((l.line, amount)),
                _ => None,
            })
            .collect()
    }

    fn table(&self, ui: &mut egui::Ui, doc: &OpenDoc, width: f32, pal: &Palette) {
        const ROW: f32 = 34.0;
        let account = doc.current_account();
        let currency = account.currency.clone();
        let inner = width - 16.0;
        let cols = [
            (inner * 0.26, egui::Align::Min, trc!("cotation", "Titre")),
            (inner * 0.15, egui::Align::Min, tr!("Code")),
            (inner * 0.09, egui::Align::Max, tr!("Quantité")),
            (inner * 0.13, egui::Align::Max, tr!("Valeur actuelle")),
            (inner * 0.11, egui::Align::Max, tr!("Cours")),
            (inner * 0.13, egui::Align::Max, tr!("Nouvelle valeur")),
            (inner * 0.13, egui::Align::Max, trc!("cotation", "Écart")),
        ];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (w, align, title) in cols {
                ui_kit::cell(
                    ui,
                    w,
                    align,
                    RichText::new(title)
                        .family(crate::theme::semibold())
                        .color(pal.fg_dark),
                );
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("quotes_rows")
            .max_height(ui_kit::dialog_height(ui.ctx(), 0.5))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (k, line) in self.lines.iter().enumerate() {
                    let op = &account.operations[line.line];
                    let (rect, _) = ui.allocate_exact_size(vec2(inner, ROW), egui::Sense::hover());
                    if k % 2 == 1 {
                        ui.painter()
                            .rect_filled(rect, 6.0, pal.stripe.gamma_multiply(0.7));
                    }
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    let ui = &mut child;
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui_kit::cell(
                        ui,
                        cols[0].0,
                        egui::Align::Min,
                        RichText::new(&op.payee).family(crate::theme::medium()),
                    );
                    let name = match self.quotes.get(&line.identifier) {
                        Some(Ok(q)) if !q.symbol.is_empty() && q.symbol != line.identifier => {
                            format!("{} · {}", line.identifier, q.symbol)
                        }
                        _ => line.identifier.clone(),
                    };
                    ui_kit::cell(
                        ui,
                        cols[1].0,
                        egui::Align::Min,
                        RichText::new(name).color(pal.fg_dark),
                    );
                    let quantity = line.quantity.map_or("?".to_string(), |q| {
                        format!("{q}").replace('.', &i18n::decimal_separator().to_string())
                    });
                    ui_kit::cell(
                        ui,
                        cols[2].0,
                        egui::Align::Max,
                        RichText::new(quantity).color(pal.fg_dark),
                    );
                    ui_kit::cell(
                        ui,
                        cols[3].0,
                        egui::Align::Max,
                        RichText::new(money(op.amount, &currency)),
                    );
                    match self.new_value(line, doc) {
                        None => {
                            let waiting = if self.progress.is_some() { "…" } else { "" };
                            ui_kit::cell(
                                ui,
                                cols[4].0,
                                egui::Align::Max,
                                RichText::new(waiting).color(pal.muted),
                            );
                        }
                        Some(Ok((price, amount))) => {
                            let price = format!("{price:.2}")
                                .replace('.', &i18n::decimal_separator().to_string());
                            ui_kit::cell(
                                ui,
                                cols[4].0,
                                egui::Align::Max,
                                RichText::new(price).color(pal.fg_dark),
                            );
                            ui_kit::cell(
                                ui,
                                cols[5].0,
                                egui::Align::Max,
                                RichText::new(money(amount, &currency))
                                    .family(crate::theme::semibold()),
                            );
                            let diff = Amount(amount.0 - op.amount.0);
                            let color = if diff.0 < 0 {
                                pal.negative
                            } else {
                                pal.positive
                            };
                            let sign = if diff.0 > 0 { "+" } else { "" };
                            ui_kit::cell(
                                ui,
                                cols[6].0,
                                egui::Align::Max,
                                RichText::new(format!("{sign}{}", money(diff, &currency)))
                                    .color(color),
                            );
                        }
                        Some(Err(e)) => {
                            let w = cols[4].0 + cols[5].0 + cols[6].0;
                            ui_kit::cell(
                                ui,
                                w,
                                egui::Align::Max,
                                RichText::new(format!("{}  {e}", icon::WARNING))
                                    .color(pal.negative),
                            );
                        }
                    }
                }
            });
    }
}

fn empty_state(ui: &mut egui::Ui, pal: &Palette) {
    ui_kit::card(ui, ui.available_width(), |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(tr!("Aucune ligne « Titre » avec un code dans ce compte.")).family(crate::theme::semibold()));
            ui.add_space(4.0);
            ui.label(
                RichText::new(tr!(
                    "Pour suivre un titre : mode de paiement « Titre », code ISIN (FR0000133308) ou symbole (AAPL) en début de détail, et quantité en tête du tiers (« 10 Orange »)."
                ))
                .color(pal.fg_dark),
            );
        });
    });
}

/// Cours d'un titre : un ISIN est d'abord converti en symbole.
fn fetch_quote(identifier: &str) -> Result<Quote, String> {
    let get = |url: String| -> Result<String, String> {
        ureq::get(&url)
            .header("User-Agent", "Mozilla/5.0 BankPerfect")
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())
    };
    let symbol = if quotes::looks_like_isin(identifier) {
        let json = get(format!("{}{}", quotes::SEARCH_URL, identifier))?;
        quotes::parse_search(&json).ok_or(tr!("code ISIN introuvable"))?
    } else {
        identifier.to_string()
    };
    let json = get(format!(
        "{}{}?range=1d&interval=1d",
        quotes::CHART_URL,
        symbol
    ))
    .map_err(|e| {
        if e.contains("404") {
            tr!("symbole {} inconnu", symbol)
        } else {
            e
        }
    })?;
    quotes::parse_chart(&json).ok_or_else(|| tr!("aucun cours pour {}", symbol))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Téléchargement réel (réseau) : `cargo test -- --ignored quote`.
    #[test]
    #[ignore]
    fn downloads_a_quote_from_an_isin() {
        let q = fetch_quote("FR0000133308").expect("cours d'Orange");
        assert_eq!(q.currency, "EUR");
        assert!(q.price > 1.0 && q.price < 100.0);
        let q = fetch_quote("AAPL").expect("cours d'Apple");
        assert_eq!(q.currency, "USD");
    }
}
