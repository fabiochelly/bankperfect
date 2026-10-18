use super::*;
use crate::model::{Mode, StandardMode};

/// Opération du jour `day` (octobre 2026 à partir du 1er).
fn op(day: i32, payee: &str, mode: StandardMode, cents: i64, tack: Tack) -> Operation {
    let date = Date(Date::from_ymd(2026, 10, 1).unwrap().0 + day - 1);
    Operation {
        uid: day as u64 * 100 + cents.unsigned_abs() % 97,
        date,
        mark_date: date,
        tack,
        mode: Mode::Standard(mode),
        payee: payee.into(),
        details: String::new(),
        category: None,
        amount: Amount::from_cents(cents),
        extra: Vec::new(),
    }
}

fn account(operations: Vec<Operation>) -> Account {
    let mut d = crate::manage::new_document("Test", Amount::ZERO);
    d.accounts[0].operations = operations;
    d.accounts[0].clone()
}

fn day(d: i32) -> Date {
    Date(Date::from_ymd(2026, 10, 1).unwrap().0 + d - 1)
}

/// Compte typique : solde initial pointé, puis des opérations non pointées.
fn sample() -> Vec<Operation> {
    use StandardMode::*;
    vec![
        op(1, "Solde initial", VirementRecu, 200_000, Tack::Reconciled),
        op(2, "PRLV EDF", Prelevement, -8_900, Tack::None),
        op(3, "CB CARREFOUR", Carte, -5_430, Tack::None),
        op(4, "Chèque", ChequeEmis, -12_000, Tack::None),
        op(5, "CB PHARMACIE", Carte, -1_290, Tack::None),
        op(6, "VIR SALAIRE", VirementRecu, 250_000, Tack::None),
        op(8, "PRLV FREE", Prelevement, -2_999, Tack::None),
        op(9, "CB LIDL", Carte, -6_318, Tack::None),
        op(11, "Chèque", ChequeEmis, -4_500, Tack::None),
        op(12, "PRLV ASSURANCE", Prelevement, -4_594, Tack::None),
        op(14, "CB BOULANGERIE", Carte, -420, Tack::None),
    ]
}

fn indices_of(account: &Account, payees: &[&str]) -> Vec<usize> {
    let mut v: Vec<usize> = payees
        .iter()
        .map(|p| {
            account
                .operations
                .iter()
                .position(|o| o.payee == *p)
                .unwrap()
        })
        .collect();
    v.sort();
    v
}

fn bank(account: &Account, cleared: &[&str]) -> Amount {
    let pointed = pointed_balance(account, None);
    let extra: Amount = indices_of(account, cleared)
        .iter()
        .map(|&i| account.operations[i].amount)
        .sum();
    pointed + extra
}

fn best(result: SmartTack) -> Proposal {
    match result {
        SmartTack::Found(list) => list.into_iter().next().unwrap(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn already_balanced() {
    let a = account(sample());
    assert_eq!(
        search(&a, Amount::from_cents(200_000), None, &Options::default()),
        SmartTack::Balanced
    );
}

#[test]
fn everything_up_to_a_date() {
    let a = account(sample());
    let cleared = [
        "PRLV EDF",
        "CB CARREFOUR",
        "CB PHARMACIE",
        "VIR SALAIRE",
        "PRLV FREE",
        "CB LIDL",
    ];
    let p = best(search(
        &a,
        bank(&a, &cleared),
        Some(day(10)),
        &Options::default(),
    ));
    assert_eq!(p.lines, indices_of(&a, &cleared));
    assert_eq!(p.exceptions(), 0);
}

#[test]
fn a_cheque_clears_late_and_another_is_cashed() {
    let a = account(sample());
    // Le premier chèque n'est pas encore encaissé, le second l'est déjà.
    let cleared = [
        "PRLV EDF",
        "CB CARREFOUR",
        "CB PHARMACIE",
        "VIR SALAIRE",
        "PRLV FREE",
        "CB LIDL",
        "Chèque",
    ];
    let mut cleared_ops = indices_of(&a, &cleared[..6]);
    let second_cheque = a
        .operations
        .iter()
        .rposition(|o| o.payee == "Chèque")
        .unwrap();
    cleared_ops.push(second_cheque);
    cleared_ops.sort();
    let target = pointed_balance(&a, None)
        + cleared_ops
            .iter()
            .map(|&i| a.operations[i].amount)
            .sum::<Amount>();
    let p = best(search(&a, target, Some(day(14)), &Options::default()));
    assert_eq!(p.lines, cleared_ops);
    // Chèques jusqu'au second, sauf le premier, pas encore encaissé.
    let first_cheque = a
        .operations
        .iter()
        .position(|o| o.payee == "Chèque")
        .unwrap();
    assert_eq!(p.cheques_until, Some(day(11)));
    assert_eq!(p.held_back, vec![first_cheque]);
}

#[test]
fn deferred_card_payments() {
    let a = account(sample());
    // Les cartes passent en fin de mois : seuls les prélèvements et virements.
    let cleared = ["PRLV EDF", "VIR SALAIRE", "PRLV FREE", "PRLV ASSURANCE"];
    let p = best(search(
        &a,
        bank(&a, &cleared),
        Some(day(14)),
        &Options::default(),
    ));
    assert_eq!(p.lines, indices_of(&a, &cleared));
    assert_eq!(p.cards_until, None);
    assert_eq!(p.until, Some(day(12)));
}

#[test]
fn an_old_transfer_not_yet_at_the_bank() {
    let a = account(sample());
    let cleared = [
        "CB CARREFOUR",
        "CB PHARMACIE",
        "VIR SALAIRE",
        "PRLV FREE",
        "CB LIDL",
    ];
    let p = best(search(
        &a,
        bank(&a, &cleared),
        Some(day(10)),
        &Options::default(),
    ));
    assert_eq!(p.lines, indices_of(&a, &cleared));
    assert_eq!(p.held_back, indices_of(&a, &["PRLV EDF"]));
}

#[test]
fn later_operations_are_ignored() {
    let a = account(sample());
    let cleared = ["PRLV EDF", "CB CARREFOUR"];
    let p = best(search(
        &a,
        bank(&a, &cleared),
        Some(day(3)),
        &Options::default(),
    ));
    assert!(p.lines.iter().all(|&i| a.operations[i].date <= day(3)));
    assert_eq!(p.lines, indices_of(&a, &cleared));
}

#[test]
fn impossible_balance() {
    let a = account(sample());
    // 0,01 € de trop : aucune combinaison.
    let target = bank(&a, &["PRLV EDF"]) + Amount::from_cents(1);
    assert_eq!(
        search(&a, target, None, &Options::default()),
        SmartTack::Impossible {
            difference: Amount::from_cents(-8_899)
        }
    );
}

#[test]
fn same_amounts_give_alternatives_oldest_first() {
    use StandardMode::*;
    let a = account(vec![
        op(1, "Solde initial", VirementRecu, 100_000, Tack::Reconciled),
        op(2, "Chèque A", ChequeEmis, -5_000, Tack::None),
        op(3, "Chèque B", ChequeEmis, -5_000, Tack::None),
    ]);
    let SmartTack::Found(list) = search(&a, Amount::from_cents(95_000), None, &Options::default())
    else {
        panic!()
    };
    assert_eq!(list.len(), 2);
    // Le chèque le plus ancien est plus probablement encaissé.
    assert_eq!(list[0].lines, vec![1]);
    assert_eq!(list[1].lines, vec![2]);
}

#[test]
fn exact_search_on_many_lines() {
    let values: Vec<i64> = (1..=200).map(|i| -(i * 37 % 1000 + 1)).collect();
    let target: i64 = values.iter().step_by(3).sum();
    let deadline = Instant::now() + Duration::from_secs(5);
    assert_eq!(reachable(&values, target, deadline), Some(true));
    let total: i64 = values.iter().sum();
    assert_eq!(reachable(&values, total - 1, deadline), Some(false));
    assert_eq!(reachable(&[-5, 7, 3], 2, deadline), Some(true));
    assert_eq!(reachable(&[-5, 7, 3], 4, deadline), Some(false));
}

#[test]
fn stays_fast_with_hundreds_of_lines() {
    use StandardMode::*;
    let mut ops = vec![op(
        1,
        "Solde initial",
        VirementRecu,
        1_000_000,
        Tack::Reconciled,
    )];
    for i in 0..300 {
        let mode = [Carte, Prelevement, ChequeEmis, Carte, VirementEmis][i % 5];
        ops.push(op(
            2 + (i as i32) / 6,
            "Achat",
            mode,
            -(1_000 + (i as i64 * 7919) % 25_000),
            Tack::None,
        ));
    }
    let a = account(ops);
    let cleared: Amount = a.operations[1..250]
        .iter()
        .filter(|o| o.mode != Mode::Standard(ChequeEmis))
        .map(|o| o.amount)
        .sum();
    let target = pointed_balance(&a, None) + cleared;
    // Sans optimisations (tests), le calcul est bien plus lent.
    let limit = Duration::from_secs(if cfg!(debug_assertions) { 30 } else { 1 });
    let options = Options {
        time_budget: limit,
        ..Options::default()
    };
    let started = Instant::now();
    let result = search(&a, target, None, &options);
    assert!(started.elapsed() < limit, "{:?}", started.elapsed());
    assert!(matches!(result, SmartTack::Found(_)), "{result:?}");
}
