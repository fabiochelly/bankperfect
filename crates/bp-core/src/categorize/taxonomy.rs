//! Catégories génériques : la liste standard proposée aux nouveaux fichiers
//! (« catégories standard ») et le vocabulaire commun du dictionnaire des
//! commerçants et des codes commerçant.
//!
//! Chaque catégorie générique a un identifiant stable (`logement.energie`) ;
//! les catégories de l'utilisateur, elles, sont libres. Le lien entre les
//! deux s'apprend sur l'historique ou, à défaut, par ressemblance des noms
//! ([`super::Categorizer`]).
//!
//! La liste standard existe en français et en anglais : un nouveau fichier
//! la reçoit dans la langue de l'interface ([`standard_categories`]). Les
//! synonymes des deux langues servent à rapprocher les noms choisis par
//! l'utilisateur, quelle que soit la langue de l'interface (voir
//! [`super::Categorizer::without_history`]).

use crate::i18n::Lang;
use crate::model::Category;

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
    /// Nom dans la liste standard anglaise.
    pub name_en: &'static str,
    /// Synonymes anglais, mêmes conventions.
    pub synonyms_en: &'static [&'static str],
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
        name_en: "",
        synonyms_en: &[],
        flow: Flow::Any,
        on_credit: None,
        fallback: None,
    }
}

impl Generic {
    /// Nom et synonymes anglais.
    const fn en(mut self, name: &'static str, synonyms: &'static [&'static str]) -> Self {
        self.name_en = name;
        self.synonyms_en = synonyms;
        self
    }

    /// Nom dans la liste standard de la langue `lang`.
    pub fn name_in(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Fr => self.name,
            Lang::En => self.name_en,
        }
    }

    /// Nom dans la liste standard de la langue en cours.
    pub fn local_name(&self) -> &'static str {
        self.name_in(crate::i18n::lang())
    }

    /// Synonymes d'une langue, nom standard compris.
    pub fn names_in(&self, lang: Lang) -> impl Iterator<Item = &'static str> + use<> {
        let synonyms = match lang {
            Lang::Fr => self.synonyms,
            Lang::En => self.synonyms_en,
        };
        synonyms.iter().copied().chain([self.name_in(lang)])
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
    )
    .en("Food", &["food", "food and drink", "food and dining"]),
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
    )
    .en(
        "Groceries",
        &[
            "groceries",
            "grocery",
            "supermarket",
            "supermarkets",
            "food shopping",
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
        )
        .en(
            "Bakery & market",
            &[
                "bakery",
                "bakeries",
                "butcher",
                "market",
                "farmers market",
                "deli",
                "greengrocer",
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
    )
    .en(
        "Restaurants",
        &[
            "restaurant",
            "restaurants",
            "dining out",
            "eating out",
            "fast food",
            "takeaway",
            "takeout",
            "coffee shops",
            "coffee",
            "dining",
            "food delivery",
        ],
    ),
    g(
        "logement",
        "Logement",
        &["logement", "habitation", "maison", "appartement"],
    )
    .en("Housing", &["housing", "home", "household", "house"]),
    on_credit(
        g("logement.loyer", "Loyer", &["loyer", "loyers"])
            .en("Rent", &["rent", "rent payment", "rental payment"]),
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
    )
    .en(
        "Mortgage",
        &["mortgage", "mortgages", "home loan", "mortgage payment"],
    ),
    g(
        "logement.charges",
        "Charges & copropriété",
        &["charges", "copropriete", "syndic"],
    )
    .en(
        "HOA & service charges",
        &[
            "hoa",
            "hoa fees",
            "hoa dues",
            "service charges",
            "condo fees",
            "strata fees",
            "body corporate",
        ],
    ),
    g(
        "logement.energie",
        "Électricité & gaz",
        &["electricite", "gaz", "energie", "edf", "engie"],
    )
    .en(
        "Electricity & gas",
        &[
            "electricity",
            "electric",
            "gas and electric",
            "gas and electricity",
            "energy",
            "utilities",
            "power",
            "heating",
        ],
    ),
    g("logement.eau", "Eau", &["eau"]).en("Water", &["water", "water and sewer", "sewer"]),
    g(
        "logement.assurance",
        "Assurance habitation",
        &["assurance habitation", "assurance"],
    )
    .en(
        "Home insurance",
        &[
            "home insurance",
            "homeowners insurance",
            "renters insurance",
            "contents insurance",
            "insurance",
        ],
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
    )
    .en(
        "Home improvement",
        &[
            "home improvement",
            "home repairs",
            "repairs",
            "maintenance",
            "diy",
            "garden",
            "gardening",
            "hardware",
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
    )
    .en(
        "Furniture & decor",
        &[
            "furniture",
            "furnishings",
            "decor",
            "home decor",
            "homewares",
            "home goods",
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
    )
    .en(
        "Home services",
        &[
            "home services",
            "cleaning",
            "housekeeping",
            "cleaner",
            "maid",
            "lawn care",
            "household help",
        ],
    ),
    g("abonnements", "Abonnements", &["abonnements", "abonnement"]).en(
        "Subscriptions",
        &[
            "subscriptions",
            "subscription",
            "memberships",
            "bills",
            "bills and utilities",
        ],
    ),
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
    )
    .en(
        "Internet & phone",
        &[
            "internet",
            "phone",
            "mobile",
            "mobile phone",
            "cell phone",
            "broadband",
            "telephone",
            "wireless",
            "cable",
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
    )
    .en(
        "Streaming & news",
        &[
            "streaming",
            "news",
            "newspapers",
            "magazines",
            "television",
            "tv",
            "cable tv",
            "music streaming",
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
    )
    .en(
        "Software & online services",
        &[
            "software",
            "apps",
            "online services",
            "cloud",
            "web services",
            "computer",
            "digital services",
        ],
    ),
    g(
        "transports",
        "Transports",
        &["transports", "transport", "voiture", "auto", "vehicule"],
    )
    .en(
        "Transportation",
        &[
            "transportation",
            "transport",
            "car",
            "auto",
            "vehicle",
            "auto and transport",
        ],
    ),
    g(
        "transports.carburant",
        "Carburant",
        &["carburant", "essence", "gazole", "diesel", "recharge"],
    )
    .en(
        "Fuel",
        &[
            "fuel",
            "gas",
            "gasoline",
            "petrol",
            "diesel",
            "gas station",
            "ev charging",
            "charging",
        ],
    ),
    g(
        "transports.entretien",
        "Entretien & réparations",
        &["entretien", "reparations", "garage", "reparation"],
    )
    .en(
        "Maintenance & repairs",
        &[
            "car maintenance",
            "car repairs",
            "auto repair",
            "auto maintenance",
            "repairs",
            "maintenance",
            "servicing",
            "car wash",
            "tires",
            "tyres",
        ],
    ),
    g(
        "transports.assurance",
        "Assurance auto",
        &["assurance auto", "assurance voiture", "assurance"],
    )
    .en(
        "Car insurance",
        &[
            "car insurance",
            "auto insurance",
            "motor insurance",
            "vehicle insurance",
            "insurance",
        ],
    ),
    g(
        "transports.credit",
        "Crédit & location auto",
        &["credit auto", "leasing", "location auto", "lld", "loa"],
    )
    .en(
        "Car loan & lease",
        &[
            "car loan",
            "auto loan",
            "car lease",
            "lease",
            "leasing",
            "car finance",
            "car payment",
            "auto payment",
        ],
    ),
    g(
        "transports.peages",
        "Péages & stationnement",
        &["peage", "peages", "parking", "stationnement"],
    )
    .en(
        "Tolls & parking",
        &["tolls", "toll", "parking", "congestion charge"],
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
    )
    .en(
        "Public transit",
        &[
            "public transit",
            "public transport",
            "transit",
            "subway",
            "metro",
            "bus",
            "commuting",
            "commute",
        ],
    ),
    g("transports.taxi", "Taxi & VTC", &["taxi", "vtc", "uber"]).en(
        "Taxi & rideshare",
        &[
            "taxi",
            "taxis",
            "rideshare",
            "ride share",
            "ride sharing",
            "uber",
            "lyft",
            "cab",
        ],
    ),
    on_credit(
        g("sante", "Santé", &["sante", "medical"]).en(
            "Health",
            &["health", "medical", "healthcare", "health care"],
        ),
        "revenus.sante",
    ),
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
        )
        .en(
            "Doctors & tests",
            &[
                "doctor",
                "doctors",
                "physician",
                "medical bills",
                "medical expenses",
                "hospital",
                "lab tests",
                "labs",
                "specialists",
                "clinic",
            ],
        ),
        "revenus.sante",
    ),
    on_credit(
        g(
            "sante.pharmacie",
            "Pharmacie",
            &["pharmacie", "medicaments"],
        )
        .en(
            "Pharmacy",
            &[
                "pharmacy",
                "pharmacies",
                "drugstore",
                "prescriptions",
                "medications",
                "medicine",
                "chemist",
            ],
        ),
        "revenus.sante",
    ),
    on_credit(
        g(
            "sante.dentaire",
            "Dentaire & optique",
            &["dentiste", "dentaire", "optique", "lunettes", "opticien"],
        )
        .en(
            "Dental & vision",
            &[
                "dental",
                "dentist",
                "vision",
                "optical",
                "optician",
                "glasses",
                "eye care",
                "eyecare",
                "orthodontist",
            ],
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
        )
        .en(
            "Physio & therapy",
            &[
                "physio",
                "physiotherapy",
                "physical therapy",
                "therapy",
                "therapist",
                "chiropractor",
                "osteopath",
                "counseling",
                "mental health",
                "psychologist",
            ],
        ),
        "revenus.sante",
    ),
    on_credit(
        g(
            "sante.mutuelle",
            "Mutuelle",
            &["mutuelle", "complementaire sante"],
        )
        .en(
            "Health insurance",
            &[
                "health insurance",
                "medical insurance",
                "dental insurance",
                "health plan",
                "medicare",
            ],
        ),
        "revenus.sante",
    ),
    g("enfants", "Enfants", &["enfants", "enfant", "bebe"])
        .en("Children", &["children", "kids", "child", "baby"]),
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
    )
    .en(
        "Childcare",
        &[
            "childcare",
            "child care",
            "daycare",
            "day care",
            "nanny",
            "babysitting",
            "babysitter",
            "nursery",
            "after school care",
        ],
    ),
    g(
        "enfants.ecole",
        "École & cantine",
        &["ecole", "scolarite", "cantine", "etudes", "scolaire"],
    )
    .en(
        "School & lunches",
        &[
            "school",
            "schools",
            "tuition",
            "education",
            "school lunches",
            "school fees",
            "university",
        ],
    ),
    g(
        "enfants.activites",
        "Activités extrascolaires",
        &["activites", "extrascolaire", "loisirs enfants"],
    )
    .en(
        "Extracurricular activities",
        &[
            "extracurricular",
            "activities",
            "kids activities",
            "after school activities",
            "camps",
            "summer camp",
            "lessons",
        ],
    ),
    g(
        "enfants.jouets",
        "Jouets & puériculture",
        &["jouets", "puericulture", "bebe"],
    )
    .en(
        "Toys & baby gear",
        &[
            "toys",
            "toy",
            "baby gear",
            "baby supplies",
            "baby",
            "diapers",
            "nappies",
        ],
    ),
    g("shopping", "Shopping", &["shopping", "achats"]).en("Shopping", &["shopping", "purchases"]),
    g(
        "shopping.vetements",
        "Vêtements & chaussures",
        &["vetements", "habillement", "chaussures", "mode"],
    )
    .en(
        "Clothing & shoes",
        &[
            "clothing", "clothes", "shoes", "apparel", "fashion", "footwear",
        ],
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
    )
    .en(
        "Hair & beauty",
        &[
            "hair",
            "haircut",
            "hairdresser",
            "beauty",
            "cosmetics",
            "personal care",
            "barber",
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
    )
    .en(
        "Electronics & appliances",
        &[
            "electronics",
            "appliances",
            "tech",
            "computers",
            "gadgets",
            "electronics and software",
        ],
    ),
    g(
        "shopping.divers",
        "Achats divers",
        &["achats", "divers", "shopping"],
    )
    .en(
        "Other purchases",
        &[
            "other purchases",
            "general merchandise",
            "shopping",
            "purchases",
            "online shopping",
            "household items",
        ],
    ),
    g("loisirs", "Loisirs", &["loisirs", "sorties"]).en(
        "Leisure",
        &["leisure", "entertainment", "fun", "recreation"],
    ),
    g(
        "loisirs.sports",
        "Sports",
        &["sport", "sports", "salle de sport"],
    )
    .en(
        "Sports",
        &["sports", "sport", "gym", "fitness", "sporting goods"],
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
    )
    .en(
        "Entertainment & events",
        &[
            "entertainment",
            "events",
            "movies",
            "cinema",
            "concerts",
            "shows",
            "theater",
            "theatre",
            "museums",
            "nightlife",
            "going out",
            "amusement",
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
    )
    .en(
        "Books, music & movies",
        &[
            "books",
            "music",
            "movies",
            "films",
            "dvds",
            "culture",
            "bookstore",
            "ebooks",
        ],
    ),
    g(
        "loisirs.jeux",
        "Jeux & hobbies",
        &["jeux", "jeux video", "hobbies", "photo", "loisirs creatifs"],
    )
    .en(
        "Games & hobbies",
        &[
            "games",
            "video games",
            "gaming",
            "hobbies",
            "crafts",
            "arts and crafts",
            "photography",
        ],
    ),
    g(
        "vacances",
        "Vacances & voyages",
        &["vacances", "voyages", "voyage"],
    )
    .en(
        "Travel & vacations",
        &[
            "travel",
            "vacation",
            "vacations",
            "holidays",
            "holiday",
            "trips",
        ],
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
        )
        .en(
            "Lodging",
            &[
                "lodging",
                "accommodation",
                "hotel",
                "hotels",
                "vacation rental",
                "airbnb",
                "hostel",
            ],
        ),
        "revenus.locatifs",
    ),
    g(
        "vacances.transport",
        "Train, avion & location",
        &["train", "avion", "billets", "location voiture"],
    )
    .en(
        "Trains, flights & car rental",
        &[
            "train",
            "trains",
            "flights",
            "airfare",
            "air travel",
            "plane tickets",
            "car rental",
            "rental car",
            "rail",
            "airline",
        ],
    ),
    g(
        "impots",
        "Impôts & taxes",
        &["impots", "impot", "taxes", "taxe"],
    )
    .en("Taxes", &["taxes", "tax", "taxation"]),
    g(
        "impots.revenu",
        "Impôt sur le revenu",
        &["impot sur le revenu", "impot revenu", "ir"],
    )
    .en(
        "Income tax",
        &[
            "income tax",
            "income taxes",
            "federal tax",
            "state tax",
            "self assessment",
        ],
    ),
    g(
        "impots.foncier",
        "Taxe foncière",
        &["taxe fonciere", "impot foncier", "foncier"],
    )
    .en(
        "Property tax",
        &[
            "property tax",
            "property taxes",
            "council tax",
            "real estate tax",
            "local property tax",
        ],
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
    )
    .en(
        "Other taxes & fines",
        &[
            "fines",
            "other taxes",
            "parking fines",
            "traffic fines",
            "penalties",
            "licenses",
            "vehicle tax",
            "road tax",
            "tv licence",
        ],
    ),
    g(
        "banque",
        "Banque & assurances",
        &["banque", "frais bancaires"],
    )
    .en(
        "Banking & insurance",
        &["banking", "bank", "bank fees", "financial"],
    ),
    g(
        "banque.frais",
        "Frais bancaires",
        &["frais bancaires", "frais", "agios", "commissions"],
    )
    .en(
        "Bank fees",
        &[
            "bank fees",
            "fees",
            "bank charges",
            "service fees",
            "account fees",
            "overdraft",
            "interest charges",
            "finance charges",
            "atm fees",
        ],
    ),
    g(
        "banque.credit",
        "Crédit à la consommation",
        &["credit consommation", "credit", "pret", "emprunt"],
    )
    .en(
        "Consumer loans",
        &[
            "loans",
            "loan",
            "personal loan",
            "personal loans",
            "consumer loans",
            "student loan",
            "student loans",
            "credit card payment",
        ],
    ),
    g(
        "banque.assurances",
        "Assurances & prévoyance",
        &["assurances", "assurance", "prevoyance"],
    )
    .en(
        "Insurance & protection",
        &[
            "insurance",
            "life insurance",
            "income protection",
            "term life",
            "insurance premiums",
        ],
    ),
    g(
        "banque.retraits",
        "Retraits d'espèces",
        &["retraits", "retrait", "especes", "liquide", "decaissement"],
    )
    .en(
        "Cash withdrawals",
        &[
            "cash",
            "atm",
            "cash withdrawals",
            "withdrawals",
            "atm withdrawals",
        ],
    ),
    g(
        "epargne",
        "Épargne & placements",
        &["epargne", "placements"],
    )
    .en(
        "Savings & investments",
        &["savings", "investments", "saving", "investing"],
    ),
    g(
        "epargne.livrets",
        "Livrets & épargne",
        &["livrets", "livret", "epargne"],
    )
    .en(
        "Savings accounts",
        &[
            "savings",
            "savings accounts",
            "savings account",
            "isa",
            "cash isa",
            "emergency fund",
        ],
    ),
    g("epargne.assurance_vie", "Assurance vie", &["assurance vie"]).en(
        "Retirement savings",
        &[
            "retirement savings",
            "retirement contributions",
            "pension contributions",
            "401k",
            "ira",
            "rrsp",
            "superannuation",
            "annuity",
        ],
    ),
    g(
        "epargne.bourse",
        "Bourse & placements",
        &["bourse", "placements", "actions", "crypto"],
    )
    .en(
        "Stocks & investments",
        &[
            "investments",
            "investing",
            "stocks",
            "brokerage",
            "shares",
            "crypto",
            "cryptocurrency",
        ],
    ),
    credit(
        g("revenus", "Revenus", &["revenus", "recettes", "rentrees"])
            .en("Income", &["income", "earnings", "revenue"]),
    ),
    credit(
        g(
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
        )
        .en(
            "Salary & bonuses",
            &[
                "salary",
                "salaries",
                "wages",
                "paycheck",
                "pay",
                "payroll",
                "bonus",
                "bonuses",
                "employment income",
            ],
        ),
    ),
    credit(
        g(
            "revenus.allocations",
            "Allocations & aides",
            &["allocations", "caf", "aides", "chomage", "prestations"],
        )
        .en(
            "Benefits & allowances",
            &[
                "benefits",
                "government benefits",
                "allowances",
                "child benefit",
                "unemployment",
                "welfare",
                "universal credit",
                "tax credits",
            ],
        ),
    ),
    credit(
        g(
            "revenus.retraites",
            "Retraites & pensions",
            &["retraite", "retraites", "pension", "pensions"],
        )
        .en(
            "Pensions & retirement",
            &[
                "pension",
                "pensions",
                "retirement",
                "retirement income",
                "social security",
                "annuity income",
            ],
        ),
    ),
    credit(
        g(
            "revenus.sante",
            "Remboursements santé",
            &[
                "remboursements sante",
                "rbt sante",
                "secu",
                "remboursement sante",
            ],
        )
        .en(
            "Health reimbursements",
            &[
                "health reimbursements",
                "medical reimbursements",
                "insurance reimbursements",
                "health refunds",
            ],
        ),
    ),
    credit(
        g(
            "revenus.locatifs",
            "Revenus locatifs",
            &["revenus locatifs", "loyers percus", "location"],
        )
        .en(
            "Rental income",
            &[
                "rental income",
                "rent received",
                "rents received",
                "rental property",
            ],
        ),
    ),
    credit(
        g(
            "revenus.financiers",
            "Revenus financiers",
            &["revenus financiers", "interets", "dividendes"],
        )
        .en(
            "Investment income",
            &[
                "investment income",
                "interest",
                "dividends",
                "interest income",
                "capital gains",
            ],
        ),
    ),
    credit(
        g(
            "revenus.autres",
            "Autres revenus",
            &["autres revenus", "remboursements", "ventes", "divers"],
        )
        .en(
            "Other income",
            &[
                "other income",
                "refunds",
                "reimbursements",
                "sales",
                "gifts received",
                "miscellaneous income",
                "cashback",
            ],
        ),
    ),
    g("divers", "Divers", &["divers", "autres"]).en(
        "Miscellaneous",
        &["miscellaneous", "misc", "other", "uncategorized"],
    ),
    g(
        "divers.animaux",
        "Animaux",
        &["animaux", "veterinaire", "chien", "chat"],
    )
    .en(
        "Pets",
        &["pets", "pet", "pet care", "veterinary", "vet", "dog", "cat"],
    ),
    g("divers.cadeaux", "Cadeaux", &["cadeaux", "cadeau"])
        .en("Gifts", &["gifts", "gift", "presents", "flowers"]),
    g(
        "divers.dons",
        "Dons & associations",
        &["dons", "associations", "dons associations", "charite"],
    )
    .en(
        "Charity & donations",
        &[
            "charity",
            "charities",
            "donations",
            "donation",
            "charitable giving",
            "giving",
            "church",
            "tithe",
        ],
    ),
    g(
        "divers.tabac",
        "Tabac & presse",
        &["tabac", "presse", "jeux d argent"],
    )
    .en(
        "Tobacco & newsstand",
        &[
            "tobacco",
            "cigarettes",
            "vaping",
            "lottery",
            "gambling",
            "newsagent",
            "newsstand",
        ],
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
    )
    .en(
        "Internal transfers",
        &[
            "internal transfers",
            "internal transfer",
            "transfers",
            "transfer",
            "account transfers",
            "transfers between accounts",
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

/// Catégories standard dans la langue en cours, au format de
/// `manage::import_categories` : une catégorie par ligne, sous-catégories en
/// retrait.
pub fn standard_categories_text() -> String {
    GENERICS
        .iter()
        .map(|g| {
            if g.slug.contains('.') {
                format!("     {}", g.local_name())
            } else {
                g.local_name().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Catégories standard dans la langue en cours, numérotées dans l'ordre.
pub fn standard_categories() -> Vec<Category> {
    GENERICS
        .iter()
        .enumerate()
        .map(|(id, g)| Category {
            id: id as u32,
            name: g.local_name().to_string(),
            is_child: g.slug.contains('.'),
        })
        .collect()
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
    fn every_generic_has_distinct_names_in_both_languages() {
        for lang in Lang::ALL {
            let mut seen = std::collections::HashSet::new();
            for g in GENERICS {
                let name = g.name_in(lang);
                assert!(!name.is_empty(), "{}", g.slug);
                assert!(seen.insert(name), "{name} en double");
            }
        }
        for g in GENERICS {
            for s in g.synonyms.iter().chain(g.synonyms_en) {
                assert!(
                    s.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == ' '),
                    "synonyme non normalisé : {s}"
                );
            }
        }
    }

    #[test]
    fn standard_list_follows_the_language() {
        let fr = standard_categories_text();
        assert!(fr.starts_with("Alimentation\n     Courses\n"), "{fr}");
        let en = crate::i18n::with_lang(Lang::En, standard_categories_text);
        assert!(en.starts_with("Food\n     Groceries\n"), "{en}");
        assert!(en.contains("\n     Mortgage\n"), "{en}");
        let list = crate::i18n::with_lang(Lang::En, standard_categories);
        assert_eq!(list.len(), GENERICS.len());
        assert_eq!(list[5].name, "Rent");
    }

    #[test]
    fn reasonable_size() {
        let parents = GENERICS.iter().filter(|g| !g.slug.contains('.')).count();
        assert!((12..=16).contains(&parents));
        assert!((55..=70).contains(&(GENERICS.len() - parents)));
    }
}
