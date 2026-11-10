//! Exports tableur et QIF (ex-scripts `export_csv.py` et `export_qif.py`).
//! Le classeur Excel et le CSV partagent le même tableau ([`table`]).

use crate::model::{Account, Amount, Document, Mode, Operation, StandardMode, Tack};
use crate::tr;
use crate::view::{SortColumn, running_balances, sorted_order};

/// Colonnes possibles d'un export d'opérations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportField {
    Account,
    Date,
    Mode,
    Payee,
    Details,
    Category,
    SubCategory,
    Amount,
    Balance,
    Tack,
}

impl ExportField {
    /// Ordre proposé. Le compte vient en dernier : un export de tous les
    /// comptes dans un seul fichier garde les colonnes d'un export d'un
    /// compte à la même place (programmes qui relisent le fichier).
    pub const ALL: [ExportField; 10] = [
        Self::Date,
        Self::Mode,
        Self::Payee,
        Self::Details,
        Self::Category,
        Self::SubCategory,
        Self::Amount,
        Self::Balance,
        Self::Tack,
        Self::Account,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Account => tr!("Compte"),
            Self::Date => tr!("Date"),
            Self::Mode => tr!("Mode"),
            Self::Payee => tr!("Tiers"),
            Self::Details => tr!("Détails"),
            Self::Category => tr!("Catégorie"),
            Self::SubCategory => tr!("Sous-catégorie"),
            Self::Amount => tr!("Montant"),
            Self::Balance => tr!("Solde"),
            Self::Tack => tr!("Pointage"),
        }
    }
}

/// Cellule du tableau exporté.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportValue {
    Text(String),
    Amount(Amount),
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Colonnes exportées, dans l'ordre.
    pub fields: Vec<ExportField>,
    /// Séparateur des colonnes (CSV seulement).
    pub separator: char,
    pub header: bool,
    /// Remplacer « Chq 1234 » par « Chèque émis ».
    pub cheque_as_mode: bool,
    /// Catégorie et sous-catégorie dans une seule colonne, avec ce séparateur.
    pub group_categories: Option<String>,
    /// Symboles des états non pointé, pointé, rapproché.
    pub tack_symbols: [String; 3],
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            // La colonne « Compte » ne sert que si plusieurs comptes se suivent.
            fields: ExportField::ALL
                .into_iter()
                .filter(|f| *f != ExportField::Account)
                .collect(),
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

/// Opérations d'un compte dans l'ordre chronologique, avec le solde de
/// chacune (indexé par opération).
pub fn chronology(document: &Document, account: &Account) -> (Vec<usize>, Vec<Amount>) {
    let order = sorted_order(document, account, SortColumn::Date, true);
    let running = running_balances(account, &order, true);
    let mut balances = vec![Amount::ZERO; account.operations.len()];
    for (&line, balance) in order.iter().zip(running) {
        balances[line] = balance;
    }
    (order, balances)
}

/// Tableau des `lines` (indices d'opérations) d'un compte. `balances` donne
/// le solde de chaque opération, indexé comme `account.operations`.
pub fn table(
    document: &Document,
    account: &Account,
    lines: &[usize],
    balances: &[Amount],
    options: &ExportOptions,
) -> Vec<Vec<ExportValue>> {
    use ExportValue::{Amount as Money, Text};
    let fields: Vec<ExportField> = options
        .fields
        .iter()
        .copied()
        .filter(|f| !(options.group_categories.is_some() && *f == ExportField::SubCategory))
        .collect();
    let mut rows = Vec::with_capacity(lines.len() + 1);
    if options.header {
        rows.push(fields.iter().map(|f| Text(f.label().into())).collect());
    }
    for &i in lines {
        let op = &account.operations[i];
        let (category, sub) = category_parts(document, op);
        let cells = fields
            .iter()
            .map(|field| match field {
                ExportField::Account => Text(account.name.clone()),
                ExportField::Date => Text(op.date.to_string()),
                ExportField::Mode => {
                    if options.cheque_as_mode && op.cheque_number().is_some() {
                        Text(StandardMode::ChequeEmis.label().to_string())
                    } else {
                        Text(op.mode_label())
                    }
                }
                ExportField::Payee => Text(crate::split::payee_label(&op.payee).into_owned()),
                ExportField::Details => Text(op.details.clone()),
                ExportField::Category => Text(match (&options.group_categories, sub.is_empty()) {
                    (Some(joiner), false) => format!("{category}{joiner}{sub}"),
                    _ => category.clone(),
                }),
                ExportField::SubCategory => Text(sub.clone()),
                ExportField::Amount => Money(op.amount),
                ExportField::Balance => Money(balances[i]),
                ExportField::Tack => Text(options.tack_symbols[tack_index(op.tack)].clone()),
            })
            .collect();
        rows.push(cells);
    }
    rows
}

/// Tableau de toutes les opérations du document, avec un seul en-tête :
/// compte par compte dans l'ordre d'affichage, chronologique dans chacun pour
/// que le solde se suive.
pub fn merged_table(document: &Document, options: &ExportOptions) -> Vec<Vec<ExportValue>> {
    let mut options = options.clone();
    let mut rows = Vec::new();
    for index in crate::reports::display_order(document) {
        let account = &document.accounts[index];
        let (lines, balances) = chronology(document, account);
        rows.extend(table(document, account, &lines, &balances, &options));
        options.header = false;
    }
    rows
}

/// Génère le contenu CSV d'un compte (voir [`table`] et [`csv_text`]).
pub fn csv(
    document: &Document,
    account: &Account,
    lines: &[usize],
    balances: &[Amount],
    options: &ExportOptions,
) -> String {
    csv_text(&table(document, account, lines, balances, options), options)
}

/// Texte CSV d'un tableau.
///
/// Pas de guillemets, comme l'original : le séparateur est remplacé par une
/// espace dans les textes. Montants avec deux décimales ; la virgule décimale
/// devient un point si le séparateur est lui-même une virgule (correction :
/// l'original produisait alors un fichier illisible).
pub fn csv_text(rows: &[Vec<ExportValue>], options: &ExportOptions) -> String {
    let sep = options.separator;
    // Séparateur décimal de la langue, sauf s'il se confond avec celui des
    // colonnes.
    let decimal = match crate::i18n::decimal_separator() {
        d if d == sep && d == ',' => '.',
        d if d == sep => ',',
        d => d,
    };
    let mut out = String::new();
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .map(|value| match value {
                ExportValue::Text(text) => text.replace(sep, " ").replace(['\r', '\n'], " "),
                ExportValue::Amount(amount) => amount.format(2, decimal),
            })
            .collect();
        out.push_str(&cells.join(&sep.to_string()));
        out.push_str("\r\n");
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
        let (_, balances) = chronology(&doc, &acc);
        let text = csv(&doc, &acc, &[0, 1], &balances, &ExportOptions::default());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "Date;Mode;Tiers;Détails;Catégorie;Sous-catégorie;Montant;Solde;Pointage"
        );
        assert_eq!(
            lines[1],
            "05/03/2026;Carte;Fnac  Paris;livre;Loisirs;Livres;-12,50;-12,50;P"
        );
        assert_eq!(
            lines[2],
            "05/03/2026;Chq 1234;Dupont;n°1234;Loisirs;;-50,00;-62,50;-"
        );
    }

    /// Le solde suit les dates, pas l'ordre d'enregistrement des opérations.
    #[test]
    fn chronology_orders_balances_by_date() {
        let (doc, mut acc) = sample();
        acc.operations[1].date = Date::from_ymd(2026, 3, 1).unwrap();
        let (order, balances) = chronology(&doc, &acc);
        assert_eq!(order, [1, 0]);
        assert_eq!(
            balances,
            [Amount::from_cents(-6250), Amount::from_cents(-5000)]
        );
    }

    /// Le tableau garde les textes intacts et les montants en nombres.
    #[test]
    fn table_keeps_raw_values() {
        let (doc, acc) = sample();
        let (_, balances) = chronology(&doc, &acc);
        let options = ExportOptions {
            fields: vec![
                ExportField::Payee,
                ExportField::Category,
                ExportField::SubCategory,
                ExportField::Balance,
            ],
            header: false,
            ..Default::default()
        };
        assert_eq!(
            table(&doc, &acc, &[0], &balances, &options),
            [[
                ExportValue::Text("Fnac; Paris".into()),
                ExportValue::Text("Loisirs".into()),
                ExportValue::Text("Livres".into()),
                ExportValue::Amount(Amount::from_cents(-1250)),
            ]]
        );
    }

    /// Tous les comptes dans un tableau : un seul en-tête, l'ordre d'affichage
    /// et le solde propre à chaque compte.
    #[test]
    fn merged_table_follows_each_account() {
        let (mut doc, first) = sample();
        let mut second = first.clone();
        second.name = "Livret".into();
        second.operations.truncate(1);
        doc.accounts = vec![first, second];
        doc.account_order = vec![1, 0];
        let options = ExportOptions {
            fields: vec![ExportField::Account, ExportField::Balance],
            ..Default::default()
        };
        let text = |t: &str| ExportValue::Text(t.into());
        let money = |cents| ExportValue::Amount(Amount::from_cents(cents));
        assert_eq!(
            merged_table(&doc, &options),
            [
                [text("Compte"), text("Solde")],
                [text("Livret"), money(-1250)],
                [text("Courant"), money(-1250)],
                [text("Courant"), money(-6250)],
            ]
        );
    }

    /// Le CSV d'un tableau réuni garde la colonne du compte, débarrassée du
    /// séparateur comme les autres textes.
    #[test]
    fn merged_csv_keeps_the_account() {
        let (mut doc, mut account) = sample();
        account.name = "Courant; joint".into();
        account.operations.truncate(1);
        doc.accounts = vec![account];
        doc.account_order = vec![0];
        let options = ExportOptions {
            fields: vec![ExportField::Account, ExportField::Amount],
            header: false,
            ..Default::default()
        };
        assert_eq!(
            csv_text(&merged_table(&doc, &options), &options),
            "Courant  joint;-12,50\r\n"
        );
    }

    #[test]
    fn csv_options() {
        let (doc, acc) = sample();
        let options = ExportOptions {
            fields: vec![
                ExportField::Amount,
                ExportField::Mode,
                ExportField::Category,
                ExportField::SubCategory,
            ],
            separator: ',',
            header: false,
            cheque_as_mode: true,
            group_categories: Some(" > ".into()),
            ..Default::default()
        };
        let text = csv(&doc, &acc, &[0, 1], &[Amount::ZERO; 2], &options);
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
