//! Pointage intelligent : quelles opérations non pointées la banque a-t-elle
//! déjà passées, sachant son solde ?
//!
//! Trouver des lignes dont la somme vaut l'écart entre le solde en banque et
//! le solde pointé est une « somme de sous-ensemble » : exponentielle en
//! force brute, et surtout **ambiguë** (dès quelques dizaines de lignes, la
//! plupart des montants s'obtiennent de plusieurs façons). On cherche donc la
//! combinaison la plus *vraisemblable* plutôt que n'importe laquelle :
//!
//! 1. rien après la date du solde ;
//! 2. la banque passe les opérations à peu près dans l'ordre des dates : on
//!    essaie chaque date limite, une pour les cartes (débit différé, délais
//!    de traitement), une pour les chèques, une pour les autres modes ;
//! 3. les chèques émis ont aussi leur date limite (encaissement plus lent) ;
//! 4. au plus trois exceptions (opération ancienne pas encore passée,
//!    opération récente déjà passée), trouvées par tables de hachage ;
//! 5. chaque hypothèse a un coût (retard des dates limites, exceptions) ;
//!    les meilleures sont proposées, jamais appliquées d'office.
//!
//! Si rien de vraisemblable ne tombe juste, une recherche exacte
//! (programmation dynamique sur les montants en centimes) dit s'il existe au
//! moins une combinaison : sinon une opération manque ou un montant diffère.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::categorize::text::Nature;
use crate::model::{Account, Amount, Date, Operation, Tack};

/// Réglages de la recherche.
#[derive(Debug, Clone)]
pub struct Options {
    /// Exceptions au plus par hypothèse.
    pub max_exceptions: usize,
    /// Temps de calcul maximal (filet de sécurité).
    pub time_budget: Duration,
    /// Propositions renvoyées au plus.
    pub max_proposals: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_exceptions: 3,
            time_budget: Duration::from_millis(400),
            max_proposals: 3,
        }
    }
}

/// Combinaison proposée.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    /// Opérations à pointer (indices dans `account.operations`), par date.
    pub lines: Vec<usize>,
    /// Somme des opérations à pointer.
    pub total: Amount,
    /// Coût de l'hypothèse : plus il est bas, plus elle est vraisemblable.
    pub score: f32,
    /// Date limite des opérations hors cartes et chèques (`None` : aucune).
    pub until: Option<Date>,
    /// Date limite des paiements par carte.
    pub cards_until: Option<Date>,
    /// Date limite des chèques émis.
    pub cheques_until: Option<Date>,
    /// Opérations antérieures aux dates limites pas encore passées.
    pub held_back: Vec<usize>,
    /// Chèques encaissés et opérations passées après les dates limites.
    pub early: Vec<usize>,
}

impl Proposal {
    /// Nombre d'exceptions (les chèques encaissés comptent).
    pub fn exceptions(&self) -> usize {
        self.held_back.len() + self.early.len()
    }
}

/// Résultat de la recherche.
#[derive(Debug, Clone, PartialEq)]
pub enum SmartTack {
    /// Le solde pointé est déjà égal au solde en banque.
    Balanced,
    /// Propositions, la plus vraisemblable d'abord.
    Found(Vec<Proposal>),
    /// Aucune combinaison de lignes non pointées ne donne ce solde.
    Impossible { difference: Amount },
    /// Aucune combinaison vraisemblable ; il en existe peut-être une
    /// quelconque (`exists`), qu'il serait hasardeux de proposer.
    NoPlausible {
        difference: Amount,
        exists: Option<bool>,
    },
}

/// Ce qui est déjà en banque d'après les pointages : opérations pointées ou
/// rapprochées, jusqu'à la date du solde.
pub fn pointed_balance(account: &Account, as_of: Option<Date>) -> Amount {
    account
        .operations
        .iter()
        .filter(|op| op.tack != Tack::None && as_of.is_none_or(|d| op.date <= d))
        .map(|op| op.amount)
        .sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Other = 0,
    Card = 1,
    Cheque = 2,
}

impl Class {
    const ALL: [Class; 3] = [Class::Other, Class::Card, Class::Cheque];

    /// Coût d'une date limite en retard sur la date du solde, par jour.
    fn lag_per_day(self) -> f32 {
        match self {
            Class::Other => OTHER_LAG_PER_DAY,
            Class::Card => CARD_LAG_PER_DAY,
            Class::Cheque => CHEQUE_LAG_PER_DAY,
        }
    }
}

fn class_of(op: &Operation) -> Class {
    match Nature::from_label(&op.payee, Some(&op.mode)) {
        Some(Nature::Card) => Class::Card,
        Some(Nature::Cheque) if op.amount.is_negative() => Class::Cheque,
        _ => Class::Other,
    }
}

// Coûts (en « exceptions ») : réglés pour que trois écarts plausibles
// restent préférés à une date limite très en retard.
const OTHER_LAG_PER_DAY: f32 = 0.04;
const CARD_LAG_PER_DAY: f32 = 0.015;
const CHEQUE_LAG_PER_DAY: f32 = 0.01;
const CHEQUE_HELD_COST: f32 = 0.5;
const CHEQUE_EARLY_COST: f32 = 0.6;
const HELD_BACK_COST: f32 = 1.0;
const HELD_BACK_PER_DAY: f32 = 0.03;
const EARLY_COST: f32 = 1.1;
const EARLY_PER_DAY: f32 = 0.05;
const MIN_ITEM_COST: f32 = CHEQUE_HELD_COST;

/// Opération candidate.
#[derive(Debug, Clone, Copy)]
struct Line {
    index: usize,
    date: Date,
    value: i64,
    class: Class,
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Meilleures propositions, sans doublon d'ensemble de lignes.
struct Best {
    max: usize,
    list: Vec<(f32, Vec<usize>, Proposal)>,
}

impl Best {
    fn worst(&self) -> f32 {
        if self.list.len() < self.max {
            f32::INFINITY
        } else {
            self.list.last().map_or(f32::INFINITY, |e| e.0)
        }
    }

    fn offer(&mut self, proposal: Proposal) {
        let key = proposal.lines.clone();
        if let Some(pos) = self.list.iter().position(|e| e.1 == key) {
            if self.list[pos].0 <= proposal.score {
                return;
            }
            self.list.remove(pos);
        }
        if proposal.score >= self.worst() {
            return;
        }
        let at = self
            .list
            .iter()
            .position(|e| e.0 > proposal.score)
            .unwrap_or(self.list.len());
        self.list.insert(at, (proposal.score, key, proposal));
        self.list.truncate(self.max);
    }
}

/// Cherche les opérations non pointées qui expliquent le solde en banque
/// `bank_balance` (au `as_of`, sinon toutes les dates).
pub fn search(
    account: &Account,
    bank_balance: Amount,
    as_of: Option<Date>,
    options: &Options,
) -> SmartTack {
    let started = Instant::now();
    let deadline = started + options.time_budget;
    let difference = Amount(bank_balance.0 - pointed_balance(account, as_of).0);
    if difference == Amount::ZERO {
        return SmartTack::Balanced;
    }
    let mut lines: Vec<Line> = account
        .operations
        .iter()
        .enumerate()
        .filter(|(_, op)| {
            op.tack == Tack::None && op.amount != Amount::ZERO && as_of.is_none_or(|d| op.date <= d)
        })
        .map(|(index, op)| Line {
            index,
            date: op.date,
            value: op.amount.0,
            class: class_of(op),
        })
        .collect();
    if lines.is_empty() {
        return SmartTack::Impossible { difference };
    }
    // Montants en centimes (ou plus gros commun diviseur) : sommes exactes.
    let unit = lines
        .iter()
        .fold(difference.0, |g, l| gcd(g, l.value))
        .max(1);
    for l in &mut lines {
        l.value /= unit;
    }
    let target = difference.0 / unit;
    lines.sort_by_key(|l| (l.date, l.index));
    let reference = as_of.unwrap_or_else(|| lines.last().expect("non vide").date);
    let days = |a: Date, b: Date| (b.0 - a.0).max(0) as f32;

    // Dates limites possibles par famille : « aucune » puis chaque date.
    let cutoffs: Vec<Vec<Option<Date>>> = Class::ALL
        .iter()
        .map(|&class| {
            let mut dates: Vec<Date> = lines
                .iter()
                .filter(|l| l.class == class)
                .map(|l| l.date)
                .collect();
            dates.dedup();
            std::iter::once(None)
                .chain(dates.into_iter().map(Some))
                .collect()
        })
        .collect();
    let first_date = lines[0].date;
    let lag = |cut: Option<Date>, class: Class| -> f32 {
        if !lines.iter().any(|l| l.class == class) {
            return 0.0;
        }
        let at = cut.unwrap_or(Date(first_date.0 - 1));
        days(at, reference) * class.lag_per_day()
    };
    // Somme des lignes de chaque famille jusqu'à chaque date limite.
    let prefix: Vec<Vec<i64>> = Class::ALL
        .iter()
        .zip(&cutoffs)
        .map(|(&class, cuts)| {
            cuts.iter()
                .map(|cut| {
                    lines
                        .iter()
                        .filter(|l| l.class == class && cut.is_some_and(|d| l.date <= d))
                        .map(|l| l.value)
                        .sum()
                })
                .collect()
        })
        .collect();
    let mut bases: Vec<(f32, [Option<Date>; 3], i64)> = Vec::new();
    for (o, &other) in cutoffs[0].iter().enumerate() {
        let lag_other = lag(other, Class::Other);
        for (c, &card) in cutoffs[1].iter().enumerate() {
            let lag_card = lag(card, Class::Card);
            for (q, &cheque) in cutoffs[2].iter().enumerate() {
                let cost = lag_other + lag_card + lag(cheque, Class::Cheque);
                let sum = prefix[0][o] + prefix[1][c] + prefix[2][q];
                bases.push((cost, [other, card, cheque], sum));
            }
        }
    }
    bases.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Lignes par montant, pour trouver les exceptions sans tout parcourir.
    let mut by_value: HashMap<i64, Vec<usize>> = HashMap::new();
    for (k, l) in lines.iter().enumerate() {
        by_value.entry(l.value).or_default().push(k);
    }

    let mut best = Best {
        max: options.max_proposals.max(1),
        list: Vec::new(),
    };
    // Approfondissement progressif : toutes les hypothèses avec au plus une
    // exception (instantané), puis deux, puis trois ; chaque passe élague
    // d'après les meilleures propositions déjà trouvées.
    let passes: &[(usize, usize)] = &[(0, 1), (2, 2), (3, 3)];
    let mut found = Vec::new();
    'passes: for &(min_k, max_k) in passes {
        if min_k > options.max_exceptions {
            break;
        }
        let max_k = max_k.min(options.max_exceptions);
        for &(base_cost, cuts, sum) in &bases {
            if base_cost + min_k as f32 * MIN_ITEM_COST >= best.worst() {
                break;
            }
            if Instant::now() > deadline {
                break 'passes;
            }
            let cut_of = |l: &Line| cuts[l.class as usize];
            let in_base = |l: &Line| cut_of(l).is_some_and(|d| l.date <= d);
            // Effet sur la somme et coût d'une exception sur la ligne `k`.
            let flip = |k: usize| -> (i64, f32) {
                let l = &lines[k];
                if in_base(l) {
                    let cost = match l.class {
                        Class::Cheque => CHEQUE_HELD_COST,
                        _ => {
                            (HELD_BACK_COST + days(l.date, reference) * HELD_BACK_PER_DAY).min(3.0)
                        }
                    };
                    (-l.value, cost)
                } else {
                    let after =
                        cut_of(l).map_or(days(first_date, l.date) + 1.0, |d| days(d, l.date));
                    let cost = match l.class {
                        Class::Cheque => CHEQUE_EARLY_COST + after * CHEQUE_LAG_PER_DAY,
                        _ => EARLY_COST + after * EARLY_PER_DAY,
                    };
                    (l.value, cost)
                }
            };
            // Lignes dont l'exception change la somme de `effect`.
            let with_effect = |effect: i64, out: &mut Vec<usize>| {
                out.clear();
                for &k in by_value.get(&effect).into_iter().flatten() {
                    if !in_base(&lines[k]) {
                        out.push(k);
                    }
                }
                for &k in by_value.get(&-effect).into_iter().flatten() {
                    if in_base(&lines[k]) {
                        out.push(k);
                    }
                }
            };
            let rest = target - sum;
            let offer = |chosen: &[usize], best: &mut Best| {
                let score = base_cost + chosen.iter().map(|&k| flip(k).1).sum::<f32>();
                if score >= best.worst() {
                    return;
                }
                let mut selected: Vec<bool> = lines.iter().map(&in_base).collect();
                for &k in chosen {
                    selected[k] = !selected[k];
                }
                let pick = |held: bool| -> Vec<usize> {
                    chosen
                        .iter()
                        .filter(|&&k| in_base(&lines[k]) == held)
                        .map(|&k| lines[k].index)
                        .collect()
                };
                let mut chosen_lines: Vec<usize> = lines
                    .iter()
                    .zip(&selected)
                    .filter(|&(_, &s)| s)
                    .map(|(l, _)| l.index)
                    .collect();
                chosen_lines.sort_unstable();
                let total = chosen_lines
                    .iter()
                    .map(|&i| account.operations[i].amount)
                    .sum();
                best.offer(Proposal {
                    lines: chosen_lines,
                    total,
                    score,
                    until: cuts[Class::Other as usize],
                    cards_until: cuts[Class::Card as usize],
                    cheques_until: cuts[Class::Cheque as usize],
                    held_back: pick(true),
                    early: pick(false),
                });
            };
            if rest == 0 {
                if min_k == 0 {
                    offer(&[], &mut best);
                }
                continue;
            }
            let worth = |k: usize, best: &Best| base_cost + k as f32 * MIN_ITEM_COST < best.worst();
            if min_k <= 1 && max_k >= 1 && worth(1, &best) {
                with_effect(rest, &mut found);
                for &k in &found {
                    offer(&[k], &mut best);
                }
            }
            if min_k <= 2 && max_k >= 2 && worth(2, &best) {
                for i in 0..lines.len() {
                    let (effect, cost) = flip(i);
                    if base_cost + cost + MIN_ITEM_COST >= best.worst() {
                        continue;
                    }
                    with_effect(rest - effect, &mut found);
                    for &j in &found {
                        if j > i {
                            offer(&[i, j], &mut best);
                        }
                    }
                }
            }
            if max_k >= 3 && worth(3, &best) {
                for i in 0..lines.len() {
                    if Instant::now() > deadline {
                        break 'passes;
                    }
                    let (effect_i, cost_i) = flip(i);
                    if base_cost + cost_i + 2.0 * MIN_ITEM_COST >= best.worst() {
                        continue;
                    }
                    for j in i + 1..lines.len() {
                        let (effect_j, cost_j) = flip(j);
                        if base_cost + cost_i + cost_j + MIN_ITEM_COST >= best.worst() {
                            continue;
                        }
                        with_effect(rest - effect_i - effect_j, &mut found);
                        for &k in &found {
                            if k > j {
                                offer(&[i, j, k], &mut best);
                            }
                        }
                    }
                }
            }
        }
    }
    if !best.list.is_empty() {
        return SmartTack::Found(best.list.into_iter().map(|e| e.2).collect());
    }
    let values: Vec<i64> = lines.iter().map(|l| l.value).collect();
    match reachable(
        &values,
        target,
        deadline.max(Instant::now() + Duration::from_millis(300)),
    ) {
        Some(false) => SmartTack::Impossible { difference },
        exists => SmartTack::NoPlausible { difference, exists },
    }
}

/// Existe-t-il un sous-ensemble de `values` de somme `target` ? Programmation
/// dynamique sur un champ de bits des sommes atteignables ; `None` si le
/// calcul dépasse les limites (mémoire, temps).
fn reachable(values: &[i64], target: i64, deadline: Instant) -> Option<bool> {
    let negative: i64 = values.iter().filter(|v| **v < 0).sum();
    let positive: i64 = values.iter().filter(|v| **v > 0).sum();
    if target < negative || target > positive {
        return Some(false);
    }
    let width = usize::try_from(positive - negative).ok()? + 1;
    if width > 400_000_000 {
        return None;
    }
    let words = width.div_ceil(64);
    let mut bits = vec![0u64; words];
    let origin = (-negative) as usize;
    bits[origin / 64] |= 1 << (origin % 64);
    for &v in values {
        if Instant::now() > deadline {
            return None;
        }
        let shift = v.unsigned_abs() as usize;
        let (ws, bs) = (shift / 64, shift % 64);
        if v > 0 {
            for i in (ws..words).rev() {
                let mut moved = bits[i - ws] << bs;
                if bs > 0 && i > ws {
                    moved |= bits[i - ws - 1] >> (64 - bs);
                }
                bits[i] |= moved;
            }
        } else {
            for i in 0..words.saturating_sub(ws) {
                let mut moved = bits[i + ws] >> bs;
                if bs > 0 && i + ws + 1 < words {
                    moved |= bits[i + ws + 1] << (64 - bs);
                }
                bits[i] |= moved;
            }
        }
    }
    let goal = (target - negative) as usize;
    Some(bits[goal / 64] >> (goal % 64) & 1 == 1)
}

#[cfg(test)]
mod tests;
