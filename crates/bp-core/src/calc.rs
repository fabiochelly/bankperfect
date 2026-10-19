//! Outils de calcul autonomes (ex-scripts `loan.py`, `numerals.py`,
//! `receipt.py`, `luhn.py` et `rib.py`, et `clef_rib` de `UnitAccounts.pas`).
//!
//! - [`Loan`] : simulateur de crédit (calcul de l'inconnue parmi capital, taux,
//!   durée et mensualité ; tableau d'amortissement ; coût total) ;
//! - [`number_to_words`], [`amount_to_words`] : nombres et montants en toutes
//!   lettres, en français ;
//! - [`to_roman`], [`from_roman`] : chiffres romains ;
//! - [`luhn_is_valid`], [`check_card_number`] : clé de Luhn des cartes ;
//! - [`rib_key`], [`iban_is_valid`], [`iban_from_bban`] : RIB et IBAN.

use crate::model::Amount;
use crate::tr;

// ---------------------------------------------------------------------------
// Simulateur de crédit
// ---------------------------------------------------------------------------

/// Valeur à calculer à partir des trois autres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoanUnknown {
    /// Capital emprunté.
    Capital,
    /// Taux annuel.
    Rate,
    /// Nombre de mensualités.
    Months,
    /// Montant de la mensualité.
    Payment,
}

/// Paramètres d'un crédit à mensualités constantes.
///
/// Les montants sont des flottants : c'est un outil de simulation, détaché des
/// comptes (l'original travaillait aussi en flottants, affichés avec quatre
/// décimales).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loan {
    /// Capital emprunté.
    pub capital: f64,
    /// Taux nominal annuel en pourcentage (ex. `3.5` pour 3,5 %) ; le taux
    /// mensuel est le douzième.
    pub annual_rate: f64,
    /// Nombre de mensualités (peut être fractionnaire après un calcul de durée).
    pub months: f64,
    /// Mensualité.
    pub payment: f64,
    /// Valeur résiduelle restant due après la dernière mensualité (location
    /// avec option d'achat, crédit « ballon ») ; 0 pour un crédit classique.
    pub residual: f64,
    /// Mensualités versées en début de période (« début de période »).
    pub start_of_period: bool,
}

/// Échec d'un calcul de crédit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LoanError {
    Impossible,
    NoConvergence,
}

impl std::fmt::Display for LoanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Impossible => tr!("les valeurs saisies ne permettent pas ce calcul"),
            Self::NoConvergence => tr!("le calcul du taux ne converge pas"),
        })
    }
}

/// Coût d'un crédit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoanCost {
    /// Total versé : mensualités et valeur résiduelle.
    pub total_paid: f64,
    /// Part des intérêts (`total_paid - capital`).
    pub interest: f64,
    /// Part du capital dans le total, en pourcentage.
    pub capital_percent: f64,
    /// Part des intérêts dans le total, en pourcentage.
    pub interest_percent: f64,
}

/// Ligne du tableau d'amortissement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmortizationRow {
    /// Numéro de l'échéance, à partir de 1.
    pub period: u32,
    /// Année et mois de l'échéance.
    pub year: i32,
    pub month: u32,
    /// Mensualité versée (la dernière peut différer si la durée n'est pas
    /// entière).
    pub payment: f64,
    /// Capital remboursé.
    pub principal: f64,
    /// Intérêts remboursés.
    pub interest: f64,
    /// Capital restant dû après l'échéance.
    pub remaining: f64,
}

const EPSILON: f64 = 1e-9;

/// Nombre maximal d'échéances d'un tableau d'amortissement.
const MAX_MONTHS: f64 = 1200.0;

impl Loan {
    /// Taux mensuel (fraction).
    pub fn monthly_rate(&self) -> f64 {
        self.annual_rate / 1200.0
    }

    /// Facteur appliqué aux mensualités : `1 + i` si elles sont versées en
    /// début de période.
    fn due_factor(&self, i: f64) -> f64 {
        if self.start_of_period { 1.0 + i } else { 1.0 }
    }

    /// Écart entre le capital et la valeur actuelle des versements, pour un
    /// taux mensuel `i` : nul quand les quatre valeurs sont cohérentes.
    ///
    /// `capital = mensualité × f × (1 − (1+i)^−n) / i + résiduel × (1+i)^−n`.
    fn balance_gap(&self, i: f64) -> f64 {
        let n = self.months;
        if i.abs() < EPSILON {
            return self.payment * n + self.residual - self.capital;
        }
        let v = (1.0 + i).powf(-n);
        self.payment * self.due_factor(i) * (1.0 - v) / i + self.residual * v - self.capital
    }

    /// Calcule `unknown` à partir des trois autres valeurs et renvoie le crédit
    /// complété.
    pub fn solve(&self, unknown: LoanUnknown) -> Result<Loan, LoanError> {
        let mut loan = *self;
        let i = self.monthly_rate();
        let n = self.months;
        let finite = |x: f64| {
            if x.is_finite() {
                Ok(x)
            } else {
                Err(LoanError::Impossible)
            }
        };
        match unknown {
            LoanUnknown::Capital => {
                loan.capital = 0.0;
                loan.capital = finite(loan.balance_gap(i))?;
            }
            LoanUnknown::Payment => {
                if n <= 0.0 {
                    return Err(LoanError::Impossible);
                }
                let payment = if i.abs() < EPSILON {
                    (self.capital - self.residual) / n
                } else {
                    let v = (1.0 + i).powf(-n);
                    (self.capital - self.residual * v) * i / ((1.0 - v) * self.due_factor(i))
                };
                loan.payment = finite(payment)?;
            }
            LoanUnknown::Months => {
                if self.payment <= 0.0 || i <= -1.0 {
                    return Err(LoanError::Impossible);
                }
                let months = if i.abs() < EPSILON {
                    (self.capital - self.residual) / self.payment
                } else {
                    // capital = A − A·v^n + résiduel·v^n avec A = mensualité·f/i
                    let a = self.payment * self.due_factor(i) / i;
                    let ratio = (a - self.capital) / (a - self.residual);
                    if ratio <= 0.0 {
                        return Err(LoanError::Impossible);
                    }
                    -ratio.ln() / (1.0 + i).ln()
                };
                if !(months.is_finite() && months > 0.0) {
                    return Err(LoanError::Impossible);
                }
                loan.months = months;
            }
            LoanUnknown::Rate => {
                loan.annual_rate = self.solve_rate()? * 1200.0;
            }
        }
        Ok(loan)
    }

    /// Taux mensuel par dichotomie puis Newton (l'original utilisait Newton
    /// seul, depuis 50 %, sans tenir compte du résiduel ni du début de période).
    fn solve_rate(&self) -> Result<f64, LoanError> {
        if self.months <= 0.0 || self.payment <= 0.0 || self.capital <= 0.0 {
            return Err(LoanError::Impossible);
        }
        // L'écart décroît avec le taux : on encadre la racine.
        // Taux mensuel entre −1 % et 100 %.
        let (mut low, mut high) = (-0.01, 1.0);
        let gap_low = self.balance_gap(low);
        let gap_high = self.balance_gap(high);
        if !(gap_low.is_finite() && gap_high.is_finite()) || gap_low < 0.0 || gap_high > 0.0 {
            return Err(LoanError::Impossible);
        }
        for _ in 0..200 {
            let mid = (low + high) / 2.0;
            let gap = self.balance_gap(mid);
            if gap.abs() < 1e-10 || (high - low) < 1e-15 {
                return Ok(mid);
            }
            if gap > 0.0 {
                low = mid;
            } else {
                high = mid;
            }
        }
        let mid = (low + high) / 2.0;
        if self.balance_gap(mid).abs() < 1e-6 {
            Ok(mid)
        } else {
            Err(LoanError::NoConvergence)
        }
    }

    /// Coût total du crédit. Correction : la valeur résiduelle est comptée
    /// dans le total versé (l'original l'ignorait).
    pub fn cost(&self) -> Option<LoanCost> {
        let total_paid = self.payment * self.months + self.residual;
        if total_paid.abs() < EPSILON || !total_paid.is_finite() {
            return None;
        }
        let interest = total_paid - self.capital;
        Some(LoanCost {
            total_paid,
            interest,
            capital_percent: self.capital * 100.0 / total_paid,
            interest_percent: interest * 100.0 / total_paid,
        })
    }

    /// Tableau d'amortissement, à partir de l'échéance de `start_month`
    /// (1–12) / `start_year`.
    ///
    /// Une durée fractionnaire donne une dernière mensualité réduite. En début
    /// de période, la première échéance ne comporte pas d'intérêts.
    pub fn amortization(&self, start_year: i32, start_month: u32) -> Vec<AmortizationRow> {
        let i = self.monthly_rate();
        // Au-delà de 100 ans d'échéances, le tableau n'a plus de sens.
        if !(self.months.is_finite()
            && self.months > 0.0
            && self.months <= MAX_MONTHS
            && self.payment.is_finite())
        {
            return Vec::new();
        }
        let count = (self.months - 1e-6).ceil().max(1.0) as u32;
        // Capital restant visé après la dernière échéance.
        let target = if self.start_of_period {
            self.residual / (1.0 + i)
        } else {
            self.residual
        };
        let mut rows = Vec::with_capacity(count as usize);
        let mut remaining = self.capital;
        let (mut year, mut month) = (start_year, start_month.clamp(1, 12));
        for period in 1..=count {
            let interest = if self.start_of_period && period == 1 {
                0.0
            } else {
                remaining * i
            };
            let mut payment = self.payment;
            let mut principal = payment - interest;
            if period == count {
                principal = remaining - target;
                payment = principal + interest;
            }
            remaining -= principal;
            if remaining.abs() < 1e-7 {
                remaining = 0.0;
            }
            rows.push(AmortizationRow {
                period,
                year,
                month,
                payment,
                principal,
                interest,
                remaining,
            });
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
        rows
    }

    /// Nouveau crédit repartant de l'échéance `row` du tableau (double-clic
    /// de l'original) : capital = capital restant, durée = échéances restantes.
    pub fn restart_after(&self, row: &AmortizationRow) -> Loan {
        Loan {
            capital: row.remaining,
            months: (self.months - row.period as f64).max(0.0),
            ..*self
        }
    }
}

// ---------------------------------------------------------------------------
// Nombres en toutes lettres
// ---------------------------------------------------------------------------

const UNITS: [&str; 17] = [
    "zéro", "un", "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf", "dix", "onze",
    "douze", "treize", "quatorze", "quinze", "seize",
];
const TENS: [&str; 7] = [
    "",
    "dix",
    "vingt",
    "trente",
    "quarante",
    "cinquante",
    "soixante",
];

/// 1 à 99. `last` : le nombre termine l'expression (accord de
/// « quatre-vingts »).
fn below_hundred(n: u64, last: bool) -> String {
    debug_assert!((1..100).contains(&n));
    match n {
        0..=16 => UNITS[n as usize].to_string(),
        17..=19 => format!("dix-{}", UNITS[(n - 10) as usize]),
        20..=69 => {
            let (tens, unit) = (TENS[(n / 10) as usize], n % 10);
            match unit {
                0 => tens.to_string(),
                1 => format!("{tens} et un"),
                _ => format!("{tens}-{}", UNITS[unit as usize]),
            }
        }
        71 => "soixante et onze".to_string(),
        70..=79 => format!("soixante-{}", below_hundred(n - 60, true)),
        80 if last => "quatre-vingts".to_string(),
        80 => "quatre-vingt".to_string(),
        _ => format!("quatre-vingt-{}", below_hundred(n - 80, true)),
    }
}

/// 1 à 999. `last` : rien ne suit, ou seulement un nom (million…) ; faux
/// devant « mille », qui rend « cent » et « quatre-vingt » invariables.
fn below_thousand(n: u64, last: bool) -> String {
    debug_assert!((1..1000).contains(&n));
    let (hundreds, rest) = (n / 100, n % 100);
    let tail = (rest > 0).then(|| below_hundred(rest, last));
    match (hundreds, tail) {
        (0, Some(tail)) => tail,
        (1, None) => "cent".to_string(),
        (1, Some(tail)) => format!("cent {tail}"),
        (h, None) => format!("{} cent{}", UNITS[h as usize], if last { "s" } else { "" }),
        (h, Some(tail)) => format!("{} cent {tail}", UNITS[h as usize]),
    }
}

/// Grandes unités (échelle longue), de la plus grande à la plus petite.
const SCALES: [(u64, &str); 5] = [
    (1_000_000_000_000_000_000, "trillion"),
    (1_000_000_000_000_000, "billiard"),
    (1_000_000_000_000, "billion"),
    (1_000_000_000, "milliard"),
    (1_000_000, "million"),
];

/// Écrit un entier positif en toutes lettres (orthographe traditionnelle :
/// traits d'union sous cent sauf autour de « et »).
///
/// Règles : vingt et un, soixante et onze, quatre-vingts mais quatre-vingt-un,
/// deux cents mais deux cent un, mille invariable et jamais « un mille »,
/// million/milliard… accordés. L'original (`DecToText`) écrivait « dix sept »
/// sans trait d'union et « milliers-de-billions » pour 10^15.
pub fn number_to_words(n: u64) -> String {
    if n == 0 {
        return UNITS[0].to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    let mut rest = n;
    for (scale, name) in SCALES {
        let group = rest / scale;
        rest %= scale;
        if group > 0 {
            // Un groupe au-delà de 999 n'existe que pour le trillion (u64).
            let words = if group >= 1000 {
                number_to_words(group)
            } else {
                below_thousand(group, true)
            };
            let plural = if group > 1 { "s" } else { "" };
            parts.push(format!("{words} {name}{plural}"));
        }
    }
    let thousands = rest / 1000;
    rest %= 1000;
    match thousands {
        0 => {}
        1 => parts.push("mille".to_string()),
        t => parts.push(format!("{} mille", below_thousand(t, false))),
    }
    if rest > 0 {
        parts.push(below_thousand(rest, true));
    }
    parts.join(" ")
}

/// Entier signé en toutes lettres (« moins … » pour un nombre négatif).
pub fn integer_to_words(n: i64) -> String {
    if n < 0 {
        format!("moins {}", number_to_words(n.unsigned_abs()))
    } else {
        number_to_words(n as u64)
    }
}

/// Noms de la monnaie pour [`amount_to_words_with`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyWords<'a> {
    pub unit_singular: &'a str,
    pub unit_plural: &'a str,
    pub cent_singular: &'a str,
    pub cent_plural: &'a str,
}

/// Euros et centimes.
pub const EURO_WORDS: CurrencyWords<'static> = CurrencyWords {
    unit_singular: "euro",
    unit_plural: "euros",
    cent_singular: "centime",
    cent_plural: "centimes",
};

/// Montant en toutes lettres, en euros : « mille deux cent trente-quatre euros
/// et cinquante-six centimes ». Arrondi au centime (demi éloigné de zéro).
pub fn amount_to_words(amount: Amount) -> String {
    amount_to_words_with(amount, &EURO_WORDS)
}

/// Montant en toutes lettres avec les noms de monnaie donnés.
///
/// « un euro », « zéro euro », « un million d'euros », « cinquante centimes »
/// (sans « zéro euro et »). L'original écrivait « … euros et … cents ».
pub fn amount_to_words_with(amount: Amount, words: &CurrencyWords) -> String {
    let abs = amount.0.unsigned_abs();
    let cents_total = (abs + 50) / 100;
    let (units, cents) = (cents_total / 100, cents_total % 100);
    let unit_part = || {
        let name = if units > 1 {
            words.unit_plural
        } else {
            words.unit_singular
        };
        let text = number_to_words(units);
        // « un million d'euros » : de/d' après un nom de grande unité.
        if units >= 1_000_000 && units % 1_000_000 == 0 {
            let elided = name
                .chars()
                .next()
                .is_some_and(|c| "aeiouyhéèêàâîôû".contains(c.to_ascii_lowercase()));
            if elided {
                format!("{text} d'{name}")
            } else {
                format!("{text} de {name}")
            }
        } else {
            format!("{text} {name}")
        }
    };
    let cent_part = || {
        let name = if cents > 1 {
            words.cent_plural
        } else {
            words.cent_singular
        };
        format!("{} {name}", number_to_words(cents))
    };
    let text = match (units, cents) {
        (_, 0) => unit_part(),
        (0, _) => cent_part(),
        _ => format!("{} et {}", unit_part(), cent_part()),
    };
    if amount.0 < 0 && cents_total > 0 {
        format!("moins {text}")
    } else {
        text
    }
}

// ---------------------------------------------------------------------------
// Chiffres romains
// ---------------------------------------------------------------------------

const ROMAN: [(u32, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
];

/// Chiffres romains de 1 à 3999.
pub fn to_roman(n: u32) -> Option<String> {
    if !(1..=3999).contains(&n) {
        return None;
    }
    let mut rest = n;
    let mut out = String::new();
    for (value, symbol) in ROMAN {
        while rest >= value {
            out.push_str(symbol);
            rest -= value;
        }
    }
    Some(out)
}

/// Valeur d'un nombre romain (insensible à la casse). Seules les écritures
/// canoniques sont acceptées (« IIII » ou « IC » sont refusés, alors que
/// l'original les additionnait).
pub fn from_roman(text: &str) -> Option<u32> {
    let text = text.trim().to_ascii_uppercase();
    let digit = |c: char| match c {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        'D' => Some(500),
        'M' => Some(1000),
        _ => None,
    };
    let values: Vec<i64> = text.chars().map(digit).collect::<Option<_>>()?;
    // Lecture de droite à gauche : un symbole inférieur au plus grand déjà lu
    // se soustrait (« IX »).
    let mut sum: i64 = 0;
    let mut max = 0;
    for &v in values.iter().rev() {
        if v >= max {
            sum += v;
            max = v;
        } else {
            sum -= v;
        }
    }
    let total = u32::try_from(sum).ok()?;
    (to_roman(total)? == text).then_some(total)
}

// ---------------------------------------------------------------------------
// Clé de Luhn
// ---------------------------------------------------------------------------

fn digits_of(text: &str) -> Option<Vec<u32>> {
    text.chars()
        .filter(|c| !matches!(c, ' ' | '-' | '\u{a0}'))
        .map(|c| c.to_digit(10))
        .collect()
}

fn luhn_sum(digits: &[u32], check_digit_present: bool) -> u32 {
    digits
        .iter()
        .rev()
        .enumerate()
        .map(|(k, &d)| {
            let doubled = if check_digit_present {
                k % 2 == 1
            } else {
                k % 2 == 0
            };
            if doubled {
                // Correction : l'original faisait (2d) % 9, qui vaut 0 pour 9.
                let d2 = d * 2;
                if d2 > 9 { d2 - 9 } else { d2 }
            } else {
                d
            }
        })
        .sum()
}

/// Le numéro (chiffres, espaces et tirets ignorés) respecte-t-il la clé de
/// Luhn ? Au moins deux chiffres.
pub fn luhn_is_valid(number: &str) -> bool {
    match digits_of(number) {
        Some(digits) if digits.len() >= 2 => luhn_sum(&digits, true).is_multiple_of(10),
        _ => false,
    }
}

/// Chiffre de contrôle à ajouter à `partial` pour respecter la clé de Luhn.
pub fn luhn_check_digit(partial: &str) -> Option<u32> {
    let digits = digits_of(partial)?;
    if digits.is_empty() {
        return None;
    }
    Some((10 - luhn_sum(&digits, false) % 10) % 10)
}

/// Résultat de la vérification d'un numéro de carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardStatus {
    Valid,
    Invalid,
    /// Saisie en cours : trop peu de chiffres pour conclure.
    Incomplete,
}

impl CardStatus {
    /// Message affiché : « Le numéro est valide/incorrect/incomplet ».
    pub fn label(self) -> &'static str {
        match self {
            Self::Valid => tr!("valide"),
            Self::Invalid => tr!("incorrect"),
            Self::Incomplete => tr!("incomplet"),
        }
    }
}

/// Vérifie un numéro de carte bancaire en cours de saisie.
///
/// L'original exigeait exactement 16 chiffres. Ici : moins de 13 chiffres =
/// incomplet ; 13 à 15 = valide si la clé est bonne, sinon incomplet (la
/// saisie d'un numéro à 16 chiffres peut être en cours, et l'American Express
/// en a 15) ; 16 à 19 = valide ou incorrect ; au-delà, ou caractère autre
/// qu'un chiffre, une espace ou un tiret = incorrect. Un numéro nul est
/// incorrect.
pub fn check_card_number(input: &str) -> CardStatus {
    let Some(digits) = digits_of(input) else {
        return CardStatus::Invalid;
    };
    let valid = luhn_is_valid(input) && digits.iter().any(|&d| d != 0);
    match digits.len() {
        0..=12 => CardStatus::Incomplete,
        13..=15 if valid => CardStatus::Valid,
        13..=15 => CardStatus::Incomplete,
        16..=19 if valid => CardStatus::Valid,
        _ => CardStatus::Invalid,
    }
}

// ---------------------------------------------------------------------------
// RIB et IBAN
// ---------------------------------------------------------------------------

/// Reste modulo 97 d'une suite de chiffres décimaux.
fn mod97(digits: &str) -> Option<u32> {
    let mut rest = 0u32;
    for c in digits.chars() {
        rest = (rest * 10 + c.to_digit(10)?) % 97;
    }
    Some(rest)
}

/// Lettres du numéro de compte converties en chiffres selon la norme du RIB :
/// A–I → 1–9, J–R → 1–9, S–Z → 2–9.
fn rib_account_digits(account: &str) -> Option<String> {
    account
        .trim()
        .to_ascii_uppercase()
        .chars()
        .map(|c| match c {
            '0'..='9' => Some(c),
            'A'..='I' => char::from_digit(c as u32 - 'A' as u32 + 1, 10),
            'J'..='R' => char::from_digit(c as u32 - 'J' as u32 + 1, 10),
            'S'..='Z' => char::from_digit(c as u32 - 'S' as u32 + 2, 10),
            _ => None,
        })
        .collect()
}

/// Clé RIB sur deux chiffres (`clef_rib` de `UnitAccounts.pas`).
///
/// Clé française : `97 − (89 × banque + 15 × guichet + 3 × compte) mod 97`
/// (01 à 97). Un guichet vide ou `00000` désigne un compte belge : clé =
/// (banque sur 3 chiffres + compte sur 7 chiffres) mod 97, 97 si nul.
/// `None` si la banque ou le guichet n'est pas numérique ou si le compte
/// contient un caractère autre qu'une lettre ou un chiffre (Delphi calculait
/// alors une clé fausse sur un compte lu comme 0).
pub fn rib_key(bank: &str, branch: &str, account: &str) -> Option<String> {
    let (bank, branch) = (bank.trim(), branch.trim());
    let bank = if bank.is_empty() { "00000" } else { bank };
    let branch = if branch.is_empty() { "00000" } else { branch };
    if !bank.chars().all(|c| c.is_ascii_digit()) || !branch.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let account = if account.trim().is_empty() {
        "0000000".to_string()
    } else {
        rib_account_digits(account)?
    };
    if branch.chars().all(|c| c == '0') {
        return Some(belgian_key(bank, &account));
    }
    let b = mod97(bank)?;
    let g = mod97(branch)?;
    let c = mod97(&account)?;
    let key = 97 - (89 * b + 15 * g + 3 * c) % 97;
    Some(format!("{key:02}"))
}

/// Clé d'un compte belge (`belgianKey`) : les 10 derniers chiffres de
/// banque + compte (compte complété à 7 chiffres), modulo 97 (97 si nul).
fn belgian_key(bank: &str, account: &str) -> String {
    let account = if account.len() >= 7 {
        account[account.len() - 7..].to_string()
    } else {
        format!("{account:0>7}")
    };
    let number = format!("{bank}{account}");
    let number = &number[number.len().saturating_sub(10)..];
    let key = match mod97(number).unwrap_or(0) {
        0 => 97,
        k => k,
    };
    format!("{key:02}")
}

/// Longueur réglementaire de l'IBAN pour les pays les plus courants.
fn iban_length(country: &str) -> Option<usize> {
    Some(match country {
        "FR" | "MC" | "IT" | "SM" => 27,
        "BE" => 16,
        "DE" | "GB" | "IE" => 22,
        "ES" | "SE" => 24,
        "CH" | "LI" => 21,
        "LU" | "AT" => 20,
        "NL" => 18,
        "PT" => 25,
        "DK" | "FI" => 18,
        "NO" => 15,
        "PL" => 28,
        "GR" => 27,
        _ => return None,
    })
}

/// Chaîne de l'IBAN réarrangée (4 premiers caractères à la fin) et convertie
/// en chiffres (A = 10 … Z = 35).
fn iban_numeric(rearranged: &str) -> Option<String> {
    let mut out = String::with_capacity(rearranged.len() * 2);
    for c in rearranged.chars() {
        match c {
            '0'..='9' => out.push(c),
            'A'..='Z' => out.push_str(&(c as u32 - 'A' as u32 + 10).to_string()),
            _ => return None,
        }
    }
    Some(out)
}

/// IBAN sans espaces, en majuscules.
pub fn normalize_iban(iban: &str) -> String {
    iban.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase()
}

/// L'IBAN est-il valide (espaces ignorés) ? Contrôle du code pays, de la
/// longueur (pour les pays connus) et de la clé modulo 97.
pub fn iban_is_valid(iban: &str) -> bool {
    let iban = normalize_iban(iban);
    if !(15..=34).contains(&iban.len()) || !iban.is_ascii() {
        return false;
    }
    let country = &iban[..2];
    if !country.chars().all(|c| c.is_ascii_uppercase())
        || !iban[2..4].chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    if iban_length(country).is_some_and(|len| len != iban.len()) {
        return false;
    }
    let rearranged = format!("{}{}", &iban[4..], &iban[..4]);
    iban_numeric(&rearranged).and_then(|n| mod97(&n)) == Some(1)
}

/// Construit l'IBAN d'un BBAN (identifiant national) : calcule la clé.
pub fn iban_from_bban(country: &str, bban: &str) -> Option<String> {
    let country = country.trim().to_ascii_uppercase();
    let bban = normalize_iban(bban);
    if country.len() != 2 || !country.chars().all(|c| c.is_ascii_uppercase()) || bban.is_empty() {
        return None;
    }
    let numeric = iban_numeric(&format!("{bban}{country}00"))?;
    let check = 98 - mod97(&numeric)?;
    let iban = format!("{country}{check:02}{bban}");
    iban_is_valid(&iban).then_some(iban)
}

/// IBAN français à partir du RIB (banque, guichet, compte, clé).
pub fn french_iban(bank: &str, branch: &str, account: &str, key: &str) -> Option<String> {
    let bban = format!(
        "{:0>5}{:0>5}{:0>11}{:0>2}",
        bank.trim(),
        branch.trim(),
        account.trim().to_ascii_uppercase(),
        key.trim()
    );
    iban_from_bban("FR", &bban)
}

/// IBAN groupé par blocs de quatre caractères.
pub fn format_iban(iban: &str) -> String {
    let iban = normalize_iban(iban);
    iban.as_bytes()
        .chunks(4)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    fn loan() -> Loan {
        Loan {
            capital: 100_000.0,
            annual_rate: 6.0,
            months: 360.0,
            payment: 0.0,
            residual: 0.0,
            start_of_period: false,
        }
    }

    #[test]
    fn loan_payment_known_value() {
        // Valeur de référence : 100 000 à 6 % sur 30 ans = 599,55 par mois.
        let solved = loan().solve(LoanUnknown::Payment).unwrap();
        assert!(
            close(solved.payment, 599.550_525, 1e-5),
            "{}",
            solved.payment
        );
        let cost = solved.cost().unwrap();
        assert!(close(cost.total_paid, 215_838.19, 0.01));
        assert!(close(cost.interest, 115_838.19, 0.01));
        assert!(close(
            cost.capital_percent + cost.interest_percent,
            100.0,
            1e-9
        ));
    }

    #[test]
    fn loan_other_unknowns_round_trip() {
        let full = loan().solve(LoanUnknown::Payment).unwrap();
        let capital = Loan {
            capital: 0.0,
            ..full
        }
        .solve(LoanUnknown::Capital)
        .unwrap();
        assert!(close(capital.capital, 100_000.0, 1e-6));
        let months = Loan {
            months: 0.0,
            ..full
        }
        .solve(LoanUnknown::Months)
        .unwrap();
        assert!(close(months.months, 360.0, 1e-6));
        let rate = Loan {
            annual_rate: 0.0,
            ..full
        }
        .solve(LoanUnknown::Rate)
        .unwrap();
        assert!(close(rate.annual_rate, 6.0, 1e-6), "{}", rate.annual_rate);
    }

    #[test]
    fn loan_zero_rate_and_errors() {
        let l = Loan {
            capital: 1200.0,
            annual_rate: 0.0,
            months: 12.0,
            payment: 0.0,
            residual: 0.0,
            start_of_period: false,
        };
        assert_eq!(l.solve(LoanUnknown::Payment).unwrap().payment, 100.0);
        let too_small = Loan {
            capital: 100_000.0,
            annual_rate: 12.0,
            months: 0.0,
            payment: 500.0,
            residual: 0.0,
            start_of_period: false,
        };
        // 500 par mois ne couvre pas les 1 000 d'intérêts mensuels.
        assert_eq!(
            too_small.solve(LoanUnknown::Months),
            Err(LoanError::Impossible)
        );
        assert_eq!(
            Loan {
                months: 0.0,
                ..loan()
            }
            .solve(LoanUnknown::Payment),
            Err(LoanError::Impossible)
        );
    }

    #[test]
    fn amortization_schedule() {
        // 1 000 à 12 % (1 %/mois) sur 2 mois : mensualité 507,512 4.
        let l = Loan {
            capital: 1000.0,
            annual_rate: 12.0,
            months: 2.0,
            payment: 0.0,
            residual: 0.0,
            start_of_period: false,
        }
        .solve(LoanUnknown::Payment)
        .unwrap();
        assert!(close(l.payment, 507.512_437_8, 1e-6));
        let rows = l.amortization(2026, 12);
        assert_eq!(rows.len(), 2);
        assert!(close(rows[0].interest, 10.0, 1e-9));
        assert!(close(rows[0].principal, 497.512_437_8, 1e-6));
        assert!(close(rows[1].interest, 5.024_875_6, 1e-6));
        assert_eq!(rows[1].remaining, 0.0);
        assert_eq!((rows[0].year, rows[0].month), (2026, 12));
        assert_eq!((rows[1].year, rows[1].month), (2027, 1));
        let total_principal: f64 = rows.iter().map(|r| r.principal).sum();
        assert!(close(total_principal, 1000.0, 1e-9));
        let restart = l.restart_after(&rows[0]);
        assert!(close(restart.capital, 502.487_562_2, 1e-6));
        assert_eq!(restart.months, 1.0);
    }

    #[test]
    fn amortization_residual_and_start_of_period() {
        let base = Loan {
            capital: 20_000.0,
            annual_rate: 4.8,
            months: 36.0,
            payment: 0.0,
            residual: 5_000.0,
            start_of_period: true,
        };
        let l = base.solve(LoanUnknown::Payment).unwrap();
        let rows = l.amortization(2026, 1);
        assert_eq!(rows.len(), 36);
        assert_eq!(rows[0].interest, 0.0);
        // Après la dernière mensualité, le résiduel actualisé d'un mois reste dû.
        assert!(close(rows[35].remaining, 5_000.0 / 1.004, 1e-6));
        assert!(close(rows[35].payment, l.payment, 1e-6));
        let rate = Loan {
            annual_rate: 0.0,
            ..l
        }
        .solve(LoanUnknown::Rate)
        .unwrap();
        assert!(close(rate.annual_rate, 4.8, 1e-6));
        let cost = l.cost().unwrap();
        assert!(close(cost.total_paid, l.payment * 36.0 + 5_000.0, 1e-9));
    }

    #[test]
    fn fractional_duration_has_short_last_payment() {
        let l = Loan {
            capital: 1000.0,
            annual_rate: 0.0,
            months: 0.0,
            payment: 300.0,
            residual: 0.0,
            start_of_period: false,
        }
        .solve(LoanUnknown::Months)
        .unwrap();
        assert!(close(l.months, 1000.0 / 300.0, 1e-12));
        let rows = l.amortization(2026, 1);
        assert_eq!(rows.len(), 4);
        assert!(close(rows[3].payment, 100.0, 1e-9));
        assert_eq!(rows[3].remaining, 0.0);
        let absurd = Loan { months: 1e12, ..l };
        assert!(absurd.amortization(2026, 1).is_empty());
    }

    #[test]
    fn numbers_in_words() {
        let cases: &[(u64, &str)] = &[
            (0, "zéro"),
            (1, "un"),
            (16, "seize"),
            (17, "dix-sept"),
            (21, "vingt et un"),
            (22, "vingt-deux"),
            (61, "soixante et un"),
            (70, "soixante-dix"),
            (71, "soixante et onze"),
            (77, "soixante-dix-sept"),
            (80, "quatre-vingts"),
            (81, "quatre-vingt-un"),
            (90, "quatre-vingt-dix"),
            (91, "quatre-vingt-onze"),
            (99, "quatre-vingt-dix-neuf"),
            (100, "cent"),
            (101, "cent un"),
            (180, "cent quatre-vingts"),
            (200, "deux cents"),
            (201, "deux cent un"),
            (1000, "mille"),
            (1001, "mille un"),
            (2000, "deux mille"),
            (21_000, "vingt et un mille"),
            (80_000, "quatre-vingt mille"),
            (200_000, "deux cent mille"),
            (1_000_000, "un million"),
            (2_000_000, "deux millions"),
            (200_000_000, "deux cents millions"),
            (80_000_000, "quatre-vingts millions"),
            (1_000_000_000, "un milliard"),
            (
                1_234_567,
                "un million deux cent trente-quatre mille cinq cent soixante-sept",
            ),
            (1_000_000_000_000, "un billion"),
            (2_000_000_000_000_000, "deux billiards"),
        ];
        for &(n, expected) in cases {
            assert_eq!(number_to_words(n), expected, "{n}");
        }
        assert!(number_to_words(u64::MAX).starts_with("dix-huit trillions "));
        assert_eq!(integer_to_words(-21), "moins vingt et un");
    }

    #[test]
    fn amounts_in_words() {
        let words = |cents: i64| amount_to_words(Amount::from_cents(cents));
        assert_eq!(words(0), "zéro euro");
        assert_eq!(words(100), "un euro");
        assert_eq!(words(200), "deux euros");
        assert_eq!(words(1), "un centime");
        assert_eq!(words(50), "cinquante centimes");
        assert_eq!(
            words(123_456),
            "mille deux cent trente-quatre euros et cinquante-six centimes"
        );
        assert_eq!(words(8_001), "quatre-vingts euros et un centime");
        assert_eq!(words(100_000_000), "un million d'euros");
        assert_eq!(words(100_000_100), "un million un euros");
        assert_eq!(
            words(-2_150),
            "moins vingt et un euros et cinquante centimes"
        );
        // Arrondi au centime : 0,005 → un centime.
        assert_eq!(amount_to_words(Amount(50)), "un centime");
        assert_eq!(amount_to_words(Amount(49)), "zéro euro");
        let dollars = CurrencyWords {
            unit_singular: "dollar",
            unit_plural: "dollars",
            cent_singular: "cent",
            cent_plural: "cents",
        };
        assert_eq!(
            amount_to_words_with(Amount::from_cents(200_000_000), &dollars),
            "deux millions de dollars"
        );
    }

    #[test]
    fn roman_numerals() {
        assert_eq!(to_roman(1994).as_deref(), Some("MCMXCIV"));
        assert_eq!(to_roman(3999).as_deref(), Some("MMMCMXCIX"));
        assert_eq!(to_roman(0), None);
        assert_eq!(to_roman(4000), None);
        assert_eq!(from_roman("mcmxciv"), Some(1994));
        assert_eq!(from_roman("IX"), Some(9));
        assert_eq!(from_roman("IIII"), None);
        assert_eq!(from_roman("IC"), None);
        assert_eq!(from_roman("ABC"), None);
        assert_eq!(from_roman(""), None);
        for n in 1..=3999 {
            assert_eq!(from_roman(&to_roman(n).unwrap()), Some(n));
        }
    }

    #[test]
    fn luhn() {
        // Numéros de test publics des réseaux de cartes.
        for number in [
            "4111 1111 1111 1111",
            "5555555555554444",
            "4012888888881881",
            "378282246310005",
            "6011111111111117",
            "79927398713",
            "4999 9999 9999 9996",
        ] {
            assert!(luhn_is_valid(number), "{number}");
        }
        assert!(!luhn_is_valid("4111111111111112"));
        assert!(!luhn_is_valid("79927398710"));
        // Bogue de l'original : 9 en position doublée comptait 0. Ce numéro,
        // valide, était déclaré incorrect.
        assert!(luhn_is_valid("4999999999999996"));
        assert_eq!(luhn_check_digit("7992739871"), Some(3));
        assert_eq!(luhn_check_digit("411111111111111"), Some(1));
        assert_eq!(luhn_check_digit("12a"), None);
    }

    #[test]
    fn card_status() {
        assert_eq!(check_card_number("4111 1111 1111 1111"), CardStatus::Valid);
        assert_eq!(
            check_card_number("4111 1111 1111 1112"),
            CardStatus::Invalid
        );
        assert_eq!(check_card_number("4111 1111"), CardStatus::Incomplete);
        assert_eq!(check_card_number("378282246310005"), CardStatus::Valid);
        assert_eq!(check_card_number("411111111111111"), CardStatus::Incomplete);
        assert_eq!(check_card_number("4111x"), CardStatus::Invalid);
        assert_eq!(check_card_number("0000000000000000"), CardStatus::Invalid);
        assert_eq!(check_card_number(&"4".repeat(20)), CardStatus::Invalid);
        assert_eq!(CardStatus::Valid.label(), "valide");
    }

    #[test]
    fn rib_keys() {
        // IBAN d'exemple FR76 3000 6000 0112 3456 7890 189 : clé RIB 89.
        assert_eq!(
            rib_key("30006", "00001", "12345678901").as_deref(),
            Some("89")
        );
        // RIB de La Banque Postale cité en exemple : 20041 01005 0500013M026 06.
        assert_eq!(
            rib_key("20041", "01005", "0500013M026").as_deref(),
            Some("06")
        );
        assert_eq!(
            rib_key("20041", "01005", "0500013m026").as_deref(),
            Some("06")
        );
        assert_eq!(rib_key("2004A", "01005", "0500013M026"), None);
        assert_eq!(rib_key("20041", "01005", "05000*3M026"), None);
        // Compte belge (guichet vide) : 539-0075470-34.
        assert_eq!(rib_key("539", "", "0075470").as_deref(), Some("34"));
        assert_eq!(rib_key("539", "00000", "75470").as_deref(), Some("34"));
    }

    #[test]
    fn ibans() {
        assert!(iban_is_valid("FR76 3000 6000 0112 3456 7890 189"));
        assert!(iban_is_valid("fr7630006000011234567890189"));
        assert!(iban_is_valid("BE68 5390 0754 7034"));
        assert!(iban_is_valid("DE89 3704 0044 0532 0130 00"));
        assert!(iban_is_valid("GB82 WEST 1234 5698 7654 32"));
        assert!(!iban_is_valid("FR76 3000 6000 0112 3456 7890 188"));
        assert!(!iban_is_valid("FR76 3000 6000 0112 3456 7890 18"));
        assert!(!iban_is_valid("1276 3000 6000 0112 3456 7890 189"));
        assert!(!iban_is_valid(""));
        assert_eq!(
            french_iban("30006", "00001", "12345678901", "89").as_deref(),
            Some("FR7630006000011234567890189")
        );
        assert_eq!(
            iban_from_bban("BE", "539007547034").as_deref(),
            Some("BE68539007547034")
        );
        assert_eq!(
            format_iban("FR7630006000011234567890189"),
            "FR76 3000 6000 0112 3456 7890 189"
        );
    }
}
