//! Catégories génériques : la liste standard proposée aux nouveaux fichiers
//! (« catégories standard ») et le vocabulaire commun du dictionnaire des
//! commerçants et des codes commerçant.
//!
//! Chaque catégorie générique a un identifiant stable (`logement.energie`) ;
//! les catégories de l'utilisateur, elles, sont libres. Le lien entre les
//! deux s'apprend sur l'historique ou, à défaut, par ressemblance des noms
//! ([`super::Categorizer`]).

/// Sens d'opération accepté par une catégorie générique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Any,
    /// Recettes seulement (revenus).
    Credit,
}

/// Catégorie générique.
#[derive(Debug, Clone, Copy)]
pub struct Generic {
    /// Identifiant : `parent` ou `parent.enfant`.
    pub slug: &'static str,
    /// Nom dans la liste standard.
    pub name: &'static str,
    /// Mots (normalisés) qui désignent cette catégorie dans les noms choisis
    /// par l'utilisateur ; le nom standard en fait toujours partie.
    pub synonyms: &'static [&'static str],
    pub flow: Flow,
    /// Catégorie à utiliser pour une recette (remboursement de mutuelle,
    /// loyer perçu…) ; sinon la même catégorie (avoir, remboursement).
    pub on_credit: Option<&'static str>,
    /// Catégorie proche, essayée si celle-ci n'a pas d'équivalent.
    pub fallback: Option<&'static str>,
}

const fn g(slug: &'static str, name: &'static str, synonyms: &'static [&'static str]) -> Generic {
    Generic {
        slug,
        name,
        synonyms,
        flow: Flow::Any,
        on_credit: None,
        fallback: None,
    }
}

const fn credit(mut generic: Generic) -> Generic {
    generic.flow = Flow::Credit;
    generic
}

const fn on_credit(mut generic: Generic, slug: &'static str) -> Generic {
    generic.on_credit = Some(slug);
    generic
}

const fn fallback(mut generic: Generic, slug: &'static str) -> Generic {
    generic.fallback = Some(slug);
    generic
}

/// Liste standard, dans l'ordre d'affichage : chaque parent suivi de ses
/// sous-catégories.
pub const GENERICS: &[Generic] = &[
    g(
        "alimentation",
        "Alimentation",
        &["alimentation", "nourriture"],
    ),
    g(
        "alimentation.courses",
        "Courses",
        &[
            "courses",
            "epicerie",
            "supermarche",
            "alimentation",
            "nourriture",
            "provisions",
        ],
    ),
    fallback(
        g(
            "alimentation.commerces",
            "Boulangerie & marché",
            &[
                "boulangerie",
                "marche",
                "primeur",
                "boucherie",
                "commerces de bouche",
                "commerces",
            ],
        ),
        "alimentation.courses",
    ),
    g(
        "alimentation.restaurants",
        "Restaurants",
        &[
            "restaurant",
            "restaurants",
            "resto",
            "restauration",
            "fast food",
            "sorties restaurant",
            "dejeuner",
        ],
    ),
    g(
        "logement",
        "Logement",
        &["logement", "habitation", "maison", "appartement"],
    ),
    on_credit(
        g("logement.loyer", "Loyer", &["loyer", "loyers"]),
        "revenus.locatifs",
    ),
    g(
        "logement.credit",
        "Crédit immobilier",
        &[
            "credit immobilier",
            "pret immobilier",
            "emprunt",
            "pret",
            "credit",
        ],
    ),
    g(
        "logement.charges",
        "Charges & copropriété",
        &["charges", "copropriete", "syndic"],
    ),
    g(
        "logement.energie",
        "Électricité & gaz",
        &["electricite", "gaz", "energie", "edf", "engie"],
    ),
    g("logement.eau", "Eau", &["eau"]),
    g(
        "logement.assurance",
        "Assurance habitation",
        &["assurance habitation", "assurance"],
    ),
    g(
        "logement.travaux",
        "Travaux & bricolage",
        &[
            "travaux",
            "bricolage",
            "jardin",
            "entretien maison",
            "reparations",
        ],
    ),
    g(
        "logement.equipement",
        "Équipement & décoration",
        &[
            "equipement",
            "decoration",
            "deco",
            "ameublement",
            "meubles",
            "mobilier",
            "rangement",
        ],
    ),
    g(
        "logement.services",
        "Services à domicile",
        &[
            "services a domicile",
            "menage",
            "aide a domicile",
            "femme de menage",
        ],
    ),
    g("abonnements", "Abonnements", &["abonnements", "abonnement"]),
    g(
        "abonnements.telecom",
        "Internet & téléphone",
        &[
            "internet",
            "telephone",
            "telephonie",
            "mobile",
            "box",
            "telecom",
        ],
    ),
    g(
        "abonnements.medias",
        "Streaming & presse",
        &[
            "streaming",
            "presse",
            "medias",
            "television",
            "tv",
            "journaux",
        ],
    ),
    g(
        "abonnements.logiciels",
        "Logiciels & services en ligne",
        &[
            "logiciels",
            "logiciel",
            "services en ligne",
            "cloud",
            "applications",
            "informatique",
        ],
    ),
    g(
        "transports",
        "Transports",
        &["transports", "transport", "voiture", "auto", "vehicule"],
    ),
    g(
        "transports.carburant",
        "Carburant",
        &["carburant", "essence", "gazole", "diesel", "recharge"],
    ),
    g(
        "transports.entretien",
        "Entretien & réparations",
        &["entretien", "reparations", "garage", "reparation"],
    ),
    g(
        "transports.assurance",
        "Assurance auto",
        &["assurance auto", "assurance voiture", "assurance"],
    ),
    g(
        "transports.credit",
        "Crédit & location auto",
        &["credit auto", "leasing", "location auto", "lld", "loa"],
    ),
    g(
        "transports.peages",
        "Péages & stationnement",
        &["peage", "peages", "parking", "stationnement"],
    ),
    g(
        "transports.commun",
        "Transports en commun",
        &[
            "transports en commun",
            "public",
            "metro",
            "bus",
            "navigo",
            "tram",
        ],
    ),
    g("transports.taxi", "Taxi & VTC", &["taxi", "vtc", "uber"]),
    on_credit(g("sante", "Santé", &["sante", "medical"]), "revenus.sante"),
    on_credit(
        g(
            "sante.medecins",
            "Médecins & examens",
            &[
                "medecin",
                "medecins",
                "docteur",
                "consultation",
                "analyses",
                "hopital",
                "specialistes",
                "examens",
            ],
        ),
        "revenus.sante",
    ),
    on_credit(
        g(
            "sante.pharmacie",
            "Pharmacie",
            &["pharmacie", "medicaments"],
        ),
        "revenus.sante",
    ),
    on_credit(
        g(
            "sante.dentaire",
            "Dentaire & optique",
            &["dentiste", "dentaire", "optique", "lunettes", "opticien"],
        ),
        "revenus.sante",
    ),
    on_credit(
        g(
            "sante.paramedical",
            "Kiné & paramédical",
            &[
                "kine",
                "kinesitherapeute",
                "paramedical",
                "osteopathe",
                "osteo",
            ],
        ),
        "revenus.sante",
    ),
    on_credit(
        g(
            "sante.mutuelle",
            "Mutuelle",
            &["mutuelle", "complementaire sante"],
        ),
        "revenus.sante",
    ),
    g("enfants", "Enfants", &["enfants", "enfant", "bebe"]),
    g(
        "enfants.garde",
        "Garde & crèche",
        &[
            "garde",
            "creche",
            "nounou",
            "assistante maternelle",
            "baby sitting",
        ],
    ),
    g(
        "enfants.ecole",
        "École & cantine",
        &["ecole", "scolarite", "cantine", "etudes", "scolaire"],
    ),
    g(
        "enfants.activites",
        "Activités extrascolaires",
        &["activites", "extrascolaire", "loisirs enfants"],
    ),
    g(
        "enfants.jouets",
        "Jouets & puériculture",
        &["jouets", "puericulture", "bebe"],
    ),
    g("shopping", "Shopping", &["shopping", "achats"]),
    g(
        "shopping.vetements",
        "Vêtements & chaussures",
        &["vetements", "habillement", "chaussures", "mode"],
    ),
    g(
        "shopping.beaute",
        "Coiffure & beauté",
        &[
            "coiffeur",
            "coiffure",
            "beaute",
            "cosmetiques",
            "esthetique",
        ],
    ),
    g(
        "shopping.hightech",
        "High-tech & électroménager",
        &[
            "high tech",
            "hifi",
            "video",
            "electromenager",
            "multimedia",
            "informatique",
            "telephonie",
        ],
    ),
    g(
        "shopping.divers",
        "Achats divers",
        &["achats", "divers", "shopping"],
    ),
    g("loisirs", "Loisirs", &["loisirs", "sorties"]),
    g(
        "loisirs.sports",
        "Sports",
        &["sport", "sports", "salle de sport"],
    ),
    g(
        "loisirs.sorties",
        "Sorties & spectacles",
        &[
            "cinema",
            "spectacles",
            "sorties",
            "concerts",
            "musees",
            "theatre",
        ],
    ),
    g(
        "loisirs.culture",
        "Livres, musique & films",
        &[
            "livres",
            "librairie",
            "bd",
            "musique",
            "films",
            "dvd",
            "blu ray",
            "culture",
            "supports musique",
        ],
    ),
    g(
        "loisirs.jeux",
        "Jeux & hobbies",
        &["jeux", "jeux video", "hobbies", "photo", "loisirs creatifs"],
    ),
    g(
        "vacances",
        "Vacances & voyages",
        &["vacances", "voyages", "voyage"],
    ),
    on_credit(
        g(
            "vacances.hebergement",
            "Hébergement",
            &[
                "hebergement",
                "hotel",
                "location vacances",
                "logement vacances",
            ],
        ),
        "revenus.locatifs",
    ),
    g(
        "vacances.transport",
        "Train, avion & location",
        &["train", "avion", "billets", "location voiture"],
    ),
    g(
        "impots",
        "Impôts & taxes",
        &["impots", "impot", "taxes", "taxe"],
    ),
    g(
        "impots.revenu",
        "Impôt sur le revenu",
        &["impot sur le revenu", "impot revenu", "ir"],
    ),
    g(
        "impots.foncier",
        "Taxe foncière",
        &["taxe fonciere", "impot foncier", "foncier"],
    ),
    g(
        "impots.autres",
        "Autres taxes & amendes",
        &[
            "amendes",
            "amende",
            "taxe d habitation",
            "autres taxes",
            "timbres",
        ],
    ),
    g(
        "banque",
        "Banque & assurances",
        &["banque", "frais bancaires"],
    ),
    g(
        "banque.frais",
        "Frais bancaires",
        &["frais bancaires", "frais", "agios", "commissions"],
    ),
    g(
        "banque.credit",
        "Crédit à la consommation",
        &["credit consommation", "credit", "pret", "emprunt"],
    ),
    g(
        "banque.assurances",
        "Assurances & prévoyance",
        &["assurances", "assurance", "prevoyance"],
    ),
    g(
        "banque.retraits",
        "Retraits d'espèces",
        &["retraits", "retrait", "especes", "liquide", "decaissement"],
    ),
    g(
        "epargne",
        "Épargne & placements",
        &["epargne", "placements"],
    ),
    g(
        "epargne.livrets",
        "Livrets & épargne",
        &["livrets", "livret", "epargne"],
    ),
    g("epargne.assurance_vie", "Assurance vie", &["assurance vie"]),
    g(
        "epargne.bourse",
        "Bourse & placements",
        &["bourse", "placements", "actions", "crypto"],
    ),
    credit(g(
        "revenus",
        "Revenus",
        &["revenus", "recettes", "rentrees"],
    )),
    credit(g(
        "revenus.salaires",
        "Salaires & primes",
        &[
            "salaire",
            "salaires",
            "paye",
            "paie",
            "primes",
            "remuneration",
        ],
    )),
    credit(g(
        "revenus.allocations",
        "Allocations & aides",
        &["allocations", "caf", "aides", "chomage", "prestations"],
    )),
    credit(g(
        "revenus.retraites",
        "Retraites & pensions",
        &["retraite", "retraites", "pension", "pensions"],
    )),
    credit(g(
        "revenus.sante",
        "Remboursements santé",
        &[
            "remboursements sante",
            "rbt sante",
            "secu",
            "remboursement sante",
        ],
    )),
    credit(g(
        "revenus.locatifs",
        "Revenus locatifs",
        &["revenus locatifs", "loyers percus", "location"],
    )),
    credit(g(
        "revenus.financiers",
        "Revenus financiers",
        &["revenus financiers", "interets", "dividendes"],
    )),
    credit(g(
        "revenus.autres",
        "Autres revenus",
        &["autres revenus", "remboursements", "ventes", "divers"],
    )),
    g("divers", "Divers", &["divers", "autres"]),
    g(
        "divers.animaux",
        "Animaux",
        &["animaux", "veterinaire", "chien", "chat"],
    ),
    g("divers.cadeaux", "Cadeaux", &["cadeaux", "cadeau"]),
    g(
        "divers.dons",
        "Dons & associations",
        &["dons", "associations", "dons associations", "charite"],
    ),
    g(
        "divers.tabac",
        "Tabac & presse",
        &["tabac", "presse", "jeux d argent"],
    ),
    g(
        "virements",
        "Virements internes",
        &[
            "virements internes",
            "virement interne",
            "transferts",
            "mouvements internes",
            "virements",
        ],
    ),
];

/// Catégorie générique par identifiant.
pub fn generic(slug: &str) -> Option<&'static Generic> {
    GENERICS.iter().find(|g| g.slug == slug)
}

/// Identifiant du parent (`logement` pour `logement.loyer`).
pub fn parent_slug(slug: &str) -> &str {
    slug.split_once('.').map_or(slug, |(parent, _)| parent)
}

/// Catégories standard au format de `manage::import_categories` : une
/// catégorie par ligne, sous-catégories en retrait.
pub fn standard_categories_text() -> String {
    GENERICS
        .iter()
        .map(|g| {
            if g.slug.contains('.') {
                format!("     {}", g.name)
            } else {
                g.name.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_child_has_its_parent_just_before() {
        let mut current = "";
        for g in GENERICS {
            match g.slug.split_once('.') {
                None => current = g.slug,
                Some((parent, _)) => assert_eq!(parent, current, "{}", g.slug),
            }
        }
        for g in GENERICS {
            for target in [g.on_credit, g.fallback].into_iter().flatten() {
                assert!(generic(target).is_some(), "{target}");
            }
        }
    }

    #[test]
    fn reasonable_size() {
        let parents = GENERICS.iter().filter(|g| !g.slug.contains('.')).count();
        assert!((12..=16).contains(&parents));
        assert!((55..=70).contains(&(GENERICS.len() - parents)));
    }
}
