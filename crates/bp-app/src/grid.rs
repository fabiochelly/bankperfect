//! Grille des opérations : en-têtes triables, ligne de filtres de colonnes
//! repliable (la loupe à gauche de « Date »), lignes virtualisées (seules les
//! lignes visibles sont dessinées).

use bp_core::filter::TackFilter;
use bp_core::model::{Amount, Date, Tack};
use bp_core::tr;
use bp_core::view::SortColumn;
use chrono::Datelike;
use egui::emath::GuiRounding as _;
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
        bp_core::i18n::month_title(d.month0() as usize).to_uppercase(),
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

/// Marges intérieures de l'île du tableau (voir `app.rs`).
pub const ISLAND_MARGIN: egui::Margin = egui::Margin {
    left: 6,
    right: 2,
    top: 6,
    bottom: 4,
};

/// Largeur de la colonne de pointage.
const TACK_WIDTH: f32 = 28.0;

/// Largeurs des colonnes réglables. Tiers et Catégorie se partagent la place
/// restante (`payee_share` : part des tiers).
#[derive(Clone, Copy, PartialEq)]
struct ColumnWidths {
    date: f32,
    mode: f32,
    payee_share: f32,
    amount: f32,
    balance: f32,
}

/// Largeurs minimales, d'après le contenu de chaque colonne.
struct ColumnMinima {
    date: f32,
    mode: f32,
    payee: f32,
    category: f32,
    amount: f32,
    balance: f32,
}

/// Position des colonnes à l'écran pour l'image en cours.
struct ColumnLayout {
    /// Place partagée entre Tiers et Catégorie.
    flex: f32,
    payee: f32,
    /// Traits entre les colonnes, de gauche à droite : après le pointage, la
    /// date, le mode, les tiers et le montant.
    rules: [f32; 5],
}

impl ColumnWidths {
    fn initial(narrow: bool) -> Self {
        if narrow {
            Self {
                date: 112.0,
                mode: 92.0,
                payee_share: 0.5,
                amount: 96.0,
                balance: 108.0,
            }
        } else {
            Self {
                date: 134.0,
                mode: 136.0,
                payee_share: 0.5,
                amount: 108.0,
                balance: 130.0,
            }
        }
    }

    fn at_least(mut self, m: &ColumnMinima) -> Self {
        self.date = self.date.max(m.date);
        self.mode = self.mode.max(m.mode);
        self.amount = self.amount.max(m.amount);
        self.balance = self.balance.max(m.balance);
        self
    }

    fn layout(&self, left: f32, total: f32, m: &ColumnMinima, ui: &egui::Ui) -> ColumnLayout {
        let s = ui.spacing().item_spacing.x;
        let fixed = TACK_WIDTH + self.date + self.mode + self.amount + self.balance + 6.0 * s;
        let flex = (total - fixed).max(m.payee + m.category);
        let payee = (flex * self.payee_share).clamp(m.payee, flex - m.category);
        // Comme `egui_extras` : au milieu de l'espace entre deux colonnes.
        let after = |x: f32, w: f32| x + w + s / 2.0;
        let tack = after(left, TACK_WIDTH);
        let date = after(tack + s / 2.0, self.date);
        let mode = after(date + s / 2.0, self.mode);
        let payee_rule = after(mode + s / 2.0, payee);
        let amount = left + total - self.balance - s / 2.0;
        ColumnLayout {
            flex,
            payee,
            rules: [tack, date, mode, payee_rule, amount],
        }
    }
}

impl ColumnMinima {
    fn of(ui: &egui::Ui) -> Self {
        let body = egui::TextStyle::Body.resolve(ui.style());
        let bold = egui::FontId::new(body.size, crate::theme::semibold());
        let width = |text: &str, font: &egui::FontId| {
            ui.painter()
                .layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE)
                .size()
                .x
        };
        let money = "-88\u{202f}888,88\u{a0}€";
        Self {
            // Marge avant le trait (14) et un peu d'air.
            date: width("88/88/8888", &body) + 14.0 + 8.0,
            // Badge réduit à son icône.
            mode: 44.0,
            payee: 90.0,
            category: (width(tr!("Catégorie"), &bold) + 16.0).max(70.0),
            amount: (width(money, &body) + 8.0).max(width(tr!("Montant"), &bold) + 24.0),
            balance: width(money, &bold) + SCROLLBAR_GUTTER + 8.0,
        }
    }
}

/// Traits entre les colonnes et poignées de redimensionnement : la date et le
/// mode prennent ou rendent leur place à Tiers et Catégorie, qui se la
/// partagent à leur limite, comme Montant et Solde à la leur. Les poignées
/// ne se saisissent que dans l'en-tête (`header_y`). Double-clic : largeur
/// d'origine.
fn column_handles(
    ui: &egui::Ui,
    narrow: bool,
    rules_y: egui::Rangef,
    header_y: egui::Rangef,
    layout: &ColumnLayout,
    m: &ColumnMinima,
    w: &mut ColumnWidths,
) {
    let initial = ColumnWidths::initial(narrow);
    // Place que Tiers et Catégorie peuvent encore céder.
    let spare = (layout.flex - m.payee - m.category).max(0.0);
    for (k, &x) in layout.rules.iter().enumerate() {
        let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        if k > 0 {
            let rect = egui::Rect::from_x_y_ranges((x - 4.0)..=(x + 4.0), header_y);
            let id = ui.id().with(("grid_resize", narrow, k));
            let response = ui.interact(rect, id, Sense::click_and_drag());
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
                stroke = ui.visuals().widgets.hovered.bg_stroke;
            }
            let dx = response.drag_delta().x;
            if response.dragged() {
                stroke = ui.visuals().widgets.active.bg_stroke;
            }
            match k {
                1 if response.double_clicked() => w.date = initial.date,
                1 => w.date = (w.date + dx).clamp(m.date, w.date + spare),
                2 if response.double_clicked() => w.mode = initial.mode,
                2 => w.mode = (w.mode + dx).clamp(m.mode, w.mode + spare),
                3 if response.double_clicked() => w.payee_share = initial.payee_share,
                3 => {
                    let payee = (layout.payee + dx).clamp(m.payee, layout.flex - m.category);
                    w.payee_share = payee / layout.flex;
                }
                _ if response.double_clicked() => {
                    w.amount = initial.amount;
                    w.balance = initial.balance;
                }
                _ => {
                    let both = w.amount + w.balance;
                    w.amount = (w.amount + dx).clamp(m.amount, both - m.balance);
                    w.balance = both - w.amount;
                }
            }
        }
        ui.painter().vline(x, rules_y, stroke);
    }
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
    // Courbe du solde : le solde progressif n'a de sens que trié par date.
    let river = if sort_by_date && doc.show_balance_river {
        // Échelle des lignes à l'écran (à l'image précédente), avec de la marge.
        let (first, last) = doc.on_screen.unwrap_or((0, 30));
        let rows = visible.get(first.saturating_sub(10)..(last + 11).min(visible.len()));
        let range = rows.unwrap_or(&visible).iter().fold(None, |range, &i| {
            let b = doc.balance_of(i);
            Some(range.map_or((b, b), |(lo, hi): (Amount, Amount)| (lo.min(b), hi.max(b))))
        });
        crate::river::River::new(ui.ctx(), (doc.file_id, doc.account), range)
    } else {
        crate::river::reset(ui.ctx());
        None
    };
    // Contenu de l'île du tableau : les séparateurs de mois n'en sortent pas.
    let grid_rect = ui.max_rect();
    let spacing_y = ui.spacing().item_spacing.y;
    // Ligne des titres de colonnes (hauteur d'un `ui.horizontal`).
    let title_row = ui.spacing().interact_size.y;

    // Fenêtre étroite : colonnes plus compactes (disposition mémorisée à part).
    let narrow = ui.available_width() < 760.0;
    // Largeurs réglées à la souris (poignées dessinées après le tableau).
    let widths_id = ui.id().with(("grid_widths", narrow));
    let minima = ColumnMinima::of(ui);
    let mut widths = ui
        .data(|d| d.get_temp::<ColumnWidths>(widths_id))
        .unwrap_or_else(|| ColumnWidths::initial(narrow))
        .at_least(&minima);
    let layout = widths.layout(ui.cursor().left(), ui.available_width(), &minima, ui);
    let table_top = ui.cursor().top();
    let mut table = TableBuilder::new(ui)
        .id_salt(("grid", narrow))
        .striped(true)
        .resizable(false)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(TACK_WIDTH))
        .column(Column::exact(widths.date).clip(true))
        .column(Column::exact(widths.mode).clip(true))
        .column(Column::exact(layout.payee).clip(true))
        .column(Column::remainder().at_least(minima.category).clip(true))
        .column(Column::exact(widths.amount).clip(true))
        .column(Column::exact(widths.balance).clip(true))
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
    // Ouverte : marge, ligne des titres, champ de 34 points et marge du bas.
    let header_height = if open {
        2.0 + title_row + spacing_y + 34.0 + 6.0
    } else {
        30.0
    };
    table
        .header(header_height, |mut header| {
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
                    tr!("Mode"),
                    SortColumn::Mode,
                    |f| &mut f.mode,
                    tr!("carte, chq…"),
                )
            });
            header.col(|ui| {
                filter_header(
                    ui,
                    doc,
                    open,
                    tr!("Tiers"),
                    SortColumn::Payee,
                    |f| &mut f.payee,
                    tr!("Filtrer tiers et détail"),
                )
            });
            header.col(|ui| {
                filter_header(
                    ui,
                    doc,
                    open,
                    tr!("Catégorie"),
                    SortColumn::Category,
                    |f| &mut f.category,
                    tr!("Filtrer"),
                )
            });
            header.col(|ui| {
                filter_header(
                    ui,
                    doc,
                    open,
                    tr!("Montant"),
                    SortColumn::Amount,
                    |f| &mut f.amount,
                    "<0, -50>50…",
                )
            });
            header.col(|ui| {
                // Même bloc que les titres triables : une ligne de la hauteur de
                // `ui.horizontal` (titre centré dedans), alignée à droite.
                ui.vertical(|ui| {
                    ui.add_space(2.0);
                    let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
                    let layout = Layout::right_to_left(Align::Center);
                    ui.allocate_ui_with_layout(row, layout, |ui| {
                        ui.add_space(SCROLLBAR_GUTTER);
                        ui.label(RichText::new(tr!("Solde")).family(crate::theme::semibold()));
                    });
                    if open {
                        ui.add_space(34.0 + ui.spacing().item_spacing.y);
                    }
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
                                    .on_hover_text(tr!("Date de pointage"));
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
                    // Le tiers d'un transfert est enregistré en français (lien
                    // avec la ligne sœur) ; il s'affiche dans la langue choisie.
                    let (glyph, payee) = if let Some(rest) = op.payee.strip_prefix(AUTO_PREFIX) {
                        (Some(icon::CLOCK), rest.to_string())
                    } else if op.payee.starts_with(TRANSFER_TO) {
                        let label = bp_core::split::payee_label(&op.payee);
                        (Some(icon::ARROW_UP_RIGHT), label.into_owned())
                    } else if op.payee.starts_with(TRANSFER_FROM) {
                        let label = bp_core::split::payee_label(&op.payee);
                        (Some(icon::ARROW_DOWN_LEFT), label.into_owned())
                    } else {
                        (None, op.payee.clone())
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
                                .on_hover_text(tr!("Aller à la ligne liée"));
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
                let (_, balance_cell) = row.col(|ui| {
                    if let Some(river) = &river {
                        let cell = ui.max_rect();
                        // Bande de la ligne sans trou avec ses voisines, comme
                        // les fonds alternés.
                        let band = cell.expand2(0.5 * ui.spacing().item_spacing).round_ui();
                        let painter = egui::Painter::new(
                            ui.ctx().clone(),
                            ui.layer_id(),
                            band.intersect(rows_clip),
                        );
                        let whole_row =
                            egui::Rect::from_x_y_ranges(grid_rect.x_range(), band.y_range())
                                .intersect(rows_clip);
                        let hovered = ui.ctx().rect_contains_pointer(ui.layer_id(), whole_row);
                        let neighbour = |p: Option<usize>| {
                            p.and_then(|p| visible.get(p))
                                .map_or(balance, |&i| doc.balance_of(i))
                                .as_f64()
                        };
                        river.paint(
                            &painter,
                            egui::Rangef::new(cell.left() + 4.0, cell.right() - SCROLLBAR_GUTTER),
                            band.y_range(),
                            cell.center().y,
                            [
                                neighbour(pos.checked_sub(1)),
                                balance.as_f64(),
                                neighbour(Some(pos + 1)),
                            ],
                            hovered,
                            &pal,
                            ui.visuals().dark_mode,
                        );
                    }
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
                if river.is_some() && crate::river::wants_tooltip(&balance_cell, rows_clip) {
                    let anchor = &balance_cell;
                    let id = balance_cell.id.with("balance_tooltip");
                    egui::Tooltip::always_open(
                        balance_cell.ctx.clone(),
                        balance_cell.layer_id,
                        id,
                        anchor,
                    )
                    .show(|ui| {
                        let mark_date = doc.sort == SortColumn::MarkDate;
                        let (points, marked) = crate::river::window(
                            &visible,
                            pos,
                            |i| {
                                let op = doc.operation(i);
                                if mark_date { op.mark_date } else { op.date }
                            },
                            |i| doc.balance_of(i),
                            doc.ascending,
                        );
                        crate::river::tooltip(ui, &points, marked, &currency, &pal);
                    });
                }

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
    // Traits jusqu'aux bords de l'île (dans ses marges), sans couvrir son liseré.
    let rules_y = egui::Rangef::new(
        table_top - f32::from(ISLAND_MARGIN.top) + 1.0,
        ui.min_rect().bottom() + f32::from(ISLAND_MARGIN.bottom) - 1.0,
    );
    let header_y = egui::Rangef::new(table_top, table_top + header_height);
    column_handles(ui, narrow, rules_y, header_y, &layout, &minima, &mut widths);
    ui.data_mut(|d| d.insert_temp(widths_id, widths));
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
    pill(ui, glyph, &op.mode_label(), color);
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
fn pill(ui: &mut egui::Ui, glyph: &str, label: &str, color: egui::Color32) -> egui::Response {
    ui.vertical(|ui| {
        ui.add_space(((ui.available_height() - PILL_HEIGHT) / 2.0).max(0.0));
        pill_body(ui, glyph, label, color)
    })
    .inner
}

/// Le badge lui-même, à la position courante.
fn pill_body(ui: &mut egui::Ui, glyph: &str, label: &str, color: egui::Color32) -> egui::Response {
    {
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
        let size = egui::vec2((galley.size().x + 16.0).min(available), PILL_HEIGHT);
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
            response.on_hover_text(label)
        } else {
            response
        }
    }
}

/// Hauteur des badges.
const PILL_HEIGHT: f32 = 22.0;

/// Catégorie dans un badge coloré avec son icône. Une sous-catégorie est
/// précédée de sa famille, en deux segments séparés par un chevron vide comme
/// un prompt de terminal ; faute de place, la famille passe dans l'infobulle.
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
    match parent {
        Some(parent) if family_pill(ui, glyph, &parent, &cat.name, color) => {}
        Some(parent) => {
            pill(ui, glyph, &cat.name, color).on_hover_text(format!("{parent} › {}", cat.name));
        }
        None => {
            pill(ui, glyph, &cat.name, color);
        }
    }
}

/// Profondeur de la pointe du chevron entre les deux segments.
const CHEVRON_DEPTH: f32 = 7.0;
/// Vide entre les deux segments, mesuré horizontalement.
const CHEVRON_GAP: f32 = 3.5;

/// Badge en deux segments « icône Famille › Catégorie » : la famille sur un
/// fond léger se termine en pointe, la catégorie sur le fond habituel des
/// badges commence par une encoche de même forme, un vide les sépare. La
/// catégorie est coupée (« … ») si la colonne est trop étroite ; rien n'est
/// dessiné (`false`) s'il n'en reste pas au moins deux lettres.
fn family_pill(ui: &mut egui::Ui, glyph: &str, family: &str, name: &str, color: Color32) -> bool {
    let font = egui::FontId::new(12.5, egui::TextStyle::Small.resolve(ui.style()).family);
    let full = format!("{family} › {name}");
    let painter = ui.painter();
    let icon = painter.layout_no_wrap(glyph.to_string(), font.clone(), color);
    let family =
        painter.layout_no_wrap(family.to_string(), font.clone(), color.gamma_multiply(0.8));
    // Pointe du chevron, mesurée depuis le bord gauche.
    let tip = 8.0 + icon.size().x + 6.0 + family.size().x + 6.0 + CHEVRON_DEPTH;
    let name_x = tip + CHEVRON_GAP + 6.0;
    let room = ui.available_width() - name_x - 10.0;
    if room < 24.0 {
        return false;
    }
    let mut job = egui::text::LayoutJob::simple_singleline(name.to_string(), font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: room,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let name = painter.layout_job(job);
    let elided = name.elided;
    let width = name_x + name.size().x + 10.0;
    ui.vertical(|ui| {
        ui.add_space(((ui.available_height() - PILL_HEIGHT) / 2.0).max(0.0));
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, PILL_HEIGHT), Sense::hover());
        if elided {
            response.on_hover_text(&full);
        }
        let painter = ui.painter();
        let (top, mid, bottom) = (rect.top(), rect.center().y, rect.bottom());
        let r = PILL_HEIGHT / 2.0;
        let tip_x = rect.left() + tip;
        // Demi-cercle d'un bout arrondi, de haut en bas.
        let arc = |center_x: f32, from: f32, to: f32| -> Vec<egui::Pos2> {
            (0..=12)
                .map(|i| {
                    let a = from + (to - from) * i as f32 / 12.0;
                    egui::pos2(center_x + r * a.cos(), mid + r * a.sin())
                })
                .collect()
        };
        use std::f32::consts::FRAC_PI_2;
        // Famille : bout gauche arrondi, pointe à droite (sens horaire).
        let mut left = arc(rect.left() + r, 3.0 * FRAC_PI_2, FRAC_PI_2);
        left.extend([
            egui::pos2(tip_x - CHEVRON_DEPTH, bottom),
            egui::pos2(tip_x, mid),
            egui::pos2(tip_x - CHEVRON_DEPTH, top),
        ]);
        // Catégorie : encoche à gauche, bout droit arrondi.
        let notch = tip_x + CHEVRON_GAP;
        let mut right = vec![egui::pos2(notch - CHEVRON_DEPTH, top)];
        right.extend(arc(rect.right() - r, -FRAC_PI_2, FRAC_PI_2));
        right.extend([
            egui::pos2(notch - CHEVRON_DEPTH, bottom),
            egui::pos2(notch, mid),
        ]);
        let feather = 1.0 / ui.ctx().pixels_per_point();
        let left_center = egui::pos2((rect.left() + tip_x) / 2.0, mid);
        let right_center = egui::pos2((notch + rect.right()) / 2.0, mid);
        fill_star_polygon(
            painter,
            &left,
            left_center,
            color.gamma_multiply(0.08),
            feather,
        );
        fill_star_polygon(
            painter,
            &right,
            right_center,
            color.gamma_multiply(0.2),
            feather,
        );
        let mut x = rect.left() + 8.0;
        for galley in [icon, family] {
            let w = galley.size().x;
            painter.galley(egui::pos2(x, mid - galley.size().y / 2.0), galley, color);
            x += w + 6.0;
        }
        let pos = egui::pos2(rect.left() + name_x, mid - name.size().y / 2.0);
        painter.galley(pos, name, color);
    });
    true
}

/// Remplit un polygone (même concave) dont tous les sommets sont visibles
/// depuis `center`, avec un bord adouci de `feather` (epaint ne remplit que
/// les polygones convexes).
fn fill_star_polygon(
    painter: &egui::Painter,
    points: &[egui::Pos2],
    center: egui::Pos2,
    color: Color32,
    feather: f32,
) {
    let n = points.len();
    if n < 3 {
        return;
    }
    // Sens de parcours : les normales des côtés pointent vers l'extérieur.
    let area: f32 = (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    let outward = |a: egui::Pos2, b: egui::Pos2| {
        let d = (b - a).normalized();
        if area > 0.0 {
            egui::vec2(d.y, -d.x)
        } else {
            egui::vec2(-d.y, d.x)
        }
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color);
    for i in 0..n {
        let prev = points[(i + n - 1) % n];
        let (p, next) = (points[i], points[(i + 1) % n]);
        let normal = (outward(prev, p) + outward(p, next)).normalized();
        // Onglet borné pour les angles aigus (pointe du chevron).
        let cos = normal.dot(outward(p, next)).max(0.3);
        let offset = normal * (feather / 2.0 / cos);
        mesh.colored_vertex(p - offset, color);
        mesh.colored_vertex(p + offset, Color32::TRANSPARENT);
    }
    for i in 0..n as u32 {
        let j = (i + 1) % n as u32;
        let (inner, outer) = (1 + 2 * i, 2 + 2 * i);
        let (inner2, outer2) = (1 + 2 * j, 2 + 2 * j);
        mesh.add_triangle(0, inner, inner2);
        mesh.add_triangle(inner, outer, outer2);
        mesh.add_triangle(inner, outer2, inner2);
    }
    painter.add(mesh);
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

/// Police de la flèche de tri, celle des titres.
fn sort_arrow_font(ui: &egui::Ui) -> egui::FontId {
    egui::FontId::new(
        egui::TextStyle::Body.resolve(ui.style()).size,
        crate::theme::semibold(),
    )
}

/// Place réservée à la flèche de tri ; sa hauteur fixe celle de la ligne
/// des titres.
fn sort_arrow_size(ui: &egui::Ui) -> egui::Vec2 {
    ui.painter()
        .layout_no_wrap(
            icon::CARET_DOWN.to_string(),
            sort_arrow_font(ui),
            Color32::WHITE,
        )
        .size()
}

/// Titre de colonne cliquable : trie sur la colonne, ou inverse le tri si
/// elle l'est déjà. La flèche du tri actuel est affichée ; au survol, une
/// flèche dans la couleur d'accent annonce le tri que donnera le clic.
fn sort_title(ui: &mut egui::Ui, doc: &mut OpenDoc, title: &str, column: SortColumn) {
    let sorted = doc.sort == column;
    // Sens après un clic : inversé sur la colonne triée, inchangé sinon.
    let next_ascending = if sorted {
        !doc.ascending
    } else {
        doc.ascending
    };
    let caret = |ascending: bool| {
        if ascending {
            icon::CARET_UP
        } else {
            icon::CARET_DOWN
        }
    };
    let clicked = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let text = RichText::new(title).family(crate::theme::semibold());
            let title = ui.add(egui::Label::new(text).sense(Sense::click()));
            let font = sort_arrow_font(ui);
            let (rect, arrow) = ui.allocate_exact_size(sort_arrow_size(ui), Sense::click());
            // Flèche affichée :
            // - au clic (bouton relâché), le tri qu'il applique, en plein ;
            // - au survol, en couleur, le tri que donnera le clic. Elle ne change
            //   pas quand on appuie sur le titre (seulement au relâchement), ni
            //   juste après un clic : le tri appliqué reste affiché jusqu'à ce
            //   que le pointeur quitte le titre ;
            // - sinon, le tri actuel de la colonne.
            let clicked = title.clicked() || arrow.clicked();
            // `hovered` passe à faux pendant l'appui : on suit le pointeur.
            let hovered = title.contains_pointer() || arrow.contains_pointer();
            // Bouton enfoncé ailleurs (glisser une poignée par-dessus) : pas
            // de flèche de survol. Un appui commencé sur le titre est retenu
            // jusqu'au relâchement (egui l'oublie s'il se prolonge).
            let any_down = ui.input(|i| i.pointer.any_down());
            let press_id = ui.id().with(("sort_press", format!("{column:?}")));
            if title.is_pointer_button_down_on() || arrow.is_pointer_button_down_on() {
                ui.data_mut(|d| d.insert_temp(press_id, true));
            } else if !any_down {
                ui.data_mut(|d| d.remove::<bool>(press_id));
            }
            let pressed_here = ui.data(|d| d.get_temp::<bool>(press_id)).unwrap_or(false);
            let pressed_elsewhere = any_down && !pressed_here;
            let clicked_id = ui.id().with(("sort_clicked", format!("{column:?}")));
            if clicked {
                ui.data_mut(|d| d.insert_temp(clicked_id, true));
            } else if !hovered {
                ui.data_mut(|d| d.remove::<bool>(clicked_id));
            }
            let just_clicked = ui.data(|d| d.get_temp::<bool>(clicked_id)).unwrap_or(false);
            let strong = ui.visuals().strong_text_color();
            let (glyph, color) = if clicked {
                (caret(next_ascending), strong)
            } else if hovered && !just_clicked && !pressed_elsewhere {
                // « Allumée » dans la couleur d'accent du thème.
                (caret(next_ascending), palette(ui).accent)
            } else if sorted {
                (caret(doc.ascending), strong)
            } else {
                ("", Color32::TRANSPARENT)
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                font,
                color,
            );
            let hint = if next_ascending {
                tr!("Trier par ordre croissant")
            } else {
                tr!("Trier par ordre décroissant")
            };
            let title = title.on_hover_text(hint);
            let arrow = arrow.on_hover_text(hint);
            if hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            title.clicked() || arrow.clicked()
        })
        .inner;
    if clicked {
        doc.sort = column;
        doc.ascending = next_ascending;
    }
}

fn date_header(ui: &mut egui::Ui, doc: &mut OpenDoc, open: bool) {
    ui.vertical(|ui| {
        ui.add_space(2.0);
        sort_title(ui, doc, tr!("Date"), SortColumn::Date);
        if !open {
            return;
        }
        let active = doc.filters.date_from.is_some() || doc.filters.date_to.is_some();
        let glyph = if active {
            icon::FUNNEL
        } else {
            icon::CALENDAR_BLANK
        };
        let label = format!("{glyph} {}", tr!("Période"));
        let button = egui::Button::new(label).selected(active);
        let response = ui.add_sized([ui.available_width(), 34.0], button);
        egui::Popup::menu(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(290.0);
                date_bound(ui, "Du", tr!("Du"), &mut doc.filters.date_from);
                date_bound(ui, "Au", tr!("Au"), &mut doc.filters.date_to);
                ui.horizontal(|ui| {
                    if ui.button(tr!("Ce mois")).clicked() {
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
                    if ui.button(tr!("Cette année")).clicked() {
                        let y = Date::today().to_naive().year();
                        doc.filters.date_from = Date::from_ymd(y, 1, 1);
                        doc.filters.date_to = Date::from_ymd(y, 12, 31);
                    }
                    if ui.button(tr!("Effacer")).clicked() {
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
fn date_bound(ui: &mut egui::Ui, salt: &str, label: &str, bound: &mut Option<Date>) {
    ui.label(RichText::new(label).small().color(palette(ui).muted));
    let id = ui.id().with(("date_bound", salt));
    let (mut text, last) = ui
        .data(|d| d.get_temp::<(String, Option<Date>)>(id))
        .filter(|(_, last)| last == bound)
        .unwrap_or_else(|| (bound.map(|d| d.to_string()).unwrap_or_default(), *bound));
    let response = crate::datepicker::date_field(
        ui,
        salt,
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
    // Haut de la ligne des champs de filtre (voir `filter_header`).
    let fields_top =
        ui.max_rect().top() + 2.0 + ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
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
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                true,
                open,
                tr!("Filtres de colonnes"),
            )
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
            tr!("Masquer les filtres de colonnes")
        } else if hidden_filters {
            tr!("Afficher les filtres de colonnes (des filtres sont actifs)")
        } else {
            tr!("Afficher les filtres de colonnes")
        };
        clicked = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tip)
            .clicked();
        if open {
            // Centré sur les champs de filtre des autres colonnes.
            ui.add_space((fields_top - ui.cursor().top()).max(0.0));
            let row = egui::vec2(ui.available_width(), 34.0);
            let layout = Layout::left_to_right(Align::Center).with_main_align(Align::Center);
            ui.allocate_ui_with_layout(row, layout, |ui| tack_filter_button(ui, doc));
        }
    });
    clicked
}

fn tack_filter_button(ui: &mut egui::Ui, doc: &mut OpenDoc) {
    let active = doc.filters.tack != TackFilter::All;
    let glyph = if active { icon::FUNNEL } else { icon::CHECKS };
    let response = ui
        .add(egui::Button::new(glyph).selected(active).frame(active))
        .on_hover_text(tr!("Filtrer par pointage"));
    egui::Popup::menu(&response).show(|ui| {
        crate::menu::popup_style(ui.style_mut());
        crate::menu::begin(ui);
        crate::menu::heading(ui, tr!("Afficher les lignes"));
        for filter in TackFilter::ALL {
            if crate::menu::choice(ui, doc.filters.tack == filter, filter.label()).clicked() {
                doc.filters.tack = filter;
            }
        }
    });
}
