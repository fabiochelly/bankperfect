//! Tableau de bord et analyse du mois : patrimoine, recettes et dépenses,
//! répartition par catégorie, évolution, échéances à venir.
//!
//! Comme le « Total des comptes » du bureau, seuls les comptes non virtuels
//! comptent. Les transferts entre comptes (lignes liées à une ligne sœur) ne
//! sont ni des recettes ni des dépenses.

use std::collections::HashMap;

use bp_core::edit::mode_is_debit;
use bp_core::model::{Amount, Date, Document, Operation, Tack};
use bp_core::reports::AccountScope;
use bp_core::scheduler;
use bp_core::split::SISTER_ACCOUNT_KEY;
use chrono::Datelike;

use crate::view::MONTHS;
use crate::{BpError, Session, money};

/// Part d'une catégorie dans les dépenses (ou les recettes) d'un mois.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CategoryShare {
    pub label: String,
    pub amount: String,
    /// Valeur absolue en unités monétaires (pour les graphiques).
    pub value: f64,
    /// Part du total, entre 0 et 1.
    pub share: f64,
    /// Sous-catégories, de la plus grosse à la plus petite.
    pub children: Vec<CategoryShare>,
}

/// Point d'une courbe.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

/// Recettes et dépenses d'un mois (barres).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MonthBar {
    /// `nov.`
    pub label: String,
    pub income: f64,
    pub expenses: f64,
}

/// Échéance ou opération à venir.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct UpcomingItem {
    /// `21/11`.
    pub date: String,
    /// Jours restants (0 : aujourd'hui).
    pub days: i32,
    pub payee: String,
    pub account: String,
    pub amount: String,
    pub negative: bool,
    /// Échéance de l'échéancier (sinon opération déjà saisie, datée plus tard).
    pub scheduled: bool,
}

/// Opération récente, tous comptes confondus.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RecentItem {
    pub uid: u64,
    /// Index réel du compte.
    pub account_index: u32,
    pub account: String,
    pub date: String,
    pub payee: String,
    pub category: String,
    pub amount: String,
    pub negative: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Dashboard {
    /// Solde de tous les comptes (non virtuels).
    pub total: String,
    pub total_negative: bool,
    /// Solde des opérations pointées ou rapprochées.
    pub pointed: String,
    /// Solde prévu à la fin du mois (opérations à venir et échéances).
    pub forecast: String,
    pub forecast_negative: bool,
    /// Variation du patrimoine sur 30 jours, formatée avec son signe.
    pub change_30_days: String,
    pub change_negative: bool,
    /// `novembre`.
    pub month: String,
    pub income: String,
    pub expenses: String,
    pub income_value: f64,
    pub expenses_value: f64,
    /// Recettes − dépenses du mois.
    pub net: String,
    pub net_negative: bool,
    /// Dépenses comparées à la même période du mois précédent
    /// (`-12 % par rapport à octobre`), vide sans point de comparaison.
    pub expenses_trend: String,
    pub expenses_up: bool,
    /// Solde total en fin de journée, sur 90 jours.
    pub history: Vec<ChartPoint>,
    /// Plus grosses catégories de dépenses du mois (5 au plus, puis
    /// « Autres »).
    pub categories: Vec<CategoryShare>,
    pub upcoming: Vec<UpcomingItem>,
    pub recent: Vec<RecentItem>,
    /// Opérations non pointées, tous comptes.
    pub pending_count: u32,
    pub pending: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MonthReport {
    /// `novembre 2026`.
    pub label: String,
    pub income: String,
    pub expenses: String,
    pub net: String,
    pub net_negative: bool,
    /// Part des recettes épargnée (net / recettes), entre 0 et 1 ; négative
    /// si les dépenses dépassent les recettes.
    pub savings_rate: f64,
    pub expense_categories: Vec<CategoryShare>,
    pub income_categories: Vec<CategoryShare>,
    /// Six mois, jusqu'au mois du rapport.
    pub months: Vec<MonthBar>,
    /// Dépenses cumulées jour par jour, ce mois-ci et le mois précédent.
    pub cumulative: Vec<ChartPoint>,
    pub previous_cumulative: Vec<ChartPoint>,
}

const SHORT_MONTHS: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
    "déc.",
];

fn first_of_month(date: Date, offset: i32) -> Date {
    let d = date.to_naive();
    let months = d.year() * 12 + d.month0() as i32 + offset;
    Date::from_ymd(months.div_euclid(12), months.rem_euclid(12) as u32 + 1, 1).unwrap_or(date)
}

fn month_name(first: Date) -> &'static str {
    MONTHS[first.to_naive().month0() as usize]
}

/// Transfert entre deux comptes du fichier.
fn is_transfer(op: &Operation) -> bool {
    op.extra(SISTER_ACCOUNT_KEY).is_some()
}

fn value(amount: Amount) -> f64 {
    amount.as_f64()
}

/// Monnaie des totaux : celle du compte par défaut.
fn currency(document: &Document) -> String {
    document
        .accounts
        .get(document.default_account)
        .or(document.accounts.first())
        .map(|a| a.currency.clone())
        .unwrap_or_default()
}

/// Opérations des comptes non virtuels, avec l'index de leur compte.
fn counted(document: &Document) -> impl Iterator<Item = (usize, &Operation)> {
    AccountScope::All
        .accounts(document)
        .into_iter()
        .flat_map(move |a| {
            document.accounts[a]
                .operations
                .iter()
                .map(move |op| (a, op))
        })
}

/// Parent et enfant d'une catégorie (`Alimentation`, `Boulangerie`).
fn category_parts(document: &Document, category: Option<u32>) -> (String, Option<String>) {
    match category.and_then(|id| document.category_label(id)) {
        Some(label) => match label.split_once(" > ") {
            Some((parent, child)) => (parent.to_string(), Some(child.to_string())),
            None => (label, None),
        },
        None => ("Sans catégorie".to_string(), None),
    }
}

/// Répartition par catégorie (parents, puis leurs enfants) des montants.
fn shares(
    document: &Document,
    ops: &[&Operation],
    currency: &str,
    limit: Option<usize>,
) -> Vec<CategoryShare> {
    let mut parents: HashMap<String, (Amount, HashMap<String, Amount>)> = HashMap::new();
    for op in ops {
        let (parent, child) = category_parts(document, op.category);
        let entry = parents.entry(parent).or_default();
        entry.0 = entry.0 + op.amount.abs();
        let child = child.unwrap_or_else(|| "Sans sous-catégorie".into());
        let c = entry.1.entry(child).or_default();
        *c = *c + op.amount.abs();
    }
    let total: f64 = parents.values().map(|(a, _)| value(*a)).sum();
    let share = |amount: Amount| {
        if total > 0.0 {
            value(amount) / total
        } else {
            0.0
        }
    };
    let mut list: Vec<CategoryShare> = parents
        .into_iter()
        .map(|(label, (amount, children))| {
            let mut children: Vec<CategoryShare> = children
                .into_iter()
                .map(|(label, a)| CategoryShare {
                    label,
                    amount: money(a, currency),
                    value: value(a),
                    share: if amount.0 > 0 {
                        a.0 as f64 / amount.0 as f64
                    } else {
                        0.0
                    },
                    children: Vec::new(),
                })
                .collect();
            children.sort_by(|a, b| b.value.total_cmp(&a.value));
            CategoryShare {
                label,
                amount: money(amount, currency),
                value: value(amount),
                share: share(amount),
                children,
            }
        })
        .collect();
    list.sort_by(|a, b| b.value.total_cmp(&a.value));
    if let Some(limit) = limit
        && list.len() > limit + 1
    {
        let rest: Vec<CategoryShare> = list.split_off(limit);
        let amount: f64 = rest.iter().map(|c| c.value).sum();
        list.push(CategoryShare {
            label: "Autres".into(),
            amount: money(
                Amount((amount * Amount::SCALE as f64).round() as i64),
                currency,
            ),
            value: amount,
            share: if total > 0.0 { amount / total } else { 0.0 },
            children: rest,
        });
    }
    list
}

/// Recettes et dépenses (hors transferts) entre deux dates incluses.
fn flows(document: &Document, from: Date, to: Date) -> (Vec<&Operation>, Vec<&Operation>) {
    let mut income = Vec::new();
    let mut expenses = Vec::new();
    for (_, op) in counted(document) {
        if op.date < from || op.date > to || is_transfer(op) || op.amount.0 == 0 {
            continue;
        }
        if op.amount.is_negative() {
            expenses.push(op);
        } else {
            income.push(op);
        }
    }
    (income, expenses)
}

fn sum(ops: &[&Operation]) -> Amount {
    Amount(ops.iter().map(|op| op.amount.0.abs()).sum())
}

/// Dépenses cumulées jour par jour d'un mois, jusqu'à `until`.
fn cumulative(document: &Document, first: Date, until: Date) -> Vec<ChartPoint> {
    let end = first_of_month(first, 1).0 - 1;
    let last = end.min(until.0);
    let (_, expenses) = flows(document, first, Date(last));
    let mut by_day: HashMap<i32, Amount> = HashMap::new();
    for op in expenses {
        let e = by_day.entry(op.date.0).or_default();
        *e = *e + op.amount.abs();
    }
    let mut total = Amount::ZERO;
    (first.0..=last)
        .map(|day| {
            total = total + by_day.get(&day).copied().unwrap_or(Amount::ZERO);
            ChartPoint {
                label: (day - first.0 + 1).to_string(),
                value: value(total),
            }
        })
        .collect()
}

#[uniffi::export]
impl Session {
    /// Compte proposé pour une saisie rapide depuis le tableau de bord.
    pub fn default_account(&self) -> u32 {
        let inner = self.lock();
        let document = &inner.document;
        let order = AccountScope::All.accounts(document);
        if document.default_account < document.accounts.len() {
            document.default_account as u32
        } else {
            order.first().copied().unwrap_or(0) as u32
        }
    }

    pub fn dashboard(&self) -> Dashboard {
        let inner = self.lock();
        let document = &inner.document;
        let currency = currency(document);
        let today = Date::today();
        let first = first_of_month(today, 0);
        let end_of_month = Date(first_of_month(today, 1).0 - 1);

        let mut total = Amount::ZERO;
        let mut pointed = Amount::ZERO;
        let mut pending = Amount::ZERO;
        let mut pending_count = 0;
        for (_, op) in counted(document) {
            total = total + op.amount;
            if op.tack == Tack::None {
                pending = pending + op.amount;
                pending_count += 1;
            } else {
                pointed = pointed + op.amount;
            }
        }

        // Fin de mois : somme des prévisions de chaque compte.
        let schedules = scheduler::load(document)
            .map(|l| l.schedules)
            .unwrap_or_default();
        let forecast = AccountScope::All
            .accounts(document)
            .into_iter()
            .filter_map(|a| {
                scheduler::forecast(document, &schedules, a, end_of_month, today, false)
                    .last()
                    .map(|l| l.balance)
            })
            .fold(Amount::ZERO, |a, b| a + b);

        // Évolution : solde total en fin de journée sur 90 jours.
        let start = Date(today.0 - 89);
        let mut before = Amount::ZERO;
        let mut deltas: HashMap<i32, Amount> = HashMap::new();
        for (_, op) in counted(document) {
            if op.date < start {
                before = before + op.amount;
            } else if op.date <= today {
                let d = deltas.entry(op.date.0).or_default();
                *d = *d + op.amount;
            }
        }
        let mut running = before;
        let history: Vec<ChartPoint> = (start.0..=today.0)
            .map(|day| {
                running = running + deltas.get(&day).copied().unwrap_or(Amount::ZERO);
                ChartPoint {
                    label: Date(day).to_string()[..5].to_string(),
                    value: value(running),
                }
            })
            .collect();
        let month_ago = history
            .get(history.len().saturating_sub(31))
            .map(|p| p.value)
            .unwrap_or(0.0);
        let now = history.last().map(|p| p.value).unwrap_or(0.0);
        let change = now - month_ago;
        let change_amount = Amount((change * Amount::SCALE as f64).round() as i64);

        let (income, expenses) = flows(document, first, today);
        let income_total = sum(&income);
        let expense_total = sum(&expenses);
        let net = Amount(income_total.0 - expense_total.0);

        // Même période du mois précédent (du 1er au même jour).
        let previous_first = first_of_month(today, -1);
        let same_day = Date(previous_first.0 + (today.0 - first.0)).min(Date(first.0 - 1));
        let (_, previous_expenses) = flows(document, previous_first, same_day);
        let previous_total = sum(&previous_expenses);
        let (expenses_trend, expenses_up) = if previous_total.0 > 0 {
            let ratio = (expense_total.0 - previous_total.0) as f64 / previous_total.0 as f64;
            let percent = (ratio * 100.0).round() as i64;
            (
                format!(
                    "{}{} % par rapport à {}",
                    if percent > 0 { "+" } else { "" },
                    percent,
                    month_name(previous_first)
                ),
                percent > 0,
            )
        } else {
            (String::new(), false)
        };

        // À venir sur 30 jours : opérations datées plus tard et échéances.
        let horizon = Date(today.0 + 30);
        let names: HashMap<&str, &str> = document
            .accounts
            .iter()
            .map(|a| (a.uid.as_str(), a.name.as_str()))
            .collect();
        let mut upcoming: Vec<(Date, UpcomingItem)> = Vec::new();
        for (a, op) in counted(document) {
            if op.date > today && op.date <= horizon {
                upcoming.push((
                    op.date,
                    UpcomingItem {
                        date: op.date.to_string()[..5].to_string(),
                        days: op.date.0 - today.0,
                        payee: op.payee.clone(),
                        account: document.accounts[a].name.clone(),
                        amount: money(op.amount, &document.accounts[a].currency),
                        negative: op.amount.is_negative(),
                        scheduled: false,
                    },
                ));
            }
        }
        for schedule in &schedules {
            for occurrence in schedule.occurrences(horizon) {
                if occurrence.date < today {
                    continue;
                }
                let amount = schedule.amount_on(occurrence.due).abs();
                let amount = if mode_is_debit(document, &schedule.mode) {
                    -amount
                } else {
                    amount
                };
                upcoming.push((
                    occurrence.date,
                    UpcomingItem {
                        date: occurrence.date.to_string()[..5].to_string(),
                        days: occurrence.date.0 - today.0,
                        payee: schedule.payee.clone(),
                        account: names
                            .get(schedule.account.as_str())
                            .unwrap_or(&"")
                            .to_string(),
                        amount: money(amount, &currency),
                        negative: amount.is_negative(),
                        scheduled: true,
                    },
                ));
            }
        }
        upcoming.sort_by_key(|(d, _)| *d);
        let upcoming = upcoming.into_iter().take(6).map(|(_, u)| u).collect();

        // Dernières opérations passées.
        let mut recent: Vec<(usize, &Operation)> = counted(document)
            .filter(|(_, op)| op.date <= today)
            .collect();
        recent.sort_by_key(|(_, op)| std::cmp::Reverse((op.date, op.uid)));
        let recent = recent
            .into_iter()
            .take(6)
            .map(|(a, op)| RecentItem {
                uid: op.uid,
                account_index: a as u32,
                account: document.accounts[a].name.clone(),
                date: op.date.to_string()[..5].to_string(),
                payee: op.payee.clone(),
                category: op
                    .category
                    .and_then(|id| document.category_label(id))
                    .unwrap_or_default(),
                amount: money(op.amount, &document.accounts[a].currency),
                negative: op.amount.is_negative(),
            })
            .collect();

        Dashboard {
            total: money(total, &currency),
            total_negative: total.is_negative(),
            pointed: money(pointed, &currency),
            forecast: money(forecast, &currency),
            forecast_negative: forecast.is_negative(),
            change_30_days: format!(
                "{}{}",
                if change_amount.0 > 0 { "+" } else { "" },
                money(change_amount, &currency)
            ),
            change_negative: change_amount.is_negative(),
            month: month_name(first).to_string(),
            income: money(income_total, &currency),
            expenses: money(expense_total, &currency),
            income_value: value(income_total),
            expenses_value: value(expense_total),
            net: money(net, &currency),
            net_negative: net.is_negative(),
            expenses_trend,
            expenses_up,
            history,
            categories: shares(document, &expenses, &currency, Some(5)),
            upcoming,
            recent,
            pending_count,
            pending: money(pending, &currency),
        }
    }

    /// Analyse d'un mois : `offset` 0 pour le mois en cours, -1 pour le
    /// précédent…
    pub fn month_report(&self, offset: i32) -> Result<MonthReport, BpError> {
        let inner = self.lock();
        let document = &inner.document;
        let currency = currency(document);
        let today = Date::today();
        let first = first_of_month(today, offset.min(0));
        // Mois en cours : jusqu'à aujourd'hui, comme le tableau de bord (les
        // opérations déjà saisies plus loin dans le mois sont des prévisions).
        let last = Date(first_of_month(first, 1).0 - 1).min(today);
        let (income, expenses) = flows(document, first, last);
        let income_total = sum(&income);
        let expense_total = sum(&expenses);
        let net = Amount(income_total.0 - expense_total.0);
        let months = (-5..=0)
            .map(|i| {
                let start = first_of_month(first, i);
                let end = Date(first_of_month(start, 1).0 - 1).min(today);
                let (inc, exp) = flows(document, start, end);
                MonthBar {
                    label: SHORT_MONTHS[start.to_naive().month0() as usize].to_string(),
                    income: value(sum(&inc)),
                    expenses: value(sum(&exp)),
                }
            })
            .collect();
        let previous = first_of_month(first, -1);
        Ok(MonthReport {
            label: format!("{} {}", month_name(first), first.to_naive().year()),
            income: money(income_total, &currency),
            expenses: money(expense_total, &currency),
            net: money(net, &currency),
            net_negative: net.is_negative(),
            savings_rate: if income_total.0 > 0 {
                net.0 as f64 / income_total.0 as f64
            } else {
                0.0
            },
            expense_categories: shares(document, &expenses, &currency, None),
            income_categories: shares(document, &income, &currency, None),
            months,
            cumulative: cumulative(document, first, today),
            previous_cumulative: cumulative(document, previous, today),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::{file, sample};
    use crate::*;

    #[test]
    fn dashboard_counts_the_month() {
        let (document, protection) = sample();
        let session = Session::open(file(&document, &protection), None).unwrap();
        for (payee, amount, credit) in [("Boulangerie", "4,20", false), ("Salaire", "2000", true)] {
            let mut draft = session.new_draft();
            draft.payee = payee.into();
            draft.amount = amount.into();
            draft.credit = credit;
            session.save_operation(0, None, draft).unwrap();
        }
        let dash = session.dashboard();
        assert_eq!(dash.total, "2\u{202F}095,80\u{A0}€");
        assert_eq!(dash.history.len(), 90);
        assert_eq!(dash.recent.len(), 3);
        // Le solde initial du compte compte aussi comme recette du mois.
        assert_eq!(dash.expenses, "4,20\u{A0}€");
        assert_eq!(dash.categories.len(), 1);
        assert!((dash.categories[0].share - 1.0).abs() < 1e-9);

        let report = session.month_report(0).unwrap();
        assert_eq!(report.months.len(), 6);
        assert_eq!(report.expenses, "4,20\u{A0}€");
        assert!(report.savings_rate > 0.99);
    }

    #[test]
    fn month_offsets() {
        use bp_core::model::Date;
        let d = Date::from_ymd(2026, 1, 15).unwrap();
        assert_eq!(
            super::first_of_month(d, -1),
            Date::from_ymd(2025, 12, 1).unwrap()
        );
        assert_eq!(
            super::first_of_month(d, 11),
            Date::from_ymd(2026, 12, 1).unwrap()
        );
        assert_eq!(
            super::first_of_month(d, 12),
            Date::from_ymd(2027, 1, 1).unwrap()
        );
    }
}
