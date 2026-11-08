//! Aide intégrée (F1) : un guide d'utilisation par rubriques, hors ligne,
//! en français ou en anglais selon la langue de l'interface.
//!
//! Le guide est écrit dans `help/fr.md` et `help/en.md`, dans un petit
//! sous-ensemble de Markdown : `## rubrique`, `### sous-titre`, paragraphes
//! d'une ligne, listes `- ` et `1. `, encadrés `> `, `**gras**` et
//! `` `touche` ``.

use bp_core::i18n::{self, Lang};
use bp_core::tr;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, RichText};
use egui_phosphor::regular as icon;

use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

const GUIDE_FR: &str = include_str!("../help/fr.md");
const GUIDE_EN: &str = include_str!("../help/en.md");

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Heading(String),
    Paragraph(String),
    Bullet(String),
    Numbered(String, String),
    Tip(String),
}

#[derive(Debug, Clone)]
struct Topic {
    title: String,
    blocks: Vec<Block>,
}

fn parse(text: &str) -> Vec<Topic> {
    let mut topics: Vec<Topic> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(title) = line.strip_prefix("## ") {
            topics.push(Topic {
                title: title.trim().to_string(),
                blocks: Vec::new(),
            });
            continue;
        }
        let Some(topic) = topics.last_mut() else {
            continue;
        };
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let block = if let Some(h) = trimmed.strip_prefix("### ") {
            Block::Heading(h.trim().to_string())
        } else if let Some(b) = trimmed.strip_prefix("- ") {
            Block::Bullet(b.trim().to_string())
        } else if let Some(t) = trimmed.strip_prefix("> ") {
            Block::Tip(t.trim().to_string())
        } else if let Some((n, rest)) = trimmed.split_once(". ")
            && !n.is_empty()
            && n.chars().all(|c| c.is_ascii_digit())
        {
            Block::Numbered(n.to_string(), rest.trim().to_string())
        } else {
            Block::Paragraph(trimmed.to_string())
        };
        topic.blocks.push(block);
    }
    topics
}

/// Morceaux d'une ligne : texte, gras ou touche.
#[derive(Debug, PartialEq)]
enum Span<'a> {
    Text(&'a str),
    Bold(&'a str),
    Key(&'a str),
}

fn spans(text: &str) -> Vec<Span<'_>> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let bold = rest.find("**");
        let key = rest.find('`');
        let next = match (bold, key) {
            (Some(b), Some(k)) => Some(b.min(k)),
            (b, k) => b.or(k),
        };
        let Some(at) = next else {
            out.push(Span::Text(rest));
            break;
        };
        if at > 0 {
            out.push(Span::Text(&rest[..at]));
        }
        let tail = &rest[at..];
        let bold = tail.starts_with("**");
        let marker = if bold { "**" } else { "`" };
        let inner = &tail[marker.len()..];
        match inner.find(marker) {
            Some(end) => {
                let content = &inner[..end];
                out.push(if bold {
                    Span::Bold(content)
                } else {
                    Span::Key(content)
                });
                rest = &inner[end + marker.len()..];
            }
            None => {
                out.push(Span::Text(tail));
                break;
            }
        }
    }
    out
}

fn inline_job(ui: &egui::Ui, text: &str, size: f32, color: Color32, width: f32) -> LayoutJob {
    let pal = palette(ui);
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let regular = TextFormat {
        font_id: FontId::proportional(size),
        color,
        line_height: Some(size * 1.55),
        ..Default::default()
    };
    for span in spans(text) {
        match span {
            Span::Text(t) => job.append(t, 0.0, regular.clone()),
            Span::Bold(t) => job.append(
                t,
                0.0,
                TextFormat {
                    font_id: FontId::new(size, crate::theme::semibold()),
                    color: pal.fg,
                    ..regular.clone()
                },
            ),
            Span::Key(t) => {
                // Touche : fond discret, sans espace à l'intérieur (la
                // touche ne se coupe pas en fin de ligne).
                job.append(
                    t,
                    2.0,
                    TextFormat {
                        font_id: FontId::monospace(size - 1.5),
                        color: pal.fg,
                        background: pal.bg_highlight,
                        ..regular.clone()
                    },
                );
                job.append("", 2.0, regular.clone());
            }
        }
    }
    job
}

pub struct HelpForm {
    selected: usize,
    search: String,
    /// Remonter en haut du texte au prochain affichage (rubrique changée).
    scroll_top: bool,
}

impl HelpForm {
    pub fn new() -> Self {
        Self {
            selected: 0,
            search: String::new(),
            scroll_top: false,
        }
    }

    pub fn show_standalone(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Keep;
        let topics = parse(match i18n::lang() {
            Lang::En => GUIDE_EN,
            _ => GUIDE_FR,
        });
        self.selected = self.selected.min(topics.len().saturating_sub(1));
        let width = ui_kit::dialog_width(ctx, 0.75, 860.0, 1240.0);
        let height = ui_kit::dialog_height(ctx, 0.82).max(520.0);
        crate::dialogs::modal(ctx, "help").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::QUESTION, pal.accent, tr!("Aide"));
            let body = height - 36.0 - 14.0 - 80.0;
            let list_width = 250.0;
            let gap = 22.0;
            let text_width = width - list_width - gap;
            let query = bp_core::filter::normalize(self.search.trim());
            let matches = |t: &Topic| {
                query.is_empty()
                    || bp_core::filter::normalize(&t.title).contains(&query)
                    || t.blocks.iter().any(|b| {
                        let s = match b {
                            Block::Heading(s)
                            | Block::Paragraph(s)
                            | Block::Bullet(s)
                            | Block::Tip(s)
                            | Block::Numbered(_, s) => s,
                        };
                        bp_core::filter::normalize(s).contains(&query)
                    })
            };
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.vertical(|ui| {
                    ui.set_width(list_width);
                    ui.set_height(body);
                    ui.add(
                        crate::theme::field(&mut self.search)
                            .hint_text(format!("{}  {}", icon::MAGNIFYING_GLASS, tr!("Rechercher")))
                            .desired_width(list_width),
                    );
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .id_salt("help_topics")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (k, topic) in topics.iter().enumerate() {
                                if !matches(topic) {
                                    continue;
                                }
                                let selected = self.selected == k;
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(list_width - 8.0, 34.0),
                                    egui::Sense::click(),
                                );
                                if selected {
                                    ui.painter().rect_filled(
                                        rect,
                                        8.0,
                                        pal.accent.gamma_multiply(0.16),
                                    );
                                } else if response.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        8.0,
                                        pal.bg_highlight.gamma_multiply(0.6),
                                    );
                                }
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::WidgetType::SelectableLabel,
                                        true,
                                        selected,
                                        &topic.title,
                                    )
                                });
                                let galley = ui.painter().layout(
                                    topic.title.clone(),
                                    FontId::new(14.0, crate::theme::medium()),
                                    if selected { pal.fg } else { pal.fg_dark },
                                    rect.width() - 20.0,
                                );
                                ui.painter().galley(
                                    egui::pos2(
                                        rect.left() + 12.0,
                                        rect.center().y - galley.size().y / 2.0,
                                    ),
                                    galley,
                                    pal.fg,
                                );
                                if response.clicked() {
                                    self.selected = k;
                                    self.scroll_top = true;
                                }
                            }
                        });
                });
                ui.vertical(|ui| {
                    ui.set_width(text_width);
                    ui.set_height(body);
                    let mut area = egui::ScrollArea::vertical()
                        .id_salt("help_text")
                        .auto_shrink([false, false]);
                    if std::mem::take(&mut self.scroll_top) {
                        area = area.vertical_scroll_offset(0.0);
                    }
                    area.show(ui, |ui| {
                        if let Some(topic) = topics.get(self.selected) {
                            topic_ui(ui, topic, text_width - 24.0);
                        }
                    });
                });
            });
            crate::dialogs::footer(ui, |ui| {
                if crate::dialogs::primary(ui, tr!("Fermer"), true).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    outcome = Outcome::Close;
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}  {}",
                            icon::COMMAND,
                            tr!("Toutes les commandes, avec leur raccourci : Ctrl+P ou F1")
                        ))
                        .small()
                        .color(pal.muted),
                    );
                });
            });
        });
        outcome
    }
}

fn topic_ui(ui: &mut egui::Ui, topic: &Topic, width: f32) {
    let pal = palette(ui);
    // Lignes de longueur confortable, même dans une grande fenêtre.
    let width = width.min(780.0);
    ui.set_max_width(width);

    ui.label(
        RichText::new(&topic.title)
            .size(22.0)
            .family(crate::theme::semibold())
            .color(pal.fg),
    );
    ui.add_space(10.0);
    for block in &topic.blocks {
        match block {
            Block::Heading(text) => {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(text)
                        .size(16.5)
                        .family(crate::theme::semibold())
                        .color(pal.fg),
                );
                ui.add_space(4.0);
            }
            Block::Paragraph(text) => {
                let job = inline_job(ui, text, 14.5, pal.fg_dark, width);
                wrap(ui, job);
                ui.add_space(8.0);
            }
            Block::Bullet(text) | Block::Numbered(_, text) => {
                let marker = match block {
                    Block::Numbered(n, _) => format!("{n}."),
                    _ => "•".to_string(),
                };
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_sized(
                        [26.0, 20.0],
                        egui::Label::new(RichText::new(marker).size(14.5).color(pal.accent)),
                    );
                    let job = inline_job(ui, text, 14.5, pal.fg_dark, width - 26.0);
                    wrap(ui, job);
                });
                ui.add_space(4.0);
            }
            Block::Tip(text) => {
                ui.add_space(4.0);
                egui::Frame::new()
                    .fill(pal.accent.gamma_multiply(0.10))
                    .stroke(egui::Stroke::new(1.0, pal.accent.gamma_multiply(0.35)))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_width(width - 28.0);
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new(icon::LIGHTBULB).size(16.0).color(pal.accent));
                            let job = inline_job(ui, text, 14.0, pal.fg_dark, width - 64.0);
                            wrap(ui, job);
                        });
                    });
                ui.add_space(10.0);
            }
        }
    }
    ui.add_space(20.0);
}

/// Texte qui passe à la ligne, même dans une rangée horizontale.
fn wrap(ui: &mut egui::Ui, job: LayoutJob) {
    ui.add(egui::Label::new(job).wrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_markers() {
        assert_eq!(
            spans("Menu **Fichier**, touche `F1`."),
            [
                Span::Text("Menu "),
                Span::Bold("Fichier"),
                Span::Text(", touche "),
                Span::Key("F1"),
                Span::Text("."),
            ]
        );
        assert_eq!(spans("**non fermé"), [Span::Text("**non fermé")]);
    }

    #[test]
    fn both_guides_have_the_same_topics() {
        let fr = parse(GUIDE_FR);
        let en = parse(GUIDE_EN);
        assert!(fr.len() >= 8, "rubriques du guide français");
        assert_eq!(
            fr.len(),
            en.len(),
            "mêmes rubriques en français et en anglais"
        );
        for topic in fr.iter().chain(&en) {
            assert!(!topic.blocks.is_empty(), "rubrique vide : {}", topic.title);
        }
    }

    #[test]
    fn blocks_are_recognized() {
        let topics = parse("## A\n\n### B\n\nTexte.\n\n- un\n1. deux\n> astuce\n");
        assert_eq!(
            topics[0].blocks,
            [
                Block::Heading("B".into()),
                Block::Paragraph("Texte.".into()),
                Block::Bullet("un".into()),
                Block::Numbered("1".into(), "deux".into()),
                Block::Tip("astuce".into()),
            ]
        );
    }
}
