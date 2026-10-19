//! Icône et couleur d'une catégorie, déduites de son nom.
//!
//! Chaque mot du libellé (la sous-catégorie d'abord, puis le parent) est
//! cherché dans une table de mots-clés français et anglais ; un mot-clé
//! reconnaît aussi les mots qui commencent par lui (« restau » → restaurant,
//! restauration…). La couleur dépend de la catégorie parente : les
//! sous-catégories d'une même famille partagent la même teinte.

use std::collections::HashMap;

use bp_core::filter::normalize;
use bp_core::model::Document;
use egui_phosphor::regular as icon;

/// (mots-clés normalisés, icône). Les entrées les plus spécifiques d'abord.
const RULES: &[(&[&str], &str)] = &[
    // Activités des enfants (avant « loisirs »).
    (
        &[
            "extrascolaire",
            "extrascolaires",
            "periscolaire",
            "extracurricular",
        ],
        icon::PUZZLE_PIECE,
    ),
    // Alimentation et restauration
    (
        &[
            "restau",
            "brasserie",
            "bistro",
            "cantine",
            "dejeuner",
            "diner",
            "resto",
        ],
        icon::FORK_KNIFE,
    ),
    (
        &["fast", "mcdo", "burger", "kebab", "snack"],
        icon::HAMBURGER,
    ),
    (&["pizza"], icon::PIZZA),
    (&["cafe", "coffee", "the", "salon"], icon::COFFEE),
    (
        &["boulang", "patiss", "pain", "viennois", "bake"],
        icon::BREAD,
    ),
    (&["vin", "cave", "alcool", "spiritueux", "wine"], icon::WINE),
    (&["biere", "bar", "pub", "beer"], icon::BEER_STEIN),
    (
        &[
            "epicerie",
            "supermarche",
            "hypermarche",
            "course",
            "grocer",
            "supermarket",
            "marche",
            "primeur",
        ],
        icon::SHOPPING_CART,
    ),
    (&["fruit", "legume", "bio"], icon::CARROT),
    (
        &["aliment", "nourriture", "food", "repas", "traiteur"],
        icon::BOWL_FOOD,
    ),
    (&["cuisine", "cooking"], icon::COOKING_POT),
    (&["gateau", "anniversaire", "cake"], icon::CAKE),
    (&["fete", "reception", "soiree", "party"], icon::CHAMPAGNE),
    // Logement
    (&["loyer", "rent", "bail", "location"], icon::HOUSE_LINE),
    (
        &["edf", "electri", "engie", "energie", "power"],
        icon::LIGHTNING,
    ),
    (
        &["gaz", "gdf", "chauffage", "fioul", "heating"],
        icon::FLAME,
    ),
    (&["eau", "water", "veolia", "suez"], icon::DROP),
    (
        &[
            "charge", "copro", "syndic", "foncier", "hoa", "condo", "strata",
        ],
        icon::BUILDINGS,
    ),
    (
        &[
            "home improvement",
            "travaux",
            "bricol",
            "renov",
            "outil",
            "quincaill",
            "leroy",
            "castorama",
        ],
        icon::HAMMER,
    ),
    (
        &[
            "reparation",
            "depannage",
            "entretien",
            "maintenance",
            "plomb",
        ],
        icon::WRENCH,
    ),
    (&["peinture", "decor", "deco"], icon::PAINT_ROLLER),
    (
        &["meuble", "mobilier", "ikea", "canape", "furniture"],
        icon::COUCH,
    ),
    (
        &["rangement", "equipement", "electromenager"],
        icon::WASHING_MACHINE,
    ),
    (&["luminaire", "lampe"], icon::LAMP),
    (&["jardin", "plante", "fleur", "garden"], icon::PLANT),
    (
        &[
            "menage",
            "nettoyage",
            "pressing",
            "laverie",
            "cleaning",
            "home services",
        ],
        icon::SPARKLE,
    ),
    (&["demenage", "moving"], icon::TRUCK),
    (
        &[
            "habitation",
            "logement",
            "maison",
            "housing",
            "home",
            "domicile",
        ],
        icon::HOUSE,
    ),
    // Communication et numérique
    (
        &[
            "mobile",
            "portable",
            "smartphone",
            "sfr",
            "orange",
            "bouygues",
            "free",
        ],
        icon::DEVICE_MOBILE,
    ),
    (&["telephone", "telephoni", "phone", "telecom"], icon::PHONE),
    (
        &["internet", "box", "fibre", "adsl", "wifi"],
        icon::WIFI_HIGH,
    ),
    (
        &[
            "informatique",
            "ordinateur",
            "computer",
            "logiciel",
            "software",
            "apple",
            "materiel",
        ],
        icon::LAPTOP,
    ),
    (
        &[
            "hifi",
            "video",
            "televi",
            "tv",
            "television",
            "audio",
            "electronic",
        ],
        icon::TELEVISION,
    ),
    (&["photo", "camera"], icon::CAMERA),
    (
        &[
            "abonnement",
            "subscription",
            "streaming",
            "netflix",
            "spotify",
            "canal",
        ],
        icon::REPEAT,
    ),
    (
        &["courrier", "poste", "timbre", "mail", "envoi"],
        icon::ENVELOPE,
    ),
    (&["colis", "livraison", "amazon", "package"], icon::PACKAGE),
    (
        &["journal", "presse", "magazine", "newspaper"],
        icon::NEWSPAPER,
    ),
    // Santé
    (&["pharmacie", "medicament", "pharma"], icon::PILL),
    (
        &[
            "medecin",
            "docteur",
            "consultation",
            "generaliste",
            "specialiste",
            "medecine",
            "doctor",
        ],
        icon::STETHOSCOPE,
    ),
    (
        &["dentiste", "dentaire", "orthodont", "dental"],
        icon::TOOTH,
    ),
    (
        &["optique", "opticien", "lunette", "lentille"],
        icon::EYEGLASSES,
    ),
    (
        &["hopital", "clinique", "hospital", "urgence"],
        icon::HOSPITAL,
    ),
    (&["vaccin", "analyse", "laboratoire", "labo"], icon::SYRINGE),
    (
        &["mutuelle", "secu", "cpam", "ameli", "health insurance"],
        icon::HEARTBEAT,
    ),
    (
        &[
            "sante", "health", "soin", "kine", "osteo", "psy", "physio", "therap",
        ],
        icon::FIRST_AID,
    ),
    // Assurances, impôts, banque (le taxi d'abord : « tax » reconnaîtrait
    // « taxi »).
    (&["taxi", "uber", "vtc", "rideshare"], icon::TAXI),
    (
        &[
            "home insurance",
            "car insurance",
            "auto insurance",
            "assurance",
            "insurance",
            "assur",
            "maif",
            "macif",
            "axa",
            "allianz",
        ],
        icon::SHIELD_CHECK,
    ),
    (
        &[
            "impot",
            "taxe",
            "tax",
            "tresor",
            "fisc",
            "urssaf",
            "amende",
            "income tax",
            "other taxes",
        ],
        icon::RECEIPT,
    ),
    (
        &[
            "agios",
            "frais",
            "commission",
            "cotisation",
            "fee",
            "bank fees",
        ],
        icon::PERCENT,
    ),
    (
        &[
            "credit", "pret", "emprunt", "loan", "mortgage", "echeance", "car loan",
        ],
        icon::HAND_COINS,
    ),
    (
        &[
            "epargne",
            "livret",
            "saving",
            "placement",
            "retirement savings",
        ],
        icon::PIGGY_BANK,
    ),
    (
        &[
            "bourse",
            "action",
            "titre",
            "investiss",
            "sicav",
            "opcvm",
            "invest",
            "stock",
            "dividende",
        ],
        icon::CHART_LINE_UP,
    ),
    (&["banque", "bank", "bancaire"], icon::BANK),
    (&["carte", "card", "cb"], icon::CREDIT_CARD),
    (
        &[
            "retrait",
            "especes",
            "liquide",
            "decaissement",
            "cash",
            "dab",
        ],
        icon::MONEY,
    ),
    (
        &["virement", "transfert", "transfer"],
        icon::ARROWS_LEFT_RIGHT,
    ),
    (&["cheque", "check"], icon::FILE_TEXT),
    (
        &["juridique", "avocat", "notaire", "legal", "huissier"],
        icon::GAVEL,
    ),
    (&["comptab", "expert", "gestion"], icon::CALCULATOR),
    // Revenus
    (
        &[
            "salaire",
            "paye",
            "paie",
            "salary",
            "traitement",
            "revenu",
            "income",
        ],
        icon::BRIEFCASE,
    ),
    (
        &[
            "prime",
            "bonus",
            "gratification",
            "interessement",
            "participation",
        ],
        icon::STAR,
    ),
    (
        &[
            "remboursement",
            "rbt",
            "refund",
            "avoir",
            "reimburs",
            "health reimburs",
        ],
        icon::ARROW_U_UP_LEFT,
    ),
    (
        &[
            "allocation",
            "caf",
            "aide",
            "pension",
            "retraite",
            "chomage",
            "pole",
            "benefit",
            "allowance",
            "retirement",
        ],
        icon::HAND_HEART,
    ),
    (
        &["vente", "facture", "client", "honoraire", "invoice"],
        icon::RECEIPT,
    ),
    (&["loyer percu", "locatif", "rental income"], icon::KEY),
    // Transport
    (
        &[
            "essence",
            "carburant",
            "station",
            "gasoil",
            "diesel",
            "total",
            "fuel",
            "petrol",
            "gasoline",
        ],
        icon::GAS_PUMP,
    ),
    (&["recharge", "borne", "charging"], icon::CHARGING_STATION),
    (
        &["peage", "autoroute", "toll", "vinci", "sanef"],
        icon::ROAD_HORIZON,
    ),
    (
        &["parking", "stationnement", "horodateur", "garage"],
        icon::GARAGE,
    ),
    (&["train", "sncf", "tgv", "ter", "rail"], icon::TRAIN),
    (
        &["bus", "metro", "ratp", "tram", "navigo", "public", "commun"],
        icon::BUS,
    ),
    (
        &["avion", "vol", "aeroport", "airline", "air"],
        icon::AIRPLANE_TILT,
    ),
    (&["bateau", "ferry", "boat"], icon::BOAT),
    (&["velo", "bike", "cycle"], icon::BICYCLE),
    (&["moto", "scooter"], icon::MOTORCYCLE),
    (
        &[
            "voiture",
            "auto",
            "vehicule",
            "car",
            "automobile",
            "controle",
        ],
        icon::CAR_PROFILE,
    ),
    (&["transport", "deplacement", "trajet"], icon::CAR),
    // Loisirs, culture, sport
    (
        &[
            "vacance", "voyage", "sejour", "holiday", "trip", "tourisme", "travel", "vacation",
        ],
        icon::SUITCASE_ROLLING,
    ),
    (
        &[
            "hotel",
            "hebergement",
            "airbnb",
            "gite",
            "camping",
            "lodging",
            "accommodation",
        ],
        icon::BED,
    ),
    (
        &["cinema", "film", "spectacle", "theatre", "concert", "movie"],
        icon::FILM_SLATE,
    ),
    (
        &["musee", "exposition", "billet", "ticket", "parc"],
        icon::TICKET,
    ),
    (
        &["jeu", "jouet", "game", "console", "toys"],
        icon::GAME_CONTROLLER,
    ),
    (&["sport", "gym", "fitness", "salle", "club"], icon::BARBELL),
    (
        &["foot", "tennis", "golf", "ski", "match"],
        icon::SOCCER_BALL,
    ),
    (&["piscine", "natation", "swim"], icon::SWIMMING_POOL),
    (
        &["livre", "librairie", "bd", "book", "lecture"],
        icon::BOOKS,
    ),
    (
        &["musique", "disque", "cd", "music", "supports"],
        icon::MUSIC_NOTES,
    ),
    (&["instrument", "guitare", "piano"], icon::GUITAR),
    (
        &["art", "dessin", "creatif", "loisirs creatifs"],
        icon::PALETTE,
    ),
    (
        &[
            "sortie",
            "loisir",
            "leisure",
            "divertissement",
            "entertain",
            "event",
        ],
        icon::BALLOON,
    ),
    // Personnes et famille
    (
        &[
            "enfant", "bebe", "creche", "nounou", "garde", "baby", "kid", "child",
        ],
        icon::BABY,
    ),
    (
        &[
            "ecole",
            "scolaire",
            "cantine",
            "etude",
            "universite",
            "formation",
            "cours",
            "education",
            "school",
            "tuition",
        ],
        icon::GRADUATION_CAP,
    ),
    (
        &[
            "animal",
            "animaux",
            "veterinaire",
            "veto",
            "chien",
            "chat",
            "pet",
        ],
        icon::PAW_PRINT,
    ),
    (&["cadeau", "gift", "noel"], icon::GIFT),
    (
        &["don", "association", "charit", "donation"],
        icon::HAND_HEART,
    ),
    (&["coiffeur", "coiffure", "barbier", "hair"], icon::SCISSORS),
    (
        &["beaute", "cosmetique", "parfum", "esthetique", "beauty"],
        icon::SPARKLE,
    ),
    (&["bijou", "bijouterie", "jewel"], icon::DIAMOND),
    (&["montre", "horlogerie"], icon::WATCH),
    (&["chaussure", "basket", "shoe"], icon::SNEAKER),
    (
        &[
            "habillement",
            "vetement",
            "habits",
            "textile",
            "mode",
            "clothing",
            "pret a porter",
        ],
        icon::T_SHIRT,
    ),
    (&["sac", "maroquinerie", "bag"], icon::HANDBAG),
    (&["tabac", "cigarette", "tobacco"], icon::CIGARETTE),
    (&["culte", "eglise", "religion"], icon::CHURCH),
    (&["famille", "family", "personnel"], icon::USERS),
    // Divers
    (
        &[
            "shopping",
            "achat",
            "magasin",
            "boutique",
            "fnac",
            "darty",
            "other purchases",
        ],
        icon::SHOPPING_BAG,
    ),
    (
        &["divers", "autre", "misc", "other", "inconnu"],
        icon::DOTS_THREE_CIRCLE,
    ),
];

const FALLBACK: &str = icon::TAG;

/// Icône et indice de couleur d'une catégorie.
#[derive(Debug, Clone, Copy)]
pub struct CategoryStyle {
    pub icon: &'static str,
    pub color: usize,
}

/// Calcule le style de chaque catégorie d'un document.
pub fn styles(document: &Document) -> HashMap<u32, CategoryStyle> {
    let mut map = HashMap::new();
    let mut parent: Option<(&str, &'static str)> = None;
    for category in &document.categories {
        if !category.is_child {
            let icon = icon_for(&category.name).unwrap_or(FALLBACK);
            parent = Some((&category.name, icon));
            map.insert(
                category.id,
                CategoryStyle {
                    icon,
                    color: color_for(&category.name),
                },
            );
        } else {
            let (parent_name, parent_icon) = parent.unwrap_or((&category.name, FALLBACK));
            let icon = icon_for(&category.name).unwrap_or(parent_icon);
            map.insert(
                category.id,
                CategoryStyle {
                    icon,
                    color: color_for(parent_name),
                },
            );
        }
    }
    map
}

/// Icône du premier mot du libellé reconnu par une règle : le premier mot
/// est le plus significatif (« Impôt sur le revenu » est un impôt, « Rbt
/// santé » un remboursement). Les mots-clés composés passent avant.
pub fn icon_for(label: &str) -> Option<&'static str> {
    let normalized = normalize(label);
    let words: Vec<&str> = normalized
        .split_whitespace()
        .filter(|w| w.len() >= 2)
        .collect();
    let padded = format!(" {} ", words.join(" "));
    for (keywords, glyph) in RULES {
        if keywords
            .iter()
            .any(|k| k.contains(' ') && padded.contains(&format!(" {k}")))
        {
            return Some(glyph);
        }
    }
    for word in &words {
        for (keywords, glyph) in RULES {
            let hit = keywords
                .iter()
                .any(|k| !k.contains(' ') && word.starts_with(k) && (k.len() >= 3 || word == k));
            if hit {
                return Some(glyph);
            }
        }
    }
    None
}

/// Couleur stable (indice dans la palette du thème) dérivée du nom.
fn color_for(name: &str) -> usize {
    let mut hash: u32 = 2_166_136_261;
    for b in normalize(name).trim().bytes() {
        hash = (hash ^ b as u32).wrapping_mul(16_777_619);
    }
    hash as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_icons_from_words() {
        assert_eq!(icon_for("Restaurant déjeuner"), Some(icon::FORK_KNIFE));
        assert_eq!(icon_for("Epicerie 200"), Some(icon::SHOPPING_CART));
        assert_eq!(icon_for("EDF"), Some(icon::LIGHTNING));
        assert_eq!(icon_for("Impôt sur le revenu"), Some(icon::RECEIPT));
        assert_eq!(icon_for("Téléphonie"), Some(icon::PHONE));
        assert_eq!(icon_for("Péage"), Some(icon::ROAD_HORIZON));
        assert_eq!(icon_for("Rbt santé"), Some(icon::ARROW_U_UP_LEFT));
        assert_eq!(icon_for("Coiffeur"), Some(icon::SCISSORS));
        assert_eq!(icon_for("Xyzzy"), None);
    }
}

#[cfg(test)]
mod standard_list {
    use super::*;

    #[test]
    fn every_standard_category_has_an_icon() {
        let missing: Vec<&str> = bp_core::categorize::taxonomy::GENERICS
            .iter()
            .flat_map(|g| [g.name, g.name_en])
            .filter(|name| icon_for(name).is_none())
            .collect();
        assert!(missing.is_empty(), "sans icône : {missing:?}");
    }

    #[test]
    fn english_names_get_fitting_icons() {
        let cases = [
            ("Groceries", icon::SHOPPING_CART),
            ("Bakery & market", icon::BREAD),
            ("Rent", icon::HOUSE_LINE),
            ("Mortgage", icon::HAND_COINS),
            ("Electricity & gas", icon::LIGHTNING),
            ("Home improvement", icon::HAMMER),
            ("Dental & vision", icon::TOOTH),
            ("Health insurance", icon::HEARTBEAT),
            ("Childcare", icon::BABY),
            ("School & lunches", icon::GRADUATION_CAP),
            ("Lodging", icon::BED),
            ("Car loan & lease", icon::HAND_COINS),
            ("Retirement savings", icon::PIGGY_BANK),
            ("Health reimbursements", icon::ARROW_U_UP_LEFT),
            ("Charity & donations", icon::HAND_HEART),
        ];
        for (name, glyph) in cases {
            assert_eq!(icon_for(name), Some(glyph), "{name}");
        }
        assert_eq!(icon_for("Fuel"), Some(icon::GAS_PUMP));
        assert_eq!(icon_for("Taxi & rideshare"), Some(icon::TAXI));
        assert_eq!(icon_for("Taxi & VTC"), Some(icon::TAXI));
        assert_eq!(icon_for("Home insurance"), Some(icon::SHIELD_CHECK));
        assert_eq!(icon_for("Income tax"), Some(icon::RECEIPT));
        assert_eq!(icon_for("Bank fees"), Some(icon::PERCENT));
        assert_eq!(icon_for("Travel & vacations"), Some(icon::SUITCASE_ROLLING));
        assert_eq!(icon_for("Gastronomie"), None);
        assert_eq!(icon_for("Toyota"), None);
    }
}
