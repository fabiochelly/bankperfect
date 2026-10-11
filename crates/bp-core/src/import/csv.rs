//! Import CSV/TSV, ex-script `import_csv.py`.
//!
//! L'utilisateur indique le séparateur, les lignes à ignorer, l'ordre de la
//! date, le séparateur décimal et la colonne de chaque champ
//! ([`CsvColumns`], devinée d'après la ligne d'en-tête par
//! [`CsvColumns::guess`]). Le montant vient soit d'une colonne Montant, soit
//! de colonnes Débit et/ou Crédit.
//!
//! L'analyseur ([`read_records`]) gère les guillemets, les guillemets
//! doublés et les cellules sur plusieurs lignes, comme la classe `SCSV` de
//! l'original.

use super::{
    DateOrder, ImportError, ImportedAccount, ParsedImport, RawFields, RejectedLine,
    build_operation, clean_text, decode_text, parse_amount, parse_date, pattern,
};
use crate::model::{Amount, Date, Tack};

/// Colonne (à partir de 0) de chaque champ ; `None` = champ absent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CsvColumns {
    pub date: Option<usize>,
    pub value_date: Option<usize>,
    pub mode: Option<usize>,
    pub payee: Option<usize>,
    pub details: Option<usize>,
    pub category: Option<usize>,
    pub subcategory: Option<usize>,
    /// Montant signé ; exclusif de `debit`/`credit`.
    pub amount: Option<usize>,
    /// Dépenses (montants rendus négatifs quel que soit leur signe).
    pub debit: Option<usize>,
    /// Recettes (montants rendus positifs quel que soit leur signe).
    pub credit: Option<usize>,
    /// Pointage : `P` ou `O` = pointé, `R` ou `N` = rapproché (conventions de
    /// l'original), autre valeur = non pointé.
    pub tack: Option<usize>,
}

impl CsvColumns {
    /// Devine les colonnes d'après les titres de la ligne d'en-tête
    /// (`guess`), comparés par motif (minuscules, sans accents ni
    /// ponctuation). Pour chaque champ, la première colonne qui convient est
    /// retenue.
    pub fn guess(header: &[String]) -> CsvColumns {
        let titles: Vec<String> = header.iter().map(|t| pattern(t)).collect();
        let find = |names: &[&str]| titles.iter().position(|t| names.contains(&t.as_str()));
        CsvColumns {
            date: find(&[
                "date",
                "dateoperation",
                "datedoperation",
                "dateop",
                "datecomptable",
                "datecompta",
            ]),
            value_date: find(&["datevaleur", "datedevaleur", "valeur"]),
            mode: find(&["mode", "type", "typeoperation", "modedepaiement"]),
            payee: find(&[
                "tiers",
                "libelle",
                "libelleoperation",
                "libellesimplifie",
                "beneficiaire",
                "description",
            ]),
            details: find(&[
                "details",
                "detail",
                "note",
                "notes",
                "memo",
                "informationscomplementaires",
            ]),
            category: find(&["categorie", "categ"]),
            subcategory: find(&[
                "souscategorie",
                "sscategorie",
                "sscateg",
                "souscateg",
                "scateg",
                "scategorie",
            ]),
            amount: find(&[
                "montant",
                "montanteuro",
                "montanteuros",
                "montanteur",
                "amount",
            ]),
            debit: find(&["debit", "debiteuros", "debiteuro", "debiteur"]),
            credit: find(&["credit", "crediteuro", "crediteuros", "crediteur"]),
            tack: find(&["pointage", "attente"]),
        }
    }

    /// Vérifie qu'une date et un montant (colonne Montant, ou Débit et/ou
    /// Crédit, mais pas les deux) sont indiqués.
    pub fn validate(&self) -> Result<(), ImportError> {
        let error = |message: &str| Err(ImportError::InvalidConfig(message.to_string()));
        if self.date.is_none() {
            error("la colonne de la date n'est pas indiquée")
        } else if self.amount.is_some() && (self.debit.is_some() || self.credit.is_some()) {
            error("indiquez soit la colonne Montant, soit les colonnes Débit et Crédit")
        } else if self.amount.is_none() && self.debit.is_none() && self.credit.is_none() {
            error("la colonne du montant n'est pas indiquée")
        } else {
            Ok(())
        }
    }
}

/// Paramètres de l'import CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct CsvConfig {
    /// Séparateur de colonnes : `;`, `,` ou tabulation.
    pub separator: char,
    /// Nombre de lignes (enregistrements) ignorées au début : 1 pour une
    /// ligne d'en-tête.
    pub skip_rows: usize,
    /// Dernière ligne (enregistrement) lue, à partir de 1 ; `None` = jusqu'à
    /// la fin.
    pub last_row: Option<usize>,
    pub date_order: DateOrder,
    /// Séparateur décimal imposé ; `None` = détection (voir
    /// [`super::parse_amount`]).
    pub decimal_separator: Option<char>,
    pub columns: CsvColumns,
    /// Date du jour : années sur 2 chiffres, dates sans année.
    pub today: Date,
}

impl Default for CsvConfig {
    fn default() -> Self {
        Self {
            separator: ';',
            skip_rows: 1,
            last_row: None,
            date_order: DateOrder::DayMonthYear,
            decimal_separator: None,
            columns: CsvColumns::default(),
            today: Date::today(),
        }
    }
}

impl CsvConfig {
    /// Réglages prédéfinis des fichiers `.tsv` « Vidéoposte » de l'original :
    /// tabulation, 9 lignes d'en-tête, date en colonne 1, mode et tiers en
    /// colonne 2, montant en colonne 3.
    pub fn videoposte() -> CsvConfig {
        CsvConfig {
            separator: '\t',
            skip_rows: 9,
            columns: CsvColumns {
                date: Some(0),
                mode: Some(1),
                payee: Some(1),
                amount: Some(2),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// Enregistrement CSV : ses cellules et le numéro de la ligne du fichier où
/// il commence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvRecord {
    pub line: usize,
    pub cells: Vec<String>,
}

/// Découpe un texte CSV en enregistrements. Une cellule qui commence par un
/// guillemet (après d'éventuelles espaces) peut contenir le séparateur, des
/// retours à la ligne et des guillemets doublés. Une ligne vide donne un
/// enregistrement à une cellule vide (la numérotation des lignes reste celle
/// de l'aperçu).
pub fn read_records(text: &str, separator: char) -> Vec<CsvRecord> {
    let mut records = Vec::new();
    let mut cells = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut started = false;
    let mut line = 1;
    let mut record_line = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                '\r' | '\n' => {
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    line += 1;
                    field.push('\n');
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.trim().is_empty() => {
                field.clear();
                in_quotes = true;
                started = true;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                cells.push(std::mem::take(&mut field));
                records.push(CsvRecord {
                    line: record_line,
                    cells: std::mem::take(&mut cells),
                });
                line += 1;
                record_line = line;
                started = false;
            }
            c if c == separator => {
                cells.push(std::mem::take(&mut field));
                started = true;
            }
            _ => {
                field.push(c);
                started = true;
            }
        }
    }
    if started || !field.is_empty() {
        cells.push(field);
        records.push(CsvRecord {
            line: record_line,
            cells,
        });
    }
    records
}

/// Devine le séparateur d'après la première ligne non vide : le plus
/// fréquent de `;`, tabulation et `,` (`;` par défaut).
pub fn detect_separator(text: &str) -> char {
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut best = (';', 0);
    for sep in [';', '\t', ','] {
        let count = first.matches(sep).count();
        if count > best.1 {
            best = (sep, count);
        }
    }
    best.0
}

/// Pointage lu dans une cellule (voir [`CsvColumns::tack`]).
pub fn parse_tack(value: &str) -> Tack {
    match value.trim().to_uppercase().as_str() {
        "P" | "O" => Tack::Pointed,
        "R" | "N" => Tack::Reconciled,
        _ => Tack::None,
    }
}

/// Analyse un fichier CSV selon `config`.
///
/// Erreur [`ImportError::InvalidConfig`] si les colonnes ne permettent pas de
/// lire une date et un montant. Les lignes entièrement vides sont ignorées ;
/// les lignes sans date ou sans montant lisible sont signalées dans
/// [`ParsedImport::rejected`]. Le résultat contient un seul compte, sans
/// identifiant.
pub fn parse(data: &[u8], config: &CsvConfig) -> Result<ParsedImport, ImportError> {
    config.columns.validate()?;
    let text = decode_text(data);
    let columns = &config.columns;
    let mut account = ImportedAccount::default();
    let mut rejected = Vec::new();
    for (index, record) in read_records(&text, config.separator).iter().enumerate() {
        let row = index + 1;
        if row <= config.skip_rows {
            continue;
        }
        if config.last_row.is_some_and(|last| row > last) {
            break;
        }
        if record.cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let cell = |column: Option<usize>| {
            column
                .and_then(|c| record.cells.get(c))
                .map(|s| clean_text(s))
                .unwrap_or_default()
        };
        let amount = read_amount(columns, config.decimal_separator, &cell);
        let date = cell(columns.date);
        let (mode, payee, details) = (
            cell(columns.mode),
            cell(columns.payee),
            cell(columns.details),
        );
        let (category, subcategory) = (cell(columns.category), cell(columns.subcategory));
        let raw = RawFields {
            line: record.line,
            date: Some(&date),
            amount,
            mode: &mode,
            payee: &payee,
            details: &details,
            category: &category,
            subcategory: &subcategory,
            tack: columns.tack.map(|c| parse_tack(&cell(Some(c)))),
            value_date: parse_date(&cell(columns.value_date), config.date_order, config.today),
        };
        match build_operation(raw, config.date_order, config.today) {
            Ok(op) => account.operations.push(op),
            Err(reason) => rejected.push(RejectedLine {
                line: record.line,
                text: record
                    .cells
                    .iter()
                    .map(|c| clean_text(c))
                    .collect::<Vec<_>>()
                    .join(&config.separator.to_string()),
                reason,
            }),
        }
    }
    let accounts = if account.operations.is_empty() {
        Vec::new()
    } else {
        vec![account]
    };
    Ok(ParsedImport { accounts, rejected })
}

/// Montant d'une ligne : colonne Montant, ou somme des colonnes Crédit
/// (positif) et Débit (négatif). Correction C45 : l'original ne gardait que
/// la dernière colonne lue, si bien qu'un débit accompagné d'un crédit
/// `0,00` donnait un montant nul et la ligne était refusée.
fn read_amount(
    columns: &CsvColumns,
    decimal: Option<char>,
    cell: &dyn Fn(Option<usize>) -> String,
) -> Result<Option<Amount>, String> {
    let parse = |text: &str| parse_amount(text, decimal).ok_or_else(|| text.to_string());
    if columns.amount.is_some() {
        let text = cell(columns.amount);
        return if text.is_empty() {
            Ok(None)
        } else {
            parse(&text).map(Some)
        };
    }
    let mut total: Option<Amount> = None;
    for (column, sign) in [(columns.debit, -1), (columns.credit, 1)] {
        let text = cell(column);
        if column.is_none() || text.is_empty() {
            continue;
        }
        let value = parse(&text)?;
        let signed = Amount(sign * value.0.abs());
        total = Some(total.unwrap_or(Amount::ZERO) + signed);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::RejectReason;
    use crate::import::tests::{today, ymd};
    use crate::model::{Mode, StandardMode};

    const BANK: &str = "\"Date opération\";\"Date valeur\";\"Libellé\";\"Débit\";\"Crédit\"\r\n\
05/03/2026;06/03/2026;\"CB FNAC PARIS\";12,50;\r\n\
06/03/2026;06/03/2026;\"VIR SEPA EMPLOYEUR\r\n\
SALAIRE MARS\";;1 500,00\r\n\
07/03/2026;07/03/2026;\"PRLV \"\"EDF\"\"\";45,00;0,00\r\n\
31/02/2026;;ERREUR;1,00;\r\n\
08/03/2026;;MONTANT FAUX;abc;\r\n\
;;;;\r\n\
09/03/2026;;SANS MONTANT;;\r\n\
10/03/2026;;ZERO;0,00;\r\n";

    fn bank_config() -> CsvConfig {
        let records = read_records(BANK, ';');
        CsvConfig {
            columns: CsvColumns::guess(&records[0].cells),
            today: today(),
            ..Default::default()
        }
    }

    #[test]
    fn records() {
        let records = read_records(BANK, ';');
        assert_eq!(records.len(), 9);
        assert_eq!(
            records[0].cells,
            vec![
                "Date opération",
                "Date valeur",
                "Libellé",
                "Débit",
                "Crédit"
            ]
        );
        assert_eq!(records[2].line, 3);
        assert_eq!(records[2].cells[2], "VIR SEPA EMPLOYEUR\nSALAIRE MARS");
        assert_eq!(records[3].line, 5);
        assert_eq!(records[3].cells[2], "PRLV \"EDF\"");
        assert_eq!(records[3].cells[4], "0,00");

        let records = read_records("a,\"b, c\" ,d\n\nx", ',');
        assert_eq!(records[0].cells, vec!["a", "b, c ", "d"]);
        assert_eq!(records[1].cells, vec![""]);
        assert_eq!(records[2].cells, vec!["x"]);
        assert_eq!(records[2].line, 3);
        // Guillemet non fermé : pas de panique, la cellule va jusqu'à la fin.
        let records = read_records("a;\"bc\nd", ';');
        assert_eq!(records[0].cells, vec!["a", "bc\nd"]);
    }

    #[test]
    fn guess_columns() {
        let config = bank_config();
        assert_eq!(
            config.columns,
            CsvColumns {
                date: Some(0),
                value_date: Some(1),
                payee: Some(2),
                debit: Some(3),
                credit: Some(4),
                ..Default::default()
            }
        );
        let header: Vec<String> =
            "Date;Mode;Tiers;Détails;Catégorie;Sous-catégorie;Montant;Pointage"
                .split(';')
                .map(String::from)
                .collect();
        let columns = CsvColumns::guess(&header);
        assert_eq!(columns.amount, Some(6));
        assert_eq!(columns.subcategory, Some(5));
        assert_eq!(columns.tack, Some(7));
        assert_eq!(columns.debit, None);
    }

    #[test]
    fn parse_debit_credit() {
        let parsed = parse(BANK.as_bytes(), &bank_config()).unwrap();
        assert_eq!(parsed.accounts.len(), 1);
        let ops = &parsed.accounts[0].operations;
        assert_eq!(ops.len(), 3);

        assert_eq!(ops[0].date, ymd(2026, 3, 5));
        assert_eq!(ops[0].value_date, Some(ymd(2026, 3, 6)));
        assert_eq!(ops[0].amount, Amount::from_cents(-1250));
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::Carte));
        assert_eq!(ops[0].payee, "CB FNAC PARIS");
        assert_eq!(ops[0].tack, None);

        assert_eq!(ops[1].amount, Amount::from_cents(150_000));
        assert_eq!(ops[1].mode, Mode::Standard(StandardMode::VirementRecu));
        assert_eq!(ops[1].payee, "VIR SEPA EMPLOYEUR SALAIRE MARS");
        assert_eq!(ops[1].line, 3);

        // Débit 45,00 et crédit 0,00 : -45,00 (C45).
        assert_eq!(ops[2].amount, Amount::from_cents(-4500));
        assert_eq!(ops[2].mode, Mode::Standard(StandardMode::Prelevement));
        assert_eq!(ops[2].payee, "PRLV \"EDF\"");

        let rejected: Vec<(usize, &RejectReason)> = parsed
            .rejected
            .iter()
            .map(|r| (r.line, &r.reason))
            .collect();
        assert_eq!(
            rejected,
            vec![
                (6, &RejectReason::InvalidDate("31/02/2026".into())),
                (7, &RejectReason::InvalidAmount("abc".into())),
                (9, &RejectReason::MissingAmount),
                (10, &RejectReason::ZeroAmount),
            ]
        );
        assert_eq!(parsed.rejected[0].text, "31/02/2026;;ERREUR;1,00;");
    }

    #[test]
    fn row_bounds() {
        let mut config = bank_config();
        config.skip_rows = 2;
        config.last_row = Some(3);
        let parsed = parse(BANK.as_bytes(), &config).unwrap();
        assert_eq!(parsed.operation_count(), 1);
        assert_eq!(
            parsed.accounts[0].operations[0].amount,
            Amount::from_cents(150_000)
        );
        assert!(parsed.rejected.is_empty());

        config.skip_rows = 0;
        config.last_row = Some(1);
        let parsed = parse(BANK.as_bytes(), &config).unwrap();
        // L'en-tête lu comme une ligne : date incorrecte.
        assert!(parsed.accounts.is_empty());
        assert_eq!(
            parsed.rejected[0].reason,
            RejectReason::InvalidDate("Date opération".into())
        );
    }

    #[test]
    fn config_errors() {
        let mut config = CsvConfig {
            today: today(),
            ..Default::default()
        };
        assert!(matches!(
            parse(b"", &config),
            Err(ImportError::InvalidConfig(_))
        ));
        config.columns.date = Some(0);
        assert!(matches!(
            parse(b"", &config),
            Err(ImportError::InvalidConfig(_))
        ));
        config.columns.amount = Some(1);
        config.columns.debit = Some(2);
        assert!(matches!(
            parse(b"", &config),
            Err(ImportError::InvalidConfig(_))
        ));
        config.columns.debit = None;
        assert_eq!(parse(b"", &config), Ok(ParsedImport::default()));
        // Débit seul : accepté.
        config.columns.amount = None;
        config.columns.debit = Some(1);
        assert!(config.columns.validate().is_ok());
    }

    #[test]
    fn comma_separator_and_options() {
        let text = "Date,Mode,Amount,Category,Pointage\n\
2026-03-05,CHQ 1234,\"-1,234.50\",Maison:Travaux,R\n\
2026-03-06,Remise,\"2,000.00\",Divers,x\n";
        let records = read_records(text, ',');
        let config = CsvConfig {
            separator: ',',
            date_order: DateOrder::YearMonthDay,
            decimal_separator: Some('.'),
            columns: CsvColumns {
                category: Some(3),
                ..CsvColumns::guess(&records[0].cells)
            },
            today: today(),
            ..Default::default()
        };
        assert_eq!(config.columns.mode, Some(1));
        let parsed = parse(text.as_bytes(), &config).unwrap();
        let ops = &parsed.accounts[0].operations;
        assert_eq!(ops[0].amount, Amount::from_cents(-123_450));
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::ChequeEmis));
        assert_eq!(ops[0].cheque_number.as_deref(), Some("1234"));
        assert_eq!(ops[0].category.as_deref(), Some("Maison:Travaux"));
        assert_eq!(ops[0].tack, Some(Tack::Reconciled));
        assert_eq!(ops[1].mode, Mode::Standard(StandardMode::DepotCheque));
        assert_eq!(ops[1].tack, Some(Tack::None));
        assert_eq!(detect_separator(text), ',');
        assert_eq!(detect_separator("a\tb\tc;d\n"), '\t');
        assert_eq!(detect_separator(""), ';');
    }

    #[test]
    fn videoposte_and_encoding() {
        let mut data = Vec::new();
        for i in 0..9 {
            data.extend_from_slice(format!("En-tête {i}\n").as_bytes());
        }
        data.extend_from_slice(b"05/03/2026\tCB Caf\xE9\t-3,20\n");
        let config = CsvConfig {
            today: today(),
            ..CsvConfig::videoposte()
        };
        let parsed = parse(&data, &config).unwrap();
        assert!(parsed.rejected.is_empty());
        let op = &parsed.accounts[0].operations[0];
        assert_eq!(op.payee, "CB Café");
        assert_eq!(op.mode, Mode::Standard(StandardMode::Carte));
        assert_eq!(op.amount, Amount::from_cents(-320));
        assert_eq!(op.line, 10);

        // UTF-8 avec BOM.
        let mut data = b"\xEF\xBB\xBFDate;Montant;Tiers\n".to_vec();
        data.extend_from_slice("05/03/2026;-1;Café\n".as_bytes());
        let records = read_records(&decode_text(&data), ';');
        let config = CsvConfig {
            columns: CsvColumns::guess(&records[0].cells),
            today: today(),
            ..Default::default()
        };
        let parsed = parse(&data, &config).unwrap();
        assert_eq!(parsed.accounts[0].operations[0].payee, "Café");
    }

    /// Un export CSV de BankPerfect se réimporte à l'identique.
    #[test]
    fn export_round_trip() {
        use crate::export;
        use crate::import::tests::{document, empty_account, existing};
        use crate::import::{ConvertOptions, to_operation};

        let doc = document(&[(0, "Loisirs", false), (1, "Livres", true)]);
        let mut account = empty_account();
        let mut card = existing(ymd(2026, 3, 5), -1250, StandardMode::Carte, "Fnac Paris");
        card.details = "livre".into();
        card.category = Some(1);
        card.tack = Tack::Pointed;
        let mut cheque = existing(ymd(2026, 3, 6), -5000, StandardMode::ChequeEmis, "Dupont");
        cheque.details = "n°1234".into();
        cheque.category = Some(0);
        let titre = existing(ymd(2026, 3, 7), 99, StandardMode::Titre, "Dividende");
        account.operations = vec![card, cheque, titre];

        let text = export::csv(&doc, &account, &[0, 1, 2], &export::CsvOptions::default());
        let records = read_records(&text, ';');
        let config = CsvConfig {
            columns: CsvColumns::guess(&records[0].cells),
            today: today(),
            ..Default::default()
        };
        let parsed = parse(text.as_bytes(), &config).unwrap();
        assert!(parsed.rejected.is_empty());
        for (imported, original) in parsed.accounts[0]
            .operations
            .iter()
            .zip(&account.operations)
        {
            let op = to_operation(&doc, imported, &ConvertOptions::default());
            assert_eq!(op.date, original.date);
            assert_eq!(op.amount, original.amount);
            assert_eq!(op.mode, original.mode);
            assert_eq!(op.payee, original.payee);
            assert_eq!(op.details, original.details);
            assert_eq!(op.category, original.category);
            assert_eq!(op.tack, original.tack);
        }
    }
}
