//! Relevé de compte en PDF (remplace l'impression avec aperçu de
//! l'ancienne version, PRN-01/02) : en-tête du compte, soldes, filtres
//! actifs, tableau des lignes affichées, pages numérotées. Le PDF s'ouvre
//! dans la visionneuse du système, qui sert d'aperçu et imprime.

use bp_core::filter::TackFilter;
use bp_core::model::{Amount, Tack};
use printpdf::{
    Color, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerReference, Point, Rect, Rgb,
};

use printpdf::path::PaintMode;

use crate::document::OpenDoc;
use crate::format::money;

const REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular-latin.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold-latin.ttf");

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 14.0;
const ROW_H: f32 = 9.0;
const ROW_H_SINGLE: f32 = 6.5;
const FOOTER: f32 = 12.0;

/// Options du relevé.
#[derive(Clone, Copy)]
pub struct StatementOptions {
    pub black_and_white: bool,
    /// Détail sous le tiers (lignes plus hautes).
    pub details: bool,
}

/// Largeurs de glyphes, pour aligner et tronquer le texte.
struct Metrics {
    face: ttf_parser::Face<'static>,
}

impl Metrics {
    fn new(bytes: &'static [u8]) -> Self {
        Self {
            face: ttf_parser::Face::parse(bytes, 0).expect("police intégrée valide"),
        }
    }

    /// Largeur en mm d'un texte à `size` points.
    fn width(&self, text: &str, size: f32) -> f32 {
        let upem = self.face.units_per_em() as f32;
        let units: f32 = text
            .chars()
            .map(|c| {
                self.face
                    .glyph_index(c)
                    .and_then(|g| self.face.glyph_hor_advance(g))
                    .unwrap_or(500) as f32
            })
            .sum();
        units / upem * size * 0.352_778
    }

    /// Texte coupé avec « … » pour tenir dans `max` mm.
    fn fit(&self, text: &str, size: f32, max: f32) -> String {
        if self.width(text, size) <= max {
            return text.to_string();
        }
        let mut out = String::new();
        for c in text.chars() {
            if self.width(&format!("{out}{c}…"), size) > max {
                break;
            }
            out.push(c);
        }
        format!("{}…", out.trim_end())
    }
}

struct Painter<'a> {
    layer: PdfLayerReference,
    regular: &'a IndirectFontRef,
    semibold: &'a IndirectFontRef,
    metrics: &'a Metrics,
    bold_metrics: &'a Metrics,
}

fn rgb(c: [u8; 3]) -> Color {
    Color::Rgb(Rgb::new(
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        None,
    ))
}

impl Painter<'_> {
    /// Texte dont `y` est la ligne de base, mesurée depuis le haut de la page.
    fn text(&self, text: &str, size: f32, x: f32, y: f32, bold: bool, color: [u8; 3]) {
        self.layer.set_fill_color(rgb(color));
        let font = if bold { self.semibold } else { self.regular };
        self.layer.use_text(text, size, Mm(x), Mm(PAGE_H - y), font);
    }

    fn text_right(&self, text: &str, size: f32, right: f32, y: f32, bold: bool, color: [u8; 3]) {
        let m = if bold {
            self.bold_metrics
        } else {
            self.metrics
        };
        self.text(text, size, right - m.width(text, size), y, bold, color);
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32, fill: [u8; 3]) {
        self.layer.set_fill_color(rgb(fill));
        self.layer.add_rect(
            Rect::new(Mm(x), Mm(PAGE_H - y - h), Mm(x + w), Mm(PAGE_H - y))
                .with_mode(PaintMode::Fill),
        );
    }

    fn hline(&self, x0: f32, x1: f32, y: f32, color: [u8; 3], thickness: f32) {
        self.layer.set_outline_color(rgb(color));
        self.layer.set_outline_thickness(thickness);
        self.layer.add_line(Line {
            points: vec![
                (Point::new(Mm(x0), Mm(PAGE_H - y)), false),
                (Point::new(Mm(x1), Mm(PAGE_H - y)), false),
            ],
            is_closed: false,
        });
    }

    /// Symbole BankPerfect (infini et barres du « bp ») dans un carré.
    fn symbol(&self, x: f32, y: f32, size: f32, color: [u8; 3]) {
        let scale = size / 440.0;
        let skew = (-12.0f32).to_radians().tan();
        let map = |px: f32, py: f32| {
            Point::new(
                Mm(x + size / 2.0 + (px + skew * py) * scale),
                Mm(PAGE_H - (y + size / 2.0 + py * scale)),
            )
        };
        self.layer.set_outline_color(rgb(color));
        self.layer.set_outline_thickness(40.0 * scale * 2.835);
        let loop_points = (0..=120)
            .map(|k| {
                let t = (k as f32 * 3.0).to_radians();
                let d = 1.0 + t.sin().powi(2);
                (
                    map(150.0 * t.cos() / d, 1.4 * 150.0 * t.sin() * t.cos() / d),
                    false,
                )
            })
            .collect();
        self.layer.add_line(Line {
            points: loop_points,
            is_closed: true,
        });
        for (px, y0, y1) in [(-150.0, 0.0, -190.0), (0.0, 0.0, 190.0)] {
            self.layer.add_line(Line {
                points: vec![(map(px, y0), false), (map(px, y1), false)],
                is_closed: false,
            });
        }
    }
}

/// Description des filtres actifs, une ligne par critère.
fn filter_lines(doc: &OpenDoc) -> Vec<String> {
    let f = &doc.filters;
    let mut lines = Vec::new();
    if !f.mode.is_empty() {
        lines.push(format!("Mode de paiement : {}", f.mode));
    }
    if !f.payee.is_empty() {
        lines.push(format!("Tiers : {}", f.payee));
    }
    if !f.category.is_empty() {
        lines.push(format!("Catégorie : {}", f.category));
    }
    if !f.amount.is_empty() {
        lines.push(format!("Montant : {}", f.amount));
    }
    match f.tack {
        TackFilter::All => {}
        TackFilter::NotPointed => lines.push("Opérations non pointées".into()),
        TackFilter::Pointed => lines.push("Opérations pointées".into()),
        TackFilter::Reconciled => lines.push("Opérations rapprochées".into()),
        TackFilter::PointedOrReconciled => lines.push("Opérations pointées ou rapprochées".into()),
    }
    match (f.date_from, f.date_to) {
        (Some(a), Some(b)) => lines.push(format!("Du {a} au {b}")),
        (Some(a), None) => lines.push(format!("Depuis le {a}")),
        (None, Some(b)) => lines.push(format!("Jusqu'au {b}")),
        (None, None) => {}
    }
    lines
}

/// Construit le relevé PDF des lignes affichées du compte courant.
pub fn build(doc: &OpenDoc, options: StatementOptions) -> Result<Vec<u8>, String> {
    let bw = options.black_and_white;
    let ink = [0x1f, 0x23, 0x35];
    let muted = [0x6b, 0x72, 0x86];
    let accent = if bw { ink } else { [0x2e, 0x5f, 0xd9] };
    let red = if bw { ink } else { [0xc0, 0x26, 0x3d] };
    let green = if bw { ink } else { [0x2f, 0x7d, 0x32] };
    let stripe = [0xf3, 0xf4, 0xf7];
    let rule = [0xd5, 0xd8, 0xe0];

    let account = doc.current_account();
    let currency = account.currency.clone();
    let lines: Vec<usize> = doc.visible().to_vec();
    let row_h = if options.details { ROW_H } else { ROW_H_SINGLE };
    let filters = filter_lines(doc);

    // Mise en page : hauteur de l'en-tête de la première page, puis lignes
    // par page, pour numéroter « Page N / M ».
    let first_header = 52.0
        + if filters.is_empty() {
            0.0
        } else {
            6.0 + 4.5 * filters.len() as f32
        };
    let table_top_next = MARGIN + 10.0;
    let usable = |top: f32| {
        ((PAGE_H - MARGIN - FOOTER - top - 7.0) / row_h)
            .floor()
            .max(1.0) as usize
    };
    let first_rows = usable(MARGIN + first_header);
    let next_rows = usable(table_top_next);
    let pages = if lines.len() <= first_rows {
        1
    } else {
        1 + (lines.len() - first_rows).div_ceil(next_rows)
    };

    let title = format!("BankPerfect — relevé du compte {}", account.name);
    let (pdf, page1, layer1) = PdfDocument::new(&title, Mm(PAGE_W), Mm(PAGE_H), "Relevé");
    let regular = pdf.add_external_font(REGULAR).map_err(|e| e.to_string())?;
    let semibold = pdf.add_external_font(SEMIBOLD).map_err(|e| e.to_string())?;
    let metrics = Metrics::new(REGULAR);
    let bold_metrics = Metrics::new(SEMIBOLD);

    // Colonnes : début (mm) de chaque colonne et bord droit du tableau.
    let left = MARGIN;
    let right = PAGE_W - MARGIN;
    let col_date = left + 2.0;
    let col_mode = left + 22.0;
    let col_payee = left + 48.0;
    let col_cat = left + 104.0;
    let col_debit_r = right - 33.0;
    let col_credit_r = right - 9.0;
    let col_tack = right - 6.0;

    let today = bp_core::model::Date::today();
    let mut index = 0;
    for page in 0..pages {
        let (page_idx, layer_idx) = if page == 0 {
            (page1, layer1)
        } else {
            pdf.add_page(Mm(PAGE_W), Mm(PAGE_H), "Relevé")
        };
        let p = Painter {
            layer: pdf.get_page(page_idx).get_layer(layer_idx),
            regular: &regular,
            semibold: &semibold,
            metrics: &metrics,
            bold_metrics: &bold_metrics,
        };
        let mut y = MARGIN;
        if page == 0 {
            // En-tête : symbole, nom du compte, banque, fichier, date.
            p.symbol(left, y, 14.0, accent);
            p.text(&account.name, 16.0, left + 18.0, y + 6.5, true, ink);
            let mut sub = account.bank_name.clone();
            if !account.iban.trim().is_empty() {
                sub = if sub.is_empty() {
                    account.iban.trim().to_string()
                } else {
                    format!("{sub} · {}", account.iban.trim())
                };
            }
            p.text(&sub, 9.0, left + 18.0, y + 12.0, false, muted);
            p.text_right(
                &format!("Relevé du {today}"),
                9.0,
                right,
                y + 6.5,
                false,
                muted,
            );
            p.text_right(
                &p.metrics.fit(&doc.file_name(), 8.0, 70.0),
                8.0,
                right,
                y + 12.0,
                false,
                muted,
            );
            y += 20.0;

            // Soldes en trois cases.
            let totals = doc.totals();
            let box_w = (right - left - 8.0) / 3.0;
            for (k, (label, value, color)) in [
                ("Pointé", totals.pointed, accent),
                (
                    "À venir",
                    totals.pending,
                    if bw { ink } else { [0xd9, 0x7b, 0x29] },
                ),
                ("Solde", totals.balance, green),
            ]
            .into_iter()
            .enumerate()
            {
                let x = left + k as f32 * (box_w + 4.0);
                p.rect(x, y, box_w, 18.0, stripe);
                p.rect(x, y, 1.2, 18.0, color);
                p.text(label, 8.5, x + 5.0, y + 6.0, false, muted);
                let amount_color = if value.is_negative() { red } else { ink };
                p.text(
                    &money(value, &currency),
                    13.0,
                    x + 5.0,
                    y + 14.0,
                    true,
                    amount_color,
                );
            }
            y += 24.0;

            if !filters.is_empty() {
                p.text("Filtres appliqués", 8.5, left, y + 4.0, true, muted);
                y += 6.0;
                for line in &filters {
                    p.text(&format!("• {line}"), 8.5, left + 2.0, y + 3.5, false, ink);
                    y += 4.5;
                }
            }
            y += 8.0;
        } else {
            p.text(
                &format!("{} — suite", account.name),
                10.0,
                left,
                y + 5.0,
                true,
                ink,
            );
            y += 10.0;
        }

        // En-tête du tableau.
        let header_y = y + 4.5;
        let header = |text: &str, x: f32| p.text(text, 8.0, x, header_y, true, muted);
        header(
            if doc.show_mark_dates {
                "Pointage"
            } else {
                "Date"
            },
            col_date,
        );
        header("Mode", col_mode);
        header("Tiers", col_payee);
        header("Catégorie", col_cat);
        p.text_right("Dépense", 8.0, col_debit_r, header_y, true, muted);
        p.text_right("Recette", 8.0, col_credit_r, header_y, true, muted);
        header("P", col_tack);
        y += 7.0;
        p.hline(left, right, y, rule, 0.6);

        let rows = if page == 0 { first_rows } else { next_rows };
        for k in 0..rows {
            let Some(&op_index) = lines.get(index) else {
                break;
            };
            index += 1;
            let op = &account.operations[op_index];
            if k % 2 == 1 {
                p.rect(left, y, right - left, row_h, stripe);
            }
            let base = y + if options.details { 4.2 } else { 4.4 };
            let date = if doc.show_mark_dates {
                op.mark_date
            } else {
                op.date
            };
            p.text(
                &crate::settings::shared::date(date),
                8.0,
                col_date,
                base,
                false,
                ink,
            );
            p.text(
                &p.metrics
                    .fit(&op.mode_label(), 8.0, col_payee - col_mode - 2.0),
                8.0,
                col_mode,
                base,
                false,
                ink,
            );
            p.text(
                &p.metrics.fit(&op.payee, 8.0, col_cat - col_payee - 2.0),
                8.0,
                col_payee,
                base,
                false,
                ink,
            );
            if options.details && !op.details.is_empty() {
                p.text(
                    &p.metrics.fit(&op.details, 6.8, col_cat - col_payee - 2.0),
                    6.8,
                    col_payee,
                    base + 3.4,
                    false,
                    muted,
                );
            }
            // Catégorie : son nom, et la famille en petit dessous (comme la grille).
            let cat_width = col_debit_r - 22.0 - col_cat;
            if let Some(cat) = op.category.and_then(|id| doc.document.category(id)) {
                p.text(
                    &p.metrics.fit(&cat.name, 8.0, cat_width),
                    8.0,
                    col_cat,
                    base,
                    false,
                    ink,
                );
                let parent = cat
                    .is_child
                    .then(|| op.category.and_then(|id| doc.document.category_label(id)))
                    .flatten()
                    .and_then(|label| label.split(" > ").next().map(str::to_string));
                if options.details
                    && let Some(parent) = parent
                {
                    p.text(
                        &p.metrics.fit(&parent, 6.8, cat_width),
                        6.8,
                        col_cat,
                        base + 3.4,
                        false,
                        muted,
                    );
                }
            }
            if op.amount.is_negative() {
                p.text_right(&money(-op.amount, ""), 8.0, col_debit_r, base, false, red);
            } else if op.amount != Amount::ZERO {
                p.text_right(&money(op.amount, ""), 8.0, col_credit_r, base, false, green);
            }
            let tack = match op.tack {
                Tack::None => "",
                Tack::Pointed => "P",
                Tack::Reconciled => "R",
            };
            p.text(tack, 8.0, col_tack, base, true, accent);
            y += row_h;
        }
        p.hline(left, right, y, rule, 0.6);

        // Pied de page.
        let footer_y = PAGE_H - MARGIN + 2.0;
        p.text(
            &format!("BankPerfect · {} ligne(s) · {currency}", lines.len()),
            7.5,
            left,
            footer_y,
            false,
            muted,
        );
        p.text_right(
            &format!("Page {} / {pages}", page + 1),
            7.5,
            right,
            footer_y,
            false,
            muted,
        );
    }
    pdf.save_to_bytes().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relevé du fichier `BP_SCREENSHOT_FILE`, écrit dans target/screenshots.
    #[test]
    #[ignore]
    fn statement_of_sample_file() {
        let path = std::path::PathBuf::from(
            std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE"),
        );
        let opened = bp_core::file::open(&path, None).expect("fichier lisible");
        let mut doc = OpenDoc::new(path, opened);
        doc.refresh();
        for (name, bw) in [("releve.pdf", false), ("releve-nb.pdf", true)] {
            let bytes = build(
                &doc,
                StatementOptions {
                    black_and_white: bw,
                    details: true,
                },
            )
            .expect("PDF");
            let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/screenshots");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }
}
