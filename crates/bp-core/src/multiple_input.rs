//! Saisie multiple (ex-script `multiple_input.py`) : analyse d'un texte libre
//! de plusieurs lignes en opérations.
//!
//! Trois formats sont reconnus, ligne par ligne :
//!
//! - **standard** : `date;mode;tiers;détails;catégorie;montant` ;
//! - **rapide** (pas de `;`, des espaces) : `date mode tiers… montant`, avec
//!   deux espaces pour séparer tiers, détails et catégorie en cas d'ambiguïté ;
//! - **tableau collé** (tabulations, ex. relevé copié d'un site bancaire) :
//!   la première colonne date, la colonne de libellé suivante et la première
//!   colonne montant qui suit sont retenues.
//!
//! Saisies partielles : date incomplète (`15`, `1503`, `15/03`), mode abrégé
//! (`prel`, `cb`, `vir+`), signe `+`/`-` sur le mode ou le montant, catégorie
//! approchée (`elec`, `mai+ene` pour plusieurs mots, `x+` pour la 2ᵉ
//! correspondance, `x++` pour la 3ᵉ…), abréviations personnelles pour le tiers
//! et le détail.
//!
//! # Abréviations
//!
//! L'original les mémorisait dans le fichier sous la clé de données
//! `fabiobpabbreviations` (ligne `bp_script_data_fabiobpabbreviations=` du
//! `.bp`) : pickle Python 2 (protocole 0) d'un dictionnaire
//! `{raccourci: texte complet}`, chaînes CP1252, clés et valeurs sans espaces
//! de bord et non vides. Elles s'éditaient sous la forme d'une ligne
//! `raccourci=texte complet` par abréviation ([`parse_abbreviations`],
//! [`format_abbreviations`]). Un raccourci remplace le tiers ou le détail
//! **entier** qui lui est égal (casse comprise).

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};

use crate::edit::mode_is_debit;
use crate::model::{Amount, Date, Document, Mode, Operation, StandardMode, Tack};

/// Clé des données persistantes des abréviations.
pub const ABBREVIATIONS_KEY: &str = "fabiobpabbreviations";

/// Abréviations : raccourci → texte complet.
pub type Abbreviations = BTreeMap<String, String>;

/// Aide affichée par le bouton « Aide ».
pub const HELP: &str = "Format de saisie (mode standard) :\n   <date> ; <mode> ; <tiers> ; <détails> ; <catégorie> ; <montant>\n\nFormat de saisie (mode rapide) :\n   Utilisez des espaces au lieu des points-virgules et BankPerfect essaiera de deviner la position des champs.\n   S'il y a risque de confusion entre les champs Tiers, Détails et Catégorie (qui peuvent contenir plusieurs mots) :\n   séparez ces champs (et uniquement ceux-là) par 2 espaces au lieu d'un.\n\nPour les champs date, mode et catégorie, il vous suffit de ne saisir\nque quelques caractères significatifs.\nExemple :   15;prel;EDF;;élec;99 (mode standard)\nExemple :   15 prél EDF elec 99 (mode rapide)\n\nVous pouvez ajouter + ou - au champ mode pour indiquer\nqu'une opération est débitrice ou créditrice (si le mode est ambigu).\nExemples : cheq+ cheq- vir+ vir-";

/// Lit les abréviations saisies sous la forme `raccourci=texte complet`, une
/// par ligne. Correction : un texte complet contenant `=` était tronqué.
pub fn parse_abbreviations(text: &str) -> Abbreviations {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .collect()
}

/// Texte d'édition des abréviations, triées par raccourci.
pub fn format_abbreviations(abbreviations: &Abbreviations) -> String {
    abbreviations
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Contexte d'analyse.
#[derive(Debug, Clone, Copy)]
pub struct InputContext<'a> {
    /// Catégories et modes personnalisés.
    pub document: &'a Document,
    /// Date du jour, pour compléter les dates partielles.
    pub today: Date,
    pub abbreviations: &'a Abbreviations,
}

/// Une ligne analysée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputLine {
    /// Texte saisi.
    pub source: String,
    /// `None` : date illisible.
    pub date: Option<Date>,
    pub mode: Mode,
    /// `None` : tiers absent (obligatoire).
    pub payee: Option<String>,
    /// Détail (numéro de chèque compris, `n°1234`).
    pub details: String,
    pub category: Option<u32>,
    /// Une catégorie a été saisie mais n'a pas été reconnue.
    pub category_unknown: bool,
    /// Montant signé selon le mode ; `None` : montant absent ou illisible.
    pub amount: Option<Amount>,
}

impl InputLine {
    /// La ligne peut-elle être insérée ? (date, tiers et montant reconnus)
    pub fn is_valid(&self) -> bool {
        self.date.is_some() && self.payee.is_some() && self.amount.is_some()
    }
}

/// Échec de l'insertion.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MultipleInputError {
    #[error("compte introuvable")]
    AccountNotFound,
    #[error("veuillez corriger les lignes contenant des valeurs incorrectes")]
    InvalidLines(Vec<usize>),
}

/// Analyse un texte de plusieurs lignes ; les lignes vides sont ignorées.
pub fn parse_input(text: &str, context: &InputContext) -> Vec<InputLine> {
    let categories = CategoryMatcher::new(context.document);
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_line_with(line, context, &categories))
        .collect()
}

/// Analyse une seule ligne.
pub fn parse_line(line: &str, context: &InputContext) -> InputLine {
    parse_line_with(line, context, &CategoryMatcher::new(context.document))
}

/// Total des montants reconnus. Correction : l'original additionnait les
/// montants tels que saisis, sans le signe imposé par le mode.
pub fn total(lines: &[InputLine]) -> Amount {
    lines.iter().filter_map(|l| l.amount).sum()
}

/// Insère les lignes à la fin du compte, avec le pointage `tack` (« Insérer »
/// ou « Insérer pointé »). Si une ligne est invalide, rien n'est inséré et
/// l'erreur donne leurs positions. Renvoie les index des nouvelles opérations.
pub fn insert_lines(
    document: &mut Document,
    account: usize,
    lines: &[InputLine],
    tack: Tack,
) -> Result<Vec<usize>, MultipleInputError> {
    if account >= document.accounts.len() {
        return Err(MultipleInputError::AccountNotFound);
    }
    let invalid: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.is_valid())
        .map(|(i, _)| i)
        .collect();
    if !invalid.is_empty() {
        return Err(MultipleInputError::InvalidLines(invalid));
    }
    let mut uid = document.next_operation_uid();
    let ops = &mut document.accounts[account].operations;
    let start = ops.len();
    for line in lines {
        let (Some(date), Some(payee), Some(amount)) = (line.date, &line.payee, line.amount) else {
            continue;
        };
        ops.push(Operation {
            uid,
            date,
            mark_date: date,
            tack,
            mode: line.mode.clone(),
            payee: payee.clone(),
            details: line.details.clone(),
            category: line.category,
            amount,
            extra: Vec::new(),
        });
        uid += 1;
    }
    Ok((start..ops.len()).collect())
}

// ---------------------------------------------------------------------------
// Analyse
// ---------------------------------------------------------------------------

/// Champs bruts d'une ligne, avant interprétation.
#[derive(Debug, Default)]
struct RawFields {
    date: String,
    mode: String,
    payee: String,
    details: String,
    category: String,
    amount: String,
    /// Sens déjà déterminé (tableau collé).
    sign: Option<i8>,
}

fn parse_line_with(line: &str, context: &InputContext, categories: &CategoryMatcher) -> InputLine {
    let raw = if line.contains('\t') {
        split_tabs(line)
    } else if !line.contains(';') && line.contains(' ') {
        split_spaces(line, categories)
    } else {
        split_semicolons(line)
    };
    let sign = raw.sign.unwrap_or_else(|| {
        if raw.amount.contains('-') || raw.mode.contains('-') {
            -1
        } else if raw.amount.contains('+') || raw.mode.contains('+') {
            1
        } else {
            sign_from_label(&raw.mode, -1)
        }
    });
    let (mode, cheque) = resolve_mode(&raw.mode, sign, context.document);
    let expand = |text: &str| {
        let text = text.trim();
        context
            .abbreviations
            .get(text)
            .cloned()
            .unwrap_or_else(|| text.to_string())
    };
    let payee = Some(expand(&raw.payee)).filter(|p| !p.is_empty());
    let mut details = expand(&raw.details);
    if let Some(number) = cheque
        && !has_cheque_number(&details)
    {
        details = if details.is_empty() {
            format!("n°{number}")
        } else {
            format!("n°{number} {details}")
        };
    }
    let category = categories.find(&raw.category);
    let amount = parse_amount(&raw.amount).map(|a| {
        let abs = a.0.abs();
        Amount(if mode_is_debit(context.document, &mode) {
            -abs
        } else {
            abs
        })
    });
    InputLine {
        source: line.to_string(),
        date: parse_date(&raw.date, context.today),
        mode,
        payee,
        details,
        category,
        category_unknown: category.is_none() && !raw.category.trim().is_empty(),
        amount,
    }
}

fn split_semicolons(line: &str) -> RawFields {
    let mut fields = line.split(';').map(str::to_string);
    let mut next = || fields.next().unwrap_or_default();
    RawFields {
        date: next(),
        mode: next(),
        payee: next(),
        details: next(),
        category: next(),
        amount: next(),
        sign: None,
    }
}

/// Mode rapide (`evaluate_with_spaces`).
fn split_spaces(line: &str, categories: &CategoryMatcher) -> RawFields {
    let words: Vec<&str> = line.split(' ').collect();
    if words.len() < 4 {
        return split_semicolons(&line.replace(' ', ";"));
    }
    let mut raw = RawFields {
        date: words[0].to_string(),
        mode: words[1].to_string(),
        ..Default::default()
    };
    let last = words[words.len() - 1];
    let middle = if is_value(last) {
        raw.amount = last.to_string();
        &words[2..words.len() - 1]
    } else {
        // L'original prenait alors un montant nul ; la ligne est invalide.
        &words[2..]
    };
    if middle.len() == 1 {
        raw.payee = middle[0].to_string();
        return raw;
    }
    let joined = middle.join(" ");
    if joined.contains("  ") {
        let parts: Vec<&str> = joined
            .split("  ")
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        match parts.as_slice() {
            [] => {}
            [payee] => raw.payee = payee.to_string(),
            [payee, second] => {
                raw.payee = payee.to_string();
                // Deux membres : catégorie si possible, sinon détail.
                if categories.find(second).is_some() {
                    raw.category = second.to_string();
                } else {
                    raw.details = second.to_string();
                }
            }
            [payee, details, rest @ ..] => {
                raw.payee = payee.to_string();
                raw.details = details.to_string();
                raw.category = rest.join(" ");
            }
        }
    } else {
        // Dernier mot comme catégorie s'il en désigne une, le reste en tiers.
        let words: Vec<&str> = joined.split(' ').filter(|w| !w.is_empty()).collect();
        match words.split_last() {
            Some((category, rest)) if !rest.is_empty() && categories.find(category).is_some() => {
                raw.payee = rest.join(" ");
                raw.category = category.to_string();
            }
            _ => raw.payee = words.join(" "),
        }
    }
    raw
}

/// Tableau collé (`evaluate_with_tabs`).
fn split_tabs(line: &str) -> RawFields {
    let words: Vec<&str> = line.split('\t').map(str::trim).collect();
    if words.len() < 3 {
        return RawFields::default();
    }
    let date_index = words.iter().position(|w| is_date(w));
    // Le libellé : première colonne non vide après la date qui n'est pas une
    // date (l'original pouvait retenir une colonne vide).
    let payee_index = date_index
        .and_then(|d| (d + 1..words.len()).find(|&i| !words[i].is_empty() && !is_date(words[i])));
    let value_index = payee_index.and_then(|p| (p + 1..words.len()).find(|&i| is_value(words[i])));
    let (Some(d), Some(p), Some(v)) = (date_index, payee_index, value_index) else {
        return RawFields {
            date: date_index.map(|d| words[d].to_string()).unwrap_or_default(),
            payee: payee_index
                .map(|p| words[p].to_string())
                .unwrap_or_default(),
            ..Default::default()
        };
    };
    // Colonne vide juste avant le montant : colonne « Crédit ».
    let last_resort = if words[v - 1].is_empty() { 1 } else { -1 };
    let value = words[v];
    let sign = if value.contains('-') {
        -1
    } else {
        sign_from_label(words[p], last_resort)
    };
    RawFields {
        date: words[d].to_string(),
        mode: words[p].to_string(),
        payee: words[p].to_string(),
        amount: value.replace('+', ""),
        sign: Some(sign),
        ..Default::default()
    }
}

/// Sens déduit d'un libellé bancaire (`calculate_sign`).
fn sign_from_label(label: &str, default: i8) -> i8 {
    let s = label.trim().to_uppercase();
    const DEBIT: [&str; 6] = [
        "PRELEVEMENT",
        "CHEQUE",
        "VR. PERMANENT",
        "ECHEANCE",
        "VIRT CPTE A CPTE EMIS",
        "RETRAIT DAB",
    ];
    const CREDIT: [&str; 4] = [
        "REMISE CHEQ",
        "VIREMENT RECU",
        "VIR CPTE A CPTE RECU",
        "BORDEREAU",
    ];
    if DEBIT.iter().any(|p| s.starts_with(p)) {
        -1
    } else if CREDIT.iter().any(|p| s.starts_with(p)) {
        1
    } else {
        default
    }
}

/// Abréviations des modes de recette et de dépense (`getmode`).
const CREDIT_MODES: [(StandardMode, &[&str]); 3] = [
    (StandardMode::Versement, &["vers", "esp"]),
    (StandardMode::DepotCheque, &["rem", "dep", "ch"]),
    (StandardMode::VirementRecu, &["vir", "vrm", "vrt"]),
];
const DEBIT_MODES: [(StandardMode, &[&str]); 6] = [
    (StandardMode::Carte, &["fac", "cb", "car", "achat"]),
    (
        StandardMode::RetraitDab,
        &["ret", "dab", "distrib", "automat"],
    ),
    (StandardMode::ChequeEmis, &["ch"]),
    (StandardMode::Prelevement, &["pr"]),
    (StandardMode::VirementEmis, &["vir", "vrm", "vrt"]),
    (StandardMode::Tip, &["tip"]),
];

/// Mode désigné par `text` pour une opération de sens `sign`, et numéro de
/// chèque éventuel.
///
/// Ajout : un libellé complet (« Virement reçu », ou un mode personnalisé)
/// désigne ce mode quel que soit le signe ; l'original transformait
/// « Virement reçu » sans `+` en « Virement émis » et ignorait les modes
/// personnalisés. `ch123` ou `chq 123` donnent un chèque numéroté.
fn resolve_mode(text: &str, sign: i8, document: &Document) -> (Mode, Option<String>) {
    let s = text.trim();
    if s.is_empty() {
        return (default_mode(sign), None);
    }
    let bytes = s.as_bytes();
    if sign < 0
        && s.len() > 6
        && bytes[2] == b'/'
        && bytes[..2]
            .iter()
            .chain(&bytes[3..5])
            .all(u8::is_ascii_digit)
    {
        // Libellé de relevé commençant par « JJ/MM » : paiement par carte.
        return (Mode::Standard(StandardMode::Carte), None);
    }
    if sign < 0 && s.chars().all(|c| c.is_ascii_digit()) {
        return (
            Mode::Standard(StandardMode::ChequeEmis),
            Some(s.to_string()),
        );
    }
    let v = pattern(s);
    let bare = v.trim_end_matches('+');
    if let Some(mode) = document
        .mode_list()
        .into_iter()
        .find(|m| pattern(m.label()) == bare)
    {
        return (mode, None);
    }
    if sign < 0
        && let Some(rest) = bare.strip_prefix("ch")
    {
        let number = rest.trim_start_matches(|c: char| c.is_ascii_alphabetic());
        if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
            return (
                Mode::Standard(StandardMode::ChequeEmis),
                Some(number.to_string()),
            );
        }
    }
    let table: &[(StandardMode, &[&str])] = if sign > 0 {
        &CREDIT_MODES
    } else {
        &DEBIT_MODES
    };
    table
        .iter()
        .find(|(_, prefixes)| prefixes.iter().any(|p| v.starts_with(p)))
        .map(|(mode, _)| (Mode::Standard(*mode), None))
        .unwrap_or_else(|| (default_mode(sign), None))
}

fn default_mode(sign: i8) -> Mode {
    Mode::Standard(if sign > 0 {
        StandardMode::VirementRecu
    } else {
        StandardMode::Prelevement
    })
}

/// Le détail porte-t-il déjà un numéro de chèque `°1234` ?
fn has_cheque_number(details: &str) -> bool {
    details
        .split('°')
        .skip(1)
        .any(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// Forme comparable d'un texte (`Pattern`) : minuscules, sans accents, sans
/// espaces ni ponctuation ` -.,;:/!?'_=`.
fn pattern(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(fold_accent)
        .filter(|c| !" -.,;:/!?'_=".contains(*c))
        .collect()
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'æ' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'œ' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        c => c,
    }
}

/// Texte de date reconnu par la détection des colonnes (`is_date`).
fn is_date(text: &str) -> bool {
    let s = text.trim();
    if s.len() > 10 {
        return false;
    }
    let parts: Vec<&str> = s.split(['/', '-']).collect();
    parts.len() == 3
        && parts.iter().any(|p| !p.is_empty())
        && parts.concat().chars().all(|c| c.is_ascii_digit())
}

/// Texte de montant reconnu (`is_value`).
fn is_value(text: &str) -> bool {
    parse_amount(text).is_some()
}

/// Montant saisi : chiffres, séparateurs `.`/`,`, signe, symboles monétaires
/// (`€`, `$`, `eur`, `f`) et espaces tolérés. Strict : « A4 » n'est pas un
/// montant.
fn parse_amount(text: &str) -> Option<Amount> {
    let s = text
        .to_lowercase()
        .replace("eur", "")
        .replace(['€', '$', 'f', ' ', '\u{a0}', '+'], "");
    let body = s.strip_prefix('-').unwrap_or(&s);
    if body.is_empty()
        || !body.chars().any(|c| c.is_ascii_digit())
        || !body
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return None;
    }
    Amount::parse(&s)
}

/// Date saisie (`strtodate`) : `15`, `1503`, `150324`, `15032024`, `15/03`,
/// `15/03/24`, `15-03-2024`… Les parties absentes viennent de `today` ; mois
/// et jour sont ramenés dans les bornes. Vide = aujourd'hui. `None` si une
/// partie n'est pas numérique (l'original prenait alors la date du jour sans
/// prévenir).
pub fn parse_date(text: &str, today: Date) -> Option<Date> {
    let now = today.to_naive();
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let t = if t.chars().all(|c| c.is_ascii_digit()) && [4, 6, 8].contains(&t.len()) {
        format!("{}/{}/{}", &t[..2], &t[2..4], &t[4..])
    } else {
        t.replace(['-', '.', '\''], "/")
    };
    let parts: Vec<&str> = t.split('/').collect();
    if parts.len() > 3 {
        return None;
    }
    let part = |i: usize, default: i64| -> Option<i64> {
        match parts.get(i).copied().unwrap_or("") {
            "" => Some(default),
            p if p.chars().all(|c| c.is_ascii_digit()) && p.len() <= 4 => p.parse().ok(),
            _ => None,
        }
    };
    let day = part(0, i64::from(now.day()))?;
    let month = part(1, i64::from(now.month()))?.clamp(1, 12) as u32;
    let mut year = part(2, i64::from(now.year()))? as i32;
    if year < 100 {
        year += now.year() - now.year() % 100;
    }
    let last_day = (28..=31)
        .rev()
        .find(|&d| NaiveDate::from_ymd_opt(year, month, d).is_some())?;
    Date::from_ymd(year, month, day.clamp(1, i64::from(last_day)) as u32)
}

/// Recherche approchée de catégorie (`GetCtg`).
struct CategoryMatcher {
    /// Formes comparables, dans l'ordre du document : `parent` pour une
    /// catégorie principale, `parent` + `enfant` pour une sous-catégorie.
    patterns: Vec<(String, u32)>,
}

impl CategoryMatcher {
    fn new(document: &Document) -> Self {
        let mut parent = String::new();
        let patterns = document
            .categories
            .iter()
            .map(|c| {
                let own = pattern(&c.name);
                if c.is_child {
                    (format!("{parent}{own}"), c.id)
                } else {
                    parent = own.clone();
                    (own, c.id)
                }
            })
            .collect();
        CategoryMatcher { patterns }
    }

    fn matches(candidate: &str, word: &str) -> bool {
        !candidate.is_empty() && (candidate.contains(word) || word.contains(candidate))
    }

    fn find(&self, text: &str) -> Option<u32> {
        let s = text.trim();
        if s.is_empty() {
            return None;
        }
        if s.contains('+') {
            return self.find_advanced(s);
        }
        let t = pattern(s);
        if t.is_empty() {
            return None;
        }
        self.patterns
            .iter()
            .find(|(p, _)| *p == t)
            .or_else(|| self.patterns.iter().find(|(p, _)| Self::matches(p, &t)))
            .map(|(_, id)| *id)
    }

    /// `a+b` : catégorie contenant tous les mots ; `x+`, `x++` : 2ᵉ, 3ᵉ…
    /// correspondance de `x`.
    fn find_advanced(&self, s: &str) -> Option<u32> {
        let stripped = s.trim_end_matches('+');
        if stripped.trim().is_empty() {
            return None;
        }
        if stripped.contains('+') {
            let words: Vec<String> = s
                .split('+')
                .map(pattern)
                .filter(|w| !w.is_empty())
                .collect();
            return self
                .patterns
                .iter()
                .find(|(p, _)| words.iter().all(|w| Self::matches(p, w)))
                .map(|(_, id)| *id);
        }
        let n = s.len() - stripped.len() + 1;
        let t = pattern(stripped);
        if t.is_empty() {
            return None;
        }
        self.patterns
            .iter()
            .filter(|(p, _)| Self::matches(p, &t))
            .nth(n - 1)
            .map(|(_, id)| *id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, CustomMode};
    use crate::reports::tests::{account, document};

    fn doc() -> Document {
        let mut doc = document(vec![account("a", "Courant", Vec::new())]);
        doc.categories.insert(
            3,
            Category {
                id: 5,
                name: "Électricité".into(),
                is_child: true,
            },
        );
        doc.categories.extend([
            Category {
                id: 6,
                name: "Loisirs".into(),
                is_child: false,
            },
            Category {
                id: 7,
                name: "Cinéma".into(),
                is_child: true,
            },
        ]);
        doc.custom_modes.push(CustomMode {
            name: "Espèces".into(),
            debit: true,
        });
        doc
    }

    fn today() -> Date {
        Date::from_ymd(2026, 10, 11).unwrap()
    }

    fn parse(text: &str) -> Vec<InputLine> {
        let doc = doc();
        let mut abbreviations = Abbreviations::new();
        abbreviations.insert("edf".into(), "Électricité de France".into());
        let context = InputContext {
            document: &doc,
            today: today(),
            abbreviations: &abbreviations,
        };
        parse_input(text, &context)
    }

    fn std(mode: StandardMode) -> Mode {
        Mode::Standard(mode)
    }

    #[test]
    fn standard_format() {
        let lines = parse("15;prel;edf;facture;élec;99,90\n\n02/09/2026;vir+;Employeur;;;2000");
        assert_eq!(lines.len(), 2);
        let l = &lines[0];
        assert_eq!(l.date, Date::from_ymd(2026, 10, 15));
        assert_eq!(l.mode, std(StandardMode::Prelevement));
        assert_eq!(l.payee.as_deref(), Some("Électricité de France"));
        assert_eq!(l.details, "facture");
        assert_eq!(l.category, Some(5), "Maison > Électricité");
        assert_eq!(l.amount, Some(Amount::from_cents(-9_990)));
        assert!(l.is_valid());
        let l = &lines[1];
        assert_eq!(l.mode, std(StandardMode::VirementRecu));
        assert_eq!(l.amount, Some(Amount::from_cents(200_000)));
        assert_eq!(total(&lines), Amount::from_cents(190_010));
    }

    #[test]
    fn quick_format() {
        let lines = parse(
            "15 prél EDF elec 99\n\
             3 cb Carrefour Market 45,10\n\
             3 cb Fnac  cadeau Paul  cinema 30\n\
             3 cb Fnac  cinema 30\n\
             3 cb Fnac  cadeau 30\n\
             4 vers Banque",
        );
        assert_eq!(lines[0].payee.as_deref(), Some("EDF"));
        assert_eq!(lines[0].category, Some(5));
        assert_eq!(lines[0].amount, Some(Amount::from_cents(-9_900)));
        assert_eq!(lines[1].payee.as_deref(), Some("Carrefour Market"));
        assert_eq!(lines[1].category, None);
        assert_eq!(lines[1].mode, std(StandardMode::Carte));
        assert_eq!(lines[2].details, "cadeau Paul");
        assert_eq!(lines[2].category, Some(7));
        assert_eq!(lines[3].category, Some(7));
        assert_eq!(lines[3].details, "");
        assert_eq!(lines[4].details, "cadeau");
        assert_eq!(lines[4].category, None);
        // « 4 vers Banque » : moins de 4 mots, pas de montant → invalide.
        assert_eq!(lines[5].payee.as_deref(), Some("Banque"));
        assert_eq!(lines[5].amount, None);
        assert!(!lines[5].is_valid());
    }

    #[test]
    fn pasted_table() {
        let lines = parse(
            "05/10/2026\tPRLV EDF\t42,50\t\n\
             06/10/2026\tVIREMENT RECU SALAIRE\t\t2 100,00\n\
             07/10/2026\t07/10 CARREFOUR\t12,30\t\n\
             08/10/2026\tDIVERS\t\t15,00",
        );
        assert_eq!(lines[0].mode, std(StandardMode::Prelevement));
        assert_eq!(lines[0].payee.as_deref(), Some("PRLV EDF"));
        assert_eq!(lines[0].amount, Some(Amount::from_cents(-4_250)));
        assert_eq!(lines[1].mode, std(StandardMode::VirementRecu));
        assert_eq!(lines[1].amount, Some(Amount::from_cents(210_000)));
        assert_eq!(lines[2].mode, std(StandardMode::Carte));
        // Colonne vide avant le montant : crédit.
        assert_eq!(lines[3].mode, std(StandardMode::VirementRecu));
        assert_eq!(lines[3].amount, Some(Amount::from_cents(1_500)));
        assert!(!parse("a\tb")[0].is_valid());
    }

    #[test]
    fn modes_and_cheques() {
        let lines = parse(
            "1;1234;Dupont;;;50\n\
             1;chq 77;Dupont;loyer;;50\n\
             1;ch;Dupont;n°88 loyer;;50\n\
             1;cheq+;Client;;;50\n\
             1;Virement reçu;Client;;;50\n\
             1;espèces;Boulanger;;;3\n\
             1;;Inconnu;;;10\n\
             1;+;Inconnu;;;10\n\
             1;dab;Retrait;;;-20",
        );
        assert_eq!(lines[0].mode, std(StandardMode::ChequeEmis));
        assert_eq!(lines[0].details, "n°1234");
        assert_eq!(lines[1].details, "n°77 loyer");
        assert_eq!(lines[2].mode, std(StandardMode::ChequeEmis));
        assert_eq!(lines[2].details, "n°88 loyer", "numéro déjà présent");
        assert_eq!(lines[3].mode, std(StandardMode::DepotCheque));
        assert_eq!(lines[3].amount, Some(Amount::from_cents(5_000)));
        assert_eq!(lines[4].mode, std(StandardMode::VirementRecu));
        assert_eq!(lines[4].amount, Some(Amount::from_cents(5_000)));
        assert_eq!(lines[5].mode, Mode::Custom("Espèces".into()));
        assert_eq!(lines[5].amount, Some(Amount::from_cents(-300)));
        assert_eq!(lines[6].mode, std(StandardMode::Prelevement));
        assert_eq!(lines[7].mode, std(StandardMode::VirementRecu));
        assert_eq!(lines[8].mode, std(StandardMode::RetraitDab));
        assert_eq!(lines[8].amount, Some(Amount::from_cents(-2_000)));
    }

    #[test]
    fn fuzzy_categories() {
        let doc = doc();
        let m = CategoryMatcher::new(&doc);
        assert_eq!(m.find("loyer"), Some(2));
        assert_eq!(m.find("MAISON"), Some(1), "égalité exacte d'abord");
        assert_eq!(m.find("mais"), Some(1));
        assert_eq!(m.find("mais+"), Some(2), "2ᵉ correspondance");
        assert_eq!(m.find("mais++"), Some(3), "3ᵉ correspondance");
        assert_eq!(m.find("mai+ele"), Some(5), "tous les mots");
        assert_eq!(m.find("loisirs cinéma"), Some(7));
        assert_eq!(m.find("zzz"), None);
        assert_eq!(m.find("+"), None);
        assert_eq!(m.find(""), None);
        let lines = parse("1;cb;X;;zzz;1");
        assert!(lines[0].category_unknown);
        assert!(lines[0].is_valid(), "catégorie inconnue tolérée");
    }

    #[test]
    fn dates() {
        let t = today();
        assert_eq!(parse_date("", t), Some(t));
        assert_eq!(parse_date("15", t), Date::from_ymd(2026, 10, 15));
        assert_eq!(parse_date("1503", t), Date::from_ymd(2026, 3, 15));
        assert_eq!(parse_date("150324", t), Date::from_ymd(2024, 3, 15));
        assert_eq!(parse_date("15032024", t), Date::from_ymd(2024, 3, 15));
        assert_eq!(parse_date("15-03-2024", t), Date::from_ymd(2024, 3, 15));
        assert_eq!(parse_date("15.3", t), Date::from_ymd(2026, 3, 15));
        assert_eq!(parse_date("31/02/2024", t), Date::from_ymd(2024, 2, 29));
        assert_eq!(parse_date("5/13", t), Date::from_ymd(2026, 12, 5));
        assert_eq!(parse_date("abc", t), None);
        assert_eq!(parse_date("1/2/3/4", t), None);
        assert!(!parse("abc;cb;X;;;1")[0].is_valid());
    }

    #[test]
    fn amounts() {
        assert_eq!(parse_amount("12,50 €"), Some(Amount::from_cents(1_250)));
        assert_eq!(
            parse_amount("1 234.5 EUR"),
            Some(Amount::from_cents(123_450))
        );
        assert_eq!(parse_amount("-3"), Some(Amount::from_cents(-300)));
        assert_eq!(parse_amount("+3"), Some(Amount::from_cents(300)));
        assert_eq!(parse_amount("A4"), None);
        assert_eq!(parse_amount(""), None);
        assert_eq!(parse_amount("-"), None);
    }

    #[test]
    fn abbreviations_round_trip() {
        let abbreviations =
            parse_abbreviations("edf = EDF Énergie\n\nmauvaise ligne\nx=a=b\n =vide\nvide=");
        assert_eq!(abbreviations.len(), 2);
        assert_eq!(abbreviations["edf"], "EDF Énergie");
        assert_eq!(abbreviations["x"], "a=b", "valeur avec « = » conservée");
        assert_eq!(
            format_abbreviations(&abbreviations),
            "edf=EDF Énergie\nx=a=b"
        );
        assert_eq!(
            parse_abbreviations(&format_abbreviations(&abbreviations)),
            abbreviations
        );
    }

    #[test]
    fn insertion() {
        let mut doc = doc();
        let abbreviations = Abbreviations::new();
        let context_doc = doc.clone();
        let context = InputContext {
            document: &context_doc,
            today: today(),
            abbreviations: &abbreviations,
        };
        let lines = parse_input("15;cb;Fnac;;;12\n16;vir+;Client;;;100", &context);
        let added = insert_lines(&mut doc, 0, &lines, Tack::Pointed).unwrap();
        assert_eq!(added, [0, 1]);
        let ops = &doc.accounts[0].operations;
        assert_eq!(ops[0].uid, 1);
        assert_eq!(ops[1].uid, 2);
        assert_eq!(ops[0].tack, Tack::Pointed);
        assert_eq!(ops[0].mark_date, ops[0].date);
        assert_eq!(ops[1].amount, Amount::from_cents(10_000));

        let bad = parse_input("15;cb;;;;12\n15;cb;X;;;12\n15;cb;Y;;;", &context);
        let before = doc.clone();
        assert_eq!(
            insert_lines(&mut doc, 0, &bad, Tack::None),
            Err(MultipleInputError::InvalidLines(vec![0, 2]))
        );
        assert_eq!(
            insert_lines(&mut doc, 3, &lines, Tack::None),
            Err(MultipleInputError::AccountNotFound)
        );
        assert_eq!(doc, before);
    }
}
