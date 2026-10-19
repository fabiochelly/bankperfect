//! Catalogue anglais : textes français → traduction.
//!
//! Extensions : gestionnaire et fenêtres des extensions (bp-app), messages
//! d'erreur de l'API Luau (bp-ext). Les noms de l'API (fonctions, champs,
//! valeurs attendues) restent tels quels.

pub const ENTRIES: &[(&str, &str)] = &[
    // Fenêtres des extensions (ext_ui.rs).
    ("OK", "OK"),
    ("Annuler", "Cancel"),
    ("Aucune donnée", "No data"),
    ("Valeur non reconnue", "Unrecognized value"),
    ("Choisir un compte", "Choose an account"),
    // Gestionnaire d'extensions (extensions.rs).
    ("Extensions", "Extensions"),
    ("Accès à Internet", "Internet access"),
    (
        "Réagit à l'ouverture du fichier",
        "Runs when the file is opened",
    ),
    (
        "Réagit à chaque opération ajoutée",
        "Runs on each added transaction",
    ),
    (
        "Réagit à chaque opération modifiée",
        "Runs on each edited transaction",
    ),
    (
        "Réagit à chaque opération supprimée",
        "Runs on each deleted transaction",
    ),
    (
        "Réagit avant chaque enregistrement",
        "Runs before each save",
    ),
    ("Réagit après chaque enregistrement", "Runs after each save"),
    (
        "Réagit au changement de compte",
        "Runs when the account changes",
    ),
    (
        "Réagit au changement de sélection",
        "Runs when the selection changes",
    ),
    (
        "Des scripts Luau ajoutent des commandes, des fenêtres et des automatismes. Ils tournent en bac à sable : ils n'accèdent qu'à vos comptes et, s'ils le déclarent, à Internet.",
        "Luau scripts add commands, windows and automations. They run in a sandbox: they can only access your accounts and, if they declare it, the Internet.",
    ),
    ("Installées", "Installed"),
    ("Aucune extension pour l'instant.", "No extensions yet."),
    (
        "Installez les exemples pour découvrir ce qu'elles permettent, ou déposez vos fichiers .luau dans le dossier des extensions.",
        "Install the examples to see what they can do, or drop your .luau files into the extensions folder.",
    ),
    ("Installer les exemples", "Install the examples"),
    ("Erreur au chargement", "Error while loading"),
    ("Désactivée", "Disabled"),
    ("format d'import", "import format"),
    ("automatismes", "automations"),
    ("{} commande", "{} command"),
    ("{} commandes", "{} commands"),
    ("Fermer", "Close"),
    ("Recharger", "Reload"),
    ("Ouvrir le dossier", "Open folder"),
    ("Documentation", "Documentation"),
    ("Exemples", "Examples"),
    (
        "Ajoute les extensions d'exemple qui manquent",
        "Adds the missing example extensions",
    ),
    ("version {}", "version {}"),
    ("par {}", "by {}"),
    ("Activée", "Enabled"),
    ("Commandes", "Commands"),
    ("sur la sélection", "on the selection"),
    ("Fonctionnement", "Behavior"),
    (
        "Importe « {name} » ({suffixes})",
        "Imports “{name}” ({suffixes})",
    ),
    ("Journal (print)", "Log (print)"),
    // Emplacements des commandes (bp_ext::MENUS).
    ("Fichier", "File"),
    ("Fichier › Exporter", "File › Export"),
    ("Édition", "Edit"),
    ("Affichage", "View"),
    ("Comptes", "Accounts"),
    ("Opérations", "Transactions"),
    ("Rapports", "Reports"),
    ("Outils", "Tools"),
    (
        "clic droit sur les opérations",
        "right-click on transactions",
    ),
    ("clic droit sur un compte", "right-click on an account"),
    // Moteur (bp-ext/lib.rs, prelude.luau).
    ("état d'avant manquant", "previous state missing"),
    ("Valider", "OK"),
    (
        "bp.ui.{} : impossible ici (construction d'une fenêtre ou chargement de l'extension)",
        "bp.ui.{}: not allowed here (while building a window or loading the extension)",
    ),
    ("{} attend une table", "{} expects a table"),
    (
        "mémoire insuffisante (limite des extensions atteinte)",
        "out of memory (extension limit reached)",
    ),
    (
        "l'extension a dépassé le temps maximal ({} s)",
        "the extension exceeded the time limit ({} s)",
    ),
    ("lecture impossible : {}", "cannot read: {}"),
    ("{ext} : {error}", "{ext}: {error}"),
    (
        "{ext} : un format d'import lit le fichier sans modifier les comptes",
        "{ext}: an import format reads the file without changing the accounts",
    ),
    (
        "coroutine.yield n'est pas utilisable dans une commande",
        "coroutine.yield cannot be used in a command",
    ),
    ("Confirmer", "Confirm"),
    ("demande inconnue : {}", "unknown request: {}"),
    (
        "Ouvrez d'abord un fichier de comptes.",
        "Open an accounts file first.",
    ),
    // API Luau (bp-ext/api.rs).
    (
        "seules les données simples (texte, nombres, tables, dates) se stockent",
        "only simple data (text, numbers, tables, dates) can be stored",
    ),
    (
        "cette extension nécessite BankPerfect {required} ou plus récent (version actuelle : {current})",
        "this extension requires BankPerfect {required} or later (current version: {current})",
    ),
    (
        "bp.command se déclare au chargement de l'extension",
        "bp.command must be declared while the extension loads",
    ),
    (
        "bp.command : title est obligatoire",
        "bp.command: title is required",
    ),
    (
        "bp.command : run (fonction) est obligatoire",
        "bp.command: run (function) is required",
    ),
    (
        "bp.command : needs « {} » inconnu (\"file\", \"selection\" ou \"nothing\")",
        "bp.command: unknown needs “{}” (\"file\", \"selection\" or \"nothing\")",
    ),
    (
        "bp.command : menu est un texte ou une liste de textes",
        "bp.command: menu is a string or a list of strings",
    ),
    (
        "bp.command : menu « {} » inconnu ({})",
        "bp.command: unknown menu “{}” ({})",
    ),
    (
        "bp.importer se déclare au chargement de l'extension",
        "bp.importer must be declared while the extension loads",
    ),
    (
        "évènement inconnu : « {} » ({})",
        "unknown event: “{}” ({})",
    ),
    (
        "bp.add_account : name est obligatoire",
        "bp.add_account: name is required",
    ),
    (
        "bp.filter : tack « {} » inconnu (\"none\", \"pointed\", \"reconciled\", \"marked\" ou \"all\")",
        "bp.filter: unknown tack “{}” (\"none\", \"pointed\", \"reconciled\", \"marked\" or \"all\")",
    ),
    ("{0}, {1} ou {2}", "{0}, {1} or {2}"),
    ("catégorie mère inconnue", "unknown parent category"),
    (
        "une sous-catégorie ne peut pas avoir d'enfant",
        "a subcategory cannot have children",
    ),
    ("catégorie inconnue", "unknown category"),
    ("bp.date(année, mois, jour)", "bp.date(year, month, day)"),
    ("date inexistante", "date does not exist"),
    (
        "bp.date(année, mois, jour) ou bp.date(\"jj/mm/aaaa\")",
        "bp.date(year, month, day) or bp.date(\"dd/mm/yyyy\")",
    ),
    ("JSON invalide : {}", "invalid JSON: {}"),
    (
        "accès à Internet non déclaré : ajoutez {code} dans {call}",
        "Internet access not declared: add {code} to {call}",
    ),
    (
        "adresse web invalide (http:// ou https://)",
        "invalid web address (http:// or https://)",
    ),
    ("requête impossible : {}", "request failed: {}"),
    ("réponse illisible : {}", "unreadable response: {}"),
    ("Ouvrir un fichier", "Open a file"),
    ("lecture de {} impossible : {}", "cannot read {}: {}"),
    ("Enregistrer", "Save"),
    (
        "bp.files.save : text est obligatoire",
        "bp.files.save: text is required",
    ),
    ("écriture impossible : {}", "cannot write: {}"),
    ("adresse web invalide", "invalid web address"),
    (
        "require : réservé aux extensions en dossier",
        "require: only available to folder extensions",
    ),
    (
        "require : module invalide « {} »",
        "require: invalid module “{}”",
    ),
    (
        "require : module introuvable « {} »",
        "require: module not found “{}”",
    ),
    // Dates (bp-ext/date.rs).
    (
        "une date s'additionne avec un nombre de jours",
        "a date can only be added to a number of days",
    ),
    ("une date est attendue", "a date is expected"),
    ("date invalide : « {} »", "invalid date: “{}”"),
    (
        "une date est attendue (reçu : {})",
        "a date is expected (got: {})",
    ),
    // Échéancier, budget, règles (bp-ext/features.rs).
    ("échéancier illisible : {}", "unreadable scheduler: {}"),
    (
        "unité inconnue « {} » (\"days\", \"weeks\", \"months\" ou \"years\")",
        "unknown unit “{}” (\"days\", \"weeks\", \"months\" or \"years\")",
    ),
    (
        "every : un nombre entier positif est attendu",
        "every: a positive whole number is expected",
    ),
    (
        "day : un jour de 1 à 31 est attendu",
        "day: a day from 1 to 31 is expected",
    ),
    (
        "échéance n°{} inexistante",
        "scheduled transaction #{} does not exist",
    ),
    ("budget illisible : {}", "unreadable budget: {}"),
    ("règles illisibles : {}", "unreadable rules: {}"),
    (
        "bp.add_schedule : payee est obligatoire",
        "bp.add_schedule: payee is required",
    ),
    (
        "bp.rules.add : contains est obligatoire",
        "bp.rules.add: contains is required",
    ),
    (
        "field « {} » inconnu (\"payee\", \"details\" ou \"both\")",
        "unknown field “{}” (\"payee\", \"details\" or \"both\")",
    ),
    (
        "une règle n'applique que les modes standard",
        "a rule can only set standard payment methods",
    ),
    ("règle refusée : {}", "rule rejected: {}"),
    ("règle n°{} inexistante", "rule #{} does not exist"),
    // Formats d'import (bp-ext/import.rs).
    (
        "bp.importer : name est obligatoire",
        "bp.importer: name is required",
    ),
    (
        "bp.importer : extensions est obligatoire ({}…)",
        "bp.importer: extensions is required ({}…)",
    ),
    (
        "bp.importer : extensions est vide",
        "bp.importer: extensions is empty",
    ),
    (
        "bp.importer : parse (fonction) est obligatoire",
        "bp.importer: parse (function) is required",
    ),
    (
        "parse doit renvoyer une liste d'opérations ou {}",
        "parse must return a list of transactions or {}",
    ),
    (
        "parse : un compte sans liste operations",
        "parse: an account without an operations list",
    ),
    // Comptes et opérations (bp-ext/objects.rs).
    (
        "accès au fichier impossible ici",
        "file access not allowed here",
    ),
    (
        "aucun fichier de comptes n'est ouvert",
        "no accounts file is open",
    ),
    ("montant invalide : « {} »", "invalid amount: “{}”"),
    (
        "un montant est attendu (reçu : {})",
        "an amount is expected (got: {})",
    ),
    (
        "pointage inconnu « {} » (attendu : \"none\", \"pointed\" ou \"reconciled\")",
        "unknown clearing status “{}” (expected: \"none\", \"pointed\" or \"reconciled\")",
    ),
    (
        "mode de paiement inconnu : « {} »",
        "unknown payment method: “{}”",
    ),
    ("catégorie inconnue : n°{}", "unknown category: #{}"),
    ("catégorie inconnue : « {} »", "unknown category: “{}”"),
    (
        "plusieurs catégories s'appellent « {name} » : précisez « Parent > {name} »",
        "several categories are named “{name}”: specify “Parent > {name}”",
    ),
    (
        "une catégorie est attendue (reçu : {})",
        "a category is expected (got: {})",
    ),
    (
        "ce compte appartient à un autre fichier",
        "this account belongs to another file",
    ),
    ("ce compte n'existe plus", "this account no longer exists"),
    ("un compte est attendu", "an account is expected"),
    (
        "un compte est attendu (reçu : {})",
        "an account is expected (got: {})",
    ),
    ("compte inconnu : « {} »", "unknown account: “{}”"),
    (
        "le montant (amount) est obligatoire",
        "the amount (amount) is required",
    ),
    (
        "cette opération appartient à un autre fichier",
        "this transaction belongs to another file",
    ),
    (
        "le compte de cette opération n'existe plus",
        "this transaction's account no longer exists",
    ),
    (
        "cette opération a été supprimée",
        "this transaction has been deleted",
    ),
    (
        "op:split : chaque part est une table {}",
        "op:split: each part is a table {}",
    ),
    (
        "op:split : une seule part peut omettre son montant",
        "op:split: only one part can omit its amount",
    ),
    (
        "les lignes rapprochées sont verrouillées (menu Opérations) : modification refusée",
        "reconciled lines are locked (Transactions menu): change refused",
    ),
    ("une opération est attendue", "a transaction is expected"),
    (
        "une opération est attendue (reçu : {})",
        "a transaction is expected (got: {})",
    ),
    (
        "le champ « {} » est réservé à BankPerfect",
        "the field “{}” is reserved for BankPerfect",
    ),
    ("nom de champ refusé : « {} »", "field name rejected: “{}”"),
    ("ventilation impossible : {}", "cannot split: {}"),
    // Fenêtres déclaratives (bp-ext/ui.rs).
    ("couleur inconnue : « {} »", "unknown color: “{}”"),
    ("state doit être une table", "state must be a table"),
    ("view doit être une fonction", "view must be a function"),
    (
        "on_click doit être une fonction",
        "on_click must be a function",
    ),
    (
        "élément de fenêtre sans type (utilisez bp.ui.*)",
        "window element without a type (use bp.ui.*)",
    ),
    (
        "ui.chart : style « {} » inconnu (\"line\", \"columns\", \"bars\" ou \"donut\")",
        "ui.chart: unknown style “{}” (\"line\", \"columns\", \"bars\" or \"donut\")",
    ),
    (
        "élément de fenêtre inconnu : « {} »",
        "unknown window element: “{}”",
    ),
    (
        "champ sans clé : bp.ui.input{{ \"{}\", … }}",
        "field without a key: bp.ui.input{{ \"{}\", … }}",
    ),
    ("clé", "key"),
    ("type de champ inconnu : « {} »", "unknown field type: “{}”"),
    ("Aucune ligne", "No rows"),
    (
        "cellule de tableau invalide : « {} »",
        "invalid table cell: “{}”",
    ),
];
