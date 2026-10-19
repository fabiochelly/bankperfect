//! Découpage et nettoyage des libellés bancaires.

use crate::filter::normalize;
use crate::i18n::{Lang, lang};
use crate::model::{Mode, StandardMode};

/// Mots d'un texte : minuscules, sans accents, lettres et chiffres seulement.
pub fn words(text: &str) -> Vec<String> {
    normalize(text)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Mots ajoutés par la banque ou sans valeur pour reconnaître un tiers.
const NOISE: &[&str] = &[
    "cb",
    "carte",
    "cartes",
    "prlv",
    "prelevement",
    "prelevements",
    "prelev",
    "sepa",
    "vir",
    "virement",
    "virements",
    "virt",
    "inst",
    "instantane",
    "recu",
    "recus",
    "emis",
    "faveur",
    "tiers",
    "paiement",
    "paiements",
    "achat",
    "facture",
    "fac",
    "ture",
    "du",
    "de",
    "des",
    "la",
    "le",
    "les",
    "et",
    "en",
    "au",
    "aux",
    "sur",
    "par",
    "pour",
    "sa",
    "sas",
    "sarl",
    "eurl",
    "sasu",
    "sci",
    "snc",
    "ref",
    "reference",
    "rum",
    "ics",
    "mandat",
    "date",
    "num",
    "numero",
    "www",
    "com",
    "fr",
    "http",
    "https",
    "sumup",
    "sq",
    "zettle",
    "izettle",
    "lyf",
    "payplug",
    "stripe",
    "dab",
    "retrait",
];

/// Mots ajoutés par les banques anglophones (Royaume-Uni, États-Unis,
/// Canada, Irlande, Australie) ou sans valeur pour reconnaître un tiers,
/// retirés en plus des précédents quand l'interface est en anglais.
const NOISE_EN: &[&str] = &[
    // Nature de l'opération.
    "card",
    "cards",
    "payment",
    "payments",
    "pmt",
    "pmts",
    "pymt",
    "pymnt",
    "purchase",
    "purchases",
    "debit",
    "debits",
    "contactless",
    "direct",
    "standing",
    "order",
    "faster",
    "transfer",
    "transfers",
    "tfr",
    "trf",
    "xfer",
    "bill",
    "withdrawal",
    "withdrawals",
    "atm",
    "pos",
    "dda",
    "ach",
    "ppd",
    "ccd",
    "web",
    "zelle",
    "fpi",
    "fpo",
    "bgc",
    "sto",
    "ddr",
    "chq",
    "chk",
    "cheque",
    "check",
    "checkcard",
    "recurring",
    "preauthorized",
    "authorized",
    "online",
    "ending",
    "txn",
    "trx",
    "ref",
    "id",
    "conf",
    "confirmation",
    // Formes de société.
    "ltd",
    "limited",
    "inc",
    "llc",
    "llp",
    "plc",
    "corp",
    "co",
    "company",
    "pty",
    // Mots outils.
    "the",
    "of",
    "and",
    "to",
    "on",
    "at",
    "from",
    "for",
    "by",
    "via",
    // Mois des dates écrites.
    "jan",
    "feb",
    "mar",
    "apr",
    "may",
    "jun",
    "jul",
    "aug",
    "sep",
    "sept",
    "oct",
    "nov",
    "dec",
    // Pays et sites.
    "uk",
    "gb",
    "gbr",
    "usa",
    "us",
    "ca",
    "au",
    "aus",
    "ie",
    "irl",
    "net",
    "org",
];

/// Codes de nature en tête des relevés anglophones (« DD » : direct debit,
/// « DEB » : débit, « CR » : crédit), trop courts pour être retirés
/// ailleurs. « BP » (bill payment, mais aussi la marque de carburant) et
/// « SO » (standing order, mais aussi So Energy) restent : ce sont des noms
/// au moins aussi souvent.
const LEADING_EN: &[&str] = &["dd", "vis", "deb", "dr", "cr", "cd"];

/// Mots significatifs d'un libellé (sans les préfixes bancaires, références,
/// dates et numéros de carte).
pub fn significant_words(text: &str) -> Vec<String> {
    let english = lang() == Lang::En;
    words(text)
        .into_iter()
        .enumerate()
        .filter(|(i, w)| {
            w.len() >= 2
                && !w.chars().any(|c| c.is_ascii_digit())
                && !NOISE.contains(&w.as_str())
                && !(english
                    && (NOISE_EN.contains(&w.as_str())
                        || (*i == 0 && LEADING_EN.contains(&w.as_str()))))
        })
        .map(|(_, w)| w)
        .collect()
}

/// Nature d'une opération : d'après le préfixe du libellé (le plus fiable,
/// les synchronisations devinant mal le mode), sinon d'après le mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nature {
    Card,
    DirectDebit,
    Transfer,
    Withdrawal,
    Cheque,
}

impl Nature {
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "cb" => Self::Card,
            "prlv" => Self::DirectDebit,
            "vir" => Self::Transfer,
            "dab" => Self::Withdrawal,
            "chq" => Self::Cheque,
            _ => return None,
        })
    }

    pub fn from_label(payee: &str, mode: Option<&Mode>) -> Option<Self> {
        let w = words(payee);
        let first = w.first().map(String::as_str).unwrap_or("");
        let second = w.get(1).map(String::as_str).unwrap_or("");
        let from_text = match first {
            "cb" | "carte" => Some(Self::Card),
            "paiement" | "facture" if matches!(second, "cb" | "carte") => Some(Self::Card),
            "prlv" | "prelevement" | "prelev" => Some(Self::DirectDebit),
            "vir" | "virement" | "virt" => Some(Self::Transfer),
            "retrait" | "dab" => Some(Self::Withdrawal),
            "chq" | "cheque" | "remise" if first != "remise" || second.starts_with("cheq") => {
                Some(Self::Cheque)
            }
            _ => None,
        };
        let from_text = from_text.or_else(|| {
            if lang() == Lang::En {
                Self::from_english_label(first, second)
            } else {
                None
            }
        });
        from_text.or(match mode {
            Some(Mode::Standard(m)) => match m {
                StandardMode::Carte => Some(Self::Card),
                StandardMode::Prelevement | StandardMode::Tip => Some(Self::DirectDebit),
                StandardMode::VirementEmis | StandardMode::VirementRecu => Some(Self::Transfer),
                StandardMode::RetraitDab => Some(Self::Withdrawal),
                StandardMode::ChequeEmis | StandardMode::DepotCheque => Some(Self::Cheque),
                StandardMode::Versement | StandardMode::Titre => None,
            },
            _ => None,
        })
    }

    /// Préfixes des relevés anglophones (« CARD PAYMENT TO », « DD »,
    /// « STANDING ORDER », « FPO », « ATM WITHDRAWAL », « CHQ »…). Seulement
    /// en anglais : « SO BIO » est une enseigne française, pas un ordre
    /// permanent.
    fn from_english_label(first: &str, second: &str) -> Option<Self> {
        Some(match (first, second) {
            ("pos" | "vis" | "card" | "contactless" | "checkcard" | "purchase", _) => Self::Card,
            ("debit" | "visa" | "mastercard" | "interac", "card" | "purchase" | "pos") => {
                Self::Card
            }
            ("dd" | "ddr", _) => Self::DirectDebit,
            ("direct", "debit") => Self::DirectDebit,
            ("ach", "credit") => Self::Transfer,
            ("ach", _) => Self::DirectDebit,
            ("direct", "deposit" | "dep") => Self::Transfer,
            (
                "so" | "sto" | "bgc" | "fpi" | "fpo" | "tfr" | "trf" | "xfer" | "transfer"
                | "zelle" | "wire" | "interac",
                _,
            ) => Self::Transfer,
            ("standing", "order") | ("faster", "payment" | "payments") => Self::Transfer,
            ("bill", "payment" | "pay" | "pmt") | ("bank", "giro" | "transfer") => Self::Transfer,
            ("online", "transfer" | "banking") => Self::Transfer,
            ("atm" | "withdrawal", _) | ("cash", "withdrawal" | "machine") => Self::Withdrawal,
            ("chq" | "cheque" | "check" | "chk", _) => Self::Cheque,
            _ => return None,
        })
    }
}

/// Clés d'un libellé pour l'historique : tous les mots significatifs du
/// tiers (clé « complète ») et les trois premiers (clé « courte »). Un tiers
/// vide ou réduit au bruit bancaire laisse la place au détail.
pub fn keys(payee: &str, details: &str) -> Option<(String, String)> {
    let mut w = significant_words(payee);
    if w.is_empty() {
        w = significant_words(details);
    }
    if w.is_empty() {
        return None;
    }
    w.truncate(8);
    let full = w.join(" ");
    w.truncate(3);
    Some((full, w.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_ignore_bank_noise() {
        assert_eq!(
            keys("CB CARREFOUR MARKET 12/10 CARTE X1234", ""),
            Some(("carrefour market".into(), "carrefour market".into()))
        );
        assert_eq!(
            keys("PRLV SEPA Free Telecom", "Free HautDebit 1506832701"),
            Some(("free telecom".into(), "free telecom".into()))
        );
        assert_eq!(
            keys("VIR", "Loyer octobre"),
            Some(("loyer octobre".into(), "loyer octobre".into()))
        );
        assert_eq!(keys("", ""), None);
    }

    #[test]
    fn nature_from_label_then_mode() {
        let card = Mode::Standard(StandardMode::Carte);
        assert_eq!(
            Nature::from_label("PRLV SEPA EDF", Some(&card)),
            Some(Nature::DirectDebit)
        );
        assert_eq!(Nature::from_label("EDF", Some(&card)), Some(Nature::Card));
        assert_eq!(
            Nature::from_label("Paiement CB Fnac", None),
            Some(Nature::Card)
        );
        assert_eq!(Nature::from_label("Remise Ventes", None), None);
        // Les préfixes anglais ne valent qu'en anglais.
        assert_eq!(Nature::from_label("SO BIO", None), None);
    }

    #[test]
    fn english_statements() {
        use crate::i18n::with_lang;
        with_lang(Lang::En, || {
            assert_eq!(
                keys("CARD PAYMENT TO TESCO STORES 2345 ON 12 MAR", ""),
                Some(("tesco stores".into(), "tesco stores".into()))
            );
            assert_eq!(
                keys("POS DEBIT STARBUCKS #1234 SEATTLE WA", ""),
                Some(("starbucks seattle wa".into(), "starbucks seattle wa".into()))
            );
            assert_eq!(
                keys("DIRECT DEBIT BRITISH GAS", ""),
                Some(("british gas".into(), "british gas".into()))
            );
            assert_eq!(
                keys("SALARY ACME LTD", ""),
                Some(("salary acme".into(), "salary acme".into()))
            );
            assert_eq!(
                keys("DD THAMES WATER", "").map(|k| k.0),
                Some("thames water".into())
            );
            assert_eq!(
                keys("CARD PAYMENT TO BP CONNECT", "").map(|k| k.0),
                Some("bp connect".into())
            );
            let cases = [
                ("CARD PAYMENT TO TESCO", Some(Nature::Card)),
                ("POS DEBIT STARBUCKS", Some(Nature::Card)),
                ("DEBIT CARD PURCHASE AMAZON", Some(Nature::Card)),
                ("CONTACTLESS PRET A MANGER", Some(Nature::Card)),
                ("DD BRITISH GAS", Some(Nature::DirectDebit)),
                ("DIRECT DEBIT THAMES WATER", Some(Nature::DirectDebit)),
                ("ACH DEBIT GEICO", Some(Nature::DirectDebit)),
                ("STANDING ORDER LANDLORD", Some(Nature::Transfer)),
                ("FASTER PAYMENT J SMITH", Some(Nature::Transfer)),
                ("BGC ACME LTD", Some(Nature::Transfer)),
                ("ZELLE TO JOHN", Some(Nature::Transfer)),
                ("ATM WITHDRAWAL", Some(Nature::Withdrawal)),
                ("CASH WITHDRAWAL HSBC", Some(Nature::Withdrawal)),
                ("CHEQUE 000123", Some(Nature::Cheque)),
                ("CHECK 1042", Some(Nature::Cheque)),
                ("NETFLIX.COM", None),
            ];
            for (label, nature) in cases {
                assert_eq!(Nature::from_label(label, None), nature, "{label}");
            }
        });
        // En français, les mots anglais restent des mots du tiers.
        assert_eq!(
            keys("CARD PAYMENT TO TESCO", "").map(|k| k.0),
            Some("card payment to tesco".into())
        );
    }
}
