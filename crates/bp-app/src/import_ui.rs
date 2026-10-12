//! Assistant d'import QIF / OFX / CSV : réglages du format à gauche, aperçu
//! vérifié à droite (doublons probables décochés), compte de destination.

use std::path::PathBuf;

use bp_core::import::{
    self, ConvertOptions, DateOrder, DuplicateOptions, ImportFormat, ImportStatus, ParsedImport,
    csv::{CsvColumns, CsvConfig, CsvRecord},
    qif::{QifField, QifOptions},
};
use bp_core::model::Tack;
use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::document::OpenDoc;
use crate::format::money;
use crate::theme::palette;
use crate::tools::Outcome;
use crate::ui_kit;

/// Champ affecté à une colonne CSV.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CsvField {
    Ignore,
    Date,
    ValueDate,
    Mode,
    Payee,
    Details,
    Category,
    Subcategory,
    Amount,
    Debit,
    Credit,
    Tack,
}

impl CsvField {
    const ALL: [CsvField; 12] = [
        CsvField::Ignore,
        CsvField::Date,
        CsvField::ValueDate,
        CsvField::Mode,
        CsvField::Payee,
        CsvField::Details,
        CsvField::Category,
        CsvField::Subcategory,
        CsvField::Amount,
        CsvField::Debit,
        CsvField::Credit,
        CsvField::Tack,
    ];

    fn label(self) -> &'static str {
        match self {
            CsvField::Ignore => "—",
            CsvField::Date => "Date",
            CsvField::ValueDate => "Date de valeur",
            CsvField::Mode => "Mode",
            CsvField::Payee => "Tiers",
            CsvField::Details => "Détail",
            CsvField::Category => "Catégorie",
            CsvField::Subcategory => "Sous-catégorie",
            CsvField::Amount => "Montant",
            CsvField::Debit => "Débit",
            CsvField::Credit => "Crédit",
            CsvField::Tack => "Pointage",
        }
    }
}

const QIF_FIELDS: [Option<QifField>; 8] = [
    None,
    Some(QifField::Date),
    Some(QifField::Mode),
    Some(QifField::Payee),
    Some(QifField::Details),
    Some(QifField::Category),
    Some(QifField::Amount),
    Some(QifField::Tack),
];

pub struct ImportForm {
    path: PathBuf,
    data: Vec<u8>,
    format: ImportFormat,
    qif: QifOptions,
    qif_letters: Vec<char>,
    qif_samples: Vec<String>,
    csv: CsvConfig,
    csv_text: String,
    csv_preview: Vec<CsvRecord>,
    csv_fields: Vec<CsvField>,
    parsed: Result<ParsedImport, String>,
    /// Compte de destination de chaque compte du fichier (`None` = ignoré).
    targets: Vec<Option<usize>>,
    /// Compte du fichier affiché dans l'aperçu.
    shown: usize,
    statuses: Vec<Vec<ImportStatus>>,
    include: Vec<Vec<bool>>,
    default_tack: Tack,
    create_categories: bool,
    store_fitid: bool,
    tolerance: u32,
    match_mode: bool,
    show_rejected: bool,
    /// Les réglages ont changé : relire le fichier.
    reparse: bool,
    /// Les doublons sont à recalculer.
    recheck: bool,
    /// Opérations fournies déjà lues (synchronisation bancaire) : libellé de
    /// la source, rien à relire.
    source: Option<String>,
    /// Pastille du format (format d'une extension) ; par défaut QIF, OFX,
    /// CSV ou Banque.
    format_name: Option<String>,
}

impl ImportForm {
    pub fn new(path: PathBuf) -> Result<Self, String> {
        let data = std::fs::read(&path).map_err(|e| format!("Lecture impossible : {e}"))?;
        let format = match path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .as_deref()
        {
            Some("ofx" | "qfx") => ImportFormat::Ofx,
            Some("qif") => ImportFormat::Qif,
            _ => ImportFormat::detect(&data),
        };
        let mut form = Self {
            path,
            format,
            qif: QifOptions::default(),
            qif_letters: Vec::new(),
            qif_samples: Vec::new(),
            csv: CsvConfig::default(),
            csv_text: String::new(),
            csv_preview: Vec::new(),
            csv_fields: Vec::new(),
            parsed: Ok(ParsedImport::default()),
            targets: Vec::new(),
            shown: 0,
            statuses: Vec::new(),
            include: Vec::new(),
            default_tack: Tack::None,
            create_categories: true,
            store_fitid: true,
            tolerance: 0,
            match_mode: true,
            show_rejected: false,
            reparse: true,
            recheck: true,
            source: None,
            format_name: None,
            data,
        };
        match format {
            ImportFormat::Qif => {
                form.qif_letters = import::qif::letters(&form.data);
                form.qif_samples = import::qif::sample_dates(&form.data, 12);
                if let Some(order) = DateOrder::guess(form.qif_samples.iter().map(String::as_str)) {
                    form.qif.date_order = order;
                }
            }
            ImportFormat::Csv => {
                form.csv_text = import::decode_text(&form.data);
                form.csv.separator = import::csv::detect_separator(&form.csv_text);
                form.refresh_csv_preview();
                let header = form
                    .csv_preview
                    .first()
                    .map(|r| r.cells.clone())
                    .unwrap_or_default();
                form.csv.columns = CsvColumns::guess(&header);
                form.csv_fields = fields_from_columns(&form.csv.columns, form.column_count());
                let dates: Vec<String> = form
                    .csv
                    .columns
                    .date
                    .map(|c| {
                        form.csv_preview
                            .iter()
                            .skip(1)
                            .filter_map(|r| r.cells.get(c).cloned())
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(order) = DateOrder::guess(dates.iter().map(String::as_str)) {
                    form.csv.date_order = order;
                }
            }
            ImportFormat::Ofx => {}
        }
        Ok(form)
    }

    fn column_count(&self) -> usize {
        self.csv_preview
            .iter()
            .map(|r| r.cells.len())
            .max()
            .unwrap_or(0)
    }

    fn refresh_csv_preview(&mut self) {
        self.csv_preview = import::csv::read_records(&self.csv_text, self.csv.separator)
            .into_iter()
            .take(40)
            .collect();
        let n = self.column_count();
        self.csv_fields.resize(n, CsvField::Ignore);
    }

    /// Aperçu d'opérations déjà lues (synchronisation bancaire), avec le
    /// compte de destination de chaque compte importé.
    pub fn from_parsed(source: String, parsed: ParsedImport, targets: Vec<Option<usize>>) -> Self {
        Self {
            path: PathBuf::new(),
            data: Vec::new(),
            format: ImportFormat::Ofx,
            qif: QifOptions::default(),
            qif_letters: Vec::new(),
            qif_samples: Vec::new(),
            csv: CsvConfig::default(),
            csv_text: String::new(),
            csv_preview: Vec::new(),
            csv_fields: Vec::new(),
            parsed: Ok(parsed),
            targets,
            shown: 0,
            statuses: Vec::new(),
            include: Vec::new(),
            default_tack: Tack::None,
            create_categories: true,
            store_fitid: true,
            tolerance: 0,
            match_mode: true,
            show_rejected: false,
            reparse: false,
            recheck: true,
            source: Some(source),
            format_name: None,
        }
    }

    /// Aperçu d'opérations lues par le format d'import d'une extension.
    pub fn from_extension(
        path: PathBuf,
        format_name: String,
        parsed: ParsedImport,
        doc: &OpenDoc,
    ) -> Self {
        let targets = parsed
            .accounts
            .iter()
            .enumerate()
            .map(|(k, a)| {
                import::match_account(&doc.document, a)
                    .or_else(|| (parsed.accounts.len() == 1 || k == 0).then_some(doc.account))
            })
            .collect();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut form = Self::from_parsed(name, parsed, targets);
        form.path = path;
        form.format_name = Some(format_name);
        form
    }

    fn parse(&mut self, doc: &OpenDoc) {
        if self.source.is_some() {
            self.recheck = true;
            return;
        }
        self.parsed = match self.format {
            ImportFormat::Qif => {
                import::qif::parse(&self.data, &self.qif).map_err(|e| e.to_string())
            }
            ImportFormat::Ofx => {
                import::ofx::parse(&self.data, &Default::default()).map_err(|e| e.to_string())
            }
            ImportFormat::Csv => {
                self.csv.columns = columns_from_fields(&self.csv_fields);
                match self.csv.columns.validate() {
                    Ok(()) => import::csv::parse(&self.data, &self.csv).map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                }
            }
        };
        let accounts = self
            .parsed
            .as_ref()
            .map(|p| p.accounts.clone())
            .unwrap_or_default();
        self.targets = accounts
            .iter()
            .enumerate()
            .map(|(k, a)| {
                import::match_account(&doc.document, a)
                    .or_else(|| (accounts.len() == 1 || k == 0).then_some(doc.account))
            })
            .collect();
        self.shown = self.shown.min(accounts.len().saturating_sub(1));
        self.recheck = true;
    }

    fn check_duplicates(&mut self, doc: &OpenDoc) {
        let options = DuplicateOptions {
            date_tolerance_days: self.tolerance,
            match_mode: self.match_mode,
        };
        let accounts = self
            .parsed
            .as_ref()
            .map(|p| p.accounts.as_slice())
            .unwrap_or(&[]);
        self.statuses = accounts
            .iter()
            .zip(&self.targets)
            .map(|(a, target)| match target {
                Some(t) => {
                    import::detect_duplicates(&doc.document.accounts[*t], &a.operations, &options)
                }
                None => vec![ImportStatus::New; a.operations.len()],
            })
            .collect();
        self.include = self
            .statuses
            .iter()
            .map(|s| s.iter().map(|st| st.is_new()).collect())
            .collect();
    }

    fn selected_count(&self) -> usize {
        self.include
            .iter()
            .zip(&self.targets)
            .filter(|(_, t)| t.is_some())
            .map(|(inc, _)| inc.iter().filter(|&&b| b).count())
            .sum()
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        if self.reparse {
            self.reparse = false;
            self.parse(doc);
        }
        if self.recheck {
            self.recheck = false;
            self.check_duplicates(doc);
        }
        let mut outcome = Outcome::Keep;
        let width = ui_kit::dialog_width(ctx, 0.85, 900.0, 1600.0);
        let height = ui_kit::dialog_height(ctx, 0.85).max(560.0);
        crate::dialogs::modal(ctx, "import").show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pal = palette(ui);
            let format_label = match self.format {
                _ if self.format_name.is_some() => self.format_name.as_deref().unwrap_or_default(),
                _ if self.source.is_some() => "Banque",
                ImportFormat::Qif => "QIF",
                ImportFormat::Ofx => "OFX",
                ImportFormat::Csv => "CSV",
            };
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(260.0, 36.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        crate::dialogs::header(
                            ui,
                            icon::DOWNLOAD_SIMPLE,
                            pal.accent,
                            "Importer des opérations",
                        )
                    },
                );
                ui_kit::pill(ui, format_label, pal.accent);
                let name = self.source.clone().unwrap_or_else(|| {
                    self.path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
                ui.label(RichText::new(name).color(pal.fg_dark));
            });
            ui.add_space(14.0);
            let body = height - 36.0 - 14.0 - 130.0;
            let gap = 20.0;
            let settings_width = if self.format == ImportFormat::Ofx {
                0.0
            } else {
                (width * 0.36).clamp(320.0, 560.0)
            };
            let preview_width =
                width - settings_width - if settings_width > 0.0 { gap } else { 0.0 };
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                if settings_width > 0.0 {
                    ui.vertical(|ui| {
                        ui.set_width(settings_width);
                        egui::ScrollArea::vertical()
                            .id_salt("import_settings")
                            .max_height(body)
                            .min_scrolled_height(body)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let w = settings_width - 14.0;
                                ui.set_max_width(w);
                                match self.format {
                                    ImportFormat::Qif => self.qif_settings(ui, w),
                                    ImportFormat::Csv => self.csv_settings(ui, w),
                                    ImportFormat::Ofx => {}
                                }
                            });
                    });
                }
                ui.vertical(|ui| {
                    ui.set_width(preview_width);
                    ui.set_height(body);
                    self.preview(ui, doc, preview_width, body);
                });
            });
            self.footer(ui, doc, width, &mut outcome);
        });
        outcome
    }

    fn qif_settings(&mut self, ui: &mut egui::Ui, width: f32) {
        let pal = palette(ui);
        let mut changed = false;
        ui_kit::section(ui, width, icon::CALENDAR_BLANK, "Dates", |ui| {
            changed |= date_order(ui, &mut self.qif.date_order);
            if !self.qif_samples.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "Exemples : {}",
                        self.qif_samples
                            .iter()
                            .take(4)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("  ·  ")
                    ))
                    .size(12.5)
                    .color(pal.muted),
                );
            }
        });
        ui_kit::section(ui, width, icon::LIST_BULLETS, "Lettres du fichier", |ui| {
            for &letter in &self.qif_letters.clone() {
                let current = self
                    .qif
                    .fields
                    .iter()
                    .find(|(l, _)| *l == letter)
                    .map(|(_, f)| *f);
                let mut chosen = current;
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [40.0, 30.0],
                        egui::Label::new(RichText::new(letter.to_string()).monospace().size(15.0)),
                    );
                    egui::ComboBox::from_id_salt(("qif_letter", letter))
                        .selected_text(chosen.map_or("Ignorer", |f| f.label()))
                        .width(ui.available_width() - 16.0)
                        .show_ui(ui, |ui| {
                            for f in QIF_FIELDS {
                                ui.selectable_value(
                                    &mut chosen,
                                    f,
                                    f.map_or("Ignorer", |f| f.label()),
                                );
                            }
                        });
                });
                if chosen != current {
                    self.qif.fields.retain(|(l, _)| *l != letter);
                    if let Some(f) = chosen {
                        self.qif.fields.push((letter, f));
                    }
                    changed = true;
                }
            }
        });
        if changed {
            self.reparse = true;
        }
    }

    fn csv_settings(&mut self, ui: &mut egui::Ui, width: f32) {
        let pal = palette(ui);
        let mut changed = false;
        let mut separator_changed = false;
        ui_kit::section(ui, width, icon::TABLE, "Format", |ui| {
            ui.label(RichText::new("Séparateur").color(pal.fg_dark));
            let mut sep = self.csv.separator;
            if ui_kit::segmented(
                ui,
                &mut sep,
                &[
                    (';', "", "Point-virgule", pal.accent),
                    (',', "", "Virgule", pal.accent),
                    ('\t', "", "Tabulation", pal.accent),
                ],
                32.0,
                None,
            ) {
                self.csv.separator = sep;
                separator_changed = true;
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Lignes d'en-tête ignorées").color(pal.fg_dark));
                changed |= ui
                    .add(egui::DragValue::new(&mut self.csv.skip_rows).range(0..=50))
                    .changed();
            });
            ui.add_space(10.0);
            ui.label(RichText::new("Ordre des dates").color(pal.fg_dark));
            changed |= date_order(ui, &mut self.csv.date_order);
            ui.add_space(10.0);
            ui.label(RichText::new("Séparateur décimal").color(pal.fg_dark));
            let mut decimal = self.csv.decimal_separator;
            if ui_kit::segmented(
                ui,
                &mut decimal,
                &[
                    (None, "", "Automatique", pal.accent),
                    (Some(','), "", "Virgule", pal.accent),
                    (Some('.'), "", "Point", pal.accent),
                ],
                32.0,
                None,
            ) {
                self.csv.decimal_separator = decimal;
                changed = true;
            }
        });
        ui_kit::section(ui, width, icon::COLUMNS, "Colonnes", |ui| {
            ui.label(
                RichText::new("Indiquez ce que contient chaque colonne du fichier. Montant, ou bien Débit et Crédit.")
                    .size(12.5)
                    .color(pal.muted),
            );
            ui.add_space(6.0);
            let header = self
                .csv_preview
                .first()
                .map(|r| r.cells.clone())
                .unwrap_or_default();
            for k in 0..self.csv_fields.len() {
                let title = header.get(k).cloned().unwrap_or_default();
                let sample = self
                    .csv_preview
                    .get(self.csv.skip_rows.max(1))
                    .and_then(|r| r.cells.get(k).cloned())
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() * 0.5);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("{}. {title}", k + 1)).color(pal.fg),
                            )
                            .truncate(),
                        );
                        ui.add(
                            egui::Label::new(RichText::new(sample).size(12.0).color(pal.muted))
                                .truncate(),
                        );
                    });
                    let mut field = self.csv_fields[k];
                    egui::ComboBox::from_id_salt(("csv_col", k))
                        .selected_text(field.label())
                        .width(ui.available_width() - 16.0)
                        .show_ui(ui, |ui| {
                            for f in CsvField::ALL {
                                ui.selectable_value(&mut field, f, f.label());
                            }
                        });
                    if field != self.csv_fields[k] {
                        // Un champ ne peut être affecté qu'à une colonne.
                        if field != CsvField::Ignore {
                            for other in &mut self.csv_fields {
                                if *other == field {
                                    *other = CsvField::Ignore;
                                }
                            }
                        }
                        self.csv_fields[k] = field;
                        changed = true;
                    }
                });
                ui.add_space(4.0);
            }
        });
        if separator_changed {
            self.refresh_csv_preview();
            let header = self
                .csv_preview
                .first()
                .map(|r| r.cells.clone())
                .unwrap_or_default();
            self.csv.columns = CsvColumns::guess(&header);
            self.csv_fields = fields_from_columns(&self.csv.columns, self.column_count());
            changed = true;
        }
        if changed {
            self.reparse = true;
        }
    }

    fn preview(&mut self, ui: &mut egui::Ui, doc: &OpenDoc, width: f32, height: f32) {
        let pal = palette(ui);
        let parsed = match &self.parsed {
            Ok(p) => p.clone(),
            Err(e) => {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(icon::WARNING).size(36.0).color(pal.warning));
                    ui.add_space(8.0);
                    ui.label(RichText::new(e).color(pal.fg_dark));
                });
                return;
            }
        };
        if parsed.accounts.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(icon::FILE_DASHED).size(36.0).color(pal.muted));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Aucune opération lisible avec ces réglages.").color(pal.fg_dark),
                );
            });
            self.rejected(ui, &parsed, width);
            return;
        }
        // Comptes du fichier (OFX multicompte) et compte de destination.
        if parsed.accounts.len() > 1 {
            let options: Vec<(usize, String)> = parsed
                .accounts
                .iter()
                .enumerate()
                .map(|(k, a)| {
                    let name = a.description();
                    (
                        k,
                        if name.is_empty() {
                            format!("Compte {}", k + 1)
                        } else {
                            name
                        },
                    )
                })
                .collect();
            ui.horizontal_wrapped(|ui| {
                for (k, name) in &options {
                    let text = format!("{name} ({})", parsed.accounts[*k].operations.len());
                    if ui.selectable_label(self.shown == *k, text).clicked() {
                        self.shown = *k;
                    }
                }
            });
            ui.add_space(8.0);
        }
        let shown = self.shown;
        let account = &parsed.accounts[shown];
        ui.horizontal(|ui| {
            ui.label(RichText::new("Vers le compte").color(pal.fg_dark));
            let mut target = self.targets.get(shown).copied().flatten();
            let label = target.map_or("Ne pas importer".to_string(), |t| {
                doc.document.accounts[t].name.clone()
            });
            egui::ComboBox::from_id_salt(("import_target", shown))
                .selected_text(label)
                .width(260.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut target, None, "Ne pas importer");
                    for &i in &doc.document.account_order {
                        ui.selectable_value(&mut target, Some(i), &doc.document.accounts[i].name);
                    }
                });
            if Some(target) != self.targets.get(shown).copied() {
                self.targets[shown] = target;
                self.recheck = true;
            }
            if let (Some(a), Some(b)) = (account.min_date(), account.max_date()) {
                ui.label(RichText::new(format!("du {a} au {b}")).color(pal.muted));
            }
        });
        ui.add_space(8.0);

        let statuses = self.statuses.get(shown).cloned().unwrap_or_default();
        let duplicates = statuses.iter().filter(|s| !s.is_new()).count();
        ui.horizontal(|ui| {
            ui_kit::pill(
                ui,
                &import::operation_count_label(account.operations.len()),
                pal.accent,
            );
            if duplicates > 0 {
                ui_kit::pill(
                    ui,
                    &format!("{duplicates} doublon(s) probable(s), décoché(s)"),
                    pal.warning,
                );
            }
            if !parsed.rejected.is_empty() {
                ui_kit::pill(
                    ui,
                    &format!("{} ligne(s) rejetée(s)", parsed.rejected.len()),
                    pal.negative,
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Tout décocher").clicked() {
                    self.include[shown].iter_mut().for_each(|b| *b = false);
                }
                if ui.small_button("Tout cocher").clicked() {
                    self.include[shown].iter_mut().for_each(|b| *b = true);
                }
            });
        });
        ui.add_space(6.0);

        let currency = self
            .targets
            .get(shown)
            .copied()
            .flatten()
            .map_or("€".to_string(), |t| {
                doc.document.accounts[t].currency.clone()
            });
        let list_height = height - 110.0 - if self.show_rejected { 140.0 } else { 0.0 };
        egui::ScrollArea::vertical()
            .id_salt(("import_rows", shown))
            .max_height(list_height.max(120.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (k, op) in account.operations.iter().enumerate() {
                    let status = statuses.get(k).copied().unwrap_or(ImportStatus::New);
                    let mut include = self
                        .include
                        .get(shown)
                        .and_then(|v| v.get(k))
                        .copied()
                        .unwrap_or(true);
                    ui.horizontal(|ui| {
                        ui.set_min_height(30.0);
                        if crate::theme::checkbox(ui, &mut include, "").changed()
                            && let Some(v) = self.include.get_mut(shown)
                        {
                            v[k] = include;
                        }
                        ui_kit::cell(
                            ui,
                            92.0,
                            egui::Align::Min,
                            RichText::new(crate::settings::shared::date(op.date)),
                        );
                        let w = (width - 540.0).max(160.0);
                        let text = if op.details.is_empty() || op.details == op.payee {
                            op.payee.clone()
                        } else {
                            format!("{}  ·  {}", op.payee, op.details)
                        };
                        let color = if include { pal.fg } else { pal.muted };
                        ui_kit::cell(ui, w, egui::Align::Min, RichText::new(text).color(color));
                        ui_kit::cell(
                            ui,
                            130.0,
                            egui::Align::Min,
                            RichText::new(op.mode.label().to_string()).color(pal.fg_dark),
                        );
                        let amount_color = if op.amount.is_negative() {
                            pal.negative
                        } else {
                            pal.positive
                        };
                        ui_kit::cell(
                            ui,
                            110.0,
                            egui::Align::Max,
                            RichText::new(money(op.amount, &currency)).color(amount_color),
                        );
                        ui.add_space(12.0);
                        match status {
                            ImportStatus::New => ui_kit::pill(ui, "Nouvelle", pal.positive),
                            ImportStatus::ProbableDuplicate {
                                existing,
                                same_fitid,
                            } => {
                                let label = if same_fitid {
                                    "Déjà importée"
                                } else {
                                    "Doublon probable"
                                };
                                ui_kit::pill(ui, label, pal.warning);
                                if let Some(t) = self.targets.get(shown).copied().flatten() {
                                    let e = &doc.document.accounts[t].operations[existing];
                                    ui.label(RichText::new(icon::INFO).color(pal.muted))
                                        .on_hover_text(format!(
                                            "Ressemble à : {} · {} · {}",
                                            e.date,
                                            e.payee,
                                            money(e.amount, &currency)
                                        ));
                                }
                            }
                        }
                    });
                }
            });
        self.rejected(ui, &parsed, width);
    }

    fn rejected(&mut self, ui: &mut egui::Ui, parsed: &ParsedImport, width: f32) {
        if parsed.rejected.is_empty() {
            return;
        }
        let pal = palette(ui);
        ui.add_space(6.0);
        let label = if self.show_rejected {
            "Masquer les lignes rejetées"
        } else {
            "Voir les lignes rejetées"
        };
        if ui.link(label).clicked() {
            self.show_rejected = !self.show_rejected;
        }
        if self.show_rejected {
            egui::ScrollArea::vertical()
                .id_salt("import_rejected")
                .max_height(130.0)
                .show(ui, |ui| {
                    for r in &parsed.rejected {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Ligne {}", r.line)).color(pal.muted));
                            ui.label(RichText::new(r.reason.to_string()).color(pal.negative));
                            ui.add_sized(
                                [width - 300.0, 20.0],
                                egui::Label::new(RichText::new(&r.text).monospace().size(12.0))
                                    .truncate(),
                            );
                        });
                    }
                });
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui, doc: &mut OpenDoc, width: f32, outcome: &mut Outcome) {
        let pal = palette(ui);
        ui.add_space(10.0);
        ui.painter().hline(
            ui.min_rect().left()..=ui.min_rect().left() + width,
            ui.cursor().top(),
            egui::Stroke::new(1.0, pal.border.gamma_multiply(0.6)),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Pointage").color(pal.fg_dark));
            ui_kit::segmented(
                ui,
                &mut self.default_tack,
                &[
                    (Tack::None, "", "À pointer", pal.fg),
                    (Tack::Pointed, "", "Pointées", pal.pointed),
                    (Tack::Reconciled, "", "Rapprochées", pal.reconciled),
                ],
                30.0,
                Some(330.0),
            );
            ui.add_space(16.0);
            crate::theme::checkbox(
                ui,
                &mut self.create_categories,
                "Créer les catégories manquantes",
            );
            ui.add_space(16.0);
            let before = (self.tolerance, self.match_mode);
            ui.label(RichText::new("Doublons : écart toléré").color(pal.fg_dark));
            ui.add(
                egui::DragValue::new(&mut self.tolerance)
                    .range(0..=10)
                    .suffix(" j"),
            );
            crate::theme::checkbox(ui, &mut self.match_mode, "même mode");
            if before != (self.tolerance, self.match_mode) {
                self.recheck = true;
            }
        });
        let count = self.selected_count();
        crate::dialogs::footer(ui, |ui| {
            let label = format!(
                "{}  Importer {}",
                icon::DOWNLOAD_SIMPLE,
                import::operation_count_label(count)
            );
            if crate::dialogs::primary(ui, &label, count > 0).clicked() {
                let n = self.import(doc);
                *outcome =
                    Outcome::Done(format!("{} importée(s)", import::operation_count_label(n)));
            }
            if crate::dialogs::button(ui, "Annuler").clicked()
                || ui.input(|i| i.key_pressed(Key::Escape))
            {
                *outcome = Outcome::Close;
            }
        });
    }

    /// Ajoute les lignes cochées aux comptes de destination.
    fn import(&mut self, doc: &mut OpenDoc) -> usize {
        let Ok(parsed) = self.parsed.clone() else {
            return 0;
        };
        let options = ConvertOptions {
            default_tack: self.default_tack,
            store_fitid: self.store_fitid,
        };
        let mut total = 0;
        for (k, account) in parsed.accounts.iter().enumerate() {
            let Some(target) = self.targets.get(k).copied().flatten() else {
                continue;
            };
            let chosen: Vec<&import::ImportedOperation> = account
                .operations
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    self.include
                        .get(k)
                        .and_then(|v| v.get(*i))
                        .copied()
                        .unwrap_or(false)
                })
                .map(|(_, op)| op)
                .collect();
            if chosen.is_empty() {
                continue;
            }
            if self.create_categories {
                import::create_missing_categories(&mut doc.document, chosen.iter().copied());
            }
            let operations: Vec<_> = chosen
                .iter()
                .map(|op| import::to_operation(&doc.document, op, &options))
                .collect();
            total += doc.import_into(target, operations);
        }
        total
    }
}

fn date_order(ui: &mut egui::Ui, order: &mut DateOrder) -> bool {
    let pal = palette(ui);
    ui_kit::segmented(
        ui,
        order,
        &[
            (
                DateOrder::DayMonthYear,
                "",
                "Jour / mois / année",
                pal.accent,
            ),
            (DateOrder::MonthDayYear, "", "Mois / jour", pal.accent),
            (
                DateOrder::YearMonthDay,
                "",
                "Année / mois / jour",
                pal.accent,
            ),
        ],
        32.0,
        None,
    )
}

fn fields_from_columns(columns: &CsvColumns, count: usize) -> Vec<CsvField> {
    let mut fields = vec![CsvField::Ignore; count];
    let mut set = |index: Option<usize>, field: CsvField| {
        if let Some(i) = index
            && i < count
            && fields[i] == CsvField::Ignore
        {
            fields[i] = field;
        }
    };
    set(columns.date, CsvField::Date);
    set(columns.value_date, CsvField::ValueDate);
    set(columns.mode, CsvField::Mode);
    set(columns.payee, CsvField::Payee);
    set(columns.details, CsvField::Details);
    set(columns.category, CsvField::Category);
    set(columns.subcategory, CsvField::Subcategory);
    set(columns.amount, CsvField::Amount);
    set(columns.debit, CsvField::Debit);
    set(columns.credit, CsvField::Credit);
    set(columns.tack, CsvField::Tack);
    fields
}

fn columns_from_fields(fields: &[CsvField]) -> CsvColumns {
    let find = |f: CsvField| fields.iter().position(|&x| x == f);
    CsvColumns {
        date: find(CsvField::Date),
        value_date: find(CsvField::ValueDate),
        mode: find(CsvField::Mode),
        payee: find(CsvField::Payee),
        details: find(CsvField::Details),
        category: find(CsvField::Category),
        subcategory: find(CsvField::Subcategory),
        amount: find(CsvField::Amount),
        debit: find(CsvField::Debit),
        credit: find(CsvField::Credit),
        tack: find(CsvField::Tack),
    }
}
