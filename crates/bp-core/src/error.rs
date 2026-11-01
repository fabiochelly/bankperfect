use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::tr;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    Io(#[from] io::Error),

    /// Écriture de `path` refusée par le système.
    WriteDenied {
        path: PathBuf,
        read_only: bool,
        cause: io::Error,
    },

    UnknownFormat,

    InvalidV5,

    UnsupportedVersion(u8),

    PasswordRequired,

    WrongPassword,

    InvalidRecoveryKey(String),

    Corrupted,

    Crypto(String),

    Json(String),

    InvalidFeatureData {
        key: String,
        message: String,
    },
}

impl Error {
    /// Précise une écriture refusée sur `path` : le message dira où et quoi
    /// faire, plutôt qu'un simple « accès refusé ».
    pub fn on_write(self, path: &Path) -> Self {
        match self {
            Self::Io(cause) if cause.kind() == io::ErrorKind::PermissionDenied => {
                Self::WriteDenied {
                    path: path.to_path_buf(),
                    read_only: path.metadata().is_ok_and(|m| m.permissions().readonly()),
                    cause,
                }
            }
            other => other,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => f.write_str(&tr!("erreur d'entrée/sortie : {}", e)),
            Self::WriteDenied {
                path,
                read_only: true,
                cause,
            } => f.write_str(&tr!(
                "« {path} » est en lecture seule ({cause}). Retirez cet attribut dans les propriétés du fichier, ou choisissez un autre emplacement avec Fichier > Enregistrer sous.",
                path = path.display(),
                cause = cause
            )),
            Self::WriteDenied { path, cause, .. } => {
                let folder = path.parent().unwrap_or(path);
                f.write_str(&tr!(
                    "BankPerfect n'a pas le droit d'écrire dans le dossier « {folder} » ({cause}). Choisissez un autre emplacement avec Fichier > Enregistrer sous.",
                    folder = folder.display(),
                    cause = cause
                ))?;
                if cfg!(windows) {
                    f.write_str(tr!(
                        " Si ce dossier est dans Documents ou sur le Bureau, autorisez BankPerfect dans Sécurité Windows > Protection contre les ransomwares."
                    ))?;
                }
                Ok(())
            }
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
