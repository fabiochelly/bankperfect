//! Modèle de données d'un fichier BankPerfect.

use std::fmt;

use chrono::{Datelike, Days, NaiveDate};
use serde::{Deserialize, Serialize};

/// Date au format Delphi `TDateTime` (partie entière) : nombre de jours depuis
/// le 30/12/1899. C'est la représentation des fichiers v5 et de l'API des
/// scripts ; on la garde telle quelle (compacte, triable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Date(pub i32);

impl Date {
    const EPOCH: NaiveDate = NaiveDate::from_ymd_opt(1899, 12, 30).expect("date valide");

    pub fn from_naive(date: NaiveDate) -> Self {
        Self((date - Self::EPOCH).num_days() as i32)
    }

    pub fn to_naive(self) -> NaiveDate {
        if self.0 >= 0 {
            Self::EPOCH + Days::new(self.0 as u64)
        } else {
            Self::EPOCH - Days::new(self.0.unsigned_abs() as u64)
        }
    }

    pub fn today() -> Self {
        Self::from_naive(chrono::Local::now().date_naive())
    }

    pub fn from_ymd(year: i32, month: u32, day: u32) -> Option<Self> {
        NaiveDate::from_ymd_opt(year, month, day).map(Self::from_naive)
    }

    /// Analyse une date saisie : `jj/mm/aaaa`, `jj/mm/aa`, `jj/mm` (année de
    /// `today`) ou `jj` (mois de `today`). Séparateurs `/`, `.`, `-` ou espace.
    pub fn parse_fr(text: &str, today: Date) -> Option<Self> {
        let parts: Vec<u32> = text
            .split(['/', '.', '-', ' '])
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().ok())
            .collect::<Option<_>>()?;
        let now = today.to_naive();
        let (day, month, year) = match parts[..] {
            [d] => (d, now.month(), now.year()),
            [d, m] => (d, m, now.year()),
            [d, m, y] if y < 100 => {
                let century = now.year() / 100 * 100;
                let year = century + y as i32;
                (d, m, if year > now.year() + 20 { year - 100 } else { year })
            }
            [d, m, y] => (d, m, y as i32),
            _ => return None,
        };
        Self::from_ymd(year, month, day)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.to_naive();
        write!(f, "{:02}/{:02}/{:04}", d.day(), d.month(), d.year())
    }
}

/// Montant signé en dix-millièmes, comme le type `Currency` de Delphi.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Amount(pub i64);

impl Amount {
    pub const SCALE: i64 = 10_000;
    pub const ZERO: Amount = Amount(0);

    pub fn from_cents(cents: i64) -> Self {
        Self(cents * 100)
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn as_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }

    /// Analyse un montant saisi ou lu dans un fichier.
    ///
    /// Reprend `Str2Currency` (on ne garde que `-`, `.`, `,` et les chiffres),
    /// en corrigeant le cas où `,` et `.` coexistent : le dernier séparateur
    /// est le séparateur décimal, les autres sont des séparateurs de milliers.
    /// Arrondi au dix-millième le plus proche (au pair en cas d'égalité).
    pub fn parse(text: &str) -> Option<Self> {
        let kept: String = text
            .chars()
            .filter(|c| matches!(c, '-' | '.' | ',' | '0'..='9'))
            .collect();
        let negative = kept.starts_with('-');
        let body: String = kept.chars().filter(|&c| c != '-').collect();
        if !body.chars().any(|c| c.is_ascii_digit()) {
            return None;
        }

        let last_sep = body.rfind(['.', ',']);
        let (int_part, frac_part) = match last_sep {
            // Un seul séparateur répété (« 1.234.567 ») : séparateurs de milliers.
            Some(pos)
                if body.matches(&body[pos..pos + 1]).count() > 1
                    && !body.contains(if &body[pos..pos + 1] == "." { ',' } else { '.' }) =>
            {
                (body.as_str(), "")
            }
            Some(pos) => (&body[..pos], &body[pos + 1..]),
            None => (body.as_str(), ""),
        };

        let mut value: i64 = 0;
        for c in int_part.chars().filter(char::is_ascii_digit) {
            value = value.checked_mul(10)?.checked_add(c.to_digit(10)? as i64)?;
        }
        value = value.checked_mul(Self::SCALE)?;

        let digits: Vec<i64> = frac_part
            .chars()
            .filter_map(|c| c.to_digit(10))
            .map(i64::from)
            .collect();
        let mut frac = 0;
        for (i, d) in digits.iter().take(4).enumerate() {
            frac += d * 10i64.pow(3 - i as u32);
        }
        if digits.len() > 4 {
            let rest = &digits[4..];
            let round_up = rest[0] > 5
                || (rest[0] == 5 && (rest[1..].iter().any(|&d| d > 0) || frac % 2 == 1));
            if round_up {
                frac += 1;
            }
        }
        value = value.checked_add(frac)?;
        Some(Self(if negative { -value } else { value }))
    }

    /// Formate avec `decimals` décimales (0 à 4), arrondi demi éloigné de zéro,
    /// sans séparateur de milliers.
    pub fn format(self, decimals: u32, decimal_separator: char) -> String {
        let decimals = decimals.min(4);
        let unit = 10i64.pow(4 - decimals);
        let abs = self.0.unsigned_abs() as i128;
        let rounded = (abs + unit as i128 / 2) / unit as i128;
        let scale = 10i128.pow(decimals);
        let sign = if self.0 < 0 && rounded != 0 { "-" } else { "" };
        if decimals == 0 {
            format!("{sign}{rounded}")
        } else {
            format!(
                "{sign}{}{decimal_separator}{:0width$}",
                rounded / scale,
                rounded % scale,
                width = decimals as usize
            )
        }
    }
}

impl std::ops::Add for Amount {
    type Output = Amount;
    fn add(self, rhs: Amount) -> Amount {
        Amount(self.0 + rhs.0)
    }
}

impl std::ops::Neg for Amount {
    type Output = Amount;
    fn neg(self) -> Amount {
        Amount(-self.0)
    }
}

impl std::iter::Sum for Amount {
    fn sum<I: Iterator<Item = Amount>>(iter: I) -> Amount {
        Amount(iter.map(|a| a.0).sum())
    }
}

/// Les dix modes de paiement standard, dans l'ordre de la liste affichée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StandardMode {
    Carte,
    RetraitDab,
    ChequeEmis,
    Prelevement,
    VirementEmis,
    Tip,
    Versement,
    DepotCheque,
    VirementRecu,
    Titre,
}

impl StandardMode {
    pub const ALL: [StandardMode; 10] = [
        Self::Carte,
        Self::RetraitDab,
        Self::ChequeEmis,
        Self::Prelevement,
        Self::VirementEmis,
        Self::Tip,
        Self::Versement,
        Self::DepotCheque,
        Self::VirementRecu,
        Self::Titre,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Carte => "Carte",
            Self::RetraitDab => "Retrait DAB",
            Self::ChequeEmis => "Chèque émis",
            Self::Prelevement => "Prélèvement",
            Self::VirementEmis => "Virement émis",
            Self::Tip => "TIP",
            Self::Versement => "Versement",
            Self::DepotCheque => "Dépôt de chèque",
            Self::VirementRecu => "Virement reçu",
            Self::Titre => "Titre",
        }
    }

    /// Code utilisé dans les fichiers v5 et par l'API des scripts (20–25, 40–43).
    pub fn code(self) -> i32 {
        let index = self.index() as i32;
        if index < 6 { 20 + index } else { 34 + index }
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|&m| m == self)
            .expect("mode listé")
    }

    pub fn is_debit(self) -> bool {
        self.index() < 6
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.label().to_lowercase() == label.to_lowercase())
    }
}

/// Mode de paiement d'une opération ou d'un tiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    Standard(StandardMode),
    /// Mode personnalisé, désigné par son nom.
    Custom(String),
}

impl Mode {
    pub fn label(&self) -> &str {
        match self {
            Mode::Standard(m) => m.label(),
            Mode::Custom(name) => name,
        }
    }
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Standard(StandardMode::Carte)
    }
}

/// Mode de paiement défini par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomMode {
    pub name: String,
    /// Mode de dépense (montant négatif par défaut) ; sinon mode de recette.
    pub debit: bool,
}

/// État de pointage d'une opération.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Tack {
    #[default]
    None,
    /// Pointée (P).
    Pointed,
    /// Rapprochée (R).
    Reconciled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    /// Identifiant unique dans le fichier, tous comptes confondus (≥ 1).
    pub uid: u64,
    pub date: Date,
    /// Date de pointage : date de l'opération à la saisie, date du jour à
    /// chaque changement de pointage.
    pub mark_date: Date,
    pub tack: Tack,
    pub mode: Mode,
    /// Tiers.
    pub payee: String,
    /// Détail.
    pub details: String,
    pub category: Option<u32>,
    pub amount: Amount,
    /// Champs supplémentaires (hors `UId`), valeurs décodées.
    pub extra: Vec<(String, String)>,
}

impl Operation {
    pub fn extra(&self, key: &str) -> Option<&str> {
        self.extra
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Libellé affiché dans la colonne Mode : `Chq 1234` pour un chèque numéroté.
    pub fn mode_label(&self) -> String {
        match self.cheque_number() {
            Some(number) => format!("Chq {number}"),
            None => self.mode.label().to_string(),
        }
    }

    /// Numéro de chèque, porté par le détail sous la forme `n°1234`.
    pub fn cheque_number(&self) -> Option<&str> {
        if self.mode != Mode::Standard(StandardMode::ChequeEmis) {
            return None;
        }
        let start = self.details.find('°')? + '°'.len_utf8();
        let rest = &self.details[start..];
        let number = rest.split(' ').next().unwrap_or("");
        Amount::parse(number)
            .filter(|_| !number.is_empty())
            .map(|_| number)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub uid: String,
    /// Nom du compte, affiché partout.
    pub name: String,
    pub bank_name: String,
    pub agency_name: String,
    pub agency_address: String,
    pub number: String,
    pub bank_code: String,
    pub agency_code: String,
    pub bic: String,
    pub iban: String,
    /// Nom du logo de la banque (`Aucun` si aucun).
    pub logo: String,
    pub currency: String,
    pub currency_rate: f64,
    pub secondary_currency: String,
    pub secondary_rate: f64,
    pub is_virtual: bool,
    pub disabled: bool,
    pub operations: Vec<Operation>,
}

impl Account {
    /// Compte virtuel au sens des calculs : marqué virtuel, sans nom, ou dont le
    /// nom commence par `_` (`IsVirtualAccount`).
    pub fn counts_as_virtual(&self) -> bool {
        self.is_virtual || self.name.is_empty() || self.name.starts_with('_')
    }

    pub fn balance(&self) -> Amount {
        self.operations.iter().map(|op| op.amount).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    /// Identifiant stable, référencé par les opérations et les tiers.
    pub id: u32,
    pub name: String,
    /// Sous-catégorie du dernier parent qui la précède.
    pub is_child: bool,
}

/// Tiers mémorisé (auto-complétion de la saisie).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Payee {
    /// Texte recherché lors de la saisie.
    pub caption: String,
    pub mode: Mode,
    pub category: Option<u32>,
    /// Tiers à insérer dans l'opération.
    pub payee: String,
    pub details: String,
    /// Montant par défaut, tel que saisi.
    pub amount: String,
}

/// Données persistantes d'une fonctionnalité (ex-scripts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureData {
    pub key: String,
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Nom du fichier/groupe de comptes (`utilisateur`).
    pub group_name: String,
    pub guid: String,
    pub user_name: String,
    pub custom_modes: Vec<CustomMode>,
    /// Comptes dans leur ordre « réel » (celui des identifiants).
    pub accounts: Vec<Account>,
    /// Ordre d'affichage : indices dans `accounts`.
    pub account_order: Vec<usize>,
    pub default_account: usize,
    /// Catégories dans l'ordre d'affichage.
    pub categories: Vec<Category>,
    pub payees: Vec<Payee>,
    pub feature_data: Vec<FeatureData>,
    /// Clés d'en-tête inconnues, conservées telles quelles.
    pub unknown_header: Vec<(String, String)>,
}

impl Document {
    pub fn category(&self, id: u32) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == id)
    }

    /// Libellé complet : `Parent > Enfant` pour une sous-catégorie.
    pub fn category_label(&self, id: u32) -> Option<String> {
        let pos = self.categories.iter().position(|c| c.id == id)?;
        let category = &self.categories[pos];
        if !category.is_child {
            return Some(category.name.clone());
        }
        let parent = self.categories[..pos].iter().rev().find(|c| !c.is_child);
        Some(match parent {
            Some(parent) => format!("{} > {}", parent.name, category.name),
            None => category.name.clone(),
        })
    }

    /// Liste des modes telle qu'affichée : les 10 standard puis les personnalisés.
    pub fn mode_list(&self) -> Vec<Mode> {
        StandardMode::ALL
            .into_iter()
            .map(Mode::Standard)
            .chain(
                self.custom_modes
                    .iter()
                    .map(|m| Mode::Custom(m.name.clone())),
            )
            .collect()
    }

    pub fn next_operation_uid(&self) -> u64 {
        self.accounts
            .iter()
            .flat_map(|a| &a.operations)
            .map(|op| op.uid)
            .max()
            .unwrap_or(0)
            + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delphi_dates() {
        assert_eq!(Date(39444).to_string(), "28/12/2007");
        assert_eq!(Date(2).to_string(), "01/01/1900");
        assert_eq!(Date(61).to_string(), "01/03/1900");
        assert_eq!(Date::from_ymd(2011, 1, 31), Some(Date(40574)));
        let today = Date::from_ymd(2026, 10, 11).unwrap();
        assert_eq!(Date::parse_fr("31/01/2011", today), Some(Date(40574)));
        assert_eq!(Date::parse_fr("31.01.11", today), Some(Date(40574)));
        assert_eq!(Date::parse_fr("5/3", today), Date::from_ymd(2026, 3, 5));
        assert_eq!(Date::parse_fr("12", today), Date::from_ymd(2026, 10, 12));
        assert_eq!(Date::parse_fr("1/1/86", today), Date::from_ymd(1986, 1, 1));
        assert_eq!(Date::parse_fr("31/02/2020", today), None);
        assert_eq!(Date::parse_fr("abc", today), None);
    }

    #[test]
    fn parse_amounts() {
        assert_eq!(Amount::parse("-1,78"), Some(Amount(-17_800)));
        assert_eq!(Amount::parse("600.00"), Some(Amount(6_000_000)));
        assert_eq!(Amount::parse("1 700,99 €"), Some(Amount(17_009_900)));
        assert_eq!(Amount::parse("1.234,56"), Some(Amount(12_345_600)));
        assert_eq!(Amount::parse("1,234.56"), Some(Amount(12_345_600)));
        assert_eq!(Amount::parse("1.234.567"), Some(Amount(12_345_670_000)));
        assert_eq!(Amount::parse("0,12345"), Some(Amount(1_234)));
        assert_eq!(Amount::parse("0,12355"), Some(Amount(1_236)));
        assert_eq!(Amount::parse(""), None);
        assert_eq!(Amount::parse("abc"), None);
    }

    #[test]
    fn format_amounts() {
        assert_eq!(Amount(-17_800).format(2, ','), "-1,78");
        assert_eq!(Amount(6_000_000).format(2, ','), "600,00");
        assert_eq!(Amount(12_345).format(2, '.'), "1.23");
        assert_eq!(Amount(12_350).format(2, ','), "1,24");
        assert_eq!(Amount(-12_350).format(2, ','), "-1,24");
        assert_eq!(Amount(-40).format(2, ','), "0,00");
        assert_eq!(Amount(17_800).format(0, ','), "2");
    }

    #[test]
    fn standard_mode_codes() {
        let codes: Vec<i32> = StandardMode::ALL.iter().map(|m| m.code()).collect();
        assert_eq!(codes, [20, 21, 22, 23, 24, 25, 40, 41, 42, 43]);
        assert_eq!(
            StandardMode::from_label("prélèvement"),
            Some(StandardMode::Prelevement)
        );
    }
}
