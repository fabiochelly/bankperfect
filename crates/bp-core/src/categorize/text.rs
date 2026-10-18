//! Découpage et nettoyage des libellés bancaires.

use crate::filter::normalize;
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

/// Mots significatifs d'un libellé (sans les préfixes bancaires, références,
/// dates et numéros de carte).
pub fn significant_words(text: &str) -> Vec<String> {
    words(text)
        .into_iter()
        .filter(|w| {
            w.len() >= 2 && !w.chars().any(|c| c.is_ascii_digit()) && !NOISE.contains(&w.as_str())
        })
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
    }
}
