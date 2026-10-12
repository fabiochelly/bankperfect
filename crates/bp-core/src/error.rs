use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("erreur d'entrée/sortie : {0}")]
    Io(#[from] io::Error),

    #[error("fichier BankPerfect non reconnu")]
    UnknownFormat,

    #[error("fichier v5 non reconnu (en-tête BP-FILE-5 absent après décodage)")]
    InvalidV5,

    #[error("version du format v6 non prise en charge : {0}")]
    UnsupportedVersion(u8),

    #[error("ce fichier est protégé par un mot de passe")]
    PasswordRequired,

    #[error("mot de passe ou clé de secours incorrect")]
    WrongPassword,

    #[error("clé de secours invalide : {0}")]
    InvalidRecoveryKey(String),

    #[error("fichier corrompu ou modifié")]
    Corrupted,

    #[error("erreur cryptographique : {0}")]
    Crypto(String),

    #[error("export JSON : {0}")]
    Json(String),

    #[error("donnée « {key} » illisible : {message}")]
    InvalidFeatureData { key: String, message: String },
}

pub type Result<T> = std::result::Result<T, Error>;
