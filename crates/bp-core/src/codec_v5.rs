//! Lecture du format historique v5 (`BP-FILE-5`).
//!
//! Portage de `LoadFileV5` / `DecryptCodec5` (codec/UnitMain.pas, unitMain.pas).
//! Le codec est une substitution polyalphabétique : chaque caractère de
//! l'alphabet `CHARS` est décalé de `DELTAS[i mod 100] + longueur` (mod 77),
//! `i` étant la position 1-based de l'octet. Les autres octets (fins de ligne,
//! accents CP1252…) sont laissés tels quels.
//!
//! Ce format n'est plus écrit : la sauvegarde se fait en v6.

use crate::error::{Error, Result};

const HEADER: &[u8] = b"BP-FILE-5";

const CHARS: &[u8; 77] =
    b"_8lL6 ,BZu2jp-ND0<(CPeE1AQyFTMG;[3Rxk4H)qdn.JVz7obW]:=OgKXIf>rmsYt+5ahicwSvU9";

const DELTAS: [i64; 100] = [
    241, 208, 211, 321, 243, 266, 267, 159, 305, 22, 223, 23, 6, 152, 120, 396, 215, 376, 297, 2,
    101, 185, 16, 2, 80, 116, 291, 147, 314, 5, 47, 184, 32, 223, 16, 74, 396, 61, 344, 362, 278,
    330, 341, 65, 55, 199, 276, 229, 139, 157, 377, 116, 227, 361, 262, 67, 154, 243, 98, 127,
    230, 351, 98, 219, 262, 293, 223, 182, 385, 287, 18, 112, 154, 272, 193, 115, 98, 393, 16,
    136, 41, 271, 318, 345, 47, 82, 165, 79, 356, 129, 112, 296, 326, 190, 5, 46, 249, 13, 367,
    44,
];

/// Position de chaque octet dans `CHARS`, ou -1 s'il n'en fait pas partie.
const POSITIONS: [i8; 256] = {
    let mut table = [-1i8; 256];
    let mut i = 0;
    while i < CHARS.len() {
        table[CHARS[i] as usize] = i as i8;
        i += 1;
    }
    table
};

/// Indique si `raw` ressemble à un fichier v5 (sans garantie : le v5 n'a pas
/// d'en-tête en clair, il faut décoder pour le savoir).
pub fn is_v5(raw: &[u8]) -> bool {
    decode_with_line_fallback(raw).is_some()
}

/// Décode un fichier v5 et renvoie son contenu texte (UTF-8), sans l'en-tête
/// `BP-FILE-5`.
pub fn decode(raw: &[u8]) -> Result<String> {
    let bytes = decode_with_line_fallback(raw).ok_or(Error::InvalidV5)?;
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes[HEADER.len()..]);
    Ok(text.into_owned())
}

/// Reproduit `LoadFileV5` : la longueur fait partie de la clé, donc si le
/// fichier a changé de fins de ligne (CRLF ↔ LF) depuis son écriture, on
/// retente avec l'autre convention.
fn decode_with_line_fallback(raw: &[u8]) -> Option<Vec<u8>> {
    let decoded = decrypt(raw);
    if decoded.starts_with(HEADER) {
        return Some(decoded);
    }
    let converted = if line_ending(&decoded) == b"\n" {
        replace(raw, b"\n", b"\r\n")
    } else {
        replace(raw, line_ending(&decoded), b"\n")
    };
    let decoded = decrypt(&converted);
    decoded.starts_with(HEADER).then_some(decoded)
}

fn decrypt(raw: &[u8]) -> Vec<u8> {
    let len = raw.len() as i64;
    raw.iter()
        .enumerate()
        .map(|(index, &byte)| match POSITIONS[byte as usize] {
            -1 => byte,
            position => {
                let delta = DELTAS[(index + 1) % 100];
                CHARS[(position as i64 - delta - len).rem_euclid(77) as usize]
            }
        })
        .collect()
}

/// Équivalent de `GetCRLF`.
fn line_ending(s: &[u8]) -> &'static [u8] {
    if s.windows(2).any(|w| w == b"\r\n") {
        b"\r\n"
    } else if s.contains(&b'\r') {
        b"\r"
    } else {
        b"\n"
    }
}

fn replace(source: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(source.len() + source.len() / 16);
    let mut rest = source;
    while let Some(pos) = rest.windows(from.len()).position(|w| w == from) {
        out.extend_from_slice(&rest[..pos]);
        out.extend_from_slice(to);
        rest = &rest[pos + from.len()..];
    }
    out.extend_from_slice(rest);
    out
}

#[cfg(test)]
pub(crate) fn encode(text: &str) -> Vec<u8> {
    // Portage d'`EncryptCodec5`, utile uniquement pour les tests.
    let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(text);
    let mut plain = HEADER.to_vec();
    plain.extend_from_slice(&replace(&bytes, b"\r\n", b"\n"));
    let len = plain.len() as i64;
    plain
        .iter()
        .enumerate()
        .map(|(index, &byte)| match POSITIONS[byte as usize] {
            -1 => byte,
            position => {
                let delta = DELTAS[(index + 1) % 100];
                CHARS[(position as i64 + delta + len).rem_euclid(77) as usize]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let text = "utilisateur=Fabio\ncompte0=Société Générale;;;Courant;€\n";
        let encoded = encode(text);
        assert!(!encoded.starts_with(HEADER));
        assert_eq!(decode(&encoded).unwrap(), text);
    }

    #[test]
    fn survives_crlf_conversion() {
        let text = "utilisateur=Fabio\ncompte0=Test\nmodes=-Carte\n";
        let converted = replace(&encode(text), b"\n", b"\r\n");
        assert_eq!(decode(&converted).unwrap(), text);
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(decode(b"hello world"), Err(Error::InvalidV5)));
    }
}
