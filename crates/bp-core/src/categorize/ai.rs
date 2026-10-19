//! Recours à l'IA (API Claude d'Anthropic), en dernier ressort et à la
//! demande de l'utilisateur : seuls les libellés que ni l'historique ni le
//! dictionnaire ne savent classer sont envoyés, nettoyés (sans numéros,
//! références ni dates), avec le sens et l'ordre de grandeur du montant et la
//! liste des catégories du fichier. Chaque réponse est rangée dans le fichier
//! pour ne jamais redemander le même libellé.
//!
//! Ce module prépare la requête et lit la réponse ; l'appel réseau est fait
//! par l'application.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Value, json};

use super::text::{keys, significant_words};
use crate::feature_store;
use crate::i18n::{Lang, lang};
use crate::model::{Amount, Document, Operation};
use crate::{tr, tr_mark};

/// Clé des réponses rangées dans le fichier : clé du tiers → libellé de la
/// catégorie (`Parent > Enfant`), ou chaîne vide si l'IA n'a pas su.
pub const STORE_KEY: &str = "categorize.ai.v1";

pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const API_VERSION: &str = "2023-06-01";
/// Modèle par défaut ; l'utilisateur peut en choisir un moins cher.
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
/// Modèles proposés dans les options : (identifiant, libellé à passer à
/// [`crate::i18n::tr`]).
pub const MODELS: [(&str, &str); 3] = [
    (
        "claude-opus-5-5",
        tr_mark!("Claude Opus 5.5 (le plus fiable)"),
    ),
    ("claude-sonnet-5-5", "Claude Sonnet 5.5"),
    (
        "claude-haiku-5-5",
        tr_mark!("Claude Haiku 5.5 (le moins cher)"),
    ),
];
/// Libellés envoyés par requête.
pub const BATCH: usize = 60;

/// Libellé à faire classer.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Clé du tiers (celle de l'historique).
    pub key: String,
    /// Texte envoyé : mots significatifs du tiers et du détail.
    pub text: String,
    pub amount: Amount,
}

/// Réponses déjà rangées dans le fichier, résolues en catégories.
pub fn answers(document: &Document) -> HashMap<String, u32> {
    let stored: BTreeMap<String, String> = match feature_store::get(document, STORE_KEY) {
        Some(Ok(map)) => map,
        _ => return HashMap::new(),
    };
    let labels: HashMap<String, u32> = document
        .categories
        .iter()
        .filter_map(|c| document.category_label(c.id).map(|l| (l, c.id)))
        .collect();
    stored
        .into_iter()
        .filter_map(|(key, label)| labels.get(&label).map(|&id| (key, id)))
        .collect()
}

/// Clés déjà soumises (y compris celles que l'IA n'a pas su classer).
fn asked(document: &Document) -> BTreeMap<String, String> {
    match feature_store::get(document, STORE_KEY) {
        Some(Ok(map)) => map,
        _ => BTreeMap::new(),
    }
}

/// Libellés à soumettre parmi `operations` : un par tiers, jamais déjà
/// demandé.
pub fn items<'a>(
    document: &Document,
    operations: impl IntoIterator<Item = &'a Operation>,
) -> Vec<Item> {
    let asked = asked(document);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for op in operations {
        let Some((key, _)) = keys(&op.payee, &op.details) else {
            continue;
        };
        if asked.contains_key(&key) || !seen.insert(key.clone()) {
            continue;
        }
        let mut words = significant_words(&op.payee);
        words.extend(significant_words(&op.details));
        words.truncate(14);
        out.push(Item {
            key,
            text: words.join(" "),
            amount: op.amount,
        });
    }
    out
}

/// Ordre de grandeur du montant (pour ne pas transmettre la valeur exacte),
/// dans la langue de la requête (celle de l'interface). En anglais, la
/// devise n'est pas supposée.
fn magnitude(amount: Amount, lang: Lang) -> String {
    let value = amount.abs().as_f64();
    let rank = [10.0, 30.0, 100.0, 300.0, 1000.0]
        .iter()
        .position(|&limit| value < limit)
        .unwrap_or(5);
    let (sign, buckets) = match lang {
        Lang::Fr => (
            if amount.is_negative() {
                "dépense"
            } else {
                "recette"
            },
            [
                "moins de 10 €",
                "10 à 30 €",
                "30 à 100 €",
                "100 à 300 €",
                "300 à 1 000 €",
                "plus de 1 000 €",
            ],
        ),
        Lang::En => (
            if amount.is_negative() {
                "expense"
            } else {
                "income"
            },
            [
                "under 10",
                "10 to 30",
                "30 to 100",
                "100 to 300",
                "300 to 1,000",
                "over 1,000",
            ],
        ),
    };
    format!("{sign}, {}", buckets[rank])
}

fn category_labels(document: &Document) -> Vec<String> {
    document
        .categories
        .iter()
        .filter_map(|c| document.category_label(c.id))
        .collect()
}

/// Corps de la requête `POST /v1/messages` pour un lot de libellés. La
/// consigne suit la langue de l'interface : en anglais, les relevés et les
/// catégories sont a priori anglophones et le foyer n'est pas supposé
/// français.
pub fn request_body(model: &str, document: &Document, items: &[Item]) -> Value {
    let labels = category_labels(document);
    let mut choices = labels.clone();
    choices.push(String::new());
    let lang = lang();
    let lines: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{i}. {} ({})", item.text, magnitude(item.amount, lang)))
        .collect();
    let prompt = match lang {
        Lang::Fr => format!(
            "Voici des opérations du compte bancaire d'une famille française (libellés \
             bancaires nettoyés, sens et ordre de grandeur du montant). Range chacune dans \
             l'une des catégories de la liste, en recopiant exactement le libellé de la \
             catégorie. Si le libellé ne permet pas de deviner raisonnablement (nom de \
             personne sans indice, référence opaque), réponds par une chaîne vide plutôt \
             que de deviner.\n\nCatégories :\n{}\n\nOpérations :\n{}",
            labels.join("\n"),
            lines.join("\n")
        ),
        Lang::En => format!(
            "Here are transactions from a household's bank account (cleaned-up bank \
             descriptions, with the direction and order of magnitude of the amount, in the \
             account's currency). Assign each one to one of the categories in the list, \
             copying the category label exactly. If the description does not allow a \
             reasonable guess (a person's name with no other clue, an opaque reference), \
             answer with an empty string rather than guessing.\n\nCategories:\n{}\n\n\
             Transactions:\n{}",
            labels.join("\n"),
            lines.join("\n")
        ),
    };
    let mut body = json!({
        "model": model,
        "max_tokens": 4096,
        "output_config": {
            "effort": "low",
            "format": {
                "type": "json_schema",
                "schema": {
                    "type": "object",
                    "properties": {
                        "results": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "integer" },
                                    "category": { "type": "string", "enum": choices },
                                },
                                "required": ["id", "category"],
                                "additionalProperties": false,
                            },
                        },
                    },
                    "required": ["results"],
                    "additionalProperties": false,
                },
            },
        },
        "messages": [{ "role": "user", "content": prompt }],
    });
    if uses_fallbacks(model) {
        body["fallbacks"] = json!("default");
    }
    body
}

/// Les modèles Opus, Fable et Sonnet 5.5 acceptent le repli automatique sur
/// un autre modèle en cas de refus (en-tête [`FALLBACK_BETA`]).
pub fn uses_fallbacks(model: &str) -> bool {
    model.starts_with("claude-opus")
        || model.starts_with("claude-fable")
        || model == "claude-sonnet-5-5"
}

pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// Lit la réponse : (clé du tiers, libellé de catégorie ou vide) pour chaque
/// libellé du lot.
pub fn parse_response(response: &Value, items: &[Item]) -> Result<Vec<(String, String)>, String> {
    if response.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        return Err(tr!("l'IA a refusé de répondre").into());
    }
    let text: String = response
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect();
    let parsed: Value =
        serde_json::from_str(&text).map_err(|_| tr!("réponse de l'IA illisible").to_string())?;
    let results = parsed
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| tr!("réponse de l'IA incomplète").to_string())?;
    let mut out: Vec<(String, String)> = Vec::new();
    for r in results {
        let (Some(id), Some(category)) = (
            r.get("id").and_then(Value::as_u64),
            r.get("category").and_then(Value::as_str),
        ) else {
            continue;
        };
        if let Some(item) = items.get(id as usize) {
            out.push((item.key.clone(), category.to_string()));
        }
    }
    Ok(out)
}

/// Message d'erreur d'une réponse HTTP en échec.
pub fn error_message(status: u16, response: &Value) -> String {
    let detail = response
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("");
    match status {
        401 | 403 => tr!("clé d'API refusée").to_string(),
        429 => tr!("trop de demandes, réessayez dans un instant").to_string(),
        _ if detail.is_empty() => tr!("erreur {status}", status = status),
        _ => tr!(
            "erreur {status} : {detail}",
            status = status,
            detail = detail
        ),
    }
}

/// Range les réponses dans le fichier (les libellés inconnus de la liste
/// sont gardés vides : ils ne seront pas redemandés).
pub fn store(
    document: &mut Document,
    answers: &[(String, String)],
) -> Result<(), feature_store::FeatureError> {
    let labels = category_labels(document);
    let mut map = asked(document);
    for (key, label) in answers {
        let label = if labels.contains(label) {
            label.clone()
        } else {
            String::new()
        };
        map.insert(key.clone(), label);
    }
    feature_store::set(document, STORE_KEY, &map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, Date, Mode, Tack};

    fn doc() -> Document {
        let mut d = crate::manage::new_document("Test", Amount::ZERO);
        d.categories = vec![
            Category {
                id: 0,
                name: "Santé".into(),
                is_child: false,
            },
            Category {
                id: 1,
                name: "Kiné".into(),
                is_child: true,
            },
        ];
        d
    }

    fn op(payee: &str, details: &str) -> Operation {
        Operation {
            uid: 1,
            date: Date(46000),
            mark_date: Date(46000),
            tack: Tack::None,
            mode: Mode::default(),
            payee: payee.into(),
            details: details.into(),
            category: None,
            amount: Amount::from_cents(-6000),
            extra: Vec::new(),
        }
    }

    #[test]
    fn request_hides_references_and_exact_amounts() {
        let d = doc();
        let ops = [
            op("VIR INST MARTIN 12/10", "Kine REF 99887766"),
            op("VIR INST MARTIN 13/11", ""),
        ];
        let items = items(&d, ops.iter());
        assert_eq!(items.len(), 1);
        let body = request_body(DEFAULT_MODEL, &d, &items);
        let text = body["messages"][0]["content"].as_str().unwrap();
        assert!(text.contains("martin kine (dépense, 30 à 100 €)"), "{text}");
        assert!(!text.contains("99887766") && !text.contains("60"));
        assert!(text.contains("Santé > Kiné"));
        assert_eq!(body["fallbacks"], "default");
    }

    #[test]
    fn request_and_errors_in_english() {
        crate::i18n::with_lang(Lang::En, || {
            let d = doc();
            let ops = [op("ZELLE TO MARTIN", "Physio")];
            let items = items(&d, ops.iter());
            let body = request_body(DEFAULT_MODEL, &d, &items);
            let text = body["messages"][0]["content"].as_str().unwrap();
            assert!(text.starts_with("Here are transactions"), "{text}");
            assert!(
                text.contains("martin physio (expense, 30 to 100)"),
                "{text}"
            );
            assert_eq!(error_message(401, &json!({})), "API key rejected");
            assert_eq!(error_message(500, &json!({})), "error 500");
            let refused = json!({ "stop_reason": "refusal", "content": [] });
            assert_eq!(
                parse_response(&refused, &items).unwrap_err(),
                "the AI declined to answer"
            );
        });
    }

    #[test]
    fn answers_are_stored_and_not_asked_again() {
        let mut d = doc();
        let ops = [op("VIR INST MARTIN", "Kine")];
        let items = items(&d, ops.iter());
        let response = json!({
            "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": "{\"results\":[{\"id\":0,\"category\":\"Santé > Kiné\"}]}" }],
        });
        let parsed = parse_response(&response, &items).unwrap();
        store(&mut d, &parsed).unwrap();
        assert_eq!(answers(&d).get("martin"), Some(&1));
        assert!(super::items(&d, ops.iter()).is_empty());
        let refused = json!({ "stop_reason": "refusal", "content": [] });
        assert!(parse_response(&refused, &items).is_err());
    }
}
