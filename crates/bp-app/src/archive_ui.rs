//! Archivage des opérations anciennes (menu Opérations), comme le plugin
//! « Archive » de l'ancienne version : une période compactée en écritures
//! de synthèse, consultables et restaurables (clic droit sur l'écriture).

use bp_core::archive::{self, Archives, Entry};
use bp_core::model::{Date, Tack};
use bp_core::{tr, tr_mark};
use egui::RichText;
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

/// Écriture de synthèse sous le curseur (une seule ligne sélectionnée).
pub fn selected_synthesis(doc: &OpenDoc) -> Option<u64> {
    let mut lines = doc.selection.iter();
    let (Some(&line), None) = (lines.next(), lines.next()) else {
        return None;
    };
    let uid = doc.current_account().operations.get(line)?.uid;
    Archives::load(&doc.document)
        .ok()?
        .entry(uid)
        .map(|e| e.synthesis)
}

/// Restaure les opérations de l'écriture de synthèse `synthesis` (annulable).
pub fn restore(doc: &mut OpenDoc, synthesis: u64) -> Result<String, String> {
    let accounts: Vec<usize> = (0..doc.document.accounts.len()).collect();
    let restored = doc
        .edit_archive(tr_mark!("restauration d'archive"), &accounts, |d| {
            archive::restore(d, synthesis)
        })
        .map_err(|e| e.to_string())?;
    match restored {
        Some((_, count)) => Ok(tr!("{} opérations restaurées", count)),
        None => Err(tr!("Archive introuvable").into()),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Account,
    All,
    Selection,
}

pub struct ArchiveForm {
    from: String,
    to: String,
    scope: Scope,
    only_reconciled: bool,
    error: Option<String>,
}

impl ArchiveForm {
    /// Une ligne sélectionnée donne la date de fin ; plusieurs lignes
    /// limitent l'archivage à elles, comme dans l'ancien plugin.
    pub fn new(doc: &OpenDoc) -> Self {
        let today = Date::today().to_naive();
        let end_of_last_year =
            chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&today) - 1, 12, 31)
                .map(Date::from_naive)
                .unwrap_or(Date::today());
        let mut to = end_of_last_year;
        let mut scope = Scope::Account;
        match doc.selection.len() {
            0 => {}
            1 => {
                let line = *doc.selection.iter().next().expect("une ligne");
                if let Some(op) = doc.current_account().operations.get(line) {
                    to = op.date;
                }
            }
            _ => scope = Scope::Selection,
        }
        Self {
            from: String::new(),
            to: to.to_string(),
            scope,
            only_reconciled: true,
            error: None,
        }
    }

    /// Lignes à archiver par compte, selon les réglages de la fenêtre.
    fn plan(&self, doc: &OpenDoc) -> Result<Vec<(usize, Vec<usize>)>, String> {
        if self.scope == Scope::Selection {
            let mut lines: Vec<usize> = doc.selection.iter().copied().collect();
            let ops = &doc.current_account().operations;
            lines.retain(|&i| !self.only_reconciled || ops[i].tack == Tack::Reconciled);
            return Ok(vec![(doc.account, lines)]);
        }
        let today = Date::today();
        let to = Date::parse_fr(&self.to, today).ok_or(tr!("Date de fin invalide"))?;
        let from = if self.from.trim().is_empty() {
            None
        } else {
            Some(Date::parse_fr(&self.from, today).ok_or(tr!("Date de début invalide"))?)
        };
        let accounts: Vec<usize> = match self.scope {
            Scope::All => doc
                .document
                .account_order
                .iter()
                .copied()
                .filter(|&i| !doc.document.accounts[i].disabled)
                .collect(),
            _ => vec![doc.account],
        };
        Ok(accounts
            .into_iter()
            .map(|a| {
                let lines =
                    archive::select(&doc.document.accounts[a], from, to, self.only_reconciled);
                (a, lines)
            })
            .filter(|(_, lines)| !lines.is_empty())
            .collect())
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.5, 640.0, 780.0);
        let plan = self.plan(doc);
        let mut submit = false;
        crate::dialogs::modal(ctx, "archive").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::ARCHIVE, pal.accent, tr!("Archiver des opérations"));
            ui_kit::subtitle(
                ui,
                tr!("Les opérations sont remplacées par une écriture de synthèse par statut de pointage : les soldes ne changent pas. Elles restent dans le fichier : un clic droit sur l'écriture de synthèse permet de les consulter ou de les restaurer."),
            );
            let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
            if self.scope == Scope::Selection {
                ui_kit::section(ui, width, icon::LIST_CHECKS, tr!("Lignes"), |ui| {
                    ui.label(tr!(
                        "Les {} lignes sélectionnées du compte « {} »",
                        doc.selection.len(),
                        doc.current_account().name
                    ));
                });
            } else {
                ui_kit::section(ui, width, icon::CALENDAR_DOTS, tr!("Période"), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr!("Du"));
                        crate::datepicker::date_field(ui, "archive_from", &mut self.from, 170.0, crate::datepicker::Placement::Popup)
                            .on_hover_text(tr!("Vide : depuis la première opération"));
                        ui.add_space(12.0);
                        ui.label(tr!("au"));
                        crate::datepicker::date_field(ui, "archive_to", &mut self.to, 170.0, crate::datepicker::Placement::Popup);
                    });
                    ui.label(RichText::new(tr!("Date de début vide : depuis la première opération.")).size(12.5).color(pal.muted));
                });
                ui_kit::section(ui, width, icon::BANK, tr!("Comptes"), |ui| {
                    let current = doc.current_account().name.clone();
                    let options = [
                        (Scope::Account, icon::BANK, current.as_str(), pal.accent),
                        (Scope::All, icon::STACK, tr!("Tous les comptes"), pal.accent),
                    ];
                    ui_kit::segmented(ui, &mut self.scope, &options, 34.0, Some(inner));
                });
            }
            ui_kit::section(ui, width, icon::CHECKS, tr!("Statut"), |ui| {
                let options = [
                    (true, icon::SEAL_CHECK, tr!("Rapprochées seulement"), pal.accent),
                    (false, icon::LIST, tr!("Toutes les opérations"), pal.accent),
                ];
                ui_kit::segmented(ui, &mut self.only_reconciled, &options, 34.0, Some(inner));
            });
            let (count, groups, accounts) = match &plan {
                Ok(plan) => (
                    plan.iter().map(|(_, l)| l.len()).sum::<usize>(),
                    plan.iter()
                        .map(|(a, l)| archive::preview(&doc.document.accounts[*a], l).len())
                        .sum::<usize>(),
                    plan.len(),
                ),
                Err(_) => (0, 0, 0),
            };
            let summary = match &plan {
                Err(e) => RichText::new(format!("{}  {e}", icon::WARNING)).color(pal.negative),
                Ok(_) if count == 0 => RichText::new(tr!("Aucune opération à archiver.")).color(pal.muted),
                Ok(_) if accounts > 1 => RichText::new(tr!(
                    "{} opérations dans {} comptes seront remplacées par {} écritures de synthèse.",
                    count,
                    accounts,
                    groups
                )),
                Ok(_) if groups == 1 => RichText::new(if bp_core::i18n::singular(count as i64) {
                    tr!("1 opération sera remplacée par 1 écriture de synthèse.").to_string()
                } else {
                    tr!("{} opérations seront remplacées par 1 écriture de synthèse.", count)
                }),
                Ok(_) => RichText::new(tr!(
                    "{} opérations seront remplacées par {} écritures de synthèse.",
                    count,
                    groups
                )),
            };
            ui.add_space(4.0);
            ui.label(summary.strong());
            if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.negative));
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, &format!("{}  {}", icon::ARCHIVE, tr!("Archiver")), count > 0).clicked() {
                    submit = true;
                }
                if crate::dialogs::button(ui, tr!("Annuler")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    outcome = Outcome::Close;
                }
            });
        });
        if submit && let Ok(plan) = plan {
            let accounts: Vec<usize> = plan.iter().map(|(a, _)| *a).collect();
            let result = doc.edit_archive(tr_mark!("archivage"), &accounts, |d| {
                let mut count = 0;
                for (a, lines) in &plan {
                    archive::archive(d, *a, lines)?;
                    count += lines.len();
                }
                Ok::<usize, bp_core::feature_store::FeatureError>(count)
            });
            match result {
                Ok(count) => outcome = Outcome::Done(tr!("{} opérations archivées", count)),
                Err(e) => self.error = Some(e.to_string()),
            }
        }
        outcome
    }
}

/// Consultation des opérations d'une écriture de synthèse, avec recherche ;
/// la recherche peut s'étendre à toutes les archives du compte.
pub struct ArchiveView {
    synthesis: u64,
    search: String,
    whole_account: bool,
}

impl ArchiveView {
    pub fn new(synthesis: u64) -> Self {
        Self {
            synthesis,
            search: String::new(),
            whole_account: false,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let archives = Archives::load(&doc.document).unwrap_or_default();
        let Some(entry) = archives.entry(self.synthesis).cloned() else {
            return Outcome::Close;
        };
        let width = ui_kit::dialog_width(ctx, 0.7, 760.0, 1100.0);
        let height = ui_kit::dialog_height(ctx, 0.5);
        let currency = doc
            .document
            .accounts
            .iter()
            .find(|a| a.uid == entry.account)
            .map(|a| a.currency.clone())
            .unwrap_or_default();
        let entries: Vec<&Entry> = if self.whole_account {
            archives
                .entries
                .iter()
                .filter(|e| e.account == entry.account)
                .collect()
        } else {
            vec![&entry]
        };
        let needle = self.search.trim().to_lowercase();
        let mut rows: Vec<&bp_core::model::Operation> = entries
            .iter()
            .flat_map(|e| &e.operations)
            .filter(|op| {
                needle.is_empty()
                    || op.payee.to_lowercase().contains(&needle)
                    || op.details.to_lowercase().contains(&needle)
            })
            .collect();
        rows.sort_by_key(|op| (op.date, op.uid));
        let mut restore = false;
        crate::dialogs::modal(ctx, "archive_view").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::ARCHIVE, pal.accent, tr!("Opérations archivées"));
            ui_kit::subtitle(
                ui,
                tr!(
                    "{} opérations du {} au {}",
                    entry.operations.len(),
                    entry.from.to_string(),
                    entry.to.to_string()
                ),
            );
            ui.horizontal(|ui| {
                ui.add(
                    crate::theme::field(&mut self.search)
                        .hint_text(tr!("Rechercher un tiers ou un détail"))
                        .desired_width(320.0),
                );
                ui.add_space(12.0);
                ui.checkbox(
                    &mut self.whole_account,
                    tr!("Dans toutes les archives du compte"),
                );
            });
            ui.add_space(8.0);
            let total: bp_core::model::Amount = rows.iter().map(|op| op.amount).sum();
            use egui_extras::{Column, TableBuilder};
            let right = egui::Layout::right_to_left(egui::Align::Center);
            TableBuilder::new(ui)
                .striped(true)
                .max_scroll_height(height)
                .auto_shrink([false, true])
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::exact(96.0))
                .column(Column::remainder().at_least(160.0).clip(true))
                .column(Column::remainder().at_least(160.0).clip(true))
                .column(Column::exact(130.0))
                .header(26.0, |mut header| {
                    for title in [tr!("Date"), tr!("Tiers"), tr!("Détail")] {
                        header.col(|ui| {
                            ui.label(RichText::new(title).color(pal.muted));
                        });
                    }
                    header.col(|ui| {
                        ui.with_layout(right, |ui| {
                            ui.label(RichText::new(tr!("Montant")).color(pal.muted))
                        });
                    });
                })
                .body(|body| {
                    body.rows(28.0, rows.len(), |mut row| {
                        let op = rows[row.index()];
                        row.col(|ui| {
                            ui.label(op.date.to_string());
                        });
                        row.col(|ui| {
                            ui.add(egui::Label::new(&op.payee).truncate());
                        });
                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(RichText::new(&op.details).color(pal.muted))
                                    .truncate(),
                            );
                        });
                        row.col(|ui| {
                            let color = if op.amount.is_negative() {
                                pal.negative
                            } else {
                                pal.positive
                            };
                            ui.with_layout(right, |ui| {
                                ui.label(RichText::new(money(op.amount, &currency)).color(color))
                            });
                        });
                    });
                });
            ui.add_space(6.0);
            ui.label(
                RichText::new(tr!(
                    "{} lignes · total {}",
                    rows.len(),
                    money(total, &currency)
                ))
                .color(pal.muted),
            );
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(
                    ui,
                    &format!(
                        "{}  {}",
                        icon::ARROW_COUNTER_CLOCKWISE,
                        tr!("Restaurer ces opérations")
                    ),
                    true,
                )
                .clicked()
                {
                    restore = true;
                }
                if crate::dialogs::button(ui, tr!("Fermer")).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    outcome = Outcome::Close;
                }
            });
        });
        if restore {
            outcome = match self::restore(doc, self.synthesis) {
                Ok(message) => Outcome::Done(message),
                Err(e) => Outcome::Done(e),
            };
        }
        outcome
    }
}
