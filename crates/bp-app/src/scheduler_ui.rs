//! Échéancier (Ctrl+E) et solde prévisionnel. Les échéances sont
//! enregistrées dans le fichier de comptes à chaque modification.

use bp_core::model::{Amount, Date, Mode, StandardMode};
use bp_core::scheduler::{self, ForecastKind, Period, PeriodUnit, Schedule, Selection};
use bp_core::{tr, trn};
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

const LABEL_WIDTH: f32 = 170.0;
/// Occurrences proposées sous « Prochaines dates », sur 24 mois au plus.
const UPCOMING_DATES: usize = 24;

/// Saisie du montant d'une seule occurrence. Le champ vide vaut « montant
/// normal » ; le montant est enregistré à la frappe, comme le reste de
/// l'échéance.
struct OccurrenceEdit {
    due: Date,
    amount: String,
    /// Donner le focus au champ à la première image, pas à chaque image.
    focus: bool,
    /// Images restantes pour amener le champ à l'écran : la zone de
    /// défilement ne connaît sa nouvelle hauteur qu'à l'image suivante.
    reveal: u8,
}

impl OccurrenceEdit {
    fn new(schedule: &Schedule, due: Date) -> Self {
        Self {
            due,
            amount: schedule
                .exception_on(due)
                .map(plain_amount)
                .unwrap_or_default(),
            focus: true,
            reveal: 3,
        }
    }

    /// Renvoie vrai si l'échéance a changé.
    fn show(&mut self, ui: &mut egui::Ui, schedule: &mut Schedule, currency: &str) -> bool {
        let pal = palette(ui);
        let mut changed = false;
        egui::Frame::new()
            .fill(pal.accent.gamma_multiply(0.08))
            .stroke(egui::Stroke::new(1.0, pal.accent.gamma_multiply(0.3)))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(tr!("Montant de l'échéance du {}", self.due))
                            .color(pal.fg_dark),
                    );
                    let r = ui.add(
                        crate::theme::field(&mut self.amount)
                            .hint_text(plain_amount(schedule.amount))
                            .desired_width(120.0),
                    );
                    if std::mem::take(&mut self.focus) {
                        r.request_focus();
                    }
                    if self.reveal > 0 {
                        self.reveal -= 1;
                        r.scroll_to_me(Some(egui::Align::Center));
                        ui.ctx().request_repaint();
                    }
                    if r.changed() {
                        let text = crate::budget_ui::amount_input(&self.amount);
                        if text.trim().is_empty() {
                            schedule.set_exception(self.due, None);
                            changed = true;
                        } else if let Some(amount) = Amount::parse(&text) {
                            schedule.set_exception(self.due, Some(amount));
                            changed = true;
                        }
                    }
                    ui.label(
                        RichText::new(tr!(
                            "montant normal : {}",
                            money(schedule.amount.abs(), currency)
                        ))
                        .color(pal.muted),
                    );
                    if schedule.exception_on(self.due).is_some()
                        && ui.button(tr!("Revenir au montant normal")).clicked()
                    {
                        schedule.set_exception(self.due, None);
                        self.amount.clear();
                        changed = true;
                    }
                });
            });
        changed
    }
}

/// Montant sans signe ni devise, tel qu'on le saisit.
fn plain_amount(amount: Amount) -> String {
    amount.abs().format(2, bp_core::i18n::decimal_separator())
}

pub struct SchedulerForm {
    schedules: Vec<Schedule>,
    selected: Option<usize>,
    /// Tampons de saisie de l'échéance sélectionnée.
    amount: String,
    next_date: String,
    end_date: String,
    /// Insertion : jusqu'à quelle date, et quelles échéances.
    until: String,
    only_manual: bool,
    message: Option<String>,
    warnings: Vec<String>,
    confirm_delete: bool,
    /// Nombre d'échéances du tout premier échéancier, reprenables.
    oldest: usize,
    /// Occurrence de l'échéance sélectionnée dont on change le montant.
    occurrence: Option<OccurrenceEdit>,
    /// Faire défiler le détail jusqu'aux prochaines dates, une fois.
    reveal_dates: bool,
    /// Échéances telles qu'à l'ouverture (ou après la dernière insertion),
    /// pour « Annuler les modifications ».
    initial: Vec<Schedule>,
    /// Étape d'annulation prise à la première modification : Ctrl+Z, une
    /// fois la fenêtre fermée, défait les modifications de l'échéancier.
    undo_tag: Option<u64>,
}

/// Index réel du compte d'uid donné.
fn account_index(doc: &OpenDoc, uid: &str) -> Option<usize> {
    doc.document.accounts.iter().position(|a| a.uid == uid)
}

impl SchedulerForm {
    pub fn new(doc: &OpenDoc, select: Option<usize>) -> Self {
        let (schedules, warnings) = match scheduler::load(&doc.document) {
            Ok(loaded) => (loaded.schedules, loaded.warnings),
            Err(e) => (Vec::new(), vec![tr!("Échéances illisibles : {}", e)]),
        };
        let mut form = Self {
            schedules,
            selected: None,
            amount: String::new(),
            next_date: String::new(),
            end_date: String::new(),
            until: Date::today().to_string(),
            only_manual: false,
            message: None,
            warnings,
            confirm_delete: false,
            oldest: scheduler::oldest_schedules(&doc.document).map_or(0, |(s, _)| s.len()),
            occurrence: None,
            reveal_dates: false,
            initial: Vec::new(),
            undo_tag: None,
        };
        form.initial = form.schedules.clone();
        let first = select.or((!form.schedules.is_empty()).then_some(0));
        form.select(first);
        form
    }

    fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|&i| i < self.schedules.len());
        self.confirm_delete = false;
        self.occurrence = None;
        if let Some(s) = self.selected.map(|i| &self.schedules[i]) {
            self.amount = s.amount.abs().format(2, bp_core::i18n::decimal_separator());
            self.next_date = s.next_date.to_string();
            self.end_date = s.end_date.map(|d| d.to_string()).unwrap_or_default();
        }
    }

    fn save(&mut self, doc: &mut OpenDoc) {
        if self
            .undo_tag
            .is_none_or(|tag| doc.last_undo_tag() != Some(tag))
        {
            self.undo_tag = Some(doc.snapshot_features(bp_core::tr_mark!("échéancier")));
        }
        if scheduler::save(&mut doc.document, &self.schedules).is_ok() {
            doc.dirty = true;
        }
    }

    /// Remet l'échéancier tel qu'à l'ouverture de la fenêtre.
    fn revert(&mut self, doc: &mut OpenDoc) {
        if self.undo_tag.is_some() && doc.last_undo_tag() == self.undo_tag {
            doc.undo();
        } else if scheduler::save(&mut doc.document, &self.initial).is_ok() {
            doc.dirty = true;
        }
        self.undo_tag = None;
        self.schedules = self.initial.clone();
        let selected = self
            .selected
            .filter(|&i| i < self.schedules.len())
            .or((!self.schedules.is_empty()).then_some(0));
        self.select(selected);
        self.message = Some(tr!("Échéancier remis comme à l'ouverture de la fenêtre.").into());
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.8, 900.0, 1500.0);
        let height = ui_kit::dialog_height(ctx, 0.8).max(560.0);
        crate::dialogs::modal(ctx, "scheduler").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::CALENDAR_DOTS, pal.accent, tr!("Échéancier"));
            let body = height - 36.0 - 14.0 - 120.0;
            let gap = 22.0;
            let list_width = (width * 0.36).clamp(320.0, 520.0);
            let detail_width = width - list_width - gap;
            let mut changed = false;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.vertical(|ui| {
                    ui.set_width(list_width);
                    ui.set_height(body);
                    changed |= self.list(ui, doc, list_width, body);
                });
                ui.vertical(|ui| {
                    ui.set_width(detail_width);
                    egui::ScrollArea::vertical()
                        .id_salt("scheduler_detail")
                        .max_height(body)
                        .min_scrolled_height(body)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let w = detail_width - 16.0;
                            ui.set_max_width(w);
                            changed |= self.detail(ui, doc, w);
                        });
                });
            });
            if changed {
                self.save(doc);
            }
            self.footer(ui, doc, &mut outcome);
        });
        outcome
    }

    fn list(&mut self, ui: &mut egui::Ui, doc: &OpenDoc, width: f32, height: f32) -> bool {
        let pal = palette(ui);
        let mut changed = false;
        let mut clicked = None;
        egui::Frame::new()
            .fill(pal.bg)
            .stroke(egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(width - 18.0);
                ui.set_height(height - 18.0);
                egui::ScrollArea::vertical()
                    .id_salt("scheduler_list")
                    .max_height(height - 18.0 - 54.0)
                    .min_scrolled_height(height - 18.0 - 54.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.schedules.is_empty() {
                            ui.add_space(30.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(icon::CALENDAR_DOTS).size(34.0).color(pal.muted));
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(tr!("Aucune échéance. Créez-en une, ou ajoutez une opération récurrente depuis la grille (clic droit)."))
                                        .color(pal.muted),
                                );
                            });
                        }
                        for (k, s) in self.schedules.iter().enumerate() {
                            let account = account_index(doc, &s.account)
                                .map_or(tr!("compte inexistant").to_string(), |i| doc.document.accounts[i].name.clone());
                            let selected = self.selected == Some(k);
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::click());
                            if selected {
                                ui.painter().rect_filled(rect, 9.0, pal.accent.gamma_multiply(0.14));
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(rect.min + egui::vec2(0.0, 12.0), egui::vec2(3.0, 28.0)),
                                    2.0,
                                    pal.accent,
                                );
                            } else if response.hovered() {
                                ui.painter().rect_filled(rect, 9.0, pal.bg_highlight.gamma_multiply(0.6));
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &s.payee)
                            });
                            let title_color = if s.is_finished() { pal.muted } else { pal.fg };
                            let left = rect.left() + 14.0;
                            ui.painter().text(
                                egui::pos2(left, rect.top() + 16.0),
                                egui::Align2::LEFT_CENTER,
                                if s.payee.is_empty() { tr!("(sans tiers)") } else { &s.payee },
                                egui::FontId::new(14.0, crate::theme::medium()),
                                title_color,
                            );
                            let mut sub = format!("{} · {} · {}", s.next_date, s.interval_label(), account);
                            if s.transfer.is_some() {
                                sub = tr!("{} · transfert", sub);
                            }
                            if s.is_finished() {
                                sub = tr!("terminée · {}", sub);
                            }
                            ui.painter().text(
                                egui::pos2(left, rect.bottom() - 14.0),
                                egui::Align2::LEFT_CENTER,
                                sub,
                                egui::FontId::new(12.0, egui::FontFamily::Proportional),
                                pal.muted,
                            );
                            let color = if s.amount.is_negative() { pal.negative } else { pal.positive };
                            ui.painter().text(
                                egui::pos2(rect.right() - 14.0, rect.top() + 16.0),
                                egui::Align2::RIGHT_CENTER,
                                money(
                                    s.amount,
                                    account_index(doc, &s.account)
                                        .map_or(doc.main_currency(), |i| &doc.document.accounts[i].currency),
                                ),
                                egui::FontId::new(13.5, crate::theme::medium()),
                                color,
                            );
                            if s.auto_insert {
                                ui.painter().text(
                                    egui::pos2(rect.right() - 14.0, rect.bottom() - 14.0),
                                    egui::Align2::RIGHT_CENTER,
                                    format!("{}  auto", icon::LIGHTNING),
                                    egui::FontId::new(12.0, egui::FontFamily::Proportional),
                                    pal.accent,
                                );
                            }
                            if response.clicked() {
                                clicked = Some(k);
                            }
                        }
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(format!("{}  {}", icon::PLUS, tr!("Nouvelle échéance"))).min_size(egui::vec2(0.0, 34.0))).clicked() {
                        let account = doc.current_account();
                        let schedule = Schedule {
                            account: account.uid.clone(),
                            next_date: Date::today(),
                            mode: Mode::Standard(StandardMode::Prelevement),
                            ..Default::default()
                        };
                        self.schedules.push(schedule);
                        clicked = Some(self.schedules.len() - 1);
                        changed = true;
                    }
                    if let Some(i) = self.selected {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new(RichText::new(icon::TRASH).color(pal.negative)).min_size(egui::vec2(34.0, 34.0)))
                                .on_hover_text(tr!("Supprimer l'échéance"))
                                .clicked()
                            {
                                self.confirm_delete = true;
                            }
                            if ui
                                .add(egui::Button::new(icon::COPY).min_size(egui::vec2(34.0, 34.0)))
                                .on_hover_text(tr!("Dupliquer"))
                                .clicked()
                            {
                                // Les montants particuliers portent sur les
                                // dates de l'échéance d'origine.
                                let copy = Schedule {
                                    exceptions: Vec::new(),
                                    ..self.schedules[i].clone()
                                };
                                self.schedules.push(copy);
                                clicked = Some(self.schedules.len() - 1);
                                changed = true;
                            }
                        });
                    }
                });
            });
        if let Some(k) = clicked {
            self.select(Some(k));
        }
        changed
    }

    fn detail(&mut self, ui: &mut egui::Ui, doc: &OpenDoc, width: f32) -> bool {
        let pal = palette(ui);
        let mut changed = false;
        let Some(i) = self.selected else {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(tr!("Sélectionnez une échéance dans la liste.")).color(pal.muted),
                );
            });
            return false;
        };
        if self.confirm_delete {
            let mut done = None;
            egui::Frame::new()
                .fill(pal.negative.gamma_multiply(0.10))
                .stroke(egui::Stroke::new(1.0, pal.negative.gamma_multiply(0.4)))
                .corner_radius(12)
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.set_width(width - 34.0);
                    ui.label(
                        RichText::new(format!(
                            "{}  {}",
                            icon::WARNING,
                            tr!(
                                "Supprimer l'échéance « {} » ? Les opérations déjà insérées restent.",
                                self.schedules[i].payee
                            )
                        ))
                        .color(pal.negative),
                    );
                    ui.add_space(10.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 36.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            if crate::dialogs::danger(ui, tr!("Supprimer")).clicked() {
                                done = Some(true);
                            }
                            if crate::dialogs::button(ui, tr!("Annuler")).clicked() {
                                done = Some(false);
                            }
                        },
                    );
                });
            ui.add_space(14.0);
            match done {
                Some(true) => {
                    self.schedules.remove(i);
                    let next =
                        (!self.schedules.is_empty()).then(|| i.min(self.schedules.len() - 1));
                    self.select(next);
                    return true;
                }
                Some(false) => self.confirm_delete = false,
                None => {}
            }
        }
        let document = &doc.document;
        let styles = &doc.category_styles;
        let full = width - 2.0 * ui_kit::CARD_PADDING - 2.0 - LABEL_WIDTH - 8.0;
        let s = &mut self.schedules[i];

        ui_kit::section(ui, width, icon::RECEIPT, tr!("Opération"), |ui| {
            ui_kit::row(ui, tr!("Tiers"), LABEL_WIDTH, |ui| {
                changed |= ui
                    .add(crate::theme::field(&mut s.payee).desired_width(full))
                    .changed();
            });
            ui.add_space(8.0);
            ui_kit::row(ui, tr!("Détail"), LABEL_WIDTH, |ui| {
                changed |= ui
                    .add(crate::theme::field(&mut s.details).desired_width(full))
                    .changed();
            });
            ui.add_space(8.0);
            ui_kit::row(ui, tr!("Mode de paiement"), LABEL_WIDTH, |ui| {
                if ui_kit::mode_combo(ui, "sched_mode", document, &mut s.mode, full) {
                    changed = true;
                }
            });
            ui.add_space(8.0);
            ui_kit::row(ui, tr!("Catégorie"), LABEL_WIDTH, |ui| {
                if ui_kit::category_combo(ui, "sched_cat", document, styles, &mut s.category, full)
                {
                    changed = true;
                }
            });
            ui.add_space(8.0);
            ui_kit::row(ui, tr!("Montant"), LABEL_WIDTH, |ui| {
                let r = ui.add(
                    crate::theme::field(&mut self.amount)
                        .hint_text(tr!("0,00"))
                        .desired_width(160.0),
                );
                let debit = bp_core::edit::mode_is_debit(document, &s.mode);
                let sign = if debit {
                    tr!("dépense (selon le mode)")
                } else {
                    tr!("recette (selon le mode)")
                };
                ui.label(RichText::new(sign).color(pal.muted));
                if (r.changed() || changed)
                    && let Some(a) = Amount::parse(&crate::budget_ui::amount_input(&self.amount))
                {
                    let a = if debit { -a.abs() } else { a.abs() };
                    if a != s.amount {
                        s.amount = a;
                        changed = true;
                    }
                }
            });
            ui.add_space(4.0);
            ui_kit::row(ui, "", LABEL_WIDTH, |ui| {
                if ui.link(tr!("Changer une seule échéance…")).clicked() {
                    self.reveal_dates = true;
                }
            });
        });

        ui_kit::section(ui, width, icon::ARROWS_CLOCKWISE, tr!("Récurrence"), |ui| {
            ui_kit::row(ui, tr!("Prochaine échéance"), LABEL_WIDTH, |ui| {
                let r = crate::datepicker::date_field(
                    ui,
                    "sched_next",
                    &mut self.next_date,
                    200.0,
                    crate::datepicker::Placement::Popup,
                );
                if r.changed()
                    && let Some(d) = Date::parse_fr(&self.next_date, Date::today())
                {
                    s.next_date = d;
                    changed = true;
                }
                if ui
                    .button(icon::CARET_LEFT)
                    .on_hover_text(tr!("Échéance précédente"))
                    .clicked()
                    && s.rewind()
                {
                    self.next_date = s.next_date.to_string();
                    changed = true;
                }
                if ui
                    .button(icon::CARET_RIGHT)
                    .on_hover_text(tr!("Échéance suivante"))
                    .clicked()
                    && s.advance()
                {
                    self.next_date = s.next_date.to_string();
                    changed = true;
                }
            });
            ui.add_space(8.0);
            ui_kit::row(ui, tr!("Tous les"), LABEL_WIDTH, |ui| {
                let mut every = s.period.every.max(1);
                if ui_kit::stepper(ui, "sched_every", &mut every, 1..=99, None) {
                    s.period.every = every;
                    changed = true;
                }
                let mut unit = s.period.unit;
                if ui_kit::segmented(
                    ui,
                    &mut unit,
                    &[
                        (PeriodUnit::Days, "", tr!("jours"), pal.accent),
                        (PeriodUnit::Weeks, "", tr!("semaines"), pal.accent),
                        (PeriodUnit::Months, "", tr!("mois"), pal.accent),
                        (PeriodUnit::Years, "", tr!("ans"), pal.accent),
                    ],
                    32.0,
                    Some((full - 80.0).min(380.0)),
                ) {
                    s.period = Period {
                        every: s.period.every.max(1),
                        unit,
                    };
                    changed = true;
                }
            });
            ui.add_space(8.0);
            ui_kit::row(ui, tr!("Dernière échéance"), LABEL_WIDTH, |ui| {
                let r = crate::datepicker::date_field(
                    ui,
                    "sched_end",
                    &mut self.end_date,
                    200.0,
                    crate::datepicker::Placement::Popup,
                );
                if r.changed() {
                    let end = Date::parse_fr(&self.end_date, Date::today());
                    if self.end_date.trim().is_empty() || end.is_some() {
                        s.end_date = end;
                        changed = true;
                    }
                }
                ui.label(RichText::new(tr!("vide = sans fin")).color(pal.muted));
            });
            ui.add_space(8.0);
            ui_kit::row(ui, tr!("Si jour non ouvré"), LABEL_WIDTH, |ui| {
                changed |=
                    crate::theme::checkbox(ui, &mut s.postpone.saturday, tr!("samedi")).changed();
                changed |=
                    crate::theme::checkbox(ui, &mut s.postpone.sunday, tr!("dimanche")).changed();
                changed |=
                    crate::theme::checkbox(ui, &mut s.postpone.holiday, tr!("férié")).changed();
                ui.label(RichText::new(tr!("→ jour ouvré suivant")).color(pal.muted));
            });
            ui.add_space(8.0);
            changed |= crate::theme::checkbox(
                ui,
                &mut s.auto_insert,
                tr!("Insérer automatiquement à l'ouverture du fichier quand l'échéance est passée"),
            )
            .changed();
        });

        ui_kit::section(ui, width, icon::BANK, tr!("Comptes"), |ui| {
            ui_kit::row(ui, tr!("Compte"), LABEL_WIDTH, |ui| {
                let current = account_index(doc, &s.account);
                let mut chosen = current;
                egui::ComboBox::from_id_salt("sched_account")
                    .selected_text(chosen.map_or(tr!("Compte inexistant").to_string(), |k| {
                        document.accounts[k].name.clone()
                    }))
                    .width(full - 12.0)
                    .show_ui(ui, |ui| {
                        for &k in &document.account_order {
                            ui.selectable_value(&mut chosen, Some(k), &document.accounts[k].name);
                        }
                    });
                if chosen != current
                    && let Some(k) = chosen
                {
                    s.account = document.accounts[k].uid.clone();
                    changed = true;
                }
            });
            ui.add_space(8.0);
            ui_kit::row(ui, tr!("Transfert vers"), LABEL_WIDTH, |ui| {
                let current = s.transfer.as_ref().and_then(|uid| account_index(doc, uid));
                let mut chosen = current;
                egui::ComboBox::from_id_salt("sched_transfer")
                    .selected_text(
                        chosen.map_or(tr!("Aucun (opération simple)").to_string(), |k| {
                            document.accounts[k].name.clone()
                        }),
                    )
                    .width(full - 12.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut chosen, None, tr!("Aucun (opération simple)"));
                        for &k in &document.account_order {
                            ui.selectable_value(&mut chosen, Some(k), &document.accounts[k].name);
                        }
                    });
                if chosen != current {
                    s.transfer = chosen.map(|k| document.accounts[k].uid.clone());
                    changed = true;
                }
            });
        });

        // Prochaines dates, pour vérifier la récurrence d'un coup d'œil et
        // changer le montant d'une seule d'entre elles.
        let currency = account_index(doc, &s.account)
            .map_or(doc.main_currency(), |k| &document.accounts[k].currency);
        let horizon = scheduler::add_months(s.next_date, 24, None).unwrap_or(s.next_date);
        let mut upcoming = s.occurrences(horizon);
        upcoming.truncate(UPCOMING_DATES);
        if self
            .occurrence
            .as_ref()
            .is_some_and(|edit| !s.is_occurrence(edit.due))
        {
            self.occurrence = None;
        }
        let mut clicked = None;
        if std::mem::take(&mut self.reveal_dates) {
            ui.scroll_to_cursor(Some(egui::Align::TOP));
        }
        ui_kit::section(
            ui,
            width,
            icon::CALENDAR_CHECK,
            tr!("Prochaines dates"),
            |ui| {
                if upcoming.is_empty() {
                    ui.label(RichText::new(tr!("Aucune (échéance terminée).")).color(pal.muted));
                    return;
                }
                ui.label(
                    RichText::new(tr!(
                        "Cliquez sur une date pour changer le montant de cette échéance seulement."
                    ))
                    .color(pal.muted),
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for o in &upcoming {
                        let mut text = if o.date != o.due {
                            tr!("{} (reportée du {})", o.date, o.due)
                        } else {
                            o.date.to_string()
                        };
                        let exception = s.exception_on(o.due);
                        if let Some(amount) = exception {
                            text = format!("{text} · {}", money(amount, currency));
                        }
                        let color = if exception.is_some() {
                            pal.warning
                        } else {
                            pal.accent
                        };
                        let selected = self.occurrence.as_ref().is_some_and(|e| e.due == o.due);
                        if ui_kit::pill_button(ui, &text, color, selected).clicked() {
                            clicked = Some(o.due);
                        }
                    }
                });
                if let Some(edit) = &mut self.occurrence {
                    ui.add_space(10.0);
                    changed |= edit.show(ui, s, currency);
                }
            },
        );
        if let Some(due) = clicked {
            self.toggle_occurrence(i, due);
        }
        changed |= self.exceptions(ui, i, currency, width);
        changed
    }

    /// Ouvre la saisie du montant d'une occurrence, ou la referme.
    fn toggle_occurrence(&mut self, index: usize, due: Date) {
        let open = self.occurrence.as_ref().is_some_and(|e| e.due == due);
        self.occurrence = (!open).then(|| OccurrenceEdit::new(&self.schedules[index], due));
    }

    /// Montants particuliers à venir, y compris ceux hors de la liste des
    /// prochaines dates et ceux qui n'ont plus d'effet (récurrence modifiée).
    fn exceptions(&mut self, ui: &mut egui::Ui, index: usize, currency: &str, width: f32) -> bool {
        let pal = palette(ui);
        let s = &self.schedules[index];
        let rows: Vec<_> = s
            .upcoming_exceptions()
            .map(|e| (e.due, e.amount, s.is_occurrence(e.due)))
            .collect();
        if rows.is_empty() {
            return false;
        }
        let mut remove = None;
        let mut edit = None;
        ui_kit::section(
            ui,
            width,
            icon::PENCIL_SIMPLE,
            tr!("Montants ponctuels"),
            |ui| {
                for (due, amount, active) in rows {
                    ui.horizontal(|ui| {
                        ui.set_min_height(28.0);
                        ui_kit::cell(ui, 96.0, egui::Align::Min, RichText::new(due.to_string()));
                        ui_kit::cell(
                            ui,
                            120.0,
                            egui::Align::Max,
                            RichText::new(money(amount, currency)),
                        );
                        if active {
                            if ui
                                .button(icon::PENCIL_SIMPLE)
                                .on_hover_text(tr!("Modifier ce montant"))
                                .clicked()
                            {
                                edit = Some(due);
                            }
                        } else {
                            ui.label(
                                RichText::new(format!(
                                    "{}  {}",
                                    icon::WARNING,
                                    tr!("Sans effet : aucune échéance à cette date.")
                                ))
                                .color(pal.warning),
                            );
                        }
                        if ui
                            .button(RichText::new(icon::TRASH).color(pal.negative))
                            .on_hover_text(tr!("Supprimer ce montant"))
                            .clicked()
                        {
                            remove = Some(due);
                        }
                    });
                }
            },
        );
        if let Some(due) = edit {
            self.toggle_occurrence(index, due);
        }
        if let Some(due) = remove {
            self.schedules[index].set_exception(due, None);
            if self.occurrence.as_ref().is_some_and(|e| e.due == due) {
                self.occurrence = None;
            }
            return true;
        }
        false
    }

    fn footer(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, outcome: &mut Outcome) {
        let pal = palette(ui);
        let mut revert = false;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr!("Insérer les échéances jusqu'au")).color(pal.fg_dark));
            crate::datepicker::date_field(
                ui,
                "sched_until",
                &mut self.until,
                180.0,
                crate::datepicker::Placement::Popup,
            );
            crate::theme::checkbox(
                ui,
                &mut self.only_manual,
                tr!("seulement celles sans insertion automatique"),
            );
            let until = Date::parse_fr(&self.until, Date::today());
            let pending: usize = until.map_or(0, |u| {
                self.schedules
                    .iter()
                    .filter(|s| !self.only_manual || !s.auto_insert)
                    .map(|s| s.occurrences(u).len())
                    .sum()
            });
            let label = format!(
                "{}  {}",
                icon::DOWNLOAD_SIMPLE,
                tr!("Insérer ({})", pending)
            );
            if ui
                .add_enabled(
                    pending > 0,
                    egui::Button::new(label).min_size(egui::vec2(0.0, 34.0)),
                )
                .clicked()
                && let Some(until) = until
            {
                let selection = if self.only_manual {
                    Selection::NotAutomatic
                } else {
                    Selection::All
                };
                let mut schedules = std::mem::take(&mut self.schedules);
                let report = doc.edit_document(|d| {
                    let report = scheduler::insert_due(d, &mut schedules, until, selection, false);
                    let _ = scheduler::save(d, &schedules);
                    report
                });
                let changes: Vec<(usize, usize)> = report
                    .inserted
                    .iter()
                    .filter_map(|l| {
                        let ops = &doc.document.accounts.get(l.account)?.operations;
                        Some((l.account, ops.iter().position(|op| op.uid == l.uid)?))
                    })
                    .collect();
                doc.apply_auto_rules(&changes);
                let mut summary = trn!(
                    report.inserted.len(),
                    "{} opération insérée",
                    "{} opérations insérées"
                );
                if !report.refused.is_empty() {
                    summary.push_str(&trn!(
                        report.refused.len(),
                        ", {} échéance refusée",
                        ", {} échéances refusées"
                    ));
                }
                self.schedules = schedules;
                // Les opérations insérées ne s'annulent pas d'ici : leur
                // état devient la nouvelle référence.
                self.initial = self.schedules.clone();
                self.undo_tag = None;
                self.select(self.selected);
                self.message = Some(summary);
            }
        });
        if self.oldest > 0 {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{}  {}",
                        icon::CLOCK_COUNTER_CLOCKWISE,
                        trn!(
                            self.oldest,
                            "Ce fichier contient aussi {} échéance du tout premier échéancier.",
                            "Ce fichier contient aussi {} échéances du tout premier échéancier."
                        )
                    ))
                    .color(pal.muted),
                );
                if ui.link(tr!("Les reprendre")).clicked()
                    && let Some((old, warnings)) = scheduler::oldest_schedules(&doc.document)
                {
                    let n = old.len();
                    self.schedules.extend(old);
                    self.save(doc);
                    self.warnings.extend(warnings);
                    self.oldest = 0;
                    self.message = Some(trn!(
                        n,
                        "{} ancienne échéance reprise : vérifiez sa prochaine date avant d'insérer.",
                        "{} anciennes échéances reprises : vérifiez leur prochaine date avant d'insérer."
                    ));
                    self.select(Some(self.schedules.len() - n));
                }
            });
        }
        if let Some(message) = &self.message {
            ui.label(RichText::new(format!("{}  {message}", icon::INFO)).color(pal.accent));
        }
        for warning in &self.warnings {
            ui.label(RichText::new(format!("{}  {warning}", icon::WARNING)).color(pal.warning));
        }
        crate::dialogs::footer(ui, |ui| {
            if crate::dialogs::primary(ui, tr!("Fermer"), true).clicked()
                || (!self.confirm_delete && ui.input(|i| i.key_pressed(Key::Escape)))
            {
                *outcome = Outcome::Close;
            }
            if crate::dialogs::button(
                ui,
                format!("{}  {}", icon::CHART_LINE_UP, tr!("Solde prévisionnel")),
            )
            .clicked()
            {
                *outcome =
                    Outcome::Switch(Box::new(crate::tools::Tool::Forecast(ForecastForm::new())));
            }
            let modified = self.schedules != self.initial;
            if ui
                .add_enabled(
                    modified,
                    egui::Button::new(format!(
                        "{}  {}",
                        icon::ARROW_COUNTER_CLOCKWISE,
                        tr!("Annuler les modifications")
                    ))
                    .min_size(egui::vec2(0.0, 34.0)),
                )
                .on_hover_text(tr!(
                    "Remet l'échéancier tel qu'il était à l'ouverture de la fenêtre"
                ))
                .clicked()
            {
                revert = true;
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{}  {}",
                        icon::CHECK,
                        tr!("Modifications enregistrées au fur et à mesure")
                    ))
                    .small()
                    .color(pal.muted),
                );
            });
        });
        if revert {
            self.revert(doc);
        }
    }
}

// ─────────────────────────────── Prévisionnel ───────────────────────────────

/// Solde prévisionnel du compte courant : opérations à venir et échéances.
pub struct ForecastForm {
    until: String,
    monthly: bool,
    /// Occurrence en cours de modification, avec l'index de son échéance.
    editing: Option<(usize, OccurrenceEdit)>,
}

impl ForecastForm {
    pub fn new() -> Self {
        let until = scheduler::add_months(Date::today(), 3, None).unwrap_or_else(Date::today);
        Self {
            until: until.to_string(),
            monthly: false,
            editing: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.6, 760.0, 1100.0);
        let height = ui_kit::dialog_height(ctx, 0.8).max(520.0);
        crate::dialogs::modal(ctx, "forecast").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            crate::dialogs::header(
                ui,
                icon::CHART_LINE_UP,
                pal.accent,
                tr!("Solde prévisionnel"),
            );
            let account = doc.current_account();
            let currency = account.currency.clone();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr!("Compte « {} », jusqu'au", account.name)).color(pal.fg_dark),
                );
                crate::datepicker::date_field(
                    ui,
                    "forecast_until",
                    &mut self.until,
                    180.0,
                    crate::datepicker::Placement::Popup,
                );
                for months in [1, 3, 6, 12] {
                    let label = if months == 1 {
                        tr!("+1 mois").to_string()
                    } else {
                        tr!("+{} mois", months)
                    };
                    if ui.small_button(label).clicked()
                        && let Some(d) = scheduler::add_months(Date::today(), months, None)
                    {
                        self.until = d.to_string();
                    }
                }
                crate::theme::checkbox(ui, &mut self.monthly, tr!("une ligne par mois"));
            });
            ui.add_space(12.0);
            let until = Date::parse_fr(&self.until, Date::today()).unwrap_or_else(Date::today);
            let mut schedules = scheduler::load(&doc.document)
                .map(|l| l.schedules)
                .unwrap_or_default();
            let lines = scheduler::forecast(
                &doc.document,
                &schedules,
                doc.account,
                until,
                Date::today(),
                self.monthly,
            );
            if let Some(last) = lines.last() {
                ui.horizontal(|ui| {
                    ui_kit::pill(
                        ui,
                        &tr!(
                            "Solde prévu au {date} : {amount}",
                            date = until,
                            amount = money(last.balance, &currency)
                        ),
                        if last.balance.is_negative() {
                            pal.negative
                        } else {
                            pal.positive
                        },
                    );
                    let lowest = lines.iter().min_by_key(|l| l.balance);
                    if let Some(low) = lowest {
                        ui_kit::pill(
                            ui,
                            &tr!(
                                "Point bas : {amount} le {date}",
                                amount = money(low.balance, &currency),
                                date = low.date
                            ),
                            pal.warning,
                        );
                    }
                });
                ui.add_space(10.0);
            }
            let body = height - 36.0 - 14.0 - 40.0 - 50.0 - 80.0;
            let mut changed = false;
            egui::ScrollArea::vertical()
                .id_salt("forecast_rows")
                .max_height(body)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let max = lines
                        .iter()
                        .map(|l| l.balance.0.unsigned_abs())
                        .max()
                        .unwrap_or(1)
                        .max(1) as f32;
                    for line in &lines {
                        let one_off = line.occurrence.is_some_and(|(index, due)| {
                            schedules
                                .get(index)
                                .is_some_and(|s| s.exception_on(due).is_some())
                        });
                        let row = ui.horizontal(|ui| {
                            ui.set_min_height(30.0);
                            ui_kit::cell(
                                ui,
                                96.0,
                                egui::Align::Min,
                                RichText::new(crate::settings::shared::date(line.date)),
                            );
                            let (glyph, color) = match line.kind {
                                ForecastKind::CurrentBalance => (icon::WALLET, pal.accent),
                                ForecastKind::Schedule if one_off => {
                                    (icon::PENCIL_SIMPLE, pal.warning)
                                }
                                ForecastKind::Schedule => (icon::CALENDAR_DOTS, pal.violet),
                                ForecastKind::Pending => (icon::CLOCK, pal.future),
                                ForecastKind::Month => (icon::CALENDAR_BLANK, pal.fg_dark),
                            };
                            ui.label(RichText::new(glyph).color(color));
                            let text = if line.kind == ForecastKind::Month && line.count > 1 {
                                trn!(line.count, "{} opération", "{} opérations")
                            } else {
                                line.payee.clone()
                            };
                            let w = (width - 520.0).max(160.0);
                            ui_kit::cell(ui, w, egui::Align::Min, RichText::new(text));
                            let amount_color = if line.amount.is_negative() {
                                pal.negative
                            } else {
                                pal.positive
                            };
                            ui_kit::cell(
                                ui,
                                110.0,
                                egui::Align::Max,
                                RichText::new(money(line.amount, &currency)).color(amount_color),
                            );
                            ui.add_space(12.0);
                            // Barre du solde (proportionnelle au plus grand solde absolu).
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(120.0, 10.0), egui::Sense::hover());
                            let part = line.balance.0.unsigned_abs() as f32 / max;
                            let bar = if line.balance.is_negative() {
                                pal.negative
                            } else {
                                pal.positive
                            };
                            ui.painter().rect_filled(rect, 5.0, pal.bg_highlight);
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    rect.min,
                                    egui::vec2(rect.width() * part, rect.height()),
                                ),
                                5.0,
                                bar,
                            );
                            let balance_color = if line.balance.is_negative() {
                                pal.negative
                            } else {
                                pal.fg
                            };
                            ui.label(
                                RichText::new(money(line.balance, &currency))
                                    .color(balance_color)
                                    .family(crate::theme::medium()),
                            );
                        });
                        let Some((index, due)) = line.occurrence else {
                            continue;
                        };
                        let Some(schedule) = schedules.get_mut(index) else {
                            continue;
                        };
                        // Par-dessus la ligne : ses libellés prendraient sinon le clic.
                        if ui
                            .interact(
                                row.response.rect,
                                ui.id().with(("forecast_line", index, due.0)),
                                egui::Sense::click(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(tr!(
                                "Cliquez pour changer le montant de cette seule échéance."
                            ))
                            .clicked()
                        {
                            let open = self
                                .editing
                                .as_ref()
                                .is_some_and(|(i, e)| *i == index && e.due == due);
                            self.editing =
                                (!open).then(|| (index, OccurrenceEdit::new(schedule, due)));
                        }
                        if let Some((_, edit)) = self
                            .editing
                            .as_mut()
                            .filter(|(i, e)| *i == index && e.due == due)
                        {
                            changed |= edit.show(ui, schedule, &currency);
                        }
                    }
                });
            if changed && scheduler::save(&mut doc.document, &schedules).is_ok() {
                doc.dirty = true;
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, tr!("Fermer"), true).clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    outcome = Outcome::Close;
                }
                if crate::dialogs::button(
                    ui,
                    format!("{}  {}", icon::CALENDAR_DOTS, tr!("Échéancier")),
                )
                .clicked()
                {
                    outcome = Outcome::Switch(Box::new(crate::tools::Tool::Scheduler(Box::new(
                        SchedulerForm::new(doc, None),
                    ))));
                }
            });
        });
        outcome
    }
}
