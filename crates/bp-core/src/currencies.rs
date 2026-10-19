//! Conversions de devises (ex-script `currencies.py`) : devises connues,
//! cours de référence de la Banque centrale européenne et conversion.
//!
//! L'original lisait les cours sur une page de xe.com qui n'existe plus ; on
//! lit le flux quotidien public de la BCE (`eurofxref-daily.xml`), qui donne
//! la valeur d'un euro dans une trentaine de devises.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Adresse du flux quotidien des cours de référence de la BCE.
pub const ECB_DAILY_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";

/// Devises connues (noms en français, à traduire par [`currency_name`] ou
/// [`currency_list`]) : celles du flux de la BCE et quelques autres courantes
/// (convertibles seulement avec un cours saisi à la main).
pub const CURRENCIES: &[(&str, &str)] = &[
    ("EUR", crate::tr_mark!("Euro")),
    ("USD", crate::tr_mark!("Dollar américain")),
    ("GBP", crate::tr_mark!("Livre sterling")),
    ("CHF", crate::tr_mark!("Franc suisse")),
    ("JPY", crate::tr_mark!("Yen japonais")),
    ("CAD", crate::tr_mark!("Dollar canadien")),
    ("AUD", crate::tr_mark!("Dollar australien")),
    ("NZD", crate::tr_mark!("Dollar néo-zélandais")),
    ("CNY", crate::tr_mark!("Yuan chinois")),
    ("HKD", crate::tr_mark!("Dollar de Hong Kong")),
    ("SGD", crate::tr_mark!("Dollar de Singapour")),
    ("KRW", crate::tr_mark!("Won sud-coréen")),
    ("INR", crate::tr_mark!("Roupie indienne")),
    ("IDR", crate::tr_mark!("Roupie indonésienne")),
    ("MYR", crate::tr_mark!("Ringgit malaisien")),
    ("PHP", crate::tr_mark!("Peso philippin")),
    ("THB", crate::tr_mark!("Baht thaïlandais")),
    ("ILS", crate::tr_mark!("Shekel israélien")),
    ("TRY", crate::tr_mark!("Livre turque")),
    ("ZAR", crate::tr_mark!("Rand sud-africain")),
    ("BRL", crate::tr_mark!("Réal brésilien")),
    ("MXN", crate::tr_mark!("Peso mexicain")),
    ("SEK", crate::tr_mark!("Couronne suédoise")),
    ("NOK", crate::tr_mark!("Couronne norvégienne")),
    ("DKK", crate::tr_mark!("Couronne danoise")),
    ("ISK", crate::tr_mark!("Couronne islandaise")),
    ("CZK", crate::tr_mark!("Couronne tchèque")),
    ("PLN", crate::tr_mark!("Zloty polonais")),
    ("HUF", crate::tr_mark!("Forint hongrois")),
    ("RON", crate::tr_mark!("Leu roumain")),
    ("BGN", crate::tr_mark!("Lev bulgare")),
    ("RUB", crate::tr_mark!("Rouble russe")),
    ("UAH", crate::tr_mark!("Hryvnia ukrainienne")),
    ("RSD", crate::tr_mark!("Dinar serbe")),
    ("MAD", crate::tr_mark!("Dirham marocain")),
    ("TND", crate::tr_mark!("Dinar tunisien")),
    ("DZD", crate::tr_mark!("Dinar algérien")),
    ("XOF", crate::tr_mark!("Franc CFA (BCEAO)")),
    ("XAF", crate::tr_mark!("Franc CFA (BEAC)")),
    ("XPF", crate::tr_mark!("Franc Pacifique")),
    ("EGP", crate::tr_mark!("Livre égyptienne")),
    ("AED", crate::tr_mark!("Dirham des Émirats")),
    ("SAR", crate::tr_mark!("Riyal saoudien")),
    ("QAR", crate::tr_mark!("Riyal qatari")),
    ("ARS", crate::tr_mark!("Peso argentin")),
    ("CLP", crate::tr_mark!("Peso chilien")),
    ("COP", crate::tr_mark!("Peso colombien")),
    ("PEN", crate::tr_mark!("Sol péruvien")),
    ("VND", crate::tr_mark!("Dong vietnamien")),
    ("TWD", crate::tr_mark!("Dollar taïwanais")),
];

/// Nom d'une devise dans la langue de l'interface (le code lui-même si elle
/// est inconnue).
pub fn currency_name(code: &str) -> &str {
    CURRENCIES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or(code, |(_, n)| crate::i18n::tr(n))
}

/// Les devises connues, (code, nom dans la langue de l'interface).
pub fn currency_list() -> impl Iterator<Item = (&'static str, &'static str)> {
    CURRENCIES
        .iter()
        .map(|&(code, name)| (code, crate::i18n::tr(name)))
}

/// Cours de référence : valeur d'un euro dans chaque devise.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rates {
    /// Date de publication (`AAAA-MM-JJ`), vide si inconnue.
    pub date: String,
    /// Unités de devise pour un euro (l'euro lui-même vaut 1).
    pub per_euro: BTreeMap<String, f64>,
}

/// Échec de lecture du flux de la BCE.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RatesError {
    Empty,
}

impl std::fmt::Display for RatesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str(crate::tr!("le flux de la BCE ne contient aucun cours")),
        }
    }
}

/// Valeur de l'attribut `name='…'` (ou `"…"`) dans une balise.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    for quote in ['\'', '"'] {
        let key = format!("{name}={quote}");
        if let Some(start) = tag.find(&key) {
            let rest = &tag[start + key.len()..];
            return rest.find(quote).map(|end| &rest[..end]);
        }
    }
    None
}

impl Rates {
    /// Lit le flux quotidien de la BCE : des balises
    /// `<Cube time='2026-10-09'>` et `<Cube currency='USD' rate='1.0876'/>`.
    pub fn parse_ecb(xml: &str) -> Result<Self, RatesError> {
        let mut rates = Rates::default();
        for tag in xml.split('<').skip(1) {
            let tag = tag.split('>').next().unwrap_or("");
            if !tag.starts_with("Cube") {
                continue;
            }
            if let Some(time) = attribute(tag, "time") {
                rates.date = time.to_string();
            }
            if let (Some(code), Some(rate)) = (attribute(tag, "currency"), attribute(tag, "rate"))
                && let Ok(rate) = rate.trim().parse::<f64>()
                && rate.is_finite()
                && rate > 0.0
            {
                rates.per_euro.insert(code.trim().to_uppercase(), rate);
            }
        }
        if rates.per_euro.is_empty() {
            return Err(RatesError::Empty);
        }
        rates.per_euro.insert("EUR".into(), 1.0);
        Ok(rates)
    }

    /// Cours d'une devise (unités pour un euro).
    pub fn rate(&self, code: &str) -> Option<f64> {
        if code == "EUR" {
            return Some(1.0);
        }
        self.per_euro.get(code).copied()
    }

    /// Convertit `amount` de la devise `from` vers `to`.
    pub fn convert(&self, amount: f64, from: &str, to: &str) -> Option<f64> {
        Some(amount / self.rate(from)? * self.rate(to)?)
    }

    /// Ajoute ou remplace les cours de `newer` (les cours saisis à la main
    /// pour des devises absentes du flux sont conservés).
    pub fn merge(&mut self, newer: Rates) {
        self.date = newer.date;
        self.per_euro.extend(newer.per_euro);
    }
}

/// Clé de recherche insensible à la casse et aux accents.
pub fn search_key(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ÿ' => 'y',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
	<gesmes:subject>Reference rates</gesmes:subject>
	<Cube>
		<Cube time='2026-10-09'>
			<Cube currency='USD' rate='1.0876'/>
			<Cube currency='JPY' rate='162.35'/>
			<Cube currency="GBP" rate="0.8412"/>
			<Cube currency='CHF' rate='bogus'/>
		</Cube>
	</Cube>
</gesmes:Envelope>"#;

    #[test]
    fn parses_the_ecb_feed() {
        let rates = Rates::parse_ecb(SAMPLE).unwrap();
        assert_eq!(rates.date, "2026-10-09");
        assert_eq!(rates.rate("USD"), Some(1.0876));
        assert_eq!(rates.rate("GBP"), Some(0.8412));
        assert_eq!(rates.rate("EUR"), Some(1.0));
        assert_eq!(rates.rate("CHF"), None);
        assert_eq!(Rates::parse_ecb("<html></html>"), Err(RatesError::Empty));
    }

    #[test]
    fn converts_through_the_euro() {
        let rates = Rates::parse_ecb(SAMPLE).unwrap();
        assert!((rates.convert(100.0, "EUR", "USD").unwrap() - 108.76).abs() < 1e-9);
        assert!((rates.convert(108.76, "USD", "EUR").unwrap() - 100.0).abs() < 1e-9);
        let gbp = rates.convert(100.0, "USD", "GBP").unwrap();
        assert!((gbp - 100.0 / 1.0876 * 0.8412).abs() < 1e-9);
        assert_eq!(rates.convert(1.0, "EUR", "XYZ"), None);
    }

    #[test]
    fn merge_keeps_manual_rates() {
        let mut rates = Rates::default();
        rates.per_euro.insert("XOF".into(), 655.957);
        rates.merge(Rates::parse_ecb(SAMPLE).unwrap());
        assert_eq!(rates.rate("XOF"), Some(655.957));
        assert_eq!(rates.date, "2026-10-09");
    }

    #[test]
    fn names_and_search() {
        assert_eq!(currency_name("CHF"), "Franc suisse");
        assert_eq!(currency_name("ABC"), "ABC");
        assert_eq!(search_key("Réal BRÉSILIEN"), "real bresilien");
        crate::i18n::with_lang(crate::i18n::Lang::En, || {
            assert_eq!(currency_name("CHF"), "Swiss franc");
            assert!(currency_list().any(|(c, n)| c == "USD" && n == "US dollar"));
        });
    }
}
