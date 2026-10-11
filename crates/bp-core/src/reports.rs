//! Rapports et données des graphiques (ex-scripts `totals.py`,
//! `categs_totals.py`, `people_totals.py`, `all_accounts_view.py` et
//! `charts.py`).
//!
//! Ce module ne fait que calculer : l'affichage (grilles, couleurs, export
//! tableur, dessin des graphiques) est laissé à l'interface.

use std::collections::BTreeMap;

use chrono::Datelike;

use crate::filter::normalize;
use crate::model::{Amount, Date, Document, Operation, Tack};
use crate::view::payee_without_ventilation;

/// Noms courts des mois, en en-tête des tableaux croisés.
pub const MONTH_NAMES: [&str; 12] = [
    "Janv", "Févr", "Mars", "Avr", "Mai", "Juin", "Juil", "Août", "Sept", "Oct", "Nov", "Déc",
];

/// Libellé de la ligne des opérations sans catégorie.
pub const NO_CATEGORY: &str = "Sans catégorie";

/// Libellé de la ligne des opérations sans tiers.
pub const NO_PAYEE: &str = "Sans tiers";

/// Filtre de lignes : `(compte, index de l'opération, opération)` → gardée ?
/// Permet de restreindre un rapport aux lignes visibles (filtres courants).
pub type LineFilter<'a> = &'a dyn Fn(usize, usize, &Operation) -> bool;

/// Filtre qui garde toutes les lignes.
pub fn all_lines(_account: usize, _line: usize, _op: &Operation) -> bool {
    true
}

/// Comptes pris en compte par un rapport.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AccountScope {
    /// « Tous les comptes » : les comptes non virtuels, dans l'ordre
    /// d'affichage.
    #[default]
    All,
    /// Un seul compte (index dans `document.accounts`), même virtuel.
    One(usize),
    /// Une sélection de comptes, dans l'ordre donné.
    Selection(Vec<usize>),
}

impl AccountScope {
    /// Index des comptes concernés (les index hors bornes sont ignorés).
    pub fn accounts(&self, document: &Document) -> Vec<usize> {
        let n = document.accounts.len();
        match self {
            Self::All => display_order(document)
                .into_iter()
                .filter(|&i| !document.accounts[i].counts_as_virtual())
                .collect(),
            Self::One(i) if *i < n => vec![*i],
            Self::One(_) => Vec::new(),
            Self::Selection(list) => {
                let mut out = Vec::new();
                for &i in list {
                    if i < n && !out.contains(&i) {
                        out.push(i);
                    }
                }
                out
            }
        }
    }
}

/// Ordre d'affichage des comptes, réparé si `account_order` est incomplet ou
/// incohérent (index hors bornes ou en double ignorés, comptes manquants
/// ajoutés à la fin).
pub fn display_order(document: &Document) -> Vec<usize> {
    let n = document.accounts.len();
    let mut seen = vec![false; n];
    let mut order = Vec::with_capacity(n);
    for &i in &document.account_order {
        if i < n && !seen[i] {
            seen[i] = true;
            order.push(i);
        }
    }
    order.extend((0..n).filter(|&i| !seen[i]));
    order
}

/// Opérations des comptes de `scope` retenues par `keep`.
fn selected_operations<'d>(
    document: &'d Document,
    scope: &AccountScope,
    keep: LineFilter<'d>,
) -> impl Iterator<Item = (usize, usize, &'d Operation)> + 'd {
    scope.accounts(document).into_iter().flat_map(move |a| {
        document.accounts[a]
            .operations
            .iter()
            .enumerate()
            .filter(move |(l, op)| keep(a, *l, op))
            .map(move |(l, op)| (a, l, op))
    })
}

fn add(a: Amount, b: Amount) -> Amount {
    Amount(a.0 + b.0)
}

/// Division arrondie au plus proche (demi éloigné de zéro).
fn div_round(value: Amount, divisor: i64) -> Amount {
    let (q, r) = (value.0 / divisor, value.0 % divisor);
    Amount(if 2 * r.abs() >= divisor.abs() {
        q + value.0.signum() * divisor.signum()
    } else {
        q
    })
}

fn year_month(date: Date) -> (i32, u32) {
    let d = date.to_naive();
    (d.year(), d.month())
}

// ---------------------------------------------------------------------------
// Total des comptes
// ---------------------------------------------------------------------------

/// Soldes pointé, à venir et final.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BalanceTotals {
    /// Somme des opérations pointées ou rapprochées.
    pub pointed: Amount,
    /// Somme des opérations non pointées.
    pub upcoming: Amount,
    /// Solde final (pointé + à venir).
    pub balance: Amount,
}

impl BalanceTotals {
    /// Ajoute une opération.
    pub fn add(&mut self, op: &Operation) {
        if op.tack == Tack::None {
            self.upcoming = add(self.upcoming, op.amount);
        } else {
            self.pointed = add(self.pointed, op.amount);
        }
        self.balance = add(self.balance, op.amount);
    }

    fn merge(&mut self, other: &BalanceTotals) {
        self.pointed = add(self.pointed, other.pointed);
        self.upcoming = add(self.upcoming, other.upcoming);
        self.balance = add(self.balance, other.balance);
    }
}

/// Ligne du rapport « Total des comptes ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTotalsRow {
    /// Index du compte dans `document.accounts`.
    pub account: usize,
    pub name: String,
    pub totals: BalanceTotals,
}

/// Rapport « Total des comptes ».
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountTotals {
    pub rows: Vec<AccountTotalsRow>,
    /// Ligne « Total ».
    pub total: BalanceTotals,
}

/// Total des comptes non virtuels, dans l'ordre d'affichage (`totals.py`).
///
/// Pointé = opérations pointées ou rapprochées ; à venir = les autres (le
/// relevé multicompte d'origine inversait ces deux totaux).
pub fn account_totals(document: &Document) -> AccountTotals {
    let mut report = AccountTotals::default();
    for account in AccountScope::All.accounts(document) {
        let mut totals = BalanceTotals::default();
        for op in &document.accounts[account].operations {
            totals.add(op);
        }
        report.total.merge(&totals);
        report.rows.push(AccountTotalsRow {
            account,
            name: document.accounts[account].name.clone(),
            totals,
        });
    }
    report
}

// ---------------------------------------------------------------------------
// Tableaux croisés : totaux par catégorie et par tiers
// ---------------------------------------------------------------------------

/// Objectif d'un tableau croisé (liste « Objectif » de l'original).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotGoal {
    /// Une colonne par mois (d'une année, ou de toutes années confondues).
    #[default]
    MonthlyTotals,
    /// Une colonne par année.
    YearlyTotals,
    /// Une colonne par année, valeurs divisées par 12 (moyenne mensuelle).
    YearlyAverages,
}

/// Options des tableaux croisés.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PivotOptions {
    pub scope: AccountScope,
    pub goal: PivotGoal,
    /// Année affichée en totaux mensuels (`None` = toutes années, mois
    /// cumulés). Ignorée pour les objectifs annuels.
    pub year: Option<i32>,
    /// « Toutes les catégories » : une ligne par catégorie et sous-catégorie ;
    /// sinon les sous-catégories sont agrégées dans leur parent.
    pub all_categories: bool,
    /// « Totaux ligne à ligne » : débit et crédit calculés sur les opérations
    /// plutôt que sur les cellules.
    pub line_totals: bool,
    /// Totaux par tiers : « Supprimer dates et nombres » des tiers.
    pub remove_dates_and_numbers: bool,
}

/// Clé d'une ligne de tableau croisé.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PivotKey {
    /// Catégorie (`None` = sans catégorie).
    Category(Option<u32>),
    /// Tiers normalisé.
    Payee(String),
}

/// Ligne d'un tableau croisé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PivotRow {
    pub key: PivotKey,
    pub label: String,
    /// Sous-catégorie (affichée en retrait).
    pub is_child: bool,
    /// Une valeur par colonne, la dernière étant le total de la ligne.
    pub values: Vec<Amount>,
}

/// Tableau croisé : lignes × (mois ou années), plus une colonne « Total ».
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pivot {
    /// En-têtes des colonnes de valeurs ; la dernière est « Total ».
    pub columns: Vec<String>,
    /// Ligne « Solde reporté » (totaux mensuels d'une année) : solde de toutes
    /// les opérations antérieures au mois ; la colonne Total donne le solde à
    /// la fin de l'année.
    pub carried_balance: Option<Vec<Amount>>,
    pub rows: Vec<PivotRow>,
    pub debit: Vec<Amount>,
    pub credit: Vec<Amount>,
    pub total: Vec<Amount>,
}

/// Années couvertes par les opérations des comptes de `scope`, croissantes
/// (liste « Années » ; vide si aucun compte n'a d'opération).
pub fn available_years(document: &Document, scope: &AccountScope) -> Vec<i32> {
    let mut years: Vec<i32> = selected_operations(document, scope, &all_lines)
        .map(|(_, _, op)| year_month(op.date).0)
        .collect();
    years.sort_unstable();
    years.dedup();
    match (years.first(), years.last()) {
        (Some(&first), Some(&last)) => (first..=last).collect(),
        _ => Vec::new(),
    }
}

/// Une opération réduite à ce qu'utilise un tableau croisé.
struct PivotEntry<K> {
    year: i32,
    month: u32,
    amount: Amount,
    key: K,
}

/// Construit un tableau croisé. `rows` donne les lignes possibles dans
/// l'ordre ; les opérations dont la clé n'y figure pas sont ignorées.
fn build_pivot<K: Ord + Clone>(
    entries: &[PivotEntry<K>],
    options: &PivotOptions,
    rows: Vec<(K, PivotKey, String, bool)>,
) -> Pivot {
    // Colonnes : mois, ou années de la première à la dernière opération.
    // Correction : l'original échouait (`min()` sur une liste vide) dès qu'un
    // compte n'avait aucune opération.
    let first_year = entries.iter().map(|e| e.year).min();
    let columns: Vec<String> = match options.goal {
        PivotGoal::MonthlyTotals => MONTH_NAMES.iter().map(|m| m.to_string()).collect(),
        PivotGoal::YearlyTotals | PivotGoal::YearlyAverages => {
            match (first_year, entries.iter().map(|e| e.year).max()) {
                (Some(first), Some(last)) => (first..=last).map(|y| y.to_string()).collect(),
                _ => Vec::new(),
            }
        }
    };
    // (année, mois) → index de colonne.
    let column_of = |year: i32, month: u32| -> Option<usize> {
        match options.goal {
            PivotGoal::MonthlyTotals => options
                .year
                .is_none_or(|y| y == year)
                .then_some(month as usize - 1),
            PivotGoal::YearlyTotals | PivotGoal::YearlyAverages => {
                first_year.map(|first| (year - first) as usize)
            }
        }
    };
    let width = columns.len();
    let index: BTreeMap<K, usize> = rows
        .iter()
        .enumerate()
        .map(|(i, (k, ..))| (k.clone(), i))
        .collect();
    let mut cells = vec![vec![Amount::ZERO; width]; rows.len()];
    let mut line_debit = vec![Amount::ZERO; width];
    let mut line_credit = vec![Amount::ZERO; width];
    for entry in entries {
        let Some(col) = column_of(entry.year, entry.month) else {
            continue;
        };
        if let Some(&row) = index.get(&entry.key) {
            cells[row][col] = add(cells[row][col], entry.amount);
        }
        if entry.amount.is_negative() {
            line_debit[col] = add(line_debit[col], entry.amount);
        } else {
            line_credit[col] = add(line_credit[col], entry.amount);
        }
    }
    if options.goal == PivotGoal::YearlyAverages {
        for value in cells
            .iter_mut()
            .flatten()
            .chain(line_debit.iter_mut())
            .chain(line_credit.iter_mut())
        {
            *value = div_round(*value, 12);
        }
    }
    let (debit, credit) = if options.line_totals {
        (line_debit, line_credit)
    } else {
        let mut debit = vec![Amount::ZERO; width];
        let mut credit = vec![Amount::ZERO; width];
        for row in &cells {
            for (col, &value) in row.iter().enumerate() {
                if value.is_negative() {
                    debit[col] = add(debit[col], value);
                } else {
                    credit[col] = add(credit[col], value);
                }
            }
        }
        (debit, credit)
    };
    let total: Vec<Amount> = debit
        .iter()
        .zip(&credit)
        .map(|(&d, &c)| add(d, c))
        .collect();

    // Colonne « Total » : somme de chaque ligne.
    let with_total = |mut values: Vec<Amount>| {
        let sum = values.iter().copied().sum();
        values.push(sum);
        values
    };
    let carried_balance = match (options.goal, options.year) {
        (PivotGoal::MonthlyTotals, Some(year)) => {
            let mut carried = vec![Amount::ZERO; 12];
            for entry in entries {
                for (m, value) in carried.iter_mut().enumerate() {
                    let month = m as u32 + 1;
                    if entry.year < year || (entry.year == year && entry.month < month) {
                        *value = add(*value, entry.amount);
                    }
                }
            }
            let end_of_year = add(carried[11], total[11]);
            carried.push(end_of_year);
            Some(carried)
        }
        _ => None,
    };
    let mut columns = columns;
    columns.push("Total".to_string());
    Pivot {
        columns,
        carried_balance,
        rows: rows
            .into_iter()
            .zip(cells)
            .map(|((_, key, label, is_child), values)| PivotRow {
                key,
                label,
                is_child,
                values: with_total(values),
            })
            .collect(),
        debit: with_total(debit),
        credit: with_total(credit),
        total: with_total(total),
    }
}

/// Catégorie de rattachement d'une opération : `None` si absente ou inconnue ;
/// le parent si `roll_up` et que c'est une sous-catégorie.
fn category_key(document: &Document, category: Option<u32>, roll_up: bool) -> Option<u32> {
    let id = category?;
    let pos = document.categories.iter().position(|c| c.id == id)?;
    if roll_up && document.categories[pos].is_child {
        return document.categories[..pos]
            .iter()
            .rev()
            .find(|c| !c.is_child)
            .map_or(Some(id), |parent| Some(parent.id));
    }
    Some(id)
}

/// Totaux par catégorie (`categs_totals.py`).
///
/// Lignes : « Sans catégorie » (retirée si elle est vide), puis les
/// catégories dans l'ordre du document (seulement les principales si
/// `all_categories` est faux). Une opération dont la catégorie n'existe plus
/// compte dans « Sans catégorie ».
pub fn category_totals(document: &Document, options: &PivotOptions) -> Pivot {
    let roll_up = !options.all_categories;
    let entries: Vec<PivotEntry<Option<u32>>> =
        selected_operations(document, &options.scope, &all_lines)
            .map(|(_, _, op)| {
                let (year, month) = year_month(op.date);
                PivotEntry {
                    year,
                    month,
                    amount: op.amount,
                    key: category_key(document, op.category, roll_up),
                }
            })
            .collect();
    let mut rows = vec![(
        None,
        PivotKey::Category(None),
        NO_CATEGORY.to_string(),
        false,
    )];
    rows.extend(
        document
            .categories
            .iter()
            .filter(|c| options.all_categories || !c.is_child)
            .map(|c| {
                (
                    Some(c.id),
                    PivotKey::Category(Some(c.id)),
                    c.name.clone(),
                    c.is_child,
                )
            }),
    );
    let mut pivot = build_pivot(&entries, options, rows);
    if pivot
        .rows
        .first()
        .is_some_and(|r| r.key == PivotKey::Category(None) && r.values.iter().all(|v| v.0 == 0))
    {
        pivot.rows.remove(0);
    }
    pivot
}

/// Retire d'un tiers les mots numériques (dates, nombres) et ceux entre
/// parenthèses ou crochets (`remove_dates_and_numbers`).
pub fn remove_dates_and_numbers(payee: &str) -> String {
    payee
        .split(' ')
        .filter(|word| {
            let bracketed = (word.starts_with('(') && word.ends_with(')'))
                || (word.starts_with('[') && word.ends_with(']'));
            let digits: String = word.chars().filter(|c| !"/-.,".contains(*c)).collect();
            let numeric = !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
            !(bracketed || numeric)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Clé de regroupement d'un tiers : sans préfixe de ventilation, en
/// minuscules, sans accents ni ponctuation, espaces réduites.
pub fn payee_key(payee: &str, remove_numbers: bool) -> String {
    let payee = payee_without_ventilation(payee);
    let payee = if remove_numbers {
        remove_dates_and_numbers(payee)
    } else {
        payee.to_string()
    };
    normalize(&payee)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Totaux par tiers (`people_totals.py`).
///
/// Les tiers sont regroupés par [`payee_key`] ; le libellé est le premier
/// tiers rencontré. Seuls les tiers ayant une opération dans les colonnes
/// affichées sont listés, triés par clé.
pub fn payee_totals(document: &Document, options: &PivotOptions) -> Pivot {
    let clean = |payee: &str| {
        let payee = payee_without_ventilation(payee);
        let payee = if options.remove_dates_and_numbers {
            remove_dates_and_numbers(payee)
        } else {
            payee.to_string()
        };
        payee.trim().to_string()
    };
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let mut entries = Vec::new();
    for (_, _, op) in selected_operations(document, &options.scope, &all_lines) {
        let (year, month) = year_month(op.date);
        let key = payee_key(&op.payee, options.remove_dates_and_numbers);
        let shown =
            options.goal != PivotGoal::MonthlyTotals || options.year.is_none_or(|y| y == year);
        if shown && !labels.contains_key(&key) {
            let label = if key.is_empty() {
                NO_PAYEE.to_string()
            } else {
                clean(&op.payee)
            };
            labels.insert(key.clone(), label);
        }
        entries.push(PivotEntry {
            year,
            month,
            amount: op.amount,
            key,
        });
    }
    let rows = labels
        .into_iter()
        .map(|(key, label)| (key.clone(), PivotKey::Payee(key), label, false))
        .collect();
    build_pivot(&entries, options, rows)
}

// ---------------------------------------------------------------------------
// Relevé multicompte
// ---------------------------------------------------------------------------

/// Ligne du relevé multicompte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatementLine {
    /// Index du compte dans `document.accounts`.
    pub account: usize,
    /// Index de l'opération dans le compte.
    pub line: usize,
}

/// Relevé multicompte : lignes fusionnées et totaux.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MultiStatement {
    pub lines: Vec<StatementLine>,
    /// Totaux « Pointé », « A venir », « Solde final » des lignes retenues.
    pub totals: BalanceTotals,
}

/// Relevé multicompte (`all_accounts_view.py`) : lignes de `accounts`
/// retenues par `keep` (en pratique, les lignes visibles avec les filtres
/// courants), triées par date puis dans l'ordre de `accounts`, puis dans
/// l'ordre du compte.
///
/// Corrections : les totaux « Pointé » et « A venir » étaient inversés
/// (les lignes non pointées étaient additionnées dans « Pointé »).
///
/// La sélection de comptes était mémorisée sous la clé `fabiobpviewall` :
/// pickle d'un dictionnaire `{"accounts": [index réels des comptes]}`.
pub fn multi_account_statement(
    document: &Document,
    accounts: &[usize],
    keep: LineFilter,
) -> MultiStatement {
    let scope = AccountScope::Selection(accounts.to_vec());
    let order = scope.accounts(document);
    let mut lines: Vec<(Date, usize, StatementLine)> = Vec::new();
    let mut totals = BalanceTotals::default();
    for (rank, &account) in order.iter().enumerate() {
        for (line, op) in document.accounts[account].operations.iter().enumerate() {
            if keep(account, line, op) {
                totals.add(op);
                lines.push((op.date, rank, StatementLine { account, line }));
            }
        }
    }
    lines.sort_by_key(|(date, rank, l)| (*date, *rank, l.line));
    MultiStatement {
        lines: lines.into_iter().map(|(.., l)| l).collect(),
        totals,
    }
}

// ---------------------------------------------------------------------------
// Graphiques
// ---------------------------------------------------------------------------

/// Nature des valeurs d'un graphique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartUnit {
    /// Montants dans la devise des comptes.
    Amount,
    /// Nombres d'opérations.
    Count,
}

/// Point d'une série : libellé et valeur.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

/// Série de données (équivalent des noms `/s=Titre` de l'original).
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
    pub name: String,
    pub points: Vec<ChartPoint>,
}

/// Données d'un graphique, prêtes à dessiner.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartData {
    pub unit: ChartUnit,
    pub series: Vec<ChartSeries>,
}

impl ChartData {
    fn single(unit: ChartUnit, name: &str, points: Vec<(String, f64)>) -> Self {
        ChartData {
            unit,
            series: vec![ChartSeries {
                name: name.to_string(),
                points: points
                    .into_iter()
                    .map(|(label, value)| ChartPoint { label, value })
                    .collect(),
            }],
        }
    }

    /// Retire les points nuls dans toutes les séries (case « Valeurs nulles »
    /// décochée).
    pub fn without_zero_points(mut self) -> Self {
        let len = self
            .series
            .iter()
            .map(|s| s.points.len())
            .max()
            .unwrap_or(0);
        let keep: Vec<bool> = (0..len)
            .map(|i| {
                self.series
                    .iter()
                    .any(|s| s.points.get(i).is_some_and(|p| p.value != 0.0))
            })
            .collect();
        for series in &mut self.series {
            let mut i = 0;
            series.points.retain(|_| {
                let kept = keep.get(i).copied().unwrap_or(false);
                i += 1;
                kept
            });
        }
        self
    }
}

/// Soldes des comptes non virtuels (toutes les lignes, sans filtre) ;
/// `detailed` : trois séries « Opérations pointées », « A venir », « Solde ».
pub fn chart_account_balances(document: &Document, detailed: bool) -> ChartData {
    let totals = account_totals(document);
    let series = |name: &str, pick: fn(&BalanceTotals) -> Amount| ChartSeries {
        name: name.to_string(),
        points: totals
            .rows
            .iter()
            .map(|row| ChartPoint {
                label: row.name.clone(),
                value: pick(&row.totals).as_f64(),
            })
            .collect(),
    };
    let series = if detailed {
        vec![
            series("Opérations pointées", |t| t.pointed),
            series("A venir", |t| t.upcoming),
            series("Solde", |t| t.balance),
        ]
    } else {
        vec![series("Solde", |t| t.balance)]
    };
    ChartData {
        unit: ChartUnit::Amount,
        series,
    }
}

/// Répartition par catégorie (principales, ou toutes si `all_categories`).
pub fn chart_categories(
    document: &Document,
    scope: &AccountScope,
    keep: LineFilter,
    all_categories: bool,
) -> ChartData {
    let mut sums: BTreeMap<Option<u32>, Amount> = BTreeMap::new();
    for (_, _, op) in selected_operations(document, scope, keep) {
        let key = category_key(document, op.category, !all_categories);
        let sum = sums.entry(key).or_default();
        *sum = add(*sum, op.amount);
    }
    let value = |key: Option<u32>| sums.get(&key).copied().unwrap_or_default().as_f64();
    let mut points = vec![(NO_CATEGORY.to_string(), value(None))];
    points.extend(
        document
            .categories
            .iter()
            .filter(|c| all_categories || !c.is_child)
            .map(|c| {
                let label = document
                    .category_label(c.id)
                    .unwrap_or_else(|| c.name.clone());
                (label, value(Some(c.id)))
            }),
    );
    ChartData::single(ChartUnit::Amount, "Catégories", points)
}

/// Répartition mensuelle : total des opérations par mois de l'année, toutes
/// années confondues.
pub fn chart_months(document: &Document, scope: &AccountScope, keep: LineFilter) -> ChartData {
    const NAMES: [&str; 12] = [
        "Jan", "Fév", "Mar", "Avr", "Mai", "Jun", "Jul", "Aoû", "Sep", "Oct", "Nov", "Déc",
    ];
    let mut values = [Amount::ZERO; 12];
    for (_, _, op) in selected_operations(document, scope, keep) {
        let m = year_month(op.date).1 as usize - 1;
        values[m] = add(values[m], op.amount);
    }
    let points = NAMES
        .iter()
        .zip(values)
        .map(|(name, v)| (name.to_string(), v.as_f64()))
        .collect();
    ChartData::single(ChartUnit::Amount, "Répartition mensuelle", points)
}

/// Total par mode de paiement (modes standard puis personnalisés ; un chèque
/// numéroté compte dans « Chèque émis »).
pub fn chart_modes(document: &Document, scope: &AccountScope, keep: LineFilter) -> ChartData {
    let modes = document.mode_list();
    let mut values = vec![Amount::ZERO; modes.len()];
    for (_, _, op) in selected_operations(document, scope, keep) {
        if let Some(i) = modes.iter().position(|m| *m == op.mode) {
            values[i] = add(values[i], op.amount);
        }
    }
    let points = modes
        .iter()
        .zip(values)
        .map(|(mode, v)| (mode.label().to_string(), v.as_f64()))
        .collect();
    ChartData::single(ChartUnit::Amount, "Modes de paiement", points)
}

/// Bornes des tranches de montant (en unités monétaires).
pub const AMOUNT_LEVELS: [i64; 12] = [0, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000];

/// Nombre d'opérations par tranche de montant absolu : « De 0 à 5 »,
/// « De 5 à 10 »… « > 10000 ». Les montants nuls sont ignorés.
pub fn chart_amount_levels(
    document: &Document,
    scope: &AccountScope,
    keep: LineFilter,
) -> ChartData {
    let n = AMOUNT_LEVELS.len();
    let mut counts = vec![0u64; n];
    for (_, _, op) in selected_operations(document, scope, keep) {
        let abs = op.amount.0.unsigned_abs();
        if abs == 0 {
            continue;
        }
        let bucket = AMOUNT_LEVELS[1..]
            .iter()
            .position(|&level| abs < (level * Amount::SCALE) as u64)
            .unwrap_or(n - 1);
        counts[bucket] += 1;
    }
    let points = (0..n)
        .map(|i| {
            let label = if i + 1 < n {
                format!("De {} à {}", AMOUNT_LEVELS[i], AMOUNT_LEVELS[i + 1])
            } else {
                format!("> {}", AMOUNT_LEVELS[n - 1])
            };
            (label, counts[i] as f64)
        })
        .collect();
    ChartData::single(ChartUnit::Count, "Niveaux des opérations", points)
}

/// Total des opérations par jour du mois (« Le 1er » à « Le 31 »).
pub fn chart_day_of_month(
    document: &Document,
    scope: &AccountScope,
    keep: LineFilter,
) -> ChartData {
    let mut values = [Amount::ZERO; 31];
    for (_, _, op) in selected_operations(document, scope, keep) {
        let d = op.date.to_naive().day() as usize - 1;
        values[d] = add(values[d], op.amount);
    }
    let points = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let label = if i == 0 {
                "Le 1er".to_string()
            } else {
                format!("Le {}", i + 1)
            };
            (label, v.as_f64())
        })
        .collect();
    ChartData::single(ChartUnit::Amount, "Répartition au cours du mois", points)
}

/// Pas de la courbe du solde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceStep {
    /// Un point par date ayant au moins une opération.
    Day,
    /// Un point par mois (solde en fin de mois), mois sans opération compris.
    Month,
}

/// Solde cumulé dans le temps (somme des comptes de `scope`). Nouveau : ce
/// graphique n'existait pas dans l'original.
pub fn chart_balance_over_time(
    document: &Document,
    scope: &AccountScope,
    keep: LineFilter,
    step: BalanceStep,
) -> ChartData {
    let mut by_date: BTreeMap<Date, Amount> = BTreeMap::new();
    for (_, _, op) in selected_operations(document, scope, keep) {
        let sum = by_date.entry(op.date).or_default();
        *sum = add(*sum, op.amount);
    }
    let mut points = Vec::new();
    let mut balance = Amount::ZERO;
    match step {
        BalanceStep::Day => {
            for (date, amount) in by_date {
                balance = add(balance, amount);
                points.push((date.to_string(), balance.as_f64()));
            }
        }
        BalanceStep::Month => {
            let mut by_month: BTreeMap<(i32, u32), Amount> = BTreeMap::new();
            for (date, amount) in by_date {
                let sum = by_month.entry(year_month(date)).or_default();
                *sum = add(*sum, amount);
            }
            if let (Some(&first), Some(&last)) = (by_month.keys().next(), by_month.keys().last()) {
                let (mut y, mut m) = first;
                while (y, m) <= last {
                    balance = add(balance, by_month.get(&(y, m)).copied().unwrap_or_default());
                    points.push((format!("{m:02}/{y}"), balance.as_f64()));
                    m += 1;
                    if m > 12 {
                        m = 1;
                        y += 1;
                    }
                }
            }
        }
    }
    ChartData::single(ChartUnit::Amount, "Solde", points)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::*;

    pub(crate) fn op(uid: u64, date: (i32, u32, u32), cents: i64) -> Operation {
        let date = Date::from_ymd(date.0, date.1, date.2).expect("date de test");
        Operation {
            uid,
            date,
            mark_date: date,
            tack: Tack::None,
            mode: Mode::Standard(if cents < 0 {
                StandardMode::Carte
            } else {
                StandardMode::VirementRecu
            }),
            payee: String::new(),
            details: String::new(),
            category: None,
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        }
    }

    pub(crate) fn account(uid: &str, name: &str, operations: Vec<Operation>) -> Account {
        Account {
            uid: uid.into(),
            name: name.into(),
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

    pub(crate) fn document(accounts: Vec<Account>) -> Document {
        let n = accounts.len();
        Document {
            group_name: String::new(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: Vec::new(),
            accounts,
            account_order: (0..n).collect(),
            default_account: 0,
            categories: vec![
                Category {
                    id: 1,
                    name: "Maison".into(),
                    is_child: false,
                },
                Category {
                    id: 2,
                    name: "Loyer".into(),
                    is_child: true,
                },
                Category {
                    id: 3,
                    name: "Énergie".into(),
                    is_child: true,
                },
                Category {
                    id: 4,
                    name: "Salaire".into(),
                    is_child: false,
                },
            ],
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        }
    }

    fn with(mut o: Operation, f: impl FnOnce(&mut Operation)) -> Operation {
        f(&mut o);
        o
    }

    /// Deux comptes réels, un compte virtuel et un compte vide.
    fn sample() -> Document {
        let courant = vec![
            with(op(1, (2024, 1, 5), -80_000), |o| {
                o.category = Some(2);
                o.tack = Tack::Pointed;
                o.payee = "Agence Durand".into();
            }),
            with(op(2, (2024, 1, 20), -6_000), |o| {
                o.category = Some(3);
                o.tack = Tack::Reconciled;
                o.payee = "EDF 01/2024".into();
            }),
            with(op(3, (2024, 2, 1), 250_000), |o| {
                o.category = Some(4);
                o.payee = "Société X".into();
            }),
            with(op(4, (2023, 12, 28), 10_000), |o| {
                o.payee = "Remboursement".into();
            }),
            with(op(5, (2024, 2, 20), -7_000), |o| {
                o.category = Some(3);
                o.payee = "edf 02/2024".into();
            }),
        ];
        let livret = vec![with(op(6, (2024, 3, 1), 50_000), |o| {
            o.category = Some(99); // catégorie supprimée
            o.tack = Tack::Pointed;
            o.payee = "[1/2] Société X".into();
        })];
        let mut virtuel = account("v", "_Virtuel", vec![op(7, (2024, 1, 1), 1_000_000)]);
        virtuel.is_virtual = true;
        let mut doc = document(vec![
            account("a", "Courant", courant),
            account("b", "Livret", livret),
            virtuel,
            account("c", "Vide", Vec::new()),
        ]);
        doc.account_order = vec![1, 0, 2, 3];
        doc
    }

    fn cents(c: i64) -> Amount {
        Amount::from_cents(c)
    }

    #[test]
    fn totals_of_accounts() {
        let report = account_totals(&sample());
        let names: Vec<&str> = report.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["Livret", "Courant", "Vide"],
            "ordre d'affichage, sans virtuel"
        );
        let courant = &report.rows[1].totals;
        assert_eq!(courant.pointed, cents(-86_000));
        assert_eq!(courant.upcoming, cents(253_000));
        assert_eq!(courant.balance, cents(167_000));
        assert_eq!(report.rows[2].totals, BalanceTotals::default());
        assert_eq!(report.total.pointed, cents(-36_000));
        assert_eq!(report.total.upcoming, cents(253_000));
        assert_eq!(report.total.balance, cents(217_000));
    }

    #[test]
    fn display_order_is_repaired() {
        let mut doc = sample();
        doc.account_order = vec![2, 9, 2];
        assert_eq!(display_order(&doc), [2, 0, 1, 3]);
    }

    #[test]
    fn categories_monthly_with_year() {
        let doc = sample();
        let options = PivotOptions {
            year: Some(2024),
            ..Default::default()
        };
        let pivot = category_totals(&doc, &options);
        assert_eq!(pivot.columns.len(), 13);
        assert_eq!(pivot.columns[12], "Total");
        let labels: Vec<&str> = pivot.rows.iter().map(|r| r.label.as_str()).collect();
        // Catégorie 99 inconnue → « Sans catégorie » (mars, Livret).
        assert_eq!(labels, [NO_CATEGORY, "Maison", "Salaire"]);
        let maison = &pivot.rows[1];
        assert_eq!(maison.values[0], cents(-86_000), "loyer + énergie agrégés");
        assert_eq!(maison.values[1], cents(-7_000));
        assert_eq!(maison.values[12], cents(-93_000));
        assert_eq!(pivot.rows[0].values[2], cents(50_000));
        let carried = pivot.carried_balance.as_ref().unwrap();
        assert_eq!(carried[0], cents(10_000), "opération de décembre 2023");
        assert_eq!(carried[1], cents(-76_000));
        assert_eq!(carried[2], cents(167_000));
        assert_eq!(carried[12], cents(217_000), "solde en fin d'année");
        assert_eq!(pivot.debit[0], cents(-86_000));
        assert_eq!(pivot.credit[1], cents(250_000));
        assert_eq!(pivot.total[12], cents(207_000));
    }

    #[test]
    fn categories_all_and_line_totals() {
        let doc = sample();
        let options = PivotOptions {
            scope: AccountScope::One(0),
            goal: PivotGoal::YearlyTotals,
            all_categories: true,
            line_totals: true,
            ..Default::default()
        };
        let pivot = category_totals(&doc, &options);
        assert_eq!(pivot.columns, ["2023", "2024", "Total"]);
        let labels: Vec<(&str, bool)> = pivot
            .rows
            .iter()
            .map(|r| (r.label.as_str(), r.is_child))
            .collect();
        assert_eq!(
            labels,
            [
                (NO_CATEGORY, false),
                ("Maison", false),
                ("Loyer", true),
                ("Énergie", true),
                ("Salaire", false)
            ]
        );
        assert_eq!(
            pivot.rows[3].values,
            [cents(0), cents(-13_000), cents(-13_000)]
        );
        assert_eq!(pivot.debit, [cents(0), cents(-93_000), cents(-93_000)]);
        assert_eq!(
            pivot.credit,
            [cents(10_000), cents(250_000), cents(260_000)]
        );
        assert!(pivot.carried_balance.is_none());
    }

    #[test]
    fn line_totals_differ_from_cell_totals() {
        let mut doc = sample();
        // Un remboursement dans « Énergie » en janvier 2024 : la cellule
        // nette reste négative, mais le crédit ligne à ligne le voit.
        doc.accounts[0]
            .operations
            .push(with(op(8, (2024, 1, 25), 1_000), |o| o.category = Some(3)));
        let mut options = PivotOptions {
            scope: AccountScope::One(0),
            year: Some(2024),
            ..Default::default()
        };
        let cells = category_totals(&doc, &options);
        assert_eq!(cells.credit[0], cents(0));
        assert_eq!(cells.debit[0], cents(-85_000));
        options.line_totals = true;
        let lines = category_totals(&doc, &options);
        assert_eq!(lines.credit[0], cents(1_000));
        assert_eq!(lines.debit[0], cents(-86_000));
        assert_eq!(cells.total, lines.total);
    }

    #[test]
    fn averages_divide_by_twelve() {
        let doc = sample();
        let options = PivotOptions {
            scope: AccountScope::One(0),
            goal: PivotGoal::YearlyAverages,
            ..Default::default()
        };
        let pivot = category_totals(&doc, &options);
        let salaire = pivot.rows.iter().find(|r| r.label == "Salaire").unwrap();
        // 2 500 / 12 = 208,3333
        assert_eq!(salaire.values[1], Amount(2_083_333));
    }

    #[test]
    fn empty_accounts_do_not_panic() {
        let doc = sample();
        for goal in [
            PivotGoal::MonthlyTotals,
            PivotGoal::YearlyTotals,
            PivotGoal::YearlyAverages,
        ] {
            let options = PivotOptions {
                scope: AccountScope::One(3),
                goal,
                ..Default::default()
            };
            let pivot = category_totals(&doc, &options);
            assert!(pivot.rows.iter().all(|r| r.values.iter().all(|v| v.0 == 0)));
            let pivot = payee_totals(&doc, &options);
            assert!(pivot.rows.is_empty());
        }
        let empty = document(Vec::new());
        assert!(available_years(&empty, &AccountScope::All).is_empty());
        assert_eq!(
            category_totals(
                &empty,
                &PivotOptions {
                    goal: PivotGoal::YearlyTotals,
                    ..Default::default()
                }
            )
            .columns,
            ["Total"]
        );
        assert_eq!(available_years(&sample(), &AccountScope::All), [2023, 2024]);
    }

    #[test]
    fn payees_grouped_and_all_years_monthly() {
        let doc = sample();
        let options = PivotOptions {
            remove_dates_and_numbers: true,
            ..Default::default()
        };
        let pivot = payee_totals(&doc, &options);
        let labels: Vec<&str> = pivot.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Agence Durand", "EDF", "Remboursement", "Société X"]
        );
        let edf = &pivot.rows[1];
        assert_eq!(edf.key, PivotKey::Payee("edf".into()));
        assert_eq!(edf.values[0], cents(-6_000));
        assert_eq!(edf.values[1], cents(-7_000));
        // Correction : sans année choisie, les mois cumulaient zéro.
        let societe = &pivot.rows[3];
        assert_eq!(societe.values[1], cents(250_000));
        assert_eq!(societe.values[2], cents(50_000), "part ventilée regroupée");
        assert_eq!(pivot.rows[2].values[11], cents(10_000));
        assert_eq!(pivot.total[12], cents(217_000));
    }

    #[test]
    fn payees_restricted_to_shown_year() {
        let doc = sample();
        let options = PivotOptions {
            year: Some(2023),
            ..Default::default()
        };
        let pivot = payee_totals(&doc, &options);
        assert_eq!(pivot.rows.len(), 1);
        assert_eq!(pivot.rows[0].label, "Remboursement");
        assert_eq!(
            remove_dates_and_numbers("Loyer (mars) 03/2024 [x] Durand 12,50"),
            "Loyer Durand"
        );
    }

    #[test]
    fn multi_account_statement_sorted_with_totals() {
        let doc = sample();
        let keep = |_: usize, _: usize, op: &Operation| op.amount.0 != Amount::from_cents(-7_000).0;
        let statement = multi_account_statement(&doc, &[1, 0, 1, 42], &keep);
        let uids: Vec<u64> = statement
            .lines
            .iter()
            .map(|l| doc.accounts[l.account].operations[l.line].uid)
            .collect();
        assert_eq!(uids, [4, 1, 2, 3, 6]);
        // Correction : « Pointé » = lignes pointées (l'original inversait).
        assert_eq!(statement.totals.pointed, cents(-86_000 + 50_000));
        assert_eq!(statement.totals.upcoming, cents(260_000));
        assert_eq!(statement.totals.balance, cents(224_000));
    }

    #[test]
    fn charts() {
        let doc = sample();
        let balances = chart_account_balances(&doc, true);
        assert_eq!(balances.series.len(), 3);
        assert_eq!(balances.series[2].points[1].value, 1670.0);
        assert_eq!(balances.series[0].name, "Opérations pointées");

        let categories = chart_categories(&doc, &AccountScope::All, &all_lines, false);
        let points = &categories.series[0].points;
        assert_eq!(points[0].label, NO_CATEGORY);
        assert_eq!(points[0].value, 600.0);
        assert_eq!(points[1].value, -930.0);
        let all = chart_categories(&doc, &AccountScope::All, &all_lines, true);
        assert_eq!(all.series[0].points[3].label, "Maison > Énergie");

        let months = chart_months(&doc, &AccountScope::All, &all_lines);
        assert_eq!(months.series[0].points[0].value, -860.0);
        assert_eq!(months.series[0].points[11].value, 100.0);

        let modes = chart_modes(&doc, &AccountScope::One(0), &all_lines);
        assert_eq!(modes.series[0].points.len(), 10);
        assert_eq!(modes.series[0].points[0].value, -930.0);

        let levels = chart_amount_levels(&doc, &AccountScope::All, &all_lines);
        let counts: Vec<f64> = levels.series[0].points.iter().map(|p| p.value).collect();
        // 800 → 500-1000 ; 60 et 70 → 50-100 ; 2 500 → 2000-5000 ; 100 → 100-200 ;
        // 500 → 500-1000.
        assert_eq!(counts, [0., 0., 0., 0., 2., 1., 0., 2., 0., 1., 0., 0.]);
        assert_eq!(levels.unit, ChartUnit::Count);
        assert_eq!(levels.series[0].points[11].label, "> 10000");

        let days = chart_day_of_month(&doc, &AccountScope::All, &all_lines);
        assert_eq!(days.series[0].points[0].label, "Le 1er");
        assert_eq!(days.series[0].points[0].value, 3000.0);

        let compact = chart_months(&doc, &AccountScope::All, &all_lines).without_zero_points();
        assert_eq!(compact.series[0].points.len(), 4);
    }

    #[test]
    fn balance_over_time() {
        let doc = sample();
        let monthly =
            chart_balance_over_time(&doc, &AccountScope::All, &all_lines, BalanceStep::Month);
        let points = &monthly.series[0].points;
        let labels: Vec<&str> = points.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels, ["12/2023", "01/2024", "02/2024", "03/2024"]);
        assert_eq!(points[1].value, -760.0);
        assert_eq!(points[3].value, 2170.0);
        let daily =
            chart_balance_over_time(&doc, &AccountScope::One(1), &all_lines, BalanceStep::Day);
        assert_eq!(daily.series[0].points.len(), 1);
        assert_eq!(daily.series[0].points[0].label, "01/03/2024");
    }
}
