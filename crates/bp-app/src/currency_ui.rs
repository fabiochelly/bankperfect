//! Convertisseur de devises (Ctrl+D) : la saisie d'un montant dans une
//! devise recalcule toutes les autres, d'après les cours de la BCE.

use std::sync::{Arc, Mutex};

use bp_core::currencies::{CURRENCIES, ECB_DAILY_URL, Rates, currency_name, search_key};
use egui::{Key, RichText, vec2};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

use crate::theme::{Palette, palette};
use crate::tools::Outcome;
use crate::ui_kit;

const FILE: &str = "devises.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    currencies: Vec<String>,
    #[serde(default)]
    rates: Rates,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            currencies: ["EUR", "USD", "GBP", "CHF"].map(String::from).to_vec(),
            rates: Rates::default(),
        }
    }
}

type Fetch = Arc<Mutex<Option<Result<Rates, String>>>>;

pub struct CurrencyForm {
    saved: Saved,
    /// Montant affiché pour chaque devise.
    amounts: Vec<String>,
    /// Devise saisie en dernier et son montant (base des conversions).
    source: (usize, f64),
    /// Cours saisi à la main en cours d'édition : (ligne, texte).
    rate_edit: Option<(usize, String)>,
    search: String,
    adding: bool,
    fetch: Option<Fetch>,
    error: Option<String>,
    message: Option<String>,
}

impl CurrencyForm {
    pub fn new() -> Self {
        let saved = crate::settings::config_file(FILE)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Saved>(&b).ok())
            .unwrap_or_default();
        let mut form = Self {
            amounts: vec![String::new(); saved.currencies.len()],
            saved,
            source: (0, 100.0),
            rate_edit: None,
            search: String::new(),
            adding: false,
            fetch: None,
            error: None,
            message: None,
        };
        form.recompute();
        form
    }

    fn persist(&mut self) {
        let result = serde_json::to_vec_pretty(&self.saved)
            .map_err(|e| e.to_string())
            .and_then(|json| crate::settings::write_config_file(FILE, &json));
        if let Err(e) = result {
            self.error = Some(format!("Enregistrement impossible : {e}"));
        }
    }

    /// Recalcule tous les montants à partir de la devise source.
    fn recompute(&mut self) {
        let (k, value) = self.source;
        let Some(from) = self.saved.currencies.get(k).cloned() else {
            return;
        };
        for (i, code) in self.saved.currencies.iter().enumerate() {
            if i == k {
                if self.amounts[i].trim().is_empty() || parse(&self.amounts[i]) != Some(value) {
                    self.amounts[i] = format_amount(value);
                }
                continue;
            }
            self.amounts[i] = self
                .saved
                .rates
                .convert(value, &from, code)
                .map(format_amount)
                .unwrap_or_default();
        }
    }

    fn start_fetch(&mut self, ctx: &egui::Context) {
        let slot: Fetch = Arc::new(Mutex::new(None));
        let thread_slot = slot.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = fetch_ecb();
            if let Ok(mut guard) = thread_slot.lock() {
                *guard = Some(result);
            }
            ctx.request_repaint();
        });
        self.fetch = Some(slot);
        self.error = None;
        self.message = None;
    }

    fn poll_fetch(&mut self) {
        let Some(slot) = &self.fetch else { return };
        let done = slot.lock().ok().and_then(|mut g| g.take());
        if let Some(result) = done {
            self.fetch = None;
            match result {
                Ok(rates) => {
                    let count = rates.per_euro.len() - 1;
                    self.saved.rates.merge(rates);
                    self.persist();
                    self.recompute();
                    self.message = Some(format!(
                        "{count} cours de la BCE mis à jour ({})",
                        display_date(&self.saved.rates.date)
                    ));
                }
                Err(e) => self.error = Some(format!("Téléchargement des cours impossible : {e}")),
            }
        }
    }

    pub fn show_standalone(&mut self, ctx: &egui::Context) -> Outcome {
        self.poll_fetch();
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.5, 680.0, 860.0);
        crate::dialogs::modal(ctx, "currencies").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(
                ui,
                icon::CURRENCY_CIRCLE_DOLLAR,
                pal.accent,
                "Convertisseur de devises",
            );
            ui_kit::subtitle(
                ui,
                "Tapez un montant dans une devise : toutes les autres se recalculent.",
            );
            self.rates_bar(ui, ctx, &pal);
            ui.add_space(12.0);
            self.rows(ui, width, &pal);
            ui.add_space(10.0);
            self.add_currency(ui, width, &pal);
            if let Some(error) = &self.error {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            } else if let Some(message) = &self.message {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!("{}  {message}", icon::CHECK_CIRCLE)).color(pal.positive),
                );
            }
            crate::dialogs::footer(ui, |ui| {
                let escape = ui.input(|i| i.key_pressed(Key::Escape))
                    && !self.adding
                    && self.rate_edit.is_none();
                if crate::dialogs::primary(ui, "Fermer", true).clicked() || escape {
                    outcome = Outcome::Close;
                }
            });
        });
        outcome
    }

    fn rates_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, pal: &Palette) {
        ui_kit::card(ui, ui.available_width(), |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::BANK).size(18.0).color(pal.accent));
                let text_w = (ui.available_width() - 270.0).max(160.0);
                ui.vertical(|ui| {
                    ui.set_max_width(text_w);
                    if self.saved.rates.date.is_empty() {
                        ui.label(
                            RichText::new("Aucun cours téléchargé")
                                .family(crate::theme::semibold()),
                        );
                        ui.label(
                            RichText::new("Cours de référence de la Banque centrale européenne.")
                                .size(12.5)
                                .color(pal.muted),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!(
                                "Cours BCE du {}",
                                display_date(&self.saved.rates.date)
                            ))
                            .family(crate::theme::semibold()),
                        );
                        ui.label(
                            RichText::new(
                                "Publiés chaque jour ouvré vers 16 h, à titre indicatif.",
                            )
                            .size(12.5)
                            .color(pal.muted),
                        );
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.fetch.is_some() {
                        ui.add(egui::Spinner::new());
                        ui.label(RichText::new("Téléchargement…").color(pal.muted));
                    } else if ui
                        .add(crate::theme::primary_button(
                            ui,
                            format!("{}  Mettre à jour les cours", icon::ARROWS_CLOCKWISE),
                        ))
                        .clicked()
                    {
                        self.start_fetch(ctx);
                    }
                });
            });
        });
    }

    fn rows(&mut self, ui: &mut egui::Ui, width: f32, pal: &Palette) {
        let mut edited = None;
        let mut remove = None;
        let mut rate_commit = None;
        let mut start_rate_edit = None;
        for (k, code) in self.saved.currencies.iter().enumerate() {
            let rate = self.saved.rates.rate(code);
            let source = self.source.0 == k;
            let frame = egui::Frame::new()
                .fill(if source {
                    pal.accent.gamma_multiply(0.10)
                } else {
                    pal.bg
                })
                .stroke(egui::Stroke::new(
                    1.0,
                    if source {
                        pal.accent.gamma_multiply(0.5)
                    } else {
                        pal.border.gamma_multiply(0.6)
                    },
                ))
                .corner_radius(12)
                .inner_margin(egui::Margin::symmetric(16, 10));
            frame.show(ui, |ui| {
                ui.set_width(width - 34.0);
                ui.horizontal(|ui| {
                    // Code de la devise.
                    let (badge, _) = ui.allocate_exact_size(vec2(56.0, 36.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(badge, 8.0, pal.accent.gamma_multiply(0.16));
                    ui.painter().text(
                        badge.center(),
                        egui::Align2::CENTER_CENTER,
                        code,
                        egui::FontId::new(14.0, crate::theme::semibold()),
                        pal.accent,
                    );
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(currency_name(code)).family(crate::theme::medium()));
                        match (&mut self.rate_edit, rate) {
                            (Some((row, text)), _) if *row == k => {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("1 € =").size(12.5).color(pal.muted));
                                    let r = ui.add(
                                        egui::TextEdit::singleline(text)
                                            .desired_width(90.0)
                                            .hint_text("cours"),
                                    );
                                    if !r.has_focus() && !r.lost_focus() {
                                        r.request_focus();
                                    }
                                    if r.lost_focus() {
                                        rate_commit = Some((k, text.clone()));
                                    }
                                });
                            }
                            (_, Some(r)) if code != "EUR" => {
                                let manual = self.saved.rates.date.is_empty() || !is_ecb(code);
                                let text = format!(
                                    "1 € = {} {code}{}",
                                    format_rate(r),
                                    if manual { " · cours manuel" } else { "" }
                                );
                                let response = ui.add(
                                    egui::Label::new(
                                        RichText::new(text).size(12.5).color(pal.muted),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if response
                                    .on_hover_text("Cliquer pour saisir un cours")
                                    .clicked()
                                {
                                    start_rate_edit = Some((k, format_rate(r)));
                                }
                            }
                            (_, None) => {
                                if ui
                                    .link(
                                        RichText::new(format!(
                                            "{}  Saisir un cours",
                                            icon::PENCIL_SIMPLE
                                        ))
                                        .size(12.5),
                                    )
                                    .clicked()
                                {
                                    start_rate_edit = Some((k, String::new()));
                                }
                            }
                            _ => {
                                ui.label(
                                    RichText::new("Devise de référence")
                                        .size(12.5)
                                        .color(pal.muted),
                                );
                            }
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.saved.currencies.len() > 1
                            && ui
                                .add(
                                    egui::Button::new(RichText::new(icon::TRASH).color(pal.muted))
                                        .frame(false),
                                )
                                .on_hover_text("Retirer cette devise")
                                .clicked()
                        {
                            remove = Some(k);
                        }
                        ui.add_space(6.0);
                        let edit = egui::TextEdit::singleline(&mut self.amounts[k])
                            .horizontal_align(egui::Align::Max)
                            .font(egui::FontId::new(18.0, crate::theme::semibold()))
                            .margin(egui::Margin::symmetric(10, 6))
                            .desired_width(220.0)
                            .hint_text(if rate.is_some() {
                                "0,00"
                            } else {
                                "cours manquant"
                            });
                        if ui.add_enabled(rate.is_some(), edit).changed() {
                            edited = Some(k);
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
        if let Some(k) = edited
            && let Some(v) = parse(&self.amounts[k])
        {
            self.source = (k, v);
            self.recompute();
        } else if let Some(k) = edited
            && self.amounts[k].trim().is_empty()
        {
            self.source = (k, 0.0);
            let keep = self.amounts[k].clone();
            self.recompute();
            self.amounts[k] = keep;
        }
        if let Some(edit) = start_rate_edit {
            self.rate_edit = Some(edit);
        }
        if let Some((k, text)) = rate_commit {
            self.rate_edit = None;
            let code = self.saved.currencies[k].clone();
            match parse(&text) {
                Some(r) if r > 0.0 => {
                    self.saved.rates.per_euro.insert(code, r);
                    self.persist();
                    self.recompute();
                    self.error = None;
                }
                None if text.trim().is_empty() => {}
                _ => self.error = Some(format!("Cours incorrect : « {} »", text.trim())),
            }
        }
        if let Some(k) = remove {
            self.saved.currencies.remove(k);
            self.amounts.remove(k);
            if self.source.0 >= self.saved.currencies.len() || self.source.0 == k {
                let v =
                    parse(self.amounts.first().map(String::as_str).unwrap_or("")).unwrap_or(0.0);
                self.source = (0, v);
            } else if self.source.0 > k {
                self.source.0 -= 1;
            }
            self.persist();
            self.recompute();
        }
    }

    fn add_currency(&mut self, ui: &mut egui::Ui, width: f32, pal: &Palette) {
        if !self.adding {
            if ui
                .button(format!("{}  Ajouter une devise", icon::PLUS))
                .clicked()
            {
                self.adding = true;
                self.search.clear();
            }
            return;
        }
        let mut chosen = None;
        ui_kit::card(ui, width, |ui| {
            ui.horizontal(|ui| {
                let r = ui.add(
                    crate::theme::field(&mut self.search)
                        .hint_text(format!(
                            "{}  Chercher : dollar, yen, CHF…",
                            icon::MAGNIFYING_GLASS
                        ))
                        .desired_width(width - 160.0),
                );
                if !r.has_focus() && !r.lost_focus() && self.search.is_empty() {
                    r.request_focus();
                }
                if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.adding = false;
                }
            });
            ui.add_space(6.0);
            let key = search_key(&self.search);
            let matches: Vec<&(&str, &str)> = CURRENCIES
                .iter()
                .filter(|(code, _)| !self.saved.currencies.iter().any(|c| c == code))
                .filter(|(code, name)| {
                    key.is_empty()
                        || search_key(code).contains(&key)
                        || search_key(name).contains(&key)
                })
                .collect();
            egui::ScrollArea::vertical()
                .id_salt("currency_search")
                .max_height(150.0)
                .show(ui, |ui| {
                    ui.set_width(width - 2.0 * ui_kit::CARD_PADDING - 16.0);
                    if matches.is_empty() {
                        ui.label(RichText::new("Aucune devise trouvée.").color(pal.muted));
                    }
                    for (code, name) in matches {
                        let available = self.saved.rates.rate(code).is_some();
                        let tail = if available {
                            ""
                        } else {
                            "  · cours à saisir"
                        };
                        if ui
                            .selectable_label(false, format!("{code}  {name}{tail}"))
                            .clicked()
                        {
                            chosen = Some(code.to_string());
                        }
                    }
                });
            if ui.input(|i| i.key_pressed(Key::Enter))
                && let Some((code, _)) = CURRENCIES
                    .iter()
                    .filter(|(code, _)| !self.saved.currencies.iter().any(|c| c == code))
                    .find(|(code, name)| {
                        !key.is_empty()
                            && (search_key(code).contains(&key) || search_key(name).contains(&key))
                    })
            {
                chosen = Some(code.to_string());
            }
        });
        if let Some(code) = chosen {
            self.saved.currencies.push(code);
            self.amounts.push(String::new());
            self.adding = false;
            self.persist();
            self.recompute();
        }
    }
}

/// Cours enregistrés par le convertisseur (vides si jamais téléchargés).
pub fn saved_rates() -> Rates {
    crate::settings::config_file(FILE)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Saved>(&b).ok())
        .map(|s| s.rates)
        .unwrap_or_default()
}

/// Télécharge et lit le flux quotidien de la BCE.
pub fn fetch_ecb() -> Result<Rates, String> {
    let xml = ureq::get(ECB_DAILY_URL)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    Rates::parse_ecb(&xml).map_err(|e| e.to_string())
}

/// Devises publiées par la BCE (cours tenus à jour par le téléchargement).
fn is_ecb(code: &str) -> bool {
    const ECB: [&str; 30] = [
        "USD", "JPY", "CZK", "DKK", "GBP", "HUF", "PLN", "RON", "SEK", "CHF", "ISK", "NOK", "TRY",
        "AUD", "BRL", "CAD", "CNY", "HKD", "IDR", "ILS", "INR", "KRW", "MXN", "MYR", "NZD", "PHP",
        "SGD", "THB", "ZAR", "BGN",
    ];
    ECB.contains(&code)
}

fn parse(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '\u{202f}' | '\u{a0}' | '€'))
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    let normalized = cleaned.replace(',', ".");
    normalized
        .parse::<f64>()
        .ok()
        .or_else(|| {
            crate::calculator::evaluate(&normalized)
                .ok()
                .map(|(v, _)| v)
        })
        .filter(|v| v.is_finite())
}

fn format_amount(v: f64) -> String {
    crate::format::money(
        bp_core::model::Amount::from_cents((v * 100.0).round() as i64),
        "",
    )
}

fn format_rate(r: f64) -> String {
    let digits = if r >= 100.0 { 2 } else { 4 };
    format!("{r:.digits$}").replace('.', ",")
}

/// `2026-10-09` → `09/10/2026`.
fn display_date(iso: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    match parts.as_slice() {
        [y, m, d] => format!("{d}/{m}/{y}"),
        _ => iso.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Téléchargement réel (réseau) : `cargo test -- --ignored ecb`.
    #[test]
    #[ignore]
    fn downloads_the_ecb_rates() {
        let rates = fetch_ecb().expect("flux BCE");
        assert!(rates.per_euro.len() > 20);
        assert!(rates.rate("USD").is_some_and(|r| r > 0.5 && r < 2.0));
        assert_eq!(rates.date.len(), 10);
    }

    #[test]
    fn typing_in_one_currency_updates_the_others() {
        let mut form = CurrencyForm {
            saved: Saved {
                currencies: vec!["EUR".into(), "USD".into(), "XOF".into()],
                rates: Rates::default(),
            },
            amounts: vec![String::new(); 3],
            source: (0, 100.0),
            rate_edit: None,
            search: String::new(),
            adding: false,
            fetch: None,
            error: None,
            message: None,
        };
        form.saved.rates.per_euro.insert("USD".into(), 1.1);
        form.recompute();
        assert_eq!(form.amounts, vec!["100,00", "110,00", ""]);
        form.source = (1, 55.0);
        form.recompute();
        assert_eq!(form.amounts[0], "50,00");
        assert_eq!(display_date("2026-10-09"), "09/10/2026");
        assert_eq!(format_rate(1.08764), "1,0876");
        assert_eq!(format_rate(162.351), "162,35");
    }
}
