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

/// Ouvre un fichier v6 avec sa clé de données (voir
/// [`Protection::data_key`]). `Ok(None)` si la clé ne l'ouvre plus.
pub fn open_with_data_key(data: &[u8], key: &[u8]) -> Result<Option<OpenedFile>> {
    let Some((plaintext, protection)) = Protection::open_with_data_key(data, key)? else {
        return Ok(None);
    };
    let payload: Payload = rmp_serde::from_slice(&plaintext).map_err(|_| Error::Corrupted)?;
    if payload.revision > PAYLOAD_REVISION {
        return Err(Error::UnsupportedVersion(payload.revision as u8));
    }
    Ok(Some(OpenedFile {
        document: payload.document,
        format: FileFormat::V6,
        protection: Some(protection),
        legacy_password: None,
        warnings: Vec::new(),
    }))
}

/// Relit une autre version (v6) d'un fichier déjà ouvert, avec la clé en
/// mémoire : voir [`Protection::reopen`]. `Ok(None)` si le mot de passe a été
/// changé ailleurs.
pub fn reopen_bytes(data: &[u8], protection: &Protection) -> Result<Option<Document>> {
    let Some(plaintext) = protection.reopen(data)? else {
        return Ok(None);
    };
    let payload: Payload = rmp_serde::from_slice(&plaintext).map_err(|_| Error::Corrupted)?;
    if payload.revision > PAYLOAD_REVISION {
        return Err(Error::UnsupportedVersion(payload.revision as u8));
    }
    Ok(Some(payload.document))
}

/// Empreinte du contenu d'un fichier, pour savoir s'il a changé sur disque.
pub fn fingerprint(data: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(data).into()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopen_with_the_key_in_memory() {
        let (protection, _) = Protection::with_password_for_tests("secret");
        let document = Document {
            group_name: "ailleurs".into(),
            ..Default::default()
        };
        let bytes = to_bytes(&document, &protection).unwrap();
        let reopened = reopen_bytes(&bytes, &protection).unwrap().unwrap();
        assert_eq!(reopened.group_name, "ailleurs");

        // Mot de passe changé ailleurs : nouvelle clé.
        let (other, _) = Protection::with_password_for_tests("secret");
        let bytes = to_bytes(&document, &other).unwrap();
        assert!(reopen_bytes(&bytes, &protection).unwrap().is_none());

        // Clé de données confiée au coffre-fort : ouvre sans mot de passe,
        // et les enregistrements suivants gardent les mêmes emplacements.
        let key = protection.data_key();
        let bytes = to_bytes(&document, &protection).unwrap();
        let opened = open_with_data_key(&bytes, &key).unwrap().unwrap();
        let again = to_bytes(&opened.document, opened.protection.as_ref().unwrap()).unwrap();
        // En-tête : magic, version, protection, sel, paramètres, deux emplacements.
        let header = 8 + 1 + 1 + 16 + 12 + 2 * 72;
        assert_eq!(again[..header], bytes[..header], "même en-tête");
        assert!(open_with_data_key(&bytes, &[0; 32]).unwrap().is_none());

        // Fichier sans mot de passe : sa clé est dans l'en-tête.
        let bytes = to_bytes(&document, &Protection::none().unwrap()).unwrap();
        assert!(reopen_bytes(&bytes, &protection).unwrap().is_some());
    }
}
