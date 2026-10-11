//! Formatage des montants et des dates pour l'affichage (français fixe).

use bp_core::model::Amount;

/// `-1 234,56 €` : séparateur de milliers insécable, virgule décimale.
pub fn money(amount: Amount, currency: &str) -> String {
    let plain = amount.format(2, ',');
    let (sign, rest) = match plain.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", plain.as_str()),
    };
    let (int_part, frac) = rest.split_once(',').unwrap_or((rest, "00"));
    let mut grouped = String::with_capacity(int_part.len() + int_part.len() / 3 * 2);
    for (i, c) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            grouped.push('\u{a0}');
        }
        grouped.push(c);
    }
    if currency.is_empty() {
        format!("{sign}{grouped},{frac}")
    } else {
        format!("{sign}{grouped},{frac}\u{a0}{currency}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_money() {
        assert_eq!(money(Amount::from_cents(-123_456), "€"), "-1\u{a0}234,56\u{a0}€");
        assert_eq!(money(Amount::from_cents(5), ""), "0,05");
        assert_eq!(money(Amount::from_cents(100_000_000), ""), "1\u{a0}000\u{a0}000,00");
    }
}
