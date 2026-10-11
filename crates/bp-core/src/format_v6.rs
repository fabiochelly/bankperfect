//! Format v6 : conteneur chiffré, rapide à écrire.
//!
//! Disposition (entiers en little-endian) :
//!
//! ```text
//! magic          8   "BPFILE6\0"
//! version        1   1
//! protection     1   0 = aucune, 1 = mot de passe
//! ── protection 0 ──
//! dek           32   clé de données en clair (simple obfuscation)
//! ── protection 1 ──
//! salt          16
//! m_kib, t, p   12   paramètres Argon2id
//! slot mdp      72   nonce (24) + DEK chiffrée par la clé dérivée du mot de passe (48)
//! slot secours  72   nonce (24) + DEK chiffrée par la clé dérivée de la clé de secours (48)
//! ── commun ──
//! codec          1   1 = zstd
//! nonce         24
//! données        …   XChaCha20-Poly1305(zstd(contenu)), AAD = tout ce qui précède
//! ```
//!
//! La dérivation Argon2id (lente, volontairement) n'a lieu qu'à l'ouverture ou
//! au changement de mot de passe. Une fois le fichier ouvert, [`Protection`]
//! garde la DEK en mémoire : chaque sauvegarde se résume à compresser,
//! chiffrer (plusieurs Go/s) et écrire.

use std::fs;
use std::io::Write;
use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use bip39::{Language, Mnemonic};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::{Error, Result};

pub const MAGIC: &[u8; 8] = b"BPFILE6\0";
const VERSION: u8 = 1;
const PROTECTION_NONE: u8 = 0;
const PROTECTION_PASSWORD: u8 = 1;
const CODEC_ZSTD: u8 = 1;
const ZSTD_LEVEL: i32 = 3;

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
const SALT_LEN: usize = 16;
const SLOT_LEN: usize = NONCE_LEN + KEY_LEN + TAG_LEN;

/// Paramètres Argon2id par défaut : 64 Mio, 3 passes (≈ 0,2–0,5 s à l'ouverture).
const ARGON2_M_KIB: u32 = 64 * 1024;
const ARGON2_T: u32 = 3;
const ARGON2_P: u32 = 1;
/// Garde-fou contre un fichier forgé qui demanderait une mémoire démesurée.
const ARGON2_MAX_M_KIB: u32 = 1024 * 1024;

const AAD_PASSWORD_SLOT: &[u8] = b"bp6-slot-password";
const AAD_RECOVERY_SLOT: &[u8] = b"bp6-slot-recovery";
const HKDF_RECOVERY_INFO: &[u8] = b"bp6-recovery-key";

type Key = Zeroizing<[u8; KEY_LEN]>;

/// Clé de secours : 24 mots (BIP-39, liste française) encodant 256 bits.
pub struct RecoveryKey(Mnemonic);

impl RecoveryKey {
    pub fn words(&self) -> Vec<&'static str> {
        self.0.words().collect()
    }

    pub fn phrase(&self) -> String {
        self.words().join(" ")
    }
}

/// Comment déverrouiller un fichier v6 protégé.
pub enum Unlock<'a> {
    None,
    Password(&'a str),
    RecoveryKey(&'a str),
}

/// Protection d'un document ouvert : la DEK et l'en-tête prêts à être réutilisés
/// à chaque sauvegarde.
pub struct Protection {
    dek: Key,
    /// En-tête de `magic` jusqu'à la fin des slots (sans codec ni nonce).
    header: Vec<u8>,
}

impl Protection {
    /// Fichier sans mot de passe : chiffré, mais la clé est dans l'en-tête.
    pub fn none() -> Result<Self> {
        let dek = random_key()?;
        let mut header = Vec::with_capacity(MAGIC.len() + 2 + KEY_LEN);
        header.extend_from_slice(MAGIC);
        header.push(VERSION);
        header.push(PROTECTION_NONE);
        header.extend_from_slice(dek.as_ref());
        Ok(Self { dek, header })
    }

    /// Fichier protégé par mot de passe. Génère une nouvelle DEK et une
    /// nouvelle clé de secours, qu'il faut montrer à l'utilisateur.
    pub fn with_password(password: &str) -> Result<(Self, RecoveryKey)> {
        Self::with_password_params(password, ARGON2_M_KIB, ARGON2_T, ARGON2_P)
    }

    fn with_password_params(
        password: &str,
        m_kib: u32,
        t: u32,
        p: u32,
    ) -> Result<(Self, RecoveryKey)> {
        let dek = random_key()?;
        let salt: [u8; SALT_LEN] = random_bytes()?;
        let recovery_entropy = Zeroizing::new(random_bytes::<KEY_LEN>()?);
        let mnemonic = Mnemonic::from_entropy_in(Language::French, recovery_entropy.as_ref())
            .map_err(|e| Error::Crypto(e.to_string()))?;

        let password_kek = derive_password_key(password, &salt, m_kib, t, p)?;
        let recovery_kek = derive_recovery_key(recovery_entropy.as_ref(), &salt)?;

        let mut header = Vec::with_capacity(MAGIC.len() + 2 + SALT_LEN + 12 + 2 * SLOT_LEN);
        header.extend_from_slice(MAGIC);
        header.push(VERSION);
        header.push(PROTECTION_PASSWORD);
        header.extend_from_slice(&salt);
        header.extend_from_slice(&m_kib.to_le_bytes());
        header.extend_from_slice(&t.to_le_bytes());
        header.extend_from_slice(&p.to_le_bytes());
        header.extend_from_slice(&wrap_key(&password_kek, &dek, AAD_PASSWORD_SLOT, &salt)?);
        header.extend_from_slice(&wrap_key(&recovery_kek, &dek, AAD_RECOVERY_SLOT, &salt)?);

        Ok((Self { dek, header }, RecoveryKey(mnemonic)))
    }

    pub fn is_password_protected(&self) -> bool {
        self.header[MAGIC.len() + 1] == PROTECTION_PASSWORD
    }

    /// Chiffre `plaintext` en un fichier v6 complet.
    pub fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let compressed = zstd::bulk::compress(plaintext, ZSTD_LEVEL)?;
        let nonce: [u8; NONCE_LEN] = random_bytes()?;

        let mut out = Vec::with_capacity(self.header.len() + 1 + NONCE_LEN + compressed.len() + TAG_LEN);
        out.extend_from_slice(&self.header);
        out.push(CODEC_ZSTD);
        out.extend_from_slice(&nonce);

        let ciphertext = cipher(&self.dek)
            .encrypt(&XNonce::from(nonce), Payload { msg: &compressed, aad: &out })
            .map_err(|_| Error::Crypto("chiffrement impossible".into()))?;
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Ouvre un fichier v6 et renvoie son contenu avec la protection à
    /// réutiliser pour les sauvegardes suivantes.
    pub fn open(data: &[u8], unlock: Unlock<'_>) -> Result<(Vec<u8>, Self)> {
        let mut reader = Reader::new(data);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(Error::UnknownFormat);
        }
        let version = reader.u8()?;
        if version != VERSION {
            return Err(Error::UnsupportedVersion(version));
        }

        let dek = match reader.u8()? {
            PROTECTION_NONE => {
                let mut dek = Zeroizing::new([0u8; KEY_LEN]);
                dek.copy_from_slice(reader.take(KEY_LEN)?);
                dek
            }
            PROTECTION_PASSWORD => {
                let salt: [u8; SALT_LEN] = reader.array()?;
                let m_kib = reader.u32()?;
                let t = reader.u32()?;
                let p = reader.u32()?;
                let password_slot = reader.take(SLOT_LEN)?;
                let recovery_slot = reader.take(SLOT_LEN)?;
                match unlock {
                    Unlock::None => return Err(Error::PasswordRequired),
                    Unlock::Password(password) => {
                        if m_kib > ARGON2_MAX_M_KIB {
                            return Err(Error::Corrupted);
                        }
                        let kek = derive_password_key(password, &salt, m_kib, t, p)?;
                        unwrap_key(&kek, password_slot, AAD_PASSWORD_SLOT, &salt)?
                    }
                    Unlock::RecoveryKey(phrase) => {
                        let mnemonic = Mnemonic::parse_in(Language::French, normalize_phrase(phrase))
                            .map_err(|e| Error::InvalidRecoveryKey(e.to_string()))?;
                        let entropy = Zeroizing::new(mnemonic.to_entropy());
                        let kek = derive_recovery_key(&entropy, &salt)?;
                        unwrap_key(&kek, recovery_slot, AAD_RECOVERY_SLOT, &salt)?
                    }
                }
            }
            _ => return Err(Error::Corrupted),
        };

        let header_len = reader.pos;
        let codec = reader.u8()?;
        if codec != CODEC_ZSTD {
            return Err(Error::Corrupted);
        }
        let nonce: [u8; NONCE_LEN] = reader.array()?;
        let aad = &data[..reader.pos];
        let compressed = cipher(&dek)
            .decrypt(&XNonce::from(nonce), Payload { msg: &data[reader.pos..], aad })
            .map_err(|_| Error::Corrupted)?;
        let plaintext = zstd::stream::decode_all(compressed.as_slice())?;

        let header = data[..header_len].to_vec();
        Ok((plaintext, Self { dek, header }))
    }
}

/// Indique si un fichier v6 demande un mot de passe pour s'ouvrir.
pub fn requires_password(data: &[u8]) -> Result<bool> {
    if !is_v6(data) || data.len() < MAGIC.len() + 2 {
        return Err(Error::UnknownFormat);
    }
    Ok(data[MAGIC.len() + 1] == PROTECTION_PASSWORD)
}

pub fn is_v6(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

/// Écrit `bytes` de façon atomique : fichier temporaire dans le même dossier,
/// `fsync`, puis renommage. Un plantage pendant l'écriture ne peut pas
/// laisser un fichier à moitié écrit.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut tmp = tempfile_in(dir, path)?;
    let result = (|| {
        tmp.1.write_all(bytes)?;
        tmp.1.sync_all()?;
        fs::rename(&tmp.0, path)?;
        #[cfg(unix)]
        fs::File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp.0);
    }
    result
}

fn tempfile_in(dir: &Path, target: &Path) -> Result<(std::path::PathBuf, fs::File)> {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for _ in 0..16 {
        let suffix = u32::from_le_bytes(random_bytes()?);
        let tmp = dir.join(format!(".{name}.{suffix:08x}.tmp"));
        match fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::Io(std::io::Error::other("impossible de créer un fichier temporaire")))
}

fn cipher(key: &[u8; KEY_LEN]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new_from_slice(key).expect("clé de 32 octets")
}

fn derive_password_key(password: &str, salt: &[u8], m_kib: u32, t: u32, p: u32) -> Result<Key> {
    let params = Params::new(m_kib, t, p, Some(KEY_LEN)).map_err(|_| Error::Corrupted)?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|e| Error::Crypto(e.to_string()))?;
    Ok(key)
}

/// La clé de secours a 256 bits d'entropie : HKDF suffit, pas besoin d'Argon2.
fn derive_recovery_key(entropy: &[u8], salt: &[u8]) -> Result<Key> {
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    Hkdf::<Sha256>::new(Some(salt), entropy)
        .expand(HKDF_RECOVERY_INFO, key.as_mut())
        .map_err(|e| Error::Crypto(e.to_string()))?;
    Ok(key)
}

fn wrap_key(kek: &[u8; KEY_LEN], dek: &[u8; KEY_LEN], label: &[u8], salt: &[u8]) -> Result<Vec<u8>> {
    let nonce: [u8; NONCE_LEN] = random_bytes()?;
    let aad = [label, salt].concat();
    let wrapped = cipher(kek)
        .encrypt(&XNonce::from(nonce), Payload { msg: dek, aad: &aad })
        .map_err(|_| Error::Crypto("chiffrement de la clé impossible".into()))?;
    Ok([nonce.as_slice(), &wrapped].concat())
}

fn unwrap_key(kek: &[u8; KEY_LEN], slot: &[u8], label: &[u8], salt: &[u8]) -> Result<Key> {
    let (nonce, wrapped) = slot.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| Error::Corrupted)?;
    let aad = [label, salt].concat();
    let dek = Zeroizing::new(
        cipher(kek)
            .decrypt(&XNonce::from(nonce), Payload { msg: wrapped, aad: &aad })
            .map_err(|_| Error::WrongPassword)?,
    );
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    key.copy_from_slice(&dek);
    Ok(key)
}

/// Tolère majuscules, espaces multiples et retours à la ligne dans la saisie.
fn normalize_phrase(phrase: &str) -> String {
    phrase.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>().join(" ")
}

fn random_key() -> Result<Key> {
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    getrandom::fill(key.as_mut()).map_err(|e| Error::Crypto(e.to_string()))?;
    Ok(key)
}

fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|e| Error::Crypto(e.to_string()))?;
    Ok(bytes)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let slice = self.data.get(self.pos..self.pos + n).ok_or(Error::Corrupted)?;
        self.pos += n;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("longueur vérifiée"))
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paramètres Argon2 minimaux pour que les tests restent rapides.
    fn fast_password(password: &str) -> (Protection, RecoveryKey) {
        Protection::with_password_params(password, 8, 1, 1).unwrap()
    }

    const CONTENT: &[u8] = "utilisateur=Fabio\ncompte0=Société Générale;€\n".as_bytes();

    #[test]
    fn roundtrip_without_password() {
        let protection = Protection::none().unwrap();
        let file = protection.seal(CONTENT).unwrap();
        assert!(!requires_password(&file).unwrap());
        let (content, reopened) = Protection::open(&file, Unlock::None).unwrap();
        assert_eq!(content, CONTENT);
        assert!(!reopened.is_password_protected());
    }

    #[test]
    fn roundtrip_with_password() {
        let (protection, _) = fast_password("secret");
        let file = protection.seal(CONTENT).unwrap();
        assert!(requires_password(&file).unwrap());
        assert!(matches!(Protection::open(&file, Unlock::None), Err(Error::PasswordRequired)));
        assert!(matches!(Protection::open(&file, Unlock::Password("faux")), Err(Error::WrongPassword)));

        let (content, reopened) = Protection::open(&file, Unlock::Password("secret")).unwrap();
        assert_eq!(content, CONTENT);
        assert!(reopened.is_password_protected());

        // Une sauvegarde après réouverture reste lisible avec le même mot de passe.
        let resaved = reopened.seal(b"modifie").unwrap();
        let (content, _) = Protection::open(&resaved, Unlock::Password("secret")).unwrap();
        assert_eq!(content, b"modifie");
    }

    #[test]
    fn recovery_key_opens_file() {
        let (protection, recovery) = fast_password("secret");
        assert_eq!(recovery.words().len(), 24);
        let file = protection.seal(CONTENT).unwrap();
        let typed = recovery.phrase().to_uppercase().replace(' ', "  \n");
        let (content, _) = Protection::open(&file, Unlock::RecoveryKey(&typed)).unwrap();
        assert_eq!(content, CONTENT);
    }

    #[test]
    fn detects_tampering() {
        let protection = Protection::none().unwrap();
        let mut file = protection.seal(CONTENT).unwrap();
        let last = file.len() - 1;
        file[last] ^= 1;
        assert!(matches!(Protection::open(&file, Unlock::None), Err(Error::Corrupted)));
    }

    #[test]
    fn header_is_authenticated() {
        let (protection, _) = fast_password("secret");
        let mut file = protection.seal(CONTENT).unwrap();
        // Altérer les paramètres Argon2 (t) doit faire échouer l'ouverture.
        file[MAGIC.len() + 2 + SALT_LEN + 4] ^= 1;
        assert!(Protection::open(&file, Unlock::Password("secret")).is_err());
    }

    #[test]
    fn nonces_differ_between_saves() {
        let protection = Protection::none().unwrap();
        assert_ne!(protection.seal(CONTENT).unwrap(), protection.seal(CONTENT).unwrap());
    }

    #[test]
    fn atomic_write_replaces_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("compte.bp");
        write_atomic(&path, b"v1").unwrap();
        write_atomic(&path, b"v2").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"v2");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
