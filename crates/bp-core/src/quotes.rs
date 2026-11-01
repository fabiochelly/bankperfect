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
use crate::model::{Account, Amount, Date, Mode, Operation, StandardMode, Tack};

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
    /// Nom du titre : le tiers sans la quantité ni le prix d'achat.
    pub name: String,
    /// Prix d'achat unitaire (« … acheté 13,87 » dans le tiers).
    pub unit_cost: Option<f64>,
    /// Date d'achat.
    pub date: Date,
    /// Valeur enregistrée sur la ligne.
    pub value: Amount,
}

/// Cours d'un titre.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    /// Code ISO de la devise du cours.
    pub currency: String,
    /// Clôture de la veille, dans la même devise.
    pub previous_close: Option<f64>,
}

/// Cours passés d'un titre, triés par date.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceHistory {
    /// Code ISO de la devise des cours.
    pub currency: String,
    pub points: Vec<(Date, f64)>,
}

/// Période d'historique demandée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryRange {
    OneMonth,
    SixMonths,
    OneYear,
    FiveYears,
    Max,
}

impl HistoryRange {
    /// Paramètres de la requête : au quotidien, puis moins finement.
    pub fn query(self) -> &'static str {
        match self {
            Self::OneMonth => "range=1mo&interval=1d",
            Self::SixMonths => "range=6mo&interval=1d",
            Self::OneYear => "range=1y&interval=1d",
            Self::FiveYears => "range=5y&interval=1wk",
            Self::Max => "range=max&interval=1mo",
        }
    }
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
            let (title, bought) = match op.payee.split_once(" acheté ") {
                Some((title, bought)) => (title, Some(bought)),
                None => (op.payee.as_str(), None),
            };
            let title = title.trim();
            let name = match quantity {
                Some(_) => title.split_once(char::is_whitespace).map_or("", |(_, n)| n),
                None => title,
            };
            Some(StockLine {
                line,
                name: match name.trim() {
                    "" => identifier.clone(),
                    name => name.to_string(),
                },
                identifier,
                quantity,
                unit_cost: bought
                    .and_then(|b| b.split_whitespace().next())
                    .and_then(positive_number),
                date: op.date,
                value: op.amount,
            })
        })
        .collect()
}

/// Nombre saisi (virgule ou point décimal), strictement positif.
pub fn positive_number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
}

/// Ligne « Titre » pour un titre saisi : la quantité en tête du tiers, le
/// prix d'achat conservé dans le tiers (« 10 Orange acheté 13,87 », comme
/// l'original), le code dans le détail. La valeur est le prix multiplié par
/// la quantité, ou nulle tant qu'aucun cours n'est connu.
pub fn new_stock_operation(
    code: &str,
    quantity: &str,
    name: &str,
    price: &str,
    date: Date,
) -> Result<Operation, String> {
    let code = code.replace(';', " ").trim().to_uppercase();
    if code.is_empty() {
        return Err(crate::tr!("Saisissez le code du titre.").into());
    }
    if code.contains(char::is_whitespace) {
        return Err(crate::tr!("Le code du titre s'écrit en un seul mot.").into());
    }
    let quantity = quantity.trim();
    let count = positive_number(quantity).ok_or(crate::tr!(
        "Saisissez le nombre de titres (ex. : 10 ou 2,5)."
    ))?;
    let name = name.replace(';', " ");
    let name = match name.trim() {
        "" => code.as_str(),
        name => name,
    };
    let price = price.trim();
    let (payee, amount) = if price.is_empty() {
        (format!("{quantity} {name}"), Amount::ZERO)
    } else {
        let unit = positive_number(price).ok_or(crate::tr!("Prix d'achat illisible."))?;
        (
            format!("{quantity} {name} acheté {price}"),
            Amount::from_cents((unit * count * 100.0).round() as i64),
        )
    };
    Ok(Operation {
        uid: 0,
        date,
        mark_date: date,
        tack: Tack::None,
        mode: Mode::Standard(StandardMode::Titre),
        payee,
        details: code,
        category: None,
        amount,
        extra: Vec::new(),
    })
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
    let scale = pence_scale(&mut currency);
    price *= scale;
    if !(price.is_finite() && price > 0.0) {
        return None;
    }
    let previous_close = meta
        .get("chartPreviousClose")
        .and_then(|p| p.as_f64())
        .filter(|p| p.is_finite() && *p > 0.0)
        .map(|p| p * scale);
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
        previous_close,
    })
}

/// Facteur de conversion des cours en pence (`GBp`), ramenés en livres.
fn pence_scale(currency: &mut String) -> f64 {
    if currency == "GBp" || currency == "GBX" {
        *currency = "GBP".into();
        0.01
    } else {
        1.0
    }
}

/// Cours de clôture lus dans la réponse « chart ».
pub fn parse_history(json: &str) -> Option<PriceHistory> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let result = value.get("chart")?.get("result")?.get(0)?;
    let mut currency = result.get("meta")?.get("currency")?.as_str()?.to_string();
    let scale = pence_scale(&mut currency);
    let times = result.get("timestamp")?.as_array()?;
    let closes = result
        .get("indicators")?
        .get("quote")?
        .get(0)?
        .get("close")?
        .as_array()?;
    let mut points: Vec<(Date, f64)> = Vec::with_capacity(times.len());
    for (time, close) in times.iter().zip(closes) {
        // Séance sans cours : `null`.
        let (Some(time), Some(close)) = (time.as_i64(), close.as_f64()) else {
            continue;
        };
        let Some(moment) = chrono::DateTime::from_timestamp(time, 0) else {
            continue;
        };
        if !(close.is_finite() && close > 0.0) {
            continue;
        }
        let point = (Date::from_naive(moment.date_naive()), close * scale);
        match points.last_mut() {
            Some(last) if last.0 >= point.0 => *last = point,
            _ => points.push(point),
        }
    }
    Some(PriceHistory {
        currency: currency.to_uppercase(),
        points,
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
        .ok_or_else(|| {
            crate::tr!(
                "cours en {currency}, non convertible en {target}",
                currency = currency,
                target = target
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            (
                lines[0].line,
                lines[0].identifier.as_str(),
                lines[0].quantity,
                lines[0].name.as_str()
            ),
            (0, "FR0000133308", Some(10.0), "Orange")
        );
        assert_eq!(lines[1].identifier, "AAPL");
        assert_eq!(lines[1].quantity, None);
        assert!(looks_like_isin("FR0000133308"));
        assert!(!looks_like_isin("AAPL"));
    }

    #[test]
    fn builds_a_stock_line_from_the_form() {
        let date = Date(45000);
        let bought = new_stock_operation(" fr0000133308 ", "2,5", "Orange", "13,80", date).unwrap();
        assert_eq!(bought.payee, "2,5 Orange acheté 13,80");
        assert_eq!(bought.details, "FR0000133308");
        assert_eq!(bought.amount, Amount::from_cents(3450));
        let bare = new_stock_operation("aapl", "10", "", "", date).unwrap();
        assert_eq!(bare.payee, "10 AAPL");
        assert_eq!(bare.amount, Amount::ZERO);

        // La ligne créée est bien relue comme un titre à évaluer.
        let mut a = account("€", "$");
        a.operations = vec![bought, bare];
        let lines = stock_lines(&a);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].identifier, "FR0000133308");
        assert_eq!(lines[0].quantity, Some(2.5));
        assert_eq!(lines[0].name, "Orange");
        assert_eq!(lines[0].unit_cost, Some(13.8));
        assert_eq!(lines[0].date, date);
        assert!(looks_like_isin(&lines[0].identifier));
        assert_eq!(lines[1].identifier, "AAPL");
        assert_eq!(lines[1].quantity, Some(10.0));
        assert_eq!(lines[1].name, "AAPL");
        assert_eq!(lines[1].unit_cost, None);
    }

    #[test]
    fn reads_the_purchase_price_of_old_lines() {
        let mut a = account("€", "$");
        a.operations = vec![
            op(
                Mode::Standard(StandardMode::Titre),
                "10 France Telecom acheté 20,5",
                "FR0000133308",
            ),
            op(Mode::Standard(StandardMode::Titre), "Apple", "AAPL"),
        ];
        let lines = stock_lines(&a);
        assert_eq!(lines[0].name, "France Telecom");
        assert_eq!(lines[0].unit_cost, Some(20.5));
        assert_eq!(lines[1].name, "Apple");
    }

    #[test]
    fn parses_a_price_history() {
        // 2026-10-01 et 2026-10-02, séance sans cours, puis le même jour en double.
        let json = r#"{"chart":{"result":[{"meta":{"currency":"GBp"},
            "timestamp":[1790841600,1790928000,1791014400,1791014500],
            "indicators":{"quote":[{"close":[100.0,200.0,null,300.0]}]}}]}}"#;
        let history = parse_history(json).unwrap();
        assert_eq!(history.currency, "GBP");
        let d1 = Date::from_ymd(2026, 10, 1).unwrap();
        let d2 = Date::from_ymd(2026, 10, 2).unwrap();
        let d3 = Date::from_ymd(2026, 10, 3).unwrap();
        assert_eq!(history.points, vec![(d1, 1.0), (d2, 2.0), (d3, 3.0)]);
        assert_eq!(parse_history(r#"{"chart":{"result":null}}"#), None);
    }

    #[test]
    fn refuses_an_incomplete_stock_line() {
        let date = Date(45000);
        assert!(new_stock_operation("", "10", "Orange", "", date).is_err());
        assert!(new_stock_operation("ORA PA", "10", "Orange", "", date).is_err());
        assert!(new_stock_operation("ORA.PA", "", "Orange", "", date).is_err());
        assert!(new_stock_operation("ORA.PA", "0", "Orange", "", date).is_err());
        assert!(new_stock_operation("ORA.PA", "dix", "Orange", "", date).is_err());
        assert!(new_stock_operation("ORA.PA", "10", "Orange", "abc", date).is_err());
    }

    #[test]
    fn parses_yahoo_answers() {
        let search = r#"{"quotes":[{"symbol":"ORA.F","quoteType":"FUTURE"},{"symbol":"ORA.PA","quoteType":"EQUITY"}]}"#;
        assert_eq!(parse_search(search).as_deref(), Some("ORA.PA"));
        assert_eq!(parse_search(r#"{"quotes":[]}"#), None);
        let chart = r#"{"chart":{"result":[{"meta":{"currency":"EUR","symbol":"ORA.PA","regularMarketPrice":13.87,"chartPreviousClose":13.5,"longName":"Orange S.A."}}]}}"#;
        let quote = parse_chart(chart).unwrap();
        assert_eq!(
            (quote.price, quote.currency.as_str(), quote.name.as_str()),
            (13.87, "EUR", "Orange S.A.")
        );
        assert_eq!(quote.previous_close, Some(13.5));
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
