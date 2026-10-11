//! Grille des opérations : en-têtes triables avec champs de filtre intégrés,
//! lignes virtualisées (seules les lignes visibles sont dessinées).

use bp_core::filter::TackFilter;
use bp_core::model::{Date, Tack};
use bp_core::view::SortColumn;
use chrono::Datelike;
use egui::{Align, Color32, Layout, RichText, Sense, Stroke};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::palette;

const TRANSFER_TO: &str = "Transfert vers le compte ";
const TRANSFER_FROM: &str = "Transfert depuis le compte ";
const AUTO_PREFIX: &str = "[auto] ";
const SCROLLBAR_GUTTER: f32 = crate::theme::SCROLLBAR_WIDTH + 10.0;

/// Ce que l'utilisateur a demandé depuis la grille.
pub enum GridAction {
    None,
    Edit(usize),
    New,
}

pub fn show(ui: &mut egui::Ui, doc: &mut OpenDoc) -> GridAction {
    let pal = palette(ui);
    let mut action = GridAction::None;
    let today = Date::today();
    let row_height = if doc.two_lines { 42.0 } else { 30.0 };
    let currency = doc.current_account().currency.clone();
    let scroll_to = doc.scroll_to.take();
    let visible: Vec<usize> = doc.visible().to_vec();
    let sort_by_date = doc.sort == SortColumn::Date;

    let mut table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(28.0))
        .column(Column::initial(120.0).at_least(110.0))
        .column(Column::initial(104.0).at_least(60.0).clip(true))
        .column(Column::remainder().at_least(90.0).clip(true).resizable(false))
        .column(Column::remainder().at_least(70.0).clip(true).resizable(false))
        .column(Column::initial(108.0).at_least(90.0))
        .column(Column::initial(130.0).at_least(104.0))
        .min_scrolled_height(0.0);
    if let Some(op) = scroll_to
        && let Some(row) = visible.iter().position(|&i| i == op)
    {
        table = table.scroll_to_row(row, None);
    }

    table
        .header(66.0, |mut header| {
            header.col(|ui| tack_filter_header(ui, doc));
            header.col(|ui| date_header(ui, doc));
            header.col(|ui| filter_header(ui, doc, "Mode", SortColumn::Mode, |f| &mut f.mode, "carte, chq…"));
            header.col(|ui| filter_header(ui, doc, "Tiers", SortColumn::Payee, |f| &mut f.payee, "Filtrer tiers et détail"));
            header.col(|ui| filter_header(ui, doc, "Catégorie", SortColumn::Category, |f| &mut f.category, "Filtrer"));
            header.col(|ui| filter_header(ui, doc, "Montant", SortColumn::Amount, |f| &mut f.amount, "<0, -50>50…"));
            header.col(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    ui.add_space(SCROLLBAR_GUTTER);
                    ui.label(RichText::new("Solde").family(crate::theme::semibold()));
                });
            });
        })
        .body(|body| {
            body.rows(row_height, visible.len(), |mut row| {
                let pos = row.index();
                let index = visible[pos];
                let op = doc.operation(index).clone();
                let balance = doc.balance_of(index);
                row.set_selected(doc.selection.contains(&index));

                let new_month = sort_by_date && pos > 0 && {
                    let prev = doc.operation(visible[pos - 1]).date.to_naive();
                    let cur = op.date.to_naive();
                    (prev.year(), prev.month()) != (cur.year(), cur.month())
                };

                let (_, tack_resp) = row.col(|ui| {
                    let (glyph, color) = match op.tack {
                        Tack::None => ("", Color32::TRANSPARENT),
                        Tack::Pointed => (icon::CHECK, pal.pointed),
                        Tack::Reconciled => (icon::CHECKS, pal.reconciled),
                    };
                    ui.centered_and_justified(|ui| ui.label(RichText::new(glyph).color(color).size(15.0)));
                });
                row.col(|ui| {
                    if op.date > today {
                        let r = ui.max_rect();
                        ui.painter().vline(r.left() - 3.0, r.y_range(), Stroke::new(2.0, pal.future));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(op.date.to_string());
                    });
                });
                row.col(|ui| {
                    ui.add(egui::Label::new(op.mode_label()).truncate());
                });
                row.col(|ui| {
                    let (glyph, payee) = if let Some(rest) = op.payee.strip_prefix(AUTO_PREFIX) {
                        (Some(icon::CLOCK), rest)
                    } else if op.payee.starts_with(TRANSFER_TO) {
                        (Some(icon::ARROW_UP_RIGHT), op.payee.as_str())
                    } else if op.payee.starts_with(TRANSFER_FROM) {
                        (Some(icon::ARROW_DOWN_LEFT), op.payee.as_str())
                    } else {
                        (None, op.payee.as_str())
                    };
                    if let Some(glyph) = glyph {
                        ui.label(RichText::new(glyph).color(pal.muted));
                    }
                    if doc.two_lines && !op.details.is_empty() {
                        ui.vertical(|ui| {
                            ui.add_space(2.0);
                            ui.add(egui::Label::new(payee).truncate());
                            ui.add(egui::Label::new(RichText::new(&op.details).small().color(pal.muted)).truncate());
                        });
                    } else {
                        let response = ui.add(egui::Label::new(payee).truncate());
                        if !op.details.is_empty() {
                            response.on_hover_text(&op.details);
                        }
                    }
                });
                row.col(|ui| {
                    let label = op.category.and_then(|id| doc.document.category_label(id)).unwrap_or_default();
                    ui.add(egui::Label::new(RichText::new(label).color(pal.muted)).truncate());
                });
                row.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let color = if op.amount.is_negative() { pal.negative } else { pal.positive };
                        ui.label(RichText::new(money(op.amount, &currency)).color(color));
                    });
                });
                row.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Place pour la barre de défilement flottante.
                        ui.add_space(SCROLLBAR_GUTTER);
                        let mut text = RichText::new(money(balance, &currency)).family(crate::theme::semibold());
                        if balance.is_negative() {
                            text = text.color(pal.negative);
                        }
                        ui.label(text);
                    });
                });

                let response = row.response();
                if new_month {
                    // Séparateur de changement de mois, sur toute la largeur de la ligne.
                    let rect = response.interact_rect;
                    egui::Painter::new(response.ctx.clone(), response.layer_id, rect.expand(1.0))
                        .hline(rect.x_range(), rect.top(), Stroke::new(1.0, pal.month_rule));
                }
                let modifiers = response.ctx.input(|i| i.modifiers);
                if tack_resp.clicked() {
                    if !doc.selection.contains(&index) {
                        doc.click_row(index, false, false);
                    }
                    doc.cycle_tack();
                } else if response.double_clicked() {
                    doc.click_row(index, false, false);
                    action = GridAction::Edit(index);
                } else if response.clicked() {
                    doc.click_row(index, modifiers.command, modifiers.shift);
                } else if response.secondary_clicked() && !doc.selection.contains(&index) {
                    doc.click_row(index, false, false);
                }
                response.context_menu(|ui| {
                    if ui.button(format!("{}  Modifier", icon::PENCIL_SIMPLE)).clicked() {
                        action = GridAction::Edit(index);
                        ui.close();
                    }
                    if ui.button(format!("{}  Pointer", icon::CHECK)).clicked() {
                        doc.cycle_tack();
                        ui.close();
                    }
                });
            });
        });
    action
}

/// En-tête de colonne : titre cliquable (tri) + champ de filtre.
fn filter_header(
    ui: &mut egui::Ui,
    doc: &mut OpenDoc,
    title: &str,
    column: SortColumn,
    field: impl Fn(&mut bp_core::filter::Filters) -> &mut String,
    hint: &str,
) {
    ui.vertical(|ui| {
        ui.add_space(2.0);
        sort_title(ui, doc, title, column);
        let value = field(&mut doc.filters);
        let active = !value.is_empty();
        let edit = crate::theme::field(value).hint_text(hint).desired_width(f32::INFINITY);
        let response = ui.add(edit);
        if active {
            response.clone().highlight();
        }
    });
}

fn sort_title(ui: &mut egui::Ui, doc: &mut OpenDoc, title: &str, column: SortColumn) {
    let mut text = RichText::new(title).family(crate::theme::semibold());
    if doc.sort == column {
        text = RichText::new(format!("{title} {}", if doc.ascending { icon::CARET_UP } else { icon::CARET_DOWN }))
            .family(crate::theme::semibold());
    }
    if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_text("Trier").clicked() {
        if doc.sort == column {
            doc.ascending = !doc.ascending;
        } else {
            doc.sort = column;
        }
    }
}

fn date_header(ui: &mut egui::Ui, doc: &mut OpenDoc) {
    ui.vertical(|ui| {
        ui.add_space(2.0);
        sort_title(ui, doc, "Date", SortColumn::Date);
        let active = doc.filters.date_from.is_some() || doc.filters.date_to.is_some();
        let label = if active { format!("{} Période", icon::FUNNEL) } else { format!("{} Période", icon::CALENDAR_BLANK) };
        let button = egui::Button::new(label).selected(active);
        let response = ui.add_sized([ui.available_width(), 34.0], button);
        egui::Popup::menu(&response).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(220.0);
            date_bound(ui, "Du", &mut doc.filters.date_from);
            date_bound(ui, "Au", &mut doc.filters.date_to);
            ui.horizontal(|ui| {
                if ui.button("Ce mois").clicked() {
                    let d = Date::today().to_naive();
                    let first = Date::from_ymd(d.year(), d.month(), 1);
                    let next = if d.month() == 12 {
                        Date::from_ymd(d.year() + 1, 1, 1)
                    } else {
                        Date::from_ymd(d.year(), d.month() + 1, 1)
                    };
                    doc.filters.date_from = first;
                    doc.filters.date_to = next.map(|n| Date(n.0 - 1));
                }
                if ui.button("Cette année").clicked() {
                    let y = Date::today().to_naive().year();
                    doc.filters.date_from = Date::from_ymd(y, 1, 1);
                    doc.filters.date_to = Date::from_ymd(y, 12, 31);
                }
                if ui.button("Effacer").clicked() {
                    doc.filters.date_from = None;
                    doc.filters.date_to = None;
                }
            });
        });
    });
}

/// Borne de date : champ texte `jj/mm/aaaa`, appliqué dès qu'il est valide.
fn date_bound(ui: &mut egui::Ui, label: &str, bound: &mut Option<Date>) {
    ui.horizontal(|ui| {
        ui.label(format!("{label} :"));
        let id = ui.id().with(label);
        let mut text = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| bound.map(|d| d.to_string()).unwrap_or_default());
        let response = ui.add(crate::theme::field(&mut text).hint_text("jj/mm/aaaa").desired_width(110.0));
        if response.changed() {
            *bound = if text.trim().is_empty() { None } else { Date::parse_fr(&text, Date::today()).or(*bound) };
        }
        if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
    });
}

fn tack_filter_header(ui: &mut egui::Ui, doc: &mut OpenDoc) {
    ui.vertical_centered(|ui| {
        ui.add_space(26.0);
        let active = doc.filters.tack != TackFilter::All;
        let glyph = if active { icon::FUNNEL } else { icon::CHECKS };
        let response = ui.add(egui::Button::new(glyph).selected(active).frame(active)).on_hover_text("Filtrer par pointage");
        egui::Popup::menu(&response).show(|ui| {
            for filter in TackFilter::ALL {
                if ui.selectable_label(doc.filters.tack == filter, filter.label()).clicked() {
                    doc.filters.tack = filter;
                    ui.close();
                }
            }
        });
    });
}
