//! Export tableur (.xlsx) : lignes affichées de la grille et tableaux des
//! rapports. Remplace l'ancien export SYLK déguisé en `.xls`.

use bp_core::i18n::{self, Lang};
use bp_core::tr;
use rust_xlsxwriter::{Format, Workbook};

/// Cellule d'un tableau exporté.
pub enum Cell {
    Text(String),
    /// Texte en gras (en-têtes, totaux).
    Bold(String),
    /// Montant en monnaie (format `# ##0,00`, `#,##0.00` en anglais).
    Money(f64),
    /// Montant en gras (totaux).
    BoldMoney(f64),
    /// Nombre entier (comptes d'opérations).
    Number(f64),
    Empty,
}

/// Écrit un tableau dans un classeur d'une feuille. La première ligne est
/// figée (en-tête).
pub fn write(path: &std::path::Path, sheet: &str, rows: &[Vec<Cell>]) -> Result<(), String> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    let name: String = sheet
        .chars()
        .filter(|c| !"[]:*?/\\".contains(*c))
        .take(31)
        .collect();
    worksheet
        .set_name(if name.is_empty() {
            tr!("Feuille")
        } else {
            &name
        })
        .map_err(|e| e.to_string())?;
    let bold = Format::new().set_bold();
    let money_format = money_format();
    let money = Format::new().set_num_format(money_format);
    let bold_money = Format::new().set_bold().set_num_format(money_format);
    let mut widths: Vec<usize> = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let (r, c16) = (r as u32, c as u16);
            let len = match cell {
                Cell::Text(t) => {
                    worksheet
                        .write_string(r, c16, t)
                        .map_err(|e| e.to_string())?;
                    t.chars().count()
                }
                Cell::Bold(t) => {
                    worksheet
                        .write_string_with_format(r, c16, t, &bold)
                        .map_err(|e| e.to_string())?;
                    t.chars().count()
                }
                Cell::Money(v) => {
                    worksheet
                        .write_number_with_format(r, c16, *v, &money)
                        .map_err(|e| e.to_string())?;
                    12
                }
                Cell::BoldMoney(v) => {
                    worksheet
                        .write_number_with_format(r, c16, *v, &bold_money)
                        .map_err(|e| e.to_string())?;
                    12
                }
                Cell::Number(v) => {
                    worksheet
                        .write_number(r, c16, *v)
                        .map_err(|e| e.to_string())?;
                    8
                }
                Cell::Empty => 0,
            };
            if widths.len() <= c {
                widths.resize(c + 1, 8);
            }
            widths[c] = widths[c].max(len.min(60));
        }
    }
    for (c, w) in widths.iter().enumerate() {
        worksheet
            .set_column_width(c as u16, *w as f64 + 2.0)
            .map_err(|e| e.to_string())?;
    }
    if !rows.is_empty() {
        worksheet
            .set_freeze_panes(1, 0)
            .map_err(|e| e.to_string())?;
    }
    workbook.save(path).map_err(|e| e.to_string())
}

/// Format des montants : séparateur des milliers en espace en français,
/// en virgule en anglais (les cellules restent des nombres).
fn money_format() -> &'static str {
    match i18n::lang() {
        Lang::Fr => "# ##0.00;[Red]-# ##0.00",
        Lang::En => "#,##0.00;[Red]-#,##0.00",
    }
}

/// Tableau en attente d'écriture : l'emplacement est demandé sans bloquer
/// l'interface, puis [`finished`] écrit le fichier.
#[derive(Clone)]
struct PendingExport {
    sheet: String,
    rows: std::sync::Arc<Vec<Vec<Cell>>>,
}

fn export_id() -> egui::Id {
    egui::Id::new("spreadsheet_export")
}

/// Demande l'emplacement du classeur ; le tableau sera écrit par [`finished`].
pub fn export(
    ctx: &egui::Context,
    default_dir: &std::path::Path,
    file_name: &str,
    sheet: &str,
    rows: Vec<Vec<Cell>>,
) {
    if crate::file_picker::is_open(ctx) {
        return;
    }
    let dialog = rfd::FileDialog::new()
        .set_title(tr!("Exporter vers un tableur"))
        .set_directory(default_dir)
        .set_file_name(format!("{file_name}.xlsx"))
        .add_filter(tr!("Classeur Excel"), &["xlsx"]);
    let pending = PendingExport {
        sheet: sheet.to_string(),
        rows: std::sync::Arc::new(rows),
    };
    ctx.data_mut(|d| d.insert_temp(export_id(), pending));
    crate::file_picker::open(ctx, export_id(), crate::file_picker::Kind::Save, dialog);
}

/// Une fois l'emplacement choisi, écrit le tableau et rend le message de
/// résultat (rien si l'utilisateur a annulé ou n'a pas encore choisi).
pub fn finished(ctx: &egui::Context) -> Option<String> {
    let choice = crate::file_picker::finished(ctx, export_id())?;
    let pending = ctx.data_mut(|d| {
        let pending = d.get_temp::<PendingExport>(export_id());
        d.remove::<PendingExport>(export_id());
        pending
    })?;
    let mut path = choice?;
    if path.extension().is_none() {
        path.set_extension("xlsx");
    }
    Some(match write(&path, &pending.sheet, &pending.rows) {
        Ok(()) => tr!("Tableau exporté : {}", path.display()),
        Err(e) => tr!("Export impossible : {}", e),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_workbook() {
        let dir = std::env::temp_dir().join(format!("bp-xlsx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rapport.xlsx");
        let rows = vec![
            vec![Cell::Bold("Catégorie".into()), Cell::Bold("Janv".into())],
            vec![Cell::Text("Alimentation".into()), Cell::Money(-12.5)],
            vec![
                Cell::Bold("Total".into()),
                Cell::BoldMoney(-12.5),
                Cell::Empty,
            ],
        ];
        write(&path, "Totaux: par/catégorie", &rows).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        // Un classeur .xlsx est une archive zip.
        assert_eq!(&bytes[..2], b"PK");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
