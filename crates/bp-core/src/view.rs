//! Tri et solde progressif de la grille des opérations.

use std::cmp::Ordering;

use crate::model::{Account, Amount, Document, Operation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Date,
    Mode,
    Payee,
    Category,
    Amount,
}

/// Ordre d'affichage des opérations d'un compte (indices dans
/// `account.operations`). Tri **stable** : l'ancienne version utilisait un
/// QuickSort instable, l'ordre des égalités variait d'un affichage à l'autre.
pub fn sorted_order(document: &Document, account: &Account, column: SortColumn, ascending: bool) -> Vec<usize> {
    let ops = &account.operations;
    let mut order: Vec<usize> = (0..ops.len()).collect();
    let labels: Vec<String> = match column {
        SortColumn::Category => ops
            .iter()
            .map(|op| op.category.and_then(|id| document.category_label(id)).unwrap_or_default())
            .collect(),
        _ => Vec::new(),
    };
    order.sort_by(|&a, &b| {
        let ordering = match column {
            SortColumn::Category => labels[a].cmp(&labels[b]),
            _ => compare(&ops[a], &ops[b], column),
        };
        if ascending { ordering } else { ordering.reverse() }
    });
    order
}

/// Comparateurs en cascade de l'ancienne version (`unitMain.pas:2221-2284`).
fn compare(a: &Operation, b: &Operation, column: SortColumn) -> Ordering {
    let by_mode = || a.mode_label().cmp(&b.mode_label());
    let by_payee = || payee_without_ventilation(&a.payee).cmp(payee_without_ventilation(&b.payee));
    let by_amount = || a.amount.cmp(&b.amount);
    let by_details = || a.details.cmp(&b.details);
    match column {
        SortColumn::Date => a
            .date
            .cmp(&b.date)
            .then_with(by_mode)
            .then_with(by_payee)
            .then_with(by_amount)
            .then_with(by_details),
        SortColumn::Mode => by_mode().then_with(by_payee).then_with(by_amount).then_with(by_details),
        SortColumn::Payee => by_payee().then_with(by_amount).then_with(by_details),
        SortColumn::Amount => by_amount().then_with(by_details),
        SortColumn::Category => Ordering::Equal,
    }
}

/// Retire le préfixe de ventilation `[…]` (fermé au plus en 7e position).
pub fn payee_without_ventilation(payee: &str) -> &str {
    if payee.starts_with('[')
        && let Some(end) = payee.find(']')
        && end < 7
        && end + 1 < payee.len()
    {
        return &payee[end + 1..];
    }
    payee
}

/// Solde progressif pour chaque position de `order`, calculé sur toutes les
/// opérations (y compris celles masquées par les filtres), dans le sens
/// chronologique de l'affichage : de haut en bas en tri ascendant, de bas en
/// haut en tri descendant.
pub fn running_balances(account: &Account, order: &[usize], ascending: bool) -> Vec<Amount> {
    let mut balances = vec![Amount::ZERO; order.len()];
    let mut total = Amount::ZERO;
    let mut accumulate = |pos: usize| {
        total = total + account.operations[order[pos]].amount;
        balances[pos] = total;
    };
    if ascending {
        (0..order.len()).for_each(&mut accumulate);
    } else {
        (0..order.len()).rev().for_each(&mut accumulate);
    }
    balances
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ventilation_prefix() {
        assert_eq!(payee_without_ventilation("[1/3] Fnac"), " Fnac");
        assert_eq!(payee_without_ventilation("[longprefix] Fnac"), "[longprefix] Fnac");
        assert_eq!(payee_without_ventilation("[1/3]"), "[1/3]");
    }
}
