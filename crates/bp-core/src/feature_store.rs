//! Rangement des données des fonctionnalités (ex-scripts) dans le document.
//!
//! Chaque fonctionnalité range sa structure sous une clé versionnée de
//! [`Document::feature_data`] (`budget.v6`, `autofields.v6`…), sérialisée en
//! MessagePack avec noms de champs (comme le reste du format v6), ce qui
//! permet d'ajouter des champs facultatifs sans casser les fichiers existants.
//!
//! Les fichiers v5 contiennent à la place les pickles Python 2 des anciens
//! scripts, sous leur clé historique (`fabiobpbudget`…). Les modules de
//! fonctionnalités lisent la nouvelle clé en priorité, sinon convertissent
//! l'ancienne via un [`PyValue`](crate::pyvalue::PyValue) fourni par le lecteur
//! de pickles ; à l'enregistrement, l'ancienne clé est retirée pour qu'il ne
//! reste pas deux versions divergentes.

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::model::{Document, FeatureData};
use crate::pyvalue::{LegacyError, PyValue};

/// Erreur de lecture ou d'écriture des données d'une fonctionnalité.
#[derive(Debug, thiserror::Error)]
pub enum FeatureError {
    Decode {
        key: String,
        message: String,
    },
    Encode {
        key: String,
        message: String,
    },
    LegacyPickle {
        key: String,
    },
    Legacy {
        key: String,
        #[source]
        source: LegacyError,
    },
}

impl std::fmt::Display for FeatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::Decode { key, message } => crate::tr!(
                "données « {key} » illisibles : {message}",
                key = key,
                message = message
            ),
            Self::Encode { key, message } => crate::tr!(
                "données « {key} » impossibles à enregistrer : {message}",
                key = key,
                message = message
            ),
            Self::LegacyPickle { key } => {
                crate::tr!("ancienne donnée « {key} » : pickle illisible", key = key)
            }
            Self::Legacy { key, source } => crate::tr!(
                "ancienne donnée « {key} » : {source}",
                key = key,
                source = source
            ),
        })
    }
}

/// Octets bruts rangés sous `key`.
pub fn raw<'a>(document: &'a Document, key: &str) -> Option<&'a [u8]> {
    document
        .feature_data
        .iter()
        .find(|f| f.key == key)
        .map(|f| f.value.as_slice())
}

/// Lit la structure rangée sous `key` (`None` si la clé est absente).
pub fn get<T: DeserializeOwned>(document: &Document, key: &str) -> Option<Result<T, FeatureError>> {
    raw(document, key).map(|bytes| {
        rmp_serde::from_slice(bytes).map_err(|e| FeatureError::Decode {
            key: key.to_string(),
            message: e.to_string(),
        })
    })
}

/// Range `value` sous `key` (remplace la valeur existante).
pub fn set<T: Serialize>(
    document: &mut Document,
    key: &str,
    value: &T,
) -> Result<(), FeatureError> {
    let bytes = rmp_serde::to_vec_named(value).map_err(|e| FeatureError::Encode {
        key: key.to_string(),
        message: e.to_string(),
    })?;
    match document.feature_data.iter_mut().find(|f| f.key == key) {
        Some(entry) => entry.value = bytes,
        None => document.feature_data.push(FeatureData {
            key: key.to_string(),
            value: bytes,
        }),
    }
    Ok(())
}

/// Retire la clé `key` ; renvoie vrai si elle existait.
pub fn remove(document: &mut Document, key: &str) -> bool {
    let before = document.feature_data.len();
    document.feature_data.retain(|f| f.key != key);
    document.feature_data.len() != before
}

/// Lecture commune : nouvelle clé, sinon ancienne clé convertie, sinon `None`.
///
/// `parse_pickle` décode un pickle Python 2 (fourni par le lecteur de pickles) ;
/// `convert` transforme la valeur générique en structure native.
pub(crate) fn load_with_legacy<T: DeserializeOwned>(
    document: &Document,
    key: &str,
    legacy_key: &str,
    parse_pickle: &dyn Fn(&[u8]) -> Option<PyValue>,
    convert: impl FnOnce(&PyValue) -> Result<T, LegacyError>,
) -> Option<Result<T, FeatureError>> {
    if let Some(result) = get(document, key) {
        return Some(result);
    }
    let bytes = raw(document, legacy_key)?;
    Some(match parse_pickle(bytes) {
        None => Err(FeatureError::LegacyPickle {
            key: legacy_key.to_string(),
        }),
        Some(value) => convert(&value).map_err(|source| FeatureError::Legacy {
            key: legacy_key.to_string(),
            source,
        }),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        #[serde(default)]
        count: u32,
    }

    pub(crate) fn empty_document() -> Document {
        Document {
            group_name: String::new(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: Vec::new(),
            accounts: Vec::new(),
            account_order: Vec::new(),
            default_account: 0,
            categories: Vec::new(),
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        }
    }

    #[test]
    fn store_and_load() {
        let mut doc = empty_document();
        assert!(get::<Sample>(&doc, "x.v6").is_none());
        let value = Sample {
            name: "é".into(),
            count: 3,
        };
        set(&mut doc, "x.v6", &value).expect("sérialisable");
        set(&mut doc, "x.v6", &value).expect("remplacement");
        assert_eq!(doc.feature_data.len(), 1);
        assert_eq!(
            get::<Sample>(&doc, "x.v6").and_then(Result::ok),
            Some(value)
        );
        doc.feature_data[0].value = vec![0xc1];
        assert!(matches!(
            get::<Sample>(&doc, "x.v6"),
            Some(Err(FeatureError::Decode { .. }))
        ));
        assert!(remove(&mut doc, "x.v6"));
        assert!(!remove(&mut doc, "x.v6"));
    }

    #[test]
    fn legacy_fallback() {
        let mut doc = empty_document();
        doc.feature_data.push(FeatureData {
            key: "old".into(),
            value: b"pickle".to_vec(),
        });
        let parse = |bytes: &[u8]| (bytes == b"pickle").then(|| PyValue::Str("ancien".into()));
        let convert = |v: &PyValue| -> Result<Sample, LegacyError> {
            Ok(Sample {
                name: v.as_str().unwrap_or_default().to_string(),
                count: 0,
            })
        };
        let loaded = load_with_legacy(&doc, "new", "old", &parse, convert);
        assert_eq!(
            loaded.and_then(Result::ok).map(|s| s.name),
            Some("ancien".to_string())
        );
        let failing = |_: &[u8]| None;
        assert!(matches!(
            load_with_legacy(&doc, "new", "old", &failing, convert),
            Some(Err(FeatureError::LegacyPickle { .. }))
        ));
        assert!(load_with_legacy(&doc, "new", "absent", &parse, convert).is_none());
    }
}
