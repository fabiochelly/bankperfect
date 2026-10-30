//! Archivage des opérations anciennes, comme le plugin « Archive » de Robert
//! Gras pour l'ancienne version : les opérations d'une période sont
//! remplacées par une écriture de synthèse par statut de pointage, et
//! gardées dans le fichier pour être consultées ou restaurées.
//!
//! Une écriture par statut (rapprochée, pointée, non pointée) : ainsi le
//! solde, le pointé et le rapproché du compte ne changent pas. Les
//! écritures de synthèse portent la catégorie « Archive », pour ne pas
//! gonfler « Sans catégorie » dans les rapports.

use serde::{Deserialize, Serialize};

use crate::feature_store::{self, FeatureError};
use crate::model::{Account, Amount, Date, Document, Mode, Operation, StandardMode, Tack};
use crate::tr;

/// Clé des archives dans les données du document.
pub const KEY: &str = "archive.v6";

/// Nom de la catégorie des écritures de synthèse.
const CATEGORY: &str = "Archive";

/// Opérations remplacées par une écriture de synthèse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Identifiant de l'écriture de synthèse.
    pub synthesis: u64,
    /// Identifiant du compte.
    pub account: String,
    pub from: Date,
    pub to: Date,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Archives {
    pub entries: Vec<Entry>,
}

impl Archives {
    pub fn load(document: &Document) -> Result<Self, FeatureError> {
        feature_store::get(document, KEY).unwrap_or(Ok(Self::default()))
    }

    fn save(&self, document: &mut Document) -> Result<(), FeatureError> {
        if self.entries.is_empty() {
            feature_store::remove(document, KEY);
            Ok(())
        } else {
            feature_store::set(document, KEY, self)
        }
    }

    /// Archive d'une écriture de synthèse.
    pub fn entry(&self, synthesis: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.synthesis == synthesis)
    }
}

/// Ce que l'archivage d'un compte remplacerait, par statut de pointage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Group {
    pub tack: Tack,
    pub count: usize,
    pub total: Amount,
}

/// Lignes du compte datées de `from` (si donné) à `to` inclus ; seulement
/// les rapprochées si `only_reconciled`.
pub fn select(
    account: &Account,
    from: Option<Date>,
    to: Date,
    only_reconciled: bool,
) -> Vec<usize> {
    account
        .operations
        .iter()
        .enumerate()
        .filter(|(_, op)| {
            from.is_none_or(|d| op.date >= d)
                && op.date <= to
                && (!only_reconciled || op.tack == Tack::Reconciled)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Regroupement par statut des lignes `lines` (une écriture par groupe).
pub fn preview(account: &Account, lines: &[usize]) -> Vec<Group> {
    [Tack::Reconciled, Tack::Pointed, Tack::None]
        .into_iter()
        .filter_map(|tack| {
            let ops: Vec<&Operation> = lines
                .iter()
                .map(|&i| &account.operations[i])
                .filter(|op| op.tack == tack)
                .collect();
            (!ops.is_empty()).then(|| Group {
                tack,
                count: ops.len(),
                total: ops.iter().map(|op| op.amount).sum(),
            })
        })
        .collect()
}

/// Remplace les lignes `lines` du compte `account` par une écriture de
/// synthèse par statut, datée de la dernière ligne archivée. Renvoie les
/// identifiants des écritures créées.
pub fn archive(
    document: &mut Document,
    account: usize,
    lines: &[usize],
) -> Result<Vec<u64>, FeatureError> {
    let mut archives = Archives::load(document)?;
    let mut lines = lines.to_vec();
    lines.sort_unstable();
    lines.dedup();
    if lines.is_empty() || account >= document.accounts.len() {
        return Ok(Vec::new());
    }
    let category = archive_category(document);
    let groups = preview(&document.accounts[account], &lines);
    let first_uid = document.next_operation_uid();
    // Retirées de la fin vers le début pour garder les index valides.
    let mut removed: Vec<Operation> = lines
        .iter()
        .rev()
        .map(|&i| document.accounts[account].operations.remove(i))
        .collect();
    removed.reverse();
    let account_uid = document.accounts[account].uid.clone();
    let mut created = Vec::new();
    for (uid, group) in (first_uid..).zip(groups) {
        let ops: Vec<Operation> = removed
            .iter()
            .filter(|op| op.tack == group.tack)
            .cloned()
            .collect();
        let from = ops.iter().map(|op| op.date).min().expect("groupe non vide");
        let to = ops.iter().map(|op| op.date).max().expect("groupe non vide");
        let mode = if group.total.is_negative() {
            StandardMode::Prelevement
        } else {
            StandardMode::VirementRecu
        };
        document.accounts[account].operations.push(Operation {
            uid,
            date: to,
            mark_date: to,
            tack: group.tack,
            mode: Mode::Standard(mode),
            payee: tr!("Opérations archivées").into(),
            details: if crate::i18n::singular(group.count as i64) {
                tr!(
                    "{} opération du {} au {}",
                    group.count,
                    from.to_string(),
                    to.to_string()
                )
            } else {
                tr!(
                    "{} opérations du {} au {}",
                    group.count,
                    from.to_string(),
                    to.to_string()
                )
            },
            category: Some(category),
            amount: group.total,
            extra: Vec::new(),
        });
        archives.entries.push(Entry {
            synthesis: uid,
            account: account_uid.clone(),
            from,
            to,
            operations: ops,
        });
        created.push(uid);
    }
    archives.save(document)?;
    Ok(created)
}

/// Remet les opérations archivées sous l'écriture de synthèse `synthesis`
/// et retire celle-ci ; renvoie l'index du compte et le nombre
/// d'opérations restaurées. Une opération dont l'identifiant a été repris
/// entre-temps en reçoit un nouveau.
pub fn restore(
    document: &mut Document,
    synthesis: u64,
) -> Result<Option<(usize, usize)>, FeatureError> {
    let mut archives = Archives::load(document)?;
    let Some(pos) = archives
        .entries
        .iter()
        .position(|e| e.synthesis == synthesis)
    else {
        return Ok(None);
    };
    let Some(account) = document
        .accounts
        .iter()
        .position(|a| a.uid == archives.entries[pos].account)
    else {
        return Ok(None);
    };
    let entry = archives.entries.remove(pos);
    document.accounts[account]
        .operations
        .retain(|op| op.uid != synthesis);
    let mut next_uid = document.next_operation_uid();
    let taken: std::collections::HashSet<u64> = document
        .accounts
        .iter()
        .flat_map(|a| a.operations.iter().map(|op| op.uid))
        .collect();
    let count = entry.operations.len();
    for mut op in entry.operations {
        if taken.contains(&op.uid) {
            op.uid = next_uid;
            next_uid += 1;
        }
        document.accounts[account].operations.push(op);
    }
    archives.save(document)?;
    Ok(Some((account, count)))
}

/// Identifiant de la catégorie « Archive », créée au besoin.
fn archive_category(document: &mut Document) -> u32 {
    let name = tr!("Archive");
    document
        .categories
        .iter()
        .find(|c| !c.is_child && (c.name == name || c.name == CATEGORY))
        .map(|c| c.id)
        .unwrap_or_else(|| crate::manage::insert_category(document, name, false, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(uid: u64, day: i32, tack: Tack, cents: i64) -> Operation {
        Operation {
            uid,
            date: Date(day),
            mark_date: Date(day),
            tack,
            mode: Mode::Standard(StandardMode::Carte),
            payee: format!("tiers {uid}"),
            details: String::new(),
            category: None,
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        }
    }

    fn document() -> Document {
        let mut document = crate::manage::new_document("test", Amount::ZERO);
        document.accounts[0].operations = vec![
            op(1, 100, Tack::Reconciled, -1000),
            op(2, 110, Tack::Reconciled, 5000),
            op(3, 120, Tack::Pointed, -300),
            op(4, 130, Tack::None, -200),
            op(5, 400, Tack::Reconciled, -700),
        ];
        document
    }

    fn totals(account: &Account) -> (Amount, Amount, Amount) {
        let sum = |f: &dyn Fn(&Operation) -> bool| {
            account
                .operations
                .iter()
                .filter(|op| f(op))
                .map(|op| op.amount)
                .sum()
        };
        (
            sum(&|_| true),
            sum(&|op| op.tack != Tack::None),
            sum(&|op| op.tack == Tack::Reconciled),
        )
    }

    #[test]
    fn archive_keeps_balances_and_restore_undoes_it() {
        let mut document = document();
        let before = totals(&document.accounts[0]);
        let original = document.accounts[0].operations.clone();
        let lines = select(&document.accounts[0], None, Date(200), false);
        assert_eq!(lines, vec![0, 1, 2, 3]);

        let created = archive(&mut document, 0, &lines).unwrap();
        assert_eq!(created.len(), 3, "une écriture par statut");
        let account = &document.accounts[0];
        assert_eq!(account.operations.len(), 4);
        assert_eq!(totals(account), before);
        let reconciled = account
            .operations
            .iter()
            .find(|op| op.uid == created[0])
            .unwrap();
        assert_eq!(reconciled.amount, Amount::from_cents(4000));
        assert_eq!(reconciled.date, Date(110));
        let category = reconciled.category.unwrap();
        assert!(
            document
                .categories
                .iter()
                .any(|c| c.id == category && c.name == "Archive")
        );

        // Un second archivage réutilise la catégorie.
        let count = document.categories.len();
        let lines = select(&document.accounts[0], Some(Date(400)), Date(400), true);
        archive(&mut document, 0, &lines).unwrap();
        assert_eq!(document.categories.len(), count);

        let archives = Archives::load(&document).unwrap();
        assert_eq!(archives.entries.len(), 4);
        for entry in archives.entries {
            restore(&mut document, entry.synthesis).unwrap().unwrap();
        }
        let mut restored = document.accounts[0].operations.clone();
        restored.sort_by_key(|op| op.uid);
        assert_eq!(restored, original);
        assert!(feature_store::raw(&document, KEY).is_none());
    }

    #[test]
    fn only_reconciled() {
        let document = document();
        let lines = select(&document.accounts[0], None, Date(200), true);
        assert_eq!(lines, vec![0, 1]);
        let groups = preview(&document.accounts[0], &lines);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].count, 2);
    }

    #[test]
    fn restore_renumbers_a_reused_uid() {
        let mut document = document();
        let created = archive(&mut document, 0, &[0]).unwrap();
        // L'identifiant 1 est repris par une nouvelle opération.
        document.accounts[0]
            .operations
            .push(op(1, 500, Tack::None, -100));
        restore(&mut document, created[0]).unwrap().unwrap();
        let mut uids: Vec<u64> = document.accounts[0]
            .operations
            .iter()
            .map(|op| op.uid)
            .collect();
        uids.sort_unstable();
        uids.dedup();
        assert_eq!(uids.len(), document.accounts[0].operations.len());
    }
}
