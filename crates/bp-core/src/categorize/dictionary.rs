//! Dictionnaires des commerçants (intégrés au programme) et recherche dans un
//! libellé : `commercants.txt` pour les relevés français, `merchants_en.txt`
//! (enseignes britanniques, américaines, canadiennes, irlandaises,
//! australiennes et internationales, mots génériques anglais) quand
//! l'interface est en anglais.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::taxonomy::{self, Flow};
use super::text::{Nature, words};
use crate::i18n::Lang;
use crate::model::Amount;

const SOURCE: &str = include_str!("commercants.txt");
const SOURCE_EN: &str = include_str!("merchants_en.txt");

/// Indice faible : nom ambigu ou mot courant.
pub const WEAK: f32 = 0.55;
/// Indice fort : enseigne ou mot sans ambiguïté.
pub const STRONG: f32 = 0.85;

#[derive(Debug, Clone)]
struct Entry {
    words: Vec<String>,
    /// Le dernier mot est un préfixe (`boulangeri*`).
    prefix: bool,
    slug: &'static str,
    weak: bool,
    debit_only: bool,
    credit_only: bool,
    /// Bornes du montant absolu, en dix-millièmes.
    min: Option<i64>,
    max: Option<i64>,
    nature: Option<Nature>,
    /// Ordre dans le fichier (départage stable).
    order: usize,
}

impl Entry {
    fn constraints(&self) -> usize {
        [
            self.debit_only || self.credit_only,
            self.min.is_some() || self.max.is_some(),
            self.nature.is_some(),
        ]
        .into_iter()
        .filter(|&b| b)
        .count()
    }

    fn accepts(&self, amount: Amount, nature: Option<Nature>) -> bool {
        let credit = amount.0 > 0;
        let abs = amount.0.abs();
        // Montant inconnu (saisie en cours) : seules les bornes sont ignorées.
        let known = amount.0 != 0;
        !(known && self.debit_only && credit)
            && !(known && self.credit_only && !credit)
            && !(known && self.min.is_some_and(|m| abs < m))
            && !(known && self.max.is_some_and(|m| abs > m))
            && self.nature.is_none_or(|n| nature == Some(n))
    }

    /// Le mot `k` du motif correspond au mot `word` du libellé.
    fn word_matches(&self, k: usize, word: &str) -> bool {
        let pattern = &self.words[k];
        if self.prefix && k + 1 == self.words.len() {
            word.starts_with(pattern.as_str())
        } else {
            word == pattern
        }
    }
}

/// Dictionnaire chargé.
pub struct Dictionary {
    entries: Vec<Entry>,
    /// Entrées par premier mot exact.
    by_first: HashMap<String, Vec<usize>>,
    /// Entrées d'un seul mot préfixe (`osteo*`).
    prefixed: Vec<usize>,
}

/// Erreur dans le fichier du dictionnaire (vérifiée par les tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryError {
    pub line: usize,
    pub message: String,
}

fn parse_euros(text: &str) -> Option<i64> {
    let value: f64 = text.replace(',', ".").parse().ok()?;
    Some((value * Amount::SCALE as f64).round() as i64)
}

impl Dictionary {
    pub fn parse(source: &str) -> Result<Self, DictionaryError> {
        let mut entries = Vec::new();
        let mut slug: Option<&'static str> = None;
        for (index, raw) in source.lines().enumerate() {
            let line = raw.trim();
            let error = |message: String| DictionaryError {
                line: index + 1,
                message,
            };
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                slug = Some(
                    taxonomy::generic(name)
                        .ok_or_else(|| error(format!("catégorie inconnue : {name}")))?
                        .slug,
                );
                continue;
            }
            let slug = slug.ok_or_else(|| error("entrée avant toute section".into()))?;
            let (pattern, options) = line.split_once(';').unwrap_or((line, ""));
            let mut list: Vec<String> = pattern.split_whitespace().map(str::to_string).collect();
            if list.is_empty() {
                return Err(error("motif vide".into()));
            }
            let mut prefix = false;
            if let Some(last) = list.last_mut()
                && let Some(stripped) = last.strip_suffix('*')
            {
                *last = stripped.to_string();
                prefix = true;
            }
            for w in &list {
                if w.is_empty()
                    || !w
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                {
                    return Err(error(format!("mot non normalisé : « {w} »")));
                }
            }
            let mut entry = Entry {
                words: std::mem::take(&mut list),
                prefix,
                slug,
                weak: false,
                debit_only: false,
                credit_only: false,
                min: None,
                max: None,
                nature: None,
                order: entries.len(),
            };
            for option in options.split_whitespace() {
                match option {
                    "~" => entry.weak = true,
                    "-" => entry.debit_only = true,
                    "+" => entry.credit_only = true,
                    _ if option.starts_with('@') => {
                        entry.nature = Some(
                            Nature::from_tag(&option[1..])
                                .ok_or_else(|| error(format!("nature inconnue : {option}")))?,
                        )
                    }
                    _ if option.starts_with('<') => entry.max = parse_euros(&option[1..]),
                    _ if option.starts_with('>') => entry.min = parse_euros(&option[1..]),
                    _ if option.contains("..") => {
                        let (a, b) = option.split_once("..").expect("..");
                        entry.min = parse_euros(a);
                        entry.max = parse_euros(b);
                        if entry.min.is_none() || entry.max.is_none() {
                            return Err(error(format!("bornes illisibles : {option}")));
                        }
                    }
                    _ => return Err(error(format!("option inconnue : {option}"))),
                }
                if (option.starts_with('<') && entry.max.is_none())
                    || (option.starts_with('>') && entry.min.is_none())
                {
                    return Err(error(format!("montant illisible : {option}")));
                }
            }
            if taxonomy::generic(slug).is_some_and(|g| g.flow == Flow::Credit) {
                if entry.debit_only {
                    return Err(error("dépense dans une section de recettes".into()));
                }
                entry.credit_only = true;
            }
            entries.push(entry);
        }
        let mut by_first: HashMap<String, Vec<usize>> = HashMap::new();
        let mut prefixed = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            if e.prefix && e.words.len() == 1 {
                prefixed.push(i);
            } else {
                by_first.entry(e.words[0].clone()).or_default().push(i);
            }
        }
        Ok(Self {
            entries,
            by_first,
            prefixed,
        })
    }

    /// Dictionnaire intégré de la langue en cours.
    pub fn builtin() -> &'static Dictionary {
        Self::builtin_for(crate::i18n::lang())
    }

    /// Dictionnaire intégré d'une langue.
    pub fn builtin_for(lang: Lang) -> &'static Dictionary {
        static FR: OnceLock<Dictionary> = OnceLock::new();
        static EN: OnceLock<Dictionary> = OnceLock::new();
        match lang {
            Lang::Fr => {
                FR.get_or_init(|| Dictionary::parse(SOURCE).expect("commercants.txt valide"))
            }
            Lang::En => {
                EN.get_or_init(|| Dictionary::parse(SOURCE_EN).expect("merchants_en.txt valide"))
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn hits(&self, tokens: &[String], amount: Amount, nature: Option<Nature>) -> Vec<usize> {
        let mut found = Vec::new();
        for start in 0..tokens.len() {
            let candidates = self
                .by_first
                .get(&tokens[start])
                .into_iter()
                .flatten()
                .chain(self.prefixed.iter());
            for &i in candidates {
                let e = &self.entries[i];
                let n = e.words.len();
                if start + n <= tokens.len()
                    && (0..n).all(|k| e.word_matches(k, &tokens[start + k]))
                    && e.accepts(amount, nature)
                {
                    found.push(i);
                }
            }
        }
        found
    }

    /// Meilleure entrée trouvée dans un texte : la plus longue, puis la plus
    /// sûre, puis la plus contrainte, puis la première du fichier.
    fn best_in(&self, tokens: &[String], amount: Amount, nature: Option<Nature>) -> Option<&Entry> {
        self.hits(tokens, amount, nature)
            .into_iter()
            .map(|i| &self.entries[i])
            .max_by_key(|e| {
                (
                    e.words.len(),
                    !e.weak,
                    e.constraints(),
                    std::cmp::Reverse(e.order),
                )
            })
    }

    /// Catégorie générique d'un libellé : d'abord le tiers, puis le détail ;
    /// un détail plus précis de la même famille complète le tiers.
    pub fn lookup(
        &self,
        payee: &str,
        details: &str,
        amount: Amount,
        nature: Option<Nature>,
    ) -> Option<Match> {
        let in_payee = self.best_in(&words(payee), amount, nature);
        let in_details = self.best_in(&words(details), amount, nature);
        let entry = match (in_payee, in_details) {
            (Some(p), Some(d))
                if d.slug != p.slug && d.slug.starts_with(&format!("{}.", p.slug)) =>
            {
                d
            }
            (Some(p), _) => p,
            (None, Some(d)) => d,
            (None, None) => return None,
        };
        Some(Match {
            slug: entry.slug,
            confidence: if entry.weak { WEAK } else { STRONG },
            pattern: entry.words.join(" "),
        })
    }
}

/// Résultat d'une recherche dans le dictionnaire.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub slug: &'static str,
    pub confidence: f32,
    /// Motif reconnu, pour l'explication affichée.
    pub pattern: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(payee: &str, details: &str, euros: i64) -> Option<&'static str> {
        let nature = Nature::from_label(payee, None);
        Dictionary::builtin()
            .lookup(payee, details, Amount::from_cents(euros * 100), nature)
            .map(|m| m.slug)
    }

    #[test]
    fn builtin_dictionary_parses() {
        let d = Dictionary::builtin();
        assert!(d.len() > 1500, "{}", d.len());
    }

    #[test]
    fn dictionary_follows_the_language() {
        let en = Dictionary::builtin_for(Lang::En);
        assert!(en.len() > 2000, "{}", en.len());
        let same = crate::i18n::with_lang(Lang::En, Dictionary::builtin);
        assert!(std::ptr::eq(same, en));
        assert!(!std::ptr::eq(Dictionary::builtin(), en));
    }

    fn find_en(payee: &str, details: &str, cents: i64) -> Option<&'static str> {
        crate::i18n::with_lang(Lang::En, || {
            let nature = Nature::from_label(payee, None);
            Dictionary::builtin()
                .lookup(payee, details, Amount::from_cents(cents), nature)
                .map(|m| m.slug)
        })
    }

    #[test]
    fn finds_english_merchants() {
        let cases = [
            (
                "CARD PAYMENT TO TESCO STORES 2345 ON 12 MAR",
                -4_520,
                "alimentation.courses",
            ),
            ("SAINSBURYS S/MKTS", -3_000, "alimentation.courses"),
            (
                "WHOLEFDS MKT 10234 AUSTIN TX",
                -6_712,
                "alimentation.courses",
            ),
            ("TRADER JOE S #552", -4_000, "alimentation.courses"),
            ("DIRECT DEBIT BRITISH GAS", -8_500, "logement.energie"),
            ("CON ED OF NY INTELL CK", -11_000, "logement.energie"),
            ("DD THAMES WATER", -3_800, "logement.eau"),
            (
                "POS DEBIT STARBUCKS #1234 SEATTLE WA",
                -575,
                "alimentation.restaurants",
            ),
            ("MCDONALD'S F12345", -899, "alimentation.restaurants"),
            ("UBER *EATS", -2_350, "alimentation.restaurants"),
            ("UBER *TRIP HELP.UBER.COM", -1_420, "transports.taxi"),
            ("NETFLIX.COM", -1_549, "abonnements.medias"),
            ("SPOTIFY USA", -1_099, "abonnements.medias"),
            ("APPLE.COM/BILL", -299, "abonnements.logiciels"),
            ("VERIZON WIRELESS PAYMENTS", -8_500, "abonnements.telecom"),
            ("AMAZON MKTPLACE PMTS", -2_999, "shopping.divers"),
            ("AMZN Mktp US*2K4", -1_850, "shopping.divers"),
            ("SHELL OIL 57444", -5_000, "transports.carburant"),
            ("BP CONNECT LONDON", -6_000, "transports.carburant"),
            ("TFL TRAVEL CHARGE", -280, "transports.commun"),
            ("CVS/PHARMACY #1234", -1_200, "sante.pharmacie"),
            ("BOOTS 1234 LONDON", -800, "sante.pharmacie"),
            ("GEICO AUTO", -12_000, "transports.assurance"),
            ("HMRC SELF ASSESSMENT", -150_000, "impots.revenu"),
            ("IRS USATAXPYMT", -200_000, "impots.revenu"),
            ("COUNCIL TAX LB CAMDEN", -15_000, "impots.foncier"),
            ("MONTHLY SERVICE FEE", -1_200, "banque.frais"),
            ("ATM WITHDRAWAL", -6_000, "banque.retraits"),
            ("ROCKET MORTGAGE", -180_000, "logement.credit"),
            ("STANDING ORDER RENT FLAT 2", -120_000, "logement.loyer"),
            ("SALARY ACME LTD", 300_000, "revenus.salaires"),
            ("ACME CORP PAYROLL PPD", 250_000, "revenus.salaires"),
            ("INTEREST PAID", 1_250, "revenus.financiers"),
            ("PETSMART #123", -4_500, "divers.animaux"),
            ("VANGUARD BUY INVESTMENT", -50_000, "epargne.bourse"),
        ];
        for (payee, cents, slug) in cases {
            assert_eq!(find_en(payee, "", cents), Some(slug), "{payee}");
        }
        // Les revenus ne valent que pour les recettes.
        assert_eq!(find_en("SALARY ACME LTD", "", -1_000), None);
        // Le dictionnaire français ne sert pas en anglais, et inversement.
        assert_eq!(find_en("CB CARREFOUR MARKET", "", -5_000), None);
        assert_eq!(find("CARD PAYMENT TO TESCO STORES", "", -45), None);
    }

    #[test]
    fn rejects_bad_lines() {
        assert!(Dictionary::parse("carrefour").is_err());
        assert!(Dictionary::parse("[inconnue]\nx").is_err());
        assert!(Dictionary::parse("[alimentation.courses]\nCarrefour").is_err());
        assert!(Dictionary::parse("[alimentation.courses]\nx ; <abc").is_err());
        assert!(Dictionary::parse("[revenus.salaires]\nx ; -").is_err());
    }

    #[test]
    fn finds_common_merchants() {
        assert_eq!(
            find("CB CARREFOUR MARKET 12/10", "", -54),
            Some("alimentation.courses")
        );
        assert_eq!(
            find("CB BOULANGERIE DU MARCHE", "", -4),
            Some("alimentation.commerces")
        );
        assert_eq!(
            find("CB BOULANGER 1234", "", -499),
            Some("shopping.hightech")
        );
        assert_eq!(
            find("PRLV SEPA EDF clients particuliers", "", -98),
            Some("logement.energie")
        );
        assert_eq!(
            find("PRLV SEPA Free Telecom", "", -30),
            Some("abonnements.telecom")
        );
        assert_eq!(
            find("PRLV SEPA LC FITNESS KREMLIN", "", -30),
            Some("loisirs.sports")
        );
        assert_eq!(
            find("PRLV SEPA GENERALI VIE SA", "", -500),
            Some("epargne.assurance_vie")
        );
        assert_eq!(
            find("VIR SEPA CPAM DES YVELINES", "", 24),
            Some("revenus.sante")
        );
        assert_eq!(
            find("CB UBER EATS", "", -24),
            Some("alimentation.restaurants")
        );
        assert_eq!(find("CB UBER TRIP", "", -14), Some("transports.taxi"));
        assert_eq!(
            find("CB TOTAL ACCESS", "", -60),
            Some("transports.carburant")
        );
        assert_eq!(
            find("PRLV SEPA TOTALENERGIES", "", -60),
            Some("logement.energie")
        );
        assert_eq!(
            find("CB PHARMACIE DE LA GARE", "", -12),
            Some("sante.pharmacie")
        );
    }

    #[test]
    fn details_refine_or_replace() {
        assert_eq!(
            find(
                "PRLV SEPA DIRECTION GENERALE DES FINANCE",
                "PRELEVEMENT A LA SOURCE REVENUS 2026",
                -1000
            ),
            Some("impots.revenu")
        );
        assert_eq!(
            find(
                "PRLV SEPA DIRECTION GENERALE DES FINANCE",
                "IMPOT 7806",
                -279
            ),
            Some("impots")
        );
        assert_eq!(
            find("VIR INST HELENE DUPONT", "Kine", -60),
            Some("sante.paramedical")
        );
    }

    #[test]
    fn amount_and_sign_constraints() {
        // Une boulangerie ne vend pas pour 300 €.
        assert_eq!(find("CB BOULANGERIE", "", -300), None);
        // Les revenus ne s'appliquent qu'aux recettes.
        assert_eq!(find("VIR SALAIRE OCTOBRE", "", -100), None);
        assert_eq!(
            find("VIR SALAIRE OCTOBRE", "", 2500),
            Some("revenus.salaires")
        );
        assert_eq!(
            find("AVANTAGES OFFRE METAL", "", 20),
            Some("revenus.autres")
        );
        assert_eq!(
            find("*FRAIS OFFRE METAL 10/2026", "", -10),
            Some("banque.frais")
        );
    }
}
