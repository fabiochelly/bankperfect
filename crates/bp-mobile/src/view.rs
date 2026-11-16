//! Consultation : opérations d'un compte, plus récentes d'abord, avec solde
//! progressif, recherche et filtre de pointage.

use bp_core::filter::normalize;
use bp_core::model::{Account, Document, Operation, Tack};
use bp_core::view::{SortColumn, running_balances, sorted_order};

use crate::{BpError, Session, money};

/// État de pointage d'une opération.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TackState {
    None,
    Pointed,
    Reconciled,
}

impl From<Tack> for TackState {
    fn from(tack: Tack) -> Self {
        match tack {
            Tack::None => Self::None,
            Tack::Pointed => Self::Pointed,
            Tack::Reconciled => Self::Reconciled,
        }
    }
}

impl From<TackState> for Tack {
    fn from(tack: TackState) -> Self {
        match tack {
            TackState::None => Self::None,
            TackState::Pointed => Self::Pointed,
            TackState::Reconciled => Self::Reconciled,
        }
    }
}

/// Opérations montrées selon leur pointage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TackView {
    All,
    /// Non pointées.
    Pending,
    /// Pointées ou rapprochées.
    Tacked,
}

/// Une ligne de la liste des opérations.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OperationRow {
    pub uid: u64,
    /// `16/11/2026`.
    pub date: String,
    /// Mois de la ligne (`novembre 2026`), pour les en-têtes de section.
    pub month: String,
    pub payee: String,
    pub details: String,
    /// Catégorie complète (`Parent > Enfant`), vide si aucune.
    pub category: String,
    pub mode: String,
    pub amount: String,
    pub negative: bool,
    pub tack: TackState,
    /// Solde après cette opération (toutes les opérations du compte
    /// comptées, même filtrées).
    pub balance: String,
    pub balance_negative: bool,
    /// Date à venir.
    pub future: bool,
}

/// Opérations d'un compte, telles qu'affichées.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AccountView {
    pub name: String,
    pub balance: String,
    pub balance_negative: bool,
    /// Solde des opérations pointées ou rapprochées.
    pub pointed: String,
    pub rows: Vec<OperationRow>,
    /// Nombre d'opérations du compte (avant recherche et filtre).
    pub total: u32,
}

const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

fn month_label(op: &Operation) -> String {
    let d = op.date.to_naive();
    let month = chrono::Datelike::month(&d) as usize;
    format!("{} {}", MONTHS[month - 1], chrono::Datelike::year(&d))
}

/// Texte où chercher : tiers, détail, catégorie, mode et montant (avec
/// virgule et point), normalisé.
fn haystack(document: &Document, op: &Operation) -> String {
    let category = op
        .category
        .and_then(|id| document.category_label(id))
        .unwrap_or_default();
    let amount = op.amount.abs();
    normalize(&format!(
        "{} {} {} {} {} {}",
        op.payee,
        op.details,
        category,
        op.mode_label(),
        amount.format(2, ','),
        amount.format(2, '.'),
    ))
}

#[uniffi::export]
impl Session {
    /// Opérations du compte `account`, plus récentes d'abord. `search` :
    /// mots qui doivent tous apparaître (tiers, détail, catégorie, mode ou
    /// montant).
    pub fn operations(
        &self,
        account: u32,
        search: String,
        tack: TackView,
    ) -> Result<AccountView, BpError> {
        let mut inner = self.lock();
        let index = inner.account(account)?;
        let words: Vec<String> = normalize(&search)
            .split_whitespace()
            .map(str::to_string)
            .collect();
        if !words.is_empty()
            && inner
                .search
                .as_ref()
                .is_none_or(|(a, r, _)| *a != index || *r != inner.revision)
        {
            let document = &inner.document;
            let texts = document.accounts[index]
                .operations
                .iter()
                .map(|op| haystack(document, op))
                .collect();
            inner.search = Some((index, inner.revision, texts));
        }
        let document = &inner.document;
        let acc: &Account = &document.accounts[index];
        let order = sorted_order(document, acc, SortColumn::Date, false);
        let balances = running_balances(acc, &order, false);
        let today = bp_core::model::Date::today();
        let texts = inner.search.as_ref().map(|(_, _, t)| t);
        let rows = order
            .iter()
            .zip(&balances)
            .filter(|&(&i, _)| {
                let op = &acc.operations[i];
                let shown = match tack {
                    TackView::All => true,
                    TackView::Pending => op.tack == Tack::None,
                    TackView::Tacked => op.tack != Tack::None,
                };
                shown
                    && (words.is_empty()
                        || texts.is_some_and(|t| words.iter().all(|w| t[i].contains(w.as_str()))))
            })
            .map(|(&i, &balance)| {
                let op = &acc.operations[i];
                OperationRow {
                    uid: op.uid,
                    date: op.date.to_string(),
                    month: month_label(op),
                    payee: op.payee.clone(),
                    details: op.details.clone(),
                    category: op
                        .category
                        .and_then(|id| document.category_label(id))
                        .unwrap_or_default(),
                    mode: op.mode_label(),
                    amount: money(op.amount, &acc.currency),
                    negative: op.amount.is_negative(),
                    tack: op.tack.into(),
                    balance: money(balance, &acc.currency),
                    balance_negative: balance.is_negative(),
                    future: op.date > today,
                }
            })
            .collect();
        let balance = acc.balance();
        let pointed = acc
            .operations
            .iter()
            .filter(|op| op.tack != Tack::None)
            .map(|op| op.amount)
            .sum();
        Ok(AccountView {
            name: acc.name.clone(),
            balance: money(balance, &acc.currency),
            balance_negative: balance.is_negative(),
            pointed: money(pointed, &acc.currency),
            rows,
            total: acc.operations.len() as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::{add, file, sample};
    use crate::*;

    #[test]
    fn rows_newest_first_with_balance() {
        let (mut document, protection) = sample();
        add(&mut document, "Boulangerie", -350);
        let session = Session::open(file(&document, &protection), None).unwrap();
        let view = session.operations(0, String::new(), TackView::All).unwrap();
        assert_eq!(view.total, 2);
        // La ligne du haut porte le solde du compte ; à date égale, l'ordre
        // est celui du bureau (mode, tiers…).
        assert_eq!(view.rows[0].balance, "96,50\u{A0}€");
        assert_eq!(view.rows[1].balance, "-3,50\u{A0}€");
    }

    #[test]
    fn search_and_tack_filter() {
        let (mut document, protection) = sample();
        add(&mut document, "Boulangerie Dupont", -350);
        add(&mut document, "Pharmacie", -1290);
        let session = Session::open(file(&document, &protection), None).unwrap();
        let payees = |search: &str, tack| -> Vec<String> {
            session
                .operations(0, search.into(), tack)
                .unwrap()
                .rows
                .into_iter()
                .map(|r| r.payee)
                .collect()
        };
        assert_eq!(payees("boulang", TackView::All), ["Boulangerie Dupont"]);
        assert_eq!(payees("DUPONT boul", TackView::All), ["Boulangerie Dupont"]);
        assert_eq!(payees("12,90", TackView::All), ["Pharmacie"]);
        assert_eq!(payees("12.90", TackView::All), ["Pharmacie"]);
        assert!(payees("pharmacie", TackView::Tacked).is_empty());
        assert_eq!(payees("", TackView::Pending).len(), 2);
    }
}
