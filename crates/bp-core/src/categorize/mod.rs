//! Catégorisation automatique des opérations.
//!
//! Les indices sont consultés du plus fiable au plus coûteux :
//!
//! 1. **historique** du fichier : le même tiers (clé nettoyée des préfixes
//!    bancaires, dates et références), en privilégiant les opérations
//!    récentes et de montant proche ; à défaut, les mots du libellé pondérés
//!    par leur rareté ;
//! 2. **code commerçant** (MCC) fourni par la synchronisation bancaire ;
//! 3. **dictionnaire** intégré des enseignes et mots génériques
//!    ([`dictionary`]), qui désigne une catégorie générique ([`taxonomy`])
//!    reliée à une catégorie de l'utilisateur, apprise sur l'historique ou
//!    devinée d'après les noms ;
//! 4. **réponses de l'IA** déjà obtenues ([`ai`]), seulement si
//!    l'utilisateur l'a demandé.
//!
//! Les règles automatiques ([`crate::autofields`]) restent prioritaires :
//! l'application ne suggère que pour les opérations qu'elles laissent sans
//! catégorie. Rien n'est stocké pour l'historique : l'index se reconstruit
//! en quelques millisecondes et suit donc chaque correction.

pub mod ai;
pub mod dictionary;
pub mod mcc;
pub mod taxonomy;
pub mod text;

use std::collections::{HashMap, HashSet};

use crate::i18n::Lang;
use crate::model::{Amount, Category, Document, Mode, Operation};
use crate::{tr, trn};
use dictionary::Dictionary;
use taxonomy::{Flow, GENERICS, Generic};
use text::{Nature, keys, significant_words, words};

/// Confiance à partir de laquelle une catégorie est appliquée sans
/// confirmation (environ 9 fois sur 10 juste sur un historique réel).
pub const AUTO_THRESHOLD: f32 = 0.8;

/// Confiance minimale d'une proposition.
const MIN_CONFIDENCE: f32 = 0.2;

/// Origine d'une proposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    History,
    Words,
    MerchantCode,
    Merchant,
    Ai,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Self::History => tr!("historique"),
            Self::Words => tr!("mots du libellé"),
            Self::MerchantCode => tr!("code commerçant"),
            Self::Merchant => tr!("commerçant reconnu"),
            Self::Ai => tr!("IA"),
        }
    }

    /// Indices indépendants les uns des autres (pour cumuler les accords).
    fn family(self) -> u8 {
        match self {
            Self::History | Self::Words => 0,
            Self::MerchantCode | Self::Merchant => 1,
            Self::Ai => 2,
        }
    }
}

/// Catégorie proposée.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub category: u32,
    /// Entre 0 et 1.
    pub confidence: f32,
    pub source: Source,
    /// Explication courte (« 12 opérations « carrefour market » »).
    pub reason: String,
    /// Catégorie générique reconnue (dictionnaire, code commerçant).
    pub generic: Option<&'static str>,
}

/// Propositions pour une opération, la meilleure d'abord (trois au plus).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Suggestion {
    pub candidates: Vec<Candidate>,
}

impl Suggestion {
    pub fn best(&self) -> Option<&Candidate> {
        self.candidates.first()
    }

    /// Catégorie assez sûre pour être appliquée sans confirmation.
    pub fn confident(&self) -> Option<u32> {
        self.best()
            .filter(|c| c.confidence >= AUTO_THRESHOLD)
            .map(|c| c.category)
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

/// Opération à catégoriser.
#[derive(Debug, Clone, Copy)]
pub struct Query<'a> {
    pub payee: &'a str,
    pub details: &'a str,
    /// Montant signé ; zéro s'il n'est pas encore connu (saisie en cours).
    pub amount: Amount,
    pub mode: Option<&'a Mode>,
    /// Code commerçant (MCC) d'un paiement par carte.
    pub merchant_code: Option<u16>,
}

impl<'a> Query<'a> {
    pub fn of(op: &'a Operation) -> Self {
        Self {
            payee: &op.payee,
            details: &op.details,
            amount: op.amount,
            mode: Some(&op.mode),
            merchant_code: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    category: u32,
    amount: i64,
    date: i32,
}

/// Index des catégories déjà attribuées et correspondances entre catégories
/// génériques et catégories du fichier.
pub struct Categorizer {
    /// Catégories existantes, avec le nom de leur parent.
    categories: HashMap<u32, (String, Option<String>)>,
    samples: Vec<Sample>,
    by_full: HashMap<String, Vec<u32>>,
    by_key: HashMap<String, Vec<u32>>,
    /// Mot → (catégorie, recette) → nombre d'opérations.
    word_categories: HashMap<String, HashMap<(u32, bool), f32>>,
    word_documents: HashMap<String, f32>,
    worded_samples: f32,
    /// Catégorie générique → catégories choisies par l'utilisateur pour les
    /// opérations que le dictionnaire reconnaît.
    learned: HashMap<&'static str, HashMap<u32, f32>>,
    /// Catégorie générique → catégorie du fichier, d'après les noms.
    by_name: HashMap<&'static str, (u32, f32)>,
    /// Réponses de l'IA rangées dans le fichier : clé du tiers → catégorie.
    ai_answers: HashMap<String, u32>,
}

impl Categorizer {
    /// Index vide (aucun historique) : seuls les noms des catégories servent.
    ///
    /// Les noms appartiennent au fichier, pas à l'interface : un fichier
    /// français ouvert en anglais garde ses rapprochements. On compare donc
    /// les noms aux synonymes de la langue qu'ils semblent parler, ou des
    /// deux faute d'indice (les mots communs aux deux langues ne comptent
    /// pas) ; on évite ainsi les faux amis (« fuel », fioul en français).
    pub fn without_history(document: &Document) -> Self {
        let categories = category_names(&document.categories);
        let langs = name_languages(&document.categories);
        let by_name = GENERICS
            .iter()
            .filter_map(|g| best_by_name(g, &document.categories, &langs).map(|m| (g.slug, m)))
            .collect();
        let ai_answers = ai::answers(document);
        Self {
            categories,
            samples: Vec::new(),
            by_full: HashMap::new(),
            by_key: HashMap::new(),
            word_categories: HashMap::new(),
            word_documents: HashMap::new(),
            worded_samples: 0.0,
            learned: HashMap::new(),
            by_name,
            ai_answers,
        }
    }

    /// Index de tout le fichier.
    pub fn new(document: &Document) -> Self {
        let mut categorizer = Self::without_history(document);
        let mut ops: Vec<&Operation> = document
            .accounts
            .iter()
            .flat_map(|a| a.operations.iter())
            .filter(|op| op.category.is_some())
            .collect();
        ops.sort_by_key(|op| (op.date, op.uid));
        for op in ops {
            categorizer.learn(op);
        }
        categorizer
    }

    /// Ajoute une opération catégorisée à l'historique.
    pub fn learn(&mut self, op: &Operation) {
        let Some(category) = op.category.filter(|c| self.categories.contains_key(c)) else {
            return;
        };
        let index = self.samples.len() as u32;
        self.samples.push(Sample {
            category,
            amount: op.amount.0,
            date: op.date.0,
        });
        if let Some((full, key)) = keys(&op.payee, &op.details) {
            self.by_full.entry(full).or_default().push(index);
            self.by_key.entry(key).or_default().push(index);
        }
        let mut list = significant_words(&format!("{} {}", op.payee, op.details));
        list.sort();
        list.dedup();
        if !list.is_empty() {
            self.worded_samples += 1.0;
        }
        for w in list {
            *self.word_documents.entry(w.clone()).or_default() += 1.0;
            *self
                .word_categories
                .entry(w)
                .or_default()
                .entry((category, op.amount.0 > 0))
                .or_default() += 1.0;
        }
        let nature = Nature::from_label(&op.payee, Some(&op.mode));
        if let Some(m) = Dictionary::builtin().lookup(&op.payee, &op.details, op.amount, nature) {
            let slug = credit_slug(m.slug, op.amount);
            *self
                .learned
                .entry(slug)
                .or_default()
                .entry(category)
                .or_default() += 1.0;
        }
    }

    /// Nombre d'opérations apprises.
    pub fn history_len(&self) -> usize {
        self.samples.len()
    }

    /// Catégorie du fichier correspondant à une catégorie générique, avec la
    /// confiance du rapprochement.
    pub fn category_for_generic(&self, slug: &str) -> Option<(u32, f32)> {
        self.mapped(slug, 0)
    }

    fn mapped(&self, slug: &str, depth: u8) -> Option<(u32, f32)> {
        // Ce que l'utilisateur a fait des opérations reconnues l'emporte sur
        // les noms, même partagé entre plusieurs catégories (confiance basse).
        if let Some(counts) = self.learned.get(slug) {
            let total: f32 = counts.values().sum();
            if let Some((&category, &n)) = counts
                .iter()
                .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(a.0)))
                && total >= 2.0
            {
                let share = n / total;
                let confidence = if share >= 0.6 {
                    share * total / (total + 1.0)
                } else {
                    share * 0.8
                };
                return Some((category, confidence));
            }
        }
        if let Some(&found) = self.by_name.get(slug) {
            return Some(found);
        }
        if depth > 2 {
            return None;
        }
        let generic = taxonomy::generic(slug)?;
        if let Some(fallback) = generic.fallback
            && let Some((category, confidence)) = self.mapped(fallback, depth + 1)
        {
            return Some((category, confidence * 0.85));
        }
        let parent = taxonomy::parent_slug(slug);
        if parent != slug {
            return self
                .mapped(parent, depth + 1)
                .map(|(category, confidence)| (category, confidence * 0.7));
        }
        None
    }

    /// Propositions pour une opération.
    pub fn suggest(&self, query: &Query) -> Suggestion {
        let mut found: Vec<Candidate> = self.history_candidates(query);
        // Un tiers déjà catégorisé par l'utilisateur : le dictionnaire peut
        // confirmer son choix, mais pèse peu pour le contredire.
        let history_choice = found
            .first()
            .filter(|c| c.source == Source::History)
            .map(|c| c.category);
        let generic_weight = |category: u32| match history_choice {
            Some(chosen) if chosen != category => 0.6,
            _ => 1.0,
        };
        if let Some(code) = query.merchant_code
            && let Some(slug) = mcc::generic_for(code)
            && let Some((category, mapping)) = self.mapped(credit_slug(slug, query.amount), 0)
        {
            found.push(Candidate {
                category,
                confidence: 0.8 * mapping * generic_weight(category),
                source: Source::MerchantCode,
                reason: tr!("code commerçant {}", code),
                generic: Some(slug),
            });
        }
        let nature = Nature::from_label(query.payee, query.mode);
        if let Some(m) =
            Dictionary::builtin().lookup(query.payee, query.details, query.amount, nature)
            && let Some((category, mapping)) = self.mapped(credit_slug(m.slug, query.amount), 0)
        {
            found.push(Candidate {
                category,
                // Une famille entière (« impôts ») est moins précise qu'une
                // catégorie (« taxe foncière »).
                confidence: m.confidence
                    * mapping
                    * generic_weight(category)
                    * if m.slug.contains('.') { 1.0 } else { 0.8 },
                source: Source::Merchant,
                reason: tr!("« {} » reconnu", m.pattern),
                generic: Some(credit_slug(m.slug, query.amount)),
            });
        }
        if let Some((full, _)) = keys(query.payee, query.details)
            && let Some(&category) = self.ai_answers.get(&full)
        {
            found.push(Candidate {
                category,
                confidence: 0.7,
                source: Source::Ai,
                reason: tr!("proposé par l'IA").into(),
                generic: None,
            });
        }
        merge(found)
    }

    fn history_candidates(&self, query: &Query) -> Vec<Candidate> {
        if let Some((full, key)) = keys(query.payee, query.details) {
            for (map, wanted, support_bias, scale) in [
                (&self.by_full, &full, 0.5, 1.0),
                (&self.by_key, &key, 1.0, 0.95),
            ] {
                if let Some(list) = map.get(wanted) {
                    let votes = self.vote(list, query.amount);
                    if !votes.is_empty() {
                        let n = list.len().min(40) as f32;
                        let support = n / (n + support_bias) * scale;
                        let reason = trn!(
                            list.len(),
                            "{} opération « {1} »",
                            "{} opérations « {1} »",
                            wanted
                        );
                        return votes
                            .into_iter()
                            .map(|(category, share)| Candidate {
                                category,
                                confidence: share * support,
                                source: Source::History,
                                reason: reason.clone(),
                                generic: None,
                            })
                            .collect();
                    }
                }
            }
        }
        self.word_candidates(query)
    }

    /// Parts de chaque catégorie parmi les opérations du même tiers, les plus
    /// récentes et de montant proche comptant davantage.
    fn vote(&self, list: &[u32], amount: Amount) -> Vec<(u32, f32)> {
        let mut recent: Vec<Sample> = list
            .iter()
            .rev()
            .take(40)
            .map(|&i| self.samples[i as usize])
            .collect();
        recent.sort_by_key(|s| std::cmp::Reverse(s.date));
        let mut scores: HashMap<u32, f32> = HashMap::new();
        for (rank, s) in recent.iter().enumerate() {
            let weight = 0.9f32.powi(rank as i32) * amount_weight(s.amount, amount.0);
            *scores.entry(s.category).or_default() += weight;
        }
        shares(scores)
    }

    fn word_candidates(&self, query: &Query) -> Vec<Candidate> {
        let mut list = significant_words(&format!("{} {}", query.payee, query.details));
        list.sort();
        list.dedup();
        let mut scores: HashMap<u32, f32> = HashMap::new();
        // Part des indices venant d'opérations du même sens (1 si tous).
        let (mut same_sign, mut all_signs) = (0.0f32, 0.0f32);
        for w in &list {
            let (Some(counts), Some(&df)) =
                (self.word_categories.get(w), self.word_documents.get(w))
            else {
                continue;
            };
            let idf = (self.worded_samples / df).ln().max(0.0);
            // Un mot vu sur des recettes dit peu d'une dépense (le nom de
            // famille des salaires sur un virement émis, par exemple).
            let credit = query.amount.0 > 0;
            let weighted = |&(_, c): &(u32, bool), &n: &f32| {
                if query.amount.0 == 0 || c == credit {
                    n
                } else {
                    n * 0.15
                }
            };
            let total: f32 = counts.iter().map(|(k, n)| weighted(k, n)).sum();
            let raw: f32 = counts.values().sum();
            same_sign += idf * total / raw;
            all_signs += idf;
            for (key, n) in counts {
                *scores.entry(key.0).or_default() += idf * weighted(key, n) / total;
            }
        }
        let sign_factor = if all_signs > 0.0 {
            same_sign / all_signs
        } else {
            1.0
        };
        let total: f32 = scores.values().sum();
        let top = scores.values().copied().fold(0.0, f32::max);
        let weight = if top > 2.0 { 0.75 } else { 0.5 };
        if total <= 0.0 {
            return Vec::new();
        }
        shares(scores)
            .into_iter()
            .map(|(category, share)| Candidate {
                category,
                confidence: share * weight * sign_factor,
                source: Source::Words,
                reason: tr!("mots déjà vus").into(),
                generic: None,
            })
            .collect()
    }

    /// La catégorie existe toujours dans le fichier.
    pub fn knows(&self, category: u32) -> bool {
        self.categories.contains_key(&category)
    }
}

/// Le dictionnaire désigne des dépenses ; pour une recette, certaines
/// catégories renvoient vers un revenu (mutuelle → remboursements santé).
fn credit_slug(slug: &'static str, amount: Amount) -> &'static str {
    if amount.0 > 0
        && let Some(target) = taxonomy::generic(slug).and_then(|g| g.on_credit)
    {
        return target;
    }
    slug
}

fn amount_weight(sample: i64, wanted: i64) -> f32 {
    if wanted == 0 {
        return 1.0;
    }
    if (sample > 0) != (wanted > 0) {
        return 0.15;
    }
    let ln = |v: i64| ((v.unsigned_abs() as f64 / Amount::SCALE as f64) + 1.0).ln() as f32;
    1.0 / (1.0 + 2.0 * (ln(sample) - ln(wanted)).abs())
}

/// Scores → parts, la plus grande d'abord.
fn shares(scores: HashMap<u32, f32>) -> Vec<(u32, f32)> {
    let total: f32 = scores.values().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut list: Vec<(u32, f32)> = scores.into_iter().map(|(c, s)| (c, s / total)).collect();
    list.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    list
}

/// Regroupe les propositions par catégorie : un accord entre indices
/// indépendants renforce la confiance.
fn merge(found: Vec<Candidate>) -> Suggestion {
    let mut by_category: Vec<(Candidate, [f32; 3])> = Vec::new();
    for c in found {
        let family = c.source.family() as usize;
        match by_category
            .iter_mut()
            .find(|(k, _)| k.category == c.category)
        {
            Some((kept, families)) => {
                families[family] = families[family].max(c.confidence);
                if c.confidence > kept.confidence {
                    *kept = Candidate {
                        confidence: kept.confidence,
                        ..c
                    };
                }
            }
            None => {
                let mut families = [0.0; 3];
                families[family] = c.confidence;
                by_category.push((c, families));
            }
        }
    }
    let mut candidates: Vec<Candidate> = by_category
        .into_iter()
        .map(|(mut c, families)| {
            let mut sorted = families;
            sorted.sort_by(|a, b| b.total_cmp(a));
            let combined = sorted[0] + (1.0 - sorted[0]) * sorted[1] * 0.5;
            c.confidence = combined.min(0.99);
            c
        })
        .filter(|c| c.confidence >= MIN_CONFIDENCE)
        .collect();
    candidates.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    candidates.truncate(3);
    Suggestion { candidates }
}

fn category_names(categories: &[Category]) -> HashMap<u32, (String, Option<String>)> {
    let mut parent: Option<String> = None;
    let mut out = HashMap::new();
    for c in categories {
        if c.is_child {
            out.insert(c.id, (c.name.clone(), parent.clone()));
        } else {
            parent = Some(c.name.clone());
            out.insert(c.id, (c.name.clone(), None));
        }
    }
    out
}

/// Mots vides des noms de catégories, français et anglais (aucun n'est un
/// mot plein de l'autre langue ; « the » reste, c'est aussi le thé).
const NAME_STOP: &[&str] = &[
    "de", "des", "du", "la", "le", "les", "et", "d", "l", "a", "en", "au", "aux", "and", "of",
    "for", "to", "in", "on", "an", "s",
];

fn name_words(text: &str) -> Vec<String> {
    words(text)
        .into_iter()
        .filter(|w| !NAME_STOP.contains(&w.as_str()))
        .collect()
}

/// Même mot, au pluriel près, ou même début sur au moins sept lettres.
fn same_word(a: &str, b: &str) -> bool {
    let trim = |w: &str| -> String {
        w.strip_suffix('s')
            .or_else(|| w.strip_suffix('x'))
            .filter(|s| s.len() >= 3)
            .unwrap_or(w)
            .to_string()
    };
    let (a, b) = (trim(a), trim(b));
    a == b || (a.len() >= 7 && b.len() >= 7 && a[..7] == b[..7])
}

/// Ressemblance entre un synonyme et un nom de catégorie.
fn name_score(synonym: &[String], name: &[String]) -> f32 {
    if synonym.is_empty() || name.is_empty() {
        return 0.0;
    }
    if synonym == name {
        return 1.0;
    }
    let found = |from: &[String], to: &[String]| {
        from.iter()
            .filter(|w| to.iter().any(|x| same_word(w, x)))
            .count() as f32
            / from.len() as f32
    };
    let coverage_synonym = found(synonym, name);
    let coverage_name = found(name, synonym);
    if coverage_synonym == 1.0 && coverage_name == 1.0 {
        return 0.95;
    }
    if coverage_synonym < 1.0 {
        return 0.0;
    }
    0.5 + 0.35 * coverage_name
}

/// Langues des synonymes à comparer aux noms des catégories : celle dont les
/// synonymes reprennent exactement le plus de noms, ou les deux à égalité.
fn name_languages(categories: &[Category]) -> Vec<Lang> {
    let names: Vec<Vec<String>> = categories.iter().map(|c| name_words(&c.name)).collect();
    let hits = |lang: Lang| {
        let synonyms: HashSet<Vec<String>> = GENERICS
            .iter()
            .flat_map(|g| g.names_in(lang))
            .map(name_words)
            .collect();
        names.iter().filter(|n| synonyms.contains(*n)).count()
    };
    match hits(Lang::Fr).cmp(&hits(Lang::En)) {
        std::cmp::Ordering::Greater => vec![Lang::Fr],
        std::cmp::Ordering::Less => vec![Lang::En],
        std::cmp::Ordering::Equal => vec![Lang::Fr, Lang::En],
    }
}

/// Catégorie du fichier dont le nom ressemble le plus à une catégorie
/// générique.
fn best_by_name(generic: &Generic, categories: &[Category], langs: &[Lang]) -> Option<(u32, f32)> {
    let is_child_generic = generic.slug.contains('.');
    let parent_generic = taxonomy::generic(taxonomy::parent_slug(generic.slug));
    let names_of = |g: &Generic| -> Vec<Vec<String>> {
        langs
            .iter()
            .flat_map(|&lang| g.names_in(lang))
            .map(name_words)
            .collect()
    };
    let synonyms = names_of(generic);
    let parent_synonyms: Vec<Vec<String>> = parent_generic.map(names_of).unwrap_or_default();
    let mut parent_name: Option<Vec<String>> = None;
    let mut best: Option<(u32, f32)> = None;
    for c in categories {
        let name = name_words(&c.name);
        if !c.is_child {
            parent_name = Some(name.clone());
        }
        let mut score = synonyms
            .iter()
            .map(|s| name_score(s, &name))
            .fold(0.0, f32::max);
        if score == 0.0 {
            continue;
        }
        if is_child_generic {
            if c.is_child {
                let parent_matches = parent_name
                    .as_ref()
                    .is_some_and(|p| parent_synonyms.iter().any(|s| name_score(s, p) > 0.0));
                if parent_matches {
                    score += 0.1;
                }
            } else {
                score *= 0.85;
            }
        } else if c.is_child {
            score *= 0.6;
        }
        // Les revenus ne vont pas dans une catégorie de dépenses du même nom.
        if generic.flow == Flow::Credit && !c.is_child && score < 1.0 {
            score *= 0.9;
        }
        let score = score.min(1.0);
        if best.is_none_or(|(_, s)| score > s) {
            best = Some((c.id, score));
        }
    }
    best.filter(|&(_, s)| s >= 0.5)
}

/// Opération sans catégorie et ce que l'on propose pour elle.
#[derive(Debug, Clone)]
pub struct Proposal {
    pub account: usize,
    pub index: usize,
    pub suggestion: Suggestion,
}

/// Propositions pour les opérations sans catégorie parmi `targets`
/// (compte, indice).
pub fn propose(
    document: &Document,
    categorizer: &Categorizer,
    targets: impl IntoIterator<Item = (usize, usize)>,
) -> Vec<Proposal> {
    targets
        .into_iter()
        .filter_map(|(account, index)| {
            let op = document.accounts.get(account)?.operations.get(index)?;
            if op.category.is_some() {
                return None;
            }
            Some(Proposal {
                account,
                index,
                suggestion: categorizer.suggest(&Query::of(op)),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
