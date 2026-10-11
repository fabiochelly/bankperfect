//! Ouverture et enregistrement des fichiers `.bp`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::format_v6::{self, Protection, Unlock};
use crate::model::Document;
use crate::{FileFormat, codec_v5, detect_format, v5_text};

/// Révision du contenu sérialisé à l'intérieur du conteneur v6.
const PAYLOAD_REVISION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Payload {
    revision: u32,
    document: Document,
}

/// Fichier ouvert, prêt à être affiché puis réenregistré.
pub struct OpenedFile {
    pub document: Document,
    pub format: FileFormat,
    /// Protection v6 à réutiliser à l'enregistrement. `None` pour un fichier v5 :
    /// l'application doit en choisir une au premier enregistrement (voir
    /// [`OpenedFile::legacy_password`]).
    pub protection: Option<Protection>,
    /// Mot de passe d'un fichier v5, à reprendre comme mot de passe v6.
    pub legacy_password: Option<String>,
    /// Anomalies corrigées à la lecture.
    pub warnings: Vec<String>,
}

/// Indique si l'ouverture du fichier demandera un mot de passe.
pub fn requires_password(data: &[u8]) -> Result<bool> {
    match detect_format(data)? {
        FileFormat::V6 => format_v6::requires_password(data),
        FileFormat::V5 => Ok(!v5_text::parse(&codec_v5::decode(data)?)?
            .password
            .is_empty()),
    }
}

/// Ouvre un fichier v5 ou v6.
pub fn open(path: &Path, password: Option<&str>) -> Result<OpenedFile> {
    open_bytes(
        &std::fs::read(path)?,
        password.map_or(Unlock::None, Unlock::Password),
    )
}

/// Ouvre un fichier v6 avec sa clé de secours.
pub fn open_with_recovery_key(path: &Path, phrase: &str) -> Result<OpenedFile> {
    open_bytes(&std::fs::read(path)?, Unlock::RecoveryKey(phrase))
}

pub fn open_bytes(data: &[u8], unlock: Unlock<'_>) -> Result<OpenedFile> {
    match detect_format(data)? {
        FileFormat::V6 => {
            let (plaintext, protection) = Protection::open(data, unlock)?;
            let payload: Payload =
                rmp_serde::from_slice(&plaintext).map_err(|_| Error::Corrupted)?;
            if payload.revision > PAYLOAD_REVISION {
                return Err(Error::UnsupportedVersion(payload.revision as u8));
            }
            Ok(OpenedFile {
                document: payload.document,
                format: FileFormat::V6,
                protection: Some(protection),
                legacy_password: None,
                warnings: Vec::new(),
            })
        }
        FileFormat::V5 => {
            let parsed = v5_text::parse(&codec_v5::decode(data)?)?;
            if !parsed.password.is_empty() {
                // Comparaison insensible à la casse : l'ancienne version stockait
                // le mot de passe en minuscules. Plus de mot de passe maître.
                match unlock {
                    Unlock::Password(p) if p.to_lowercase() == parsed.password.to_lowercase() => {}
                    Unlock::Password(_) => return Err(Error::WrongPassword),
                    _ => return Err(Error::PasswordRequired),
                }
            }
            let legacy_password = match unlock {
                Unlock::Password(p) if !parsed.password.is_empty() => Some(p.to_string()),
                _ => None,
            };
            Ok(OpenedFile {
                document: parsed.document,
                format: FileFormat::V5,
                protection: None,
                legacy_password,
                warnings: parsed.warnings,
            })
        }
    }
}

/// Sérialise et chiffre un document au format v6.
pub fn to_bytes(document: &Document, protection: &Protection) -> Result<Vec<u8>> {
    let payload = PayloadRef {
        revision: PAYLOAD_REVISION,
        document,
    };
    let plaintext = rmp_serde::to_vec_named(&payload).map_err(|e| Error::Crypto(e.to_string()))?;
    protection.seal(&plaintext)
}

/// Enregistre un document au format v6, de façon atomique.
pub fn save(path: &Path, document: &Document, protection: &Protection) -> Result<()> {
    format_v6::write_atomic(path, &to_bytes(document, protection)?)
}

#[derive(Serialize)]
struct PayloadRef<'a> {
    revision: u32,
    document: &'a Document,
}
