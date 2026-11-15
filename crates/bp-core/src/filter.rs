//! Filtres de la grille des opérations.
//!
//! Reprend la syntaxe de l'ancienne version (`UnitFilterString.pas`,
//! inventaire 01 §9) : mots = ET, `|` = OU, `/mot` = NON, `(mot` = début de
//! mot, `mot)` = fin de mot, `"une expression"`. Corrections : `=texte` est
//! une égalité stricte et `//` désigne un champ vide (C15, C16).
//!
//! Pour rester instantané sur des dizaines d'années d'écritures, les textes
//! normalisés de chaque opération sont calculés une fois ([`SearchIndex`]) ;
//! filtrer revient ensuite à des recherches de sous-chaînes sans allocation.

use crate::model::{Account, Amount, Date, Document, Operation, Tack};
use crate::tr;

/// Normalisation `SimpleString` : minuscules, accents retirés, chiffres et
/// virgule conservés, tout autre caractère remplacé par une espace.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        for lower in c.to_lowercase() {
            out.push(match fold_accent(lower) {
                c @ ('a'..='z' | '0'..='9' | ',') => c,
                _ => ' ',
            });
        }
    }
    out
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'œ' => 'o',
        'æ' => 'a',
        c => c,
    }
}

/// Textes normalisés d'une opération, encadrés d'espaces pour les tests de
/// début/fin de mot.
#[derive(Debug, Clone)]
pub struct IndexedOperation {
    mode: String,
    payee: String,
    category: String,
}

/// Index de recherche d'un compte, aligné sur `account.operations`.
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    rows: Vec<IndexedOperation>,
}

impl SearchIndex {
    pub fn build(document: &Document, account: &Account) -> Self {
        let ops = &account.operations;
        let index = |ops: &[Operation]| -> Vec<IndexedOperation> {
            ops.iter().map(|op| index_operation(document, op)).collect()
        };
        // Gros comptes : réparti sur les cœurs (normaliser le texte de
        // 100 000 opérations prend une trentaine de millisecondes sur un seul).
        const CHUNK: usize = 8_192;
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        if ops.len() < 2 * CHUNK || threads < 2 {
            return Self { rows: index(ops) };
        }
        let chunk = ops.len().div_ceil(threads).max(CHUNK);
        let rows = std::thread::scope(|scope| {
            let parts: Vec<_> = ops
                .chunks(chunk)
                .map(|part| scope.spawn(move || index(part)))
                .collect();
            let mut rows = Vec::with_capacity(ops.len());
            for part in parts {
                rows.extend(part.join().expect("index de recherche"));
            }
            rows
        });
        Self { rows }
    }

    pub fn update(&mut self, document: &Document, account: &Account, index: usize) {
        let row = index_operation(document, &account.operations[index]);
        if index < self.rows.len() {
            self.rows[index] = row;
        } else {
            self.rows.push(row);
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

fn padded(text: &str) -> String {
    let normalized = normalize(text);
    let mut out = String::with_capacity(normalized.len() + 2);
    out.push(' ');
    out.push_str(normalized.trim());
    out.push(' ');
    out
}

fn index_operation(document: &Document, op: &Operation) -> IndexedOperation {
    let mut payee = format!("{} {}", op.payee, op.details);
    for (key, value) in &op.extra {
        payee.push(' ');
        payee.push_str(key);
        payee.push(' ');
        payee.push_str(value);
    }
    IndexedOperation {
        mode: padded(&op.mode_label()),
        payee: padded(&payee),
        category: padded(
            &op.category
                .and_then(|id| document.category_label(id))
                .unwrap_or_default(),
        ),
    }
}

/// Filtre de pointage (panneau de la colonne P) : les états de pointage
/// affichés, cochés indépendamment (au moins un).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "TackFilterRepr", into = "TackFilterRepr")]
pub struct TackFilter {
    pub not_pointed: bool,
    pub pointed: bool,
    pub reconciled: bool,
}

impl Default for TackFilter {
    fn default() -> Self {
        Self::ALL
    }
}

impl TackFilter {
    pub const NOT_POINTED: Self = Self::new(true, false, false);
    pub const POINTED: Self = Self::new(false, true, false);
    pub const RECONCILED: Self = Self::new(false, false, true);
    pub const POINTED_OR_RECONCILED: Self = Self::new(false, true, true);
    /// Non pointées et pointées : tout sauf les rapprochées.
    pub const NOT_RECONCILED: Self = Self::new(true, true, false);
    pub const NOT_POINTED_OR_RECONCILED: Self = Self::new(true, false, true);
    pub const ALL: Self = Self::new(true, true, true);

    pub const fn new(not_pointed: bool, pointed: bool, reconciled: bool) -> Self {
        Self {
            not_pointed,
            pointed,
            reconciled,
        }
    }

    /// L'état `tack` est-il coché ?
    pub fn shows(self, tack: Tack) -> bool {
        match tack {
            Tack::None => self.not_pointed,
            Tack::Pointed => self.pointed,
            Tack::Reconciled => self.reconciled,
        }
    }

    /// Coche ou décoche l'état `tack`. Décocher le dernier état coché est
    /// refusé (faux) : aucune ligne ne serait plus affichée.
    pub fn set(&mut self, tack: Tack, on: bool) -> bool {
        let mut next = *self;
        match tack {
            Tack::None => next.not_pointed = on,
            Tack::Pointed => next.pointed = on,
            Tack::Reconciled => next.reconciled = on,
        }
        if !(next.not_pointed || next.pointed || next.reconciled) {
            return false;
        }
        *self = next;
        true
    }

    /// Pointage simple (sans rapprochement) : une ligne est pointée ou non,
    /// qu'elle soit enregistrée « pointée » ou « rapprochée ».
    pub fn simple_pointed(self) -> bool {
        self.pointed || self.reconciled
    }

    /// Pointage simple : « Pointées » vaut pour les lignes enregistrées
    /// pointées comme rapprochées (un filtre « Pointées » seul ne garderait
    /// sinon presque rien, les lignes pointées y étant rapprochées).
    pub fn simplified(self) -> Self {
        let pointed = self.simple_pointed();
        Self::new(self.not_pointed, pointed, pointed)
    }

    /// Aucun état n'est écarté (pour le mode de pointage indiqué).
    pub fn is_all(self, simple: bool) -> bool {
        if simple {
            self.not_pointed && self.simple_pointed()
        } else {
            self == Self::ALL
        }
    }

    /// Pointage simple : coche ou décoche « Pointées » (pointées et
    /// rapprochées ensemble) ; refusé si plus rien ne serait affiché.
    pub fn set_simple_pointed(&mut self, on: bool) -> bool {
        if !on && !self.not_pointed {
            return false;
        }
        self.pointed = on;
        self.reconciled = on;
        true
    }

    /// Libellé du filtre ; `simple` : pointage simple, sans rapprochement.
    pub fn label(self, simple: bool) -> &'static str {
        if simple {
            return match (self.not_pointed, self.simple_pointed()) {
                (true, true) => tr!("Toutes"),
                (true, false) => tr!("Non pointées"),
                _ => tr!("Pointées"),
            };
        }
        match (self.not_pointed, self.pointed, self.reconciled) {
            (true, false, false) => tr!("Non pointées"),
            (false, true, false) => tr!("Pointées"),
            (false, false, true) => tr!("Rapprochées"),
            (false, true, true) => tr!("Pointées et rapprochées"),
            (true, true, false) => tr!("Non rapprochées"),
            (true, false, true) => tr!("Non pointées et rapprochées"),
            _ => tr!("Toutes"),
        }
    }
}

/// Forme enregistrée (filtres prédéfinis du document) : les noms des
/// anciennes versions, que celles-ci relisent, et deux nouveaux.
#[derive(serde::Serialize, serde::Deserialize)]
enum TackFilterRepr {
    NotPointed,
    Pointed,
    Reconciled,
    PointedOrReconciled,
    NotReconciled,
    NotPointedOrReconciled,
    All,
}

impl From<TackFilterRepr> for TackFilter {
    fn from(repr: TackFilterRepr) -> Self {
        match repr {
            TackFilterRepr::NotPointed => Self::NOT_POINTED,
            TackFilterRepr::Pointed => Self::POINTED,
            TackFilterRepr::Reconciled => Self::RECONCILED,
            TackFilterRepr::PointedOrReconciled => Self::POINTED_OR_RECONCILED,
            TackFilterRepr::NotReconciled => Self::NOT_RECONCILED,
            TackFilterRepr::NotPointedOrReconciled => Self::NOT_POINTED_OR_RECONCILED,
            TackFilterRepr::All => Self::ALL,
        }
    }
}

impl From<TackFilter> for TackFilterRepr {
    fn from(filter: TackFilter) -> Self {
        match (filter.not_pointed, filter.pointed, filter.reconciled) {
            (true, false, false) => Self::NotPointed,
            (false, true, false) => Self::Pointed,
            (false, false, true) => Self::Reconciled,
            (false, true, true) => Self::PointedOrReconciled,
            (true, true, false) => Self::NotReconciled,
            (true, false, true) => Self::NotPointedOrReconciled,
            _ => Self::All,
        }
    }
}

/// État des filtres tel que saisi par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filters {
    pub mode: String,
    pub payee: String,
    pub category: String,
    pub amount: String,
    pub tack: TackFilter,
    pub date_from: Option<Date>,
    pub date_to: Option<Date>,
    /// Masquer les lignes rapprochées antérieures au 1er du mois courant.
    pub hide_old_lines: bool,
}

impl Filters {
    pub fn is_active(&self) -> bool {
        !self.mode.is_empty()
            || !self.payee.is_empty()
            || !self.category.is_empty()
            || !self.amount.is_empty()
            || self.tack != TackFilter::ALL
            || self.date_from.is_some()
            || self.date_to.is_some()
    }

    /// `simple_tack` : pointage simple, sans rapprochement (voir
    /// [`TackFilter::simplified`]).
    pub fn compile(&self, today: Date, simple_tack: bool) -> CompiledFilters {
        let first_of_month = {
            let d = today.to_naive();
            Date::from_ymd(chrono::Datelike::year(&d), chrono::Datelike::month(&d), 1)
                .unwrap_or(today)
        };
        CompiledFilters {
            mode: TextQuery::parse(&self.mode),
            payee: TextQuery::parse(&self.payee),
            category: TextQuery::parse(&self.category),
            amount: AmountQuery::parse(&self.amount),
            tack: if simple_tack {
                self.tack.simplified()
            } else {
                self.tack
            },
            date_from: self.date_from,
            date_to: self.date_to,
            hide_before: self.hide_old_lines.then_some(first_of_month),
        }
    }
}

/// Filtres prêts à être appliqués.
#[derive(Debug, Clone)]
pub struct CompiledFilters {
    mode: TextQuery,
    payee: TextQuery,
    category: TextQuery,
    amount: AmountQuery,
    tack: TackFilter,
    date_from: Option<Date>,
    date_to: Option<Date>,
    hide_before: Option<Date>,
}

impl CompiledFilters {
    pub fn matches(&self, op: &Operation, indexed: &IndexedOperation) -> bool {
        self.matches_values(op) && self.matches_text(indexed)
    }

    /// Critères sur le pointage, les dates et le montant.
    fn matches_values(&self, op: &Operation) -> bool {
        self.tack.shows(op.tack)
            && self.date_from.is_none_or(|d| op.date >= d)
            && self.date_to.is_none_or(|d| op.date <= d)
            && self
                .hide_before
                .is_none_or(|d| !(op.tack == Tack::Reconciled && op.date < d))
            && self.amount.matches(op.amount)
    }

    /// Critères de texte, sur l'index.
    fn matches_text(&self, indexed: &IndexedOperation) -> bool {
        self.mode.matches(&indexed.mode)
            && self.category.matches(&indexed.category)
            && self.payee.matches(&indexed.payee)
    }

    /// Un critère de texte (mode, tiers, catégorie) est posé : `apply` a
    /// besoin de l'index. Sans lui, l'index peut rester vide.
    pub fn needs_index(&self) -> bool {
        [&self.mode, &self.payee, &self.category]
            .iter()
            .any(|q| **q != TextQuery::Any)
    }

    /// Filtre `order` (indices d'opérations) en conservant son ordre.
    pub fn apply(&self, account: &Account, index: &SearchIndex, order: &[usize]) -> Vec<usize> {
        let text = self.needs_index();
        order
            .iter()
            .copied()
            .filter(|&i| {
                self.matches_values(&account.operations[i])
                    && (!text || self.matches_text(&index.rows[i]))
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TextQuery {
    Any,
    /// `//` : champ vide.
    Empty,
    /// `=texte` : égalité stricte (après normalisation).
    Exact(String),
    /// Conjonction de disjonctions de termes.
    Terms(Vec<Vec<Term>>),
}

#[derive(Debug, Clone, PartialEq)]
struct Term {
    /// Motif à chercher dans la valeur encadrée d'espaces.
    pattern: String,
    negate: bool,
}

impl TextQuery {
    fn parse(query: &str) -> Self {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Self::Any;
        }
        if trimmed == "//" {
            return Self::Empty;
        }
        if let Some(rest) = trimmed.strip_prefix('=') {
            return Self::Exact(padded(rest));
        }

        let mut groups: Vec<Vec<Term>> = Vec::new();
        let mut join_next = false;
        for token in tokenize(trimmed) {
            if token.text == "|" && !token.quoted {
                join_next = true;
                continue;
            }
            let alternatives: Vec<String> = if token.quoted {
                vec![token.text]
            } else {
                token.text.split('|').map(str::to_string).collect()
            };
            let leading_or = !token.quoted && token_starts_with_or(&alternatives);
            let trailing_or = !token.quoted
                && alternatives.len() > 1
                && alternatives.last().is_some_and(String::is_empty);
            let terms: Vec<Term> = alternatives
                .iter()
                .filter_map(|a| Term::parse(a, token.quoted))
                .collect();
            if terms.is_empty() {
                join_next |= trailing_or || leading_or;
                continue;
            }
            match groups.last_mut() {
                Some(last) if join_next || leading_or => last.extend(terms),
                _ => groups.push(terms),
            }
            join_next = trailing_or;
        }
        if groups.is_empty() {
            Self::Any
        } else {
            Self::Terms(groups)
        }
    }

    fn matches(&self, padded_value: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Empty => padded_value.trim().is_empty(),
            Self::Exact(expected) => padded_value == expected,
            Self::Terms(groups) => groups
                .iter()
                .all(|group| group.iter().any(|t| t.matches(padded_value))),
        }
    }
}

fn token_starts_with_or(alternatives: &[String]) -> bool {
    alternatives.len() > 1 && alternatives.first().is_some_and(String::is_empty)
}

impl Term {
    fn parse(raw: &str, quoted: bool) -> Option<Self> {
        let mut text = raw;
        let mut negate = false;
        let mut word_start = false;
        let mut word_end = false;
        if !quoted {
            if let Some(rest) = text.strip_prefix('/') {
                negate = true;
                text = rest;
            }
            if let Some(rest) = text.strip_prefix('(') {
                word_start = true;
                text = rest;
            }
            if let Some(rest) = text.strip_suffix(')') {
                word_end = true;
                text = rest;
            }
        }
        let normalized = normalize(text);
        let core = normalized.trim();
        if core.is_empty() {
            return None;
        }
        let mut pattern = String::with_capacity(core.len() + 2);
        if word_start {
            pattern.push(' ');
        }
        pattern.push_str(core);
        if word_end {
            pattern.push(' ');
        }
        Some(Self { pattern, negate })
    }

    fn matches(&self, padded_value: &str) -> bool {
        padded_value.contains(&self.pattern) != self.negate
    }
}

struct Token {
    text: String,
    quoted: bool,
}

fn tokenize(query: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = query.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let text: String = chars.by_ref().take_while(|&c| c != '"').collect();
            tokens.push(Token { text, quoted: true });
        } else {
            let mut text = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '"' {
                    break;
                }
                text.push(c);
                chars.next();
            }
            tokens.push(Token {
                text,
                quoted: false,
            });
        }
    }
    tokens
}

#[derive(Debug, Clone, PartialEq)]
enum AmountQuery {
    Any,
    AtMost(Amount),
    AtLeast(Amount),
    Between(Amount, Amount),
    /// Sous-chaîne du montant formaté avec 2 décimales et une virgule.
    Contains(String),
}

impl AmountQuery {
    fn parse(query: &str) -> Self {
        let q: String = query
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '=')
            .collect();
        if q.is_empty() {
            return Self::Any;
        }
        if let Some(rest) = q.strip_prefix('<') {
            return Amount::parse(rest).map_or(Self::Any, Self::AtMost);
        }
        if let Some(rest) = q.strip_prefix('>') {
            return Amount::parse(rest).map_or(Self::Any, Self::AtLeast);
        }
        if let Some((low, high)) = q.split_once('>').or_else(|| q.split_once('<')) {
            if let (Some(a), Some(b)) = (Amount::parse(low), Amount::parse(high)) {
                return Self::Between(a.min(b), a.max(b));
            }
            return Self::Any;
        }
        Self::Contains(q.replace('.', ","))
    }

    fn matches(&self, amount: Amount) -> bool {
        match self {
            Self::Any => true,
            Self::AtMost(max) => amount <= *max,
            Self::AtLeast(min) => amount >= *min,
            Self::Between(min, max) => amount >= *min && amount <= *max,
            Self::Contains(text) => amount.format(2, ',').contains(text.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(query: &str, value: &str) -> bool {
        TextQuery::parse(query).matches(&padded(value))
    }

    #[test]
    fn parallel_index_matches_sequential() {
        let op = |i: usize| Operation {
            uid: i as u64 + 1,
            date: Date(45_000 + i as i32 / 10),
            mark_date: Date(45_000),
            tack: Tack::None,
            mode: crate::model::Mode::Custom(["Carte", "Chèque", "Espèces"][i % 3].into()),
            payee: format!("Tiers {}", i % 977),
            details: if i.is_multiple_of(2) {
                "Détail".into()
            } else {
                String::new()
            },
            category: None,
            amount: Amount::from_cents(-(i as i64)),
            extra: Vec::new(),
        };
        let account = Account {
            uid: "a".into(),
            name: "Courant".into(),
            bank_name: String::new(),
            agency_name: String::new(),
            agency_address: String::new(),
            number: String::new(),
            bank_code: String::new(),
            agency_code: String::new(),
            bic: String::new(),
            iban: String::new(),
            logo: String::new(),
            currency: "€".into(),
            currency_rate: 1.0,
            secondary_currency: String::new(),
            secondary_rate: 1.0,
            is_virtual: false,
            disabled: false,
            operations: (0..50_000).map(op).collect(),
        };
        let document = Document::default();
        let index = SearchIndex::build(&document, &account);
        assert_eq!(index.len(), account.operations.len());
        for (row, op) in index.rows.iter().zip(&account.operations) {
            let expected = index_operation(&document, op);
            assert_eq!(
                (&row.mode, &row.payee, &row.category),
                (&expected.mode, &expected.payee, &expected.category)
            );
        }
    }

    #[test]
    fn normalization() {
        assert_eq!(
            normalize("Société Générale-Paris!"),
            "societe generale paris "
        );
        assert_eq!(normalize("Œuf, 12€"), "ouf, 12 ");
    }

    #[test]
    fn simple_and_advanced_queries() {
        assert!(q("gene", "Société Générale"));
        assert!(q("societe gen", "Société Générale"));
        assert!(!q("societe lcl", "Société Générale"));
        assert!(q("edf|gdf", "Facture GDF"));
        assert!(q("edf | gdf", "Facture GDF"));
        assert!(q("facture edf|gdf", "Facture GDF"));
        assert!(!q("facture edf|gdf", "Avoir GDF"));
        assert!(q("/carte", "Virement"));
        assert!(!q("/carte", "Carte bleue"));
        assert!(q("(car", "Carte bleue"));
        assert!(!q("(arte", "Carte bleue"));
        assert!(q("bleue)", "Carte bleue"));
        assert!(!q("bleu)", "Carte bleue"));
        assert!(q("\"carte bleue\"", "Ma carte bleue"));
        assert!(!q("\"carte bleue\"", "carte très bleue"));
    }

    #[test]
    fn fixed_syntaxes() {
        assert!(q("=carte", "Carte"));
        assert!(!q("=carte", "Carte FNAC"));
        assert!(q("//", ""));
        assert!(!q("//", "x"));
    }

    #[test]
    fn amount_queries() {
        let m =
            |query: &str, cents: i64| AmountQuery::parse(query).matches(Amount::from_cents(cents));
        assert!(m("<0", -500));
        assert!(!m("<0", 500));
        assert!(m(">-100", -9_999));
        assert!(!m(">-100", -10_001));
        assert!(m("-50>50", 4_999));
        assert!(!m("-50>50", 5_001));
        assert!(m("45.99", -4_599));
        assert!(m("45,9", 4_599));
        assert!(!m("46", 4_599));
    }

    /// Les cases se combinent ; la dernière cochée ne se décoche pas.
    #[test]
    fn tack_filter_combines_states() {
        let mut f = TackFilter::ALL;
        assert!(f.set(Tack::Reconciled, false));
        assert_eq!(f, TackFilter::NOT_RECONCILED);
        assert!(f.shows(Tack::None) && f.shows(Tack::Pointed) && !f.shows(Tack::Reconciled));
        assert!(f.set(Tack::Pointed, false));
        assert!(!f.set(Tack::None, false), "plus rien ne serait affiché");
        assert_eq!(f, TackFilter::NOT_POINTED);
        let mut p = TackFilter::POINTED_OR_RECONCILED;
        assert!(!p.set_simple_pointed(false));
        assert_eq!(p, TackFilter::POINTED_OR_RECONCILED);
    }

    /// Pointage simple : « Pointées » retient aussi les lignes enregistrées
    /// rapprochées (c'est ainsi qu'elles sont pointées dans ce mode).
    #[test]
    fn simple_tack_reads_reconciled_as_pointed() {
        assert_eq!(
            TackFilter::POINTED.simplified(),
            TackFilter::POINTED_OR_RECONCILED
        );
        assert_eq!(TackFilter::NOT_RECONCILED.simplified(), TackFilter::ALL);
        assert_eq!(TackFilter::POINTED.label(true), "Pointées");
        assert!(TackFilter::NOT_RECONCILED.is_all(true));
        assert!(!TackFilter::NOT_RECONCILED.is_all(false));
    }

    /// Enregistrés sous les noms des versions précédentes, qui les relisent.
    #[test]
    fn tack_filter_keeps_its_stored_names() {
        for (filter, name) in [
            (TackFilter::NOT_POINTED, "NotPointed"),
            (TackFilter::POINTED, "Pointed"),
            (TackFilter::RECONCILED, "Reconciled"),
            (TackFilter::POINTED_OR_RECONCILED, "PointedOrReconciled"),
            (TackFilter::ALL, "All"),
            (TackFilter::NOT_RECONCILED, "NotReconciled"),
            (
                TackFilter::NOT_POINTED_OR_RECONCILED,
                "NotPointedOrReconciled",
            ),
        ] {
            let json = serde_json::to_string(&filter).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            assert_eq!(serde_json::from_str::<TackFilter>(&json).unwrap(), filter);
        }
    }
}
