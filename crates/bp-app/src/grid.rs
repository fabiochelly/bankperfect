//! Grille des opérations : en-têtes triables, ligne de filtres de colonnes
//! repliable (la loupe à gauche de « Date »), lignes virtualisées (seules les
//! lignes visibles sont dessinées).

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

/// Hauteur de la pastille du mois, à cheval sur deux lignes.
const MONTH_CHIP_HEIGHT: f32 = 18.0;

/// Séparateur de changement de mois en haut de la ligne `row` : pastille du
/// mois qui commence (à partir de `x`), prolongée par un filet qui s'estompe.
/// Le filet couvre le haut de la ligne, à partir de sa limite avec la ligne
/// précédente (`top`), sur un pixel physique.
fn month_separator(
    painter: &egui::Painter,
    row: egui::Rect,
    top: f32,
    x: f32,
    date: Date,
    pal: &crate::theme::Palette,
) {
    let d = date.to_naive();
    let label = format!(
        "{} {}",
        crate::datepicker::MONTHS[d.month0() as usize].to_uppercase(),
        d.year()
    );
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &label,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::new(10.0, crate::theme::semibold()),
            color: pal.month_rule,
            extra_letter_spacing: 0.6,
            ..Default::default()
        },
    );
    let galley = painter.layout_job(job);
    let pixel = 1.0 / painter.pixels_per_point();
    let y = top + pixel / 2.0;
    let chip = egui::Rect::from_min_size(
        egui::pos2(x, y - MONTH_CHIP_HEIGHT / 2.0),
        egui::vec2(galley.size().x + 14.0, MONTH_CHIP_HEIGHT),
    );
    // Filet : plein contre la pastille, transparent au bout de la ligne.
    let line = egui::Rect::from_x_y_ranges(chip.right()..=row.right(), top..=(top + pixel));
    let strong = pal.month_rule.gamma_multiply(0.55);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(line.left_top(), strong);
    mesh.colored_vertex(line.right_top(), Color32::TRANSPARENT);
    mesh.colored_vertex(line.right_bottom(), Color32::TRANSPARENT);
    mesh.colored_vertex(line.left_bottom(), strong);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
    // Fond opaque : la pastille cache la limite entre les deux lignes.
    painter.rect(
        chip,
        MONTH_CHIP_HEIGHT / 2.0,
        pal.bg.lerp_to_gamma(pal.month_rule, 0.14),
        Stroke::new(1.0, pal.month_rule.gamma_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
    let pos = chip.center() - galley.size() / 2.0;
    painter.galley(pos, galley, pal.month_rule);
}

/// Ce que l'utilisateur a demandé depuis la grille.
pub enum GridAction {
    None,
    Edit(usize),
    New,
    /// Commande du menu contextuel.
    Command(crate::commands::Command),
}

pub fn show(ui: &mut egui::Ui, doc: &mut OpenDoc) -> GridAction {
    let pal = palette(ui);
    let mut action = GridAction::None;
    let today = Date::today();
    // Affichage sur 2 lignes : tiers et détail, catégorie et sa famille.
    let row_height = 46.0;
    let currency = doc.current_account().currency.clone();
    let scroll_to = doc.scroll_to.take();
    let visible = doc.visible();
    let sort_by_date = matches!(doc.sort, SortColumn::Date | SortColumn::MarkDate);
    let show_mark = doc.show_mark_dates;
    // Contenu de l'île du tableau : les séparateurs de mois n'en sortent pas.
    let grid_rect = ui.max_rect();
    let spacing_y = ui.spacing().item_spacing.y;

    // Fenêtre étroite : colonnes plus compactes (disposition mémorisée à part).
    let narrow = ui.available_width() < 760.0;
    let (date_w, mode_w, amount_w, balance_w) = if narrow {
        (112.0, 92.0, 96.0, 108.0)
    } else {
        (134.0, 136.0, 108.0, 130.0)
    };
    let mut table = TableBuilder::new(ui)
        .id_salt(("grid", narrow))
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(28.0))
        .column(Column::initial(date_w).at_least(108.0))
        .column(Column::initial(mode_w).at_least(44.0).clip(true))
        .column(
            Column::remainder()
                .at_least(90.0)
                .clip(true)
                .resizable(false),
        )
        .column(
            Column::remainder()
                .at_least(70.0)
                .clip(true)
                .resizable(false),
        )
        .column(Column::initial(amount_w).at_least(86.0))
        .column(Column::initial(balance_w).at_least(96.0).resizable(false))
        .min_scrolled_height(0.0);
    if let Some(op) = scroll_to
        && let Some(row) = visible.iter().position(|&i| i == op)
    {
        table = table.scroll_to_row(row, None);
    }

    let mut first_drawn = usize::MAX;
    let mut last_drawn = 0;
    // Filtres de colonnes : une ligne de champs sous les titres, ouverte
    // par la loupe.
    let open = doc.show_column_filters;
    table
        .header(if open { 66.0 } else { 30.0 }, |mut header| {
            header.col(|ui| {
                if loupe_header(ui, doc) {
                    action = GridAction::Command(crate::commands::Command::ToggleColumnFilters);
                }
            });
            header.col(|ui| date_header(ui, doc, open));
            header.col(|ui| {
                filter_header(
                    ui,
                    doc,
                    open,
                    "Mode",
                    SortColumn::Mode,
                    |f| &mut f.mode,
                    "carte, chq…",
                )
            });
            header.col(|ui| {
                filter_header(
                    ui,
                    doc,
                    open,
                    "Tiers",
                    SortColumn::Payee,
                    |f| &mut f.payee,
                    "Filtrer tiers et détail",
                )
            });
            header.col(|ui| {
                filter_header(
                    ui,
                    doc,
                    open,
                    "Catégorie",
                    SortColumn::Category,
                    |f| &mut f.category,
                    "Filtrer",
                )
            });
            header.col(|ui| {
                filter_header(
                    ui,
                    doc,
                    open,
                    "Montant",
                    SortColumn::Amount,
                    |f| &mut f.amount,
                    "<0, -50>50…",
                )
            });
            header.col(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    ui.add_space(SCROLLBAR_GUTTER);
                    ui.label(RichText::new("Solde").family(crate::theme::semibold()));
                });
            });
        })
        .body(|mut body| {
            // Zone visible des lignes : les séparateurs de mois n'en sortent pas.
            let rows_clip = body.ui_mut().clip_rect();
            body.rows(row_height, visible.len(), |mut row| {
                let pos = row.index();
                first_drawn = first_drawn.min(pos);
                last_drawn = last_drawn.max(pos);
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
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new(glyph).color(color).size(15.0))
                    });
                });
                let (date_cell, _) = row.col(|ui| {
                    if op.date > today {
                        let r = ui.max_rect();
                        ui.painter().vline(
                            r.left() - 3.0,
                            r.y_range(),
                            Stroke::new(2.0, pal.future),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Marge avant le séparateur de colonne.
                        ui.add_space(14.0);
                        if show_mark && op.tack != Tack::None {
                            // Date de pointage en petit, sous la date de l'opération.
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                                    ui.label(crate::settings::shared::date(op.date));
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            icon::CHECK,
                                            crate::settings::shared::date(op.mark_date)
                                        ))
                                        .size(11.5)
                                        .color(pal.future),
                                    )
                                    .on_hover_text("Date de pointage");
                                });
                            });
                        } else {
                            ui.label(crate::settings::shared::date(op.date));
                        }
                    });
                });
                row.col(|ui| {
                    mode_badge(ui, doc, &op, &pal);
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
                        let transfer = !op.payee.starts_with(AUTO_PREFIX);
                        if transfer {
                            // Icône de transfert : mène à la ligne liée.
                            let link = ui
                                .add(
                                    egui::Label::new(RichText::new(glyph).color(pal.accent))
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text("Aller à la ligne liée");
                            if link.clicked() {
                                doc.selection.clear();
                                doc.selection.insert(index);
                                action = GridAction::Command(crate::commands::Command::GoToSister);
                            }
                        } else {
                            ui.label(RichText::new(glyph).color(pal.muted));
                        }
                    }
                    if !op.details.is_empty() {
                        ui.vertical(|ui| {
                            ui.add_space(2.0);
                            ui.add(egui::Label::new(payee).truncate());
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&op.details).small().color(pal.muted),
                                )
                                .truncate(),
                            );
                        });
                    } else {
                        let response = ui.add(egui::Label::new(payee).truncate());
                        if !op.details.is_empty() {
                            response.on_hover_text(&op.details);
                        }
                    }
                });
                row.col(|ui| category_badge(ui, doc, op.category, &pal));
                row.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let color = if op.amount.is_negative() {
                            pal.negative
                        } else {
                            pal.positive
                        };
                        ui.label(RichText::new(money(op.amount, &currency)).color(color));
                    });
                });
                row.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Place pour la barre de défilement flottante.
                        ui.add_space(SCROLLBAR_GUTTER);
                        let mut text = RichText::new(money(balance, &currency))
                            .family(crate::theme::semibold());
                        if balance.is_negative() {
                            text = text.color(pal.negative);
                        }
                        ui.label(text);
                    });
                });

                let response = row.response();
                if new_month {
                    // Séparateur de changement de mois, sans déborder de l'île ni
                    // passer sur les en-têtes.
                    let rect = response.interact_rect;
                    let clip = rect
                        .expand2(egui::vec2(1.0, MONTH_CHIP_HEIGHT))
                        .intersect(grid_rect)
                        .intersect(rows_clip);
                    let painter = egui::Painter::new(response.ctx.clone(), response.layer_id, clip);
                    // Limite avec la ligne précédente : celle des fonds alternés,
                    // au milieu de l'espacement, calée sur les pixels.
                    let pixels = response.ctx.pixels_per_point();
                    let top = ((rect.top() - spacing_y / 2.0) * pixels).round() / pixels;
                    month_separator(&painter, rect, top, date_cell.left() + 4.0, op.date, &pal);
                }
                // Sélection dès l'appui, comme les listes natives (egui attend
                // le relâchement pour `clicked`).
                let (modifiers, pressed) = response
                    .ctx
                    .input(|i| (i.modifiers, i.pointer.primary_pressed()));
                if tack_resp.hovered() && pressed {
                    if !doc.selection.contains(&index) {
                        doc.click_row(index, false, false);
                    }
                    doc.cycle_tack();
                } else if response.double_clicked() {
                    action = GridAction::Edit(index);
                } else if response.hovered() && pressed {
                    doc.click_row(index, modifiers.command, modifiers.shift);
                } else if response.secondary_clicked() && !doc.selection.contains(&index) {
                    doc.click_row(index, false, false);
                }
                response.context_menu(|ui| {
                    use crate::commands::Command::*;
                    crate::menu::popup_style(ui.style_mut());
                    crate::menu::begin(ui);
                    let mut entry = |ui: &mut egui::Ui, command: crate::commands::Command| {
                        let info = command.info(Some(doc));
                        if crate::menu::item_enabled(
                            ui,
                            info.enabled,
                            Some(info.icon),
                            &info.label,
                            info.shortcut,
                        )
                        .clicked()
                        {
                            action = GridAction::Command(command);
                        }
                    };
                    entry(ui, EditOperation);
                    entry(ui, Tack);
                    entry(ui, SetMarkDate);
                    crate::menu::separator(ui);
                    entry(ui, CutLines);
                    entry(ui, CopyLines);
                    entry(ui, PasteLines);
                    crate::menu::separator(ui);
                    entry(ui, AddToPayees);
                    entry(ui, AddToScheduler);
                    entry(ui, ExtraFields);
                    entry(ui, SplitOperation);
                    if doc.sister_of(index).is_some() {
                        entry(ui, GoToSister);
                    } else {
                        entry(ui, ConvertToTransfer);
                    }
                    // Commandes d'extension placées ici (`menu = "grid"`).
                    let placed = crate::extensions::placed("grid");
                    if !placed.is_empty() {
                        crate::menu::separator(ui);
                        for (i, _) in placed {
                            entry(ui, Extension(i));
                        }
                    }
                    crate::menu::separator(ui);
                    entry(ui, Delete);
                });
            });
        });
    // Lignes entièrement visibles (les rangées extrêmes sont souvent coupées).
    doc.on_screen = (first_drawn <= last_drawn).then(|| {
        let first = if first_drawn > 0 { first_drawn + 1 } else { 0 };
        let last = if last_drawn + 1 < visible.len() {
            last_drawn.saturating_sub(1)
        } else {
            last_drawn
        };
        (first.min(last), last)
    });
    action
}

/// Mode de paiement dans un badge : bleu pour la carte bancaire, rouge ou
/// orange pour les autres dépenses, vert pour l'argent reçu.
fn mode_badge(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    op: &bp_core::model::Operation,
    pal: &crate::theme::Palette,
) {
    let (glyph, color) = mode_style(&doc.document, &op.mode, pal);
    pill(ui, glyph, &op.mode_label(), color, 9.0);
}

/// Icône et couleur d'un mode de paiement.
pub fn mode_style(
    document: &bp_core::model::Document,
    mode: &bp_core::model::Mode,
    pal: &crate::theme::Palette,
) -> (&'static str, egui::Color32) {
    use bp_core::model::{Mode, StandardMode as S};
    match mode {
        Mode::Standard(m) => match m {
            S::Carte => (icon::CREDIT_CARD, pal.accent),
            S::RetraitDab => (icon::MONEY, pal.future),
            S::ChequeEmis => (icon::FILE_TEXT, pal.future),
            S::Prelevement => (icon::ARROWS_CLOCKWISE, pal.negative),
            S::VirementEmis => (icon::ARROW_UP_RIGHT, pal.negative),
            S::Tip => (icon::RECEIPT, pal.future),
            S::Versement => (icon::HAND_COINS, pal.positive),
            S::DepotCheque => (icon::BANK, pal.positive),
            S::VirementRecu => (icon::ARROW_DOWN_LEFT, pal.positive),
            S::Titre => (icon::CHART_LINE_UP, pal.positive),
        },
        Mode::Custom(name) => {
            let debit = document
                .custom_modes
                .iter()
                .find(|m| &m.name == name)
                .is_none_or(|m| m.debit);
            let glyph = crate::category_icons::icon_for(name).unwrap_or(icon::TAG);
            (glyph, if debit { pal.future } else { pal.positive })
        }
    }
}

/// Badge arrondi teinté : icône et libellé dans la couleur donnée.
fn pill(ui: &mut egui::Ui, glyph: &str, label: &str, color: egui::Color32, top_space: f32) {
    ui.vertical(|ui| {
        if top_space > 0.0 {
            ui.add_space(top_space);
        }
        let family = egui::TextStyle::Small.resolve(ui.style()).family;
        let font = egui::FontId::new(12.5, family);
        let available = ui.available_width();
        // Libellé coupé avec « … » ; icône seule si la colonne est trop étroite.
        let text = if available < 64.0 {
            glyph.to_string()
        } else {
            format!("{glyph}  {label}")
        };
        let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
        job.wrap = egui::text::TextWrapping {
            max_width: (available - 16.0).max(12.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let galley = ui.painter().layout_job(job);
        let size = egui::vec2((galley.size().x + 16.0).min(available), 22.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter()
            .rect_filled(rect, 11.0, color.gamma_multiply(0.16));
        let elided = galley.elided;
        ui.painter().galley(
            egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
        if elided || available < 64.0 {
            response.on_hover_text(label);
        }
    });
}

/// Catégorie dans un badge coloré avec son icône ; la famille (catégorie
/// parente) en petit dessous.
fn category_badge(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    category: Option<u32>,
    pal: &crate::theme::Palette,
) {
    let Some(id) = category else { return };
    let Some(cat) = doc.document.category(id) else {
        return;
    };
    let style = doc.category_styles.get(&id).copied();
    let color = style.map_or(pal.muted, |s| crate::theme::category_color(pal, s.color));
    let glyph = style.map_or(icon::TAG, |s| s.icon);
    let parent = if cat.is_child {
        doc.document
            .category_label(id)
            .and_then(|label| label.split(" > ").next().map(str::to_string))
    } else {
        None
    };
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        pill(
            ui,
            glyph,
            &cat.name,
            color,
            if parent.is_none() { 9.0 } else { 0.0 },
        );
        if let Some(parent) = parent {
            ui.add(egui::Label::new(RichText::new(parent).small().color(pal.muted)).truncate());
        }
    });
}

/// En-tête de colonne : titre cliquable (tri) + champ de filtre.
fn filter_header(
    ui: &mut egui::Ui,
    doc: &mut OpenDoc,
    open: bool,
    title: &str,
    column: SortColumn,
    field: impl Fn(&mut bp_core::filter::Filters) -> &mut String,
    hint: &str,
) {
    ui.vertical(|ui| {
        ui.add_space(2.0);
        sort_title(ui, doc, title, column);
        if !open {
            return;
        }
        let value = field(&mut doc.filters);
        let active = !value.is_empty();
        let edit = crate::theme::field(value)
            .hint_text(hint)
            .desired_width(f32::INFINITY);
        let response = ui.add(edit);
        if active {
            response.clone().highlight();
        }
    });
}

fn sort_title(ui: &mut egui::Ui, doc: &mut OpenDoc, title: &str, column: SortColumn) {
    let mut text = RichText::new(title).family(crate::theme::semibold());
    if doc.sort == column {
        text = RichText::new(format!(
            "{title} {}",
            if doc.ascending {
                icon::CARET_UP
            } else {
                icon::CARET_DOWN
            }
        ))
        .family(crate::theme::semibold());
    }
    if ui
        .add(egui::Label::new(text).sense(Sense::click()))
        .on_hover_text("Trier")
        .clicked()
    {
        if doc.sort == column {
            doc.ascending = !doc.ascending;
        } else {
            doc.sort = column;
        }
    }
}

fn date_header(ui: &mut egui::Ui, doc: &mut OpenDoc, open: bool) {
    ui.vertical(|ui| {
        ui.add_space(2.0);
        sort_title(ui, doc, "Date", SortColumn::Date);
        if !open {
            return;
        }
        let active = doc.filters.date_from.is_some() || doc.filters.date_to.is_some();
        let label = if active {
            format!("{} Période", icon::FUNNEL)
        } else {
            format!("{} Période", icon::CALENDAR_BLANK)
        };
        let button = egui::Button::new(label).selected(active);
        let response = ui.add_sized([ui.available_width(), 34.0], button);
        egui::Popup::menu(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(290.0);
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

/// Borne de date : champ `jj/mm/aaaa` avec calendrier, appliquée dès qu'elle
/// est valide. Le texte saisi est conservé tant que la borne ne change pas
/// par ailleurs (boutons « Ce mois », « Cette année »…).
fn date_bound(ui: &mut egui::Ui, label: &str, bound: &mut Option<Date>) {
    ui.label(RichText::new(label).small().color(palette(ui).muted));
    let id = ui.id().with(("date_bound", label));
    let (mut text, last) = ui
        .data(|d| d.get_temp::<(String, Option<Date>)>(id))
        .filter(|(_, last)| last == bound)
        .unwrap_or_else(|| (bound.map(|d| d.to_string()).unwrap_or_default(), *bound));
    let response = crate::datepicker::date_field(
        ui,
        label,
        &mut text,
        266.0,
        crate::datepicker::Placement::Inline,
    );
    let mut new_bound = last;
    if response.changed() {
        new_bound = if text.trim().is_empty() {
            None
        } else {
            Date::parse_fr(&text, Date::today()).or(last)
        };
        *bound = new_bound;
    }
    ui.data_mut(|d| d.insert_temp(id, (text, new_bound)));
    ui.add_space(4.0);
}

/// Première colonne de l'en-tête : la loupe qui ouvre ou ferme la ligne des
/// filtres de colonnes (bleue quand elle est ouverte ; couleur des filtres
/// quand elle est fermée sur des filtres actifs), puis, ligne ouverte, le
/// filtre par pointage. Renvoie vrai si la loupe a été cliquée.
fn loupe_header(ui: &mut egui::Ui, doc: &mut OpenDoc) -> bool {
    let pal = palette(ui);
    let open = doc.show_column_filters;
    let f = &doc.filters;
    let hidden_filters = !open
        && (!f.mode.is_empty()
            || !f.payee.is_empty()
            || !f.category.is_empty()
            || !f.amount.is_empty()
            || f.date_from.is_some()
            || f.date_to.is_some()
            || f.tack != TackFilter::All);
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        ui.add_space(2.0);
        let galley = ui.painter().layout_no_wrap(
            icon::MAGNIFYING_GLASS.to_string(),
            egui::FontId::proportional(16.0),
            Color32::PLACEHOLDER,
        );
        let (rect, response) =
            ui.allocate_exact_size(galley.size() + egui::vec2(6.0, 4.0), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, "Filtres de colonnes")
        });
        let color = if open {
            pal.accent
        } else if hidden_filters {
            pal.pointed
        } else if response.hovered() {
            pal.fg
        } else {
            pal.muted
        };
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, color);
        let tip = if open {
            "Masquer les filtres de colonnes"
        } else if hidden_filters {
            "Afficher les filtres de colonnes (des filtres sont actifs)"
        } else {
            "Afficher les filtres de colonnes"
        };
        clicked = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tip)
            .clicked();
        if open {
            ui.add_space(4.0);
            tack_filter_button(ui, doc);
        }
    });
    clicked
}

fn tack_filter_button(ui: &mut egui::Ui, doc: &mut OpenDoc) {
    let active = doc.filters.tack != TackFilter::All;
    let glyph = if active { icon::FUNNEL } else { icon::CHECKS };
    let response = ui
        .add(egui::Button::new(glyph).selected(active).frame(active))
        .on_hover_text("Filtrer par pointage");
    egui::Popup::menu(&response).show(|ui| {
        crate::menu::popup_style(ui.style_mut());
        crate::menu::begin(ui);
        crate::menu::heading(ui, "Afficher les lignes");
        for filter in TackFilter::ALL {
            if crate::menu::choice(ui, doc.filters.tack == filter, filter.label()).clicked() {
                doc.filters.tack = filter;
            }
        }
    });
}
