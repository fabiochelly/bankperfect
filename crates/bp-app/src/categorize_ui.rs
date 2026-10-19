//! « Catégoriser les opérations… » : propositions pour les opérations sans
//! catégorie des lignes affichées ou de tout le fichier, à valider avant
//! d'être appliquées (en une étape d'annulation). En dernier recours, et
//! seulement à la demande, les libellés inconnus peuvent être soumis à l'IA.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bp_core::categorize::{AUTO_THRESHOLD, Suggestion, ai, propose};
use bp_core::{tr, tr_mark, trn};
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Lignes affichées du compte courant (filtres compris).
    Visible,
    File,
}

struct Row {
    account: usize,
    index: usize,
    suggestion: Suggestion,
    choice: Option<u32>,
    apply: bool,
    /// Choix modifié à la main : gardé quand les propositions sont refaites.
    edited: bool,
}

/// Réglages de l'IA repris des options.
pub struct AiSettings {
    pub enabled: bool,
    pub api_key: String,
    pub model: String,
}

type AiJob = Arc<Mutex<Option<Result<Vec<(String, String)>, String>>>>;

pub struct CategorizeForm {
    scope: Scope,
    rows: Vec<Row>,
    /// Propositions à refaire (portée changée, réponses de l'IA reçues).
    stale: bool,
    ai: AiSettings,
    ai_job: Option<AiJob>,
    message: Option<String>,
}

const ROW_HEIGHT: f32 = 36.0;

impl CategorizeForm {
    pub fn new(doc: &OpenDoc, ai: AiSettings) -> Self {
        // Des lignes filtrées : on part d'elles ; sinon de tout le fichier.
        let filtered = doc.visible_len() < doc.current_account().operations.len();
        Self {
            scope: if filtered {
                Scope::Visible
            } else {
                Scope::File
            },
            rows: Vec::new(),
            stale: true,
            ai,
            ai_job: None,
            message: None,
        }
    }

    fn targets(&self, doc: &OpenDoc) -> Vec<(usize, usize)> {
        match self.scope {
            Scope::Visible => doc.visible().iter().map(|&i| (doc.account, i)).collect(),
            Scope::File => doc
                .document
                .account_order
                .iter()
                .flat_map(|&a| {
                    let n = doc.document.accounts[a].operations.len();
                    (0..n).rev().map(move |i| (a, i))
                })
                .collect(),
        }
    }

    fn recompute(&mut self, doc: &OpenDoc) {
        let categorizer = doc.categorizer();
        let previous: Vec<Row> = std::mem::take(&mut self.rows);
        self.rows = propose(&doc.document, &categorizer, self.targets(doc))
            .into_iter()
            .map(|p| {
                if let Some(old) = previous
                    .iter()
                    .find(|r| r.edited && r.account == p.account && r.index == p.index)
                {
                    return Row {
                        account: p.account,
                        index: p.index,
                        suggestion: p.suggestion,
                        choice: old.choice,
                        apply: old.apply,
                        edited: true,
                    };
                }
                let choice = p.suggestion.best().map(|b| b.category);
                Row {
                    account: p.account,
                    index: p.index,
                    apply: p.suggestion.confident().is_some(),
                    choice,
                    suggestion: p.suggestion,
                    edited: false,
                }
            })
            .collect();
    }

    /// Opérations que ni l'historique ni le dictionnaire ne savent classer.
    fn unknown_rows(&self) -> impl Iterator<Item = &Row> {
        self.rows
            .iter()
            .filter(|r| !r.edited && r.suggestion.best().is_none_or(|b| b.confidence < 0.5))
    }

    fn start_ai(&mut self, ctx: &egui::Context, doc: &OpenDoc) {
        let ops = self
            .unknown_rows()
            .map(|r| &doc.document.accounts[r.account].operations[r.index]);
        let items = ai::items(&doc.document, ops);
        if items.is_empty() {
            self.message = Some(tr!("Rien de nouveau à demander à l'IA.").into());
            return;
        }
        let bodies: Vec<(serde_json::Value, Vec<ai::Item>)> = items
            .chunks(ai::BATCH)
            .map(|chunk| {
                (
                    ai::request_body(&self.ai.model, &doc.document, chunk),
                    chunk.to_vec(),
                )
            })
            .collect();
        let key = self.ai.api_key.trim().to_string();
        let fallbacks = ai::uses_fallbacks(&self.ai.model);
        let job: AiJob = Arc::new(Mutex::new(None));
        let slot = job.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = ask_ai(&key, fallbacks, &bodies);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
            ctx.request_repaint();
        });
        self.message = Some(trn!(
            items.len(),
            "Question posée à l'IA pour {} libellé…",
            "Question posée à l'IA pour {} libellés…"
        ));
        self.ai_job = Some(job);
    }

    fn poll_ai(&mut self, doc: &mut OpenDoc) {
        let Some(job) = &self.ai_job else {
            return;
        };
        let Some(result) = job.lock().ok().and_then(|mut g| g.take()) else {
            return;
        };
        self.ai_job = None;
        match result {
            Ok(answers) => {
                let found = answers
                    .iter()
                    .filter(|(_, label)| !label.is_empty())
                    .count();
                doc.store_ai_answers(&answers);
                self.message = Some(trn!(
                    found,
                    "L'IA a proposé une catégorie pour {0} libellé sur {1}.",
                    "L'IA a proposé une catégorie pour {0} libellés sur {1}.",
                    answers.len()
                ));
                self.stale = true;
            }
            Err(e) => self.message = Some(tr!("IA : {}", e)),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        self.poll_ai(doc);
        if self.stale {
            self.stale = false;
            self.recompute(doc);
        }
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.85, 900.0, 1500.0);
        let height = ui_kit::dialog_height(ctx, 0.85).max(560.0);
        crate::dialogs::modal(ctx, "categorize").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            crate::dialogs::header(
                ui,
                icon::MAGIC_WAND,
                pal.accent,
                tr!("Catégoriser les opérations"),
            );

            ui.horizontal(|ui| {
                let before = self.scope;
                ui_kit::segmented(
                    ui,
                    &mut self.scope,
                    &[
                        (
                            Scope::Visible,
                            icon::FUNNEL,
                            tr!("Lignes affichées"),
                            pal.accent,
                        ),
                        (Scope::File, icon::FILES, tr!("Tout le fichier"), pal.accent),
                    ],
                    34.0,
                    Some(380.0),
                );
                if self.scope != before {
                    self.stale = true;
                }
                ui.add_space(12.0);
                let sure = self
                    .rows
                    .iter()
                    .filter(|r| r.suggestion.confident().is_some())
                    .count();
                let none = self.rows.iter().filter(|r| r.suggestion.is_empty()).count();
                ui_kit::pill(ui, &tr!("{} sans catégorie", self.rows.len()), pal.accent);
                ui_kit::pill(ui, &trn!(sure, "{} sûre", "{} sûres"), pal.positive);
                ui_kit::pill(
                    ui,
                    &tr!("{} à confirmer", self.rows.len() - sure - none),
                    pal.warning,
                );
                ui_kit::pill(ui, &tr!("{} sans proposition", none), pal.muted);
            });
            ui.add_space(12.0);

            let list_height = height - 36.0 - 14.0 - 46.0 - 12.0 - 90.0;
            if self.rows.is_empty() {
                ui.allocate_ui(egui::vec2(width, list_height), |ui| {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(icon::CHECK_CIRCLE)
                                .size(36.0)
                                .color(pal.positive),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(tr!("Toutes ces opérations ont déjà une catégorie."))
                                .color(pal.fg_dark),
                        );
                    });
                });
            } else {
                self.list(ui, doc, width, list_height);
            }

            let count = self
                .rows
                .iter()
                .filter(|r| r.apply && r.choice.is_some())
                .count();
            crate::dialogs::footer(ui, |ui| {
                let label = format!(
                    "{}  {}",
                    icon::CHECK,
                    trn!(
                        count,
                        "Appliquer à {} opération",
                        "Appliquer à {} opérations"
                    )
                );
                if crate::dialogs::primary(ui, &label, count > 0).clicked() {
                    let changes: Vec<(usize, usize, u32)> = self
                        .rows
                        .iter()
                        .filter(|r| r.apply)
                        .filter_map(|r| r.choice.map(|c| (r.account, r.index, c)))
                        .collect();
                    let n = doc.set_categories(&changes, tr_mark!("catégoriser"));
                    outcome = Outcome::Done(trn!(
                        n,
                        "{} opération catégorisée",
                        "{} opérations catégorisées"
                    ));
                }
                if crate::dialogs::button(ui, tr!("Fermer")).clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    outcome = Outcome::Close;
                }
                if self.ai.enabled {
                    let unknown = self.unknown_rows().count();
                    let busy = self.ai_job.is_some();
                    let ready = !self.ai.api_key.trim().is_empty();
                    let text = if busy {
                        format!("{}  {}", icon::HOURGLASS, tr!("L'IA réfléchit…"))
                    } else {
                        format!(
                            "{}  {}",
                            icon::SPARKLE,
                            tr!("Demander à l'IA ({})", unknown)
                        )
                    };
                    let response = ui.add_enabled(
                        !busy && ready && unknown > 0,
                        egui::Button::new(text).min_size(egui::vec2(100.0, 36.0)),
                    );
                    let response = if ready {
                        response.on_hover_text(tr!(
                            "Envoie à Anthropic les libellés inconnus (sans numéros ni \
                             références), le sens et l'ordre de grandeur des montants et vos \
                             noms de catégories"
                        ))
                    } else {
                        response
                            .on_disabled_hover_text(tr!("Indiquez une clé d'API dans les options"))
                    };
                    if response.clicked() {
                        self.start_ai(ui.ctx(), doc);
                    }
                }
                if let Some(message) = &self.message {
                    ui.label(RichText::new(message).color(pal.muted));
                }
                // À gauche du pied (disposition de droite à gauche).
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if ui.button(tr!("Cocher les sûres")).clicked() {
                        for r in &mut self.rows {
                            r.apply = r.edited || r.suggestion.confident().is_some();
                        }
                    }
                    if ui.button(tr!("Tout cocher")).clicked() {
                        for r in &mut self.rows {
                            r.apply = r.choice.is_some();
                        }
                    }
                    if ui.button(tr!("Tout décocher")).clicked() {
                        self.rows.iter_mut().for_each(|r| r.apply = false);
                    }
                });
            });
        });
        outcome
    }

    fn list(&mut self, ui: &mut egui::Ui, doc: &OpenDoc, width: f32, height: f32) {
        let pal = palette(ui);
        let show_account = self.scope == Scope::File && doc.document.accounts.len() > 1;
        let (date_w, account_w, amount_w, category_w, confidence_w) = (
            96.0,
            if show_account { 150.0 } else { 0.0 },
            110.0,
            270.0,
            70.0,
        );
        let gap = ui.spacing().item_spacing.x;
        let gutter = crate::theme::SCROLLBAR_WIDTH + 6.0;
        let fixed = 28.0 + date_w + account_w + amount_w + category_w + confidence_w + gutter;
        let label_w = (width - fixed - 6.0 * gap).max(160.0);
        egui::ScrollArea::vertical()
            .id_salt("categorize_rows")
            .max_height(height)
            .min_scrolled_height(height)
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, self.rows.len(), |ui, range| {
                for k in range {
                    let row = &mut self.rows[k];
                    let op = &doc.document.accounts[row.account].operations[row.index];
                    let currency = &doc.document.accounts[row.account].currency;
                    ui.horizontal(|ui| {
                        ui.set_height(ROW_HEIGHT);
                        crate::theme::checkbox(ui, &mut row.apply, "");
                        ui_kit::cell(
                            ui,
                            date_w,
                            egui::Align::Min,
                            RichText::new(crate::settings::shared::date(op.date)),
                        );
                        if show_account {
                            ui_kit::cell(
                                ui,
                                account_w,
                                egui::Align::Min,
                                RichText::new(&doc.document.accounts[row.account].name)
                                    .color(pal.fg_dark),
                            );
                        }
                        let text = if op.details.is_empty() {
                            op.payee.clone()
                        } else {
                            format!("{}  ·  {}", op.payee, op.details)
                        };
                        ui_kit::cell_response(ui, label_w, egui::Align::Min, RichText::new(&text))
                            .on_hover_text(&text);
                        let color = if op.amount.is_negative() {
                            pal.negative
                        } else {
                            pal.positive
                        };
                        ui_kit::cell(
                            ui,
                            amount_w,
                            egui::Align::Max,
                            RichText::new(money(op.amount, currency)).color(color),
                        );
                        ui.add_space(6.0);
                        // Largeur fixe : les pastilles de confiance restent alignées.
                        let cell = egui::vec2(category_w, ROW_HEIGHT);
                        let layout = egui::Layout::left_to_right(egui::Align::Center);
                        let changed = ui
                            .allocate_ui_with_layout(cell, layout, |ui| {
                                ui.set_min_width(category_w);
                                ui.set_max_width(category_w);
                                ui.push_id(("categorize_choice", row.account, row.index), |ui| {
                                    crate::ui_kit::category_combo_fixed(
                                        ui,
                                        "choice",
                                        &doc.document,
                                        &doc.category_styles,
                                        &mut row.choice,
                                        category_w,
                                    )
                                })
                                .inner
                            })
                            .inner;
                        if changed {
                            row.edited = true;
                            row.apply = row.choice.is_some();
                        }
                        confidence_cell(ui, row, confidence_w);
                    });
                }
            });
    }
}

/// Pastille de confiance de la proposition retenue, avec son explication.
fn confidence_cell(ui: &mut egui::Ui, row: &Row, width: f32) {
    let pal = palette(ui);
    let candidate = row
        .choice
        .and_then(|c| row.suggestion.candidates.iter().find(|x| x.category == c));
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(egui::vec2(width, ROW_HEIGHT), layout, |ui| {
        ui.set_min_width(width);
        match candidate {
            _ if row.edited => {
                ui.label(RichText::new(icon::USER).color(pal.muted))
                    .on_hover_text(tr!("Choisie à la main"));
            }
            Some(c) => {
                let color = if c.confidence >= AUTO_THRESHOLD {
                    pal.positive
                } else if c.confidence >= 0.5 {
                    pal.warning
                } else {
                    pal.muted
                };
                let percent = (c.confidence * 100.0).round();
                let others: Vec<String> = row
                    .suggestion
                    .candidates
                    .iter()
                    .skip(1)
                    .map(|o| tr!("{} %", (o.confidence * 100.0).round()))
                    .collect();
                let mut hover = tr!("D'après : {} ({})", c.reason, c.source.label());
                if !others.is_empty() {
                    hover.push('\n');
                    hover.push_str(&tr!(
                        "Autres propositions dans la liste : {}",
                        others.join(", ")
                    ));
                }
                ui_kit::pill(ui, &tr!("{} %", percent), color);
                ui.interact(
                    ui.min_rect(),
                    ui.id().with("confidence"),
                    egui::Sense::hover(),
                )
                .on_hover_text(hover);
            }
            None => {
                ui.label(RichText::new("—").color(pal.muted));
            }
        }
    });
}

/// Appels à l'API, lot par lot (dans un fil à part).
fn ask_ai(
    key: &str,
    fallbacks: bool,
    bodies: &[(serde_json::Value, Vec<ai::Item>)],
) -> Result<Vec<(String, String)>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(180)))
        .build()
        .into();
    let mut answers = Vec::new();
    for (body, items) in bodies {
        let mut request = agent
            .post(ai::API_URL)
            .header("x-api-key", key)
            .header("anthropic-version", ai::API_VERSION)
            .header("content-type", "application/json");
        if fallbacks {
            request = request.header("anthropic-beta", ai::FALLBACK_BETA);
        }
        let mut response = request
            .send(body.to_string().as_str())
            .map_err(|e| tr!("connexion impossible : {}", e))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if status >= 400 {
            return Err(ai::error_message(status, &value));
        }
        answers.extend(ai::parse_response(&value, items)?);
    }
    Ok(answers)
}
