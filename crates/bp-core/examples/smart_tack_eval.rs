//! Banc d'essai du pointage intelligent sur un fichier réel.
//! Usage : cargo run --release -p bp-core --example smart_tack_eval -- <fichier.bp> [mot de passe]
//!
//! À différentes dates, les opérations des dernières semaines sont dépointées
//! et la banque simulée en a déjà passé une partie (virements et
//! prélèvements avec deux jours de délai, cartes trois jours, chèques une
//! semaine, plus quelques retards au hasard). On vérifie que la recherche
//! retrouve exactement ces lignes. N'affiche que des statistiques.

use std::time::{Duration, Instant};

use bp_core::model::{Account, Date, Mode, StandardMode, Tack};
use bp_core::smart_tack::{Options, SmartTack, pointed_balance, search};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("chemin d'un fichier .bp");
    let password = args.next();
    let document = bp_core::file::open(path.as_ref(), password.as_deref())
        .expect("fichier lisible")
        .document;
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let (mut cases, mut first, mut top3, mut nothing, mut impossible) = (0, 0, 0, 0, 0);
    let mut slowest = Duration::ZERO;
    let mut total_time = Duration::ZERO;
    let mut sizes = Vec::new();
    // Une opération sur N passe avec dix jours de retard (0 : jamais).
    let late_rate: u64 = std::env::var("LATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let mut misses: std::collections::BTreeMap<usize, usize> = Default::default();
    for account in &document.accounts {
        if account.operations.len() < 100 {
            continue;
        }
        let first_date = account.operations.iter().map(|o| o.date).min().unwrap();
        let last_date = account.operations.iter().map(|o| o.date).max().unwrap();
        let mut day = first_date.0 + 120;
        while day <= last_date.0 {
            let as_of = Date(day);
            let window = 30 + (random() % 30) as i32;
            let mut a: Account = account.clone();
            let mut truth = Vec::new();
            for (i, op) in a.operations.iter_mut().enumerate() {
                if op.date.0 > day - window {
                    op.tack = Tack::None;
                    if op.date > as_of {
                        continue;
                    }
                    let delay = match &op.mode {
                        Mode::Standard(StandardMode::Carte) => 3,
                        Mode::Standard(StandardMode::ChequeEmis) => 7,
                        _ => 2,
                    } + if late_rate > 0 && random() % late_rate == 0 {
                        10
                    } else {
                        0
                    };
                    if op.date.0 + delay <= day {
                        truth.push(i);
                    }
                } else {
                    op.tack = Tack::Reconciled;
                }
            }
            day += 17;
            let unpointed = a
                .operations
                .iter()
                .filter(|o| o.tack == Tack::None && o.date <= as_of)
                .count();
            if truth.is_empty() || unpointed < 5 {
                continue;
            }
            let bank = pointed_balance(&a, Some(as_of))
                + truth.iter().map(|&i| a.operations[i].amount).sum();
            let started = Instant::now();
            let result = search(&a, bank, Some(as_of), &Options::default());
            let elapsed = started.elapsed();
            slowest = slowest.max(elapsed);
            total_time += elapsed;
            sizes.push(unpointed);
            cases += 1;
            match result {
                SmartTack::Found(list) => {
                    if list[0].lines == truth {
                        first += 1;
                    } else {
                        let diff = list[0].lines.iter().filter(|i| !truth.contains(i)).count()
                            + truth.iter().filter(|i| !list[0].lines.contains(i)).count();
                        *misses.entry(diff.min(10)).or_default() += 1;
                    }
                    if list.iter().any(|p| p.lines == truth) {
                        top3 += 1;
                    }
                }
                SmartTack::Impossible { .. } => impossible += 1,
                _ => nothing += 1,
            }
        }
    }
    sizes.sort();
    let pct = |n: usize| 100.0 * n as f64 / cases.max(1) as f64;
    println!(
        "{cases} essais, lignes non pointées : médiane {}, max {}",
        sizes.get(sizes.len() / 2).copied().unwrap_or(0),
        sizes.last().copied().unwrap_or(0)
    );
    println!("bonne combinaison en tête      : {:5.1} %", pct(first));
    println!("bonne combinaison parmi les 3  : {:5.1} %", pct(top3));
    println!("rien de vraisemblable          : {:5.1} %", pct(nothing));
    println!("déclaré impossible (anormal)   : {:5.1} %", pct(impossible));
    println!("écarts quand la tête est fausse (lignes en trop ou en moins) : {misses:?}");
    println!(
        "temps moyen {:.1?}, maximum {:.1?}",
        total_time / cases.max(1) as u32,
        slowest
    );
}
