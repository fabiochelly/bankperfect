//! Formatage des montants et des nombres pour l'affichage, selon la langue :
//! `-1 234,56 €` en français, `-€1,234.56` en anglais.

use bp_core::i18n::{self, Lang};
use bp_core::model::Amount;

/// Sépare les milliers de la partie entière `digits`.
fn group(digits: &str, separator: char) -> String {
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3 * 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(separator);
        }
        grouped.push(c);
    }
    grouped
}

/// Place la devise : après le nombre en français ; en anglais, un symbole
/// d'un caractère se met devant (`-€12.50`), un code derrière (`12.50 CHF`).
fn with_currency(sign: &str, number: &str, currency: &str) -> String {
    if currency.is_empty() {
        format!("{sign}{number}")
    } else if i18n::lang() == Lang::En && currency.chars().count() == 1 {
        format!("{sign}{currency}{number}")
    } else {
        format!("{sign}{number}\u{a0}{currency}")
    }
}

/// `-1 234,56 €` : espace fine insécable entre les milliers, virgule
/// décimale (en anglais : `-€1,234.56`).
pub fn money(amount: Amount, currency: &str) -> String {
    let plain = amount.format(2, '.');
    let (sign, rest) = match plain.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", plain.as_str()),
    };
    let (int_part, frac) = rest.split_once('.').unwrap_or((rest, "00"));
    let number = format!(
        "{}{}{frac}",
        group(int_part, i18n::thousands_separator()),
        i18n::decimal_separator()
    );
    with_currency(sign, &number, currency)
}

/// Montant abrégé pour les espaces restreints : `2,34 M€`, `52,6 k€`, sinon
/// le montant complet.
pub fn money_compact(amount: Amount, currency: &str) -> String {
    let value = amount.as_f64();
    let (number, suffix) = if value.abs() >= 1_000_000.0 {
        (format!("{:.2}", value.abs() / 1_000_000.0), "M")
    } else if value.abs() >= 10_000.0 {
        (format!("{:.1}", value.abs() / 1_000.0), "k")
    } else {
        return money(amount, currency);
    };
    let number = number.replace('.', &i18n::decimal_separator().to_string());
    let sign = if value < 0.0 { "-" } else { "" };
    if i18n::lang() == Lang::En {
        with_currency(sign, &format!("{number}{suffix}"), currency)
    } else {
        format!("{sign}{number}\u{a0}{suffix}{currency}")
    }
}

/// Icône Phosphor d'une devise (`€`, `$`, `£`…), sinon des billets.
pub fn currency_icon(currency: &str) -> &'static str {
    use egui_phosphor::regular as icon;
    match currency.trim() {
        "€" | "EUR" => icon::CURRENCY_EUR,
        "$" | "USD" => icon::CURRENCY_DOLLAR,
        "£" | "GBP" => icon::CURRENCY_GBP,
        "¥" | "JPY" => icon::CURRENCY_JPY,
        _ => icon::MONEY,
    }
}

/// Nombre entier avec séparateur des milliers : `159 037`, `159,037`.
pub fn count(n: usize) -> String {
    group(&n.to_string(), i18n::thousands_separator())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_compact_money() {
        assert_eq!(
            money_compact(Amount::from_cents(234_100_000), "€"),
            "2,34\u{a0}M€"
        );
        assert_eq!(
            money_compact(Amount::from_cents(-5_262_734), "€"),
            "-52,6\u{a0}k€"
        );
        assert_eq!(
            money_compact(Amount::from_cents(857_907), "€"),
            "8\u{202f}579,07\u{a0}€"
        );
    }

    #[test]
    fn formats_count() {
        assert_eq!(count(7), "7");
        assert_eq!(count(159_037), "159\u{202f}037");
    }

    #[test]
    fn formats_in_english() {
        i18n::with_lang(Lang::En, || {
            assert_eq!(money(Amount::from_cents(-123_456), "€"), "-€1,234.56");
            assert_eq!(money(Amount::from_cents(5), ""), "0.05");
            assert_eq!(money(Amount::from_cents(150), "CHF"), "1.50\u{a0}CHF");
            assert_eq!(
                money_compact(Amount::from_cents(-5_262_734), "$"),
                "-$52.6k"
            );
            assert_eq!(count(159_037), "159,037");
        });
    }

    #[test]
    fn formats_money() {
        assert_eq!(
            money(Amount::from_cents(-123_456), "€"),
            "-1\u{202f}234,56\u{a0}€"
        );
        assert_eq!(money(Amount::from_cents(5), ""), "0,05");
        assert_eq!(
            money(Amount::from_cents(100_000_000), ""),
            "1\u{202f}000\u{202f}000,00"
        );
    }
}
