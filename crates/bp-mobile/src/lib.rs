//! Pont entre le cœur de BankPerfect et l'application Android.
//!
//! Toute la logique reste en Rust (`bp-core`) : Kotlin affiche des états et
//! transmet des ordres. Les octets du fichier passent par Kotlin, qui seul
//! sait lire et écrire dans Dropbox, Google Drive, OneDrive… (sélecteur de
//! fichiers Android). Les fonctions peuvent bloquer (dérivation du mot de
//! passe, fusion) : elles sont appelées depuis un fil de travail.
//!
//! Synchronisation : la session garde la version du fichier lue ou écrite en
//! dernier (l'ancêtre commun). Quand le fichier distant a changé, il est
//! fusionné avec la version locale ([`bp_core::merge`]) : à la reprise de
//! l'application ([`Session::refresh`]) et avant chaque enregistrement
//! ([`Session::save`]).

use std::sync::{Arc, Mutex, MutexGuard};

use bp_core::FileFormat;
use bp_core::categorize::Categorizer;
use bp_core::file::{self, OpenedFile};
use bp_core::format_v6::{Protection, Unlock};
use bp_core::merge;
use bp_core::model::{Amount, Document, Tack};

mod entry;
mod view;

pub use entry::{CategoryGuess, CategoryItem, Draft, ModeItem, PayeeChoice};
pub use view::{AccountView, OperationRow, TackState, TackView};

uniffi::setup_scaffolding!();

/// Erreur transmise à Kotlin avec son message (`flat_error` : le texte
/// affiché est celui de `Display`).
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum BpError {
    #[error("Mot de passe requis")]
    PasswordRequired,
    #[error("Mot de passe incorrect")]
    WrongPassword,
    /// La clé gardée pour l'empreinte digitale n'ouvre plus le fichier (mot
    /// de passe changé depuis) : il faut le mot de passe.
    #[error("Le mot de passe de ce fichier a changé : saisissez-le")]
    KeyRejected,
    /// Fichier de BankPerfect 8 ou antérieur : à convertir sur ordinateur.
    #[error(
        "Ce fichier est au format de BankPerfect 8. Ouvrez-le et enregistrez-le une fois avec BankPerfect 9 sur ordinateur."
    )]
    OldFormat,
    /// Le fichier distant a été chiffré avec une autre clé : son mot de passe
    /// a été changé sur un autre appareil.
    #[error(
        "Ce fichier a été enregistré sur un autre appareil avec un autre mot de passe : fermez-le et rouvrez-le."
    )]
    PasswordChangedElsewhere,
    /// Saisie refusée (date, montant…) : le message dit quoi corriger.
    #[error("{reason}")]
    Invalid { reason: String },
    #[error("{reason}")]
    Failed { reason: String },
}

impl BpError {
    pub(crate) fn invalid(reason: impl Into<String>) -> Self {
        Self::Invalid {
            reason: reason.into(),
        }
    }
}

impl From<bp_core::Error> for BpError {
    fn from(error: bp_core::Error) -> Self {
        match error {
            bp_core::Error::PasswordRequired => Self::PasswordRequired,
            bp_core::Error::WrongPassword => Self::WrongPassword,
            error => Self::Failed {
                reason: error.to_string(),
            },
        }
    }
}

impl From<merge::MergeError> for BpError {
    fn from(error: merge::MergeError) -> Self {
        Self::Failed {
            reason: error.to_string(),
        }
    }
}

/// Le fichier demande-t-il un mot de passe ?
#[uniffi::export]
pub fn requires_password(data: Vec<u8>) -> Result<bool, BpError> {
    Ok(file::requires_password(&data)?)
}

/// Un compte, tel qu'affiché dans la liste des comptes.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AccountSummary {
    /// Index réel du compte dans le fichier.
    pub index: u32,
    pub name: String,
    pub bank: String,
    /// Solde, formaté (`1 234,56 €`).
    pub balance: String,
    pub negative: bool,
    /// Solde des opérations pointées ou rapprochées (« en banque »).
    pub pointed: String,
    /// Opérations à pointer.
    pub pending: u32,
    pub disabled: bool,
}

/// Résultat d'une fusion avec la version distante.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SyncReport {
    /// Le fichier distant avait changé : ses modifications sont intégrées.
    pub incoming: bool,
    /// Points modifiés des deux côtés (la version de cet appareil est gardée).
    pub conflicts: Vec<String>,
}

/// Fichier à écrire, avec le bilan de la fusion qui l'a précédé.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SaveResult {
    pub bytes: Vec<u8>,
    pub report: SyncReport,
}

/// Fichier de comptes ouvert.
#[derive(uniffi::Object)]
pub struct Session {
    inner: Mutex<Inner>,
}

pub(crate) struct Inner {
    pub(crate) document: Document,
    /// Version du fichier lue ou écrite en dernier (ancêtre commun).
    base: Document,
    base_hash: [u8; 32],
    protection: Protection,
    dirty: bool,
    /// Incrémenté à chaque modification du document (caches).
    pub(crate) revision: u64,
    /// Catégorisation automatique, construite à la première proposition.
    pub(crate) categorizer: Option<(u64, Categorizer)>,
    /// Textes de recherche d'un compte : (compte, révision, texte normalisé
    /// de chaque opération).
    pub(crate) search: Option<(usize, u64, Vec<String>)>,
    /// Enregistrement préparé par [`Session::save`], en attente de
    /// [`Session::saved`] : document écrit et empreinte des octets.
    pending: Option<(Document, [u8; 32])>,
}

impl Inner {
    fn new(opened: OpenedFile, data: &[u8]) -> Result<Self, BpError> {
        let protection = opened.protection.ok_or(BpError::OldFormat)?;
        Ok(Self {
            base: opened.document.clone(),
            document: opened.document,
            base_hash: file::fingerprint(data),
            protection,
            dirty: false,
            revision: 0,
            categorizer: None,
            search: None,
            pending: None,
        })
    }

    /// Le document vient d'être modifié ici.
    pub(crate) fn changed(&mut self) {
        self.dirty = true;
        self.revision += 1;
    }

    /// Intègre la version distante `remote` si elle a changé.
    fn integrate(&mut self, remote: &[u8]) -> Result<SyncReport, BpError> {
        let hash = file::fingerprint(remote);
        if hash == self.base_hash {
            return Ok(SyncReport {
                incoming: false,
                conflicts: Vec::new(),
            });
        }
        let theirs = file::reopen_bytes(remote, &self.protection)?
            .ok_or(BpError::PasswordChangedElsewhere)?;
        let merged = merge::merge(&self.base, &self.document, &theirs)?;
        self.dirty = merged.document != theirs;
        if merged.document != self.document {
            self.document = merged.document;
            self.revision += 1;
        }
        self.base = theirs;
        self.base_hash = hash;
        Ok(SyncReport {
            incoming: true,
            conflicts: merged.conflicts.iter().map(|c| c.describe()).collect(),
        })
    }

    /// Index réel d'un compte, vérifié.
    pub(crate) fn account(&self, index: u32) -> Result<usize, BpError> {
        let index = index as usize;
        if index < self.document.accounts.len() {
            Ok(index)
        } else {
            Err(BpError::invalid("Compte introuvable"))
        }
    }

    /// Compte et position d'une opération, par son identifiant.
    pub(crate) fn find(&self, uid: u64) -> Result<(usize, usize), BpError> {
        self.document
            .accounts
            .iter()
            .enumerate()
            .find_map(|(a, account)| {
                account
                    .operations
                    .iter()
                    .position(|op| op.uid == uid)
                    .map(|i| (a, i))
            })
            .ok_or_else(|| BpError::invalid("Opération introuvable (supprimée ailleurs ?)"))
    }
}

#[uniffi::export]
impl Session {
    /// Ouvre un fichier v6 (`password` : `None` s'il n'en demande pas).
    #[uniffi::constructor]
    pub fn open(data: Vec<u8>, password: Option<String>) -> Result<Arc<Self>, BpError> {
        if bp_core::detect_format(&data)? == FileFormat::V5 {
            return Err(BpError::OldFormat);
        }
        let unlock = password.as_deref().map_or(Unlock::None, Unlock::Password);
        let opened = file::open_bytes(&data, unlock)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner::new(opened, &data)?),
        }))
    }

    /// Ouvre un fichier avec la clé gardée par le coffre-fort du téléphone
    /// ([`Session::data_key`]), sans dériver le mot de passe.
    #[uniffi::constructor]
    pub fn open_with_key(data: Vec<u8>, key: Vec<u8>) -> Result<Arc<Self>, BpError> {
        if bp_core::detect_format(&data)? == FileFormat::V5 {
            return Err(BpError::OldFormat);
        }
        let opened = file::open_with_data_key(&data, &key)?.ok_or(BpError::KeyRejected)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner::new(opened, &data)?),
        }))
    }

    /// Clé du fichier à confier au coffre-fort du téléphone (déverrouillage
    /// par empreinte digitale). `None` pour un fichier sans mot de passe.
    pub fn data_key(&self) -> Option<Vec<u8>> {
        let inner = self.lock();
        inner
            .protection
            .is_password_protected()
            .then(|| inner.protection.data_key().to_vec())
    }

    /// Nom du groupe de comptes.
    pub fn title(&self) -> String {
        self.lock().document.group_name.clone()
    }

    /// Comptes dans l'ordre d'affichage.
    pub fn accounts(&self) -> Vec<AccountSummary> {
        let inner = self.lock();
        let document = &inner.document;
        let mut order: Vec<usize> = document
            .account_order
            .iter()
            .copied()
            .filter(|&i| i < document.accounts.len())
            .collect();
        for i in 0..document.accounts.len() {
            if !order.contains(&i) {
                order.push(i);
            }
        }
        order
            .into_iter()
            .map(|i| {
                let account = &document.accounts[i];
                let balance = account.balance();
                let pointed: Amount = account
                    .operations
                    .iter()
                    .filter(|op| op.tack != Tack::None)
                    .map(|op| op.amount)
                    .sum();
                AccountSummary {
                    index: i as u32,
                    name: account.name.clone(),
                    bank: account.bank_name.clone(),
                    balance: money(balance, &account.currency),
                    negative: balance.is_negative(),
                    pointed: money(pointed, &account.currency),
                    pending: account
                        .operations
                        .iter()
                        .filter(|op| op.tack == Tack::None)
                        .count() as u32,
                    disabled: account.disabled,
                }
            })
            .collect()
    }

    /// Des modifications restent à enregistrer.
    pub fn is_dirty(&self) -> bool {
        self.lock().dirty
    }

    /// Intègre la version distante si elle a changé depuis la dernière
    /// lecture ou écriture (à la reprise de l'application).
    pub fn refresh(&self, remote: Vec<u8>) -> Result<SyncReport, BpError> {
        self.lock().integrate(&remote)
    }

    /// Prépare l'enregistrement : fusionne d'abord la version distante
    /// `remote` (absente si le fichier a disparu), puis renvoie les octets à
    /// écrire. Une fois écrits, appeler [`Session::saved`].
    pub fn save(&self, remote: Option<Vec<u8>>) -> Result<SaveResult, BpError> {
        let mut inner = self.lock();
        let report = match remote {
            Some(remote) => inner.integrate(&remote)?,
            None => SyncReport {
                incoming: false,
                conflicts: Vec::new(),
            },
        };
        let bytes = file::to_bytes(&inner.document, &inner.protection)?;
        inner.pending = Some((inner.document.clone(), file::fingerprint(&bytes)));
        Ok(SaveResult { bytes, report })
    }

    /// Les octets renvoyés par [`Session::save`] sont écrits : ils deviennent
    /// la version commune.
    pub fn saved(&self) {
        let mut inner = self.lock();
        if let Some((document, hash)) = inner.pending.take() {
            inner.dirty = inner.document != document;
            inner.base = document;
            inner.base_hash = hash;
        }
    }
}

impl Session {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner> {
        // Une panique pendant une opération ne doit pas rendre le fichier
        // inaccessible : l'état reste cohérent (chaque étape le remplace d'un
        // bloc).
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Montant avec séparateur de milliers et monnaie : `-1 234,56 €`.
pub(crate) fn money(amount: Amount, currency: &str) -> String {
    let text = amount.format(2, ',');
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text.as_str()),
    };
    let (int, frac) = digits.split_once(',').unwrap_or((digits, "00"));
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            grouped.push('\u{202F}');
        }
        grouped.push(c);
    }
    let symbol = match currency {
        "" | "EUR" => "€",
        other => other,
    };
    format!("{sign}{grouped},{frac}\u{00A0}{symbol}")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bp_core::model::Operation;

    pub(crate) fn file(document: &Document, protection: &Protection) -> Vec<u8> {
        file::to_bytes(document, protection).unwrap()
    }

    pub(crate) fn sample() -> (Document, Protection) {
        let mut document = bp_core::manage::new_document("Comptes", Amount::from_cents(10_000));
        document.guid = "{TEST}".into();
        (document, Protection::none().unwrap())
    }

    pub(crate) fn add(document: &mut Document, payee: &str, cents: i64) {
        let mut op: Operation = document.accounts[0].operations[0].clone();
        op.uid = document.next_operation_uid();
        op.payee = payee.into();
        op.amount = Amount::from_cents(cents);
        op.tack = Tack::None;
        document.accounts[0].operations.push(op);
    }

    #[test]
    fn money_format() {
        assert_eq!(
            money(Amount::from_cents(123_456), "€"),
            "1\u{202F}234,56\u{A0}€"
        );
        assert_eq!(money(Amount::from_cents(-5), "EUR"), "-0,05\u{A0}€");
        assert_eq!(
            money(Amount::from_cents(100_000_000), "$"),
            "1\u{202F}000\u{202F}000,00\u{A0}$"
        );
    }

    #[test]
    fn open_and_list_accounts() {
        let (document, protection) = sample();
        let session = Session::open(file(&document, &protection), None).unwrap();
        let accounts = session.accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].balance, "100,00\u{A0}€");
        assert!(!session.is_dirty());
        assert!(session.data_key().is_none(), "pas de mot de passe");
    }

    #[test]
    fn password_and_data_key() {
        let (document, _) = sample();
        let (protection, _) = Protection::with_password("motdepasse").unwrap();
        let data = file(&document, &protection);
        assert!(requires_password(data.clone()).unwrap());
        assert!(matches!(
            Session::open(data.clone(), None),
            Err(BpError::PasswordRequired)
        ));
        assert!(matches!(
            Session::open(data.clone(), Some("autre".into())),
            Err(BpError::WrongPassword)
        ));
        let session = Session::open(data.clone(), Some("motdepasse".into())).unwrap();
        let key = session.data_key().expect("fichier protégé");
        let again = Session::open_with_key(data.clone(), key).unwrap();
        assert_eq!(again.accounts().len(), 1);
        assert!(matches!(
            Session::open_with_key(data, vec![7; 32]),
            Err(BpError::KeyRejected)
        ));
    }

    #[test]
    fn save_merges_the_remote_version() {
        let (document, protection) = sample();
        let session = Session::open(file(&document, &protection), None).unwrap();
        // Saisie sur le téléphone.
        add(&mut session.lock().document, "Boulangerie", -350);
        session.lock().changed();
        // Pendant ce temps, l'ordinateur a enregistré une autre opération.
        let mut remote = document.clone();
        add(&mut remote, "Pharmacie", -1290);
        let result = session.save(Some(file(&remote, &protection))).unwrap();
        assert!(result.report.incoming);
        session.saved();
        assert!(!session.is_dirty());
        let written = file::open_bytes(&result.bytes, Unlock::None)
            .unwrap()
            .document;
        let payees: Vec<&str> = written.accounts[0]
            .operations
            .iter()
            .map(|o| o.payee.as_str())
            .collect();
        assert!(payees.contains(&"Boulangerie") && payees.contains(&"Pharmacie"));
        // Relire ce qu'on vient d'écrire ne change rien.
        assert!(!session.refresh(result.bytes).unwrap().incoming);
    }
}
