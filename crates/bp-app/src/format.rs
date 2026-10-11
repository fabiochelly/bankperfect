//! Formatage des montants et des dates pour l'affichage (français fixe).

use bp_core::model::Amount;

/// `-1 234,56 €` : espace fine insécable entre les milliers, virgule décimale.
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
            grouped.push('\u{202f}');
        }
        grouped.push(c);
    }
    if currency.is_empty() {
        format!("{sign}{grouped},{frac}")
    } else {
        format!("{sign}{grouped},{frac}\u{a0}{currency}")
    }
}

/// Montant abrégé pour les espaces restreints : `2,34 M€`, `52,6 k€`, sinon
/// le montant complet.
pub fn money_compact(amount: Amount, currency: &str) -> String {
    let value = amount.as_f64();
    let unit = |suffix: &str| {
        if currency.is_empty() {
            suffix.to_string()
        } else {
            format!("{suffix}{currency}")
        }
    };
    let text = if value.abs() >= 1_000_000.0 {
        format!("{:.2}\u{a0}{}", value / 1_000_000.0, unit("M"))
    } else if value.abs() >= 10_000.0 {
        format!("{:.1}\u{a0}{}", value / 1_000.0, unit("k"))
    } else {
        return money(amount, currency);
    };
    text.replace('.', ",")
}

/// Nombre entier avec espace fine insécable entre les milliers : `159 037`.
pub fn count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 * 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
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
