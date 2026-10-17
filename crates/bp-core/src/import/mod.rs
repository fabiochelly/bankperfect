//! Imports de relevés bancaires (ex-scripts `import_qif.py`, `import_ofx.py`
//! et `import_csv.py`).
//!
//! Le traitement se fait en trois temps, sans interface graphique :
//!
//! 1. **Analyse** du fichier par [`qif::parse`], [`ofx::parse`] ou
//!    [`csv::parse`] : on obtient un [`ParsedImport`], c'est-à-dire un ou
//!    plusieurs [`ImportedAccount`] (l'OFX peut contenir plusieurs comptes)
//!    et la liste des lignes illisibles ([`RejectedLine`]).
//! 2. **Préparation** : correspondance avec un compte du document
//!    ([`match_account`]), repérage des doublons probables
//!    ([`detect_duplicates`]), création éventuelle des catégories manquantes
//!    ([`create_missing_categories`]). L'interface laisse l'utilisateur
//!    cocher ou décocher chaque ligne.
//! 3. **Conversion** des lignes acceptées en [`Operation`] ([`to_operation`]) ;
//!    l'appelant attribue les identifiants (`uid`) et les ajoute au compte.
//!
//! Les corrections par rapport aux scripts d'origine sont listées dans
//! `docs/corrections.md` (C42 à C56).

pub mod csv;
pub mod ofx;
pub mod qif;

use std::collections::HashSet;
use std::fmt;

use chrono::Datelike;

use crate::filter::normalize;
use crate::model::{
    Account, Amount, Category, Date, Document, Mode, Operation, StandardMode, Tack,
};

/// Clé des champs supplémentaires où l'on peut conserver l'identifiant OFX
/// d'une opération importée (voir [`ConvertOptions::store_fitid`]).
pub const FITID_KEY: &str = "FITID";

// ---------------------------------------------------------------------------
// Types communs
// ---------------------------------------------------------------------------

/// Erreur empêchant toute analyse du fichier. Les lignes isolées illisibles ne
/// sont pas des erreurs : elles sont signalées dans [`ParsedImport::rejected`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("le fichier n'est pas un fichier QIF correct")]
    NotQif,

    #[error("le fichier n'est pas un fichier OFX correct")]
    NotOfx,

    #[error("paramètres d'import incorrects : {0}")]
    InvalidConfig(String),
}

/// Format d'un fichier à importer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    Qif,
    Ofx,
    Csv,
}

impl ImportFormat {
    /// Devine le format d'après le contenu : en-tête OFX (`OFXHEADER`, balise
    /// `<OFX>`), en-tête QIF (`!Type:`) ou lignes QIF (`D…`, `T…`, `^`) ;
    /// sinon CSV.
    pub fn detect(data: &[u8]) -> ImportFormat {
        let text = decode_text(data);
        let upper = text.to_uppercase();
        if upper.contains("OFXHEADER") || upper.contains("<OFX>") {
            return ImportFormat::Ofx;
        }
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        if lines
            .next()
            .is_some_and(|l| l.to_lowercase().starts_with("!type:"))
        {
            return ImportFormat::Qif;
        }
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        let has_caret = lines.contains(&"^");
        let has_date = lines.iter().any(|l| l.starts_with('D'));
        let has_amount = lines
            .iter()
            .any(|l| l.starts_with('T') || l.starts_with('U'));
        if has_caret && has_date && has_amount {
            ImportFormat::Qif
        } else {
            ImportFormat::Csv
        }
    }
}

/// Opération lue dans un fichier, avant sa conversion en [`Operation`].
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedOperation {
    /// Numéro de la ligne du fichier où commence l'opération (à partir de 1),
    /// pour les messages.
    pub line: usize,
    pub date: Date,
    /// Date de valeur, si le fichier la fournit (OFX `DTAVAIL`, colonne CSV).
    /// Indicative : le modèle n'a pas de date de valeur.
    pub value_date: Option<Date>,
    /// Montant signé (négatif = dépense), tel qu'il figure dans le fichier.
    pub amount: Amount,
    /// Mode de paiement deviné (modifiable par l'interface).
    pub mode: Mode,
    /// Numéro de chèque ; il sera placé en tête du détail (`n°1234`).
    pub cheque_number: Option<String>,
    /// Tiers.
    pub payee: String,
    /// Détail.
    pub details: String,
    /// Catégorie, par nom, telle qu'elle figure dans le fichier.
    pub category: Option<String>,
    /// Sous-catégorie, par nom.
    pub subcategory: Option<String>,
    /// Pointage lu dans le fichier ; `None` = pointage par défaut de l'import.
    pub tack: Option<Tack>,
    /// Identifiant de la transaction attribué par la banque (OFX `FITID`).
    pub fitid: Option<String>,
}

/// Compte trouvé dans un fichier, avec ses opérations lisibles.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportedAccount {
    /// Nom du compte (QIF `!Account`).
    pub name: Option<String>,
    /// Code banque (OFX `BANKID`).
    pub bank_id: Option<String>,
    /// Code guichet (OFX `BRANCHID`).
    pub branch_id: Option<String>,
    /// Numéro de compte (OFX `ACCTID`).
    pub account_id: Option<String>,
    /// Identifiant de la réponse OFX (`TRNUID`).
    pub transaction_uid: Option<String>,
    /// Devise (OFX `CURDEF`).
    pub currency: Option<String>,
    /// Relevé de carte de crédit (OFX `CCSTMTRS`).
    pub credit_card: bool,
    /// Solde annoncé par la banque (OFX `LEDGERBAL`, synchronisation).
    pub balance: Option<StatementBalance>,
    pub operations: Vec<ImportedOperation>,
}

/// Solde d'un compte selon la banque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatementBalance {
    pub amount: Amount,
    /// Le solde tient compte des opérations jusqu'à cette date incluse.
    pub date: Option<Date>,
    /// Solde comptable (opérations comptabilisées seulement) ; sinon solde
    /// disponible, qui compte aussi les opérations en attente.
    pub booked: bool,
}

impl ImportedAccount {
    /// Description affichable : `n° banque-guichet-compte`, `ID …` ou le nom
    /// du compte (chaîne vide si le fichier n'identifie pas le compte).
    pub fn description(&self) -> String {
        let parts: Vec<&str> = [&self.bank_id, &self.branch_id, &self.account_id]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        if !parts.is_empty() {
            format!("n° {}", parts.join("-"))
        } else if let Some(name) = &self.name {
            name.clone()
        } else if let Some(uid) = &self.transaction_uid {
            format!("ID {uid}")
        } else {
            String::new()
        }
    }

    /// Plus petite date des opérations.
    pub fn min_date(&self) -> Option<Date> {
        self.operations.iter().map(|op| op.date).min()
    }

    /// Plus grande date des opérations.
    pub fn max_date(&self) -> Option<Date> {
        self.operations.iter().map(|op| op.date).max()
    }
}

/// Raison du rejet d'une ligne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    MissingDate,
    InvalidDate(String),
    MissingAmount,
    InvalidAmount(String),
    /// Montant nul (l'original refusait les montants inférieurs à 0,0001).
    ZeroAmount,
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDate => write!(f, "date absente"),
            Self::InvalidDate(text) => write!(f, "date incorrecte « {text} »"),
            Self::MissingAmount => write!(f, "montant absent"),
            Self::InvalidAmount(text) => write!(f, "montant incorrect « {text} »"),
            Self::ZeroAmount => write!(f, "montant nul"),
        }
    }
}

/// Ligne (ou enregistrement) du fichier qui n'a pas pu être lue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedLine {
    /// Numéro de la ligne du fichier où commence l'enregistrement (à partir de 1).
    pub line: usize,
    /// Contenu de l'enregistrement, pour le rapport.
    pub text: String,
    pub reason: RejectReason,
}

/// Résultat de l'analyse d'un fichier.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedImport {
    /// Comptes ayant au moins une opération lisible, dans l'ordre du fichier.
    pub accounts: Vec<ImportedAccount>,
    /// Lignes illisibles, dans l'ordre du fichier.
    pub rejected: Vec<RejectedLine>,
}

impl ParsedImport {
    /// Nombre total d'opérations lisibles.
    pub fn operation_count(&self) -> usize {
        self.accounts.iter().map(|a| a.operations.len()).sum()
    }
}

/// Libellé d'un nombre d'opérations : « aucune opération », « une
/// opération », « 12 opérations » (correction C43 : le cas « une ligne »
/// n'était jamais affiché par l'import OFX).
pub fn operation_count_label(count: usize) -> String {
    match count {
        0 => "aucune opération".to_string(),
        1 => "une opération".to_string(),
        n => format!("{n} opérations"),
    }
}

// ---------------------------------------------------------------------------
// Encodage et textes
// ---------------------------------------------------------------------------

/// Décode le contenu d'un fichier : UTF-8 (avec ou sans BOM), UTF-16 avec BOM,
/// sinon Windows-1252 (encodage habituel des fichiers bancaires français).
///
/// Un fichier qui déclare un encodage (en-tête OFX `CHARSET`, XML
/// `encoding=`) est traité de la même façon : un texte en Windows-1252 n'est
/// presque jamais de l'UTF-8 valide, et les banques déclarent souvent un
/// encodage faux.
pub fn decode_text(data: &[u8]) -> String {
    if let Some((encoding, bom_len)) = encoding_rs::Encoding::for_bom(data) {
        let (text, _) = encoding.decode_without_bom_handling(&data[bom_len..]);
        return text.into_owned();
    }
    match std::str::from_utf8(data) {
        Ok(text) => text.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(data).0.into_owned(),
    }
}

/// Motif de comparaison (`Pattern` des scripts) : minuscules, sans accents,
/// lettres et chiffres seulement.
pub fn pattern(text: &str) -> String {
    normalize(text)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// Nettoie un texte lu : retours à la ligne et tabulations remplacés par des
/// espaces, espaces multiples réduites, espaces de début et de fin retirées.
pub fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Dates et montants
// ---------------------------------------------------------------------------

/// Ordre des éléments d'une date dans le fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateOrder {
    /// Jour, mois, année (JMA) : `31/12/2026`.
    #[default]
    DayMonthYear,
    /// Mois, jour, année (MJA) : `12/31'26` (Quicken américain).
    MonthDayYear,
    /// Année, mois, jour (AMJ) : `2026-12-31`.
    YearMonthDay,
}

impl DateOrder {
    pub const ALL: [DateOrder; 3] = [Self::DayMonthYear, Self::MonthDayYear, Self::YearMonthDay];

    /// Libellé court de l'original : JMA, MJA ou AMJ.
    pub fn label(self) -> &'static str {
        match self {
            Self::DayMonthYear => "JMA",
            Self::MonthDayYear => "MJA",
            Self::YearMonthDay => "AMJ",
        }
    }

    /// Devine l'ordre d'après des exemples de dates : une première partie sur
    /// 4 chiffres indique AMJ, une partie supérieure à 12 en première ou en
    /// deuxième position indique JMA ou MJA. `None` si rien ne permet de
    /// trancher.
    pub fn guess<'a>(samples: impl IntoIterator<Item = &'a str>) -> Option<DateOrder> {
        let mut day_first = false;
        let mut month_first = false;
        for sample in samples {
            let tokens = date_tokens(sample);
            if tokens.len() < 2 {
                continue;
            }
            if tokens[0].len() == 4 && tokens[0].chars().all(|c| c.is_ascii_digit()) {
                return Some(Self::YearMonthDay);
            }
            let first: Option<u32> = tokens[0].parse().ok();
            let second: Option<u32> = tokens[1].parse().ok();
            match (first, second) {
                (Some(a), _) if a > 12 => day_first = true,
                (_, Some(b)) if b > 12 => month_first = true,
                (Some(_), None) => day_first = true,
                (None, Some(_)) => month_first = true,
                _ => {}
            }
        }
        match (day_first, month_first) {
            (true, false) => Some(Self::DayMonthYear),
            (false, true) => Some(Self::MonthDayYear),
            _ => None,
        }
    }
}

/// Morceaux d'une date : on ignore une éventuelle heure (`12:30:00`, `T…`)
/// et on découpe sur tout ce qui n'est ni lettre ni chiffre.
fn date_tokens(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    for word in text.split_whitespace() {
        // Format ISO « 2026-03-05T12:00:00 » : on coupe au T.
        let word = match word.find('T') {
            Some(pos) if pos > 0 && word[..pos].ends_with(|c: char| c.is_ascii_digit()) => {
                &word[..pos]
            }
            _ => word,
        };
        if word.contains(':') {
            continue;
        }
        tokens.extend(
            word.split(|c: char| !c.is_alphanumeric())
                .filter(|s| !s.is_empty()),
        );
    }
    tokens
}

/// Noms de mois reconnus (français et anglais, sans accents).
const MONTH_NAMES: [&[&str]; 12] = [
    &["janvier", "january"],
    &["fevrier", "february"],
    &["mars", "march"],
    &["avril", "april"],
    &["mai", "may"],
    &["juin", "june"],
    &["juillet", "july"],
    &["aout", "august"],
    &["septembre", "september"],
    &["octobre", "october"],
    &["novembre", "november"],
    &["decembre", "december"],
];

fn parse_month(token: &str) -> Option<u32> {
    if token.chars().all(|c| c.is_ascii_digit()) {
        return token.parse().ok();
    }
    let p = pattern(token);
    if p.len() < 3 {
        return None;
    }
    MONTH_NAMES
        .iter()
        .position(|names| names.iter().any(|name| name.starts_with(&p)))
        .map(|i| i as u32 + 1)
}

fn parse_number(token: &str) -> Option<u32> {
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    token.parse().ok()
}

/// Année sur 2 chiffres : fenêtre glissante, comme [`Date::parse_fr`] (une
/// année plus de 20 ans dans le futur est ramenée au siècle précédent).
fn parse_year(token: &str, today: Date) -> Option<i32> {
    let value = parse_number(token)? as i32;
    if token.len() > 2 {
        return Some(value);
    }
    let now = today.to_naive().year();
    let year = now / 100 * 100 + value;
    Some(if year > now + 20 { year - 100 } else { year })
}

/// Analyse une date selon `order`.
///
/// Accepte les séparateurs `/ - . ' , ;` et l'espace, les noms de mois en
/// français et en anglais (complets ou abrégés), les années sur 2 chiffres,
/// les dates compactes sur 8 chiffres (`20261231`, `31122026`), et les dates
/// sans année (année de `today`) sauf en AMJ. Une heure éventuelle est
/// ignorée. Renvoie `None` si la date est incorrecte.
pub fn parse_date(text: &str, order: DateOrder, today: Date) -> Option<Date> {
    let mut tokens = date_tokens(text);
    if tokens.len() == 1 && tokens[0].len() == 8 && parse_number(tokens[0]).is_some() {
        let t = tokens[0];
        tokens = match order {
            DateOrder::YearMonthDay => vec![&t[..4], &t[4..6], &t[6..]],
            _ => vec![&t[..2], &t[2..4], &t[4..]],
        };
    }
    let (year, month, day) = match (order, tokens.as_slice()) {
        (DateOrder::DayMonthYear, [d, m]) => {
            (today.to_naive().year(), parse_month(m)?, parse_number(d)?)
        }
        (DateOrder::MonthDayYear, [m, d]) => {
            (today.to_naive().year(), parse_month(m)?, parse_number(d)?)
        }
        (DateOrder::DayMonthYear, [d, m, y]) => {
            (parse_year(y, today)?, parse_month(m)?, parse_number(d)?)
        }
        (DateOrder::MonthDayYear, [m, d, y]) => {
            (parse_year(y, today)?, parse_month(m)?, parse_number(d)?)
        }
        (DateOrder::YearMonthDay, [y, m, d]) => {
            (parse_year(y, today)?, parse_month(m)?, parse_number(d)?)
        }
        _ => return None,
    };
    Date::from_ymd(year, month, day)
}

/// Analyse un montant.
///
/// `decimal_separator` : `Some(',')` ou `Some('.')` impose le séparateur
/// décimal (l'autre est alors un séparateur de milliers) ; `None` applique la
/// règle de [`Amount::parse`] (le dernier séparateur est décimal).
///
/// Accepte les espaces (y compris insécables) et l'apostrophe comme
/// séparateurs de milliers, un symbole ou code de devise en tête ou en fin
/// (`€`, `$`, `£`, `EUR`, `F`…), un signe `+` ou `-` en tête ou un `-` final,
/// et les parenthèses comptables `(12,50)`. Tout autre caractère rend le
/// montant incorrect (`None`).
pub fn parse_amount(text: &str, decimal_separator: Option<char>) -> Option<Amount> {
    let trimmed = text.trim_matches(|c: char| {
        c.is_whitespace() || c.is_alphabetic() || matches!(c, '€' | '$' | '£')
    });
    let (inner, parenthesized) = match trimmed.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        Some(inner) => (inner.trim(), true),
        None => (trimmed, false),
    };
    let inner = inner.trim_matches(|c: char| {
        c.is_whitespace() || c.is_alphabetic() || matches!(c, '€' | '$' | '£')
    });
    if !inner.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let valid = inner.chars().all(|c| {
        c.is_ascii_digit() || c.is_whitespace() || matches!(c, '.' | ',' | '-' | '+' | '\'')
    });
    if !valid || inner.matches(['-', '+']).count() > 1 {
        return None;
    }
    let trailing_minus = inner.ends_with('-');
    let leading_sign_ok =
        inner.starts_with(['-', '+']) || trailing_minus || !inner.contains(['-', '+']);
    if !leading_sign_ok {
        return None;
    }
    let body: String = inner
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '\'' | '+'))
        .filter(|&c| match decimal_separator {
            Some(',') => c != '.',
            Some('.') => c != ',',
            _ => true,
        })
        .collect();
    let body = match decimal_separator {
        Some(',') => body.replace(',', "."),
        _ => body,
    };
    let body = body.trim_end_matches('-');
    let amount = Amount::parse(body)?;
    Some(if (parenthesized || trailing_minus) && amount.0 > 0 {
        -amount
    } else {
        amount
    })
}

// ---------------------------------------------------------------------------
// Modes de paiement
// ---------------------------------------------------------------------------

/// Déduit le mode à partir d'un champ « mode » (QIF `N`, colonne Mode d'un
/// CSV) : portage de `getmode`, recopié dans les trois scripts.
///
/// - un nombre seul sur une dépense est un numéro de chèque ;
/// - un libellé de mode standard exact (« Virement reçu ») de signe cohérent
///   est repris tel quel ;
/// - sinon, le début du texte normalisé est comparé à des préfixes (`cb`,
///   `car`, `fac` → Carte ; `ret`, `dab` → Retrait DAB ; `ch` → chèque ;
///   `pr` → Prélèvement ; `vir` → virement…). Pour un chèque émis, le numéro
///   est cherché à la fin du texte (`CHQ 1234`, `CHEQUE1234`).
///
/// Renvoie `None` si aucune règle ne s'applique.
pub fn guess_mode_from_field(text: &str, negative: bool) -> Option<(StandardMode, Option<String>)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if negative && text.chars().all(|c| c.is_ascii_digit()) {
        return Some((StandardMode::ChequeEmis, Some(text.to_string())));
    }
    if let Some(mode) = StandardMode::from_label(text)
        && mode.is_debit() == negative
        && mode != StandardMode::ChequeEmis
    {
        return Some((mode, None));
    }
    let v = pattern(text);
    let table: &[(StandardMode, &[&str])] = if negative {
        &[
            (
                StandardMode::Carte,
                &["fac", "cb", "car", "achatcarte", "achatcb"],
            ),
            (
                StandardMode::RetraitDab,
                &["ret", "dab", "distrib", "automat", "atm"],
            ),
            (StandardMode::ChequeEmis, &["ch"]),
            (StandardMode::Prelevement, &["pr"]),
            (
                StandardMode::VirementEmis,
                &["vir", "vrm", "vrt", "xfer", "transf"],
            ),
            (StandardMode::Tip, &["tip"]),
        ]
    } else {
        &[
            (StandardMode::Versement, &["vers", "esp"]),
            (StandardMode::DepotCheque, &["rem", "dep", "ch"]),
            (
                StandardMode::VirementRecu,
                &["vir", "vrm", "vrt", "xfer", "transf"],
            ),
        ]
    };
    let mode = table
        .iter()
        .find(|(_, prefixes)| prefixes.iter().any(|p| v.starts_with(p)))
        .map(|(mode, _)| *mode)?;
    if mode == StandardMode::ChequeEmis {
        let last = text
            .rsplit([' ', '.', '°', 'Q', 'E', 'H'])
            .next()
            .unwrap_or("");
        if !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()) {
            return Some((mode, Some(last.to_string())));
        }
    }
    Some((mode, None))
}

/// Déduit le mode à partir des mots d'un libellé (tiers, détail) : portage
/// des heuristiques de `calculate_paymode` (import OFX), étendues aux
/// libellés accentués et à quelques abréviations courantes.
pub fn guess_mode_from_label(label: &str, negative: bool) -> Option<StandardMode> {
    // « C.B. » → « CB » : les points sont retirés avant la normalisation.
    let normalized = normalize(&label.replace('.', ""));
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let has = |list: &[&str]| words.iter().any(|w| list.contains(w));
    let cheque = ["cheque", "cheques", "chq", "cheq", "check"];
    let transfer = ["vir", "virt", "virement", "vrt"];
    if negative {
        if has(&cheque) {
            Some(StandardMode::ChequeEmis)
        } else if has(&["retrait", "ret", "retr", "dab", "atm"]) {
            Some(StandardMode::RetraitDab)
        } else if has(&["carte", "achat", "cb", "mastercard", "visa"]) {
            Some(StandardMode::Carte)
        } else if has(&["prelevement", "prel", "prlv", "prelev"]) {
            Some(StandardMode::Prelevement)
        } else if has(&["tip"]) {
            Some(StandardMode::Tip)
        } else if has(&transfer) {
            Some(StandardMode::VirementEmis)
        } else {
            None
        }
    } else if has(&transfer) {
        Some(StandardMode::VirementRecu)
    } else if has(&cheque) || has(&["remchq"]) {
        Some(StandardMode::DepotCheque)
    } else if has(&["versement", "especes", "vrst"]) {
        Some(StandardMode::Versement)
    } else {
        None
    }
}

/// Mode par défaut de l'original : Prélèvement pour une dépense, Virement
/// reçu pour une recette.
pub fn default_mode(negative: bool) -> StandardMode {
    if negative {
        StandardMode::Prelevement
    } else {
        StandardMode::VirementRecu
    }
}

/// Premier nombre d'au moins 4 chiffres d'un texte (`find_number_in_field`
/// de l'import CSV), pour retrouver un numéro de chèque.
pub fn find_cheque_number(text: &str) -> Option<String> {
    text.replace('°', " ")
        .replace('-', "")
        .split_whitespace()
        .find(|w| w.len() > 3 && w.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
}

/// Mode et numéro de chèque d'une opération QIF ou CSV : champ « mode »
/// d'abord ([`guess_mode_from_field`]), puis mots du tiers et du détail
/// ([`guess_mode_from_label`]), sinon [`default_mode`]. Pour un chèque émis
/// sans numéro, celui-ci est cherché dans le détail puis dans le tiers.
pub fn guess_mode(
    mode_text: &str,
    payee: &str,
    details: &str,
    negative: bool,
) -> (StandardMode, Option<String>) {
    let (mode, mut number) = guess_mode_from_field(mode_text, negative)
        .or_else(|| guess_mode_from_label(payee, negative).map(|m| (m, None)))
        .or_else(|| guess_mode_from_label(details, negative).map(|m| (m, None)))
        .unwrap_or((default_mode(negative), None));
    if mode == StandardMode::ChequeEmis && number.is_none() {
        number = find_cheque_number(details).or_else(|| find_cheque_number(payee));
    }
    (mode, number)
}

// ---------------------------------------------------------------------------
// Correspondance des comptes
// ---------------------------------------------------------------------------

/// Identifiant bancaire comparable : lettres et chiffres en majuscules.
fn normalize_id(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn same_code(a: &str, b: &str) -> bool {
    a.trim_start_matches('0') == b.trim_start_matches('0')
}

/// Retrouve le compte du document correspondant à un compte importé (indice
/// dans `document.accounts`).
///
/// L'original comparait `n° banque-guichet-compte` à l'identique. Ici :
/// - le numéro de compte OFX est comparé (sans les zéros de tête) au numéro du
///   compte, éventuellement suivi de la clé RIB (2 caractères), ou cherché
///   dans l'IBAN ;
/// - un code banque ou guichet renseigné des deux côtés et différent exclut
///   le compte ;
/// - à défaut d'identifiant, le nom (QIF `!Account`) est comparé sans tenir
///   compte de la casse.
pub fn match_account(document: &Document, imported: &ImportedAccount) -> Option<usize> {
    let wanted = imported
        .account_id
        .as_deref()
        .map(normalize_id)
        .filter(|s| !s.is_empty());
    let Some(wanted) = wanted else {
        let name = imported.name.as_deref()?.trim().to_lowercase();
        return document
            .accounts
            .iter()
            .position(|a| !name.is_empty() && a.name.trim().to_lowercase() == name);
    };
    let wanted_trimmed = wanted.trim_start_matches('0');
    let mut best: Option<(usize, u8)> = None;
    for (index, account) in document.accounts.iter().enumerate() {
        let mut score = 0;
        let mut conflict = false;
        for (theirs, ours) in [
            (&imported.bank_id, &account.bank_code),
            (&imported.branch_id, &account.agency_code),
        ] {
            let theirs = theirs.as_deref().map(normalize_id).unwrap_or_default();
            let ours = normalize_id(ours);
            if !theirs.is_empty() && !ours.is_empty() {
                if same_code(&theirs, &ours) {
                    score += 1;
                } else {
                    conflict = true;
                }
            }
        }
        if conflict {
            continue;
        }
        let number = normalize_id(&account.number);
        let number_trimmed = number.trim_start_matches('0');
        let iban = normalize_id(&account.iban);
        // Numéro identique, numéro suivi de la clé RIB, ou numéro contenu
        // dans l'IBAN.
        let with_key = !number_trimmed.is_empty()
            && wanted_trimmed.starts_with(number_trimmed)
            && wanted_trimmed.len() - number_trimmed.len() <= 2;
        let in_iban = wanted.len() >= 5 && iban.contains(&wanted);
        let matched = if !number_trimmed.is_empty() && number_trimmed == wanted_trimmed {
            Some(4)
        } else if with_key || in_iban {
            Some(3)
        } else {
            None
        };
        if let Some(m) = matched {
            let total = m + score;
            if best.is_none_or(|(_, s)| total > s) {
                best = Some((index, total));
            }
        }
    }
    best.map(|(index, _)| index)
}

// ---------------------------------------------------------------------------
// Doublons
// ---------------------------------------------------------------------------

/// Règles de détection des doublons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateOptions {
    /// Écart de dates toléré, en jours (0 = même date, comme l'original).
    pub date_tolerance_days: u32,
    /// Exiger le même mode de paiement (comme l'original ; un « Chq 1234 »
    /// vaut « Chèque émis »).
    pub match_mode: bool,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self {
            date_tolerance_days: 0,
            match_mode: true,
        }
    }
}

/// Statut d'une ligne importée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStatus {
    /// Aucune opération existante ne lui ressemble.
    New,
    /// Ressemble à l'opération `existing` (indice dans `account.operations`).
    ProbableDuplicate {
        existing: usize,
        /// Même identifiant bancaire (`FITID`) : doublon certain.
        same_fitid: bool,
    },
}

impl ImportStatus {
    pub fn is_new(self) -> bool {
        self == ImportStatus::New
    }
}

fn similar_labels(a: &str, b: &str) -> bool {
    let (a, b) = (pattern(a), pattern(b));
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

/// Repère, pour chaque ligne importée, une opération existante du compte qui
/// lui ressemble.
///
/// Règle de l'original : même date, même mode (chèques numérotés confondus)
/// et même montant ; ici la date peut avoir une tolérance et le mode peut
/// être ignoré ([`DuplicateOptions`]). Si plusieurs opérations conviennent,
/// on retient la plus proche en date, puis de même mode, puis de tiers
/// semblable. Deux lignes portant le même `FITID` (conservé dans les champs
/// supplémentaires, voir [`FITID_KEY`]) sont toujours des doublons ; deux
/// `FITID` différents ne le sont jamais.
///
/// Chaque opération existante ne peut justifier qu'un seul doublon
/// (correction C46 : deux achats identiques le même jour dans le fichier
/// étaient tous deux refusés alors qu'un seul existait déjà).
pub fn detect_duplicates(
    account: &Account,
    imported: &[ImportedOperation],
    options: &DuplicateOptions,
) -> Vec<ImportStatus> {
    let ops = &account.operations;
    let mut used = vec![false; ops.len()];
    let mut statuses = vec![ImportStatus::New; imported.len()];

    // 1. Identifiants bancaires identiques.
    for (status, imp) in statuses.iter_mut().zip(imported) {
        let Some(fitid) = imp.fitid.as_deref() else {
            continue;
        };
        if let Some(i) =
            (0..ops.len()).find(|&i| !used[i] && ops[i].extra(FITID_KEY) == Some(fitid))
        {
            used[i] = true;
            *status = ImportStatus::ProbableDuplicate {
                existing: i,
                same_fitid: true,
            };
        }
    }

    // 2. Même montant, date proche, même mode.
    let tolerance = i64::from(options.date_tolerance_days);
    for (status, imp) in statuses.iter_mut().zip(imported) {
        if !status.is_new() {
            continue;
        }
        let best = (0..ops.len())
            .filter(|&i| !used[i])
            .filter(|&i| ops[i].amount == imp.amount)
            .filter(|&i| (i64::from(ops[i].date.0) - i64::from(imp.date.0)).abs() <= tolerance)
            .filter(|&i| !options.match_mode || ops[i].mode == imp.mode)
            .filter(|&i| match (ops[i].extra(FITID_KEY), imp.fitid.as_deref()) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            })
            .min_by_key(|&i| {
                (
                    (i64::from(ops[i].date.0) - i64::from(imp.date.0)).abs(),
                    ops[i].mode != imp.mode,
                    !similar_labels(&ops[i].payee, &imp.payee),
                )
            });
        if let Some(i) = best {
            used[i] = true;
            *status = ImportStatus::ProbableDuplicate {
                existing: i,
                same_fitid: false,
            };
        }
    }
    statuses
}

/// Solde pointé du compte une fois les lignes cochées importées, à comparer
/// au solde de la banque : opérations pointées ou rapprochées, lignes cochées
/// (elles viennent du relevé, qu'on les pointe ou non à l'import) et
/// opérations existantes reconnues dans une ligne décochée (elles figurent au
/// relevé). Avec `until`, seules les opérations jusqu'à cette date comptent.
pub fn pointed_after_import(
    account: &Account,
    imported: &[ImportedOperation],
    statuses: &[ImportStatus],
    include: &[bool],
    until: Option<Date>,
) -> Amount {
    let counts = |d: Date| until.is_none_or(|u| d <= u);
    let ops = &account.operations;
    let mut total: Amount = ops
        .iter()
        .filter(|op| op.tack != Tack::None && counts(op.date))
        .map(|op| op.amount)
        .sum();
    for (k, imp) in imported.iter().enumerate() {
        if !counts(imp.date) {
            continue;
        }
        if include.get(k).copied().unwrap_or(false) {
            total = total + imp.amount;
        } else if let Some(ImportStatus::ProbableDuplicate { existing, .. }) = statuses.get(k)
            && let Some(op) = ops.get(*existing)
            && op.tack == Tack::None
        {
            total = total + op.amount;
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Catégories
// ---------------------------------------------------------------------------

/// Indice de la catégorie principale dont dépend la catégorie `pos`.
fn parent_position(categories: &[Category], pos: usize) -> Option<usize> {
    if !categories[pos].is_child {
        return Some(pos);
    }
    categories[..pos].iter().rposition(|c| !c.is_child)
}

/// Catégories (positions) dont le motif est égal à `wanted` ou le contient,
/// ou est contenu dedans ; les égalités d'abord. Les catégories au motif vide
/// sont ignorées (correction C52 : elles correspondaient à tout).
fn matching_categories(
    patterns: &[String],
    wanted: &str,
    keep: impl Fn(usize) -> bool,
) -> Vec<usize> {
    let mut exact = Vec::new();
    let mut partial = Vec::new();
    for (pos, p) in patterns.iter().enumerate() {
        if p.is_empty() || !keep(pos) {
            continue;
        }
        if p == wanted {
            exact.push(pos);
        } else if p.contains(wanted) || wanted.contains(p.as_str()) {
            partial.push(pos);
        }
    }
    exact.extend(partial);
    exact
}

/// Retrouve par son nom l'identifiant d'une catégorie (portage corrigé de
/// `GetCtg`).
///
/// Les noms sont comparés par motif (minuscules, sans accents ni
/// ponctuation), d'abord à l'identique puis par inclusion.
/// - Une seule des deux valeurs renseignée : recherche parmi toutes les
///   catégories.
/// - Catégorie et sous-catégorie : une sous-catégorie qui convient et dont le
///   parent convient est préférée ; sinon la première sous-catégorie qui
///   convient, sinon la première catégorie principale qui convient.
///
/// Renvoie toujours un **identifiant** (correction C42 : une branche de
/// l'original renvoyait la position dans la liste).
pub fn resolve_category(document: &Document, category: &str, subcategory: &str) -> Option<u32> {
    let categories = &document.categories;
    let patterns: Vec<String> = categories.iter().map(|c| pattern(&c.name)).collect();
    let (cat, sub) = (pattern(category), pattern(subcategory));
    let id = |pos: usize| categories[pos].id;
    match (cat.is_empty(), sub.is_empty()) {
        (true, true) => None,
        (false, true) | (true, false) => {
            let wanted = if cat.is_empty() { &sub } else { &cat };
            matching_categories(&patterns, wanted, |_| true)
                .first()
                .map(|&pos| id(pos))
        }
        (false, false) => {
            let parents = matching_categories(&patterns, &cat, |pos| !categories[pos].is_child);
            let children = matching_categories(&patterns, &sub, |pos| categories[pos].is_child);
            if let Some(&pos) = children.iter().find(|&&pos| {
                parent_position(categories, pos).is_some_and(|parent| parents.contains(&parent))
            }) {
                return Some(id(pos));
            }
            children.first().or(parents.first()).map(|&pos| id(pos))
        }
    }
}

/// Nom de catégorie acceptable : sans `;` ni `|` (séparateurs du format de
/// fichier), espaces superflues retirées.
fn category_name(text: &str) -> String {
    clean_text(&text.replace([';', '|'], " "))
}

/// Comme [`resolve_category`], mais crée ce qui manque (option « Créer les
/// catégories manquantes ») : une catégorie principale, ou une
/// sous-catégorie rangée sous son parent (créé lui aussi au besoin). Une
/// sous-catégorie inconnue est créée même si son parent existe. L'original
/// créait une catégorie principale nommée « catégorie sous-catégorie ».
pub fn ensure_category(document: &mut Document, category: &str, subcategory: &str) -> Option<u32> {
    let (cat, sub) = (category_name(category), category_name(subcategory));
    let (cat_pattern, sub_pattern) = (pattern(&cat), pattern(&sub));
    let found = resolve_category(document, &cat, &sub);
    let mut next_id = document
        .categories
        .iter()
        .map(|c| c.id + 1)
        .max()
        .unwrap_or(0);
    if cat_pattern.is_empty() || sub_pattern.is_empty() {
        if found.is_some() || (cat_pattern.is_empty() && sub_pattern.is_empty()) {
            return found;
        }
        let name = if cat_pattern.is_empty() { sub } else { cat };
        document.categories.push(Category {
            id: next_id,
            name,
            is_child: false,
        });
        return Some(next_id);
    }
    // Catégorie et sous-catégorie : il faut une sous-catégorie.
    let found_pos = found.and_then(|id| document.categories.iter().position(|c| c.id == id));
    if let Some(pos) = found_pos
        && document.categories[pos].is_child
    {
        return found;
    }
    let parent = match found_pos.or_else(|| {
        document
            .categories
            .iter()
            .position(|c| !c.is_child && pattern(&c.name) == cat_pattern)
    }) {
        Some(pos) => pos,
        None => {
            document.categories.push(Category {
                id: next_id,
                name: cat,
                is_child: false,
            });
            next_id += 1;
            document.categories.len() - 1
        }
    };
    let insert_at = parent
        + 1
        + document.categories[parent + 1..]
            .iter()
            .take_while(|c| c.is_child)
            .count();
    document.categories.insert(
        insert_at,
        Category {
            id: next_id,
            name: sub,
            is_child: true,
        },
    );
    Some(next_id)
}

/// Crée les catégories manquantes des opérations importées
/// ([`ensure_category`]) ; renvoie les identifiants créés.
pub fn create_missing_categories<'a>(
    document: &mut Document,
    operations: impl IntoIterator<Item = &'a ImportedOperation>,
) -> Vec<u32> {
    let before: HashSet<u32> = document.categories.iter().map(|c| c.id).collect();
    let mut created = Vec::new();
    for op in operations {
        let cat = op.category.as_deref().unwrap_or("");
        let sub = op.subcategory.as_deref().unwrap_or("");
        ensure_category(document, cat, sub);
        for c in &document.categories {
            if !before.contains(&c.id) && !created.contains(&c.id) {
                created.push(c.id);
            }
        }
    }
    created
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// Options de conversion des lignes acceptées.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConvertOptions {
    /// Pointage des opérations dont le fichier ne précise pas le pointage.
    pub default_tack: Tack,
    /// Conserver l'identifiant OFX dans les champs supplémentaires
    /// ([`FITID_KEY`]) pour reconnaître à coup sûr les doublons lors des
    /// imports suivants.
    pub store_fitid: bool,
}

/// Texte d'opération acceptable : `;` remplacé par une espace (séparateur du
/// format de fichier, comme `LineAdd`), espaces superflues retirées.
fn operation_text(text: &str) -> String {
    clean_text(&text.replace(';', " "))
}

/// Le détail commence-t-il déjà par `n°<numéro>` ?
fn starts_with_cheque_tag(details: &str, tag: &str) -> bool {
    details
        .strip_prefix(tag)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Convertit une ligne acceptée en [`Operation`].
///
/// - `uid` vaut 0 : l'appelant attribue l'identifiant
///   ([`Document::next_operation_uid`]) ;
/// - le montant garde le signe du fichier (le mode deviné en découle déjà) ;
/// - la date de pointage est la date de l'opération, comme `LineAdd` ;
/// - un numéro de chèque est placé en tête du détail (`n°1234 …`) ;
/// - la catégorie est retrouvée par son nom ([`resolve_category`]) ; si elle
///   n'existe pas, l'opération n'a pas de catégorie (appeler
///   [`create_missing_categories`] avant pour la créer).
pub fn to_operation(
    document: &Document,
    imported: &ImportedOperation,
    options: &ConvertOptions,
) -> Operation {
    let category = resolve_category(
        document,
        imported.category.as_deref().unwrap_or(""),
        imported.subcategory.as_deref().unwrap_or(""),
    );
    let mut details = operation_text(&imported.details);
    if let Some(number) = imported.cheque_number.as_deref().filter(|n| !n.is_empty()) {
        let tag = format!("n°{number}");
        if !starts_with_cheque_tag(&details, &tag) {
            details = if details.is_empty() {
                tag
            } else {
                format!("{tag} {details}")
            };
        }
    }
    let mut extra = Vec::new();
    if options.store_fitid
        && let Some(fitid) = imported.fitid.as_deref().filter(|f| !f.is_empty())
    {
        extra.push((FITID_KEY.to_string(), fitid.to_string()));
    }
    Operation {
        uid: 0,
        date: imported.date,
        mark_date: imported.date,
        tack: imported.tack.unwrap_or(options.default_tack),
        mode: imported.mode.clone(),
        payee: operation_text(&imported.payee),
        details,
        category,
        amount: imported.amount,
        extra,
    }
}

// ---------------------------------------------------------------------------
// Aides internes aux analyseurs
// ---------------------------------------------------------------------------

/// Construit une opération à partir des champs bruts communs à QIF et CSV ;
/// renvoie la raison du rejet si la date ou le montant manque.
pub(crate) struct RawFields<'a> {
    pub line: usize,
    pub date: Option<&'a str>,
    pub amount: Result<Option<Amount>, String>,
    pub mode: &'a str,
    pub payee: &'a str,
    pub details: &'a str,
    pub category: &'a str,
    pub subcategory: &'a str,
    pub tack: Option<Tack>,
    pub value_date: Option<Date>,
}

pub(crate) fn build_operation(
    raw: RawFields<'_>,
    order: DateOrder,
    today: Date,
) -> Result<ImportedOperation, RejectReason> {
    let date_text = raw.date.map(str::trim).filter(|d| !d.is_empty());
    let Some(date_text) = date_text else {
        return Err(RejectReason::MissingDate);
    };
    let date = parse_date(date_text, order, today)
        .ok_or_else(|| RejectReason::InvalidDate(date_text.to_string()))?;
    let amount = match raw.amount {
        Ok(Some(amount)) => amount,
        Ok(None) => return Err(RejectReason::MissingAmount),
        Err(text) => return Err(RejectReason::InvalidAmount(text)),
    };
    if amount == Amount::ZERO {
        return Err(RejectReason::ZeroAmount);
    }
    let payee = clean_text(raw.payee);
    let details = clean_text(raw.details);
    let (mode, cheque_number) = guess_mode(raw.mode, &payee, &details, amount.is_negative());
    let optional = |s: &str| Some(clean_text(s)).filter(|s| !s.is_empty());
    Ok(ImportedOperation {
        line: raw.line,
        date,
        value_date: raw.value_date,
        amount,
        mode: Mode::Standard(mode),
        cheque_number,
        payee,
        details,
        category: optional(raw.category),
        subcategory: optional(raw.subcategory),
        tack: raw.tack,
        fitid: None,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn today() -> Date {
        Date::from_ymd(2026, 10, 11).unwrap()
    }

    pub fn ymd(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    pub fn empty_account() -> Account {
        Account {
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
            operations: Vec::new(),
        }
    }

    pub fn document(categories: &[(u32, &str, bool)]) -> Document {
        Document {
            group_name: String::new(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: Vec::new(),
            accounts: Vec::new(),
            account_order: Vec::new(),
            default_account: 0,
            categories: categories
                .iter()
                .map(|&(id, name, is_child)| Category {
                    id,
                    name: name.into(),
                    is_child,
                })
                .collect(),
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        }
    }

    pub fn existing(date: Date, cents: i64, mode: StandardMode, payee: &str) -> Operation {
        Operation {
            uid: 1,
            date,
            mark_date: date,
            tack: Tack::None,
            mode: Mode::Standard(mode),
            payee: payee.into(),
            details: String::new(),
            category: None,
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        }
    }

    pub fn imported(date: Date, cents: i64, mode: StandardMode, payee: &str) -> ImportedOperation {
        ImportedOperation {
            line: 1,
            date,
            value_date: None,
            amount: Amount::from_cents(cents),
            mode: Mode::Standard(mode),
            cheque_number: None,
            payee: payee.into(),
            details: String::new(),
            category: None,
            subcategory: None,
            tack: None,
            fitid: None,
        }
    }

    #[test]
    fn decoding() {
        assert_eq!(decode_text("Dépôt".as_bytes()), "Dépôt");
        assert_eq!(decode_text(b"\xEF\xBB\xBFD\xC3\xA9p"), "Dép");
        assert_eq!(decode_text(b"D\xE9p\xF4t \x80"), "Dépôt €");
        assert_eq!(decode_text(b"\xFF\xFEA\x00\xE9\x00"), "Aé");
    }

    #[test]
    fn patterns() {
        assert_eq!(pattern("Électricité / Gaz"), "electricitegaz");
        assert_eq!(pattern("  -- "), "");
        assert_eq!(
            clean_text("  CB\tCARREFOUR \n 12/03  "),
            "CB CARREFOUR 12/03"
        );
    }

    #[test]
    fn dates() {
        let t = today();
        let dmy = DateOrder::DayMonthYear;
        assert_eq!(parse_date("31/12/2025", dmy, t), Some(ymd(2025, 12, 31)));
        assert_eq!(parse_date("31.12.25", dmy, t), Some(ymd(2025, 12, 31)));
        assert_eq!(parse_date(" 1/ 5/98", dmy, t), Some(ymd(1998, 5, 1)));
        assert_eq!(
            parse_date("1/5'02", DateOrder::MonthDayYear, t),
            Some(ymd(2002, 1, 5))
        );
        assert_eq!(
            parse_date("12/25'98", DateOrder::MonthDayYear, t),
            Some(ymd(1998, 12, 25))
        );
        assert_eq!(
            parse_date("2026-03-05", DateOrder::YearMonthDay, t),
            Some(ymd(2026, 3, 5))
        );
        assert_eq!(
            parse_date("2026-03-05T12:30:00", DateOrder::YearMonthDay, t),
            Some(ymd(2026, 3, 5))
        );
        assert_eq!(
            parse_date("05/03/2026 12:30", dmy, t),
            Some(ymd(2026, 3, 5))
        );
        assert_eq!(parse_date("5 mars 2026", dmy, t), Some(ymd(2026, 3, 5)));
        assert_eq!(parse_date("05-févr.-26", dmy, t), Some(ymd(2026, 2, 5)));
        assert_eq!(
            parse_date("Mar 5, 2026", DateOrder::MonthDayYear, t),
            Some(ymd(2026, 3, 5))
        );
        assert_eq!(parse_date("5 August 2026", dmy, t), Some(ymd(2026, 8, 5)));
        assert_eq!(parse_date("05/03", dmy, t), Some(ymd(2026, 3, 5)));
        assert_eq!(parse_date("05/03", DateOrder::YearMonthDay, t), None);
        assert_eq!(
            parse_date("20260305", DateOrder::YearMonthDay, t),
            Some(ymd(2026, 3, 5))
        );
        assert_eq!(parse_date("05032026", dmy, t), Some(ymd(2026, 3, 5)));
        assert_eq!(parse_date("31/02/2026", dmy, t), None);
        assert_eq!(parse_date("ju/03/2026", dmy, t), None);
        assert_eq!(parse_date("", dmy, t), None);
        assert_eq!(parse_date("Date", dmy, t), None);
    }

    #[test]
    fn date_order_guess() {
        assert_eq!(
            DateOrder::guess(["01/02/2026", "25/02/2026"]),
            Some(DateOrder::DayMonthYear)
        );
        assert_eq!(DateOrder::guess(["2/25'26"]), Some(DateOrder::MonthDayYear));
        assert_eq!(
            DateOrder::guess(["2026-02-25"]),
            Some(DateOrder::YearMonthDay)
        );
        assert_eq!(DateOrder::guess(["01/02/2026"]), None);
    }

    #[test]
    fn amounts() {
        let p = |s| parse_amount(s, None);
        assert_eq!(p("-12,50"), Some(Amount::from_cents(-1250)));
        assert_eq!(p("1 234,56 €"), Some(Amount::from_cents(123_456)));
        assert_eq!(p("1\u{a0}234,56"), Some(Amount::from_cents(123_456)));
        assert_eq!(p("EUR 12.5"), Some(Amount::from_cents(1250)));
        assert_eq!(p("+12.50"), Some(Amount::from_cents(1250)));
        assert_eq!(p("12,50-"), Some(Amount::from_cents(-1250)));
        assert_eq!(p("(12,50)"), Some(Amount::from_cents(-1250)));
        assert_eq!(p("1'234.50"), Some(Amount::from_cents(123_450)));
        assert_eq!(p("05/03/2026"), None);
        assert_eq!(p("12-50"), None);
        assert_eq!(p("abc"), None);
        assert_eq!(p(""), None);
        assert_eq!(p("--12"), None);
        assert_eq!(
            parse_amount("1.234", Some(',')),
            Some(Amount::from_cents(123_400))
        );
        assert_eq!(
            parse_amount("1.234,5", Some(',')),
            Some(Amount::from_cents(123_450))
        );
        assert_eq!(
            parse_amount("1,234", Some('.')),
            Some(Amount::from_cents(123_400))
        );
        assert_eq!(parse_amount("1,234", None), Some(Amount(12_340)));
    }

    #[test]
    fn modes_from_field() {
        use StandardMode::*;
        let g = guess_mode_from_field;
        assert_eq!(g("1234", true), Some((ChequeEmis, Some("1234".into()))));
        assert_eq!(g("CHQ 1234", true), Some((ChequeEmis, Some("1234".into()))));
        assert_eq!(
            g("CHEQUE1234", true),
            Some((ChequeEmis, Some("1234".into())))
        );
        assert_eq!(g("Chq n°567", true), Some((ChequeEmis, Some("567".into()))));
        assert_eq!(g("Chèque", true), Some((ChequeEmis, None)));
        assert_eq!(g("CB 12/03", true), Some((Carte, None)));
        assert_eq!(g("Carte", true), Some((Carte, None)));
        assert_eq!(g("Facture carte", true), Some((Carte, None)));
        assert_eq!(g("Retrait DAB", true), Some((RetraitDab, None)));
        assert_eq!(g("ATM", true), Some((RetraitDab, None)));
        assert_eq!(g("Prélèvement", true), Some((Prelevement, None)));
        assert_eq!(g("PRLV SEPA", true), Some((Prelevement, None)));
        assert_eq!(g("Virement émis", true), Some((VirementEmis, None)));
        assert_eq!(g("TIP", true), Some((Tip, None)));
        assert_eq!(g("Versement espèces", false), Some((Versement, None)));
        assert_eq!(g("Remise chèques", false), Some((DepotCheque, None)));
        assert_eq!(g("Dépôt de chèque", false), Some((DepotCheque, None)));
        assert_eq!(g("VIR SEPA", false), Some((VirementRecu, None)));
        assert_eq!(g("Titre", false), Some((Titre, None)));
        assert_eq!(g("1234", false), None);
        assert_eq!(g("Divers", true), None);
        assert_eq!(g("", true), None);
    }

    #[test]
    fn modes_from_label() {
        use StandardMode::*;
        let g = guess_mode_from_label;
        assert_eq!(g("CB CARREFOUR 12/03", true), Some(Carte));
        assert_eq!(g("C.B. FNAC", true), Some(Carte));
        assert_eq!(g("PAIEMENT PAR CARTE X1234", true), Some(Carte));
        assert_eq!(g("RETRAIT DAB 12/03", true), Some(RetraitDab));
        assert_eq!(g("PRLV SEPA EDF", true), Some(Prelevement));
        assert_eq!(g("PRÉLÈVEMENT FREE", true), Some(Prelevement));
        assert_eq!(g("VIR SEPA LOYER", true), Some(VirementEmis));
        assert_eq!(g("CHEQUE 1234", true), Some(ChequeEmis));
        assert_eq!(g("VIREMENT SALAIRE", false), Some(VirementRecu));
        assert_eq!(g("REMISE CHEQUE", false), Some(DepotCheque));
        assert_eq!(g("VERSEMENT ESPECES", false), Some(Versement));
        // Pas de faux positifs sur des débuts de mots (« CARREFOUR » ≠ carte).
        assert_eq!(g("CARREFOUR", true), None);
        assert_eq!(g("CHAUSSURES", true), None);
    }

    #[test]
    fn combined_mode_guess() {
        use StandardMode::*;
        assert_eq!(guess_mode("", "CB FNAC", "", true), (Carte, None));
        assert_eq!(guess_mode("", "FNAC", "", true), (Prelevement, None));
        assert_eq!(guess_mode("", "EMPLOYEUR", "", false), (VirementRecu, None));
        assert_eq!(
            guess_mode("Chèque émis", "Dupont", "loyer 1234567", true),
            (ChequeEmis, Some("1234567".into()))
        );
        assert_eq!(
            guess_mode("Divers", "PRLV EDF", "", true),
            (Prelevement, None)
        );
        assert_eq!(find_cheque_number("n°12 n°45678"), Some("45678".into()));
    }

    #[test]
    fn count_labels() {
        assert_eq!(operation_count_label(0), "aucune opération");
        assert_eq!(operation_count_label(1), "une opération");
        assert_eq!(operation_count_label(12), "12 opérations");
    }

    #[test]
    fn format_detection() {
        assert_eq!(
            ImportFormat::detect(b"OFXHEADER:100\r\n<OFX>"),
            ImportFormat::Ofx
        );
        assert_eq!(
            ImportFormat::detect(b"<?xml version=\"1.0\"?><OFX></OFX>"),
            ImportFormat::Ofx
        );
        assert_eq!(
            ImportFormat::detect(b"!Type:Bank\nD01/02/2026\n"),
            ImportFormat::Qif
        );
        assert_eq!(
            ImportFormat::detect(b"D01/02/2026\nT-1\n^\n"),
            ImportFormat::Qif
        );
        assert_eq!(
            ImportFormat::detect(b"Date;Montant\n01/02/2026;-1\n"),
            ImportFormat::Csv
        );
    }

    #[test]
    fn account_matching() {
        let mut doc = document(&[]);
        let mut a = empty_account();
        a.name = "Livret".into();
        a.bank_code = "30004".into();
        a.agency_code = "00123".into();
        a.number = "00012345678".into();
        let mut b = empty_account();
        b.name = "Courant".into();
        b.iban = "FR76 1027 8060 0000 0204 1250 128".into();
        let mut c = empty_account();
        c.name = "Autre banque".into();
        c.bank_code = "99999".into();
        c.number = "12345678".into();
        doc.accounts = vec![c, a, b];

        let ofx = |bank: Option<&str>, branch: Option<&str>, acct: &str| ImportedAccount {
            bank_id: bank.map(Into::into),
            branch_id: branch.map(Into::into),
            account_id: Some(acct.into()),
            ..Default::default()
        };
        assert_eq!(
            match_account(&doc, &ofx(Some("30004"), Some("123"), "12345678")),
            Some(1)
        );
        // Numéro suivi de la clé RIB.
        assert_eq!(
            match_account(&doc, &ofx(Some("30004"), None, "0001234567812")),
            Some(1)
        );
        // Recherche dans l'IBAN.
        assert_eq!(
            match_account(&doc, &ofx(None, None, "00020412501")),
            Some(2)
        );
        // Banque différente : aucun compte.
        assert_eq!(
            match_account(&doc, &ofx(Some("11111"), None, "12345678")),
            None
        );
        // Sans banque : le compte de même numéro au meilleur score.
        assert!(match_account(&doc, &ofx(None, None, "12345678")).is_some());
        assert_eq!(match_account(&doc, &ofx(None, None, "999")), None);

        let named = ImportedAccount {
            name: Some("livret".into()),
            ..Default::default()
        };
        assert_eq!(match_account(&doc, &named), Some(1));
        assert_eq!(match_account(&doc, &ImportedAccount::default()), None);

        let described = ofx(Some("30004"), Some("00123"), "00012345678");
        assert_eq!(described.description(), "n° 30004-00123-00012345678");
    }

    #[test]
    fn duplicates() {
        use StandardMode::*;
        let mut account = empty_account();
        account.operations = vec![
            existing(ymd(2026, 3, 5), -450, Carte, "Café"),
            existing(ymd(2026, 3, 6), -5000, ChequeEmis, "Dupont"),
            existing(ymd(2026, 3, 9), -1999, Prelevement, "Free"),
        ];
        let lines = vec![
            imported(ymd(2026, 3, 5), -450, Carte, "CAFE"),
            // Deuxième café identique : une seule opération existe déjà.
            imported(ymd(2026, 3, 5), -450, Carte, "CAFE"),
            // Chèque numéroté : même mode « Chèque émis ».
            imported(ymd(2026, 3, 6), -5000, ChequeEmis, "Dupont"),
            // Un jour d'écart : nouveau sans tolérance.
            imported(ymd(2026, 3, 10), -1999, Prelevement, "FREE"),
            // Autre montant.
            imported(ymd(2026, 3, 5), -451, Carte, "CAFE"),
        ];
        let statuses = detect_duplicates(&account, &lines, &DuplicateOptions::default());
        let dup = |existing| ImportStatus::ProbableDuplicate {
            existing,
            same_fitid: false,
        };
        assert_eq!(
            statuses,
            vec![
                dup(0),
                ImportStatus::New,
                dup(1),
                ImportStatus::New,
                ImportStatus::New
            ]
        );

        let tolerant = DuplicateOptions {
            date_tolerance_days: 2,
            match_mode: true,
        };
        assert_eq!(detect_duplicates(&account, &lines, &tolerant)[3], dup(2));

        // Mode différent : doublon seulement si le mode est ignoré.
        let other_mode = vec![imported(ymd(2026, 3, 5), -450, Prelevement, "CAFE")];
        assert_eq!(
            detect_duplicates(&account, &other_mode, &DuplicateOptions::default()),
            vec![ImportStatus::New]
        );
        let ignore_mode = DuplicateOptions {
            match_mode: false,
            ..Default::default()
        };
        assert_eq!(
            detect_duplicates(&account, &other_mode, &ignore_mode),
            vec![dup(0)]
        );
    }

    #[test]
    fn pointed_balance_after_import() {
        let d = ymd(2026, 10, 1);
        let mut account = empty_account();
        let mut pointed = existing(d, -10_000, StandardMode::Carte, "Pointée");
        pointed.tack = Tack::Pointed;
        let pending = existing(d, -2_500, StandardMode::Carte, "Saisie");
        let cheque = existing(d, -7_000, StandardMode::ChequeEmis, "Chèque non encaissé");
        account.operations = vec![pointed, pending, cheque];
        let lines = [
            imported(d, -2_500, StandardMode::Carte, "Saisie"),
            imported(d, 50_000, StandardMode::VirementRecu, "Salaire"),
            imported(
                ymd(2026, 10, 9),
                -1_000,
                StandardMode::Carte,
                "Après le solde",
            ),
        ];
        let statuses = detect_duplicates(&account, &lines, &DuplicateOptions::default());
        let include: Vec<bool> = statuses.iter().map(|s| s.is_new()).collect();
        assert_eq!(include, [false, true, true]);
        // Pointée, doublon non pointé reconnu, salaire ; ni le chèque ni la
        // ligne postérieure au solde.
        assert_eq!(
            pointed_after_import(
                &account,
                &lines,
                &statuses,
                &include,
                Some(ymd(2026, 10, 8))
            ),
            Amount::from_cents(-10_000 - 2_500 + 50_000)
        );
        assert_eq!(
            pointed_after_import(&account, &lines, &statuses, &include, None),
            Amount::from_cents(-10_000 - 2_500 + 50_000 - 1_000)
        );
    }

    #[test]
    fn duplicates_by_fitid() {
        use StandardMode::*;
        let mut account = empty_account();
        let mut op = existing(ymd(2026, 3, 5), -450, Carte, "Café");
        op.extra.push((FITID_KEY.into(), "A1".into()));
        let mut op2 = existing(ymd(2026, 3, 5), -450, Carte, "Café");
        op2.extra.push((FITID_KEY.into(), "A2".into()));
        account.operations = vec![op, op2];

        let mut same = imported(ymd(2026, 3, 1), -999, Prelevement, "x");
        same.fitid = Some("A2".into());
        let mut other = imported(ymd(2026, 3, 5), -450, Carte, "CAFE");
        other.fitid = Some("B7".into());
        let statuses = detect_duplicates(&account, &[other, same], &DuplicateOptions::default());
        assert_eq!(
            statuses,
            vec![
                ImportStatus::New,
                ImportStatus::ProbableDuplicate {
                    existing: 1,
                    same_fitid: true
                }
            ]
        );
    }

    fn sample_categories() -> Document {
        document(&[
            (10, "Alimentation", false),
            (11, "Supermarché", true),
            (12, "Restaurant", true),
            (20, "Loisirs", false),
            (21, "Livres", true),
            (22, "Restaurant", true),
            (30, "---", false),
        ])
    }

    #[test]
    fn category_resolution() {
        let doc = sample_categories();
        assert_eq!(resolve_category(&doc, "", ""), None);
        assert_eq!(resolve_category(&doc, "alimentation", ""), Some(10));
        assert_eq!(resolve_category(&doc, "Supermarche", ""), Some(11));
        assert_eq!(resolve_category(&doc, "", "Livres"), Some(21));
        // Inclusion : « Livres d'occasion » contient « livres ».
        assert_eq!(resolve_category(&doc, "Livres d'occasion", ""), Some(21));
        // Le parent départage deux sous-catégories de même nom.
        assert_eq!(resolve_category(&doc, "Loisirs", "Restaurant"), Some(22));
        assert_eq!(
            resolve_category(&doc, "Alimentation", "Restaurant"),
            Some(12)
        );
        // Sous-catégorie inconnue : la catégorie principale.
        assert_eq!(resolve_category(&doc, "Loisirs", "Cinéma"), Some(20));
        // La catégorie « --- » (motif vide) ne correspond pas à tout.
        assert_eq!(resolve_category(&doc, "Impôts", ""), None);
        assert_eq!(resolve_category(&doc, "Impôts", "Taxe foncière"), None);
    }

    #[test]
    fn category_creation() {
        let mut doc = sample_categories();
        assert_eq!(ensure_category(&mut doc, "Loisirs", "Livres"), Some(21));
        assert_eq!(ensure_category(&mut doc, "Loisirs", ""), Some(20));
        assert_eq!(ensure_category(&mut doc, "", ""), None);
        assert_eq!(ensure_category(&mut doc, "Impôts; taxes", ""), Some(31));
        assert_eq!(
            doc.categories.last().map(|c| c.name.as_str()),
            Some("Impôts taxes")
        );
        // Nouvelle sous-catégorie sous un parent existant : rangée après ses
        // sœurs.
        assert_eq!(
            ensure_category(&mut doc, "Alimentation", "Boulangerie"),
            Some(32)
        );
        assert_eq!(doc.categories[3].name, "Boulangerie");
        assert_eq!(
            doc.category_label(32).as_deref(),
            Some("Alimentation > Boulangerie")
        );
        assert_eq!(
            resolve_category(&doc, "Alimentation", "Boulangerie"),
            Some(32)
        );
        // Parent et sous-catégorie inconnus : les deux sont créés.
        assert_eq!(ensure_category(&mut doc, "Transport", "Essence"), Some(34));
        let n = doc.categories.len();
        assert_eq!(doc.categories[n - 2].name, "Transport");
        assert!(!doc.categories[n - 2].is_child);
        assert_eq!(doc.categories[n - 1].name, "Essence");
        assert!(doc.categories[n - 1].is_child);
        assert_eq!(
            doc.category_label(34).as_deref(),
            Some("Transport > Essence")
        );

        let mut doc = document(&[
            (0, "Loisirs", false),
            (1, "Livres", true),
            (2, "Divers", false),
        ]);
        let mut op = imported(ymd(2026, 1, 1), -100, StandardMode::Carte, "x");
        op.category = Some("Loisirs".into());
        op.subcategory = Some("Livres".into());
        assert!(create_missing_categories(&mut doc, [&op]).is_empty());
        op.category = Some("Divers".into());
        op.subcategory = Some("Cadeaux".into());
        let mut op2 = op.clone();
        op2.category = Some("Santé".into());
        op2.subcategory = Some("Pharmacie".into());
        assert_eq!(
            create_missing_categories(&mut doc, [&op, &op2, &op2]),
            vec![3, 4, 5]
        );
        assert_eq!(doc.category_label(3).as_deref(), Some("Divers > Cadeaux"));
        assert_eq!(doc.category_label(5).as_deref(), Some("Santé > Pharmacie"));
    }

    #[test]
    fn conversion() {
        let doc = sample_categories();
        let mut op = imported(
            ymd(2026, 3, 6),
            -5000,
            StandardMode::ChequeEmis,
            "Dupont; Jean",
        );
        op.cheque_number = Some("1234".into());
        op.details = "loyer".into();
        op.category = Some("Loisirs".into());
        op.subcategory = Some("Livres".into());
        op.fitid = Some("XYZ".into());
        let converted = to_operation(&doc, &op, &ConvertOptions::default());
        assert_eq!(converted.uid, 0);
        assert_eq!(converted.details, "n°1234 loyer");
        assert_eq!(converted.payee, "Dupont Jean");
        assert_eq!(converted.cheque_number(), Some("1234"));
        assert_eq!(converted.mode_label(), "Chq 1234");
        assert_eq!(converted.category, Some(21));
        assert_eq!(converted.amount, Amount::from_cents(-5000));
        assert_eq!(converted.mark_date, converted.date);
        assert_eq!(converted.tack, Tack::None);
        assert!(converted.extra.is_empty());

        // Numéro déjà présent : pas de doublement ; FITID conservé ; pointage.
        op.details = "n°1234 loyer".into();
        op.tack = Some(Tack::Reconciled);
        let options = ConvertOptions {
            default_tack: Tack::Pointed,
            store_fitid: true,
        };
        let converted = to_operation(&doc, &op, &options);
        assert_eq!(converted.details, "n°1234 loyer");
        assert_eq!(converted.tack, Tack::Reconciled);
        assert_eq!(converted.extra(FITID_KEY), Some("XYZ"));
        op.tack = None;
        assert_eq!(to_operation(&doc, &op, &options).tack, Tack::Pointed);

        // « n°12345 » ne vaut pas « n°1234 ».
        op.details = "n°12345".into();
        assert_eq!(to_operation(&doc, &op, &options).details, "n°1234 n°12345");

        // Catégorie inconnue : pas de catégorie.
        op.category = Some("Inconnue".into());
        op.subcategory = None;
        assert_eq!(to_operation(&doc, &op, &options).category, None);
    }

    /// Des fichiers aléatoires (générateur déterministe) ne provoquent
    /// jamais de panique.
    #[test]
    fn garbage_never_panics() {
        const PIECES: &[&str] = &[
            "<",
            ">",
            "</",
            "<STMTTRN>",
            "<DTPOSTED>",
            "<TRNAMT>",
            "<NAME>",
            "<OFX>",
            "<STMTRS>",
            "<![CDATA[",
            "]]>",
            "<!--",
            "&amp;",
            "&#x",
            ";",
            ",",
            "\"",
            "\n",
            "\r",
            "^",
            "D",
            "T",
            "!Type:Bank",
            "!Account",
            "N",
            "L[",
            ":",
            "/",
            "-",
            ".",
            "'",
            "2026",
            "0305",
            "12",
            "31",
            "é",
            "€",
            "ß",
            "\u{a0}",
            "(",
            ")",
            "+",
            "abc",
        ];
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let csv_config = csv::CsvConfig {
            columns: csv::CsvColumns {
                date: Some(0),
                amount: Some(1),
                payee: Some(2),
                tack: Some(3),
                value_date: Some(9),
                ..Default::default()
            },
            separator: ',',
            today: today(),
            ..Default::default()
        };
        let qif_options = qif::QifOptions {
            today: today(),
            ..Default::default()
        };
        for _ in 0..3000 {
            let len = (next() % 60) as usize;
            let mut text = String::new();
            for _ in 0..len {
                text.push_str(PIECES[(next() % PIECES.len() as u64) as usize]);
            }
            let mut bytes = text.into_bytes();
            if next() % 4 == 0 && !bytes.is_empty() {
                let i = (next() % bytes.len() as u64) as usize;
                bytes[i] = (next() % 256) as u8;
            }
            let _ = ofx::parse(&bytes, &ofx::OfxOptions::default());
            let _ = qif::parse(&bytes, &qif_options);
            let _ = csv::parse(&bytes, &csv_config);
            let _ = ImportFormat::detect(&bytes);
        }
    }
}
