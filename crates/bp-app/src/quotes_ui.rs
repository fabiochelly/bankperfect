//! Portefeuille de titres du compte courant : positions, cours du jour,
//! plus-values, ventes, dividendes et graphiques. Les calculs sont dans
//! [`bp_core::portfolio`] et [`bp_core::quotes`].

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use bp_core::currencies::Rates;
use bp_core::i18n;
use bp_core::model::{Amount, Date, Tack};
use bp_core::portfolio::{self, Movement, MovementKind, Position, Sale};
use bp_core::quotes::{self, HistoryRange, PriceHistory, Quote, StockLine};
use bp_core::reports::{ChartData, ChartPoint, ChartSeries, ChartUnit};
use bp_core::{tr, tr_mark, trc, trn};
use egui::{Color32, Key, RichText, vec2};
use egui_phosphor::regular as icon;

use crate::datepicker::{self, Placement};
use crate::dialogs::{self, Choice};
use crate::document::OpenDoc;
use crate::format::money;
use crate::settings::shared;
use crate::theme::{Palette, palette};
use crate::tools::Outcome;
use crate::ui_kit;

/// Cours datés par code, dans la devise du compte.
type DatedPrices = HashMap<String, Vec<(Date, f64)>>;

/// Avancement du téléchargement des cours, partagé avec le fil de travail.
#[derive(Default)]
struct Progress {
    total: usize,
    done: usize,
    quotes: HashMap<String, Result<Quote, String>>,
    rates: Option<Rates>,
    finished: bool,
}

/// Téléchargement des historiques, partagé avec le fil de travail.
#[derive(Default)]
struct HistoryJob {
    histories: HashMap<String, Result<PriceHistory, String>>,
    rates: Option<Rates>,
    finished: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Positions,
    Evolution,
    Allocation,
    Movements,
}

/// Saisie affichée sous les positions.
#[derive(Clone, PartialEq, Eq)]
enum Panel {
    Buy,
    Sell(String),
    Dividend(String),
}

/// Action demandée sur une ligne du tableau.
enum RowAction {
    Buy(String),
    Sell(String),
    Dividend(String),
    Chart(String),
    Delete(String),
}

/// Champs de saisie, conservés d'une image à l'autre.
#[derive(Default)]
struct Entry {
    code: String,
    name: String,
    quantity: String,
    price: String,
    fees: String,
    date: String,
    pay: bool,
    error: Option<String>,
}

impl Entry {
    fn fresh() -> Self {
        Self {
            date: Date::today().to_string(),
            ..Self::default()
        }
    }

    fn date(&self) -> Result<Date, String> {
        Date::parse_fr(&self.date, Date::today()).ok_or(tr!("Date illisible.").to_string())
    }
}

pub struct QuotesForm {
    lines: Vec<StockLine>,
    movements: Vec<Movement>,
    progress: Option<Arc<Mutex<Progress>>>,
    quotes: HashMap<String, Result<Quote, String>>,
    rates: Rates,
    message: Option<String>,
    view: View,
    panel: Panel,
    entry: Entry,
    pending_delete: Option<String>,
    range: HistoryRange,
    /// Titre tracé dans « Évolution » (`None` : tout le portefeuille).
    chart_code: Option<String>,
    histories: HashMap<String, Result<PriceHistory, String>>,
    history_job: Option<Arc<Mutex<HistoryJob>>>,
}

fn to_amount(value: f64) -> Amount {
    Amount::from_cents((value * 100.0).round() as i64)
}

fn signed_money(value: f64, currency: &str) -> String {
    let sign = if value > 0.0 { "+" } else { "" };
    format!("{sign}{}", money(to_amount(value), currency))
}

fn percent(value: f64) -> String {
    format!("{value:+.2} %").replace('.', &i18n::decimal_separator().to_string())
}

fn quantity_text(quantity: f64) -> String {
    let rounded = (quantity * 1e6).round() / 1e6;
    format!("{rounded}").replace('.', &i18n::decimal_separator().to_string())
}

fn gain_color(value: f64, pal: &Palette) -> Color32 {
    if value < 0.0 {
        pal.negative
    } else if value > 0.0 {
        pal.positive
    } else {
        pal.fg_dark
    }
}

impl QuotesForm {
    pub fn new(doc: &OpenDoc) -> Self {
        Self {
            lines: quotes::stock_lines(doc.current_account()),
            movements: portfolio::movements(&doc.current_account().operations),
            progress: None,
            quotes: HashMap::new(),
            rates: crate::currency_ui::saved_rates(),
            message: None,
            view: View::Positions,
            panel: Panel::Buy,
            entry: Entry::fresh(),
            pending_delete: None,
            range: HistoryRange::OneYear,
            chart_code: None,
            histories: HashMap::new(),
            history_job: None,
        }
    }

    /// Relit le compte après une modification.
    fn reload(&mut self, doc: &OpenDoc) {
        self.lines = quotes::stock_lines(doc.current_account());
        self.movements = portfolio::movements(&doc.current_account().operations);
        let held: HashSet<&String> = self.lines.iter().map(|l| &l.identifier).collect();
        if self.chart_code.as_ref().is_some_and(|c| !held.contains(c)) {
            self.chart_code = None;
        }
    }

    fn codes(&self) -> Vec<String> {
        let mut codes: Vec<String> = self.lines.iter().map(|l| l.identifier.clone()).collect();
        codes.sort();
        codes.dedup();
        codes
    }

    fn start(&mut self, ctx: &egui::Context, doc: &OpenDoc) {
        let identifiers = self.codes();
        let progress = Arc::new(Mutex::new(Progress {
            total: identifiers.len(),
            ..Progress::default()
        }));
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
            return None;
        }
        Some((p.done, p.total))
    }

    /// Lance le téléchargement des historiques manquants.
    fn fetch_histories(&mut self, ctx: &egui::Context, doc: &OpenDoc) {
        if self.history_job.is_some() {
            return;
        }
        let missing: Vec<String> = self
            .codes()
            .into_iter()
            .filter(|code| !self.histories.contains_key(code))
            .collect();
        if missing.is_empty() {
            return;
        }
        let job = Arc::new(Mutex::new(HistoryJob::default()));
        let shared = job.clone();
        let ctx = ctx.clone();
        let range = self.range;
        let account_currency = quotes::iso_currency(&doc.current_account().currency);
        let secondary = quotes::iso_currency(&doc.current_account().secondary_currency);
        let have_rates = !self.rates.per_euro.is_empty();
        std::thread::spawn(move || {
            let mut need_rates = false;
            for code in missing {
                let result = fetch_history(&code, range);
                if let Ok(h) = &result
                    && h.currency != account_currency
                    && h.currency != secondary
                {
                    need_rates = true;
                }
                if let Ok(mut j) = shared.lock() {
                    j.histories.insert(code, result);
                }
                ctx.request_repaint();
            }
            let rates = (need_rates && !have_rates)
                .then(crate::currency_ui::fetch_ecb)
                .and_then(Result::ok);
            if let Ok(mut j) = shared.lock() {
                j.rates = rates;
                j.finished = true;
            }
            ctx.request_repaint();
        });
        self.history_job = Some(job);
    }

    fn poll_histories(&mut self) {
        let Some(job) = &self.history_job else { return };
        let Ok(mut j) = job.lock() else { return };
        for (code, history) in j.histories.drain() {
            self.histories.insert(code, history);
        }
        if j.finished {
            if let Some(rates) = j.rates.take() {
                self.rates = rates;
            }
            drop(j);
            self.history_job = None;
        }
    }

    fn known_rates(&self) -> Option<&Rates> {
        (!self.rates.per_euro.is_empty()).then_some(&self.rates)
    }

    /// Cours d'un code dans la devise du compte.
    fn price_of(&self, code: &str, doc: &OpenDoc) -> Option<Result<f64, String>> {
        let quote = self.quotes.get(code)?;
        Some(quote.clone().and_then(|q| {
            quotes::price_in_account_currency(
                q.price,
                &q.currency,
                doc.current_account(),
                self.known_rates(),
            )
        }))
    }

    /// Variation du jour d'un code, en pourcentage.
    fn day_change(&self, code: &str) -> Option<f64> {
        let quote = self.quotes.get(code)?.as_ref().ok()?;
        let previous = quote.previous_close?;
        Some((quote.price / previous - 1.0) * 100.0)
    }

    /// Montants à écrire sur les lignes dont la valeur a changé (les lignes
    /// rapprochées verrouillées restent telles quelles).
    fn changes(&self, doc: &OpenDoc) -> Vec<(usize, Amount)> {
        let operations = &doc.current_account().operations;
        self.lines
            .iter()
            .filter_map(|l| {
                let price = self.price_of(&l.identifier, doc)?.ok()?;
                let amount = to_amount(price * l.quantity?);
                let op = &operations[l.line];
                (op.amount != amount && !(doc.locked && op.tack == Tack::Reconciled))
                    .then_some((l.line, amount))
            })
            .collect()
    }

    fn open_panel(&mut self, panel: Panel) {
        self.entry = Entry::fresh();
        self.panel = panel;
    }

    fn buy(&mut self, doc: &mut OpenDoc) -> bool {
        let entry = &self.entry;
        let result = entry.date().and_then(|date| {
            quotes::new_stock_operation(
                &entry.code,
                &entry.quantity,
                &entry.name,
                &entry.price,
                date,
            )
        });
        let lot = match result {
            Ok(lot) => lot,
            Err(error) => {
                self.entry.error = Some(error);
                return false;
            }
        };
        self.message = Some(tr!("Titre {} ajouté au compte.", lot.details));
        let (account, pay) = (doc.account, entry.pay);
        let _ = doc.edit_accounts(tr_mark!("achat de titre"), &[account], |document| {
            portfolio::add_purchase(document, account, lot, pay);
            Ok::<(), String>(())
        });
        self.reload(doc);
        self.entry = Entry::fresh();
        true
    }

    fn sell(&mut self, code: &str, doc: &mut OpenDoc) {
        let entry = &self.entry;
        let fees = match entry.fees.trim() {
            "" | "0" => Some(0.0),
            text => quotes::positive_number(text),
        };
        let sale = entry.date().and_then(|date| {
            Ok(Sale {
                code: code.to_string(),
                quantity: quotes::positive_number(&entry.quantity)
                    .ok_or(tr!("Saisissez le nombre de titres (ex. : 10 ou 2,5)."))?,
                price: quotes::positive_number(&entry.price)
                    .ok_or(tr!("Prix de vente illisible."))?,
                fees: fees.ok_or(tr!("Frais illisibles."))?,
                date,
            })
        });
        let (account, locked) = (doc.account, doc.locked);
        let result = sale.and_then(|sale| {
            doc.edit_accounts(tr_mark!("vente de titres"), &[account], |document| {
                portfolio::sell(document, account, &sale, locked)
            })
        });
        match result {
            Ok(sold) => {
                let currency = doc.current_account().currency.clone();
                self.message = Some(match sold.gain {
                    Some(gain) => tr!(
                        "Vente enregistrée : {} au compte, plus-value {}.",
                        money(sold.proceeds, &currency),
                        signed_money(gain, &currency)
                    ),
                    None => tr!(
                        "Vente enregistrée : {} au compte.",
                        money(sold.proceeds, &currency)
                    ),
                });
                self.reload(doc);
                self.open_panel(Panel::Buy);
            }
            Err(error) => self.entry.error = Some(error),
        }
    }

    fn dividend(&mut self, code: &str, name: &str, doc: &mut OpenDoc) {
        let entry = &self.entry;
        let operation = entry
            .date()
            .and_then(|date| portfolio::dividend_operation(code, name, &entry.price, date));
        match operation {
            Ok(operation) => {
                doc.upsert_operation(None, operation);
                self.message = Some(tr!("Dividende de {} enregistré.", name));
                self.reload(doc);
                self.open_panel(Panel::Buy);
            }
            Err(error) => self.entry.error = Some(error),
        }
    }

    fn delete(&mut self, code: &str, doc: &mut OpenDoc) {
        let (account, locked) = (doc.account, doc.locked);
        let result =
            doc.edit_accounts(tr_mark!("suppression d'un titre"), &[account], |document| {
                portfolio::remove_position(document, account, code, locked)
            });
        match result {
            Ok(_) => {
                self.message = Some(tr!("Titre {} retiré du compte.", code));
                self.reload(doc);
                if matches!(&self.panel, Panel::Sell(c) | Panel::Dividend(c) if c == code) {
                    self.open_panel(Panel::Buy);
                }
            }
            Err(error) => self.entry.error = Some(error),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let running = self.poll();
        self.poll_histories();
        if self.view == View::Evolution {
            self.fetch_histories(ctx, doc);
        }
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.78, 900.0, 1320.0);
        let mut apply = false;
        let currency = doc.current_account().currency.clone();
        let positions = {
            let this = &*self;
            let doc = &*doc;
            portfolio::positions(&self.lines, &|code| {
                this.price_of(code, doc).and_then(Result::ok)
            })
        };
        dialogs::modal(ctx, "quotes").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            dialogs::header(ui, icon::TREND_UP, pal.accent, tr!("Portefeuille de titres"));
            ui_kit::subtitle(
                ui,
                tr!(
                    "Compte « {} » : vos titres, leur valeur au cours du jour et vos plus-values.",
                    doc.current_account().name
                ),
            );
            self.tiles(ui, &positions, &currency, width, &pal);
            ui.add_space(14.0);
            let tabs = [
                (View::Positions, icon::LIST_BULLETS, tr!("Positions"), pal.accent),
                (View::Evolution, icon::CHART_LINE, tr!("Évolution"), pal.accent),
                (View::Allocation, icon::CHART_DONUT, tr!("Répartition"), pal.accent),
                (
                    View::Movements,
                    icon::RECEIPT,
                    tr!("Ventes et dividendes"),
                    pal.accent,
                ),
            ];
            ui_kit::segmented(ui, &mut self.view, &tabs, 38.0, Some(width));
            ui.add_space(14.0);
            match self.view {
                View::Positions => {
                    if positions.is_empty() {
                        empty_state(ui, &pal);
                    } else if let Some(action) =
                        self.table(ui, &positions, doc, &currency, width, &pal)
                    {
                        match action {
                            RowAction::Buy(code) => {
                                self.open_panel(Panel::Buy);
                                self.entry.code = code.clone();
                                self.entry.name = positions
                                    .iter()
                                    .find(|p| p.code == code)
                                    .map(|p| p.name.clone())
                                    .unwrap_or_default();
                            }
                            RowAction::Sell(code) => {
                                self.open_panel(Panel::Sell(code.clone()));
                                if let Some(Ok(price)) = self.price_of(&code, doc) {
                                    self.entry.price = format!("{price:.2}")
                                        .replace('.', &i18n::decimal_separator().to_string());
                                }
                            }
                            RowAction::Dividend(code) => self.open_panel(Panel::Dividend(code)),
                            RowAction::Chart(code) => {
                                self.chart_code = Some(code);
                                self.view = View::Evolution;
                            }
                            RowAction::Delete(code) => self.pending_delete = Some(code),
                        }
                    }
                    ui.add_space(14.0);
                    self.panel_form(ui, &positions, doc, width, running.is_none(), &pal);
                }
                View::Evolution => self.evolution(ui, &positions, doc, &currency, width, &pal),
                View::Allocation => self.allocation(ui, &positions, &currency, &pal),
                View::Movements => self.journal(ui, &currency, width, &pal),
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
                ui.label(
                    RichText::new(format!("{}  {message}", icon::CHECK_CIRCLE)).color(pal.positive),
                );
            }
            dialogs::footer(ui, |ui| {
                let changes = self.changes(doc);
                if !changes.is_empty() {
                    let label = tr!("{}  Appliquer ({})", icon::CHECK, changes.len());
                    if dialogs::primary(ui, &label, true).clicked() {
                        apply = true;
                    }
                }
                if let Some((done, total)) = running {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        RichText::new(tr!("Téléchargement {}/{}…", done, total)).color(pal.muted),
                    );
                } else {
                    let label = tr!("{}  Actualiser les cours", icon::DOWNLOAD_SIMPLE);
                    let enabled = !self.lines.is_empty();
                    let clicked = if changes.is_empty() {
                        dialogs::primary(ui, &label, enabled).clicked()
                    } else {
                        dialogs::button(ui, label).clicked()
                    };
                    if clicked {
                        self.start(ctx, doc);
                    }
                }
                let escape = self.pending_delete.is_none()
                    && ui.input(|i| i.key_pressed(Key::Escape));
                if dialogs::button(ui, tr!("Fermer")).clicked() {
                    outcome = Outcome::Close;
                } else if escape {
                    if self.panel == Panel::Buy {
                        outcome = Outcome::Close;
                    } else {
                        self.open_panel(Panel::Buy);
                    }
                }
            });
        });
        if apply {
            let changes = self.changes(doc);
            let n = doc.set_amounts(&changes, tr_mark!("mise à jour des cotations"));
            doc.refresh();
            self.reload(doc);
            self.message = Some(match n {
                0 => tr!("Aucune valeur modifiée").into(),
                n => trn!(
                    n,
                    "{} ligne « Titre » mise à jour",
                    "{} lignes « Titre » mises à jour"
                ),
            });
        }
        if let Some(code) = self.pending_delete.clone() {
            match dialogs::confirm(
                ctx,
                "quotes_delete",
                icon::TRASH,
                tr!("Retirer ce titre ?"),
                &tr!(
                    "Toutes les lignes de {} seront retirées du compte, sans enregistrer de vente. Pour une cession réelle, utilisez « Vendre ».",
                    code
                ),
                tr!("Retirer"),
                Some(tr!("Annuler")),
                true,
            ) {
                Choice::Yes => {
                    self.delete(&code, doc);
                    self.pending_delete = None;
                }
                Choice::Pending => {}
                _ => self.pending_delete = None,
            }
        }
        outcome
    }

    /// Bandeau des totaux.
    fn tiles(
        &self,
        ui: &mut egui::Ui,
        positions: &[Position],
        currency: &str,
        width: f32,
        pal: &Palette,
    ) {
        const GAP: f32 = 10.0;
        let total = portfolio::summary(positions);
        let realized: f64 = self.movements.iter().filter_map(|m| m.gain).sum();
        let dividends: f64 = self
            .movements
            .iter()
            .filter(|m| m.kind == MovementKind::Dividend)
            .map(|m| m.amount.as_f64())
            .sum();
        let day: f64 = positions
            .iter()
            .filter_map(|p| {
                let change = self.day_change(&p.code)?;
                Some(p.value - p.value / (1.0 + change / 100.0))
            })
            .sum();
        let has_day = positions.iter().any(|p| self.day_change(&p.code).is_some());
        let uncosted = if total.uncosted > 0 {
            trn!(
                total.uncosted,
                "{} titre sans prix d'achat exclu",
                "{} titres sans prix d'achat exclus"
            )
        } else {
            total.gain_percent().map(percent).unwrap_or_default()
        };
        let tiles: [(&str, String, String, Color32); 5] = [
            (
                tr!("Valeur"),
                money(to_amount(total.value), currency),
                trn!(positions.len(), "{} titre", "{} titres"),
                pal.fg,
            ),
            (
                tr!("Investi"),
                money(to_amount(total.cost), currency),
                String::new(),
                pal.fg,
            ),
            (
                tr!("Plus-value latente"),
                signed_money(total.gain, currency),
                uncosted,
                gain_color(total.gain, pal),
            ),
            (
                tr!("Réalisé"),
                signed_money(realized + dividends, currency),
                tr!("{} de dividendes", money(to_amount(dividends), currency)),
                gain_color(realized + dividends, pal),
            ),
            (
                tr!("Aujourd'hui"),
                if has_day {
                    signed_money(day, currency)
                } else {
                    "—".to_string()
                },
                if has_day && total.value > day {
                    percent(day / (total.value - day) * 100.0)
                } else {
                    tr!("cours à actualiser").to_string()
                },
                gain_color(day, pal),
            ),
        ];
        let tile_width = (width - GAP * (tiles.len() - 1) as f32) / tiles.len() as f32;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for (label, value, detail, color) in tiles {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(tile_width, 78.0), egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect(
                    rect,
                    12.0,
                    pal.bg,
                    egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)),
                    egui::StrokeKind::Inside,
                );
                let left = rect.left() + 14.0;
                painter.text(
                    egui::pos2(left, rect.top() + 12.0),
                    egui::Align2::LEFT_TOP,
                    label.to_uppercase(),
                    egui::FontId::new(11.0, crate::theme::semibold()),
                    pal.muted,
                );
                painter.text(
                    egui::pos2(left, rect.top() + 29.0),
                    egui::Align2::LEFT_TOP,
                    value,
                    egui::FontId::new(18.0, crate::theme::semibold()),
                    color,
                );
                painter.text(
                    egui::pos2(left, rect.top() + 55.0),
                    egui::Align2::LEFT_TOP,
                    detail,
                    egui::FontId::proportional(12.0),
                    pal.muted,
                );
            }
        });
    }

    /// Tableau des positions. Renvoie l'action demandée sur une ligne.
    fn table(
        &self,
        ui: &mut egui::Ui,
        positions: &[Position],
        doc: &OpenDoc,
        currency: &str,
        width: f32,
        pal: &Palette,
    ) -> Option<RowAction> {
        const ROW: f32 = 36.0;
        const ACTION: f32 = 28.0;
        let inner = width - 16.0;
        let actions = 5.0 * ACTION;
        let flexible = inner - actions - 8.0;
        let cols = [
            (flexible * 0.20, egui::Align::Min, trc!("cotation", "Titre")),
            (flexible * 0.13, egui::Align::Min, tr!("Code")),
            (flexible * 0.10, egui::Align::Max, tr!("Quantité")),
            (flexible * 0.10, egui::Align::Max, tr!("Revient")),
            (flexible * 0.10, egui::Align::Max, tr!("Cours")),
            (flexible * 0.08, egui::Align::Max, tr!("Jour")),
            (flexible * 0.12, egui::Align::Max, tr!("Valeur")),
            (flexible * 0.11, egui::Align::Max, tr!("Plus-value")),
            (flexible * 0.06, egui::Align::Max, tr!("Poids")),
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
        let total: f64 = positions.iter().map(|p| p.value).sum();
        let mut action = None;
        egui::ScrollArea::vertical()
            .id_salt("quotes_rows")
            .max_height(ui_kit::dialog_height(ui.ctx(), 0.3))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (k, position) in positions.iter().enumerate() {
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
                    let code = &position.code;
                    ui_kit::cell(
                        ui,
                        cols[0].0,
                        egui::Align::Min,
                        RichText::new(&position.name).family(crate::theme::medium()),
                    );
                    let symbol = match self.quotes.get(code) {
                        Some(Ok(q)) if !q.symbol.is_empty() && q.symbol != *code => {
                            format!("{code} · {}", q.symbol)
                        }
                        _ => code.clone(),
                    };
                    ui_kit::cell(
                        ui,
                        cols[1].0,
                        egui::Align::Min,
                        RichText::new(symbol).color(pal.fg_dark),
                    );
                    let quantity = if position.incomplete {
                        "?".to_string()
                    } else {
                        quantity_text(position.quantity)
                    };
                    let response = ui_kit::cell_response(
                        ui,
                        cols[2].0,
                        egui::Align::Max,
                        RichText::new(quantity).color(pal.fg_dark),
                    );
                    if position.incomplete {
                        ui_kit::hover(response, tr!("quantité illisible en tête du tiers"));
                    }
                    let unit = position
                        .unit_cost()
                        .map_or("—".to_string(), |u| money(to_amount(u), currency));
                    ui_kit::cell(
                        ui,
                        cols[3].0,
                        egui::Align::Max,
                        RichText::new(unit).color(pal.fg_dark),
                    );
                    match self.price_of(code, doc) {
                        Some(Ok(price)) => ui_kit::cell(
                            ui,
                            cols[4].0,
                            egui::Align::Max,
                            RichText::new(money(to_amount(price), currency)).color(pal.fg_dark),
                        ),
                        Some(Err(error)) => {
                            let response = ui_kit::cell_response(
                                ui,
                                cols[4].0,
                                egui::Align::Max,
                                RichText::new(icon::WARNING).color(pal.negative),
                            );
                            ui_kit::hover(response, &error);
                        }
                        None => {
                            let text = if self.progress.is_some() {
                                "…"
                            } else {
                                "—"
                            };
                            ui_kit::cell(
                                ui,
                                cols[4].0,
                                egui::Align::Max,
                                RichText::new(text).color(pal.muted),
                            );
                        }
                    }
                    let (day, day_color) = self
                        .day_change(code)
                        .map_or(("—".to_string(), pal.muted), |d| {
                            (percent(d), gain_color(d, pal))
                        });
                    ui_kit::cell(
                        ui,
                        cols[5].0,
                        egui::Align::Max,
                        RichText::new(day).color(day_color),
                    );
                    ui_kit::cell(
                        ui,
                        cols[6].0,
                        egui::Align::Max,
                        RichText::new(money(to_amount(position.value), currency))
                            .family(crate::theme::semibold()),
                    );
                    let response = match position.gain() {
                        Some(gain) => ui_kit::cell_response(
                            ui,
                            cols[7].0,
                            egui::Align::Max,
                            RichText::new(signed_money(gain, currency))
                                .color(gain_color(gain, pal)),
                        ),
                        None => ui_kit::cell_response(
                            ui,
                            cols[7].0,
                            egui::Align::Max,
                            RichText::new("—").color(pal.muted),
                        ),
                    };
                    if let Some(rate) = position.gain_percent() {
                        ui_kit::hover(response, &percent(rate));
                    }
                    let weight = if total > 0.0 {
                        format!("{:.0} %", position.value / total * 100.0)
                    } else {
                        "—".to_string()
                    };
                    ui_kit::cell(
                        ui,
                        cols[8].0,
                        egui::Align::Max,
                        RichText::new(weight).color(pal.fg_dark),
                    );
                    ui.add_space(8.0);
                    let button = |ui: &mut egui::Ui, glyph: &str, tip: &str, color: Color32| {
                        let response = ui.add_sized(
                            vec2(ACTION, ACTION),
                            egui::Button::new(RichText::new(glyph).size(16.0).color(color))
                                .frame(false),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip)
                        });
                        ui_kit::hover(response, tip).clicked()
                    };
                    if button(ui, icon::PLUS_CIRCLE, tr!("Acheter encore"), pal.fg_dark) {
                        action = Some(RowAction::Buy(code.clone()));
                    }
                    if button(ui, icon::HAND_COINS, tr!("Vendre"), pal.fg_dark) {
                        action = Some(RowAction::Sell(code.clone()));
                    }
                    if button(ui, icon::COINS, tr!("Dividende"), pal.fg_dark) {
                        action = Some(RowAction::Dividend(code.clone()));
                    }
                    if button(ui, icon::CHART_LINE, tr!("Voir le cours"), pal.fg_dark) {
                        action = Some(RowAction::Chart(code.clone()));
                    }
                    if button(ui, icon::TRASH, tr!("Retirer ce titre"), pal.muted) {
                        action = Some(RowAction::Delete(code.clone()));
                    }
                }
            });
        action
    }

    /// Saisie sous le tableau : achat, vente ou dividende.
    fn panel_form(
        &mut self,
        ui: &mut egui::Ui,
        positions: &[Position],
        doc: &mut OpenDoc,
        width: f32,
        enabled: bool,
        pal: &Palette,
    ) {
        let panel = self.panel.clone();
        let target = match &panel {
            Panel::Buy => None,
            Panel::Sell(code) | Panel::Dividend(code) => positions.iter().find(|p| p.code == *code),
        };
        let (glyph, title) = match (&panel, target) {
            (Panel::Sell(_), Some(p)) => (icon::HAND_COINS, tr!("Vendre {}", p.name)),
            (Panel::Dividend(_), Some(p)) => (icon::COINS, tr!("Dividende de {}", p.name)),
            _ => (icon::PLUS, tr!("Ajouter un titre").to_string()),
        };
        if target.is_none() && panel != Panel::Buy {
            self.open_panel(Panel::Buy);
            return;
        }
        ui_kit::section(ui, width, glyph, &title, |ui| {
            ui.add_enabled_ui(enabled, |ui| match (&panel, target) {
                (Panel::Sell(code), Some(p)) => self.sell_fields(ui, code, p, doc),
                (Panel::Dividend(code), Some(p)) => self.dividend_fields(ui, code, p, doc),
                _ => self.buy_fields(ui, doc),
            });
            if let Some(error) = &self.entry.error {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
        });
    }

    fn buy_fields(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc) {
        const GAP: f32 = 10.0;
        const BUTTON: f32 = 110.0;
        const DATE: f32 = 130.0;
        let mut submit = false;
        let code = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                let inner = ui.available_width() - BUTTON - DATE - 5.0 * GAP;
                let mut field = |ui: &mut egui::Ui, text: &mut String, hint: &str, share: f32| {
                    let response = ui.add(
                        crate::theme::field(text)
                            .hint_text(hint)
                            .desired_width(inner * share),
                    );
                    submit |= response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    response
                };
                let entry = &mut self.entry;
                let code = field(ui, &mut entry.code, tr!("ISIN ou symbole"), 0.27);
                field(ui, &mut entry.quantity, tr!("Quantité"), 0.15);
                field(ui, &mut entry.name, tr!("Nom (facultatif)"), 0.26);
                field(ui, &mut entry.price, tr!("Prix d'achat (facultatif)"), 0.32);
                datepicker::date_field(
                    ui,
                    "quotes_buy_date",
                    &mut entry.date,
                    DATE,
                    Placement::Popup,
                );
                let label = tr!("{}  Ajouter", icon::PLUS);
                submit |= ui
                    .add(egui::Button::new(label).min_size(vec2(BUTTON, 36.0)))
                    .clicked();
                code
            })
            .inner;
        ui.add_space(8.0);
        crate::theme::checkbox(
            ui,
            &mut self.entry.pay,
            tr!("Débiter ce compte du prix d'achat (compte-titres avec liquidités)"),
        );
        if submit && self.buy(doc) {
            code.request_focus();
        }
    }

    fn sell_fields(
        &mut self,
        ui: &mut egui::Ui,
        code: &str,
        position: &Position,
        doc: &mut OpenDoc,
    ) {
        const GAP: f32 = 10.0;
        const BUTTON: f32 = 110.0;
        const DATE: f32 = 130.0;
        let mut submit = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let inner = ui.available_width() - 2.0 * BUTTON - DATE - 5.0 * GAP;
            let held = tr!("Quantité (max. {})", quantity_text(position.quantity));
            let mut field = |ui: &mut egui::Ui, text: &mut String, hint: &str, share: f32| {
                let response = ui.add(
                    crate::theme::field(text)
                        .hint_text(hint)
                        .desired_width(inner * share),
                );
                submit |= response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            };
            let entry = &mut self.entry;
            field(ui, &mut entry.quantity, &held, 0.36);
            field(ui, &mut entry.price, tr!("Prix de vente unitaire"), 0.36);
            field(ui, &mut entry.fees, tr!("Frais (facultatif)"), 0.28);
            datepicker::date_field(
                ui,
                "quotes_sell_date",
                &mut entry.date,
                DATE,
                Placement::Popup,
            );
            let label = tr!("{}  Vendre", icon::HAND_COINS);
            submit |= ui
                .add(egui::Button::new(label).min_size(vec2(BUTTON, 36.0)))
                .clicked();
            cancel = ui
                .add(egui::Button::new(tr!("Annuler")).min_size(vec2(BUTTON, 36.0)))
                .clicked();
        });
        if cancel {
            self.open_panel(Panel::Buy);
        } else if submit {
            self.sell(code, doc);
        }
    }

    fn dividend_fields(
        &mut self,
        ui: &mut egui::Ui,
        code: &str,
        position: &Position,
        doc: &mut OpenDoc,
    ) {
        const GAP: f32 = 10.0;
        const BUTTON: f32 = 130.0;
        const DATE: f32 = 130.0;
        let mut submit = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let inner = ui.available_width() - 2.0 * BUTTON - DATE - 3.0 * GAP;
            let entry = &mut self.entry;
            let response = ui.add(
                crate::theme::field(&mut entry.price)
                    .hint_text(tr!("Montant net encaissé"))
                    .desired_width(inner),
            );
            submit |= response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            datepicker::date_field(
                ui,
                "quotes_div_date",
                &mut entry.date,
                DATE,
                Placement::Popup,
            );
            let label = tr!("{}  Enregistrer", icon::CHECK);
            submit |= ui
                .add(egui::Button::new(label).min_size(vec2(BUTTON, 36.0)))
                .clicked();
            cancel = ui
                .add(egui::Button::new(tr!("Annuler")).min_size(vec2(BUTTON, 36.0)))
                .clicked();
        });
        if cancel {
            self.open_panel(Panel::Buy);
        } else if submit {
            self.dividend(code, &position.name, doc);
        }
    }

    /// Cours passés, par code, dans la devise du compte. Renvoie aussi les
    /// erreurs à signaler.
    fn converted_histories(&self, doc: &OpenDoc) -> (DatedPrices, Vec<String>) {
        let mut prices = HashMap::new();
        let mut problems = Vec::new();
        let account = doc.current_account();
        let mut codes: Vec<&String> = self.histories.keys().collect();
        codes.sort();
        for code in codes {
            let converted = self.histories[code]
                .as_ref()
                .map_err(String::clone)
                .and_then(|h| {
                    let factor = quotes::price_in_account_currency(
                        1.0,
                        &h.currency,
                        account,
                        self.known_rates(),
                    )?;
                    Ok(h.points.iter().map(|&(d, p)| (d, p * factor)).collect())
                });
            match converted {
                Ok(points) => {
                    prices.insert(code.clone(), points);
                }
                Err(error) => problems.push(format!("{code} : {error}")),
            }
        }
        (prices, problems)
    }

    /// Courbes de l'onglet « Évolution ».
    fn evolution(
        &mut self,
        ui: &mut egui::Ui,
        positions: &[Position],
        doc: &OpenDoc,
        currency: &str,
        width: f32,
        pal: &Palette,
    ) {
        if positions.is_empty() {
            empty_state(ui, pal);
            return;
        }
        let loading = self.history_job.is_some();
        ui.horizontal(|ui| {
            let selected = self
                .chart_code
                .as_ref()
                .and_then(|code| positions.iter().find(|p| p.code == *code))
                .map_or(tr!("Tout le portefeuille"), |p| p.name.as_str());
            crate::ui_kit::Combo::new("quotes_chart_code")
                .selected_text(selected)
                .width(300.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.chart_code, None, tr!("Tout le portefeuille"));
                    for p in positions {
                        ui.selectable_value(
                            &mut self.chart_code,
                            Some(p.code.clone()),
                            format!("{} · {}", p.name, p.code),
                        );
                    }
                });
            ui.add_space(12.0);
            let ranges = [
                (HistoryRange::OneMonth, "", tr!("1 mois"), pal.accent),
                (HistoryRange::SixMonths, "", tr!("6 mois"), pal.accent),
                (HistoryRange::OneYear, "", tr!("1 an"), pal.accent),
                (HistoryRange::FiveYears, "", tr!("5 ans"), pal.accent),
                (HistoryRange::Max, "", tr!("Tout"), pal.accent),
            ];
            let changed = ui
                .add_enabled_ui(!loading, |ui| {
                    ui_kit::segmented(ui, &mut self.range, &ranges, 34.0, Some(380.0))
                })
                .inner;
            if changed {
                self.histories.clear();
            }
            if loading {
                ui.add(egui::Spinner::new());
            }
        });
        ui.add_space(10.0);
        let height = ui_kit::dialog_height(ui.ctx(), 0.42);
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
        let (prices, problems) = self.converted_histories(doc);
        let data = self.chart_data(positions, &prices);
        let points = data.as_ref().map_or(0, |d| d.series[0].points.len());
        if points >= 2 {
            let format = |v: f64| money(to_amount(v), currency);
            if let Some(data) = &data {
                crate::charts_ui::curve(ui, rect, data, &format, pal, 1.0);
            }
        } else {
            let text = if loading {
                tr!("Téléchargement de l'historique…")
            } else if self.chart_code.is_none() {
                tr!(
                    "Pas encore d'historique : le portefeuille commence à la date d'achat de ses titres. Choisissez un titre pour voir l'évolution de son cours."
                )
            } else {
                tr!("Aucun cours passé pour ce titre.")
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(14.0),
                pal.muted,
            );
        }
        ui.add_space(6.0);
        if let Some(values) = data.as_ref().map(|d| &d.series[0].points)
            && let (Some(first), Some(last)) = (values.first(), values.last())
            && points >= 2
            && first.value > 0.0
        {
            let change = (last.value / first.value - 1.0) * 100.0;
            let mut text = tr!("Sur la période : {}", percent(change));
            if self.chart_code.is_none() {
                text.push_str(tr!(
                    " · avec les quantités actuelles et le change du jour ; les titres déjà vendus n'y figurent pas."
                ));
            }
            ui.label(RichText::new(text).size(12.5).color(pal.fg_dark));
        }
        for problem in problems {
            ui.label(
                RichText::new(format!("{}  {problem}", icon::WARNING))
                    .size(12.5)
                    .color(pal.negative),
            );
        }
    }

    fn chart_data(&self, positions: &[Position], prices: &DatedPrices) -> Option<ChartData> {
        let label = |date: Date| shared::date(date);
        let series = |name: &str, points: Vec<ChartPoint>| ChartSeries {
            name: name.to_string(),
            points,
        };
        let mut list = Vec::new();
        match &self.chart_code {
            Some(code) => {
                let history = prices.get(code)?;
                let position = positions.iter().find(|p| p.code == *code)?;
                let name = tr!("Cours de {}", position.name);
                list.push(series(
                    &name,
                    history
                        .iter()
                        .map(|&(date, value)| ChartPoint {
                            label: label(date),
                            value,
                        })
                        .collect(),
                ));
                if let Some(cost) = position.unit_cost() {
                    list.push(series(
                        tr!("Prix de revient"),
                        history
                            .iter()
                            .map(|&(date, _)| ChartPoint {
                                label: label(date),
                                value: cost,
                            })
                            .collect(),
                    ));
                }
            }
            None => {
                let history = portfolio::value_history(&self.lines, prices);
                list.push(series(
                    tr!("Valeur du portefeuille"),
                    history
                        .iter()
                        .map(|p| ChartPoint {
                            label: label(p.date),
                            value: p.value,
                        })
                        .collect(),
                ));
                if self.lines.iter().all(|l| l.unit_cost.is_some()) {
                    list.push(series(
                        tr!("Montant investi"),
                        history
                            .iter()
                            .map(|p| ChartPoint {
                                label: label(p.date),
                                value: p.invested,
                            })
                            .collect(),
                    ));
                }
            }
        }
        Some(ChartData {
            unit: ChartUnit::Amount,
            series: list,
        })
    }

    /// Anneau de répartition de la valeur.
    fn allocation(&self, ui: &mut egui::Ui, positions: &[Position], currency: &str, pal: &Palette) {
        let points: Vec<(String, f64)> = positions
            .iter()
            .filter(|p| p.value > 0.0)
            .map(|p| (p.name.clone(), p.value))
            .collect();
        if points.is_empty() {
            empty_state(ui, pal);
            return;
        }
        let height = ui_kit::dialog_height(ui.ctx(), 0.46);
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
        let format = |v: f64| money(to_amount(v), currency);
        crate::charts_ui::donut(ui, rect, &points, &format, pal, 1.0);
    }

    /// Journal des ventes et dividendes, avec les totaux par année.
    fn journal(&self, ui: &mut egui::Ui, currency: &str, width: f32, pal: &Palette) {
        if self.movements.is_empty() {
            ui_kit::card(ui, width, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(tr!("Aucune vente ni aucun dividende enregistré."))
                            .family(crate::theme::semibold()),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(tr!(
                            "Utilisez les boutons de chaque ligne de l'onglet « Positions »."
                        ))
                        .color(pal.fg_dark),
                    );
                });
            });
            return;
        }
        ui_kit::caption(ui, icon::CALENDAR_BLANK, tr!("Par année"));
        for year in portfolio::year_totals(&self.movements) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(year.year.to_string()).family(crate::theme::semibold()));
                ui.add_space(14.0);
                ui.label(
                    RichText::new(tr!("Plus-values : {}", signed_money(year.gain, currency)))
                        .color(gain_color(year.gain, pal)),
                );
                ui.add_space(14.0);
                ui.label(
                    RichText::new(tr!(
                        "Dividendes : {}",
                        money(to_amount(year.dividends), currency)
                    ))
                    .color(pal.fg_dark),
                );
            });
        }
        ui.add_space(12.0);
        const ROW: f32 = 34.0;
        let inner = width - 16.0;
        let cols = [
            (inner * 0.13, egui::Align::Min, tr!("Date")),
            (inner * 0.13, egui::Align::Min, tr!("Nature")),
            (inner * 0.44, egui::Align::Min, tr!("Libellé")),
            (inner * 0.15, egui::Align::Max, tr!("Montant")),
            (inner * 0.15, egui::Align::Max, tr!("Plus-value")),
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
            .id_salt("quotes_journal")
            .max_height(ui_kit::dialog_height(ui.ctx(), 0.3))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (k, movement) in self.movements.iter().enumerate() {
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
                    let nature = match movement.kind {
                        MovementKind::Sale => tr!("Vente"),
                        MovementKind::Dividend => tr!("Dividende"),
                    };
                    ui_kit::cell(
                        ui,
                        cols[0].0,
                        egui::Align::Min,
                        RichText::new(shared::date(movement.date)).color(pal.fg_dark),
                    );
                    ui_kit::cell(ui, cols[1].0, egui::Align::Min, RichText::new(nature));
                    ui_kit::cell(
                        ui,
                        cols[2].0,
                        egui::Align::Min,
                        RichText::new(&movement.label).color(pal.fg_dark),
                    );
                    ui_kit::cell(
                        ui,
                        cols[3].0,
                        egui::Align::Max,
                        RichText::new(money(movement.amount, currency)),
                    );
                    let (gain, color) = movement.gain.map_or(("—".to_string(), pal.muted), |g| {
                        (signed_money(g, currency), gain_color(g, pal))
                    });
                    ui_kit::cell(
                        ui,
                        cols[4].0,
                        egui::Align::Max,
                        RichText::new(gain).color(color),
                    );
                }
            });
    }
}

fn empty_state(ui: &mut egui::Ui, pal: &Palette) {
    ui_kit::card(ui, ui.available_width(), |ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(tr!("Aucun titre suivi dans ce compte."))
                    .family(crate::theme::semibold()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(tr!(
                    "Ajoutez-en un depuis l'onglet « Positions » : il devient une ligne de mode « Titre » du compte, dont la valeur suit le cours."
                ))
                .color(pal.fg_dark),
            );
        });
    });
}

fn http_get(url: &str) -> Result<String, String> {
    ureq::get(url)
        .header("User-Agent", "Mozilla/5.0 BankPerfect")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Symbole d'un titre : un ISIN est d'abord converti.
fn resolve_symbol(identifier: &str) -> Result<String, String> {
    if quotes::looks_like_isin(identifier) {
        let json = http_get(&format!("{}{}", quotes::SEARCH_URL, identifier))?;
        Ok(quotes::parse_search(&json).ok_or(tr!("code ISIN introuvable"))?)
    } else {
        Ok(identifier.to_string())
    }
}

/// Requête « chart » pour un symbole.
fn fetch_chart(symbol: &str, query: &str) -> Result<String, String> {
    http_get(&format!("{}{}?{}", quotes::CHART_URL, symbol, query)).map_err(|e| {
        if e.contains("404") {
            tr!("symbole {} inconnu", symbol)
        } else {
            e
        }
    })
}

/// Cours d'un titre.
fn fetch_quote(identifier: &str) -> Result<Quote, String> {
    let symbol = resolve_symbol(identifier)?;
    let json = fetch_chart(&symbol, "range=1d&interval=1d")?;
    quotes::parse_chart(&json).ok_or_else(|| tr!("aucun cours pour {}", symbol))
}

/// Cours passés d'un titre.
fn fetch_history(identifier: &str, range: HistoryRange) -> Result<PriceHistory, String> {
    let symbol = resolve_symbol(identifier)?;
    let json = fetch_chart(&symbol, range.query())?;
    quotes::parse_history(&json).ok_or_else(|| tr!("aucun cours pour {}", symbol))
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
        assert!(q.previous_close.is_some());
        let q = fetch_quote("AAPL").expect("cours d'Apple");
        assert_eq!(q.currency, "USD");
    }

    /// Téléchargement réel (réseau) : `cargo test -- --ignored quote`.
    #[test]
    #[ignore]
    fn downloads_a_quote_history() {
        let h = fetch_history("FR0000133308", HistoryRange::OneYear).expect("historique d'Orange");
        assert_eq!(h.currency, "EUR");
        assert!(h.points.len() > 200);
        assert!(h.points.windows(2).all(|w| w[0].0 < w[1].0));
        let h = fetch_history("AAPL", HistoryRange::Max).expect("historique d'Apple");
        assert!(h.points.len() > 100);
    }
}
