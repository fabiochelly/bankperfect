//! Modifications en masse (ex-scripts `search_replace.py` et `modifygroup.py`).

use crate::model::{Account, Amount, Date, Document, Mode, Tack};

/// Remplace `find` par `replacement` dans le tiers et le détail des `lines`.
/// Sensible à la casse ; un `;` du remplacement devient une espace. Renvoie le
/// nombre de remplacements effectués.
pub fn replace_text(
    account: &mut Account,
    lines: &[usize],
    find: &str,
    replacement: &str,
) -> usize {
    if find.is_empty() {
        return 0;
    }
    let replacement = replacement.replace(';', " ");
    let mut count = 0;
    for &i in lines {
        let op = &mut account.operations[i];
        for text in [&mut op.payee, &mut op.details] {
            let n = text.matches(find).count();
            if n > 0 {
                *text = text.replace(find, &replacement);
                count += n;
            }
        }
    }
    count
}

/// Nombre d'occurrences de `find` dans le tiers et le détail des `lines`.
pub fn count_matches(account: &Account, lines: &[usize], find: &str) -> usize {
    if find.is_empty() {
        return 0;
    }
    lines
        .iter()
        .map(|&i| {
            let op = &account.operations[i];
            op.payee.matches(find).count() + op.details.matches(find).count()
        })
        .sum()
}

/// Champs à appliquer à un groupe d'opérations ; `None` = inchangé.
#[derive(Debug, Clone, Default)]
pub struct GroupEdit {
    pub date: Option<Date>,
    /// Appliqué seulement aux lignes dont le montant a le même signe que le mode.
    pub mode: Option<Mode>,
    pub payee: Option<String>,
    pub details: Option<String>,
    /// `Some(None)` retire la catégorie.
    pub category: Option<Option<u32>>,
    /// Valeur absolue ; le signe vient du mode de chaque ligne.
    pub amount: Option<Amount>,
    pub tack: Option<Tack>,
}

/// Le mode est-il un mode de dépense ?
pub fn mode_is_debit(document: &Document, mode: &Mode) -> bool {
    match mode {
        Mode::Standard(m) => m.is_debit(),
        Mode::Custom(name) => document
            .custom_modes
            .iter()
            .find(|m| &m.name == name)
            .is_none_or(|m| m.debit),
    }
}

/// Applique `edit` aux `lines` ; renvoie le nombre de lignes modifiées.
pub fn apply_group_edit(
    document: &Document,
    account: &mut Account,
    lines: &[usize],
    edit: &GroupEdit,
) -> usize {
    let today = Date::today();
    let mut changed = 0;
    for &i in lines {
        let op = &mut account.operations[i];
        let before = op.clone();
        if let Some(date) = edit.date {
            op.date = date;
            op.mark_date = date;
        }
        if let Some(mode) = &edit.mode
            && mode_is_debit(document, mode) == op.amount.is_negative()
        {
            op.mode = mode.clone();
        }
        if let Some(payee) = &edit.payee {
            op.payee = payee.replace(';', " ");
        }
        if let Some(details) = &edit.details {
            op.details = details.replace(';', " ");
        }
        if let Some(category) = edit.category {
            op.category = category;
        }
        if let Some(amount) = edit.amount {
            let abs = Amount(amount.0.abs());
            op.amount = if mode_is_debit(document, &op.mode) {
                -abs
            } else {
                abs
            };
        }
        if let Some(tack) = edit.tack
            && op.tack != tack
        {
            op.tack = tack;
            op.mark_date = today;
        }
        if *op != before {
            changed += 1;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn account() -> Account {
        let op = |payee: &str, cents: i64, mode: StandardMode| Operation {
            uid: 1,
            date: Date(45000),
            mark_date: Date(45000),
            tack: Tack::None,
            mode: Mode::Standard(mode),
            payee: payee.into(),
            details: "EDF facture EDF".into(),
            category: None,
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        };
        Account {
            uid: "a".into(),
            name: "Courant".into(),
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
            operations: vec![
                op("EDF", -5000, StandardMode::Prelevement),
                op("Salaire", 200_000, StandardMode::VirementRecu),
            ],
        }
    }

    fn document() -> Document {
        Document {
            group_name: String::new(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: Vec::new(),
            accounts: Vec::new(),
            account_order: Vec::new(),
            default_account: 0,
            categories: Vec::new(),
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        }
    }

    #[test]
    fn replaces_in_payee_and_details() {
        let mut acc = account();
        assert_eq!(replace_text(&mut acc, &[0], "EDF", "Engie;"), 3);
        assert_eq!(acc.operations[0].payee, "Engie ");
        assert_eq!(acc.operations[0].details, "Engie  facture Engie ");
        assert_eq!(
            acc.operations[1].details, "EDF facture EDF",
            "ligne non sélectionnée intacte"
        );
        assert_eq!(
            replace_text(&mut acc, &[0, 1], "edf", "x"),
            0,
            "sensible à la casse"
        );
    }

    #[test]
    fn group_edit_respects_mode_sign() {
        let mut acc = account();
        let edit = GroupEdit {
            mode: Some(Mode::Standard(StandardMode::Carte)),
            amount: Some(Amount::from_cents(1234)),
            tack: Some(Tack::Pointed),
            ..Default::default()
        };
        assert_eq!(apply_group_edit(&document(), &mut acc, &[0, 1], &edit), 2);
        let ops = &acc.operations;
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::Carte));
        assert_eq!(ops[0].amount, Amount::from_cents(-1234));
        assert_eq!(
            ops[1].mode,
            Mode::Standard(StandardMode::VirementRecu),
            "recette : mode de dépense non appliqué"
        );
        assert_eq!(ops[1].amount, Amount::from_cents(1234));
        assert!(ops.iter().all(|op| op.tack == Tack::Pointed));
    }
}
