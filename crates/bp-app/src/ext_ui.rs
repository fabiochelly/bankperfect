//! Rendu des boîtes de dialogue et fenêtres des extensions avec les
//! composants de l'application (cartes, champs, listes colorées…).

use std::collections::HashMap;
use std::sync::OnceLock;

use bp_core::model::Amount;
use bp_ext::ui::{Align, ButtonStyle, Color, InputKind, TableView, TextStyle, WindowSize};
use bp_ext::{Answer, Dialog, FrameMut, UiEvent, Widget, Window};
use egui::{Color32, Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::{Palette, palette};
use crate::ui_kit;

/// Réponse de l'utilisateur à la fenêtre du dessus.
pub enum Reply {
    Answer(Answer),
    Event(UiEvent),
}

/// Icône Phosphor d'après son nom (`"magnifying-glass"`).
pub fn glyph(name: Option<&str>) -> &'static str {
    static ICONS: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    let icons = ICONS.get_or_init(|| {
        icon::ICONS
            .iter()
            .map(|(name, glyph)| (name.to_ascii_lowercase().replace('_', "-"), *glyph))
            .collect()
    });
    name.and_then(|n| icons.get(&n.trim().to_ascii_lowercase().replace(['_', ' '], "-")))
        .copied()
        .unwrap_or(icon::PUZZLE_PIECE)
}

pub fn color(pal: &Palette, c: Color) -> Color32 {
    match c {
        Color::Accent => pal.accent,
        Color::Positive => pal.positive,
        Color::Negative => pal.negative,
        Color::Warning => pal.warning,
        Color::Muted => pal.muted,
        Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

fn amount_color(pal: &Palette, value: f64) -> Color32 {
    if value < 0.0 {
        pal.negative
    } else if value > 0.0 {
        pal.positive
    } else {
        pal.fg_dark
    }
}

fn styled(pal: &Palette, text: &str, style: TextStyle) -> RichText {
    let rt = RichText::new(text);
    match style {
        TextStyle::Body => rt.color(pal.fg),
        TextStyle::Muted => rt.color(pal.fg_dark),
        TextStyle::Strong => rt.family(crate::theme::semibold()).color(pal.fg),
        TextStyle::Heading => rt.size(16.0).family(crate::theme::semibold()).color(pal.fg),
        TextStyle::Small => rt.size(12.0).color(pal.muted),
        TextStyle::Mono => rt.monospace().color(pal.fg),
    }
}

/// Affiche la pile des fenêtres d'extension ; seule la dernière répond.
pub fn show(
    ctx: &egui::Context,
    frames: Vec<FrameMut<'_>>,
    doc: Option<&OpenDoc>,
) -> Option<Reply> {
    let count = frames.len();
    let mut reply = None;
    for (i, frame) in frames.into_iter().enumerate() {
        let top = i + 1 == count;
        let r = match frame {
            FrameMut::Dialog(dialog) => dialog_ui(ctx, i, dialog).map(Reply::Answer),
            FrameMut::Window(window) => window_ui(ctx, i, window, doc, top).map(Reply::Event),
        };
        if top {
            reply = r;
        }
    }
    reply
}

fn dialog_ui(ctx: &egui::Context, index: usize, dialog: &mut Dialog) -> Option<Answer> {
    let mut answer = None;
    crate::dialogs::modal(ctx, &format!("ext_dialog_{index}")).show(ctx, |ui| {
        let pal = palette(ui);
        ui.set_width(ui_kit::dialog_width(ctx, 0.34, 460.0, 540.0));
        match dialog {
            Dialog::Alert { title, message } => {
                crate::dialogs::header(ui, icon::INFO, pal.accent, title);
                ui.label(RichText::new(message.as_str()).color(pal.fg_dark));
                crate::dialogs::footer(ui, |ui| {
                    if crate::dialogs::primary(ui, "OK", true).clicked() {
                        answer = Some(Answer::Ok);
                    }
                });
                if ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) {
                    answer = Some(Answer::Ok);
                }
            }
            Dialog::Confirm {
                title,
                message,
                ok,
                danger,
            } => {
                let tint = if *danger { pal.negative } else { pal.warning };
                crate::dialogs::header(ui, icon::QUESTION, tint, title);
                ui.label(RichText::new(message.as_str()).color(pal.fg_dark));
                crate::dialogs::footer(ui, |ui| {
                    let clicked = if *danger {
                        crate::dialogs::danger(ui, ok).clicked()
                    } else {
                        crate::dialogs::primary(ui, ok, true).clicked()
                    };
                    if clicked {
                        answer = Some(Answer::Ok);
                    }
                    if crate::dialogs::button(ui, "Annuler").clicked() {
                        answer = Some(Answer::Cancel);
                    }
                });
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    answer = Some(Answer::Cancel);
                } else if !*danger && ui.input(|i| i.key_pressed(Key::Enter)) {
                    answer = Some(Answer::Ok);
                }
            }
            Dialog::Prompt {
                title,
                label,
                text,
                placeholder,
                ok,
            } => {
                crate::dialogs::header(ui, icon::PENCIL_SIMPLE, pal.accent, title);
                if !label.is_empty() {
                    ui.label(RichText::new(label.as_str()).color(pal.fg_dark));
                    ui.add_space(8.0);
                }
                let mut field = crate::theme::field(text).desired_width(f32::INFINITY);
                if let Some(p) = placeholder {
                    field = field.hint_text(p.as_str());
                }
                let response = ui.add(field);
                if !response.has_focus() && ui.memory(|m| m.focused().is_none()) {
                    response.request_focus();
                }
                crate::dialogs::footer(ui, |ui| {
                    if crate::dialogs::primary(ui, ok, true).clicked() {
                        answer = Some(Answer::Ok);
                    }
                    if crate::dialogs::button(ui, "Annuler").clicked() {
                        answer = Some(Answer::Cancel);
                    }
                });
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    answer = Some(Answer::Ok);
                } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                    answer = Some(Answer::Cancel);
                }
            }
        }
    });
    answer
}

fn window_width(ctx: &egui::Context, size: WindowSize) -> f32 {
    match size {
        WindowSize::Small => ui_kit::dialog_width(ctx, 0.36, 480.0, 560.0),
        WindowSize::Medium => ui_kit::dialog_width(ctx, 0.5, 600.0, 780.0),
        WindowSize::Large => ui_kit::dialog_width(ctx, 0.7, 760.0, 1120.0),
        WindowSize::Fraction(f) => ui_kit::dialog_width(ctx, f, 480.0, 1600.0),
    }
}

/// Contexte de rendu d'une fenêtre.
struct Painter<'a> {
    doc: Option<&'a OpenDoc>,
    pal: Palette,
    event: Option<UiEvent>,
    /// Identifiant de la fenêtre (champs, tableaux).
    salt: u64,
}

fn window_ui(
    ctx: &egui::Context,
    index: usize,
    window: &Window,
    doc: Option<&OpenDoc>,
    top: bool,
) -> Option<UiEvent> {
    let pal = crate::theme::palette_of(ctx);
    let mut painter = Painter {
        doc,
        pal,
        event: None,
        salt: window.id,
    };
    let width = window_width(ctx, window.size);
    crate::dialogs::modal(ctx, &format!("ext_window_{index}")).show(ctx, |ui| {
        ui.set_width(width);
        crate::dialogs::header(ui, glyph(window.icon.as_deref()), pal.accent, &window.title);
        if let Some(subtitle) = &window.subtitle {
            ui_kit::subtitle(ui, subtitle.as_str());
        }
        egui::ScrollArea::vertical()
            .id_salt(("ext_body", window.id))
            .max_height(ui_kit::dialog_height(ctx, 0.68))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.set_width(width);
                painter.widgets(ui, &window.body, width);
            });
        crate::dialogs::footer(ui, |ui| {
            // Pied disposé de droite à gauche : le dernier bouton est à droite.
            for w in window.footer.iter().rev() {
                painter.widget(ui, w, 0.0);
            }
        });
    });
    if top && painter.event.is_none() && ctx.input(|i| i.key_pressed(Key::Escape)) {
        painter.event = Some(UiEvent::Dismiss);
    }
    if top { painter.event } else { None }
}

impl Painter<'_> {
    fn emit(&mut self, event: UiEvent) {
        if self.event.is_none() {
            self.event = Some(event);
        }
    }

    fn widgets(&mut self, ui: &mut egui::Ui, widgets: &[Widget], width: f32) {
        for (i, w) in widgets.iter().enumerate() {
            if i > 0 && !matches!(w, Widget::Section { .. } | Widget::Space(_)) {
                ui.add_space(8.0);
            }
            self.widget(ui, w, width);
        }
    }

    fn widget(&mut self, ui: &mut egui::Ui, widget: &Widget, width: f32) {
        let pal = self.pal;
        match widget {
            Widget::Text {
                text,
                style,
                color,
                align,
            } => {
                let mut rt = styled(&pal, text, *style);
                if let Some(c) = color {
                    rt = rt.color(self::color(&pal, *c));
                }
                let label = egui::Label::new(rt).wrap();
                match align {
                    Align::Left => {
                        ui.add(label);
                    }
                    Align::Center => {
                        ui.vertical_centered(|ui| ui.add(label));
                    }
                    Align::Right => {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                            ui.add(label)
                        });
                    }
                }
            }
            Widget::Money {
                amount,
                currency,
                style,
                colored,
            } => {
                let text = money(
                    Amount((amount * Amount::SCALE as f64).round() as i64),
                    currency,
                );
                let mut rt = styled(&pal, &text, *style);
                if *colored {
                    rt = rt.color(amount_color(&pal, *amount));
                }
                ui.label(rt);
            }
            Widget::Section {
                title,
                icon: glyph_name,
                children,
            } => {
                let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
                match title {
                    Some(title) => {
                        let g = glyph(glyph_name.as_deref().or(Some("squares-four")));
                        ui_kit::section(ui, width, g, title, |ui| {
                            self.widgets(ui, children, inner)
                        });
                    }
                    None => {
                        ui_kit::card(ui, width, |ui| self.widgets(ui, children, inner));
                        ui.add_space(14.0);
                    }
                }
            }
            Widget::Row { children, align } => {
                let n = children.len().max(1) as f32;
                let spacing = 10.0;
                let tall = children
                    .iter()
                    .any(|c| matches!(c, Widget::Button { .. } | Widget::Input(_)));
                let child_width = ((width - spacing * (n - 1.0)) / n).max(40.0);
                let layout = if *align == Align::Right {
                    egui::Layout::right_to_left(egui::Align::Min)
                } else {
                    egui::Layout::left_to_right(egui::Align::Min)
                };
                ui.allocate_ui_with_layout(egui::vec2(width, 0.0), layout, |ui| {
                    ui.spacing_mut().item_spacing.x = spacing;
                    let items: Vec<&Widget> = if *align == Align::Right {
                        children.iter().rev().collect()
                    } else {
                        children.iter().collect()
                    };
                    for child in items {
                        let w = if matches!(
                            child,
                            Widget::Button { .. }
                                | Widget::Pill { .. }
                                | Widget::Text { .. }
                                | Widget::Money { .. }
                        ) {
                            0.0
                        } else {
                            child_width
                        };
                        if w > 0.0 {
                            ui.allocate_ui_with_layout(
                                egui::vec2(w, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_width(w);
                                    self.widget(ui, child, w)
                                },
                            );
                        } else if tall
                            && matches!(
                                child,
                                Widget::Text { .. } | Widget::Money { .. } | Widget::Pill { .. }
                            )
                        {
                            // Texte à mi-hauteur des boutons et des champs.
                            ui.vertical(|ui| {
                                ui.add_space(if matches!(child, Widget::Pill { .. }) {
                                    5.0
                                } else {
                                    7.0
                                });
                                self.widget(ui, child, w)
                            });
                        } else {
                            self.widget(ui, child, w);
                        }
                    }
                });
            }
            Widget::Columns(children) => {
                let n = children.len().max(1);
                let spacing = 14.0;
                let column = (width - spacing * (n as f32 - 1.0)) / n as f32;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = spacing;
                    for child in children {
                        ui.allocate_ui_with_layout(
                            egui::vec2(column, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(column);
                                self.widget(ui, child, column);
                            },
                        );
                    }
                });
            }
            Widget::Input(input) => self.input(ui, input, width),
            Widget::Button {
                id,
                label,
                icon: glyph_name,
                style,
                enabled,
            } => {
                let text = match glyph_name {
                    Some(g) => format!("{}  {label}", glyph(Some(g))),
                    None => label.clone(),
                };
                let response = match style {
                    ButtonStyle::Primary => crate::dialogs::primary(ui, &text, *enabled),
                    ButtonStyle::Danger => {
                        let button = egui::Button::new(
                            RichText::new(&text)
                                .family(crate::theme::medium())
                                .color(Color32::WHITE),
                        )
                        .fill(pal.negative)
                        .min_size(egui::vec2(110.0, 36.0));
                        ui.add_enabled(*enabled, button)
                    }
                    ButtonStyle::Link => ui.add_enabled(*enabled, egui::Link::new(text)),
                    ButtonStyle::Normal => ui.add_enabled(
                        *enabled,
                        egui::Button::new(text).min_size(egui::vec2(100.0, 36.0)),
                    ),
                };
                if response.clicked() {
                    self.emit(UiEvent::Click { button: *id });
                }
            }
            Widget::Table(table) => self.table(ui, table, width),
            Widget::Stat {
                label,
                value,
                icon: glyph_name,
                color,
                hint,
            } => {
                let tint = color.map_or(pal.fg, |c| self::color(&pal, c));
                egui::Frame::new()
                    .fill(pal.bg)
                    .stroke(egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(16, 14))
                    .show(ui, |ui| {
                        ui.set_width((width - 34.0).max(60.0));
                        let caption = match glyph_name {
                            Some(g) => format!("{}  {}", glyph(Some(g)), label.to_uppercase()),
                            None => label.to_uppercase(),
                        };
                        ui.label(
                            RichText::new(caption)
                                .size(11.5)
                                .family(crate::theme::semibold())
                                .color(pal.muted),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(value.as_str())
                                .size(20.0)
                                .family(crate::theme::semibold())
                                .color(tint),
                        );
                        if let Some(hint) = hint {
                            ui.label(RichText::new(hint.as_str()).size(12.0).color(pal.fg_dark));
                        }
                    });
            }
            Widget::Pill { text, color } => ui_kit::pill(ui, text, self::color(&pal, *color)),
            Widget::Notice {
                text,
                color,
                icon: glyph_name,
            } => {
                let tint = self::color(&pal, *color);
                egui::Frame::new()
                    .fill(tint.gamma_multiply(0.12))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_width((width - 28.0).max(60.0));
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(glyph(glyph_name.as_deref().or(Some("info"))))
                                    .size(16.0)
                                    .color(tint),
                            );
                            ui.add(
                                egui::Label::new(RichText::new(text.as_str()).color(pal.fg)).wrap(),
                            );
                        });
                    });
            }
            Widget::Progress {
                fraction,
                text,
                color,
            } => {
                let mut bar = egui::ProgressBar::new(*fraction)
                    .desired_width(width)
                    .desired_height(10.0)
                    .corner_radius(5.0)
                    .fill(color.map_or(pal.accent, |c| self::color(&pal, c)));
                if let Some(text) = text {
                    ui.label(RichText::new(text.as_str()).size(12.5).color(pal.fg_dark));
                    ui.add_space(4.0);
                    bar = bar.desired_height(10.0);
                }
                ui.add(bar);
            }
            Widget::Chart {
                style,
                data,
                currency,
                height,
            } => {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, *height), egui::Sense::hover());
                let format = |v: f64| match data.unit {
                    bp_core::reports::ChartUnit::Amount => crate::format::money(
                        bp_core::model::Amount::from_cents((v * 100.0).round() as i64),
                        currency,
                    ),
                    bp_core::reports::ChartUnit::Count => format!("{}", v.round() as i64),
                };
                let empty = data.series.iter().all(|s| s.points.is_empty());
                if empty {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Aucune donnée",
                        egui::FontId::proportional(14.0),
                        pal.muted,
                    );
                } else {
                    match style {
                        bp_ext::ui::ChartStyle::Line => {
                            crate::charts_ui::curve(ui, rect, data, &format, &pal, 1.0)
                        }
                        bp_ext::ui::ChartStyle::Columns => {
                            crate::charts_ui::columns(ui, rect, data, &format, &pal, 1.0)
                        }
                        bp_ext::ui::ChartStyle::Bars => {
                            crate::charts_ui::horizontal_bars(ui, rect, data, &format, &pal, 1.0)
                        }
                        bp_ext::ui::ChartStyle::Donut => {
                            let points: Vec<(String, f64)> = data.series[0]
                                .points
                                .iter()
                                .filter(|p| p.value != 0.0)
                                .map(|p| (p.label.clone(), p.value.abs()))
                                .collect();
                            crate::charts_ui::donut(ui, rect, &points, &format, &pal, 1.0)
                        }
                    }
                }
            }
            Widget::Bars(bars) => {
                let max = bars
                    .iter()
                    .map(|b| b.value.abs())
                    .fold(0.0, f64::max)
                    .max(1e-9);
                let label_width = (width * 0.32).min(220.0);
                let value_width = 120.0;
                let bar_width = (width - label_width - value_width - 16.0).max(40.0);
                for b in bars {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui_kit::cell(
                            ui,
                            label_width,
                            egui::Align::Min,
                            RichText::new(&b.label).color(pal.fg),
                        );
                        let (rect, _) = ui
                            .allocate_exact_size(egui::vec2(bar_width, 22.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                rect.min + egui::vec2(0.0, 5.0),
                                egui::vec2(bar_width, 12.0),
                            ),
                            6.0,
                            pal.bg_dark,
                        );
                        let fill = b.color.map_or_else(
                            || {
                                if b.value < 0.0 {
                                    pal.negative
                                } else {
                                    pal.accent
                                }
                            },
                            |c| self::color(&pal, c),
                        );
                        let w = (b.value.abs() / max) as f32 * bar_width;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                rect.min + egui::vec2(0.0, 5.0),
                                egui::vec2(w.max(3.0), 12.0),
                            ),
                            6.0,
                            fill,
                        );
                        ui_kit::cell(
                            ui,
                            value_width,
                            egui::Align::Max,
                            RichText::new(&b.text).color(pal.fg_dark),
                        );
                    });
                }
            }
            Widget::Separator => {
                ui.add_space(4.0);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
                ui.painter().hline(
                    rect.x_range(),
                    rect.center().y,
                    egui::Stroke::new(1.0, pal.border),
                );
                ui.add_space(4.0);
            }
            Widget::Space(size) => ui.add_space(*size),
        }
    }

    fn input(&mut self, ui: &mut egui::Ui, input: &bp_ext::ui::Input, width: f32) {
        let pal = self.pal;
        const LABEL: f32 = 170.0;
        let has_label = input.label.is_some() && width > 360.0;
        let field_width = input
            .width
            .unwrap_or(if has_label {
                width - LABEL - 8.0
            } else {
                width
            })
            .min(width);
        let show = |painter: &mut Self, ui: &mut egui::Ui| {
            ui.vertical(|ui| {
                painter.field(ui, input, field_width);
                if input.invalid {
                    ui.label(
                        RichText::new("Valeur non reconnue")
                            .size(12.0)
                            .color(pal.negative),
                    );
                } else if let Some(hint) = &input.hint {
                    ui.label(RichText::new(hint.as_str()).size(12.0).color(pal.muted));
                }
            });
        };
        match &input.label {
            Some(label) if has_label => {
                ui_kit::row(ui, label, LABEL, |ui| show(self, ui));
            }
            Some(label) => {
                ui.label(
                    RichText::new(label.as_str())
                        .family(crate::theme::medium())
                        .color(pal.fg),
                );
                ui.add_space(4.0);
                show(self, ui);
            }
            None => show(self, ui),
        }
    }

    fn field(&mut self, ui: &mut egui::Ui, input: &bp_ext::ui::Input, width: f32) {
        let pal = self.pal;
        let id = ("ext_field", self.salt, input.key.as_str());
        ui.add_enabled_ui(input.enabled, |ui| match &input.kind {
            InputKind::Text {
                multiline,
                password,
            } => {
                let mut text = input.text.clone();
                let mut edit = if *multiline {
                    egui::TextEdit::multiline(&mut text)
                        .desired_rows(4)
                        .margin(egui::Margin::symmetric(10, 8))
                } else {
                    crate::theme::field(&mut text).password(*password)
                };
                edit = edit.id_salt(id).desired_width(width);
                if let Some(p) = &input.placeholder {
                    edit = edit.hint_text(p.as_str());
                }
                if ui.add(edit).changed() {
                    self.emit(UiEvent::Text {
                        key: input.key.clone(),
                        text,
                    });
                }
            }
            InputKind::Number { .. } | InputKind::Amount => {
                let mut text = input.text.clone();
                let mut edit = crate::theme::field(&mut text)
                    .id_salt(id)
                    .desired_width(width)
                    .horizontal_align(egui::Align::Max);
                if let Some(p) = &input.placeholder {
                    edit = edit.hint_text(p.as_str());
                }
                if input.invalid {
                    edit = edit.text_color(pal.negative);
                }
                if ui.add(edit).changed() {
                    self.emit(UiEvent::Text {
                        key: input.key.clone(),
                        text,
                    });
                }
            }
            InputKind::Date => {
                let mut text = input.text.clone();
                let response = crate::datepicker::date_field(
                    ui,
                    id,
                    &mut text,
                    width,
                    crate::datepicker::Placement::Popup,
                );
                if response.changed() {
                    self.emit(UiEvent::Text {
                        key: input.key.clone(),
                        text,
                    });
                }
            }
            InputKind::Checkbox => {
                let mut checked = input.checked;
                let response = ui
                    .allocate_ui_with_layout(
                        egui::vec2(width, 36.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| crate::theme::checkbox(ui, &mut checked, input.text.as_str()),
                    )
                    .inner;
                if response.changed() {
                    self.emit(UiEvent::Bool {
                        key: input.key.clone(),
                        value: checked,
                    });
                }
            }
            InputKind::Switch => {
                let mut checked = input.checked;
                let response = ui
                    .allocate_ui_with_layout(
                        egui::vec2(width, 36.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| switch(ui, &mut checked, &input.text, &pal),
                    )
                    .inner;
                if response.changed() {
                    self.emit(UiEvent::Bool {
                        key: input.key.clone(),
                        value: checked,
                    });
                }
            }
            InputKind::Select { options, selected } => {
                let mut current = *selected;
                let text = current.and_then(|i| options.get(i)).map_or_else(
                    || {
                        RichText::new(input.placeholder.clone().unwrap_or_default())
                            .color(pal.muted)
                    },
                    |o| RichText::new(o.as_str()),
                );
                egui::ComboBox::from_id_salt(id)
                    .selected_text(text)
                    .width(width)
                    .height(420.0)
                    .show_ui(ui, |ui| {
                        for (i, option) in options.iter().enumerate() {
                            ui.selectable_value(&mut current, Some(i), option.as_str());
                        }
                    });
                if current != *selected {
                    self.emit(UiEvent::Pick {
                        key: input.key.clone(),
                        index: current,
                    });
                }
            }
            InputKind::Choice { options, selected } => {
                let mut current = selected.unwrap_or(usize::MAX);
                let glyphs: Vec<&'static str> = options
                    .iter()
                    .map(|(_, g)| g.as_deref().map_or("", |g| glyph(Some(g))))
                    .collect();
                let list: Vec<(usize, &str, &str, Color32)> = options
                    .iter()
                    .enumerate()
                    .map(|(i, (label, _))| (i, glyphs[i], label.as_str(), pal.accent))
                    .collect();
                if ui_kit::segmented(ui, &mut current, &list, 36.0, Some(width)) {
                    self.emit(UiEvent::Pick {
                        key: input.key.clone(),
                        index: Some(current),
                    });
                }
            }
            InputKind::Account { selected } => {
                let Some(doc) = self.doc else { return };
                let document = &doc.document;
                let mut current = *selected;
                let text = current.and_then(|i| document.accounts.get(i)).map_or_else(
                    || {
                        RichText::new(
                            input
                                .placeholder
                                .clone()
                                .unwrap_or_else(|| "Choisir un compte".into()),
                        )
                        .color(pal.muted)
                    },
                    |a| RichText::new(format!("{}  {}", icon::BANK, a.name)),
                );
                egui::ComboBox::from_id_salt(id)
                    .selected_text(text)
                    .width(width)
                    .height(420.0)
                    .show_ui(ui, |ui| {
                        for &i in &document.account_order {
                            let account = &document.accounts[i];
                            if account.disabled {
                                continue;
                            }
                            ui.selectable_value(
                                &mut current,
                                Some(i),
                                format!("{}  {}", icon::BANK, account.name),
                            );
                        }
                    });
                if current != *selected {
                    self.emit(UiEvent::Pick {
                        key: input.key.clone(),
                        index: current,
                    });
                }
            }
            InputKind::Category { selected } => {
                let Some(doc) = self.doc else { return };
                let mut current = *selected;
                if ui_kit::category_combo(
                    ui,
                    &format!("ext_cat_{}_{}", self.salt, input.key),
                    &doc.document,
                    &doc.category_styles,
                    &mut current,
                    width,
                ) {
                    self.emit(UiEvent::Category {
                        key: input.key.clone(),
                        id: current,
                    });
                }
            }
            InputKind::Mode { selected } => {
                let Some(doc) = self.doc else { return };
                let mut current = selected.clone().unwrap_or_default();
                if ui_kit::mode_combo(
                    ui,
                    &format!("ext_mode_{}_{}", self.salt, input.key),
                    &doc.document,
                    &mut current,
                    width,
                ) {
                    self.emit(UiEvent::Mode {
                        key: input.key.clone(),
                        mode: current,
                    });
                }
            }
        });
    }

    fn table(&mut self, ui: &mut egui::Ui, table: &TableView, width: f32) {
        const ROW: f32 = 32.0;
        let pal = self.pal;
        let inner = width - 8.0;
        // Colonnes à largeur fixe, le reste partagé entre les autres.
        let fixed: f32 = table.columns.iter().filter_map(|c| c.width).sum();
        let flexible = table
            .columns
            .iter()
            .filter(|c| c.width.is_none())
            .count()
            .max(1);
        let share = ((inner - fixed) / flexible as f32).max(40.0);
        let widths: Vec<f32> = table
            .columns
            .iter()
            .map(|c| c.width.unwrap_or(share))
            .collect();
        let align = |a: Align| match a {
            Align::Right => egui::Align::Max,
            Align::Center => egui::Align::Center,
            Align::Left => egui::Align::Min,
        };
        if !table.columns.is_empty() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(4.0);
                for (c, w) in table.columns.iter().zip(&widths) {
                    ui_kit::cell(
                        ui,
                        *w,
                        align(c.align),
                        RichText::new(&c.title)
                            .family(crate::theme::semibold())
                            .color(pal.fg_dark),
                    );
                }
            });
            let (rect, _) = ui.allocate_exact_size(egui::vec2(inner, 1.0), egui::Sense::hover());
            ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                egui::Stroke::new(1.0, pal.border),
            );
        }
        if table.rows.is_empty() {
            ui.add_space(18.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(&table.empty).color(pal.muted));
            });
            ui.add_space(18.0);
            return;
        }
        let height = table.height.unwrap_or_else(|| {
            (table.rows.len() as f32 * ROW).min(ui_kit::dialog_height(ui.ctx(), 0.42))
        });
        let anchor_id = egui::Id::new(("ext_table_anchor", self.salt, table.id));
        egui::ScrollArea::vertical()
            .id_salt(("ext_table", self.salt, table.id))
            .max_height(height)
            .min_scrolled_height(height.min(table.rows.len() as f32 * ROW))
            .auto_shrink([false, true])
            .show_rows(ui, ROW, table.rows.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for r in range {
                    let sense = if table.selectable || table.activable {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    };
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(inner, ROW), sense);
                    let selected = table.selected.contains(&r);
                    if selected {
                        ui.painter().rect_filled(rect, 6.0, pal.selection);
                    } else if response.hovered() && table.selectable {
                        ui.painter().rect_filled(rect, 6.0, pal.bg_highlight);
                    } else if r % 2 == 1 {
                        ui.painter()
                            .rect_filled(rect, 6.0, pal.stripe.gamma_multiply(0.7));
                    }
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect.shrink2(egui::vec2(4.0, 0.0)))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    child.spacing_mut().item_spacing.x = 0.0;
                    for (k, cell) in table.rows[r].iter().enumerate() {
                        let w = widths.get(k).copied().unwrap_or(share);
                        let column_align = table.columns.get(k).map_or(Align::Left, |c| c.align);
                        let mut text = match &cell.icon {
                            Some(g) => RichText::new(format!("{}  {}", glyph(Some(g)), cell.text)),
                            None => RichText::new(&cell.text),
                        };
                        text = text.color(match (cell.color, cell.amount) {
                            (Some(c), _) => color(&pal, c),
                            (None, Some(v)) => amount_color(&pal, v),
                            (None, None) => pal.fg,
                        });
                        if cell.strong {
                            text = text.family(crate::theme::semibold());
                        }
                        ui_kit::cell(
                            &mut child,
                            w,
                            align(cell.align.unwrap_or(column_align)),
                            text,
                        );
                    }
                    if response.double_clicked() && table.activable {
                        self.emit(UiEvent::TableActivate {
                            table: table.id,
                            row: r,
                        });
                    } else if response.clicked() && table.selectable {
                        let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                        let mut rows = table.selected.clone();
                        let anchor: Option<usize> = ui.data(|d| d.get_temp(anchor_id));
                        if table.multiple
                            && shift
                            && let Some(a) = anchor
                        {
                            rows = (a.min(r)..=a.max(r)).collect();
                        } else if table.multiple && ctrl {
                            if let Some(p) = rows.iter().position(|&x| x == r) {
                                rows.remove(p);
                            } else {
                                rows.push(r);
                            }
                            ui.data_mut(|d| d.insert_temp(anchor_id, r));
                        } else {
                            rows = vec![r];
                            ui.data_mut(|d| d.insert_temp(anchor_id, r));
                        }
                        self.emit(UiEvent::TableSelect {
                            table: table.id,
                            rows,
                        });
                    }
                }
            });
    }
}

/// Interrupteur (case à cocher en forme de glissière).
fn switch(ui: &mut egui::Ui, on: &mut bool, label: &str, pal: &Palette) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::TextStyle::Body.resolve(ui.style()),
        pal.fg,
    );
    let size = egui::vec2(38.0 + 10.0 + galley.size().x, 24.0_f32.max(galley.size().y));
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool(response.id, *on);
    let track = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - 11.0),
        egui::vec2(38.0, 22.0),
    );
    let fill = if *on { pal.accent } else { pal.bg_dark };
    ui.painter().rect(
        track,
        11.0,
        fill,
        egui::Stroke::new(1.0, if *on { pal.accent } else { pal.muted }),
        egui::StrokeKind::Inside,
    );
    let x = egui::lerp(track.left() + 11.0..=track.right() - 11.0, t);
    ui.painter().circle_filled(
        egui::pos2(x, track.center().y),
        8.0,
        if *on { pal.on_accent } else { pal.fg_dark },
    );
    ui.painter().galley(
        egui::pos2(
            track.right() + 10.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        if ui.is_enabled() { pal.fg } else { pal.muted },
    );
    response
}
