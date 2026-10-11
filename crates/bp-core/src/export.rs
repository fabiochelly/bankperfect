//! Exports CSV et QIF (ex-scripts `export_csv.py` et `export_qif.py`).

use crate::model::{Account, Document, Mode, Operation, StandardMode, Tack};

/// Colonnes possibles d'un export CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvField {
    Date,
    Mode,
    Payee,
    Details,
    Category,
    SubCategory,
    Amount,
    Tack,
}

impl CsvField {
    pub const ALL: [CsvField; 8] = [
        Self::Date,
        Self::Mode,
        Self::Payee,
        Self::Details,
        Self::Category,
        Self::SubCategory,
        Self::Amount,
        Self::Tack,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Date => "Date",
            Self::Mode => "Mode",
            Self::Payee => "Tiers",
            Self::Details => "Détails",
            Self::Category => "Catégorie",
            Self::SubCategory => "Sous-catégorie",
            Self::Amount => "Montant",
            Self::Tack => "Pointage",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CsvOptions {
    /// Colonnes exportées, dans l'ordre.
    pub fields: Vec<CsvField>,
    pub separator: char,
    pub header: bool,
    /// Remplacer « Chq 1234 » par « Chèque émis ».
    pub cheque_as_mode: bool,
    /// Catégorie et sous-catégorie dans une seule colonne, avec ce séparateur.
    pub group_categories: Option<String>,
    /// Symboles des états non pointé, pointé, rapproché.
    pub tack_symbols: [String; 3],
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            fields: CsvField::ALL.to_vec(),
            separator: ';',
            header: true,
            cheque_as_mode: false,
            group_categories: None,
            tack_symbols: ["-".into(), "P".into(), "R".into()],
        }
    }
}

/// Catégorie et sous-catégorie d'une opération (sous-catégorie vide pour une
/// catégorie principale).
fn category_parts(document: &Document, op: &Operation) -> (String, String) {
    let Some(id) = op.category else {
        return (String::new(), String::new());
    };
    match document.category_label(id) {
        Some(label) => match label.split_once(" > ") {
            Some((parent, child)) => (parent.to_string(), child.to_string()),
            None => (label, String::new()),
        },
        None => (String::new(), String::new()),
    }
}

/// Génère le contenu CSV des `lines` (indices d'opérations) d'un compte.
///
/// Pas de guillemets, comme l'original : le séparateur est remplacé par une
/// espace dans les textes. Montants avec deux décimales ; la virgule décimale
/// devient un point si le séparateur est lui-même une virgule (correction :
/// l'original produisait alors un fichier illisible).
pub fn csv(
    document: &Document,
    account: &Account,
    lines: &[usize],
    options: &CsvOptions,
) -> String {
    let sep = options.separator;
    let decimal = if sep == ',' { '.' } else { ',' };
    let clean = |s: &str| s.replace(sep, " ").replace(['\r', '\n'], " ");
    let fields: Vec<CsvField> = options
        .fields
        .iter()
        .copied()
        .filter(|f| !(options.group_categories.is_some() && *f == CsvField::SubCategory))
        .collect();
    let mut out = String::new();
    let mut push_row = |cells: Vec<String>| {
        out.push_str(&cells.join(&sep.to_string()));
        out.push_str("\r\n");
    };
    if options.header {
        push_row(fields.iter().map(|f| f.label().to_string()).collect());
    }
    for &i in lines {
        let op = &account.operations[i];
        let (category, sub) = category_parts(document, op);
        let cells = fields
            .iter()
            .map(|field| match field {
                CsvField::Date => op.date.to_string(),
                CsvField::Mode => {
                    if options.cheque_as_mode && op.cheque_number().is_some() {
                        StandardMode::ChequeEmis.label().to_string()
                    } else {
                        clean(&op.mode_label())
                    }
                }
                CsvField::Payee => clean(&op.payee),
                CsvField::Details => clean(&op.details),
                CsvField::Category => match (&options.group_categories, sub.is_empty()) {
                    (Some(joiner), false) => clean(&format!("{category}{joiner}{sub}")),
                    _ => clean(&category),
                },
                CsvField::SubCategory => clean(&sub),
                CsvField::Amount => op.amount.format(2, decimal),
                CsvField::Tack => options.tack_symbols[tack_index(op.tack)].clone(),
            })
            .collect();
        push_row(cells);
    }
    out
}

fn tack_index(tack: Tack) -> usize {
    match tack {
        Tack::None => 0,
        Tack::Pointed => 1,
        Tack::Reconciled => 2,
    }
}

/// Génère un fichier QIF `!Type:Bank` des `lines` d'un compte.
pub fn qif(document: &Document, account: &Account, lines: &[usize]) -> String {
    let mut out = String::from("!Type:Bank\r\n");
    for &i in lines {
        let op = &account.operations[i];
        let (category, sub) = category_parts(document, op);
        let mode = match &op.mode {
            Mode::Standard(StandardMode::ChequeEmis) => {
                op.cheque_number().unwrap_or("").to_string()
            }
            mode => mode.label().to_string(),
        };
        out.push_str(&format!("D{}\r\n", op.date));
        if !mode.is_empty() {
            out.push_str(&format!("N{mode}\r\n"));
        }
        out.push_str(&format!("P{}\r\n", op.payee));
        if !op.details.is_empty() {
            out.push_str(&format!("M{}\r\n", op.details));
        }
        if !category.is_empty() {
            let label = if sub.is_empty() {
                category
            } else {
                format!("{category}:{sub}")
            };
            out.push_str(&format!("L{label}\r\n"));
        }
        out.push_str(&format!("T{}\r\n^\r\n", op.amount.format(2, '.')));
    }
    out
}

/// Nom de fichier proposé : `<fichier sans extension> - <compte>.<ext>`.
pub fn suggested_file_name(file_stem: &str, account: &Account, extension: &str) -> String {
    let name: String = account
        .name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("{file_stem} - {name}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn sample() -> (Document, Account) {
        let op = |payee: &str,
                  details: &str,
                  cents: i64,
                  mode: Mode,
                  category: Option<u32>,
                  tack: Tack| Operation {
            uid: 1,
            date: Date::from_ymd(2026, 3, 5).unwrap(),
            mark_date: Date::from_ymd(2026, 3, 5).unwrap(),
            tack,
            mode,
            payee: payee.into(),
            details: details.into(),
            category,
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        };
        let account = Account {
            uid: "a".into(),
            name: "Courant".into(),
            bank_name: String::new(),
            agency_name: String::new(),
            agency_address: String::new(),
            number: String::new(),
            bank_code: String::new(),
            agency_code: String::new(),
            bic: String::new(),
            iban: String::new(),
            logo: String::new(),
            currency: "€".into(),
            currency_rate: 1.0,
            secondary_currency: "F".into(),
            secondary_rate: 6.55957,
            is_virtual: false,
            disabled: false,
            operations: vec![
                op(
                    "Fnac; Paris",
                    "livre",
                    -1250,
                    Mode::Standard(StandardMode::Carte),
                    Some(1),
                    Tack::Pointed,
                ),
                op(
                    "Dupont",
                    "n°1234",
                    -5000,
                    Mode::Standard(StandardMode::ChequeEmis),
                    Some(0),
                    Tack::None,
                ),
            ],
        };
        let document = Document {
            group_name: String::new(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: Vec::new(),
            accounts: Vec::new(),
            account_order: Vec::new(),
            default_account: 0,
            categories: vec![
                Category {
                    id: 0,
                    name: "Loisirs".into(),
                    is_child: false,
                },
                Category {
                    id: 1,
                    name: "Livres".into(),
                    is_child: true,
                },
            ],
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        };
        (document, account)
    }

    #[test]
    fn csv_export() {
        let (doc, acc) = sample();
        let text = csv(&doc, &acc, &[0, 1], &CsvOptions::default());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "Date;Mode;Tiers;Détails;Catégorie;Sous-catégorie;Montant;Pointage"
        );
        assert_eq!(
            lines[1],
            "05/03/2026;Carte;Fnac  Paris;livre;Loisirs;Livres;-12,50;P"
        );
        assert_eq!(
            lines[2],
            "05/03/2026;Chq 1234;Dupont;n°1234;Loisirs;;-50,00;-"
        );
    }

    #[test]
    fn csv_options() {
        let (doc, acc) = sample();
        let options = CsvOptions {
            fields: vec![
                CsvField::Amount,
                CsvField::Mode,
                CsvField::Category,
                CsvField::SubCategory,
            ],
            separator: ',',
            header: false,
            cheque_as_mode: true,
            group_categories: Some(" > ".into()),
            ..Default::default()
        };
        let text = csv(&doc, &acc, &[0, 1], &options);
        assert_eq!(
            text,
            "-12.50,Carte,Loisirs > Livres\r\n-50.00,Chèque émis,Loisirs\r\n"
        );
    }

    #[test]
    fn qif_export() {
        let (doc, acc) = sample();
        let text = qif(&doc, &acc, &[0, 1]);
        assert_eq!(
            text,
            "!Type:Bank\r\nD05/03/2026\r\nNCarte\r\nPFnac; Paris\r\nMlivre\r\nLLoisirs:Livres\r\nT-12.50\r\n^\r\n\
             D05/03/2026\r\nN1234\r\nPDupont\r\nMn°1234\r\nLLoisirs\r\nT-50.00\r\n^\r\n"
        );
    }
}
