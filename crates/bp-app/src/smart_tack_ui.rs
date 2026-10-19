//! « Pointage intelligent… » : d'après le solde annoncé par la banque,
//! propose les opérations non pointées qu'elle a déjà passées. Les
//! propositions sont vérifiées et validées par l'utilisateur, en une étape
//! d'annulation.

use std::sync::{Arc, Mutex};

use bp_core::model::{Amount, Date};
use bp_core::smart_tack::{self, Options, Proposal, SmartTack};
use bp_core::{tr, trn};
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

type Job = Arc<Mutex<Option<SmartTack>>>;

pub struct SmartTackForm {
    /// Compte concerné (celui affiché à l'ouverture).
    account: usize,
    balance: String,
    date: String,
    job: Option<Job>,
    result: Option<SmartTack>,
    /// Proposition affichée.
    shown: usize,
    /// Lignes cochées de la proposition affichée.
    checked: Vec<(usize, bool)>,
    error: Option<String>,
    /// Recherche à lancer à la prochaine image (solde connu à l'ouverture).
    start: bool,
}

const LABEL_WIDTH: f32 = 150.0;
const ROW_HEIGHT: f32 = 32.0;

impl SmartTackForm {
    pub fn new(doc: &OpenDoc) -> Self {
        let known = doc.bank_balance(doc.account);
        Self {
            account: doc.account,
            balance: known.map_or(String::new(), |(a, _)| {
                a.format(2, bp_core::i18n::decimal_separator())
            }),
            date: known
                .and_then(|(_, d)| d)
                .unwrap_or_else(Date::today)
                .to_string(),
            job: None,
            result: None,
            shown: 0,
            checked: Vec::new(),
            error: None,
            start: known.is_some(),
        }
    }

    fn inputs(&self) -> Result<(Amount, Date), String> {
        let amount = bp_core::budget::parse_amount_input(&self.balance)
            .ok_or(tr!("Indiquez le solde affiché par votre banque"))?;
        let date =
            Date::parse_fr(&self.date, Date::today()).ok_or(tr!("Date invalide (jj/mm/aaaa)"))?;
        Ok((amount, date))
    }

    fn launch(&mut self, ctx: &egui::Context, doc: &OpenDoc) {
        let (amount, date) = match self.inputs() {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        self.error = None;
        self.result = None;
        let account = doc.document.accounts[self.account].clone();
        let job: Job = Arc::new(Mutex::new(None));
        let slot = job.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = smart_tack::search(&account, amount, Some(date), &Options::default());
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
            ctx.request_repaint();
        });
        self.job = Some(job);
    }

    fn poll(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        let Some(result) = job.lock().ok().and_then(|mut g| g.take()) else {
            return;
        };
        self.job = None;
        self.shown = 0;
        self.result = Some(result);
        self.select_shown();
    }

    fn proposals(&self) -> &[Proposal] {
        match &self.result {
            Some(SmartTack::Found(list)) => list,
            _ => &[],
        }
    }

    fn select_shown(&mut self) {
        self.checked = self
            .proposals()
            .get(self.shown)
            .map(|p| p.lines.iter().map(|&i| (i, true)).collect())
            .unwrap_or_default();
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        self.poll();
        if std::mem::take(&mut self.start) {
            self.launch(ctx, doc);
        }
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.7, 820.0, 1200.0);
        let height = ui_kit::dialog_height(ctx, 0.85).max(560.0);
        crate::dialogs::modal(ctx, "smart_tack").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::CHECKS, pal.pointed, tr!("Pointage intelligent"));
            let account = &doc.document.accounts[self.account];
            let currency = account.currency.clone();
            ui.label(
                RichText::new(tr!(
                    "Indiquez le solde affiché par votre banque : BankPerfect cherche les \
                     opérations non pointées qu'elle a déjà passées, en tenant compte des délais \
                     habituels (cartes, chèques)."
                ))
                .color(pal.fg_dark),
            );
            ui.add_space(12.0);
            let mut search = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}  {}", icon::BANK, account.name)).color(pal.muted));
            });
            ui.add_space(6.0);
            ui_kit::row(ui, tr!("Solde en banque"), LABEL_WIDTH, |ui| {
                let field = ui.add(
                    crate::theme::field(&mut self.balance)
                        .hint_text(tr!("0,00"))
                        .horizontal_align(egui::Align::Max)
                        .desired_width(160.0),
                );
                if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    search = true;
                }
                ui.label(RichText::new(&currency).color(pal.muted));
                ui.add_space(18.0);
                ui.label(RichText::new(tr!("au")).color(pal.fg_dark));
                crate::datepicker::date_field(
                    ui,
                    "smart_tack_date",
                    &mut self.date,
                    150.0,
                    crate::datepicker::Placement::Popup,
                );
                ui.add_space(12.0);
                let busy = self.job.is_some();
                let label = if busy {
                    format!("{}  {}", icon::HOURGLASS, tr!("Recherche…"))
                } else {
                    format!("{}  {}", icon::MAGNIFYING_GLASS, tr!("Chercher"))
                };
                if ui
                    .add_enabled(!busy, egui::Button::new(label).min_size(egui::vec2(120.0, 34.0)))
                    .clicked()
                {
                    search = true;
                }
            });
            if search {
                self.launch(ui.ctx(), doc);
            }
            if let Ok((amount, date)) = self.inputs() {
                let pointed = smart_tack::pointed_balance(account, Some(date));
                let unpointed = account
                    .operations
                    .iter()
                    .filter(|o| o.tack == bp_core::model::Tack::None && o.date <= date)
                    .count();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(LABEL_WIDTH + 8.0);
                    ui.label(
                        RichText::new(trn!(
                            unpointed,
                            "Solde pointé {1} · écart {2} · {0} opération non pointée jusqu'à cette date",
                            "Solde pointé {1} · écart {2} · {0} opérations non pointées jusqu'à cette date",
                            money(pointed, &currency),
                            money(Amount(amount.0 - pointed.0), &currency)
                        ))
                        .color(pal.muted),
                    );
                });
            }
            if let Some(error) = &self.error {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            ui.add_space(14.0);
            let body = height - 36.0 - 14.0 - 290.0;
            // Hauteur fixe : le pied reste en bas de la fenêtre.
            let layout = egui::Layout::top_down(egui::Align::Min);
            ui.allocate_ui_with_layout(egui::vec2(width, body), layout, |ui| {
                ui.set_min_height(body);
                self.results(ui, doc, &currency, width, body);
            });

            let count = self.checked.iter().filter(|(_, c)| *c).count();
            crate::dialogs::footer(ui, |ui| {
                let label = format!(
                    "{}  {}",
                    icon::CHECK,
                    trn!(count, "Pointer {} opération", "Pointer {} opérations")
                );
                if crate::dialogs::primary(ui, &label, count > 0).clicked() {
                    let lines: Vec<usize> = self
                        .checked
                        .iter()
                        .filter(|(_, c)| *c)
                        .map(|&(i, _)| i)
                        .collect();
                    let n = doc.point_lines(self.account, &lines);
                    outcome = Outcome::Done(trn!(n, "{} opération pointée", "{} opérations pointées"));
                }
                if crate::dialogs::button(ui, tr!("Fermer")).clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    outcome = Outcome::Close;
                }
            });
        });
        outcome
    }

    fn results(
        &mut self,
        ui: &mut egui::Ui,
        doc: &OpenDoc,
        currency: &str,
        width: f32,
        height: f32,
    ) {
        let pal = palette(ui);
        let message = |ui: &mut egui::Ui, glyph: &str, color: egui::Color32, text: &str| {
            ui.allocate_ui(egui::vec2(width, height), |ui| {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(glyph).size(36.0).color(color));
                    ui.add_space(8.0);
                    ui.label(RichText::new(text).color(pal.fg_dark));
                });
            });
        };
        let result = match &self.result {
            None if self.job.is_some() => {
                message(ui, icon::HOURGLASS, pal.muted, tr!("Recherche en cours…"));
                return;
            }
            None => {
                message(
                    ui,
                    icon::BANK,
                    pal.muted,
                    tr!("Saisissez le solde de votre relevé ou de votre application bancaire."),
                );
                return;
            }
            Some(r) => r.clone(),
        };
        match result {
            SmartTack::Balanced => message(
                ui,
                icon::CHECK_CIRCLE,
                pal.positive,
                tr!("Le solde pointé correspond déjà au solde en banque."),
            ),
            SmartTack::Impossible { difference } => message(
                ui,
                icon::WARNING,
                pal.warning,
                &tr!(
                    "Aucune combinaison d'opérations non pointées ne donne cet écart ({}).\n\
                     Une opération manque, est en double ou a un montant différent.",
                    money(difference, currency)
                ),
            ),
            SmartTack::NoPlausible { difference, exists } => {
                let detail = match exists {
                    Some(true) => tr!(
                        "Certaines combinaisons tombent juste, mais aucune ne ressemble à \
                         l'ordre de passage habituel : les proposer serait du hasard."
                    ),
                    _ => tr!("Le calcul complet a pris trop de temps."),
                };
                message(
                    ui,
                    icon::QUESTION,
                    pal.warning,
                    &tr!(
                        "Aucune combinaison vraisemblable pour un écart de {}.\n{}\n\
                         Vérifiez d'abord les opérations manquantes ou les montants.",
                        money(difference, currency),
                        detail
                    ),
                );
            }
            SmartTack::Found(list) => self.found(ui, doc, &list, currency, width, height),
        }
    }

    fn found(
        &mut self,
        ui: &mut egui::Ui,
        doc: &OpenDoc,
        list: &[Proposal],
        currency: &str,
        width: f32,
        height: f32,
    ) {
        let pal = palette(ui);
        let account = &doc.document.accounts[self.account];
        ui.horizontal(|ui| {
            if list.len() > 1 {
                let before = self.shown;
                let options: Vec<(usize, String)> = list
                    .iter()
                    .enumerate()
                    .map(|(k, _)| (k, tr!("Proposition {}", k + 1)))
                    .collect();
                let options: Vec<(usize, &str, &str, egui::Color32)> = options
                    .iter()
                    .map(|(k, label)| (*k, "", label.as_str(), pal.accent))
                    .collect();
                ui_kit::segmented(
                    ui,
                    &mut self.shown,
                    &options,
                    32.0,
                    Some(130.0 * list.len() as f32),
                );
                if self.shown != before {
                    self.select_shown();
                }
                ui.add_space(10.0);
                // Deux hypothèses presque aussi vraisemblables : à vérifier.
                if list[1].score - list[0].score < 0.5 {
                    ui_kit::pill(
                        ui,
                        tr!("Plusieurs combinaisons plausibles : vérifiez"),
                        pal.warning,
                    );
                }
            }
        });
        let Some(p) = list.get(self.shown) else {
            return;
        };
        ui.add_space(8.0);
        let date = |d: Option<Date>| d.map(crate::settings::shared::date);
        let others = match date(p.until) {
            Some(d) => tr!("Autres opérations jusqu'au {}", d),
            None => tr!("Autres opérations : aucune").to_string(),
        };
        let cards = match date(p.cards_until) {
            Some(d) => tr!("cartes jusqu'au {}", d),
            None => tr!("cartes : aucune").to_string(),
        };
        let cheques = match date(p.cheques_until) {
            Some(d) => tr!("chèques jusqu'au {}", d),
            None => tr!("chèques : aucun").to_string(),
        };
        let mut summary = format!("{others} · {cards} · {cheques}");
        let names = |lines: &[usize]| -> String {
            lines
                .iter()
                .map(|&i| {
                    let o = &account.operations[i];
                    format!("{} ({})", o.payee, money(o.amount, currency))
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        if !p.held_back.is_empty() {
            summary.push('\n');
            summary.push_str(&tr!("Pas encore passées : {}", names(&p.held_back)));
        }
        if !p.early.is_empty() {
            summary.push('\n');
            summary.push_str(&tr!("Déjà passées : {}", names(&p.early)));
        }
        ui.label(RichText::new(summary).color(pal.fg_dark));
        ui.add_space(8.0);

        let checked_total: Amount = self
            .checked
            .iter()
            .filter(|(_, c)| *c)
            .map(|&(i, _)| account.operations[i].amount)
            .sum();
        let matches = checked_total == p.total;
        ui.horizontal(|ui| {
            ui_kit::pill(
                ui,
                &trn!(p.lines.len(), "{} ligne", "{} lignes"),
                pal.accent,
            );
            let (text, color) = if matches {
                (
                    format!(
                        "{} {}",
                        icon::CHECK,
                        tr!("total : solde en banque retrouvé")
                    ),
                    pal.positive,
                )
            } else {
                (
                    tr!(
                        "total coché {} au lieu de {}",
                        money(checked_total, currency),
                        money(p.total, currency)
                    ),
                    pal.warning,
                )
            };
            ui_kit::pill(ui, &text, color);
        });
        ui.add_space(6.0);
        let list_height = (height - 150.0).max(140.0);
        let gap = ui.spacing().item_spacing.x;
        let label_w = (width - 28.0 - 96.0 - 150.0 - 120.0 - 5.0 * gap - 20.0).max(160.0);
        egui::ScrollArea::vertical()
            .id_salt(("smart_tack_rows", self.shown))
            .max_height(list_height)
            .auto_shrink([false, true])
            .show_rows(ui, ROW_HEIGHT, self.checked.len(), |ui, range| {
                for k in range {
                    let (i, checked) = &mut self.checked[k];
                    let o = &account.operations[*i];
                    ui.horizontal(|ui| {
                        ui.set_height(ROW_HEIGHT);
                        crate::theme::checkbox(ui, checked, "");
                        ui_kit::cell(
                            ui,
                            96.0,
                            egui::Align::Min,
                            RichText::new(crate::settings::shared::date(o.date)),
                        );
                        let (glyph, color) = crate::grid::mode_style(&doc.document, &o.mode, &pal);
                        ui_kit::cell(
                            ui,
                            150.0,
                            egui::Align::Min,
                            RichText::new(format!("{glyph}  {}", o.mode_label())).color(color),
                        );
                        let text = if o.details.is_empty() {
                            o.payee.clone()
                        } else {
                            format!("{}  ·  {}", o.payee, o.details)
                        };
                        ui_kit::cell(ui, label_w, egui::Align::Min, RichText::new(text));
                        let color = if o.amount.is_negative() {
                            pal.negative
                        } else {
                            pal.positive
                        };
                        ui_kit::cell(
                            ui,
                            120.0,
                            egui::Align::Max,
                            RichText::new(money(o.amount, currency)).color(color),
                        );
                    });
                }
            });
    }
}
