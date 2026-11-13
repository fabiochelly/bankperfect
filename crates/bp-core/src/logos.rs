//! Logos des comptes : une image choisie par l'utilisateur pour chaque
//! compte, rangée dans le fichier de comptes (elle le suit d'une machine à
//! l'autre, chiffrée comme le reste).
//!
//! L'application réduit l'image et la convertit en PNG avant de la confier
//! à ce module, qui ne fait que la ranger par `Account::uid`. Aucun logo
//! n'est fourni avec BankPerfect.

use serde::{Deserialize, Serialize};

use crate::feature_store::{self, FeatureError};
use crate::model::Document;

/// Clé des logos dans le fichier de comptes.
pub const FEATURE_KEY: &str = "logos.v6";

/// Logo d'un compte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountLogo {
    /// `Account::uid` du compte.
    pub account: String,
    /// Image au format PNG.
    #[serde(with = "serde_bytes")]
    pub png: Vec<u8>,
}

/// Logos enregistrés dans le fichier de comptes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Logos {
    #[serde(default)]
    pub logos: Vec<AccountLogo>,
}

impl Logos {
    pub fn load(document: &Document) -> Self {
        feature_store::get(document, FEATURE_KEY)
            .and_then(Result::ok)
            .unwrap_or_default()
    }

    /// Logo du compte `uid`.
    pub fn get(&self, uid: &str) -> Option<&[u8]> {
        self.logos
            .iter()
            .find(|l| l.account == uid)
            .map(|l| l.png.as_slice())
    }
}

/// Donne le logo `png` au compte `account` (remplace le précédent).
pub fn set(document: &mut Document, account: usize, png: Vec<u8>) -> Result<(), FeatureError> {
    let Some(uid) = document.accounts.get(account).map(|a| a.uid.clone()) else {
        return Ok(());
    };
    let mut logos = Logos::load(document);
    logos.logos.retain(|l| l.account != uid);
    logos.logos.push(AccountLogo { account: uid, png });
    save(document, logos)
}

/// Retire le logo du compte `account`.
pub fn remove(document: &mut Document, account: usize) -> Result<(), FeatureError> {
    let Some(uid) = document.accounts.get(account).map(|a| a.uid.clone()) else {
        return Ok(());
    };
    let mut logos = Logos::load(document);
    logos.logos.retain(|l| l.account != uid);
    save(document, logos)
}

/// Enregistre les logos, sans ceux des comptes supprimés ; la clé disparaît
/// quand il n'en reste aucun.
fn save(document: &mut Document, mut logos: Logos) -> Result<(), FeatureError> {
    logos
        .logos
        .retain(|l| document.accounts.iter().any(|a| a.uid == l.account));
    if logos.logos.is_empty() {
        feature_store::remove(document, FEATURE_KEY);
        Ok(())
    } else {
        feature_store::set(document, FEATURE_KEY, &logos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manage;
    use crate::model::Amount;

    fn document() -> Document {
        let mut document = manage::new_document("Courant", Amount::ZERO);
        manage::add_account(&mut document, "Livret", Amount::ZERO);
        document
    }

    #[test]
    fn a_logo_follows_its_account() {
        let mut document = document();
        set(&mut document, 0, vec![1, 2, 3]).unwrap();
        set(&mut document, 1, vec![4]).unwrap();
        set(&mut document, 0, vec![5, 6]).unwrap();
        let logos = Logos::load(&document);
        assert_eq!(logos.get(&document.accounts[0].uid), Some(&[5, 6][..]));
        assert_eq!(logos.get(&document.accounts[1].uid), Some(&[4][..]));
        assert_eq!(logos.logos.len(), 2);

        // Les comptes changent de rang, pas de logo.
        let livret = document.accounts[1].uid.clone();
        manage::delete_account(&mut document, 0);
        assert_eq!(Logos::load(&document).get(&livret), Some(&[4][..]));
    }

    #[test]
    fn removing_the_last_logo_removes_the_key() {
        let mut document = document();
        set(&mut document, 0, vec![1]).unwrap();
        set(&mut document, 1, vec![2]).unwrap();
        // Le logo d'un compte supprimé part au prochain enregistrement.
        manage::delete_account(&mut document, 0);
        remove(&mut document, 5).unwrap();
        assert_eq!(Logos::load(&document).logos.len(), 2);
        remove(&mut document, 0).unwrap();
        assert!(feature_store::raw(&document, FEATURE_KEY).is_none());
    }

    #[test]
    fn logos_survive_the_file_format() {
        let mut document = document();
        let png: Vec<u8> = (0..=255).collect();
        set(&mut document, 0, png.clone()).unwrap();
        let json = crate::json::to_json(&document).unwrap();
        let back = crate::json::from_json(&json).unwrap();
        assert_eq!(
            Logos::load(&back).get(&back.accounts[0].uid),
            Some(png.as_slice())
        );
    }
}
