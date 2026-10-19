//! Catalogue anglais : textes français → traduction.
//!
//! Partie « catégorisation » : `categorize/` (origines et explications des
//! propositions, erreurs de l'IA) et `manage.rs` (textes créés avec un
//! nouveau fichier ou un nouveau compte). Les noms des catégories standard
//! sont dans `categorize/taxonomy.rs` (`name_en`).

pub const ENTRIES: &[(&str, &str)] = &[
    // categorize/mod.rs
    ("historique", "history"),
    ("mots du libellé", "description words"),
    ("code commerçant", "merchant code"),
    ("commerçant reconnu", "recognized merchant"),
    ("IA", "AI"),
    ("code commerçant {}", "merchant code {}"),
    ("« {} » reconnu", "“{}” recognized"),
    ("proposé par l'IA", "suggested by AI"),
    ("{} opération « {1} »", "{} transaction “{1}”"),
    ("{} opérations « {1} »", "{} transactions “{1}”"),
    ("mots déjà vus", "words seen before"),
    // categorize/ai.rs
    (
        "Claude Opus 5.5 (le plus fiable)",
        "Claude Opus 5.5 (most reliable)",
    ),
    (
        "Claude Haiku 5.5 (le moins cher)",
        "Claude Haiku 5.5 (cheapest)",
    ),
    ("l'IA a refusé de répondre", "the AI declined to answer"),
    ("réponse de l'IA illisible", "unreadable AI response"),
    ("réponse de l'IA incomplète", "incomplete AI response"),
    ("clé d'API refusée", "API key rejected"),
    (
        "trop de demandes, réessayez dans un instant",
        "too many requests, try again in a moment",
    ),
    ("erreur {status}", "error {status}"),
    ("erreur {status} : {detail}", "error {status}: {detail}"),
    // manage.rs
    ("Solde initial", "Opening balance"),
    ("Compte", "Account"),
    ("Nouveau mode", "New payment method"),
    ("Courant", "Checking"),
    (
        "Ajustement des arrondis de conversion",
        "Currency conversion rounding adjustment",
    ),
    ("Solde: {} {}", "Balance: {} {}"),
];
