//! Règles de calcul automatique des champs (Ctrl+F) : « si le tiers ou le
//! détail contient… alors imposer mode, tiers, détail, date, catégorie ».

use bp_core::autofields::{AutoFieldRules, CategoryChange, Rule, RuleField};
use bp_core::model::{Mode, StandardMode};
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

const LABEL_WIDTH: f32 = 160.0;

pub struct RulesForm {
    rules: AutoFieldRules,
    selected: Option<usize>,
    message: Option<String>,
    errors: Vec<String>,
}

impl RulesForm {
    pub fn new(doc: &OpenDoc) -> Self {
        let rules = AutoFieldRules::load(&doc.document, &bp_core::legacy_pickle::parse_py)
            .unwrap_or_default();
        let selected = (!rules.rules.is_empty()).then_some(0);
        Self {
            rules,
            selected,
            message: None,
            errors: Vec::new(),
        }
    }

    fn save(&self, doc: &mut OpenDoc) {
        if self.rules.save(&mut doc.document).is_ok() {
            doc.dirty = true;
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.8, 900.0, 1500.0);
        let height = ui_kit::dialog_height(ctx, 0.8).max(560.0);
        crate::dialogs::modal(ctx, "rules").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::MAGIC_WAND, pal.accent, "Règles automatiques");
            let body = height - 36.0 - 14.0 - 110.0;
            let gap = 22.0;
            let list_width = (width * 0.4).clamp(340.0, 600.0);
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
                        .id_salt("rule_detail")
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
            if let Some(message) = &self.message {
                ui.label(RichText::new(format!("{}  {message}", icon::INFO)).color(pal.accent));
            }
            for error in self.errors.iter().take(3) {
                ui.label(RichText::new(format!("{}  {error}", icon::WARNING)).color(pal.warning));
            }
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, "Fermer", true).clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    outcome = Outcome::Close;
                }
                let active = self.rules.rules.iter().filter(|r| r.active).count();
                let label = format!(
                    "{}  Appliquer les {active} règle(s) actives aux lignes affichées",
                    icon::LIGHTNING
                );
                if ui
                    .add_enabled(
                        active > 0,
                        egui::Button::new(label).min_size(egui::vec2(0.0, 36.0)),
                    )
                    .clicked()
                {
                    let (n, errors) = doc.apply_rules(&self.rules, None);
                    self.message = Some(format!("{n} ligne(s) modifiée(s) (Ctrl+Z pour annuler)"));
                    self.errors = errors;
                }
            });
        });
        outcome
    }

    fn list(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32, height: f32) -> bool {
        let pal = palette(ui);
        let mut changed = false;
        let mut clicked = None;
        let categories = doc.document.categories.clone();
        egui::Frame::new()
            .fill(pal.bg)
            .stroke(egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(width - 18.0);
                ui.set_height(height - 18.0);
                egui::ScrollArea::vertical()
                    .id_salt("rules_list")
                    .max_height(height - 18.0 - 54.0)
                    .min_scrolled_height(height - 18.0 - 54.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.rules.rules.is_empty() {
                            ui.add_space(30.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(icon::MAGIC_WAND).size(34.0).color(pal.muted));
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new("Aucune règle. Exemple : si le tiers contient « EDF », mettre la catégorie Électricité.")
                                        .color(pal.muted),
                                );
                            });
                        }
                        for (k, rule) in self.rules.rules.iter_mut().enumerate() {
                            let (condition, actions) = rule.summary(&categories);
                            let selected = self.selected == Some(k);
                            ui.horizontal(|ui| {
                                if crate::theme::checkbox(ui, &mut rule.active, "").on_hover_text("Règle active").changed() {
                                    changed = true;
                                }
                                let (rect, response) =
                                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 50.0), egui::Sense::click());
                                if selected {
                                    ui.painter().rect_filled(rect, 9.0, pal.accent.gamma_multiply(0.14));
                                } else if response.hovered() {
                                    ui.painter().rect_filled(rect, 9.0, pal.bg_highlight.gamma_multiply(0.6));
                                }
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &condition)
                                });
                                let color = if rule.active { pal.fg } else { pal.muted };
                                let galley = ui_kit::one_line(ui, format!("{}  {condition}", icon::MAGNIFYING_GLASS), egui::FontId::new(13.5, crate::theme::medium()), color, rect.width() - 80.0);
                                ui.painter().galley(rect.min + egui::vec2(10.0, 7.0), galley, color);
                                let actions_text = if actions.is_empty() { "aucune action".to_string() } else { actions };
                                let galley = ui_kit::one_line(ui, format!("{}  {actions_text}", icon::ARROW_RIGHT), egui::FontId::new(12.0, egui::FontFamily::Proportional), pal.muted, rect.width() - 20.0);
                                ui.painter().galley(rect.min + egui::vec2(10.0, 28.0), galley, pal.muted);
                                if rule.auto_apply {
                                    ui.painter().text(
                                        egui::pos2(rect.right() - 10.0, rect.top() + 14.0),
                                        egui::Align2::RIGHT_CENTER,
                                        format!("{}  auto", icon::LIGHTNING),
                                        egui::FontId::new(12.0, egui::FontFamily::Proportional),
                                        pal.accent,
                                    );
                                }
                                if response.clicked() {
                                    clicked = Some(k);
                                }
                            });
                        }
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(format!("{}  Nouvelle règle", icon::PLUS)).min_size(egui::vec2(0.0, 34.0))).clicked() {
                        let mut rule = Rule::new("");
                        rule.active = true;
                        self.rules.rules.push(rule);
                        clicked = Some(self.rules.rules.len() - 1);
                        changed = true;
                    }
                    let (import, export) = (egui::Id::new("rules_import"), egui::Id::new("rules_export"));
                    if ui.add(egui::Button::new(icon::DOWNLOAD_SIMPLE).min_size(egui::vec2(34.0, 34.0))).on_hover_text("Importer des règles…").clicked() {
                        let dialog = rfd::FileDialog::new().add_filter("Règles", &["bprules", "txt", "dat"]);
                        crate::file_picker::open(ui.ctx(), import, crate::file_picker::Kind::File, dialog);
                    }
                    if let Some(path) = crate::file_picker::chosen(ui.ctx(), import) {
                        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| AutoFieldRules::import_bytes(&b).map_err(|e| e.to_string())) {
                            Ok(rules) => {
                                let n = rules.len();
                                self.rules.rules.extend(rules);
                                changed = true;
                                self.message = Some(format!("{n} règle(s) importée(s)"));
                            }
                            Err(e) => self.errors = vec![e],
                        }
                    }
                    if ui.add(egui::Button::new(icon::EXPORT).min_size(egui::vec2(34.0, 34.0))).on_hover_text("Exporter les règles…").clicked() {
                        let dialog = rfd::FileDialog::new().set_file_name("regles.bprules").add_filter("Règles", &["bprules"]);
                        crate::file_picker::open(ui.ctx(), export, crate::file_picker::Kind::Save, dialog);
                    }
                    if let Some(path) = crate::file_picker::chosen(ui.ctx(), export) {
                        self.message = Some(match std::fs::write(&path, self.rules.export_text()) {
                            Ok(()) => format!("Règles exportées dans {}", path.display()),
                            Err(e) => format!("Export impossible : {e}"),
                        });
                    }
                    if let Some(i) = self.selected {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new(icon::TRASH).color(pal.negative)).min_size(egui::vec2(34.0, 34.0))).on_hover_text("Supprimer la règle").clicked() {
                                self.rules.rules.remove(i);
                                self.selected = (!self.rules.rules.is_empty()).then(|| i.min(self.rules.rules.len() - 1));
                                changed = true;
                            }
                            if ui.add_enabled(i + 1 < self.rules.rules.len(), egui::Button::new(icon::ARROW_DOWN).min_size(egui::vec2(34.0, 34.0))).on_hover_text("Descendre (les règles s'appliquent dans l'ordre)").clicked() {
                                self.rules.rules.swap(i, i + 1);
                                self.selected = Some(i + 1);
                                changed = true;
                            }
                            if ui.add_enabled(i > 0, egui::Button::new(icon::ARROW_UP).min_size(egui::vec2(34.0, 34.0))).on_hover_text("Monter").clicked() {
                                self.rules.rules.swap(i - 1, i);
                                self.selected = Some(i - 1);
                                changed = true;
                            }
                        });
                    }
                });
            });
        if let Some(k) = clicked {
            self.selected = Some(k);
        }
        changed
    }

    fn detail(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32) -> bool {
        let pal = palette(ui);
        let mut changed = false;
        let Some(i) = self.selected.filter(|&i| i < self.rules.rules.len()) else {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Sélectionnez une règle dans la liste.").color(pal.muted));
            });
            return false;
        };
        let full = width - 2.0 * ui_kit::CARD_PADDING - 2.0 - LABEL_WIDTH - 8.0;
        let document = doc.document.clone_without_operations();
        let styles = doc.category_styles.clone();
        let rule = &mut self.rules.rules[i];

        ui_kit::section(ui, width, icon::MAGNIFYING_GLASS, "Si je trouve", |ui| {
            ui_kit::row(ui, "Dans", LABEL_WIDTH, |ui| {
                changed |= ui_kit::segmented(
                    ui,
                    &mut rule.field,
                    &[
                        (RuleField::Payee, "", "le tiers", pal.accent),
                        (RuleField::Details, "", "le détail", pal.accent),
                        (RuleField::PayeeOrDetails, "", "les deux", pal.accent),
                    ],
                    32.0,
                    Some(full.min(380.0)),
                );
            });
            ui.add_space(8.0);
            ui_kit::row(ui, "Le texte", LABEL_WIDTH, |ui| {
                changed |= ui
                    .add(
                        crate::theme::field(&mut rule.contains)
                            .hint_text("edf+facture, !avoir, ou une formule =…")
                            .desired_width(full),
                    )
                    .changed();
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Mots séparés par + : tous doivent être présents (!mot = absent), sans tenir compte des majuscules. Une formule commence par = : =\"tresor\" in tiers and montant < -70.",
                )
                .size(12.5)
                .color(pal.muted),
            );
            ui.add_space(8.0);
            changed |= crate::theme::checkbox(
                ui,
                &mut rule.plus_is_or,
                "Un seul des mots suffit (+ veut dire OU)",
            )
            .changed();
            ui.add_space(6.0);
            changed |= crate::theme::checkbox(
                ui,
                &mut rule.only_uncategorized,
                "Seulement les opérations sans catégorie",
            )
            .changed();
        });

        ui_kit::section(ui, width, icon::PENCIL_SIMPLE, "Alors je modifie", |ui| {
            // Mode (modes standard seulement, comme l'original).
            ui.horizontal(|ui| {
                let mut on = rule.set_mode.is_some();
                ui.allocate_ui_with_layout(
                    egui::vec2(LABEL_WIDTH, 34.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(LABEL_WIDTH);
                        if crate::theme::checkbox(ui, &mut on, "Mode").changed() {
                            rule.set_mode = on.then_some(StandardMode::Carte);
                            changed = true;
                        }
                    },
                );
                ui.add_enabled_ui(on, |ui| {
                    let mut mode = Mode::Standard(rule.set_mode.unwrap_or(StandardMode::Carte));
                    let before = mode.clone();
                    egui::ComboBox::from_id_salt("rule_mode")
                        .selected_text(mode.label().to_string())
                        .width(full - 12.0)
                        .show_ui(ui, |ui| {
                            for m in StandardMode::ALL {
                                ui.selectable_value(&mut mode, Mode::Standard(m), m.label());
                            }
                        });
                    if mode != before
                        && let Mode::Standard(m) = mode
                    {
                        rule.set_mode = Some(m);
                        changed = true;
                    }
                });
            });
            ui.add_space(6.0);
            for (label, value, hint) in [
                ("Tiers", &mut rule.set_payee, "nouveau tiers"),
                (
                    "Détail",
                    &mut rule.set_details,
                    "nouveau détail, ou =formule",
                ),
                ("Date", &mut rule.set_date, "jj/mm/aaaa ou =formule"),
            ] {
                ui.horizontal(|ui| {
                    let mut on = value.is_some();
                    ui.allocate_ui_with_layout(
                        egui::vec2(LABEL_WIDTH, 34.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(LABEL_WIDTH);
                            if crate::theme::checkbox(ui, &mut on, label).changed() {
                                *value = on.then(String::new);
                                changed = true;
                            }
                        },
                    );
                    if let Some(text) = value {
                        changed |= ui
                            .add(
                                crate::theme::field(text)
                                    .hint_text(hint)
                                    .desired_width(full),
                            )
                            .changed();
                    }
                });
                ui.add_space(6.0);
            }
            ui.horizontal(|ui| {
                let mut on = rule.set_category.is_some();
                ui.allocate_ui_with_layout(
                    egui::vec2(LABEL_WIDTH, 34.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(LABEL_WIDTH);
                        if crate::theme::checkbox(ui, &mut on, "Catégorie").changed() {
                            rule.set_category = on.then_some(CategoryChange::Clear);
                            changed = true;
                        }
                    },
                );
                if let Some(change) = rule.set_category {
                    let mut category = match change {
                        CategoryChange::Clear => None,
                        CategoryChange::Set(id) => Some(id),
                    };
                    if ui_kit::category_combo(
                        ui,
                        "rule_cat",
                        &document,
                        &styles,
                        &mut category,
                        full,
                    ) {
                        rule.set_category =
                            Some(category.map_or(CategoryChange::Clear, CategoryChange::Set));
                        changed = true;
                    }
                }
            });
        });

        ui_kit::section(ui, width, icon::LIGHTNING, "Quand", |ui| {
            changed |= crate::theme::checkbox(ui, &mut rule.active, "Règle active").changed();
            ui.add_space(6.0);
            changed |= crate::theme::checkbox(
                ui,
                &mut rule.auto_apply,
                "Appliquer automatiquement aux opérations saisies, importées ou insérées par l'échéancier",
            )
            .changed();
        });

        // Test sur les lignes affichées.
        let mut apply_now = false;
        let account = doc.current_account();
        let lines: Vec<usize> = doc.visible().to_vec();
        let result = rule.find_matches(&doc.document.categories, account, &lines);
        ui_kit::section(
            ui,
            width,
            icon::FLASK,
            "Essai sur les lignes affichées",
            |ui| match result {
                Ok(outcome) => {
                    ui.horizontal(|ui| {
                        let n = outcome.matched.len();
                        let text = match n {
                            0 => "aucune ligne ne vérifie la règle".to_string(),
                            1 => "1 ligne vérifie la règle".to_string(),
                            n => format!("{n} lignes vérifient la règle"),
                        };
                        ui_kit::pill(ui, &text, if n == 0 { pal.muted } else { pal.accent });
                        if n > 0
                            && ui
                                .button(format!("{}  Appliquer à ces lignes", icon::LIGHTNING))
                                .clicked()
                        {
                            apply_now = true;
                        }
                    });
                    for &line in outcome.matched.iter().take(5) {
                        let op = &doc.current_account().operations[line];
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · {}",
                                op.date,
                                op.payee,
                                crate::format::money(op.amount, "€")
                            ))
                            .size(12.5)
                            .color(pal.fg_dark),
                        );
                    }
                }
                Err(e) => {
                    ui.label(RichText::new(format!("{}  {e}", icon::WARNING)).color(pal.negative));
                }
            },
        );
        if apply_now {
            let (changed_lines, errors) = doc.apply_rules(&self.rules, Some(i));
            self.message = Some(format!(
                "{changed_lines} ligne(s) modifiée(s) (Ctrl+Z pour annuler)"
            ));
            self.errors = errors;
        }
        changed
    }
}
