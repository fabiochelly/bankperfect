//! Prolonge un vrai fichier BankPerfect jusqu'à aujourd'hui pour en faire un
//! exemple réaliste : chaque compte actif est complété de sa dernière date
//! jusqu'à la date du jour (plus quelques opérations à venir), en imitant le
//! comportement observé dans son historique.
//!
//! Usage : `cargo run --release -p bp-core --example extend_sample -- <entrée.bp> <sortie.bp> [graine]`
//!
//! Démarche, compte par compte :
//! - détection des opérations récurrentes (même libellé normalisé, même
//!   catégorie, même sens ; périodicité mensuelle, bimestrielle, trimestrielle
//!   ou annuelle ; jour habituel ; montant fixe, révisé chaque année ou
//!   variable) puis reproduction à leur rythme ;
//! - dépenses et recettes courantes tirées au sort parmi les opérations non
//!   récurrentes de la dernière année (fréquence récente, saisonnalité,
//!   inflation et petits aléas sur les montants, chèques numérotés à la suite) ;
//! - solde maintenu dans la fourchette observée par des virements d'épargne ou
//!   des transferts entre comptes, comme le faisait l'utilisateur ;
//! - pointage : rapproché avant J-45, mélange pointé / non pointé ensuite,
//!   opérations futures (jusqu'à J+35) non pointées.
//!
//! Aucun tiers, aucune catégorie ni aucun mode n'est inventé : tout est repris
//! du fichier d'origine (les libellés bancaires sont seulement redatés).

use std::collections::{BTreeMap, HashMap};

use bp_core::format_v6::Protection;
use bp_core::model::*;
use bp_core::split::{SISTER_ACCOUNT_KEY, SISTER_LINE_KEY, TRANSFER_FROM, TRANSFER_TO};
use chrono::{Datelike, Weekday};

/// Jours d'opérations à venir générés après aujourd'hui.
const HORIZON_DAYS: i32 = 35;
/// Fenêtre récente où pointé et non pointé se mélangent.
const RECENT_DAYS: i32 = 45;
/// Inflation annuelle appliquée aux montants.
const INFLATION: f64 = 0.02;
/// Au-delà, une opération ponctuelle n'est pas rejouée au hasard.
const MAX_RANDOM_AMOUNT: f64 = 2000.0;

// ---------------------------------------------------------------------------
// Hasard reproductible
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    /// SplitMix64.
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Entier dans `lo..=hi`.
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }

    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    fn normal(&mut self) -> f64 {
        let u1 = self.unit().max(1e-12);
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    fn poisson(&mut self, lambda: f64) -> u32 {
        if lambda <= 0.0 {
            return 0;
        }
        if lambda > 30.0 {
            return (lambda + lambda.sqrt() * self.normal()).round().max(0.0) as u32;
        }
        let limit = (-lambda).exp();
        let (mut k, mut p) = (0, 1.0);
        loop {
            p *= self.unit();
            if p <= limit {
                return k;
            }
            k += 1;
        }
    }

    /// Indice tiré selon des poids cumulés.
    fn weighted(&mut self, cumulative: &[f64]) -> usize {
        let x = self.unit() * cumulative.last().copied().unwrap_or(0.0);
        cumulative
            .partition_point(|&c| c <= x)
            .min(cumulative.len() - 1)
    }
}

// ---------------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------------

fn month_index(d: Date) -> i32 {
    let n = d.to_naive();
    n.year() * 12 + n.month0() as i32
}

fn month_start(mi: i32) -> Date {
    Date::from_ymd(mi.div_euclid(12), mi.rem_euclid(12) as u32 + 1, 1).expect("mois valide")
}

fn month_end(mi: i32) -> Date {
    Date(month_start(mi + 1).0 - 1)
}

fn day_in_month(mi: i32, day: u32) -> Date {
    let last = month_end(mi).0 - month_start(mi).0 + 1;
    Date(month_start(mi).0 + (day as i32).clamp(1, last) - 1)
}

fn day_of_month(d: Date) -> u32 {
    d.to_naive().day()
}

fn is_weekend(d: Date) -> bool {
    matches!(d.to_naive().weekday(), Weekday::Sat | Weekday::Sun)
}

/// Jour ouvré suivant, ou précédent si le report change de mois.
fn business_day(d: Date) -> Date {
    let mut n = d;
    while is_weekend(n) {
        n = Date(n.0 + 1);
    }
    if month_index(n) != month_index(d) {
        n = d;
        while is_weekend(n) {
            n = Date(n.0 - 1);
        }
    }
    n
}

fn previous_business_day(d: Date) -> Date {
    let mut n = d;
    while is_weekend(n) {
        n = Date(n.0 - 1);
    }
    n
}

fn years_between(from: Date, to: Date) -> f64 {
    (to.0 - from.0) as f64 / 365.25
}

fn inflation(from: Date, to: Date) -> f64 {
    (1.0 + INFLATION).powf(years_between(from, to).max(0.0))
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

// ---------------------------------------------------------------------------
// Montants
// ---------------------------------------------------------------------------

fn euros(a: Amount) -> f64 {
    a.as_f64()
}

fn to_amount(x: f64) -> Amount {
    Amount::from_cents((x * 100.0).round() as i64)
}

fn cents(a: Amount) -> i64 {
    a.0 / 100
}

/// Pas d'arrondi naturel d'un montant : 10 €, 1 € ou le centime.
fn natural_unit(a: Amount) -> f64 {
    let c = cents(a);
    if c % 1000 == 0 {
        10.0
    } else if c % 100 == 0 {
        1.0
    } else {
        0.01
    }
}

fn round_to(x: f64, unit: f64) -> f64 {
    let r = (x / unit).round() * unit;
    if r == 0.0 { unit.copysign(x) } else { r }
}

// ---------------------------------------------------------------------------
// Libellés
// ---------------------------------------------------------------------------

/// Mots significatifs d'un libellé : sans chiffres, en majuscules, sans « s »
/// final (SALAIRES = SALAIRE).
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2 && !w.chars().any(|c| c.is_ascii_digit()))
        .map(|w| {
            let mut w = w.to_uppercase();
            if w.chars().count() > 3 && w.ends_with('S') {
                w.pop();
            }
            w
        })
        .collect()
}

fn is_transfer(op: &Operation) -> bool {
    op.payee.starts_with(TRANSFER_TO) || op.payee.starts_with(TRANSFER_FROM)
}

/// Libellé normalisé : tiers et début du détail (le détail seul pour un
/// transfert de compte à compte, dont le tiers est générique).
fn label_key(op: &Operation) -> String {
    if is_transfer(op) && !op.details.trim().is_empty() {
        return words(&op.details)
            .into_iter()
            .take(6)
            .collect::<Vec<_>>()
            .join(" ");
    }
    let mut parts: Vec<String> = words(&op.payee).into_iter().take(6).collect();
    parts.extend(words(&op.details).into_iter().take(3));
    parts.join(" ")
}

fn upper_text(op: &Operation) -> String {
    format!("{} {}", op.payee, op.details).to_uppercase()
}

/// Mouvement d'épargne, de clôture ou de régularisation : jamais rejoué au
/// hasard (le solde est équilibré à part).
fn is_internal_move(doc: &Document, op: &Operation) -> bool {
    const KEYWORDS: &[&str] = &[
        "EPARGNE",
        "EPRAGNE",
        "LIVRET",
        "L.A3",
        "PEL3",
        "TRANSFERT",
        "CLOTURE",
        "RACCORD",
        "SOLDE INITIAL",
        "REGUL",
        "00916",
        "SURPLUS",
        "OUVERTURE",
        "FACTURE CARTE",
        "ACHATS CARTE",
    ];
    let text = upper_text(op);
    KEYWORDS.iter().any(|k| text.contains(k))
        || op
            .category
            .and_then(|c| doc.category(c))
            .is_some_and(|c| c.name.eq_ignore_ascii_case("Epargne"))
}

const DATE_FORMATS: &[&str] = &[
    "%d/%m/%y", "%d.%m.%y", "%d/%m/%Y", "%d.%m.%Y", "%d/%m", "%d.%m", "%m/%Y", "%m/%y", "%d%m%y",
    "%m%y",
];
const MONTHS: [&str; 12] = [
    "JANVIER",
    "FEVRIER",
    "MARS",
    "AVRIL",
    "MAI",
    "JUIN",
    "JUILLET",
    "AOUT",
    "SEPTEMBRE",
    "OCTOBRE",
    "NOVEMBRE",
    "DECEMBRE",
];
const MONTHS_SHORT: [&str; 12] = [
    "JANV", "FEV", "MARS", "AVR", "MAI", "JUIN", "JUIL", "AOUT", "SEPT", "OCT", "NOV", "DEC",
];

/// Reconnaît une date citée dans un libellé (éventuellement tronquée en fin
/// de libellé) : renvoie son format et son décalage en jours avec la date de
/// l'opération.
fn find_date_token(token: &str, truncated: bool, orig: Date) -> Option<(&'static str, i32)> {
    let digits = token.chars().filter(char::is_ascii_digit).count();
    if digits < 1
        || !token
            .chars()
            .all(|c| c.is_ascii_digit() || c == '/' || c == '.')
        || !token.starts_with(|c: char| c.is_ascii_digit())
    {
        return None;
    }
    if !truncated && token.len() < 4 {
        return None;
    }
    for &format in DATE_FORMATS {
        for lag in (0..=31).chain(-3..0) {
            let candidate = Date(orig.0 - lag).to_naive().format(format).to_string();
            if candidate == token
                || (truncated && candidate.len() > token.len() && candidate.starts_with(token))
            {
                return Some((format, lag));
            }
        }
    }
    None
}

fn is_time_token(token: &str) -> bool {
    let b = token.as_bytes();
    b.len() == 5 && b[2] == b'H' && [0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_digit())
}

/// Référence d'ordre de virement `Vmmjjhhmm` (éventuellement tronquée).
fn is_order_token(token: &str) -> bool {
    token.len() >= 5
        && token.len() <= 9
        && token.starts_with('V')
        && token[1..].chars().all(|c| c.is_ascii_digit())
}

/// Référence alphanumérique de type PayPal (lettres et chiffres mêlés).
fn is_reference_token(token: &str) -> bool {
    if !(12..=20).contains(&token.len()) || !token.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let mut transitions = 0;
    let bytes = token.as_bytes();
    for w in bytes.windows(2) {
        if w[0].is_ascii_digit() != w[1].is_ascii_digit() {
            transitions += 1;
        }
    }
    transitions >= 4
}

fn random_digits(rng: &mut Rng, n: usize) -> String {
    (0..n)
        .map(|_| char::from(b'0' + rng.int(0, 9) as u8))
        .collect()
}

fn match_case(model: &str, word: &str) -> String {
    if model.chars().all(|c| !c.is_lowercase()) {
        word.to_string()
    } else {
        let lower = word.to_lowercase();
        let mut chars = lower.chars();
        chars
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
            .unwrap_or_default()
    }
}

/// Résultat d'un redatage : texte et décalage de la date citée.
struct Redated {
    text: String,
    lag: Option<i32>,
}

/// Redate un libellé bancaire : dates citées (« 27/07/11 », « 01/09 »,
/// « ECH 05.11.10 », « SALAIRE 0711 »…), heures (« 13H35 »), références
/// d'ordre (« V07270038 »), noms de mois, numéros de bordereau et références
/// PayPal. `orig` est la date de l'opération modèle, `new` celle de la copie.
fn redate(text: &str, orig: Date, new: Date, time: (u32, u32), rng: &mut Rng) -> Redated {
    let tokens: Vec<&str> = text.split(' ').collect();
    let last_word = tokens.iter().rposition(|t| !t.is_empty());
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut lag_found: Option<i32> = None;
    let mut skip_next = false;
    for (i, &token) in tokens.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        let previous = out.last().map(|s| s.to_uppercase()).unwrap_or_default();
        let truncated = Some(i) == last_word || previous == "ECH";
        let event = |lag: i32| Date(new.0 - lag);
        let replaced = if token.is_empty() {
            None
        } else if let Some((format, lag)) = find_date_token(token, truncated, orig) {
            lag_found.get_or_insert(lag);
            let full = event(lag).to_naive().format(format).to_string();
            Some(full[..token.len().min(full.len())].to_string())
        } else if is_time_token(token) {
            Some(format!("{:02}H{:02}", time.0, time.1))
        } else if is_order_token(token) {
            let e = event(lag_found.unwrap_or(0)).to_naive();
            let full = format!("V{:02}{:02}{:02}{:02}", e.month(), e.day(), time.0, time.1);
            Some(full[..token.len()].to_string())
        } else if (previous == "BORDEREAU" || previous == "/EID")
            && token.chars().all(|c| c.is_ascii_digit())
        {
            Some(random_digits(rng, token.len()))
        } else if is_reference_token(token) {
            let keep = token.len() * 3 / 5;
            const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ0123456789";
            let tail: String = (keep..token.len())
                .map(|_| char::from(ALPHABET[rng.int(0, ALPHABET.len() as i64 - 1) as usize]))
                .collect();
            Some(format!("{}{tail}", &token[..keep]))
        } else {
            let upper = token.to_uppercase();
            let month = MONTHS
                .iter()
                .position(|m| *m == upper)
                .map(|m| (m, false))
                .or_else(|| {
                    MONTHS_SHORT
                        .iter()
                        .position(|m| *m == upper)
                        .map(|m| (m, true))
                });
            month.and_then(|(m, short)| {
                let lag =
                    (-3..=40).find(|&l| Date(orig.0 - l).to_naive().month0() as usize == m)?;
                let e = event(lag).to_naive();
                let name = if short { MONTHS_SHORT } else { MONTHS }[e.month0() as usize];
                let mut s = match_case(token, name);
                // Année qui suit le mois (« DECEMBRE 2010 », « NOV 20 »).
                if let Some(&next) = tokens.get(i + 1) {
                    let year_then = Date(orig.0 - lag).to_naive().year().to_string();
                    if !next.is_empty() && next.chars().all(|c| c.is_ascii_digit()) {
                        if next == year_then {
                            s = format!("{s} {}", e.year());
                            skip_next = true;
                        } else if Some(i + 1) == last_word && year_then.starts_with(next) {
                            s = format!("{s} {}", &e.year().to_string()[..next.len()]);
                            skip_next = true;
                        }
                    }
                }
                Some(s)
            })
        };
        out.push(replaced.unwrap_or_else(|| token.to_string()));
    }
    Redated {
        text: out.join(" "),
        lag: lag_found,
    }
}

/// Met à jour les montants cités dans un libellé (« GROUPON ALL 69,00EUR »,
/// « QOS ISR 35,00ILS », « USD000042,32 /FRAIS 0000,81 ») : le montant de
/// l'opération est remplacé, une devise étrangère suit la même proportion.
fn rescale_amounts(text: &str, old: f64, new: f64) -> String {
    if old == 0.0 || text.is_empty() {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let int_len = i - start;
        let is_amount = int_len > 0
            && (start == 0 || !chars[start - 1].is_ascii_digit())
            && chars.get(i) == Some(&',')
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
            && chars.get(i + 2).is_some_and(char::is_ascii_digit)
            && !chars.get(i + 3).is_some_and(char::is_ascii_digit);
        if is_amount {
            let literal: String = chars[start..i + 3].iter().collect();
            let value: f64 = literal.replace(',', ".").parse().unwrap_or(0.0);
            let scaled = if (value - old).abs() <= old * 0.01 + 0.01 {
                new
            } else {
                value * new / old
            };
            let cents = (scaled * 100.0).round() as i64;
            out += &format!(
                "{:0width$},{:02}",
                cents / 100,
                cents % 100,
                width = int_len
            );
            i += 3;
        } else if int_len > 0 {
            out.extend(&chars[start..i]);
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Numéro de chèque cité dans le tiers (« CHEQUE N° 7246477 », « CHEQUE N°8756813 »).
fn cheque_number_in(payee: &str) -> Option<(usize, usize, u64)> {
    let upper = payee.to_uppercase();
    let pos = upper.find("CHEQUE N")?;
    let start = payee[pos..]
        .find(|c: char| c.is_ascii_digit())
        .map(|p| p + pos)?;
    let end = payee[start..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(payee.len(), |p| p + start);
    Some((start, end, payee[start..end].parse().ok()?))
}

// ---------------------------------------------------------------------------
// Récurrences
// ---------------------------------------------------------------------------

enum AmountKind {
    /// Échéance de prêt : montant fixe.
    Constant(f64),
    /// Montant fixe révisé chaque année (abonnement, assurance, virement permanent).
    Revised {
        base: f64,
        base_year: i32,
        unit: f64,
    },
    /// Montant variable autour d'une médiane.
    Variable {
        median: f64,
        spread: f64,
        min: f64,
        max: f64,
        unit: f64,
        base_date: Date,
    },
}

struct Recurrence {
    account: usize,
    template: Operation,
    /// Période en mois.
    period: i32,
    day: u32,
    last_month: i32,
    kind: AmountKind,
    occurrences: usize,
}

/// Facteur de révision annuelle d'un montant fixe : identique pour deux
/// récurrences de même montant (les deux côtés d'un virement permanent
/// évoluent ensemble).
fn revision_factor(base: f64, base_year: i32, year: i32) -> f64 {
    let mut rng = Rng(((base.abs() * 100.0).round() as u64) ^ 0x5EED_BA5E);
    let mut factor = 1.0;
    for _ in base_year..year {
        factor *= 1.0 + rng.unit() * 0.035;
    }
    factor
}

impl Recurrence {
    fn amount(&self, date: Date, rng: &mut Rng) -> Amount {
        let value = match self.kind {
            AmountKind::Constant(v) => v,
            AmountKind::Revised {
                base,
                base_year,
                unit,
            } => {
                let year = date.to_naive().year();
                round_to(base * revision_factor(base, base_year, year), unit)
            }
            AmountKind::Variable {
                median,
                spread,
                min,
                max,
                unit,
                base_date,
            } => {
                let infl = inflation(base_date, date);
                let noise = 1.0 + spread * rng.normal().clamp(-2.0, 2.0);
                let v = (median * noise).clamp(min.min(max), max.max(min)) * infl;
                round_to(v, unit)
            }
        };
        to_amount(value)
    }

    fn describe(&self) -> String {
        match self.kind {
            AmountKind::Constant(v) => format!("{v:.2} fixe"),
            AmountKind::Revised { base, .. } => format!("{base:.2} révisé/an"),
            AmountKind::Variable { median, spread, .. } => {
                format!("~{median:.2} ±{:.0} %", spread * 100.0)
            }
        }
    }
}

fn is_card_like(mode: &Mode) -> bool {
    matches!(
        mode,
        Mode::Standard(StandardMode::Carte | StandardMode::RetraitDab) | Mode::Custom(_)
    )
}

/// Cherche une périodicité dans des occurrences triées par date. Renvoie la
/// période en mois. `alive` : la récurrence doit encore avoir cours à la fin
/// de l'historique ; sinon on reconnaît aussi les récurrences éteintes.
fn detect_period(ops: &[&Operation], start: Date, end: Date, alive: bool) -> Option<i32> {
    let last = ops.last()?.date;
    let end = if alive { end } else { Date(last.0.max(end.0)) };
    let recent_end = if alive { end } else { last };
    let card = ops.iter().any(|o| is_card_like(&o.mode));

    // Mensuelle : sur les 13 derniers mois de l'historique.
    let recent: Vec<&&Operation> = ops
        .iter()
        .filter(|o| o.date.0 >= recent_end.0 - 400)
        .collect();
    if recent.len() >= 2 && (!alive || last.0 >= end.0 - 50) {
        let mut months: BTreeMap<i32, usize> = BTreeMap::new();
        for o in &recent {
            *months.entry(month_index(o.date)).or_default() += 1;
        }
        let distinct = months.len();
        let first_month = *months.keys().next()?;
        let span = month_index(last) - first_month + 1;
        let per_month = recent.len() as f64 / distinct as f64;
        let mut days: Vec<f64> = recent.iter().map(|o| day_of_month(o.date) as f64).collect();
        let med = median(&mut days);
        let mut dev: Vec<f64> = days.iter().map(|d| (d - med).abs()).collect();
        let mad = median(&mut dev);
        let coverage = distinct as f64 / span as f64;
        // Compte ouvert depuis peu : deux mois consécutifs suffisent.
        let young = recent[0].date.0 - start.0 <= 75;
        if per_month <= 1.25
            && mad <= 3.0
            && coverage >= 0.7
            && (distinct >= 3 || (distinct == 2 && span == 2 && young))
        {
            return Some(1);
        }
    }
    if card || ops.len() < 2 {
        return None;
    }

    // Autres périodes : écarts entre occurrences successives.
    let intervals: Vec<i32> = ops.windows(2).map(|w| w[1].date.0 - w[0].date.0).collect();
    let mut sorted: Vec<f64> = intervals.iter().map(|&i| i as f64).collect();
    let med = median(&mut sorted);
    let recent_intervals = &intervals[intervals.len().saturating_sub(4)..];
    let check = |period: i32, lo: f64, hi: f64, all_lo: i32, all_hi: i32, min_count: usize| {
        ops.len() >= min_count
            && (lo..=hi).contains(&med)
            && recent_intervals
                .iter()
                .all(|&i| (all_lo..=all_hi).contains(&i))
            && (!alive || last.0 >= end.0 - (period * 30 + period * 15 + 10))
    };
    if check(2, 50.0, 72.0, 40, 85, 3) {
        Some(2)
    } else if check(3, 80.0, 100.0, 70, 115, 3) {
        Some(3)
    } else if check(12, 340.0, 390.0, 320, 410, 2) {
        Some(12)
    } else {
        None
    }
}

fn build_recurrence(
    account: usize,
    ops: &[&Operation],
    period: i32,
    doc: &Document,
    anchor_year: i32,
) -> Option<Recurrence> {
    // Modèle : la dernière occurrence qui n'est pas un transfert lié.
    let template = ops.iter().rev().find(|o| !is_transfer(o))?;
    let recent: Vec<&&Operation> = ops.iter().rev().take(12).collect();
    let mut days: Vec<f64> = recent.iter().map(|o| day_of_month(o.date) as f64).collect();
    let day = if period == 12 {
        day_of_month(template.date)
    } else {
        median(&mut days).round() as u32
    };
    let amounts: Vec<f64> = recent.iter().map(|o| euros(o.amount)).collect();
    let last = amounts[0];
    let fixed = amounts.len() >= 2
        && amounts[1] == last
        && amounts.iter().filter(|&&a| a == last).count() * 2 >= amounts.len();
    let category_name = template
        .category
        .and_then(|c| doc.category_label(c))
        .unwrap_or_default()
        .to_uppercase();
    let text = upper_text(template);
    let loan = ["PRET", "EMPRUNT", "FINANCEMEN", "ECHEANCE", "CREDIT"]
        .iter()
        .any(|k| text.contains(k))
        || category_name.contains("LOYER");
    let unit = amounts
        .iter()
        .map(|&a| natural_unit(to_amount(a)))
        .fold(10.0, f64::min);
    let kind = if fixed {
        let round = natural_unit(to_amount(last)) >= 10.0;
        if (loan || last.abs() >= 300.0) && !round {
            AmountKind::Constant(last)
        } else {
            AmountKind::Revised {
                base: last,
                base_year: anchor_year,
                unit: if last.abs() >= 1000.0 && round {
                    50.0
                } else {
                    natural_unit(to_amount(last))
                },
            }
        }
    } else {
        let mut sorted = amounts.clone();
        let med = median(&mut sorted);
        let mut dev: Vec<f64> = amounts.iter().map(|a| (a - med).abs()).collect();
        let mad = median(&mut dev);
        let spread = (1.4826 * mad / med.abs().max(0.01)).clamp(0.02, 0.3);
        let min = amounts.iter().copied().fold(f64::INFINITY, f64::min);
        let max = amounts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        AmountKind::Variable {
            median: med,
            spread,
            min,
            max,
            unit,
            base_date: template.date,
        }
    };
    Some(Recurrence {
        account,
        template: (*template).clone(),
        period,
        day: day.clamp(1, 31),
        last_month: month_index(ops.last()?.date),
        kind,
        occurrences: ops.len(),
    })
}

/// Détecte les récurrences d'un compte. Renvoie aussi les opérations
/// consommées (exclues ensuite du tirage au sort), y compris celles des
/// récurrences éteintes (ancien prêt…), qui ne doivent pas ressurgir au hasard.
/// Les montants fixes sont révisés à partir de `anchor_year`, la même année
/// pour tous les comptes.
fn detect_recurrences(
    account: usize,
    ops: &[Operation],
    doc: &Document,
    anchor_year: i32,
) -> (Vec<Recurrence>, Vec<bool>) {
    let mut used = vec![false; ops.len()];
    let mut result = Vec::new();
    let (Some(start), Some(end)) = (
        ops.iter().map(|o| o.date).min(),
        ops.iter().map(|o| o.date).max(),
    ) else {
        return (result, used);
    };
    let sign = |o: &Operation| if o.amount.is_negative() { '-' } else { '+' };
    // Quatre passes, de la plus précise à la plus large : libellé et
    // catégorie, libellé seul, catégorie, puis montant exact (échéance dont le
    // libellé a changé en cours de route).
    for pass in 0..4 {
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, op) in ops.iter().enumerate() {
            if used[i] || op.amount == Amount::ZERO {
                continue;
            }
            let key = match pass {
                0 => format!("{}|{:?}|{}", label_key(op), op.category, sign(op)),
                1 => format!("{}|{}", label_key(op), sign(op)),
                2 => {
                    let Some(c) = op.category else { continue };
                    if is_card_like(&op.mode) {
                        continue;
                    }
                    format!("cat {c}|{}|{}", sign(op), op.mode.label())
                }
                _ => {
                    if is_card_like(&op.mode) || natural_unit(op.amount) >= 10.0 {
                        continue;
                    }
                    format!("montant {}|{}", op.amount.0, op.mode.label())
                }
            };
            if pass < 2 && label_key(op).is_empty() {
                continue;
            }
            groups.entry(key).or_default().push(i);
        }
        for mut members in groups.into_values() {
            members.sort_by_key(|&i| (ops[i].date, ops[i].uid));
            // D'abord les sous-groupes de montant identique (deux prêts le même
            // jour chez le même tiers, virement permanent parmi des virements
            // ponctuels…), puis le reste.
            let mut by_amount: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
            for &i in &members {
                by_amount.entry(ops[i].amount.0).or_default().push(i);
            }
            let mut candidates: Vec<Vec<usize>> =
                by_amount.into_values().filter(|v| v.len() >= 2).collect();
            candidates.push(members.clone());
            let mut found = false;
            for candidate in candidates {
                let remaining: Vec<usize> = candidate.into_iter().filter(|&i| !used[i]).collect();
                let list: Vec<&Operation> = remaining.iter().map(|&i| &ops[i]).collect();
                if let Some(period) = detect_period(&list, start, end, true)
                    && let Some(rec) = build_recurrence(account, &list, period, doc, anchor_year)
                {
                    for &i in &remaining {
                        used[i] = true;
                    }
                    result.push(rec);
                    found = true;
                } else if list.len() >= 4 && detect_period(&list, start, end, false).is_some() {
                    for &i in &remaining {
                        used[i] = true;
                    }
                }
            }
            // Les variantes d'un même libellé récurrent (montant exceptionnel
            // d'un virement permanent…) ne sont pas rejouées au hasard.
            if found && pass < 2 {
                for &i in &members {
                    used[i] = true;
                }
            }
        }
    }
    (result, used)
}

// ---------------------------------------------------------------------------
// Tirage des opérations courantes
// ---------------------------------------------------------------------------

struct Pool {
    /// Indices des modèles dans l'historique du compte.
    templates: Vec<usize>,
    /// Poids cumulés par mois calendaire cible (saisonnalité).
    cumulative: [Vec<f64>; 12],
    /// Nombre moyen d'opérations courantes par jour, récemment.
    rate_per_day: f64,
}

fn build_pool(ops: &[Operation], used: &[bool], doc: &Document) -> Pool {
    let end = ops.iter().map(|o| o.date).max().unwrap_or(Date(0));
    let templates: Vec<usize> = (0..ops.len())
        .filter(|&i| {
            let op = &ops[i];
            !used[i]
                && op.date.0 > end.0 - 365
                && op.amount != Amount::ZERO
                && euros(op.amount).abs() <= MAX_RANDOM_AMOUNT
                && !is_internal_move(doc, op)
                && !is_transfer(op)
        })
        .collect();
    // Une grosse opération ponctuelle (libellé vu une seule fois) pèse moins
    // qu'une habitude : sur un historique court, les frais d'ouverture ne
    // doivent pas revenir tous les mois.
    let mut seen: HashMap<String, usize> = HashMap::new();
    for op in ops {
        *seen.entry(label_key(op)).or_default() += 1;
    }
    let habit = |op: &Operation| match (seen[&label_key(op)], euros(op.amount).abs()) {
        (n, _) if n >= 2 => 1.0,
        (_, a) if a >= 300.0 => 0.15,
        _ => 1.0,
    };
    // Fréquence pondérée exponentiellement (demi-vie 30 jours) : reflète le
    // rythme de la fin de l'historique.
    let half_life = 30.0;
    let weight = |age: i32| 0.5f64.powf(age as f64 / half_life);
    let numerator: f64 = templates
        .iter()
        .map(|&i| weight(end.0 - ops[i].date.0) * habit(&ops[i]))
        .sum();
    let denominator: f64 = (0..365).map(weight).sum();
    let cumulative = std::array::from_fn(|m| {
        let mut acc = 0.0;
        templates
            .iter()
            .map(|&i| {
                let op = &ops[i];
                let recency = 0.5f64.powf((end.0 - op.date.0) as f64 / 180.0);
                let month = op.date.to_naive().month0() as i32;
                let distance = (month - m as i32)
                    .rem_euclid(12)
                    .min((m as i32 - month).rem_euclid(12));
                let season = match distance {
                    0 => 3.0,
                    1 => 1.6,
                    _ => 1.0,
                };
                acc += recency * season * habit(op);
                acc
            })
            .collect()
    });
    Pool {
        templates,
        cumulative,
        rate_per_day: numerator / denominator,
    }
}

/// Facteur saisonnier du nombre d'opérations, par mois calendaire, d'après les
/// années complètes de tous les comptes actifs.
fn seasonal_factors(doc: &Document, accounts: &[usize]) -> [f64; 12] {
    let mut ratios: [Vec<f64>; 12] = Default::default();
    for &a in accounts {
        let mut counts: BTreeMap<i32, f64> = BTreeMap::new();
        for op in &doc.accounts[a].operations {
            *counts.entry(month_index(op.date)).or_default() += 1.0;
        }
        let mut by_year: BTreeMap<i32, Vec<(usize, f64)>> = BTreeMap::new();
        for (&mi, &n) in &counts {
            by_year
                .entry(mi.div_euclid(12))
                .or_default()
                .push((mi.rem_euclid(12) as usize, n));
        }
        for months in by_year.values().filter(|v| v.len() == 12) {
            let mean = months.iter().map(|m| m.1).sum::<f64>() / 12.0;
            let min = months.iter().map(|m| m.1).fold(f64::INFINITY, f64::min);
            // Année représentative seulement (ni ouverture ni fermeture du compte).
            if mean >= 5.0 && min >= 0.4 * mean {
                for &(m, n) in months {
                    ratios[m].push(n / mean);
                }
            }
        }
    }
    std::array::from_fn(|m| {
        let r = &ratios[m];
        if r.is_empty() {
            1.0
        } else {
            (r.iter().sum::<f64>() / r.len() as f64).clamp(0.8, 1.35)
        }
    })
}

/// Montant d'une copie d'opération courante : inflation et petit aléa, en
/// gardant les montants ronds ronds (retraits, chèques de 50 €…).
fn random_amount(template: &Operation, date: Date, rng: &mut Rng) -> Amount {
    let base = euros(template.amount);
    let infl = inflation(template.date, date);
    let unit = natural_unit(template.amount);
    let value = if unit >= 10.0 {
        round_to(base * infl, 10.0)
    } else if unit >= 1.0 {
        round_to(
            base * infl * (1.0 + 0.05 * rng.normal().clamp(-2.0, 2.0)),
            1.0,
        )
    } else {
        round_to(
            base * infl * (0.12 * rng.normal().clamp(-2.5, 2.5)).exp(),
            0.01,
        )
    };
    to_amount(value)
}

// ---------------------------------------------------------------------------
// Équilibrage du solde
// ---------------------------------------------------------------------------

enum Flow {
    /// Virement d'épargne, sur le modèle d'une opération existante.
    Savings(Operation),
    /// Transfert de compte à compte avec un autre compte du fichier.
    Internal { partner: usize, details: String },
}

struct Controller {
    low: f64,
    center: f64,
    high: f64,
    inflow: Option<Flow>,
    outflow: Option<Flow>,
}

fn build_controller(doc: &Document, a: usize, active: &[usize]) -> Controller {
    let ops = &doc.accounts[a].operations;
    let mut sorted: Vec<&Operation> = ops.iter().collect();
    sorted.sort_by_key(|o| (o.date, o.uid));
    let end = sorted.last().map_or(Date(0), |o| o.date);
    // Solde minimal de chaque mois sur les six derniers mois de l'historique :
    // c'est la réserve que l'utilisateur laissait sur le compte.
    let mut balance = 0.0;
    let mut monthly_min: BTreeMap<i32, f64> = BTreeMap::new();
    for op in &sorted {
        balance += euros(op.amount);
        if op.date.0 >= end.0 - 180 {
            let m = monthly_min.entry(month_index(op.date)).or_insert(balance);
            *m = m.min(balance);
        }
    }
    let mut mins: Vec<f64> = monthly_min.into_values().collect();
    mins.sort_by(f64::total_cmp);
    let center = percentile(&mins, 0.5).clamp(500.0, 5000.0);
    let low = 300.0;
    let high = center + (center * 0.5).max(1000.0);

    let latest = |pred: &dyn Fn(&Operation) -> bool| -> Option<Operation> {
        sorted.iter().rev().find(|o| pred(o)).map(|o| (*o).clone())
    };
    let savings_out = latest(&|o| {
        let t = upper_text(o);
        o.amount.is_negative()
            && (t.contains("EPARGNE") || t.contains("EPRAGNE"))
            && !t.contains("DESEPARGNE")
            && !is_transfer(o)
    });
    let savings_in = latest(&|o| {
        !o.amount.is_negative() && upper_text(o).contains("EPARGNE") && !is_transfer(o)
    });
    // Sinon : transferts avec un autre compte actif, comme dans l'historique.
    let internal = latest(&|o| is_transfer(o)).and_then(|o| {
        let name = o
            .payee
            .strip_prefix(TRANSFER_TO)
            .or_else(|| o.payee.strip_prefix(TRANSFER_FROM))?;
        let partner = active
            .iter()
            .copied()
            .find(|&p| p != a && doc.accounts[p].name == name)?;
        Some(Flow::Internal {
            partner,
            details: o.details.trim().to_string(),
        })
    });
    let clone_internal = |f: &Option<Flow>| match f {
        Some(Flow::Internal { partner, details }) => Some(Flow::Internal {
            partner: *partner,
            details: details.clone(),
        }),
        _ => None,
    };
    let (inflow, outflow) = match (savings_in, savings_out) {
        (Some(i), Some(o)) => (Some(Flow::Savings(i)), Some(Flow::Savings(o))),
        (i, o) => (
            i.map(Flow::Savings).or_else(|| clone_internal(&internal)),
            o.map(Flow::Savings).or_else(|| clone_internal(&internal)),
        ),
    };
    Controller {
        low,
        center,
        high,
        inflow,
        outflow,
    }
}

// ---------------------------------------------------------------------------
// Génération
// ---------------------------------------------------------------------------

struct NewOp {
    account: usize,
    op: Operation,
    /// Indice de la ligne sœur (transfert de compte à compte).
    sister: Option<usize>,
}

struct Generator {
    rng: Rng,
    new_ops: Vec<NewOp>,
    /// Prochain numéro de chèque par compte.
    cheques: HashMap<usize, u64>,
}

impl Generator {
    fn random_time(&mut self) -> (u32, u32) {
        (self.rng.int(7, 23) as u32, self.rng.int(0, 59) as u32)
    }

    /// Copie redatée d'une opération modèle. `business` : date reportée au
    /// jour ouvré (opérations passées par la banque).
    fn instantiate(
        &mut self,
        account: usize,
        template: &Operation,
        date: Date,
        amount: Amount,
        business: bool,
    ) -> Operation {
        let time = self.random_time();
        let payee = redate(&template.payee, template.date, date, time, &mut self.rng);
        let details = redate(&template.details, template.date, date, time, &mut self.rng);
        let (old, new) = (euros(template.amount).abs(), euros(amount).abs());
        let mut payee_text = rescale_amounts(&payee.text, old, new);
        let details_text = rescale_amounts(&details.text, old, new);
        if template.mode == Mode::Standard(StandardMode::ChequeEmis)
            && let Some((start, end, _)) = cheque_number_in(&payee_text)
            && let Some(next) = self.cheques.get_mut(&account)
        {
            payee_text = format!("{}{}{}", &payee_text[..start], next, &payee_text[end..]);
            *next += 1;
        }
        // Libellé daté par la banque (carte débitée le lendemain, retrait…) :
        // la date de l'opération tombe un jour ouvré.
        let date = if business || payee.lag.is_some_and(|l| l > 0) {
            business_day(date)
        } else {
            date
        };
        Operation {
            uid: 0,
            date,
            mark_date: date,
            tack: Tack::None,
            mode: template.mode.clone(),
            payee: payee_text,
            details: details_text,
            category: template.category,
            amount,
            extra: Vec::new(),
        }
    }

    fn push(&mut self, account: usize, op: Operation) -> usize {
        self.new_ops.push(NewOp {
            account,
            op,
            sister: None,
        });
        self.new_ops.len() - 1
    }

    /// Mouvement d'équilibrage : `amount` positif = entrée dans `account`.
    fn flow(&mut self, doc: &Document, account: usize, flow: &Flow, date: Date, amount: f64) {
        match flow {
            Flow::Savings(template) => {
                let op = self.instantiate(account, template, date, to_amount(amount), true);
                self.push(account, op);
            }
            Flow::Internal { partner, details } => {
                let date = previous_business_day(date);
                let make =
                    |to_name: &str, prefix: &str, mode: StandardMode, value: f64| Operation {
                        uid: 0,
                        date,
                        mark_date: date,
                        tack: Tack::None,
                        mode: Mode::Standard(mode),
                        payee: format!("{prefix}{to_name}"),
                        details: details.clone(),
                        category: None,
                        amount: to_amount(value),
                        extra: Vec::new(),
                    };
                let (mine, theirs) = if amount > 0.0 {
                    (
                        make(
                            &doc.accounts[*partner].name,
                            TRANSFER_FROM,
                            StandardMode::VirementRecu,
                            amount,
                        ),
                        make(
                            &doc.accounts[account].name,
                            TRANSFER_TO,
                            StandardMode::VirementEmis,
                            -amount,
                        ),
                    )
                } else {
                    (
                        make(
                            &doc.accounts[*partner].name,
                            TRANSFER_TO,
                            StandardMode::VirementEmis,
                            amount,
                        ),
                        make(
                            &doc.accounts[account].name,
                            TRANSFER_FROM,
                            StandardMode::VirementRecu,
                            -amount,
                        ),
                    )
                };
                let i = self.push(account, mine);
                let j = self.push(*partner, theirs);
                self.new_ops[i].sister = Some(j);
                self.new_ops[j].sister = Some(i);
            }
        }
    }

    fn balance_before(&self, doc: &Document, account: usize, date: Date) -> f64 {
        let hist: f64 = doc.accounts[account]
            .operations
            .iter()
            .filter(|o| o.date < date)
            .map(|o| euros(o.amount))
            .sum();
        hist + self
            .new_ops
            .iter()
            .filter(|n| n.account == account && n.op.date < date)
            .map(|n| euros(n.op.amount))
            .sum::<f64>()
    }

    /// Solde minimal sur `[from, to]` et premier jour où le solde passe sous
    /// `threshold`.
    fn balance_path(
        &self,
        doc: &Document,
        account: usize,
        from: Date,
        to: Date,
        threshold: f64,
    ) -> (f64, Option<Date>) {
        let mut daily: BTreeMap<i32, f64> = BTreeMap::new();
        for n in self.new_ops.iter().filter(|n| n.account == account) {
            if n.op.date >= from && n.op.date <= to {
                *daily.entry(n.op.date.0).or_default() += euros(n.op.amount);
            }
        }
        let mut balance = self.balance_before(doc, account, from);
        let mut min = balance;
        let mut first_below = (balance < threshold).then_some(from);
        for (&d, &delta) in &daily {
            balance += delta;
            min = min.min(balance);
            if balance < threshold && first_below.is_none() {
                first_below = Some(Date(d));
            }
        }
        (min, first_below)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage : extend_sample <entrée.bp> <sortie.bp> [graine]");
        std::process::exit(2);
    }
    let seed: u64 = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_111_027);
    let opened = bp_core::file::open(args[1].as_ref(), None).expect("lecture du fichier d'entrée");
    let mut doc = opened.document;
    let today = Date::today();
    let horizon = Date(today.0 + HORIZON_DAYS);

    let active: Vec<usize> = (0..doc.accounts.len())
        .filter(|&a| {
            let acc = &doc.accounts[a];
            !acc.disabled && !acc.counts_as_virtual() && !acc.operations.is_empty()
        })
        .collect();
    let seasons = seasonal_factors(&doc, &active);
    // Année à partir de laquelle les montants fixes sont révisés.
    let anchor_year = active
        .iter()
        .filter_map(|&a| doc.accounts[a].operations.iter().map(|o| o.date).max())
        .max()
        .map_or(today.to_naive().year(), |d| d.to_naive().year());
    println!(
        "saisonnalité : {}",
        seasons
            .iter()
            .map(|s| format!("{s:.2}"))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let mut generator = Generator {
        rng: Rng(seed),
        new_ops: Vec::new(),
        cheques: HashMap::new(),
    };
    let mut controllers: HashMap<usize, Controller> = HashMap::new();
    let mut ends: HashMap<usize, Date> = HashMap::new();
    let mut recurrence_count = 0;

    for &a in &active {
        let mut ops = doc.accounts[a].operations.clone();
        ops.sort_by_key(|o| (o.date, o.uid));
        let end = ops.last().expect("compte non vide").date;
        ends.insert(a, end);
        let (recurrences, used) = detect_recurrences(a, &ops, &doc, anchor_year);
        let pool = build_pool(&ops, &used, &doc);
        let controller = build_controller(&doc, a, &active);

        // Chéquier : suite du dernier chèque émis.
        if let Some(last) = ops
            .iter()
            .rev()
            .filter(|o| o.mode == Mode::Standard(StandardMode::ChequeEmis))
            .find_map(|o| cheque_number_in(&o.payee).map(|c| c.2))
        {
            let max = ops
                .iter()
                .filter_map(|o| cheque_number_in(&o.payee).map(|c| c.2))
                .filter(|&n| n.abs_diff(last) < 1000)
                .max()
                .unwrap_or(last);
            generator.cheques.insert(a, max + 1);
        }

        println!(
            "\n== {} : historique jusqu'au {end}, {:.1} opérations courantes/mois ({} modèles), solde visé {:.0} [{:.0} ; {:.0}]",
            doc.accounts[a].name,
            pool.rate_per_day * 30.4,
            pool.templates.len(),
            controller.center,
            controller.low,
            controller.high
        );
        for rec in &recurrences {
            let period = match rec.period {
                1 => "mensuel".to_string(),
                2 => "bimestriel".to_string(),
                3 => "trimestriel".to_string(),
                12 => "annuel".to_string(),
                p => format!("{p} mois"),
            };
            println!(
                "   récurrent {period:<11} le {:>2} : {:<55} {:<30} {} ({} occ.)",
                rec.day,
                rec.template.payee.chars().take(55).collect::<String>(),
                rec.template
                    .category
                    .and_then(|c| doc.category_label(c))
                    .unwrap_or_default(),
                rec.describe(),
                rec.occurrences
            );
        }
        recurrence_count += recurrences.len();

        // Récurrences, jusqu'à l'horizon (opérations à venir comprises).
        for rec in &recurrences {
            let mut k = 1;
            loop {
                let mi = rec.last_month + k * rec.period;
                k += 1;
                // Petit flottement du jour pour les montants variables.
                let jitter = match rec.kind {
                    AmountKind::Variable { .. } if generator.rng.chance(0.3) => {
                        generator.rng.int(-1, 1)
                    }
                    _ => 0,
                };
                let raw = day_in_month(mi, (rec.day as i64 + jitter).clamp(1, 31) as u32);
                if raw > horizon {
                    break;
                }
                let business = !is_card_like(&rec.template.mode);
                let amount = rec.amount(raw, &mut generator.rng);
                let op = generator.instantiate(rec.account, &rec.template, raw, amount, business);
                if op.date > end && op.date <= horizon {
                    generator.push(a, op);
                }
            }
        }

        // Opérations courantes, jusqu'à aujourd'hui.
        if !pool.templates.is_empty() {
            let mut mi = month_index(Date(end.0 + 1));
            while month_start(mi) <= today {
                let from = Date(month_start(mi).0.max(end.0 + 1));
                let to = Date(month_end(mi).0.min(today.0));
                let days = (to.0 - from.0 + 1).max(0) as f64;
                let month0 = mi.rem_euclid(12) as usize;
                let count = generator
                    .rng
                    .poisson(pool.rate_per_day * days * seasons[month0]);
                for _ in 0..count {
                    let t = pool.templates[generator.rng.weighted(&pool.cumulative[month0])];
                    let template = &ops[t];
                    let date = Date(generator.rng.int(from.0 as i64, to.0 as i64) as i32);
                    let amount = random_amount(template, date, &mut generator.rng);
                    let business = matches!(
                        template.mode,
                        Mode::Standard(
                            StandardMode::Prelevement
                                | StandardMode::VirementEmis
                                | StandardMode::VirementRecu
                                | StandardMode::DepotCheque
                                | StandardMode::Tip
                        )
                    );
                    let op = generator.instantiate(a, template, date, amount, business);
                    if op.date > end && op.date <= today {
                        generator.push(a, op);
                    }
                }
                mi += 1;
            }
        }
        controllers.insert(a, controller);
    }

    // Équilibrage mois par mois ; d'abord les comptes alimentés par transfert
    // depuis un autre compte du fichier (le compte source est traité ensuite).
    let mut order = active.clone();
    order.sort_by_key(|a| {
        let c = &controllers[a];
        !matches!(
            (&c.inflow, &c.outflow),
            (Some(Flow::Internal { .. }), _) | (_, Some(Flow::Internal { .. }))
        )
    });
    let first_month = active
        .iter()
        .map(|a| month_index(Date(ends[a].0 + 1)))
        .min();
    if let Some(first_month) = first_month {
        for mi in first_month..=month_index(today) {
            for &a in &order {
                let start = Date(month_start(mi).0.max(ends[&a].0 + 1));
                if start > month_end(mi) {
                    continue;
                }
                let current = mi == month_index(today);
                let until = if current { horizon } else { month_end(mi) };
                let c = &controllers[&a];
                // Renflouement juste avant le premier passage sous le plancher,
                // d'un montant couvrant le creux du mois.
                for _ in 0..3 {
                    let (min, first_below) = generator.balance_path(&doc, a, start, until, c.low);
                    let (Some(flow), Some(below)) = (c.inflow.as_ref(), first_below) else {
                        break;
                    };
                    let amount = ((c.center - min) / 100.0).ceil() * 100.0;
                    let mut date = Date((below.0 - generator.rng.int(0, 2) as i32).max(start.0));
                    while is_weekend(date) && date.0 > start.0 {
                        date = Date(date.0 - 1);
                    }
                    let date = Date(date.0.min(today.0));
                    generator.flow(&doc, a, flow, date, amount);
                }
                if !current && let Some(flow) = &c.outflow {
                    // Fin de mois : l'argent qui ne servira pas d'ici la fin du
                    // mois suivant (solde minimal à venir) part en épargne.
                    let date = previous_business_day(Date(
                        month_end(mi).0 - generator.rng.int(0, 3) as i32,
                    ));
                    let date = Date(date.0.max(start.0));
                    let ahead = Date(month_end(mi + 1).0.min(horizon.0));
                    let (min, _) = generator.balance_path(&doc, a, date, ahead, c.low);
                    if min > c.high {
                        let amount = ((min - c.center) / 100.0).floor() * 100.0;
                        generator.flow(&doc, a, flow, date, -amount);
                    }
                }
            }
        }
    }

    // Pointage, identifiants, liens des transferts.
    let reconciled_before = Date(today.0 - RECENT_DAYS);
    let mut rng = Rng(seed ^ 0xC0FFEE);
    let statement_date = |date: Date, rng: &mut Rng| {
        let next = month_index(date) + 1;
        let d = business_day(day_in_month(next, 10 + rng.int(0, 3) as u32));
        Date(d.0.min(today.0))
    };
    let point = |op: &mut Operation, rng: &mut Rng| {
        if op.date > today {
            op.tack = Tack::None;
            op.mark_date = op.date;
        } else if op.date < reconciled_before {
            op.tack = Tack::Reconciled;
            op.mark_date = statement_date(op.date, rng);
        } else {
            // Crédits, virements et prélèvements apparaissent vite sur le
            // relevé : pointés après deux jours. Cartes, retraits et chèques
            // restent plus souvent à pointer (d'autant plus qu'ils sont
            // récents) : le solde en banque reste réaliste.
            let days = today.0 - op.date.0;
            let age = days as f64 / RECENT_DAYS as f64;
            let quick = !op.amount.is_negative()
                || is_transfer(op)
                || matches!(
                    op.mode,
                    Mode::Standard(
                        StandardMode::Prelevement | StandardMode::VirementEmis | StandardMode::Tip
                    )
                );
            if (quick && days >= 2) || (!quick && rng.chance(0.15 + 0.8 * age)) {
                op.tack = Tack::Pointed;
                op.mark_date = Date((op.date.0 + rng.int(1, 4) as i32).min(today.0));
            } else {
                op.tack = Tack::None;
                op.mark_date = op.date;
            }
        }
    };
    // L'historique ancien des comptes actifs est rapproché lui aussi.
    for &a in &active {
        for op in &mut doc.accounts[a].operations {
            if op.tack != Tack::Reconciled && op.date < reconciled_before {
                point(op, &mut rng);
            }
        }
    }
    let mut order: Vec<usize> = (0..generator.new_ops.len()).collect();
    // Dans une même journée, les crédits avant les débits (le solde affiché
    // ligne à ligne ne plonge pas artificiellement).
    order.sort_by_key(|&i| {
        let n = &generator.new_ops[i];
        (n.op.date, n.account, n.op.amount.is_negative(), i)
    });
    for (uid, &i) in (doc.next_operation_uid()..).zip(&order) {
        generator.new_ops[i].op.uid = uid;
    }
    for i in 0..generator.new_ops.len() {
        point(&mut generator.new_ops[i].op, &mut rng);
        if let Some(j) = generator.new_ops[i].sister {
            let sister_account = doc.accounts[generator.new_ops[j].account].uid.clone();
            let sister_line = generator.new_ops[j].op.uid.to_string();
            // Les deux lignes d'un transfert ont le même pointage.
            if i > j {
                let (tack, mark) = (
                    generator.new_ops[j].op.tack,
                    generator.new_ops[j].op.mark_date,
                );
                generator.new_ops[i].op.tack = tack;
                generator.new_ops[i].op.mark_date = mark;
            }
            let op = &mut generator.new_ops[i].op;
            op.extra
                .push((SISTER_ACCOUNT_KEY.to_string(), sister_account));
            op.extra.push((SISTER_LINE_KEY.to_string(), sister_line));
        }
    }
    let mut added: HashMap<usize, usize> = HashMap::new();
    for &i in &order {
        let n = &generator.new_ops[i];
        *added.entry(n.account).or_default() += 1;
        doc.accounts[n.account].operations.push(n.op.clone());
    }

    bp_core::file::save(
        args[2].as_ref(),
        &doc,
        &Protection::none().expect("protection"),
    )
    .expect("écriture du fichier de sortie");

    // Vérification : relecture et résumé.
    let reopened = bp_core::file::open(args[2].as_ref(), None).expect("relecture du fichier écrit");
    let doc = reopened.document;
    println!(
        "\n{} récurrences reproduites ; fichier relu : {} ({} comptes)",
        recurrence_count,
        args[2],
        doc.accounts.len()
    );
    println!(
        "{:<14} {:>8} {:>12} {:>14} {:>12} {:>7} {:>10} {:>8}",
        "compte",
        "ajoutées",
        "en banque",
        "solde du jour",
        "solde final",
        "pointé",
        "non pointé",
        "à venir"
    );
    for &a in &active {
        let acc = &doc.accounts[a];
        let past = || acc.operations.iter().filter(|o| o.date <= today);
        let today_balance: Amount = past().map(|o| o.amount).sum();
        let pointed = past().filter(|o| o.tack == Tack::Pointed).count();
        let unpointed = past().filter(|o| o.tack == Tack::None).count();
        let future = acc.operations.iter().filter(|o| o.date > today).count();
        let in_bank: Amount = acc
            .operations
            .iter()
            .filter(|o| o.tack != Tack::None)
            .map(|o| o.amount)
            .sum();
        println!(
            "{:<14} {:>8} {:>12} {:>14} {:>12} {:>7} {:>10} {:>8}",
            acc.name,
            added.get(&a).copied().unwrap_or(0),
            in_bank.format(2, ','),
            today_balance.format(2, ','),
            acc.balance().format(2, ','),
            pointed,
            unpointed,
            future
        );
    }
    // Export complet (date, compte, solde…) si EXTEND_SAMPLE_DUMP est défini.
    if let Ok(path) = std::env::var("EXTEND_SAMPLE_DUMP") {
        let mut out = String::new();
        for (a, acc) in doc.accounts.iter().enumerate() {
            let mut ops: Vec<&Operation> = acc.operations.iter().collect();
            ops.sort_by_key(|o| (o.date, o.uid));
            let mut balance = Amount::ZERO;
            for op in ops {
                balance = balance + op.amount;
                out += &format!(
                    "{a}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    op.uid,
                    op.date,
                    op.tack,
                    op.mode_label(),
                    op.payee,
                    op.details,
                    op.category
                        .and_then(|c| doc.category_label(c))
                        .unwrap_or_default(),
                    op.amount.format(2, ','),
                    balance.format(2, ',')
                );
            }
        }
        std::fs::write(path, out).expect("écriture de l'export");
    }

    // Trajectoire du solde : fin d'année, tous les trois ans, et minimum.
    for &a in &active {
        let acc = &doc.accounts[a];
        let mut ops: Vec<&Operation> = acc.operations.iter().collect();
        ops.sort_by_key(|o| (o.date, o.uid));
        let start = ends[&a];
        let mut balance = Amount::ZERO;
        let mut min = (Amount(i64::MAX), start);
        let mut points = Vec::new();
        let mut next_year = start.to_naive().year();
        for op in &ops {
            while op.date.to_naive().year() > next_year {
                if (next_year - start.to_naive().year()) % 3 == 0 {
                    points.push(format!("{next_year} : {}", balance.format(0, ',')));
                }
                next_year += 1;
            }
            balance = balance + op.amount;
            if op.date > start && op.date <= today && balance < min.0 {
                min = (balance, op.date);
            }
        }
        println!(
            "   {:<12} fin {} ; minimum {} le {}",
            acc.name,
            points.join(", "),
            min.0.format(2, ','),
            min.1
        );
    }

    // Aperçu : les dernières opérations de chaque compte, à venir comprises.
    for &a in &active {
        let acc = &doc.accounts[a];
        let mut ops: Vec<&Operation> = acc.operations.iter().collect();
        ops.sort_by_key(|o| (o.date, o.uid));
        println!("\n-- {} : dernières opérations", acc.name);
        for op in &ops[ops.len().saturating_sub(18)..] {
            println!(
                "   {} {:<10} {:<14} {:<48} {:<28} {:>9}",
                op.date,
                format!("{:?}", op.tack),
                op.mode_label(),
                op.payee.chars().take(48).collect::<String>(),
                op.category
                    .and_then(|c| doc.category_label(c))
                    .unwrap_or_default()
                    .chars()
                    .take(28)
                    .collect::<String>(),
                op.amount.format(2, ',')
            );
        }
    }
    let mut all: Vec<u64> = doc
        .accounts
        .iter()
        .flat_map(|a| a.operations.iter().map(|o| o.uid))
        .collect();
    let total = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), total, "identifiants d'opérations uniques");
}
