//! Licence nominative des personnes qui ont acheté BankPerfect : elle
//! n'ouvre aucune fonction, elle affiche « Licence n° … · Nom ».
//!
//! La clé reçue à l'achat est envoyée une seule fois au serveur de licences,
//! qui la vérifie dans la base des ventes et renvoie un certificat signé en
//! Ed25519 :
//!
//! ```text
//! BP1.<charge utile JSON en base64url>.<signature en base64url>
//! ```
//!
//! Le certificat est gardé dans les réglages et vérifié hors ligne à chaque
//! démarrage avec la clé publique ci-dessous. Elle n'a rien de secret :
//! sans la clé privée, qui ne quitte pas le serveur, personne ne peut
//! fabriquer de certificat.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bp_core::tr;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;

/// Clé publique du serveur de licences.
const PUBLIC_KEY: [u8; 32] = [
    0x2c, 0x6d, 0x5d, 0xd4, 0x48, 0x40, 0x40, 0x57, 0x82, 0x31, 0x9a, 0xe2, 0xf5, 0x13, 0xf3, 0x62,
    0x99, 0x18, 0xdf, 0x4a, 0x25, 0xdc, 0xaa, 0xfd, 0xb1, 0x60, 0x6f, 0xce, 0x2e, 0x9d, 0x85, 0x76,
];

/// Page d'activation (`BP_LICENSE_URL` la remplace, pour les essais).
const ACTIVATION_URL: &str = "https://chelly.net/bankperfect/licences/check-bp-license.php";

/// Licence vérifiée.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct License {
    #[serde(rename = "n")]
    pub number: u32,
    pub name: String,
    #[serde(default)]
    pub gold: bool,
}

impl License {
    /// « Licence n° 1234 · Nom » (ou « Licence Gold n° … »).
    pub fn label(&self) -> String {
        if self.gold {
            tr!("Licence Gold n° {} · {}", self.number, self.name)
        } else {
            tr!("Licence n° {} · {}", self.number, self.name)
        }
    }
}

/// Licence contenue dans un certificat, s'il est signé par le serveur.
pub fn verify(certificate: &str) -> Option<License> {
    verify_with(certificate, &PUBLIC_KEY)
}

fn verify_with(certificate: &str, public_key: &[u8; 32]) -> Option<License> {
    let mut parts = certificate.trim().split('.');
    let (Some("BP1"), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let payload = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
    let signature = Signature::from_slice(&signature).ok()?;
    VerifyingKey::from_bytes(public_key)
        .ok()?
        .verify_strict(&payload, &signature)
        .ok()?;
    serde_json::from_slice(&payload).ok()
}

#[derive(Deserialize)]
struct Answer {
    ok: bool,
    #[serde(default)]
    certificate: String,
    #[serde(default)]
    error: String,
}

/// Envoie la clé au serveur de licences ; renvoie le certificat vérifié et
/// la licence, ou un message d'erreur à afficher. Bloquant : à appeler hors
/// du fil de l'interface.
pub fn activate(key: &str) -> Result<(String, License), String> {
    let url = std::env::var("BP_LICENSE_URL").unwrap_or_else(|_| ACTIVATION_URL.into());
    let key: String = key.split_whitespace().collect();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .build()
        .into();
    let mut response = agent
        .post(&url)
        .send_form([("key", key.as_str())])
        .map_err(|e| {
            tr!(
                "Le serveur de licences ne répond pas. Vérifiez la connexion à Internet. ({})",
                e
            )
        })?;
    let unexpected =
        || tr!("Réponse inattendue du serveur de licences. Réessayez plus tard.").to_string();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|_| unexpected())?;
    let answer: Answer = serde_json::from_str(&body).map_err(|_| unexpected())?;
    if !answer.ok {
        return Err(if answer.error.is_empty() {
            tr!("Cette clé de licence n'a pas été acceptée.").into()
        } else {
            answer.error
        });
    }
    let license = verify(&answer.certificate).ok_or_else(|| {
        tr!("Le certificat reçu du serveur de licences n'est pas valide.").to_string()
    })?;
    Ok((answer.certificate, license))
}

/// Clé saisie dans l'ancienne version (Windows : `[APPLICATION] Licence` de
/// `%APPDATA%\BankPerfect\bankperfect.ini`), pour préremplir l'activation.
pub fn legacy_key() -> Option<String> {
    let path = std::path::Path::new(&std::env::var_os("APPDATA")?)
        .join("BankPerfect")
        .join("bankperfect.ini");
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let mut in_application = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_application = line.eq_ignore_ascii_case("[application]");
        } else if in_application
            && let Some((name, value)) = line.split_once('=')
            && name.trim().eq_ignore_ascii_case("licence")
            && !value.trim().is_empty()
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn certificate(signer: &SigningKey, payload: &str) -> String {
        let signature = signer.sign(payload.as_bytes());
        format!(
            "BP1.{}.{}",
            URL_SAFE_NO_PAD.encode(payload),
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        )
    }

    #[test]
    fn verifies_signed_certificates_only() {
        let signer = SigningKey::from_bytes(&[7; 32]);
        let public = signer.verifying_key().to_bytes();
        let payload = r#"{"v":1,"n":150,"name":"Hélène Martin","gold":true,"issued":"2026-10-13"}"#;
        let good = certificate(&signer, payload);
        let license = verify_with(&good, &public).expect("certificat valide");
        assert_eq!(license.number, 150);
        assert_eq!(license.name, "Hélène Martin");
        assert!(license.gold);
        assert_eq!(license.label(), "Licence Gold n° 150 · Hélène Martin");

        // Charge utile modifiée : la signature ne correspond plus.
        let forged_payload = URL_SAFE_NO_PAD.encode(payload.replace("150", "151"));
        let mut parts: Vec<&str> = good.split('.').collect();
        parts[1] = &forged_payload;
        assert!(verify_with(&parts.join("."), &public).is_none());

        // Signé par une autre clé.
        let other = SigningKey::from_bytes(&[9; 32]);
        assert!(verify_with(&certificate(&other, payload), &public).is_none());

        // Mal formé.
        assert!(verify_with("", &public).is_none());
        assert!(verify_with("BP1.abc", &public).is_none());
        assert!(verify_with(&format!("{good}.x"), &public).is_none());
    }

    /// Certificat émis par check-bp-license.php (banc de test local, vraie
    /// clé privée du serveur).
    #[test]
    fn verifies_a_server_certificate() {
        let issued = "BP1.eyJ2IjoxLCJuIjoxMjM0LCJuYW1lIjoiSmVhbiBEdXBvbnQiLCJnb2xkIjpmYWxzZSwiaXNzdWVkIjoiMjAyNi0xMC0xMyJ9.06WMFwz6d7yvWB6nAKNlEaO-zYJ6jHGUmvlAAEvHEt-11MyX5hlTOQ608Z1Tt7LmzxFQIIAi1iOFGXKWVkOuBw";
        let license = verify(issued).expect("certificat du serveur");
        assert_eq!(license.label(), "Licence n° 1234 · Jean Dupont");
    }
}
