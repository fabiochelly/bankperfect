//! Cotations des lignes « Titre » (ex-script `update_stock.py`).
//!
//! Une ligne de mode « Titre » porte le code du titre dans son détail (code
//! ISIN, ou symbole boursier) et la quantité en tête du tiers
//! (« 10 France Telecom… »). Sa valeur est le cours multiplié par la
//! quantité, converti dans la devise du compte.
//!
//! L'original lisait une page de MSN Money qui n'existe plus. Les cours sont
//! désormais lus sur l'interface publique (non officielle, sans clé) de
//! Yahoo Finance : recherche du symbole à partir de l'ISIN, puis cours.

use crate::currencies::Rates;
use crate::model::{Account, Mode, StandardMode};

/// Recherche d'un symbole (ISIN accepté).
pub const SEARCH_URL: &str =
    "https://query1.finance.yahoo.com/v1/finance/search?quotesCount=5&newsCount=0&q=";
/// Cours d'un symbole.
pub const CHART_URL: &str = "https://query1.finance.yahoo.com/v8/finance/chart/";

/// Ligne « Titre » du compte à évaluer.
#[derive(Debug, Clone, PartialEq)]
pub struct StockLine {
    /// Index de l'opération dans le compte.
    pub line: usize,
    /// Code ISIN ou symbole (premier mot du détail).
    pub identifier: String,
    /// Quantité (premier mot du tiers), `None` si illisible.
    pub quantity: Option<f64>,
}

/// Cours d'un titre.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    /// Code ISO de la devise du cours.
    pub currency: String,
}

/// Lignes « Titre » du compte ayant un code dans leur détail.
pub fn stock_lines(account: &Account) -> Vec<StockLine> {
    account
        .operations
        .iter()
        .enumerate()
        .filter(|(_, op)| op.mode == Mode::Standard(StandardMode::Titre))
        .filter_map(|(line, op)| {
            let identifier = op.details.split_whitespace().next()?.to_string();
            let quantity = op
                .payee
                .split_whitespace()
                .next()
                .and_then(|q| q.replace(',', ".").parse::<f64>().ok())
                .filter(|q| q.is_finite());
            Some(StockLine {
                line,
                identifier,
                quantity,
            })
        })
        .collect()
}

/// Le texte a-t-il la forme d'un code ISIN (2 lettres, 9 caractères, 1
/// chiffre de contrôle) ?
pub fn looks_like_isin(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 12
        && b[..2].iter().all(u8::is_ascii_uppercase)
        && b[2..11].iter().all(u8::is_ascii_alphanumeric)
        && b[11].is_ascii_digit()
}

/// Symbole trouvé par la recherche (premier titre coté).
pub fn parse_search(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let quotes = value.get("quotes")?.as_array()?;
    let tradable = |q: &&serde_json::Value| {
        matches!(
            q.get("quoteType").and_then(|t| t.as_str()),
            Some("EQUITY" | "ETF" | "MUTUALFUND" | "INDEX" | "BOND")
        )
    };
    quotes
        .iter()
        .find(tradable)
        .or_else(|| quotes.first())
        .and_then(|q| q.get("symbol")?.as_str())
        .map(str::to_string)
}

/// Cours lu dans la réponse « chart ». Les cours en pence (`GBp`) sont
/// ramenés en livres.
pub fn parse_chart(json: &str) -> Option<Quote> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let meta = value.get("chart")?.get("result")?.get(0)?.get("meta")?;
    let mut price = meta.get("regularMarketPrice")?.as_f64()?;
    let mut currency = meta.get("currency")?.as_str()?.to_string();
    if currency == "GBp" || currency == "GBX" {
        price /= 100.0;
        currency = "GBP".into();
    }
    if !(price.is_finite() && price > 0.0) {
        return None;
    }
    let name = ["longName", "shortName", "symbol"]
        .iter()
        .find_map(|k| meta.get(*k).and_then(|v| v.as_str()))
        .unwrap_or_default()
        .to_string();
    Some(Quote {
        symbol: meta
            .get("symbol")
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_string(),
        name,
        price,
        currency: currency.to_uppercase(),
    })
}

/// Code ISO d'une devise de compte (`€`, `$`, `£`… ou déjà un code).
pub fn iso_currency(symbol: &str) -> String {
    match symbol.trim() {
        "€" | "" => "EUR".into(),
        "$" | "US$" => "USD".into(),
        "£" => "GBP".into(),
        "¥" => "JPY".into(),
        "FS" | "Fr." => "CHF".into(),
        other => other.to_uppercase(),
    }
}

/// Convertit un cours dans la devise du compte : avec la devise secondaire
/// du compte et ses taux quand elle correspond (comme l'original), sinon
/// avec les cours de la BCE.
pub fn price_in_account_currency(
    price: f64,
    currency: &str,
    account: &Account,
    rates: Option<&Rates>,
) -> Result<f64, String> {
    let target = iso_currency(&account.currency);
    if currency == target {
        return Ok(price);
    }
    if iso_currency(&account.secondary_currency) == currency
        && account.currency_rate > 0.0
        && account.secondary_rate > 0.0
    {
        return Ok(price * account.currency_rate / account.secondary_rate);
    }
    rates
        .and_then(|r| r.convert(price, currency, &target))
        .ok_or_else(|| format!("cours en {currency}, non convertible en {target}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Amount, Date, Operation, Tack};

    fn op(mode: Mode, payee: &str, details: &str) -> Operation {
        Operation {
            uid: 1,
            date: Date(45000),
            mark_date: Date(45000),
            tack: Tack::None,
            mode,
            payee: payee.into(),
            details: details.into(),
            category: None,
            amount: Amount::ZERO,
            extra: Vec::new(),
        }
    }

    fn account(currency: &str, secondary: &str) -> Account {
        let mut a = crate::manage::new_document("Test", Amount::ZERO)
            .accounts
            .remove(0);
        a.currency = currency.into();
        a.secondary_currency = secondary.into();
        a.currency_rate = 1.0;
        a.secondary_rate = 0.9;
        a
    }

    #[test]
    fn finds_stock_lines() {
        let mut a = account("€", "$");
        a.operations = vec![
            op(
                Mode::Standard(StandardMode::Titre),
                "10 Orange",
                "FR0000133308 Paris",
            ),
            op(
                Mode::Standard(StandardMode::Carte),
                "12 courses",
                "FR0000133308",
            ),
            op(Mode::Standard(StandardMode::Titre), "Apple", "AAPL"),
            op(Mode::Standard(StandardMode::Titre), "2,5 Fonds", ""),
        ];
        let lines = stock_lines(&a);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0],
            StockLine {
                line: 0,
                identifier: "FR0000133308".into(),
                quantity: Some(10.0)
            }
        );
        assert_eq!(lines[1].identifier, "AAPL");
        assert_eq!(lines[1].quantity, None);
        assert!(looks_like_isin("FR0000133308"));
        assert!(!looks_like_isin("AAPL"));
    }

    #[test]
    fn parses_yahoo_answers() {
        let search = r#"{"quotes":[{"symbol":"ORA.F","quoteType":"FUTURE"},{"symbol":"ORA.PA","quoteType":"EQUITY"}]}"#;
        assert_eq!(parse_search(search).as_deref(), Some("ORA.PA"));
        assert_eq!(parse_search(r#"{"quotes":[]}"#), None);
        let chart = r#"{"chart":{"result":[{"meta":{"currency":"EUR","symbol":"ORA.PA","regularMarketPrice":13.87,"longName":"Orange S.A."}}]}}"#;
        let quote = parse_chart(chart).unwrap();
        assert_eq!(
            (quote.price, quote.currency.as_str(), quote.name.as_str()),
            (13.87, "EUR", "Orange S.A.")
        );
        let pence = r#"{"chart":{"result":[{"meta":{"currency":"GBp","symbol":"VOD.L","regularMarketPrice":7250}}]}}"#;
        let quote = parse_chart(pence).unwrap();
        assert_eq!((quote.price, quote.currency.as_str()), (72.5, "GBP"));
        assert_eq!(parse_chart(r#"{"chart":{"result":null}}"#), None);
    }

    #[test]
    fn converts_into_the_account_currency() {
        let euro = account("€", "$");
        assert_eq!(
            price_in_account_currency(10.0, "EUR", &euro, None),
            Ok(10.0)
        );
        // Devise secondaire du compte : taux du compte.
        let p = price_in_account_currency(9.0, "USD", &euro, None).unwrap();
        assert!((p - 10.0).abs() < 1e-9);
        // Autre devise : cours de la BCE.
        let mut rates = Rates::default();
        rates.per_euro.insert("CHF".into(), 0.93);
        let p = price_in_account_currency(9.3, "CHF", &euro, Some(&rates)).unwrap();
        assert!((p - 10.0).abs() < 1e-9);
        assert!(price_in_account_currency(1.0, "JPY", &euro, Some(&rates)).is_err());
    }
}
