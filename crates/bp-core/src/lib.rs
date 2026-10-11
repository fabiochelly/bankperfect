//! Cœur de BankPerfect : formats de fichiers, modèle de données et calculs.

pub mod autofields;
pub mod autofilters;
pub mod backup;
pub mod budget;
pub mod calc;
pub mod codec_v5;
pub mod currencies;
pub mod edit;
pub mod error;
pub mod export;
pub mod feature_store;
pub mod file;
pub mod filter;
pub mod format_v6;
pub mod formula;
pub mod import;
pub mod legacy_pickle;
pub mod manage;
pub mod model;
pub mod pyvalue;
pub mod quotes;
pub mod reports;
pub mod scheduler;
pub mod split;
pub mod v5_text;
pub mod view;

pub use error::{Error, Result};

/// Format d'un fichier `.bp` sur disque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Format historique de BankPerfect 5 à 8 (lecture seule).
    V5,
    /// Nouveau format chiffré.
    V6,
}

/// Détecte le format d'un fichier `.bp` d'après son contenu.
pub fn detect_format(data: &[u8]) -> Result<FileFormat> {
    if format_v6::is_v6(data) {
        Ok(FileFormat::V6)
    } else if codec_v5::is_v5(data) {
        Ok(FileFormat::V5)
    } else {
        Err(Error::UnknownFormat)
    }
}
