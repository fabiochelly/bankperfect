//! Banc d'essai de la catégorisation automatique sur un fichier réel.
//! Usage : cargo run --release -p bp-core --example categorize_eval -- <fichier.bp> [mot de passe]
//!
//! Rejoue l'historique dans l'ordre chronologique : chaque opération
//! catégorisée est prédite avec le passé seul, puis apprise. N'affiche que
//! des statistiques (aucun libellé).

use std::collections::BTreeMap;

use bp_core::categorize::{AUTO_THRESHOLD, Categorizer, Query};
use bp_core::model::Operation;

#[derive(Default)]
struct Tally {
    proposed: usize,
    right: usize,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("chemin d'un fichier .bp");
    let password = args.next();
    let opened = bp_core::file::open(path.as_ref(), password.as_deref()).expect("fichier lisible");
    let document = opened.document;
    let mut ops: Vec<&Operation> = document
        .accounts
        .iter()
        .flat_map(|a| a.operations.iter())
        .filter(|op| op.category.is_some())
        .collect();
    ops.sort_by_key(|op| (op.date, op.uid));

    let mut categorizer = Categorizer::without_history(&document);
    let mut overall = Tally::default();
    let mut confident = Tally::default();
    let mut by_source: BTreeMap<(&str, &str), Tally> = BTreeMap::new();
    let mut buckets: BTreeMap<u32, Tally> = BTreeMap::new();
    let mut in_top3 = 0;
    let mut wrong_generic: BTreeMap<&str, usize> = BTreeMap::new();
    for op in &ops {
        let truth = op.category.expect("catégorisée");
        let suggestion = categorizer.suggest(&Query::of(op));
        if let Some(best) = suggestion.best() {
            let ok = best.category == truth;
            overall.proposed += 1;
            overall.right += ok as usize;
            let level = if best.confidence >= AUTO_THRESHOLD {
                "sûre"
            } else {
                "à confirmer"
            };
            let s = by_source.entry((best.source.label(), level)).or_default();
            s.proposed += 1;
            s.right += ok as usize;
            let b = buckets.entry((best.confidence * 10.0) as u32).or_default();
            b.proposed += 1;
            b.right += ok as usize;
            if best.confidence >= AUTO_THRESHOLD {
                confident.proposed += 1;
                confident.right += ok as usize;
            }
            if !ok
                && best.confidence >= AUTO_THRESHOLD
                && let Some(g) = best.generic
            {
                *wrong_generic.entry(g).or_insert(0) += 1;
            }
            in_top3 += suggestion.candidates.iter().any(|c| c.category == truth) as usize;
        }
        categorizer.learn(op);
    }
    let started = std::time::Instant::now();
    let full = Categorizer::new(&document);
    println!(
        "index complet construit en {:.1?} ({} opérations)",
        started.elapsed(),
        full.history_len()
    );
    let n = ops.len().max(1) as f64;
    let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
    println!(
        "{} opérations catégorisées, {} catégories",
        ops.len(),
        document.categories.len()
    );
    println!(
        "proposition      : {:5.1} % des opérations, juste {:5.1} %, bonne catégorie parmi les 3 : {:5.1} %",
        pct(overall.proposed, ops.len()),
        pct(overall.right, overall.proposed),
        pct(in_top3, overall.proposed)
    );
    println!(
        "appliquée seule  : {:5.1} % des opérations (confiance ≥ {AUTO_THRESHOLD}), juste {:5.1} %",
        pct(confident.proposed, ops.len()),
        pct(confident.right, confident.proposed)
    );
    println!(
        "bien catégorisé  : {:5.1} % du total",
        100.0 * overall.right as f64 / n
    );
    println!("\npar origine :");
    for ((source, level), t) in &by_source {
        println!(
            "  {source:<20} {level:<12} {:5} proposées, juste {:5.1} %",
            t.proposed,
            pct(t.right, t.proposed)
        );
    }
    if std::env::var_os("DETAIL").is_some() {
        let mut list: Vec<_> = wrong_generic.into_iter().collect();
        list.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        println!("\nerreurs sûres du dictionnaire par catégorie générique :");
        for (g, n) in list.iter().take(15) {
            let mapped = categorizer
                .category_for_generic(g)
                .and_then(|(id, c)| document.category_label(id).map(|l| format!("{l} ({c:.2})")));
            println!("  {g:<28} {n:4}  → {}", mapped.unwrap_or_default());
        }
    }
    println!("\npar confiance :");
    for (bucket, t) in &buckets {
        println!(
            "  {:.1}–{:.1}  {:5} proposées, juste {:5.1} %",
            *bucket as f64 / 10.0,
            (*bucket + 1) as f64 / 10.0,
            t.proposed,
            pct(t.right, t.proposed)
        );
    }
}
