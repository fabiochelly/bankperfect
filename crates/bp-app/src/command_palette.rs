//! Palette de commandes (Ctrl+K) : toutes les fonctions de l'application,
//! trouvées par recherche floue (insensible à la casse et aux accents).

use bp_core::tr;
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, FontId, Id, Key, Modal, Modifiers, RichText, Sense};
use egui_phosphor::regular as icon;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::commands::{Command, Info};
use crate::document::OpenDoc;

const WIDTH: f32 = 640.0;
const ROW_HEIGHT: f32 = 40.0;
const MAX_LIST_HEIGHT: f32 = 440.0;

pub struct CommandPalette {
    query: String,
    selected: usize,
    /// La sélection vient du clavier : la faire défiler jusqu'à elle.
    scroll_to_selected: bool,
    matcher: Matcher,
}

pub enum PaletteResult {
    Keep,
    Close,
    Run(Command),
}

struct Entry {
    command: Command,
    info: Info,
    /// Positions (en caractères) du libellé trouvées par la recherche.
    highlights: Vec<usize>,
    recent: bool,
}

impl Default for CommandPalette {
    fn default() -> Self {
        let mut config = Config::DEFAULT;
        config.ignore_case = true;
        config.normalize = true;
        config.prefer_prefix = true;
        Self {
            query: String::new(),
            selected: 0,
            scroll_to_selected: false,
            matcher: Matcher::new(config),
        }
    }
}

impl CommandPalette {
    fn entries(&mut self, doc: Option<&OpenDoc>, recent: &[Command]) -> Vec<Entry> {
        let commands: Vec<(Command, Info)> = Command::all(doc)
            .into_iter()
            .map(|c| (c, c.info(doc)))
            .filter(|(_, info)| info.enabled)
            .collect();
        let query = self.query.trim();
        if query.is_empty() {
            // Récemment utilisées d'abord, puis l'ordre des menus.
            let mut entries: Vec<Entry> = recent
                .iter()
                .filter_map(|r| commands.iter().position(|(c, _)| c == r))
                .map(|k| Entry {
                    command: commands[k].0,
                    info: commands[k].0.info(doc),
                    highlights: Vec::new(),
                    recent: true,
                })
                .collect();
            entries.extend(
                commands
                    .into_iter()
                    .filter(|(c, _)| !recent.contains(c))
                    .map(|(command, info)| Entry {
                        command,
                        info,
                        highlights: Vec::new(),
                        recent: false,
                    }),
            );
            return entries;
        }
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut buffer = Vec::new();
        let mut indices = Vec::new();
        let mut scored: Vec<(u32, usize, Entry)> = Vec::new();
        for (order, (command, info)) in commands.into_iter().enumerate() {
            let haystack = format!("{} {} {}", info.label, info.group, info.keywords);
            indices.clear();
            let Some(score) = pattern.indices(
                Utf32Str::new(&haystack, &mut buffer),
                &mut self.matcher,
                &mut indices,
            ) else {
                continue;
            };
            let label_len = info.label.chars().count();
            let mut highlights: Vec<usize> = indices
                .iter()
                .map(|&i| i as usize)
                .filter(|&i| i < label_len)
                .collect();
            highlights.sort_unstable();
            highlights.dedup();
            // Un mot trouvé dans le libellé passe avant un synonyme.
            let bonus = if highlights.is_empty() { 0 } else { 40 };
            scored.push((
                score + bonus,
                order,
                Entry {
                    command,
                    info,
                    highlights,
                    recent: recent.contains(&command),
                },
            ));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, _, e)| e).collect()
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: Option<&OpenDoc>,
        recent: &[Command],
    ) -> PaletteResult {
        let mut result = PaletteResult::Keep;
        let entries = self.entries(doc, recent);
        if entries.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(entries.len() - 1);
        }

        // Navigation au clavier, prise avant le champ de recherche.
        let (up, down, enter, page_up, page_down) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::PageUp),
                i.consume_key(Modifiers::NONE, Key::PageDown),
            )
        });
        if !entries.is_empty() {
            let last = entries.len() - 1;
            let before = self.selected;
            if up {
                self.selected = if self.selected == 0 {
                    last
                } else {
                    self.selected - 1
                };
            }
            if down {
                self.selected = if self.selected == last {
                    0
                } else {
                    self.selected + 1
                };
            }
            if page_up {
                self.selected = self.selected.saturating_sub(10);
            }
            if page_down {
                self.selected = (self.selected + 10).min(last);
            }
            if self.selected != before {
                self.scroll_to_selected = true;
            }
            if enter {
                result = PaletteResult::Run(entries[self.selected].command);
            }
        }

        let pal = crate::theme::palette_of(ctx);
        let area =
            Modal::default_area(Id::new("command_palette")).anchor(Align2::CENTER_TOP, [0.0, 90.0]);
        let frame = egui::Frame::new()
            .fill(pal.menu_bg)
            .stroke(egui::Stroke::new(1.0, pal.border))
            .corner_radius(14)
            .inner_margin(egui::Margin::same(8))
            .shadow(egui::Shadow {
                offset: [0, 12],
                blur: 40,
                spread: 0,
                color: Color32::from_black_alpha(120),
            });
        let modal = Modal::new(Id::new("command_palette"))
            .area(area)
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                // Champ de recherche.
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(icon::MAGNIFYING_GLASS)
                            .size(18.0)
                            .color(pal.muted),
                    );
                    let edit = egui::TextEdit::singleline(&mut self.query)
                        .id(Id::new("command_palette_query"))
                        .hint_text(tr!("Rechercher une commande…"))
                        .font(FontId::proportional(17.0))
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::symmetric(4, 10))
                        .desired_width(f32::INFINITY);
                    let response = ui.add(edit);
                    response.request_focus();
                    if response.changed() {
                        self.selected = 0;
                        self.scroll_to_selected = true;
                    }
                });
                ui.add_space(4.0);
                ui.painter().hline(
                    ui.min_rect().x_range(),
                    ui.cursor().top(),
                    egui::Stroke::new(1.0, pal.border),
                );
                ui.add_space(6.0);

                if entries.is_empty() {
                    ui.add_space(14.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(tr!("Aucune commande ne correspond")).color(pal.muted),
                        );
                    });
                    ui.add_space(14.0);
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(MAX_LIST_HEIGHT)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let mut previous_recent = None;
                            for (k, entry) in entries.iter().enumerate() {
                                if self.query.trim().is_empty()
                                    && previous_recent != Some(entry.recent)
                                {
                                    let title = if entry.recent {
                                        tr!("Récemment utilisées")
                                    } else {
                                        tr!("Toutes les commandes")
                                    };
                                    ui.add_space(if k == 0 { 2.0 } else { 8.0 });
                                    ui.label(
                                        RichText::new(format!("   {title}"))
                                            .size(11.5)
                                            .color(pal.muted),
                                    );
                                    ui.add_space(2.0);
                                    previous_recent = Some(entry.recent);
                                }
                                let response = row(ui, entry, k == self.selected, &pal);
                                if response.hovered()
                                    && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
                                {
                                    self.selected = k;
                                }
                                if response.clicked() {
                                    result = PaletteResult::Run(entry.command);
                                }
                                if k == self.selected && self.scroll_to_selected {
                                    response.scroll_to_me(None);
                                }
                            }
                        });
                    self.scroll_to_selected = false;
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    let hint = tr!(
                        "{}{}  naviguer     Entrée  exécuter     Échap  fermer",
                        icon::ARROW_UP,
                        icon::ARROW_DOWN
                    );
                    ui.label(RichText::new(hint).size(11.5).color(pal.muted));
                });
            });
        if modal.should_close() && matches!(result, PaletteResult::Keep) {
            result = PaletteResult::Close;
        }
        result
    }
}

/// Ligne de résultat : icône, libellé (lettres trouvées surlignées), groupe
/// et raccourci en touches.
fn row(
    ui: &mut egui::Ui,
    entry: &Entry,
    selected: bool,
    pal: &crate::theme::Palette,
) -> egui::Response {
    // Gouttière à droite pour la barre de défilement.
    let width = ui.available_width() - 14.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &entry.info.label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 8.0, pal.menu_hover);
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.min + egui::vec2(0.0, 9.0),
                egui::vec2(3.0, ROW_HEIGHT - 18.0),
            ),
            2.0,
            pal.accent,
        );
    }
    let center_y = rect.center().y;
    painter.text(
        egui::pos2(rect.left() + 22.0, center_y),
        Align2::CENTER_CENTER,
        entry.info.icon,
        FontId::proportional(17.0),
        if selected { pal.accent } else { pal.fg_dark },
    );

    // Libellé avec les lettres trouvées en couleur d'accent.
    let mut job = LayoutJob::default();
    let normal = TextFormat::simple(FontId::new(14.5, crate::theme::medium()), pal.fg);
    let found = TextFormat::simple(FontId::new(14.5, crate::theme::semibold()), pal.accent);
    for (k, c) in entry.info.label.chars().enumerate() {
        let format = if entry.highlights.binary_search(&k).is_ok() {
            found.clone()
        } else {
            normal.clone()
        };
        job.append(&c.to_string(), 0.0, format);
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    painter.galley(
        egui::pos2(rect.left() + 44.0, center_y - galley.size().y / 2.0),
        galley,
        pal.fg,
    );

    // À droite : raccourci en touches, puis le groupe.
    let mut x = rect.right() - 10.0;
    if let Some(shortcut) = entry.info.shortcut {
        for key in shortcut.split('+').rev() {
            let galley = painter.layout_no_wrap(
                key.to_string(),
                FontId::new(11.5, crate::theme::medium()),
                pal.fg_dark,
            );
            let w = galley.size().x + 12.0;
            let key_rect =
                egui::Rect::from_min_size(egui::pos2(x - w, center_y - 11.0), egui::vec2(w, 22.0));
            painter.rect(
                key_rect,
                5.0,
                pal.bg_dark,
                egui::Stroke::new(1.0, pal.border),
                egui::StrokeKind::Inside,
            );
            painter.galley(
                egui::pos2(key_rect.left() + 6.0, center_y - galley.size().y / 2.0),
                galley,
                pal.fg_dark,
            );
            x -= w + 4.0;
        }
        x -= 8.0;
    }
    painter.text(
        egui::pos2(x, center_y),
        Align2::RIGHT_CENTER,
        entry.info.group,
        FontId::proportional(12.0),
        pal.muted,
    );
    response
}
