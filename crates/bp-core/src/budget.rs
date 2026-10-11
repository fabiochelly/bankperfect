//! Budgets (ex-scripts `Budget/budget.py` et `BudgetYear/budget_year.py`).
//!
//! Deux outils indépendants :
//!
//! - le **budget mensuel** ([`MonthlyBudget`]) : un montant prévu par mois et
//!   par catégorie (négatif = dépense, positif = recette), comparé au réel
//!   d'une période de référence (mois, année ou toutes opérations en moyenne
//!   mensuelle, ou filtre prédéfini) ;
//! - le **budget annuel** ([`AnnualBudgets`]) : des tableaux nommés pour une
//!   année, faits de lignes libres avec 12 montants prévus et 12 montants
//!   réels, saisis à la main ou calculés à partir d'une liste de catégories.
//!
//! Les montants sont des [`Amount`] (dix-millièmes) ; les moyennes sont
//! arrondies au dix-millième le plus proche.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::autofilters::AutoFilter;
use crate::feature_store::{self, FeatureError};
use crate::filter::{SearchIndex, normalize};
use crate::formula::{Formula, Value};
use crate::model::{Amount, Date, Document};
use crate::pyvalue::{LegacyError, PyValue};

/// Libellé de la ligne des opérations sans catégorie.
pub const UNCATEGORIZED_LABEL: &str = "[Sans catégorie]";

/// Noms des mois, de janvier à décembre.
pub const MONTH_NAMES: [&str; 12] = [
    "Janvier",
    "Février",
    "Mars",
    "Avril",
    "Mai",
    "Juin",
    "Juillet",
    "Août",
    "Septembre",
    "Octobre",
    "Novembre",
    "Décembre",
];

// ---------------------------------------------------------------------------
// Outils communs

/// Montant le plus proche d'un flottant des anciens pickles (`None` si non fini
/// ou hors limites).
pub fn amount_from_f64(value: f64) -> Option<Amount> {
    let scaled = (value * Amount::SCALE as f64).round();
    (scaled.is_finite() && scaled.abs() < 9.0e18).then_some(Amount(scaled as i64))
}

/// Division arrondie au plus proche (demi éloigné de zéro).
fn div_round(numerator: i128, denominator: i128) -> i64 {
    if denominator == 0 {
        return 0;
    }
    let negative = (numerator < 0) != (denominator < 0);
    let (n, d) = (numerator.abs(), denominator.abs());
    let q = (n + d / 2) / d;
    let q = if negative { -q } else { q };
    q.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

/// Écart en pourcentage du prévu : `100 × (réel − prévu) / |prévu|`.
fn percent(real: Amount, budget: Amount) -> Option<f64> {
    (budget.0 != 0).then(|| 100.0 * (real.0 - budget.0) as f64 / budget.0.abs() as f64)
}

/// Analyse un montant saisi dans les fenêtres de budget (`str_to_float`) :
/// espaces, `€`, `$`, `F` et `%` ignorés, `,` ou `.` décimal, séparateurs de
/// milliers tolérés, et petites expressions arithmétiques (`1200/12`,
/// `45*2+10`). Contrairement à l'original, `/` est une vraie division
/// (`100/3` = 33,3333 et non 33). Renvoie `None` si le texte est vide ou
/// invalide.
pub fn parse_amount_input(text: &str) -> Option<Amount> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '$' | '€' | 'F' | '%'))
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    // Seul le dernier point est décimal (« 1.234.56 » → 1234.56).
    let cleaned = match cleaned.rfind('.') {
        Some(last) if cleaned.matches('.').count() > 1 => {
            let (head, tail) = cleaned.split_at(last);
            format!("{}{tail}", head.replace('.', ""))
        }
        _ => cleaned,
    };
    let body = cleaned.strip_prefix(['-', '+']).unwrap_or(&cleaned);
    if body.chars().any(|c| c.is_ascii_digit())
        && body.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return Amount::parse(&cleaned);
    }
    let formula = Formula::parse(&cleaned).ok()?.with_float_arithmetic();
    match formula.eval(&|_| None).ok()? {
        Value::Float(f) => amount_from_f64(f),
        Value::Int(i) => i.checked_mul(Amount::SCALE).map(Amount),
        _ => None,
    }
}

fn legacy_category(value: &PyValue) -> Result<Option<u32>, LegacyError> {
    match value.as_i64() {
        Some(-1) => Ok(None),
        Some(id) => u32::try_from(id).map(Some).map_err(|_| {
            LegacyError::Structure(format!("identifiant de catégorie invalide : {id}"))
        }),
        None => Err(LegacyError::Structure(
            "identifiant de catégorie non entier".into(),
        )),
    }
}

fn legacy_amount(value: &PyValue) -> Result<Amount, LegacyError> {
    value
        .as_f64()
        .and_then(amount_from_f64)
        .ok_or_else(|| LegacyError::Structure("montant non numérique".into()))
}

/// Comptes retenus : `None` = tous les comptes du document.
fn selected_accounts(document: &Document, accounts: Option<&[usize]>) -> Vec<usize> {
    match accounts {
        None => (0..document.accounts.len()).collect(),
        Some(list) => {
            let mut seen = BTreeSet::new();
            list.iter()
                .copied()
                .filter(|&i| i < document.accounts.len() && seen.insert(i))
                .collect()
        }
    }
}

/// Structure des catégories : ordre d'affichage, parents et enfants.
struct CategoryTree {
    /// `None` (sans catégorie) puis les catégories dans l'ordre d'affichage.
    ids: Vec<Option<u32>>,
    /// Catégories de premier niveau (dont `None`).
    parents: BTreeSet<Option<u32>>,
    children: HashMap<Option<u32>, Vec<Option<u32>>>,
}

impl CategoryTree {
    fn new(document: &Document) -> Self {
        let mut ids = vec![None];
        let mut parents = BTreeSet::from([None]);
        let mut children: HashMap<Option<u32>, Vec<Option<u32>>> = HashMap::new();
        let mut current: Option<Option<u32>> = None;
        for category in &document.categories {
            let id = Some(category.id);
            ids.push(id);
            match current {
                // Une sous-catégorie sans parent qui la précède est traitée
                // comme une catégorie de premier niveau.
                Some(parent) if category.is_child => children.entry(parent).or_default().push(id),
                _ => {
                    parents.insert(id);
                    current = Some(id);
                }
            }
        }
        Self {
            ids,
            parents,
            children,
        }
    }

    fn children(&self, id: Option<u32>) -> &[Option<u32>] {
        self.children.get(&id).map_or(&[], Vec::as_slice)
    }
}

// ---------------------------------------------------------------------------
// Budget mensuel

/// Nature d'une ligne de budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetKind {
    /// Dépense (montant prévu négatif).
    Expense,
    /// Recette (montant prévu positif).
    Income,
}

impl BudgetKind {
    pub fn label(self) -> &'static str {
        match self {
            BudgetKind::Expense => "Dépense",
            BudgetKind::Income => "Recette",
        }
    }
}

/// Montant mensuel prévu pour une catégorie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetEntry {
    /// `None` : opérations sans catégorie.
    pub category: Option<u32>,
    /// Montant signé : négatif pour une dépense, positif pour une recette.
    pub amount: Amount,
}

/// Budget mensuel : un montant prévu par catégorie.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyBudget {
    pub entries: Vec<BudgetEntry>,
}

/// Période de référence du budget mensuel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetPeriod {
    /// Un mois précis, sans moyenne (« Mois en cours »).
    Month { year: i32, month: u32 },
    /// Moyenne mensuelle d'une année (« Tout AAAA (moyenne) »).
    Year(i32),
    /// Moyenne mensuelle de toutes les opérations.
    All,
    /// Moyenne mensuelle des opérations retenues par un filtre prédéfini.
    Filter(AutoFilter),
}

impl BudgetPeriod {
    pub fn current_month(today: Date) -> Self {
        let d = today.to_naive();
        BudgetPeriod::Month {
            year: d.year(),
            month: d.month(),
        }
    }

    pub fn current_year(today: Date) -> Self {
        BudgetPeriod::Year(today.to_naive().year())
    }

    /// Phrase décrivant les opérations prises en compte.
    pub fn description(&self, today: Date) -> String {
        match self {
            BudgetPeriod::Month { year, month } => {
                let name = MONTH_NAMES
                    .get((*month as usize).wrapping_sub(1))
                    .map_or_else(|| month.to_string(), |n| n.to_lowercase());
                let article = if name.starts_with(['a', 'o']) {
                    "d'"
                } else {
                    "de "
                };
                format!("Opérations concernées : toutes les lignes du mois {article}{name} {year}")
            }
            BudgetPeriod::Year(year) => {
                format!("Opérations concernées : toutes les lignes de l'année {year}")
            }
            BudgetPeriod::All => "Opérations concernées : toutes".into(),
            BudgetPeriod::Filter(filter) => filter.description(today),
        }
    }
}

/// Calcul du nombre de mois servant à la moyenne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MonthCounting {
    /// « Mois inclus dans la période » : nombre de mois ayant au moins une
    /// opération.
    #[default]
    MonthsWithOperations,
    /// « Mois calculés (jours / 30,4375) » : jours entre la première et la
    /// dernière opération, divisés par 30,4375.
    Days,
}

/// Durée sur laquelle le réel est moyenné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodSpan {
    /// Mois précis : pas de moyenne.
    SingleMonth,
    /// Nombre de mois ayant des opérations.
    Months(u32),
    /// Nombre de jours, de la première à la dernière opération incluses.
    Days(i64),
}

/// Jours moyens par mois, en dix-millièmes (30,4375).
const DAYS_PER_MONTH_E4: i128 = 304_375;

impl PeriodSpan {
    /// Moyenne mensuelle de `total` (ou `total` lui-même sans moyenne).
    pub fn monthly_average(self, total: Amount) -> Amount {
        match self {
            PeriodSpan::SingleMonth | PeriodSpan::Months(0) | PeriodSpan::Days(0) => total,
            PeriodSpan::Months(n) => Amount(div_round(total.0 as i128, i128::from(n))),
            PeriodSpan::Days(d) => Amount(div_round(
                total.0 as i128 * DAYS_PER_MONTH_E4,
                i128::from(d) * 10_000,
            )),
        }
    }

    /// Complément du libellé de période : « (3 mois) », « (2 mois et 5 jours) ».
    pub fn label(self) -> String {
        match self {
            PeriodSpan::SingleMonth => String::new(),
            PeriodSpan::Months(n) => format!("({n} mois)"),
            PeriodSpan::Days(count) => {
                let scaled = i128::from(count) * 10_000;
                let months = scaled / DAYS_PER_MONTH_E4;
                let days = (scaled - months * DAYS_PER_MONTH_E4) / 10_000;
                let unit = if days == 1 { "jour" } else { "jours" };
                match (months, days) {
                    (0, _) => format!("({days} {unit})"),
                    (_, 0) => format!("({months} mois)"),
                    _ => format!("({months} mois et {days} {unit})"),
                }
            }
        }
    }
}

/// Options d'affichage du budget mensuel (cases et listes de la fenêtre).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetOptions {
    pub period: BudgetPeriod,
    pub counting: MonthCounting,
    /// Indices dans `document.accounts` ; `None` = tous les comptes.
    pub accounts: Option<Vec<usize>>,
    /// « Lignes sans budget » : afficher les catégories sans montant prévu.
    pub show_unbudgeted: bool,
    /// « Lignes à zéro » : afficher les catégories sans opération.
    pub show_zero: bool,
    /// « Sous-catégories » : afficher les sous-catégories séparément ; sinon
    /// leurs montants sont agrégés à la catégorie parente.
    pub show_subcategories: bool,
}

impl BudgetOptions {
    /// Réglages initiaux de la fenêtre : mois en cours, toutes les cases cochées.
    pub fn new(today: Date) -> Self {
        Self {
            period: BudgetPeriod::current_month(today),
            counting: MonthCounting::MonthsWithOperations,
            accounts: None,
            show_unbudgeted: true,
            show_zero: true,
            show_subcategories: true,
        }
    }
}

/// Ligne du budget mensuel.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetRow {
    pub category: Option<u32>,
    /// Nom de la catégorie (sans le parent) ou [`UNCATEGORIZED_LABEL`].
    pub label: String,
    pub is_child: bool,
    /// `None` si la ligne n'a pas de budget.
    pub kind: Option<BudgetKind>,
    /// Montant prévu (`None` sans budget).
    pub budget: Option<Amount>,
    /// Réel (moyenne mensuelle selon la période).
    pub real: Amount,
    /// Réel − prévu (`None` sans budget).
    pub difference: Option<Amount>,
    /// Écart en % du prévu (`None` sans budget).
    pub percent: Option<f64>,
}

/// Ligne de totaux.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BudgetTotal {
    pub budget: Amount,
    pub real: Amount,
    pub difference: Amount,
    /// `None` si le prévu total est nul.
    pub percent: Option<f64>,
}

impl BudgetTotal {
    fn add(&mut self, budget: Amount, real: Amount) {
        self.budget = self.budget + budget;
        self.real = self.real + real;
    }

    fn finish(mut self) -> Self {
        self.difference = Amount(self.real.0 - self.budget.0);
        self.percent = percent(self.real, self.budget);
        self
    }
}

/// Résultat du budget mensuel.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetReport {
    pub rows: Vec<BudgetRow>,
    /// Totaux des lignes budgétées en dépense.
    pub expenses: BudgetTotal,
    /// Totaux des lignes budgétées en recette.
    pub income: BudgetTotal,
    /// Totaux de toutes les lignes budgétées.
    pub total: BudgetTotal,
    pub span: PeriodSpan,
    /// Phrase décrivant la période, suivie de la durée moyennée.
    pub description: String,
}

impl MonthlyBudget {
    /// Clé de rangement dans le nouveau format.
    pub const KEY: &'static str = "budget.v6";
    /// Clé de l'ancien script (pickle d'un dict `{id_catégorie: montant}`).
    pub const LEGACY_KEY: &'static str = "fabiobpbudget";

    /// Montant prévu (signé) pour `category`.
    pub fn get(&self, category: Option<u32>) -> Option<Amount> {
        self.entries
            .iter()
            .find(|e| e.category == category)
            .map(|e| e.amount)
    }

    /// Fixe le montant signé ; un montant nul retire la catégorie du budget.
    pub fn set_signed(&mut self, category: Option<u32>, amount: Amount) {
        self.entries.retain(|e| e.category != category);
        if amount.0 != 0 {
            self.entries.push(BudgetEntry { category, amount });
        }
    }

    /// Saisie de la fenêtre « Budget de la ligne » : montant (valeur absolue)
    /// et nature ; `None` ou un montant nul retire la catégorie du budget.
    pub fn set(&mut self, category: Option<u32>, kind: BudgetKind, amount: Option<Amount>) {
        let amount = amount.map_or(0, |a| a.0.saturating_abs());
        let signed = match kind {
            BudgetKind::Expense => -amount,
            BudgetKind::Income => amount,
        };
        self.set_signed(category, Amount(signed));
    }

    pub fn remove(&mut self, category: Option<u32>) {
        self.entries.retain(|e| e.category != category);
    }

    fn budgeted(&self, tree: &CategoryTree, id: Option<u32>) -> bool {
        self.get(id).is_some() || tree.children(id).iter().any(|&c| self.get(c).is_some())
    }

    /// Réel par catégorie sur la période, et durée de moyenne.
    fn real_totals(
        document: &Document,
        options: &BudgetOptions,
        today: Date,
    ) -> (HashMap<Option<u32>, Amount>, PeriodSpan) {
        let accounts = selected_accounts(document, options.accounts.as_deref());
        let mut totals: HashMap<Option<u32>, Amount> = HashMap::new();
        let mut months = BTreeSet::new();
        let mut first: Option<Date> = None;
        let mut last: Option<Date> = None;
        let mut take = |op: &crate::model::Operation| {
            let d = op.date.to_naive();
            months.insert((d.year(), d.month()));
            first = Some(first.map_or(op.date, |f| f.min(op.date)));
            last = Some(last.map_or(op.date, |l| l.max(op.date)));
            let entry = totals.entry(op.category).or_default();
            *entry = *entry + op.amount;
        };
        match &options.period {
            BudgetPeriod::Filter(filter) => {
                let compiled = filter.to_filters(today).compile(today);
                for &a in &accounts {
                    let account = &document.accounts[a];
                    let index = SearchIndex::build(document, account);
                    let all: Vec<usize> = (0..account.operations.len()).collect();
                    for i in compiled.apply(account, &index, &all) {
                        take(&account.operations[i]);
                    }
                }
            }
            period => {
                for &a in &accounts {
                    for op in &document.accounts[a].operations {
                        let d = op.date.to_naive();
                        let keep = match period {
                            BudgetPeriod::Month { year, month } => {
                                d.year() == *year && d.month() == *month
                            }
                            BudgetPeriod::Year(year) => d.year() == *year,
                            _ => true,
                        };
                        if keep {
                            take(op);
                        }
                    }
                }
            }
        }
        let span = match (&options.period, options.counting) {
            (BudgetPeriod::Month { .. }, _) => PeriodSpan::SingleMonth,
            (_, MonthCounting::MonthsWithOperations) => PeriodSpan::Months(months.len() as u32),
            (_, MonthCounting::Days) => match (first, last) {
                // Jours comptés bornes incluses (voir docs/corrections.md).
                (Some(f), Some(l)) => PeriodSpan::Days(i64::from(l.0 - f.0) + 1),
                _ => PeriodSpan::Days(0),
            },
        };
        (totals, span)
    }

    /// Calcule le tableau du budget mensuel.
    pub fn report(
        &self,
        document: &Document,
        options: &BudgetOptions,
        today: Date,
    ) -> BudgetReport {
        let tree = CategoryTree::new(document);
        let (totals, span) = Self::real_totals(document, options, today);
        let aggregate = !options.show_subcategories;
        let amount_of = |map: &dyn Fn(Option<u32>) -> Amount, id: Option<u32>| {
            let own = map(id);
            if aggregate {
                tree.children(id).iter().fold(own, |sum, &c| sum + map(c))
            } else {
                own
            }
        };
        let real_of = |id| span.monthly_average(totals.get(&id).copied().unwrap_or_default());
        let budget_of = |id| self.get(id).unwrap_or_default();

        let mut rows = Vec::new();
        let mut expenses = BudgetTotal::default();
        let mut income = BudgetTotal::default();
        let mut total = BudgetTotal::default();
        for &id in &tree.ids {
            if aggregate && !tree.parents.contains(&id) {
                continue;
            }
            let budgeted = self.budgeted(&tree, id);
            let real = amount_of(&real_of, id);
            if !options.show_unbudgeted && !budgeted {
                continue;
            }
            if !options.show_zero && real.0 == 0 && !budgeted {
                continue;
            }
            let estimated = amount_of(&budget_of, id);
            let (label, is_child) = match id.and_then(|i| document.category(i)) {
                Some(c) => (
                    c.name.trim().to_string(),
                    c.is_child && !tree.parents.contains(&id),
                ),
                None => (UNCATEGORIZED_LABEL.to_string(), false),
            };
            let mut row = BudgetRow {
                category: id,
                label,
                is_child,
                kind: None,
                budget: None,
                real,
                difference: None,
                percent: None,
            };
            if estimated.0 != 0 {
                let kind = if estimated.0 > 0 {
                    income.add(estimated, real);
                    BudgetKind::Income
                } else {
                    expenses.add(estimated, real);
                    BudgetKind::Expense
                };
                total.add(estimated, real);
                row.kind = Some(kind);
                row.budget = Some(estimated);
                row.difference = Some(Amount(real.0 - estimated.0));
                row.percent = percent(real, estimated);
            }
            rows.push(row);
        }
        let mut description = options.period.description(today);
        let span_label = span.label();
        if !span_label.is_empty() {
            description.push(' ');
            description.push_str(&span_label);
        }
        BudgetReport {
            rows,
            expenses: expenses.finish(),
            income: income.finish(),
            total: total.finish(),
            span,
            description,
        }
    }

    /// Conversion depuis l'ancien pickle : dict `{id_catégorie: montant}`
    /// (`-1` = sans catégorie, montants flottants signés).
    pub fn from_legacy(value: &PyValue) -> Result<Self, LegacyError> {
        let Some(items) = value.as_dict() else {
            return Err(LegacyError::Structure(
                "budget : dictionnaire attendu".into(),
            ));
        };
        let mut budget = Self::default();
        for (key, amount) in items {
            budget.set_signed(legacy_category(key)?, legacy_amount(amount)?);
        }
        Ok(budget)
    }

    /// Lit le budget du document (nouvelle clé, sinon ancien pickle décodé par
    /// `parse_pickle`, sinon budget vide).
    pub fn load(
        document: &Document,
        parse_pickle: &dyn Fn(&[u8]) -> Option<PyValue>,
    ) -> Result<Self, FeatureError> {
        feature_store::load_with_legacy(
            document,
            Self::KEY,
            Self::LEGACY_KEY,
            parse_pickle,
            Self::from_legacy,
        )
        .unwrap_or_else(|| Ok(Self::default()))
    }

    /// Range le budget dans le document (et retire l'ancienne clé).
    pub fn save(&self, document: &mut Document) -> Result<(), FeatureError> {
        feature_store::set(document, Self::KEY, self)?;
        feature_store::remove(document, Self::LEGACY_KEY);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Budget annuel

/// Erreur de saisie du budget annuel.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnnualBudgetError {
    #[error("le nom spécifié est incorrect (2 caractères au moins)")]
    InvalidName,
    #[error("le nom spécifié existe déjà")]
    DuplicateName,
    #[error("l'année saisie est incorrecte (1900 à 9999)")]
    InvalidYear,
    #[error("le titre est incorrect (2 caractères au moins)")]
    InvalidTitle,
    #[error("vous devez sélectionner au moins une catégorie à affecter")]
    NoCategory,
    #[error("tableau inexistant")]
    TableNotFound,
    #[error("ligne inexistante")]
    LineNotFound,
    #[error("mois invalide (0 à 11)")]
    InvalidMonth,
    #[error("le réalisé de cette ligne est calculé automatiquement")]
    RealIsAutomatic,
}

/// Colonne de valeurs d'une ligne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Budget,
    Real,
    /// Réel − budget (calculé, non modifiable).
    Difference,
}

/// Portée d'une saisie de valeur (« Au mois sélectionné »…).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spread {
    /// Au mois sélectionné.
    Month,
    /// À tous les mois de l'année.
    AllMonths,
    /// Un mois sur deux (à partir du mois sélectionné, dans les deux sens).
    EveryTwoMonths,
    /// Un mois sur trois.
    EveryThreeMonths,
}

/// Cellule de la grille du budget annuel : colonne 0 = titre, puis 12 mois ×
/// (budget, réel, écart), puis les totaux × 3 (40 colonnes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridColumn {
    Title,
    /// Mois de 0 (janvier) à 11.
    Month {
        month: usize,
        kind: ValueKind,
    },
    Total(ValueKind),
}

impl GridColumn {
    /// Nombre de colonnes de la grille.
    pub const COUNT: usize = 40;

    /// Colonne de la grille d'indice `column`. Le mois est calculé en division
    /// **entière** (`(colonne − 1) / 3`), ce que Python 3 aurait cassé.
    pub fn from_index(column: usize) -> Option<Self> {
        if column == 0 {
            return Some(GridColumn::Title);
        }
        if column >= Self::COUNT {
            return None;
        }
        let kind = match (column - 1) % 3 {
            0 => ValueKind::Budget,
            1 => ValueKind::Real,
            _ => ValueKind::Difference,
        };
        let month = (column - 1) / 3;
        Some(if month < 12 {
            GridColumn::Month { month, kind }
        } else {
            GridColumn::Total(kind)
        })
    }

    /// Une saisie est-elle possible dans cette colonne ? Seuls le budget et
    /// le réel d'un mois le sont (l'original laissait saisir dans la colonne
    /// « Total budget », d'où un index hors limites).
    pub fn editable(self) -> Option<(usize, ValueKind)> {
        match self {
            GridColumn::Month {
                month,
                kind: kind @ (ValueKind::Budget | ValueKind::Real),
            } => Some((month, kind)),
            _ => None,
        }
    }
}

/// Ligne d'un tableau de budget annuel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnualLine {
    pub title: String,
    /// Réel calculé à partir des catégories (sinon saisi à la main).
    #[serde(default)]
    pub auto_real: bool,
    /// Catégories additionnées pour le réel (`None` = sans catégorie). Les
    /// sous-catégories ne sont pas incluses implicitement.
    #[serde(default)]
    pub categories: Vec<Option<u32>>,
    /// Montants prévus, de janvier à décembre.
    pub budget: [Amount; 12],
    /// Montants réels, de janvier à décembre.
    pub real: [Amount; 12],
}

/// Saisie d'une ligne (fenêtre « Ajouter / Modifier une ligne »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineInput {
    pub title: String,
    pub auto_real: bool,
    pub categories: Vec<Option<u32>>,
}

impl LineInput {
    fn validate(&self) -> Result<LineInput, AnnualBudgetError> {
        let title = self.title.trim();
        if title.chars().count() < 2 {
            return Err(AnnualBudgetError::InvalidTitle);
        }
        if self.auto_real && self.categories.is_empty() {
            return Err(AnnualBudgetError::NoCategory);
        }
        Ok(LineInput {
            title: title.to_string(),
            ..self.clone()
        })
    }
}

/// Valeurs d'un mois (ou du total) d'une ligne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MonthValues {
    pub budget: Amount,
    pub real: Amount,
    /// Réel − budget.
    pub difference: Amount,
}

impl MonthValues {
    fn new(budget: Amount, real: Amount) -> Self {
        Self {
            budget,
            real,
            difference: Amount(real.0 - budget.0),
        }
    }

    fn add(self, other: MonthValues) -> Self {
        Self::new(self.budget + other.budget, self.real + other.real)
    }
}

/// Ligne calculée du budget annuel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnualRow {
    pub title: String,
    pub auto_real: bool,
    pub months: [MonthValues; 12],
    pub total: MonthValues,
}

/// Tableau calculé : lignes et ligne « Totaux ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnualReport {
    pub rows: Vec<AnnualRow>,
    pub totals: AnnualRow,
}

/// Tableau de budget annuel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnualBudget {
    pub name: String,
    pub year: i32,
    /// Identifiants (`Account::uid`) des comptes pris en compte ; `None` =
    /// tous les comptes.
    #[serde(default)]
    pub accounts: Option<Vec<String>>,
    #[serde(default)]
    pub lines: Vec<AnnualLine>,
}

impl AnnualBudget {
    pub fn new(name: impl Into<String>, year: i32) -> Self {
        Self {
            name: name.into(),
            year,
            accounts: None,
            lines: Vec::new(),
        }
    }

    /// Indices des comptes retenus dans `document.accounts`.
    pub fn account_indices(&self, document: &Document) -> Vec<usize> {
        match &self.accounts {
            None => (0..document.accounts.len()).collect(),
            Some(uids) => document
                .accounts
                .iter()
                .enumerate()
                .filter(|(_, a)| uids.contains(&a.uid))
                .map(|(i, _)| i)
                .collect(),
        }
    }

    /// Choisit les comptes (bouton « Comptes ») par leurs indices.
    pub fn set_accounts(&mut self, document: &Document, indices: Option<&[usize]>) {
        self.accounts = indices.map(|list| {
            selected_accounts(document, Some(list))
                .into_iter()
                .map(|i| document.accounts[i].uid.clone())
                .collect()
        });
    }

    pub fn add_line(&mut self, input: &LineInput) -> Result<usize, AnnualBudgetError> {
        let input = input.validate()?;
        self.lines.push(AnnualLine {
            title: input.title,
            auto_real: input.auto_real,
            categories: input.categories,
            budget: [Amount::ZERO; 12],
            real: [Amount::ZERO; 12],
        });
        Ok(self.lines.len() - 1)
    }

    /// Modifie le titre, le mode de calcul et les catégories d'une ligne ; ses
    /// montants sont conservés.
    pub fn update_line(
        &mut self,
        index: usize,
        input: &LineInput,
    ) -> Result<(), AnnualBudgetError> {
        let input = input.validate()?;
        let line = self
            .lines
            .get_mut(index)
            .ok_or(AnnualBudgetError::LineNotFound)?;
        line.title = input.title;
        line.auto_real = input.auto_real;
        line.categories = input.categories;
        Ok(())
    }

    pub fn remove_line(&mut self, index: usize) -> Result<AnnualLine, AnnualBudgetError> {
        if index >= self.lines.len() {
            return Err(AnnualBudgetError::LineNotFound);
        }
        Ok(self.lines.remove(index))
    }

    /// Saisit `value` dans la colonne `kind` du mois `month` (0 à 11) de la
    /// ligne `line`, éventuellement répétée sur d'autres mois.
    pub fn set_value(
        &mut self,
        line: usize,
        kind: ValueKind,
        month: usize,
        value: Amount,
        spread: Spread,
    ) -> Result<(), AnnualBudgetError> {
        if month >= 12 {
            return Err(AnnualBudgetError::InvalidMonth);
        }
        let line = self
            .lines
            .get_mut(line)
            .ok_or(AnnualBudgetError::LineNotFound)?;
        let values = match kind {
            ValueKind::Budget => &mut line.budget,
            ValueKind::Real if line.auto_real => return Err(AnnualBudgetError::RealIsAutomatic),
            ValueKind::Real => &mut line.real,
            ValueKind::Difference => return Err(AnnualBudgetError::InvalidMonth),
        };
        let step = match spread {
            Spread::Month => {
                values[month] = value;
                return Ok(());
            }
            Spread::AllMonths => 1,
            Spread::EveryTwoMonths => 2,
            Spread::EveryThreeMonths => 3,
        };
        for (i, v) in values.iter_mut().enumerate() {
            if (i as i64 - month as i64).rem_euclid(step) == 0 {
                *v = value;
            }
        }
        Ok(())
    }

    /// Réel de l'année par (mois 0..11, catégorie), sur les comptes retenus.
    fn real_by_month(&self, document: &Document) -> HashMap<(usize, Option<u32>), Amount> {
        let mut real = HashMap::new();
        for a in self.account_indices(document) {
            for op in &document.accounts[a].operations {
                let d = op.date.to_naive();
                if d.year() != self.year {
                    continue;
                }
                let entry = real
                    .entry((d.month0() as usize, op.category))
                    .or_insert(Amount::ZERO);
                *entry = *entry + op.amount;
            }
        }
        real
    }

    fn auto_values(
        line: &AnnualLine,
        real: &HashMap<(usize, Option<u32>), Amount>,
    ) -> [Amount; 12] {
        std::array::from_fn(|month| {
            line.categories
                .iter()
                .filter_map(|c| real.get(&(month, *c)))
                .copied()
                .sum()
        })
    }

    /// Recalcule et mémorise le réel des lignes automatiques (comme
    /// l'original à chaque affichage).
    pub fn refresh_auto_real(&mut self, document: &Document) {
        let real = self.real_by_month(document);
        for line in self.lines.iter_mut().filter(|l| l.auto_real) {
            line.real = Self::auto_values(line, &real);
        }
    }

    /// Calcule le tableau (réel des lignes automatiques recalculé), sans
    /// modifier les données.
    pub fn report(&self, document: &Document) -> AnnualReport {
        let real = self.real_by_month(document);
        let rows: Vec<AnnualRow> = self
            .lines
            .iter()
            .map(|line| {
                let real_values = if line.auto_real {
                    Self::auto_values(line, &real)
                } else {
                    line.real
                };
                let months: [MonthValues; 12] =
                    std::array::from_fn(|m| MonthValues::new(line.budget[m], real_values[m]));
                AnnualRow {
                    title: line.title.clone(),
                    auto_real: line.auto_real,
                    total: months
                        .iter()
                        .copied()
                        .fold(MonthValues::default(), MonthValues::add),
                    months,
                }
            })
            .collect();
        let months: [MonthValues; 12] = std::array::from_fn(|m| {
            rows.iter()
                .map(|r| r.months[m])
                .fold(MonthValues::default(), MonthValues::add)
        });
        let totals = AnnualRow {
            title: "Totaux".into(),
            auto_real: false,
            total: months
                .iter()
                .copied()
                .fold(MonthValues::default(), MonthValues::add),
            months,
        };
        AnnualReport { rows, totals }
    }

    /// Conversion depuis un élément de l'ancienne liste :
    /// `{"name", "year", "accounts"?, "lines": [{"title", "autocalc_real",
    /// "categs", "estimated_values"[12], "real_values"[12]}]}`. Les comptes,
    /// mémorisés par indice réel, sont convertis en identifiants.
    pub fn from_legacy(value: &PyValue, document: &Document) -> Result<Self, LegacyError> {
        if value.as_dict().is_none() {
            return Err(LegacyError::Structure(
                "budget annuel : dictionnaire attendu".into(),
            ));
        }
        let year = value
            .get("year")
            .and_then(PyValue::as_i64)
            .and_then(|y| i32::try_from(y).ok())
            .ok_or_else(|| LegacyError::Structure("budget annuel : année manquante".into()))?;
        let accounts = match value.get("accounts") {
            None | Some(PyValue::None) => None,
            Some(list) => {
                let items = list.as_list().ok_or_else(|| {
                    LegacyError::Structure("budget annuel : liste de comptes attendue".into())
                })?;
                Some(
                    items
                        .iter()
                        .filter_map(PyValue::as_i64)
                        .filter_map(|i| usize::try_from(i).ok())
                        .filter_map(|i| document.accounts.get(i))
                        .map(|a| a.uid.clone())
                        .collect(),
                )
            }
        };
        let lines = match value.get("lines") {
            None => Vec::new(),
            Some(lines) => lines
                .as_list()
                .ok_or_else(|| {
                    LegacyError::Structure("budget annuel : liste de lignes attendue".into())
                })?
                .iter()
                .map(AnnualLine::from_legacy)
                .collect::<Result<_, _>>()?,
        };
        Ok(Self {
            name: value.text("name").to_string(),
            year,
            accounts,
            lines,
        })
    }
}

impl AnnualLine {
    fn from_legacy(value: &PyValue) -> Result<Self, LegacyError> {
        if value.as_dict().is_none() {
            return Err(LegacyError::Structure(
                "ligne de budget : dictionnaire attendu".into(),
            ));
        }
        let values = |key: &str| -> Result<[Amount; 12], LegacyError> {
            let mut out = [Amount::ZERO; 12];
            if let Some(list) = value.get(key) {
                let items = list.as_list().ok_or_else(|| {
                    LegacyError::Structure(format!(
                        "ligne de budget : « {key} » doit être une liste"
                    ))
                })?;
                for (slot, item) in out.iter_mut().zip(items) {
                    *slot = legacy_amount(item)?;
                }
            }
            Ok(out)
        };
        let categories = match value.get("categs") {
            None => Vec::new(),
            Some(list) => list
                .as_list()
                .ok_or_else(|| {
                    LegacyError::Structure("ligne de budget : catégories en liste attendues".into())
                })?
                .iter()
                .map(legacy_category)
                .collect::<Result<_, _>>()?,
        };
        Ok(Self {
            title: value.text("title").to_string(),
            auto_real: value.flag("autocalc_real"),
            categories,
            budget: values("estimated_values")?,
            real: values("real_values")?,
        })
    }
}

/// Ensemble des tableaux de budget annuel, triés par nom.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnualBudgets {
    pub tables: Vec<AnnualBudget>,
}

fn compare_names(a: &str, b: &str) -> Ordering {
    normalize(a).cmp(&normalize(b)).then_with(|| a.cmp(b))
}

impl AnnualBudgets {
    /// Clé de rangement dans le nouveau format.
    pub const KEY: &'static str = "budget_year.v6";
    /// Clé de l'ancien script (pickle d'une liste de dicts).
    pub const LEGACY_KEY: &'static str = "fabiobpbudgetyear";

    fn sort(&mut self) {
        self.tables.sort_by(|a, b| compare_names(&a.name, &b.name));
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.tables.iter().position(|t| t.name == name)
    }

    /// Libellés de la fenêtre d'accueil : « nom (année) ».
    pub fn labels(&self) -> Vec<String> {
        self.tables
            .iter()
            .map(|t| format!("{} ({})", t.name, t.year))
            .collect()
    }

    fn check(
        &self,
        name: &str,
        year: i32,
        except: Option<usize>,
    ) -> Result<String, AnnualBudgetError> {
        let name = name.trim();
        if name.chars().count() < 2 {
            return Err(AnnualBudgetError::InvalidName);
        }
        if self
            .tables
            .iter()
            .enumerate()
            .any(|(i, t)| Some(i) != except && t.name == name)
        {
            return Err(AnnualBudgetError::DuplicateName);
        }
        if !(1900..=9999).contains(&year) {
            return Err(AnnualBudgetError::InvalidYear);
        }
        Ok(name.to_string())
    }

    /// Crée un tableau vide ; renvoie sa position après tri.
    pub fn create(&mut self, name: &str, year: i32) -> Result<usize, AnnualBudgetError> {
        let name = self.check(name, year, None)?;
        self.tables.push(AnnualBudget::new(name.clone(), year));
        self.sort();
        self.index_of(&name).ok_or(AnnualBudgetError::TableNotFound)
    }

    /// Renomme un tableau et change son année ; renvoie sa nouvelle position.
    pub fn rename(
        &mut self,
        index: usize,
        name: &str,
        year: i32,
    ) -> Result<usize, AnnualBudgetError> {
        if index >= self.tables.len() {
            return Err(AnnualBudgetError::TableNotFound);
        }
        let name = self.check(name, year, Some(index))?;
        self.tables[index].name = name.clone();
        self.tables[index].year = year;
        self.sort();
        self.index_of(&name).ok_or(AnnualBudgetError::TableNotFound)
    }

    pub fn delete(&mut self, index: usize) -> Result<AnnualBudget, AnnualBudgetError> {
        if index >= self.tables.len() {
            return Err(AnnualBudgetError::TableNotFound);
        }
        Ok(self.tables.remove(index))
    }

    /// Duplique un tableau sous le nom « Copie de … » (suffixé « (2) », « (3) »…
    /// si ce nom existe déjà) ; renvoie la position de la copie.
    pub fn duplicate(&mut self, index: usize) -> Result<usize, AnnualBudgetError> {
        let mut copy = self
            .tables
            .get(index)
            .cloned()
            .ok_or(AnnualBudgetError::TableNotFound)?;
        let base = format!("Copie de {}", copy.name);
        let mut name = base.clone();
        let mut n = 2;
        while self.index_of(&name).is_some() {
            name = format!("{base} ({n})");
            n += 1;
        }
        copy.name = name.clone();
        self.tables.push(copy);
        self.sort();
        self.index_of(&name).ok_or(AnnualBudgetError::TableNotFound)
    }

    /// Conversion depuis l'ancien pickle (liste de tableaux).
    pub fn from_legacy(value: &PyValue, document: &Document) -> Result<Self, LegacyError> {
        let items = value
            .as_list()
            .ok_or_else(|| LegacyError::Structure("budgets annuels : liste attendue".into()))?;
        let mut budgets = Self {
            tables: items
                .iter()
                .map(|t| AnnualBudget::from_legacy(t, document))
                .collect::<Result<_, _>>()?,
        };
        budgets.sort();
        Ok(budgets)
    }

    /// Lit les tableaux du document (nouvelle clé, sinon ancien pickle décodé
    /// par `parse_pickle`, sinon aucun tableau).
    pub fn load(
        document: &Document,
        parse_pickle: &dyn Fn(&[u8]) -> Option<PyValue>,
    ) -> Result<Self, FeatureError> {
        feature_store::load_with_legacy(document, Self::KEY, Self::LEGACY_KEY, parse_pickle, |v| {
            Self::from_legacy(v, document)
        })
        .unwrap_or_else(|| Ok(Self::default()))
    }

    /// Range les tableaux dans le document (et retire l'ancienne clé).
    pub fn save(&self, document: &mut Document) -> Result<(), FeatureError> {
        feature_store::set(document, Self::KEY, self)?;
        feature_store::remove(document, Self::LEGACY_KEY);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature_store::tests::empty_document;
    use crate::model::{Account, Category, Mode, Operation, StandardMode, Tack};
    use crate::pyvalue::parse_repr;

    fn ymd(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).expect("date valide")
    }

    fn eur(cents: i64) -> Amount {
        Amount::from_cents(cents)
    }

    fn op(date: Date, cents: i64, category: Option<u32>, payee: &str, tack: Tack) -> Operation {
        Operation {
            uid: 0,
            date,
            mark_date: date,
            tack,
            mode: Mode::Standard(if cents < 0 {
                StandardMode::Carte
            } else {
                StandardMode::VirementRecu
            }),
            payee: payee.into(),
            details: String::new(),
            category,
            amount: eur(cents),
            extra: Vec::new(),
        }
    }

    fn account(uid: &str, operations: Vec<Operation>) -> Account {
        Account {
            uid: uid.into(),
            name: uid.into(),
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
            operations,
        }
    }

    const ALIM: u32 = 0;
    const EPICERIE: u32 = 1;
    const RESTO: u32 = 48;
    const SALAIRE: u32 = 7;
    const LOISIRS: u32 = 9;

    /// Petit document : Alimentation (Épicerie, Restaurant), Salaire, Loisirs.
    fn document() -> Document {
        let mut doc = empty_document();
        let cat = |id, name: &str, is_child| Category {
            id,
            name: name.into(),
            is_child,
        };
        doc.categories = vec![
            cat(ALIM, "Alimentation", false),
            cat(EPICERIE, "     Epicerie", true),
            cat(RESTO, "     Restaurant", true),
            cat(SALAIRE, "Salaire", false),
            cat(LOISIRS, "Loisirs", false),
        ];
        let n = Tack::None;
        doc.accounts = vec![
            account(
                "courant",
                vec![
                    op(ymd(2026, 9, 3), -6_000, Some(EPICERIE), "Super U", n),
                    op(
                        ymd(2026, 9, 28),
                        250_000,
                        Some(SALAIRE),
                        "Employeur",
                        Tack::Pointed,
                    ),
                    op(ymd(2026, 10, 2), -4_000, Some(EPICERIE), "Super U", n),
                    op(ymd(2026, 10, 5), -3_000, Some(RESTO), "Pizzeria", n),
                    op(ymd(2026, 10, 6), -1_000, Some(ALIM), "Marché", n),
                    op(ymd(2026, 10, 7), -500, None, "Divers", n),
                    op(ymd(2026, 10, 28), 250_000, Some(SALAIRE), "Employeur", n),
                    op(ymd(2025, 12, 20), -10_000, Some(LOISIRS), "Cinéma", n),
                ],
            ),
            account(
                "livret",
                vec![op(
                    ymd(2026, 10, 15),
                    -2_000,
                    Some(EPICERIE),
                    "Épicerie fine",
                    n,
                )],
            ),
        ];
        doc
    }

    fn budget() -> MonthlyBudget {
        let mut b = MonthlyBudget::default();
        b.set(Some(EPICERIE), BudgetKind::Expense, Some(eur(5_000)));
        b.set(Some(RESTO), BudgetKind::Expense, Some(eur(2_000)));
        b.set(Some(SALAIRE), BudgetKind::Income, Some(eur(240_000)));
        b
    }

    fn row(report: &BudgetReport, category: Option<u32>) -> &BudgetRow {
        report
            .rows
            .iter()
            .find(|r| r.category == category)
            .unwrap_or_else(|| panic!("ligne {category:?} absente"))
    }

    #[test]
    fn current_month_with_subcategories() {
        let doc = document();
        let today = ymd(2026, 10, 11);
        let report = budget().report(&doc, &BudgetOptions::new(today), today);
        let labels: Vec<&str> = report.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                UNCATEGORIZED_LABEL,
                "Alimentation",
                "Epicerie",
                "Restaurant",
                "Salaire",
                "Loisirs"
            ]
        );
        let epicerie = row(&report, Some(EPICERIE));
        assert!(epicerie.is_child);
        assert_eq!(epicerie.kind, Some(BudgetKind::Expense));
        assert_eq!(epicerie.budget, Some(eur(-5_000)));
        assert_eq!(epicerie.real, eur(-6_000), "deux comptes : -40 et -20");
        assert_eq!(epicerie.difference, Some(eur(-1_000)));
        assert_eq!(epicerie.percent, Some(-20.0));
        let alim = row(&report, Some(ALIM));
        assert_eq!(alim.budget, None, "parent non budgété lui-même");
        assert_eq!(alim.real, eur(-1_000), "réel propre seulement");
        assert_eq!(row(&report, Some(LOISIRS)).real, Amount::ZERO);
        assert_eq!(report.expenses.budget, eur(-7_000));
        assert_eq!(report.expenses.real, eur(-9_000));
        assert_eq!(report.income.budget, eur(240_000));
        assert_eq!(report.income.real, eur(250_000));
        assert_eq!(report.total.budget, eur(233_000));
        assert_eq!(report.total.real, eur(241_000));
        assert_eq!(report.total.difference, eur(8_000));
        assert_eq!(report.span, PeriodSpan::SingleMonth);
        assert_eq!(
            report.description,
            "Opérations concernées : toutes les lignes du mois d'octobre 2026"
        );
    }

    #[test]
    fn aggregated_subcategories_and_hidden_lines() {
        let doc = document();
        let today = ymd(2026, 10, 11);
        let options = BudgetOptions {
            show_subcategories: false,
            show_unbudgeted: false,
            ..BudgetOptions::new(today)
        };
        let report = budget().report(&doc, &options, today);
        let labels: Vec<&str> = report.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Alimentation", "Salaire"],
            "sans catégorie et Loisirs non budgétés"
        );
        let alim = row(&report, Some(ALIM));
        assert_eq!(
            alim.budget,
            Some(eur(-7_000)),
            "budgets des enfants agrégés"
        );
        assert_eq!(alim.real, eur(-10_000), "réel propre + enfants");
        assert_eq!(alim.kind, Some(BudgetKind::Expense));

        // « Lignes à zéro » décochée : une catégorie parente sans opération
        // propre mais dont les enfants en ont reste affichée en mode agrégé.
        let mut only_children = document();
        only_children.accounts[0]
            .operations
            .retain(|o| o.category != Some(ALIM));
        let options = BudgetOptions {
            show_subcategories: false,
            show_zero: false,
            ..BudgetOptions::new(today)
        };
        let report = MonthlyBudget::default().report(&only_children, &options, today);
        let labels: Vec<&str> = report.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, [UNCATEGORIZED_LABEL, "Alimentation", "Salaire"]);
        assert_eq!(report.total, BudgetTotal::default());
    }

    #[test]
    fn yearly_and_all_averages() {
        let doc = document();
        let today = ymd(2026, 10, 11);
        let options = BudgetOptions {
            period: BudgetPeriod::current_year(today),
            ..BudgetOptions::new(today)
        };
        let report = budget().report(&doc, &options, today);
        assert_eq!(report.span, PeriodSpan::Months(2), "septembre et octobre");
        assert_eq!(row(&report, Some(SALAIRE)).real, eur(250_000));
        assert_eq!(
            row(&report, Some(EPICERIE)).real,
            eur(-6_000),
            "(-60 - 40 - 20) / 2"
        );
        assert_eq!(
            row(&report, Some(LOISIRS)).real,
            Amount::ZERO,
            "décembre 2025 exclu"
        );
        assert_eq!(
            report.description,
            "Opérations concernées : toutes les lignes de l'année 2026 (2 mois)"
        );

        let options = BudgetOptions {
            period: BudgetPeriod::All,
            ..BudgetOptions::new(today)
        };
        let report = budget().report(&doc, &options, today);
        assert_eq!(report.span, PeriodSpan::Months(3));
        assert_eq!(
            row(&report, Some(LOISIRS)).real,
            Amount(-333_333),
            "-100 / 3, arrondi"
        );

        // Mode jours : du 20/12/2025 au 28/10/2026 inclus = 313 jours.
        let options = BudgetOptions {
            period: BudgetPeriod::All,
            counting: MonthCounting::Days,
            ..BudgetOptions::new(today)
        };
        let report = budget().report(&doc, &options, today);
        assert_eq!(report.span, PeriodSpan::Days(313));
        assert_eq!(report.span.label(), "(10 mois et 8 jours)");
        // -100 × 30,4375 / 313 = -9,7244…
        assert_eq!(row(&report, Some(LOISIRS)).real, Amount(-97_244));
    }

    #[test]
    fn span_labels_and_average() {
        assert_eq!(PeriodSpan::Days(1).label(), "(1 jour)");
        assert_eq!(PeriodSpan::Days(20).label(), "(20 jours)");
        assert_eq!(PeriodSpan::Days(31).label(), "(1 mois)");
        assert_eq!(PeriodSpan::Days(62).label(), "(2 mois et 1 jour)");
        assert_eq!(PeriodSpan::Months(4).label(), "(4 mois)");
        assert_eq!(PeriodSpan::SingleMonth.label(), "");
        assert_eq!(
            PeriodSpan::Days(1).monthly_average(eur(100)),
            Amount(304_375)
        );
        assert_eq!(PeriodSpan::Months(0).monthly_average(eur(100)), eur(100));
        assert_eq!(
            PeriodSpan::Months(3).monthly_average(Amount(-5)),
            Amount(-2),
            "demi éloigné de zéro"
        );
    }

    #[test]
    fn filter_period_and_account_selection() {
        let doc = document();
        let today = ymd(2026, 10, 11);
        let filter = AutoFilter {
            date_from: "1/m-1/a".into(),
            date_to: "dj/m-1/a".into(),
            ..AutoFilter::new("Mois précédent")
        };
        let options = BudgetOptions {
            period: BudgetPeriod::Filter(filter),
            ..BudgetOptions::new(today)
        };
        let report = budget().report(&doc, &options, today);
        assert_eq!(report.span, PeriodSpan::Months(1));
        assert_eq!(
            row(&report, Some(EPICERIE)).real,
            eur(-6_000),
            "septembre seulement"
        );
        assert!(
            report
                .description
                .contains("du 01/09/2026 au 30/09/2026 (1 mois)"),
            "{}",
            report.description
        );

        let pointed = AutoFilter {
            tack: crate::filter::TackFilter::Pointed,
            ..AutoFilter::new("Pointées")
        };
        let options = BudgetOptions {
            period: BudgetPeriod::Filter(pointed),
            accounts: Some(vec![0, 0, 7]),
            ..BudgetOptions::new(today)
        };
        let report = budget().report(&doc, &options, today);
        assert_eq!(row(&report, Some(SALAIRE)).real, eur(250_000));
        assert_eq!(row(&report, Some(EPICERIE)).real, Amount::ZERO);

        let options = BudgetOptions {
            accounts: Some(vec![1]),
            ..BudgetOptions::new(today)
        };
        let report = budget().report(&doc, &options, today);
        assert_eq!(
            row(&report, Some(EPICERIE)).real,
            eur(-2_000),
            "livret seulement"
        );
    }

    #[test]
    fn budget_entry_editing() {
        let mut b = MonthlyBudget::default();
        b.set(Some(1), BudgetKind::Income, Some(eur(-1_000)));
        assert_eq!(b.get(Some(1)), Some(eur(1_000)), "valeur absolue");
        b.set(Some(1), BudgetKind::Expense, Some(eur(1_000)));
        assert_eq!(b.get(Some(1)), Some(eur(-1_000)));
        assert_eq!(b.entries.len(), 1);
        b.set(Some(1), BudgetKind::Expense, None);
        assert_eq!(b.get(Some(1)), None);
        b.set(None, BudgetKind::Expense, Some(eur(10)));
        b.set(None, BudgetKind::Expense, Some(Amount::ZERO));
        assert!(b.entries.is_empty());
    }

    #[test]
    fn amount_input() {
        assert_eq!(parse_amount_input("150"), Some(eur(15_000)));
        assert_eq!(parse_amount_input("1 234,56 €"), Some(eur(123_456)));
        assert_eq!(parse_amount_input("1.234,56"), Some(eur(123_456)));
        assert_eq!(parse_amount_input("-12.5"), Some(eur(-1_250)));
        assert_eq!(parse_amount_input("1200/12"), Some(eur(10_000)));
        assert_eq!(parse_amount_input("100/3"), Some(Amount(333_333)));
        assert_eq!(parse_amount_input("45*2+10,5"), Some(eur(10_050)));
        assert_eq!(parse_amount_input(""), None);
        assert_eq!(parse_amount_input("abc"), None);
        assert_eq!(parse_amount_input("1/0"), None);
        assert_eq!(parse_amount_input("open('x')"), None);
    }

    #[test]
    fn monthly_legacy_and_persistence() {
        // Contenu réel d'Exemple.bp : {24: -12.16}.
        let legacy = parse_repr("{24: -12.16, -1: 30, 5: 0.0}").expect("littéral valide");
        let b = MonthlyBudget::from_legacy(&legacy).expect("structure valide");
        assert_eq!(b.get(Some(24)), Some(eur(-1_216)));
        assert_eq!(b.get(None), Some(eur(3_000)));
        assert_eq!(b.get(Some(5)), None, "montant nul ignoré");
        assert!(MonthlyBudget::from_legacy(&parse_repr("{'a': 1}").expect("valide")).is_err());
        assert!(MonthlyBudget::from_legacy(&parse_repr("{1: 'x'}").expect("valide")).is_err());
        assert!(MonthlyBudget::from_legacy(&parse_repr("[1]").expect("valide")).is_err());

        let mut doc = document();
        let none = |_: &[u8]| None;
        assert_eq!(
            MonthlyBudget::load(&doc, &none).ok(),
            Some(MonthlyBudget::default())
        );
        doc.feature_data.push(crate::model::FeatureData {
            key: MonthlyBudget::LEGACY_KEY.into(),
            value: b"(dp1\nI24\nF-12.16\ns.".to_vec(),
        });
        let parse = |_: &[u8]| Some(legacy.clone());
        assert_eq!(MonthlyBudget::load(&doc, &parse).ok(), Some(b.clone()));
        b.save(&mut doc).expect("enregistrable");
        assert!(feature_store::raw(&doc, MonthlyBudget::LEGACY_KEY).is_none());
        assert_eq!(MonthlyBudget::load(&doc, &none).ok(), Some(b));
    }

    #[test]
    fn grid_columns_integer_division() {
        assert_eq!(GridColumn::from_index(0), Some(GridColumn::Title));
        assert_eq!(
            GridColumn::from_index(1),
            Some(GridColumn::Month {
                month: 0,
                kind: ValueKind::Budget
            })
        );
        assert_eq!(
            GridColumn::from_index(5),
            Some(GridColumn::Month {
                month: 1,
                kind: ValueKind::Real
            })
        );
        assert_eq!(
            GridColumn::from_index(36),
            Some(GridColumn::Month {
                month: 11,
                kind: ValueKind::Difference
            })
        );
        assert_eq!(
            GridColumn::from_index(37),
            Some(GridColumn::Total(ValueKind::Budget))
        );
        assert_eq!(
            GridColumn::from_index(39),
            Some(GridColumn::Total(ValueKind::Difference))
        );
        assert_eq!(GridColumn::from_index(40), None);
        assert_eq!(
            GridColumn::from_index(35).and_then(GridColumn::editable),
            Some((11, ValueKind::Real))
        );
        assert_eq!(
            GridColumn::from_index(36).and_then(GridColumn::editable),
            None
        );
        assert_eq!(
            GridColumn::from_index(37).and_then(GridColumn::editable),
            None,
            "total non modifiable"
        );
    }

    #[test]
    fn annual_tables_management() {
        let mut budgets = AnnualBudgets::default();
        assert_eq!(budgets.create("Maison", 2026), Ok(0));
        assert_eq!(budgets.create(" alpha ", 2025), Ok(0));
        assert_eq!(budgets.labels(), ["alpha (2025)", "Maison (2026)"]);
        assert_eq!(
            budgets.create("Maison", 2027),
            Err(AnnualBudgetError::DuplicateName)
        );
        assert_eq!(
            budgets.create("M", 2027),
            Err(AnnualBudgetError::InvalidName)
        );
        assert_eq!(
            budgets.create("Vacances", 1899),
            Err(AnnualBudgetError::InvalidYear)
        );
        assert_eq!(
            budgets.rename(1, "Maison", 2028),
            Ok(1),
            "même nom autorisé pour lui-même"
        );
        assert_eq!(
            budgets.rename(1, "alpha", 2028),
            Err(AnnualBudgetError::DuplicateName)
        );
        assert_eq!(budgets.duplicate(1), Ok(1));
        assert_eq!(budgets.duplicate(2), Ok(2));
        assert_eq!(
            budgets.labels(),
            [
                "alpha (2025)",
                "Copie de Maison (2028)",
                "Copie de Maison (2) (2028)",
                "Maison (2028)"
            ]
        );
        assert_eq!(budgets.delete(9), Err(AnnualBudgetError::TableNotFound));
        assert!(budgets.delete(0).is_ok());
        assert_eq!(budgets.tables.len(), 3);
    }

    #[test]
    fn annual_lines_values_and_report() {
        let doc = document();
        let mut table = AnnualBudget::new("Test", 2026);
        let alim = LineInput {
            title: " Alimentation ".into(),
            auto_real: true,
            categories: vec![Some(EPICERIE), Some(RESTO), None],
        };
        assert_eq!(table.add_line(&alim), Ok(0));
        assert_eq!(table.lines[0].title, "Alimentation");
        let manual = LineInput {
            title: "Autres".into(),
            auto_real: false,
            categories: Vec::new(),
        };
        assert_eq!(table.add_line(&manual), Ok(1));
        assert_eq!(
            table.add_line(&LineInput {
                title: "A".into(),
                ..manual.clone()
            }),
            Err(AnnualBudgetError::InvalidTitle)
        );
        assert_eq!(
            table.add_line(&LineInput {
                categories: Vec::new(),
                ..alim.clone()
            }),
            Err(AnnualBudgetError::NoCategory)
        );

        table
            .set_value(0, ValueKind::Budget, 0, eur(-55_000), Spread::AllMonths)
            .expect("ok");
        table
            .set_value(0, ValueKind::Budget, 9, eur(-10_000), Spread::Month)
            .expect("ok");
        assert_eq!(
            table.set_value(0, ValueKind::Real, 9, eur(1), Spread::Month),
            Err(AnnualBudgetError::RealIsAutomatic)
        );
        table
            .set_value(1, ValueKind::Real, 1, eur(-100), Spread::EveryThreeMonths)
            .expect("ok");
        let real: Vec<i64> = table.lines[1].real.iter().map(|a| a.0 / 100).collect();
        assert_eq!(real, [0, -100, 0, 0, -100, 0, 0, -100, 0, 0, -100, 0]);
        table
            .set_value(1, ValueKind::Budget, 3, eur(-50), Spread::EveryTwoMonths)
            .expect("ok");
        let budget: Vec<i64> = table.lines[1].budget.iter().map(|a| a.0 / 100).collect();
        assert_eq!(
            budget,
            [0, -50, 0, -50, 0, -50, 0, -50, 0, -50, 0, -50],
            "dans les deux sens"
        );
        assert_eq!(
            table.set_value(1, ValueKind::Budget, 12, eur(1), Spread::Month),
            Err(AnnualBudgetError::InvalidMonth)
        );
        assert_eq!(
            table.set_value(5, ValueKind::Budget, 1, eur(1), Spread::Month),
            Err(AnnualBudgetError::LineNotFound)
        );

        let report = table.report(&doc);
        let alim_row = &report.rows[0];
        assert_eq!(alim_row.months[8].real, eur(-6_000), "septembre : épicerie");
        assert_eq!(
            alim_row.months[9].real,
            eur(-9_500),
            "octobre : -40 -30 -5 (sans catégorie) -20"
        );
        assert_eq!(alim_row.months[9].budget, eur(-10_000));
        assert_eq!(alim_row.months[9].difference, eur(500));
        assert_eq!(alim_row.total.budget, eur(-55_000 * 11 - 10_000));
        assert_eq!(alim_row.total.real, eur(-15_500));
        assert_eq!(report.totals.months[1].real, eur(-100));
        assert_eq!(report.totals.total.real, eur(-15_500 - 400));
        assert_eq!(
            report.totals.total.difference,
            Amount(report.totals.total.real.0 - report.totals.total.budget.0)
        );
        assert_eq!(
            table.lines[0].real,
            [Amount::ZERO; 12],
            "report() ne modifie rien"
        );

        table.set_accounts(&doc, Some(&[1]));
        assert_eq!(table.accounts, Some(vec!["livret".to_string()]));
        table.refresh_auto_real(&doc);
        assert_eq!(table.lines[0].real[9], eur(-2_000));
        assert_eq!(table.lines[1].real[1], eur(-100), "ligne manuelle intacte");

        table
            .update_line(
                0,
                &LineInput {
                    title: "Courses".into(),
                    ..alim
                },
            )
            .expect("ok");
        assert_eq!(table.lines[0].budget[0], eur(-55_000), "montants conservés");
        assert!(table.remove_line(0).is_ok());
        assert_eq!(table.remove_line(5), Err(AnnualBudgetError::LineNotFound));
    }

    #[test]
    fn annual_legacy_from_exemple() {
        // Contenu réel d'Exemple.bp (fabiobpbudgetyear).
        let text = "[{'accounts': [0, 4, 2, 1, 3], 'lines': [{'autocalc_real': 1, 'categs': [0, 1, 48, 2], 'estimated_values': [-550.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'real_values': [-3.81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'title': 'Alimentation'}, {'autocalc_real': 0, 'categs': [], 'estimated_values': [-400.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'real_values': [-350.36, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'title': 'Autres'}], 'name': 'Test de budget', 'year': 2010}]";
        let legacy = parse_repr(text).expect("littéral valide");
        let doc = document();
        let budgets = AnnualBudgets::from_legacy(&legacy, &doc).expect("structure valide");
        let table = &budgets.tables[0];
        assert_eq!(table.name, "Test de budget");
        assert_eq!(table.year, 2010);
        assert_eq!(
            table.accounts,
            Some(vec!["courant".to_string(), "livret".to_string()]),
            "indices 2, 3, 4 inexistants dans le document de test"
        );
        assert_eq!(table.lines.len(), 2);
        assert!(table.lines[0].auto_real);
        assert_eq!(
            table.lines[0].categories,
            [Some(0), Some(1), Some(48), Some(2)]
        );
        assert_eq!(table.lines[0].budget[0], eur(-55_000));
        assert_eq!(table.lines[1].real[0], eur(-35_036));
        assert!(!table.lines[1].auto_real);

        let short = parse_repr("[{'name': 'x', 'year': 2020, 'lines': [{'title': 't', 'categs': [-1], 'estimated_values': [1, 2]}]}]")
            .expect("valide");
        let budgets = AnnualBudgets::from_legacy(&short, &doc).expect("tolérant");
        assert_eq!(budgets.tables[0].accounts, None);
        assert_eq!(budgets.tables[0].lines[0].categories, [None]);
        assert_eq!(budgets.tables[0].lines[0].budget[1], eur(200));
        assert_eq!(budgets.tables[0].lines[0].budget[2], Amount::ZERO);

        for bad in [
            "{}",
            "[{'name': 'x'}]",
            "[{'year': 2020, 'lines': 3}]",
            "[{'year': 2020, 'lines': [{'categs': ['a']}]}]",
        ] {
            let value = parse_repr(bad).expect("littéral valide");
            assert!(AnnualBudgets::from_legacy(&value, &doc).is_err(), "{bad}");
        }

        let mut doc = doc;
        budgets.save(&mut doc).expect("enregistrable");
        let none = |_: &[u8]| None;
        assert_eq!(AnnualBudgets::load(&doc, &none).ok(), Some(budgets));
    }
}
