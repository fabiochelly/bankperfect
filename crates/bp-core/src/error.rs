use std::fmt;
use std::io;

use crate::tr;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    Io(#[from] io::Error),

    UnknownFormat,

    InvalidV5,

    UnsupportedVersion(u8),

    PasswordRequired,

    WrongPassword,

    InvalidRecoveryKey(String),

    Corrupted,

    Crypto(String),

    Json(String),

    InvalidFeatureData { key: String, message: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => f.write_str(&tr!("erreur d'entrée/sortie : {}", e)),
            Self::UnknownFormat => f.write_str(tr!("fichier BankPerfect non reconnu")),
            Self::InvalidV5 => f.write_str(tr!(
                "fichier v5 non reconnu (en-tête BP-FILE-5 absent après décodage)"
            )),
            Self::UnsupportedVersion(v) => {
                f.write_str(&tr!("version du format v6 non prise en charge : {}", v))
            }
            Self::PasswordRequired => {
                f.write_str(tr!("ce fichier est protégé par un mot de passe"))
            }
            Self::WrongPassword => f.write_str(tr!("mot de passe ou clé de secours incorrect")),
            Self::InvalidRecoveryKey(m) => f.write_str(&tr!("clé de secours invalide : {}", m)),
            Self::Corrupted => f.write_str(tr!("fichier corrompu ou modifié")),
            Self::Crypto(m) => f.write_str(&tr!("erreur cryptographique : {}", m)),
            Self::Json(m) => f.write_str(&tr!("export JSON : {}", m)),
            Self::InvalidFeatureData { key, message } => f.write_str(&tr!(
                "donnée « {key} » illisible : {message}",
                key = key,
                message = message
            )),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
