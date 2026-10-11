//! Ventilation d'une opération et transferts de compte à compte (ex-scripts
//! `ventilate.py`, `transfer.py` et `convert_line_to_transfer.py`, et
//! `GetTransferSister` de `unitMain.pas`).
//!
//! Un transfert est un couple d'opérations dans deux comptes différents :
//!
//! - tiers `Transfert vers le compte <destination>` (débit) et
//!   `Transfert depuis le compte <source>` (crédit) ;
//! - champs supplémentaires `#sisterAccUid` (UID du compte de l'autre ligne)
//!   et `#sisterLineUid` (UId de l'autre ligne) sur chacune des deux lignes.

use crate::edit::mode_is_debit;
use crate::model::{Amount, Date, Document, Mode, Operation, StandardMode, Tack};
use crate::view::payee_without_ventilation;

/// Début du tiers d'un transfert sortant.
pub const TRANSFER_TO: &str = "Transfert vers le compte ";
/// Début du tiers d'un transfert entrant.
pub const TRANSFER_FROM: &str = "Transfert depuis le compte ";
/// Champ supplémentaire : UID du compte de la ligne sœur.
pub const SISTER_ACCOUNT_KEY: &str = "#sisterAccUid";
/// Champ supplémentaire : UId de la ligne sœur.
pub const SISTER_LINE_KEY: &str = "#sisterLineUid";

/// Échec d'une ventilation ou d'un transfert. Aucune modification n'est faite
/// quand une erreur est renvoyée.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplitError {
    #[error("compte introuvable")]
    AccountNotFound,
    #[error("opération introuvable")]
    LineNotFound,
    #[error("aucune part à ventiler")]
    NoParts,
    #[error("la part n°{0} a un montant nul ou négatif")]
    InvalidPart(usize),
    #[error("le montant d'origine n'a pas été totalement ventilé (reste {})", .0.format(2, ','))]
    Unbalanced(Amount),
    #[error("les deux comptes liés au transfert doivent être différents")]
    SameAccount,
    #[error("le montant est incorrect (il ne doit pas être nul)")]
    ZeroAmount,
    #[error("le mode de paiement de la ligne ne permet pas de créer un transfert")]
    ModeNotConvertible,
    #[error("la ligne est déjà un transfert")]
    AlreadyTransfer,
}

// ---------------------------------------------------------------------------
// Champs supplémentaires
// ---------------------------------------------------------------------------

/// Écrit un champ supplémentaire (`SetExtraValue`) : remplace la première
/// entrée de même nom (nom insensible à la casse) ou l'ajoute à la fin ; une
/// valeur vide supprime toutes les entrées de ce nom. `UId` est refusé
/// (l'identifiant de ligne est `Operation::uid`) : renvoie `false`.
pub fn set_extra(op: &mut Operation, key: &str, value: &str) -> bool {
    if key.is_empty() || key.eq_ignore_ascii_case("UId") {
        return false;
    }
    if value.is_empty() {
        op.extra.retain(|(k, _)| k != key);
        return true;
    }
    match op
        .extra
        .iter_mut()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
    {
        Some(entry) => entry.1 = value.to_string(),
        None => op.extra.push((key.to_string(), value.to_string())),
    }
    true
}

// ---------------------------------------------------------------------------
// Ventilation
// ---------------------------------------------------------------------------

/// Une part de ventilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPart {
    pub details: String,
    pub category: Option<u32>,
    /// Montant de la part, en valeur absolue (le signe est celui de
    /// l'opération d'origine).
    pub amount: Amount,
}

/// Montant restant à ventiler (valeur absolue de l'opération moins la somme
/// des parts) ; négatif si les parts dépassent le montant.
pub fn remaining_to_split(original: Amount, parts: &[SplitPart]) -> Amount {
    Amount(original.0.abs() - parts.iter().map(|p| p.amount.0).sum::<i64>())
}

/// Répartit `total` (valeur absolue) en `n` parts égales au centime près :
/// les centimes restants vont aux premières parts. Si `total` comporte des
/// fractions de centime, la répartition se fait au dix-millième.
pub fn split_evenly(total: Amount, n: usize) -> Vec<Amount> {
    if n == 0 {
        return Vec::new();
    }
    let total = total.0.abs();
    let unit = if total % 100 == 0 { 100 } else { 1 };
    let units = total / unit;
    let (base, extra) = (units / n as i64, units % n as i64);
    (0..n as i64)
        .map(|i| Amount((base + i64::from(i < extra)) * unit))
        .collect()
}

/// Ventile l'opération `line` du compte `account` : elle est remplacée, à la
/// même position, par une ligne par part (`ventilate.py`).
///
/// Chaque ligne garde la date, le mode, le pointage et les champs
/// supplémentaires de l'origine (sauf les liens de transfert), reçoit un
/// nouvel UId, le tiers `[i/N] tiers` et le détail `détail (Total: X)`. La
/// somme des parts doit être exactement égale au montant d'origine (montants
/// entiers en dix-millièmes : aucune erreur d'arrondi). Renvoie les index des
/// nouvelles lignes.
pub fn split_operation(
    document: &mut Document,
    account: usize,
    line: usize,
    parts: &[SplitPart],
) -> Result<Vec<usize>, SplitError> {
    let original = document
        .accounts
        .get(account)
        .ok_or(SplitError::AccountNotFound)?
        .operations
        .get(line)
        .ok_or(SplitError::LineNotFound)?
        .clone();
    if parts.is_empty() {
        return Err(SplitError::NoParts);
    }
    if let Some(i) = parts.iter().position(|p| p.amount.0 <= 0) {
        return Err(SplitError::InvalidPart(i + 1));
    }
    let rest = remaining_to_split(original.amount, parts);
    if rest.0 != 0 {
        return Err(SplitError::Unbalanced(rest));
    }
    let sign = if original.amount.is_negative() { -1 } else { 1 };
    // Une ligne déjà ventilée ne cumule pas les préfixes « [1/2] [1/3] ».
    let payee = payee_without_ventilation(&original.payee).trim_start();
    let total = original.amount.format(2, ',');
    let n = parts.len();
    let mut uid = document.next_operation_uid();
    let new_ops: Vec<Operation> = parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let details = part.details.replace(';', " ");
            let details = details.trim();
            let mut op = Operation {
                uid,
                payee: format!("[{}/{n}] {payee}", i + 1),
                details: if details.is_empty() {
                    format!("(Total: {total})")
                } else {
                    format!("{details} (Total: {total})")
                },
                category: part.category,
                amount: Amount(sign * part.amount.0),
                ..original.clone()
            };
            op.extra
                .retain(|(k, _)| k != SISTER_ACCOUNT_KEY && k != SISTER_LINE_KEY);
            uid += 1;
            op
        })
        .collect();
    let ops = &mut document.accounts[account].operations;
    ops.splice(line..=line, new_ops);
    Ok((line..line + n).collect())
}

// ---------------------------------------------------------------------------
// Transferts
// ---------------------------------------------------------------------------

/// Sens d'une ligne de transfert, d'après son tiers (`TransferType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    /// « Transfert vers le compte … » (sortie).
    To,
    /// « Transfert depuis le compte … » (entrée).
    From,
}

/// Sens d'un transfert et nom du compte cité, si le tiers en est un.
pub fn transfer_direction(payee: &str) -> Option<(TransferDirection, &str)> {
    if let Some(name) = payee.strip_prefix(TRANSFER_TO) {
        Some((TransferDirection::To, name))
    } else {
        payee
            .strip_prefix(TRANSFER_FROM)
            .map(|name| (TransferDirection::From, name))
    }
}

/// Type de transfert proposé par la fenêtre « Transfert de compte à compte » :
/// chacun associe un mode émis (compte source) et un mode reçu (destination).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransferKind {
    #[default]
    Transfer,
    Cheque,
    Cash,
}

impl TransferKind {
    pub const ALL: [TransferKind; 3] = [Self::Transfer, Self::Cheque, Self::Cash];

    pub fn label(self) -> &'static str {
        match self {
            Self::Transfer => "Virement",
            Self::Cheque => "Chèque",
            Self::Cash => "Espèces",
        }
    }

    /// Modes (source, destination).
    pub fn modes(self) -> (StandardMode, StandardMode) {
        match self {
            Self::Transfer => (StandardMode::VirementEmis, StandardMode::VirementRecu),
            Self::Cheque => (StandardMode::ChequeEmis, StandardMode::DepotCheque),
            Self::Cash => (StandardMode::RetraitDab, StandardMode::Versement),
        }
    }
}

/// Demande de transfert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRequest {
    pub date: Date,
    pub kind: TransferKind,
    pub details: String,
    pub category: Option<u32>,
    /// Montant transféré (le signe est ignoré).
    pub amount: Amount,
    /// Compte débité.
    pub from: usize,
    /// Compte crédité.
    pub to: usize,
}

/// Position d'une opération : (compte, index de la ligne).
pub type LinePosition = (usize, usize);

/// Lignes d'un transfert créé ou converti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferLines {
    /// Ligne du compte débité (ou ligne convertie).
    pub first: LinePosition,
    /// Ligne sœur.
    pub second: LinePosition,
}

/// Lie deux lignes par leurs champs `#sisterAccUid` / `#sisterLineUid`.
fn link(document: &mut Document, a: LinePosition, b: LinePosition) {
    let (uid_a, line_a) = (
        document.accounts[a.0].uid.clone(),
        document.accounts[a.0].operations[a.1].uid,
    );
    let (uid_b, line_b) = (
        document.accounts[b.0].uid.clone(),
        document.accounts[b.0].operations[b.1].uid,
    );
    let op_a = &mut document.accounts[a.0].operations[a.1];
    set_extra(op_a, SISTER_ACCOUNT_KEY, &uid_b);
    set_extra(op_a, SISTER_LINE_KEY, &line_b.to_string());
    let op_b = &mut document.accounts[b.0].operations[b.1];
    set_extra(op_b, SISTER_ACCOUNT_KEY, &uid_a);
    set_extra(op_b, SISTER_LINE_KEY, &line_a.to_string());
}

fn new_operation(uid: u64, date: Date, mode: StandardMode, payee: String) -> Operation {
    Operation {
        uid,
        date,
        mark_date: date,
        tack: Tack::None,
        mode: Mode::Standard(mode),
        payee,
        details: String::new(),
        category: None,
        amount: Amount::ZERO,
        extra: Vec::new(),
    }
}

/// Crée un transfert (`transfer.py`) : une ligne « Transfert vers le compte
/// <to> » débitée dans `from`, une ligne « Transfert depuis le compte
/// <from> » créditée dans `to`, ajoutées en fin de compte et liées.
///
/// Corrections : plus de traces de débogage ; tout est vérifié avant la
/// moindre modification.
pub fn create_transfer(
    document: &mut Document,
    request: &TransferRequest,
) -> Result<TransferLines, SplitError> {
    let n = document.accounts.len();
    if request.from >= n || request.to >= n {
        return Err(SplitError::AccountNotFound);
    }
    if request.from == request.to {
        return Err(SplitError::SameAccount);
    }
    let amount = request.amount.0.abs();
    if amount == 0 {
        return Err(SplitError::ZeroAmount);
    }
    let (mode_from, mode_to) = request.kind.modes();
    let details = request.details.replace(';', " ");
    let from_name = document.accounts[request.from].name.clone();
    let to_name = document.accounts[request.to].name.clone();
    let uid = document.next_operation_uid();

    let mut debit = new_operation(
        uid,
        request.date,
        mode_from,
        format!("{TRANSFER_TO}{to_name}"),
    );
    debit.details = details.clone();
    debit.category = request.category;
    debit.amount = Amount(-amount);
    let mut credit = new_operation(
        uid + 1,
        request.date,
        mode_to,
        format!("{TRANSFER_FROM}{from_name}"),
    );
    credit.details = details;
    credit.category = request.category;
    credit.amount = Amount(amount);

    let first = push(document, request.from, debit);
    let second = push(document, request.to, credit);
    link(document, first, second);
    Ok(TransferLines { first, second })
}

fn push(document: &mut Document, account: usize, op: Operation) -> LinePosition {
    let ops = &mut document.accounts[account].operations;
    ops.push(op);
    (account, ops.len() - 1)
}

/// Mode réciproque utilisé par « Convertir la ligne en transfert » : le mode
/// de la ligne créée dans l'autre compte. `None` si le mode ne s'y prête pas
/// (carte, TIP, titre, modes personnalisés).
pub fn reciprocal_mode(mode: &Mode) -> Option<StandardMode> {
    use StandardMode::*;
    match mode {
        Mode::Standard(Prelevement | VirementEmis) => Some(VirementRecu),
        Mode::Standard(ChequeEmis) => Some(DepotCheque),
        Mode::Standard(RetraitDab) => Some(Versement),
        Mode::Standard(VirementRecu) => Some(VirementEmis),
        Mode::Standard(DepotCheque) => Some(ChequeEmis),
        Mode::Standard(Versement) => Some(RetraitDab),
        _ => None,
    }
}

/// Convertit une ligne existante en transfert (`convert_line_to_transfer.py`).
///
/// Crée dans `other` la ligne miroir (mode réciproque, montant opposé, même
/// date et catégorie, non pointée), remplace les tiers par « Transfert
/// vers/depuis le compte … », met l'ancien tiers en tête du détail
/// (`tiers, détail`) et lie les deux lignes.
///
/// Correction : l'original écrivait les liens même si l'ajout de la ligne
/// miroir avait échoué (sur la ligne 0 de l'autre compte) ; ici tout est
/// vérifié avant modification.
pub fn convert_to_transfer(
    document: &mut Document,
    account: usize,
    line: usize,
    other: usize,
) -> Result<TransferLines, SplitError> {
    let n = document.accounts.len();
    if account >= n || other >= n {
        return Err(SplitError::AccountNotFound);
    }
    if account == other {
        return Err(SplitError::SameAccount);
    }
    let op = document.accounts[account]
        .operations
        .get(line)
        .ok_or(SplitError::LineNotFound)?
        .clone();
    if transfer_direction(&op.payee).is_some() {
        return Err(SplitError::AlreadyTransfer);
    }
    let mode2 = reciprocal_mode(&op.mode).ok_or(SplitError::ModeNotConvertible)?;
    let account_name = document.accounts[account].name.clone();
    let other_name = document.accounts[other].name.clone();
    let (this_prefix, other_prefix) = if op.amount.is_negative() {
        (TRANSFER_TO, TRANSFER_FROM)
    } else {
        (TRANSFER_FROM, TRANSFER_TO)
    };
    let details = [op.payee.trim(), op.details.trim()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");

    let mut mirror = new_operation(
        document.next_operation_uid(),
        op.date,
        mode2,
        format!("{other_prefix}{account_name}"),
    );
    mirror.details = details.clone();
    mirror.category = op.category;
    mirror.amount = -op.amount;

    let second = push(document, other, mirror);
    let current = &mut document.accounts[account].operations[line];
    current.payee = format!("{this_prefix}{other_name}");
    current.details = details;
    link(document, (account, line), second);
    Ok(TransferLines {
        first: (account, line),
        second,
    })
}

/// Recherche la ligne sœur d'une ligne de transfert (`GetTransferSister`).
///
/// 1. Compte sœur : celui dont l'UID est `#sisterAccUid`, sinon celui dont le
///    nom suit « Transfert vers/depuis le compte » (casse ignorée à défaut
///    d'égalité exacte).
/// 2. Ligne sœur : celle dont l'UId est `#sisterLineUid`, sinon la première
///    ligne de même date, de tiers « Transfert » de sens opposé et de montant
///    exactement opposé.
pub fn find_sister(document: &Document, account: usize, line: usize) -> Option<LinePosition> {
    let op = document.accounts.get(account)?.operations.get(line)?;
    let (direction, name) = transfer_direction(&op.payee)?;
    let sister_account = op
        .extra(SISTER_ACCOUNT_KEY)
        .filter(|uid| !uid.is_empty())
        .and_then(|uid| document.accounts.iter().position(|a| a.uid == uid))
        .or_else(|| document.accounts.iter().position(|a| a.name == name))
        .or_else(|| {
            let lower = name.to_lowercase();
            document
                .accounts
                .iter()
                .position(|a| a.name.to_lowercase() == lower)
        })?;
    let ops = &document.accounts[sister_account].operations;
    if let Some(uid) = op
        .extra(SISTER_LINE_KEY)
        .and_then(|v| v.trim().parse::<u64>().ok())
        && let Some(i) = ops.iter().position(|o| o.uid == uid)
    {
        return Some((sister_account, i));
    }
    let opposite = match direction {
        TransferDirection::To => TRANSFER_FROM,
        TransferDirection::From => TRANSFER_TO,
    };
    ops.iter()
        .position(|o| o.date == op.date && o.payee.starts_with(opposite) && o.amount == -op.amount)
        .map(|i| (sister_account, i))
}

/// Le montant d'une ligne respecte-t-il le sens de son mode ? (utile pour
/// vérifier une conversion : un prélèvement positif est un remboursement).
pub fn amount_matches_mode(document: &Document, op: &Operation) -> bool {
    op.amount.0 == 0 || mode_is_debit(document, &op.mode) == op.amount.is_negative()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::tests::{account, document, op};

    fn sample() -> Document {
        let mut fnac = op(10, (2024, 5, 3), -12_345);
        fnac.payee = "Fnac".into();
        fnac.details = "Livres et disques".into();
        fnac.category = Some(1);
        fnac.tack = Tack::Pointed;
        fnac.extra = vec![("freeicon".into(), "star".into())];
        let mut virement = op(11, (2024, 5, 4), -50_000);
        virement.mode = Mode::Standard(StandardMode::VirementEmis);
        virement.payee = "Épargne".into();
        virement.details = "mai".into();
        virement.category = Some(4);
        document(vec![
            account("uid-a", "Courant", vec![fnac, virement]),
            account("uid-b", "Livret A", vec![op(12, (2024, 1, 1), 100_000)]),
        ])
    }

    fn part(details: &str, category: Option<u32>, cents: i64) -> SplitPart {
        SplitPart {
            details: details.into(),
            category,
            amount: Amount::from_cents(cents),
        }
    }

    #[test]
    fn split_sums_to_original() {
        let mut doc = sample();
        let parts = [part("Livres", Some(2), 4_000), part("", Some(3), 8_345)];
        assert_eq!(
            remaining_to_split(doc.accounts[0].operations[0].amount, &parts),
            Amount::ZERO
        );
        let lines = split_operation(&mut doc, 0, 0, &parts).unwrap();
        assert_eq!(lines, [0, 1]);
        let ops = &doc.accounts[0].operations;
        assert_eq!(ops.len(), 3);
        let sum: Amount = ops[..2].iter().map(|o| o.amount).sum();
        assert_eq!(sum, Amount::from_cents(-12_345), "au centime près");
        assert_eq!(ops[0].payee, "[1/2] Fnac");
        assert_eq!(ops[1].payee, "[2/2] Fnac");
        assert_eq!(ops[0].details, "Livres (Total: -123,45)");
        assert_eq!(ops[1].details, "(Total: -123,45)");
        assert_eq!(ops[0].category, Some(2));
        assert_eq!(ops[0].tack, Tack::Pointed);
        assert_eq!(ops[0].extra("freeicon"), Some("star"));
        assert_eq!((ops[0].uid, ops[1].uid), (13, 14));
        assert_eq!(ops[2].uid, 11, "ligne suivante intacte");

        // Reventiler une part ne cumule pas les préfixes.
        split_operation(
            &mut doc,
            0,
            1,
            &[part("a", None, 8_000), part("b", None, 345)],
        )
        .unwrap();
        assert_eq!(doc.accounts[0].operations[1].payee, "[1/2] Fnac");
    }

    #[test]
    fn split_errors_leave_document_untouched() {
        let mut doc = sample();
        let before = doc.clone();
        assert_eq!(
            split_operation(&mut doc, 0, 0, &[part("", None, 12_000)]),
            Err(SplitError::Unbalanced(Amount::from_cents(345)))
        );
        assert_eq!(
            split_operation(&mut doc, 0, 0, &[part("", None, 12_345), part("", None, 0)]),
            Err(SplitError::InvalidPart(2))
        );
        assert_eq!(
            split_operation(&mut doc, 0, 0, &[]),
            Err(SplitError::NoParts)
        );
        assert_eq!(
            split_operation(&mut doc, 0, 9, &[part("", None, 1)]),
            Err(SplitError::LineNotFound)
        );
        assert_eq!(
            split_operation(&mut doc, 5, 0, &[part("", None, 1)]),
            Err(SplitError::AccountNotFound)
        );
        assert_eq!(doc, before);
    }

    #[test]
    fn even_split() {
        let parts = split_evenly(Amount::from_cents(-10_000), 3);
        assert_eq!(
            parts,
            [
                Amount::from_cents(3_334),
                Amount::from_cents(3_333),
                Amount::from_cents(3_333)
            ]
        );
        assert_eq!(
            parts.iter().copied().sum::<Amount>(),
            Amount::from_cents(10_000)
        );
        assert_eq!(
            split_evenly(Amount(10), 3),
            [Amount(4), Amount(3), Amount(3)]
        );
        assert!(split_evenly(Amount(10), 0).is_empty());
    }

    #[test]
    fn transfer_creates_two_linked_lines() {
        let mut doc = sample();
        let request = TransferRequest {
            date: Date::from_ymd(2024, 6, 1).unwrap(),
            kind: TransferKind::Transfer,
            details: "épargne;juin".into(),
            category: Some(4),
            amount: Amount::from_cents(-20_000),
            from: 0,
            to: 1,
        };
        let lines = create_transfer(&mut doc, &request).unwrap();
        assert_eq!(lines.first, (0, 2));
        assert_eq!(lines.second, (1, 1));
        let debit = &doc.accounts[0].operations[2];
        let credit = &doc.accounts[1].operations[1];
        assert_eq!(debit.payee, "Transfert vers le compte Livret A");
        assert_eq!(credit.payee, "Transfert depuis le compte Courant");
        assert_eq!(debit.amount, Amount::from_cents(-20_000));
        assert_eq!(credit.amount, Amount::from_cents(20_000));
        assert_eq!(debit.mode, Mode::Standard(StandardMode::VirementEmis));
        assert_eq!(credit.mode, Mode::Standard(StandardMode::VirementRecu));
        assert_eq!(debit.details, "épargne juin");
        assert_eq!((debit.uid, credit.uid), (13, 14));
        assert_eq!(debit.extra(SISTER_ACCOUNT_KEY), Some("uid-b"));
        assert_eq!(debit.extra(SISTER_LINE_KEY), Some("14"));
        assert_eq!(credit.extra(SISTER_ACCOUNT_KEY), Some("uid-a"));
        assert_eq!(credit.extra(SISTER_LINE_KEY), Some("13"));
        assert_eq!(find_sister(&doc, 0, 2), Some((1, 1)));
        assert_eq!(find_sister(&doc, 1, 1), Some((0, 2)));
        assert_eq!(find_sister(&doc, 0, 0), None, "pas un transfert");
    }

    #[test]
    fn transfer_errors() {
        let mut doc = sample();
        let before = doc.clone();
        let mut request = TransferRequest {
            date: Date(45000),
            kind: TransferKind::Cash,
            details: String::new(),
            category: None,
            amount: Amount::ZERO,
            from: 0,
            to: 1,
        };
        assert_eq!(
            create_transfer(&mut doc, &request),
            Err(SplitError::ZeroAmount)
        );
        request.amount = Amount::from_cents(100);
        request.to = 0;
        assert_eq!(
            create_transfer(&mut doc, &request),
            Err(SplitError::SameAccount)
        );
        request.to = 7;
        assert_eq!(
            create_transfer(&mut doc, &request),
            Err(SplitError::AccountNotFound)
        );
        assert_eq!(doc, before);
    }

    #[test]
    fn convert_line_to_transfer() {
        let mut doc = sample();
        let lines = convert_to_transfer(&mut doc, 0, 1, 1).unwrap();
        assert_eq!(
            lines,
            TransferLines {
                first: (0, 1),
                second: (1, 1)
            }
        );
        let current = &doc.accounts[0].operations[1];
        let mirror = &doc.accounts[1].operations[1];
        assert_eq!(current.payee, "Transfert vers le compte Livret A");
        assert_eq!(current.details, "Épargne, mai");
        assert_eq!(current.amount, Amount::from_cents(-50_000));
        assert_eq!(mirror.payee, "Transfert depuis le compte Courant");
        assert_eq!(mirror.details, "Épargne, mai");
        assert_eq!(mirror.mode, Mode::Standard(StandardMode::VirementRecu));
        assert_eq!(mirror.amount, Amount::from_cents(50_000));
        assert_eq!(mirror.category, Some(4));
        assert_eq!(mirror.date, current.date);
        assert_eq!(mirror.tack, Tack::None);
        assert!(amount_matches_mode(&doc, mirror));
        assert_eq!(find_sister(&doc, 0, 1), Some((1, 1)));
        assert_eq!(find_sister(&doc, 1, 1), Some((0, 1)));
        // Déjà un transfert.
        assert_eq!(
            convert_to_transfer(&mut doc, 0, 1, 1),
            Err(SplitError::AlreadyTransfer)
        );
    }

    #[test]
    fn convert_errors_leave_document_untouched() {
        let mut doc = sample();
        let before = doc.clone();
        // Carte : non convertible ; rien ne doit être écrit (bogue d'origine).
        assert_eq!(
            convert_to_transfer(&mut doc, 0, 0, 1),
            Err(SplitError::ModeNotConvertible)
        );
        assert_eq!(
            convert_to_transfer(&mut doc, 0, 1, 0),
            Err(SplitError::SameAccount)
        );
        assert_eq!(
            convert_to_transfer(&mut doc, 0, 5, 1),
            Err(SplitError::LineNotFound)
        );
        assert_eq!(
            convert_to_transfer(&mut doc, 0, 1, 3),
            Err(SplitError::AccountNotFound)
        );
        assert_eq!(doc, before);
    }

    #[test]
    fn sister_fallbacks() {
        let mut doc = sample();
        let date = Date::from_ymd(2024, 7, 1).unwrap();
        let mut out = op(20, (2024, 7, 1), -1_000);
        out.payee = "Transfert vers le compte livret a".into();
        let mut decoy = op(21, (2024, 7, 1), 2_000);
        decoy.payee = "Transfert depuis le compte Courant".into();
        let mut inbound = op(22, (2024, 7, 1), 1_000);
        inbound.payee = "Transfert depuis le compte Courant".into();
        assert_eq!(inbound.date, date);
        doc.accounts[0].operations.push(out);
        doc.accounts[1].operations.push(decoy);
        doc.accounts[1].operations.push(inbound);
        // Sans champs supplémentaires : nom (casse ignorée), date, montant opposé.
        assert_eq!(find_sister(&doc, 0, 2), Some((1, 2)));
        assert_eq!(find_sister(&doc, 1, 2), Some((0, 2)));
        // Lien par UId prioritaire, même si le nom du compte a changé.
        doc.accounts[1].name = "Livret renommé".into();
        let out = &mut doc.accounts[0].operations[2];
        set_extra(out, SISTER_ACCOUNT_KEY, "uid-b");
        set_extra(out, SISTER_LINE_KEY, "21");
        assert_eq!(find_sister(&doc, 0, 2), Some((1, 1)));
    }

    #[test]
    fn extra_values() {
        let mut o = op(1, (2024, 1, 1), 100);
        assert!(set_extra(&mut o, "note", "a"));
        assert!(set_extra(&mut o, "NOTE", "b"));
        assert_eq!(o.extra, [("note".to_string(), "b".to_string())]);
        assert!(!set_extra(&mut o, "UId", "5"));
        assert!(set_extra(&mut o, "note", ""));
        assert!(o.extra.is_empty());
    }
}
