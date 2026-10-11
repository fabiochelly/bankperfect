//! Import QIF (Quicken Interchange Format), ex-script `import_qif.py`.
//!
//! Un fichier QIF est une suite d'enregistrements terminés par `^` ; chaque
//! ligne commence par une lettre qui désigne le champ : `D` date, `T` (ou
//! `U`) montant, `P` tiers, `M` mémo (détail), `N` numéro ou mode, `L`
//! catégorie (`Catégorie:Sous-catégorie`), `C` pointage. La correspondance
//! lettre → champ est paramétrable comme dans l'original.
//!
//! Écarts avec l'original (voir `docs/corrections.md`) : les en-têtes `!Type:`
//! non bancaires (catégories, classes, mémorisés, placements) et la liste
//! `!Account` ne sont plus lus comme des opérations, chaque `!Account` donne
//! un compte distinct, le dernier enregistrement sans `^` final n'est plus
//! perdu, les virements `L[Compte]` ne créent plus de catégorie, et les années
//! sur 2 chiffres suivent une fenêtre glissante (`98` = 1998).

use super::{
    DateOrder, ImportError, ImportedAccount, ParsedImport, RawFields, RejectedLine,
    build_operation, decode_text, parse_amount,
};
use crate::model::{Date, Tack};

/// Champ BankPerfect associé à une lettre QIF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QifField {
    Date,
    Mode,
    Payee,
    Details,
    Category,
    Amount,
    Tack,
}

impl QifField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Date => "Date",
            Self::Mode => "Mode",
            Self::Payee => "Tiers",
            Self::Details => "Détails",
            Self::Category => "Catégorie",
            Self::Amount => "Montant",
            Self::Tack => "Pointage",
        }
    }
}

/// Paramètres de l'import QIF.
#[derive(Debug, Clone, PartialEq)]
pub struct QifOptions {
    pub date_order: DateOrder,
    /// Correspondance lettre → champ ; les lettres absentes sont ignorées.
    pub fields: Vec<(char, QifField)>,
    /// Date du jour : années sur 2 chiffres, dates sans année.
    pub today: Date,
}

impl QifOptions {
    /// Correspondance par défaut : `D` date, `N` mode, `P` tiers, `M` détail,
    /// `L` catégorie, `T` et `U` montant, `C` pointage.
    pub fn default_fields() -> Vec<(char, QifField)> {
        vec![
            ('D', QifField::Date),
            ('N', QifField::Mode),
            ('P', QifField::Payee),
            ('M', QifField::Details),
            ('L', QifField::Category),
            ('T', QifField::Amount),
            ('U', QifField::Amount),
            ('C', QifField::Tack),
        ]
    }

    fn field(&self, letter: char) -> Option<QifField> {
        self.fields
            .iter()
            .find(|(l, _)| *l == letter)
            .map(|(_, f)| *f)
    }
}

impl Default for QifOptions {
    fn default() -> Self {
        Self {
            date_order: DateOrder::DayMonthYear,
            fields: Self::default_fields(),
            today: Date::today(),
        }
    }
}

/// Lettres utilisées par les lignes du fichier, triées (`getprefixes`), pour
/// proposer la correspondance à l'utilisateur.
pub fn letters(data: &[u8]) -> Vec<char> {
    let text = decode_text(data);
    let mut letters: Vec<char> = text
        .lines()
        .filter_map(|l| l.trim().chars().next())
        .filter(char::is_ascii_uppercase)
        .collect();
    letters.sort_unstable();
    letters.dedup();
    letters
}

/// Dates brutes (champ `D`) des premiers enregistrements, pour deviner
/// l'ordre de la date ([`DateOrder::guess`]).
pub fn sample_dates(data: &[u8], limit: usize) -> Vec<String> {
    decode_text(data)
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix('D'))
        .take(limit)
        .map(str::to_string)
        .collect()
}

/// Section du fichier en cours de lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Transactions,
    AccountList,
    Ignored,
}

/// Types `!Type:` contenant des opérations bancaires.
fn is_transaction_type(kind: &str) -> bool {
    matches!(
        kind.trim().to_lowercase().as_str(),
        "bank" | "cash" | "ccard" | "oth a" | "oth l"
    )
}

/// Pointage QIF : `*` ou `c` = pointé, `X` ou `R` = rapproché.
fn parse_tack(value: &str) -> Option<Tack> {
    match value.trim() {
        "*" | "c" | "C" => Some(Tack::Pointed),
        "X" | "x" | "R" | "r" => Some(Tack::Reconciled),
        _ => None,
    }
}

/// Catégorie QIF `Catégorie:Sous-catégorie/Classe` ; `[Compte]` désigne un
/// virement de compte à compte, pas une catégorie.
fn split_category(value: &str) -> (String, String) {
    let value = value.trim();
    if value.starts_with('[') {
        return (String::new(), String::new());
    }
    let value = value.split('/').next().unwrap_or("");
    match value.split_once(':') {
        Some((cat, sub)) => (
            cat.trim().to_string(),
            sub.replace(':', " ").trim().to_string(),
        ),
        None => (value.to_string(), String::new()),
    }
}

struct Parser<'a> {
    options: &'a QifOptions,
    result: ParsedImport,
    current: Option<ImportedAccount>,
    pending_name: Option<String>,
}

impl Parser<'_> {
    fn start_account(&mut self, name: Option<String>) {
        if let Some(account) = self.current.take() {
            self.result.accounts.push(account);
        }
        self.current = Some(ImportedAccount {
            name,
            ..Default::default()
        });
    }

    fn record(&mut self, section: Section, start_line: usize, lines: &[(char, &str)]) {
        if lines.is_empty() {
            return;
        }
        match section {
            Section::Ignored => {}
            Section::AccountList => {
                if let Some((_, name)) = lines.iter().rev().find(|(l, _)| *l == 'N') {
                    self.pending_name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
                }
            }
            Section::Transactions => self.transaction(start_line, lines),
        }
    }

    fn transaction(&mut self, start_line: usize, lines: &[(char, &str)]) {
        let mut date = None;
        let mut amount_text: Option<&str> = None;
        let mut mode = String::new();
        let mut payee = String::new();
        let mut details = String::new();
        let mut category = (String::new(), String::new());
        let mut tack = None;
        let append = |target: &mut String, value: &str| {
            if !value.trim().is_empty() {
                if !target.is_empty() {
                    target.push(' ');
                }
                target.push_str(value.trim());
            }
        };
        for &(letter, value) in lines {
            match self.options.field(letter) {
                Some(QifField::Date) => {
                    date.get_or_insert(value);
                }
                Some(QifField::Amount) => {
                    amount_text.get_or_insert(value);
                }
                Some(QifField::Mode) => append(&mut mode, value),
                Some(QifField::Payee) => append(&mut payee, value),
                Some(QifField::Details) => append(&mut details, value),
                Some(QifField::Category) => {
                    if category.0.is_empty() && category.1.is_empty() {
                        category = split_category(value);
                    }
                }
                Some(QifField::Tack) => tack = parse_tack(value),
                None => {}
            }
        }
        let amount = match amount_text.map(str::trim).filter(|t| !t.is_empty()) {
            None => Ok(None),
            Some(text) => parse_amount(text, None)
                .map(Some)
                .ok_or_else(|| text.to_string()),
        };
        let raw = RawFields {
            line: start_line,
            date,
            amount,
            mode: &mode,
            payee: &payee,
            details: &details,
            category: &category.0,
            subcategory: &category.1,
            tack,
            value_date: None,
        };
        match build_operation(raw, self.options.date_order, self.options.today) {
            Ok(op) => {
                if self.current.is_none() {
                    self.start_account(None);
                }
                if let Some(account) = self.current.as_mut() {
                    account.operations.push(op);
                }
            }
            Err(reason) => self.result.rejected.push(RejectedLine {
                line: start_line,
                text: lines
                    .iter()
                    .map(|(l, v)| format!("{l}{}", v.trim()))
                    .collect::<Vec<_>>()
                    .join(", "),
                reason,
            }),
        }
    }
}

/// Analyse un fichier QIF.
///
/// Erreur [`ImportError::NotQif`] si le fichier ne contient ni fin
/// d'enregistrement `^` ni en-tête `!Type:`, ou si les lettres associées à la
/// date et au montant n'y figurent pas. Les enregistrements sans date ou sans
/// montant lisible sont signalés dans [`ParsedImport::rejected`].
pub fn parse(data: &[u8], options: &QifOptions) -> Result<ParsedImport, ImportError> {
    let text = decode_text(data);
    let used = letters(data);
    let has_letter_for = |field: QifField| {
        options
            .fields
            .iter()
            .any(|(l, f)| *f == field && used.contains(l))
    };
    let has_structure = text.lines().any(|l| {
        let l = l.trim();
        l == "^" || l.to_lowercase().starts_with("!type:")
    });
    if !has_structure || !has_letter_for(QifField::Date) || !has_letter_for(QifField::Amount) {
        return Err(ImportError::NotQif);
    }

    let mut parser = Parser {
        options,
        result: ParsedImport::default(),
        current: None,
        pending_name: None,
    };
    let mut section = Section::Transactions;
    let mut record: Vec<(char, &str)> = Vec::new();
    let mut record_start = 0;
    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('!') {
            parser.record(section, record_start, &record);
            record.clear();
            let header = line.to_lowercase();
            if let Some(kind) = header.strip_prefix("!type:") {
                section = if is_transaction_type(kind) {
                    if let Some(name) = parser.pending_name.take() {
                        parser.start_account(Some(name));
                    }
                    Section::Transactions
                } else {
                    Section::Ignored
                };
            } else if header.starts_with("!account") {
                section = Section::AccountList;
            } else if !header.starts_with("!option") && !header.starts_with("!clear") {
                section = Section::Ignored;
            }
            continue;
        }
        if line == "^" {
            parser.record(section, record_start, &record);
            record.clear();
            continue;
        }
        let mut chars = line.chars();
        let Some(letter) = chars.next().filter(char::is_ascii_uppercase) else {
            continue;
        };
        if record.is_empty() {
            record_start = line_number;
        }
        record.push((letter, chars.as_str()));
    }
    // Dernier enregistrement sans `^` final.
    parser.record(section, record_start, &record);
    if let Some(account) = parser.current.take() {
        parser.result.accounts.push(account);
    }
    parser.result.accounts.retain(|a| !a.operations.is_empty());
    Ok(parser.result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::tests::{today, ymd};
    use crate::import::{ImportStatus, RejectReason, to_operation};
    use crate::model::{Amount, Mode, StandardMode};

    fn options() -> QifOptions {
        QifOptions {
            today: today(),
            ..Default::default()
        }
    }

    const SAMPLE: &str = "!Type:Bank\r\n\
D05/03/2026\r\n\
T-12.50\r\n\
PCB FNAC PARIS\r\n\
MLivre\r\n\
LLoisirs:Livres\r\n\
C*\r\n\
^\r\n\
D06/03/2026\r\n\
T-50,00\r\n\
N1234\r\n\
PDupont\r\n\
^\r\n\
D07/03/26\r\n\
U1 500,00\r\n\
T1 500,00\r\n\
PEMPLOYEUR\r\n\
NVIR\r\n\
^\r\n\
D31/02/2026\r\n\
T-1\r\n\
^\r\n\
D08/03/2026\r\n\
Tabc\r\n\
^\r\n\
PSans date\r\n\
T-3\r\n\
^\r\n\
D09/03/2026\r\n\
T-20\r\n\
L[Livret A]\r\n\
PEPARGNE\r\n";

    #[test]
    fn parse_sample() {
        let parsed = parse(SAMPLE.as_bytes(), &options()).unwrap();
        assert_eq!(parsed.accounts.len(), 1);
        let ops = &parsed.accounts[0].operations;
        assert_eq!(ops.len(), 4);

        assert_eq!(ops[0].line, 2);
        assert_eq!(ops[0].date, ymd(2026, 3, 5));
        assert_eq!(ops[0].amount, Amount::from_cents(-1250));
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::Carte));
        assert_eq!(ops[0].payee, "CB FNAC PARIS");
        assert_eq!(ops[0].details, "Livre");
        assert_eq!(ops[0].category.as_deref(), Some("Loisirs"));
        assert_eq!(ops[0].subcategory.as_deref(), Some("Livres"));
        assert_eq!(ops[0].tack, Some(Tack::Pointed));

        assert_eq!(ops[1].mode, Mode::Standard(StandardMode::ChequeEmis));
        assert_eq!(ops[1].cheque_number.as_deref(), Some("1234"));
        assert_eq!(ops[1].tack, None);

        assert_eq!(ops[2].date, ymd(2026, 3, 7));
        assert_eq!(ops[2].amount, Amount::from_cents(150_000));
        assert_eq!(ops[2].mode, Mode::Standard(StandardMode::VirementRecu));

        // Dernier enregistrement sans `^`, virement vers un compte : pas de
        // catégorie.
        assert_eq!(ops[3].payee, "EPARGNE");
        assert_eq!(ops[3].category, None);

        let reasons: Vec<&RejectReason> = parsed.rejected.iter().map(|r| &r.reason).collect();
        assert_eq!(
            reasons,
            vec![
                &RejectReason::InvalidDate("31/02/2026".into()),
                &RejectReason::InvalidAmount("abc".into()),
                &RejectReason::MissingDate,
            ]
        );
        assert_eq!(parsed.rejected[0].line, 20);
        assert_eq!(parsed.rejected[2].text, "PSans date, T-3");
    }

    #[test]
    fn us_dates_and_custom_mapping() {
        let text = "!Type:CCard\nD12/25'98\nT-10.00\nPStore\nAAddress line\n^\nD1/ 5'02\nT-1.00\nPOther\n^\n";
        let mut options = options();
        options.date_order = DateOrder::MonthDayYear;
        let parsed = parse(text.as_bytes(), &options).unwrap();
        let ops = &parsed.accounts[0].operations;
        assert_eq!(ops[0].date, ymd(1998, 12, 25));
        assert_eq!(ops[1].date, ymd(2002, 1, 5));

        // Le tiers lu dans la lettre A, la lettre P ignorée.
        options.fields = vec![
            ('D', QifField::Date),
            ('T', QifField::Amount),
            ('A', QifField::Payee),
        ];
        let parsed = parse(text.as_bytes(), &options).unwrap();
        assert_eq!(parsed.accounts[0].operations[0].payee, "Address line");
        assert_eq!(parsed.accounts[0].operations[1].payee, "");
    }

    #[test]
    fn several_accounts_and_ignored_sections() {
        let text = "!Type:Cat\nNAlimentation\nE\n^\n\
!Option:AutoSwitch\n!Account\nNCompte courant\nTBank\n^\n\
!Type:Bank\nD01/03/2026\nT-5\nPA\n^\n\
!Account\nNLivret\nTBank\n^\n\
!Type:Bank\nD02/03/2026\nT100\nPB\n^\n\
!Type:Memorized\nKC\nT-9\nPModèle\n^\n";
        let parsed = parse(text.as_bytes(), &options()).unwrap();
        assert!(parsed.rejected.is_empty());
        assert_eq!(parsed.accounts.len(), 2);
        assert_eq!(parsed.accounts[0].name.as_deref(), Some("Compte courant"));
        assert_eq!(parsed.accounts[0].operations.len(), 1);
        assert_eq!(parsed.accounts[1].name.as_deref(), Some("Livret"));
        assert_eq!(parsed.accounts[1].description(), "Livret");
        assert_eq!(parsed.accounts[1].operations[0].payee, "B");
    }

    #[test]
    fn not_qif() {
        assert_eq!(
            parse(b"Date;Montant\n01/01/2026;12\n", &options()),
            Err(ImportError::NotQif)
        );
        assert_eq!(parse(b"PFoo\n^\n", &options()), Err(ImportError::NotQif));
        assert_eq!(parse(b"", &options()), Err(ImportError::NotQif));
    }

    #[test]
    fn cp1252_file() {
        let data = b"!Type:Bank\nD01/03/2026\nT-5\nPCaf\xE9 \x80\n^\n";
        let parsed = parse(data, &options()).unwrap();
        assert_eq!(parsed.accounts[0].operations[0].payee, "Café €");
    }

    #[test]
    fn letters_and_samples() {
        assert_eq!(
            letters(SAMPLE.as_bytes()),
            vec!['C', 'D', 'L', 'M', 'N', 'P', 'T', 'U']
        );
        let samples = sample_dates(SAMPLE.as_bytes(), 2);
        assert_eq!(samples, vec!["05/03/2026", "06/03/2026"]);
        // 05/03 et 06/03 sont ambigus…
        assert_eq!(DateOrder::guess(samples.iter().map(String::as_str)), None);
        // … 31/02 (même incorrecte) indique le jour en premier.
        let samples = sample_dates(SAMPLE.as_bytes(), 10);
        assert_eq!(
            DateOrder::guess(samples.iter().map(String::as_str)),
            Some(DateOrder::DayMonthYear)
        );
    }

    /// Un export QIF de BankPerfect se réimporte à l'identique.
    #[test]
    fn export_round_trip() {
        use crate::export;
        use crate::import::tests::{document, empty_account, existing};
        use crate::import::{ConvertOptions, DuplicateOptions, detect_duplicates};

        let doc = document(&[(0, "Loisirs", false), (1, "Livres", true)]);
        let mut account = empty_account();
        let mut card = existing(ymd(2026, 3, 5), -1250, StandardMode::Carte, "Fnac");
        card.details = "livre".into();
        card.category = Some(1);
        let mut cheque = existing(ymd(2026, 3, 6), -5000, StandardMode::ChequeEmis, "Dupont");
        cheque.details = "n°1234".into();
        cheque.category = Some(0);
        let salary = existing(
            ymd(2026, 3, 7),
            150_000,
            StandardMode::VirementRecu,
            "Employeur",
        );
        account.operations = vec![card, cheque, salary];

        let text = export::qif(&doc, &account, &[0, 1, 2]);
        let parsed = parse(text.as_bytes(), &options()).unwrap();
        assert!(parsed.rejected.is_empty());
        let imported = &parsed.accounts[0].operations;
        let converted: Vec<_> = imported
            .iter()
            .map(|op| to_operation(&doc, op, &ConvertOptions::default()))
            .collect();
        for (a, b) in converted.iter().zip(&account.operations) {
            assert_eq!(a.date, b.date);
            assert_eq!(a.amount, b.amount);
            assert_eq!(a.mode, b.mode);
            assert_eq!(a.payee, b.payee);
            assert_eq!(a.details, b.details);
            assert_eq!(a.category, b.category);
        }
        // Réimporter dans le même compte : tout est doublon.
        let statuses = detect_duplicates(&account, imported, &DuplicateOptions::default());
        assert!(statuses.iter().all(|s| !s.is_new()));
        assert!(matches!(
            statuses[1],
            ImportStatus::ProbableDuplicate { existing: 1, .. }
        ));
    }
}
