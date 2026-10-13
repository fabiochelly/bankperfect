//! Génère un fichier de démonstration réaliste (foyer fictif, trois ans
//! d'historique jusqu'à un mois après aujourd'hui), pour les captures d'écran
//! de la documentation. Aucune donnée personnelle.
//! Usage : cargo run -p bp-core --example gen_demo -- <sortie.bp>

use bp_core::format_v6::Protection;
use bp_core::model::*;
use chrono::{Datelike, NaiveDate};

/// Générateur pseudo-aléatoire déterministe (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Nombre dans [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.next() as usize % items.len()]
    }
}

/// Catégories : parent suivi de ses sous-catégories.
const CATEGORIES: &[(&str, &[&str])] = &[
    ("Alimentation", &["Courses", "Boulangerie", "Restaurant"]),
    (
        "Logement",
        &["Loyer", "Électricité", "Internet", "Assurance habitation"],
    ),
    ("Transport", &["Carburant", "Transports en commun", "Train"]),
    ("Loisirs", &["Cinéma", "Abonnements", "Vacances", "Sport"]),
    ("Santé", &["Pharmacie", "Médecin", "Mutuelle"]),
    ("Shopping", &["Vêtements", "High-tech", "Cadeaux"]),
    ("Revenus", &["Salaire", "Remboursements", "Intérêts"]),
    ("Épargne", &[]),
    ("Banque", &["Frais bancaires", "Retraits"]),
    ("Téléphone", &[]),
];

fn categories() -> Vec<Category> {
    let mut out = Vec::new();
    for (parent, children) in CATEGORIES {
        out.push(Category {
            id: out.len() as u32,
            name: parent.to_string(),
            is_child: false,
        });
        for child in *children {
            out.push(Category {
                id: out.len() as u32,
                name: child.to_string(),
                is_child: true,
            });
        }
    }
    out
}

/// Opérations d'un compte en cours de construction.
struct Ledger<'a> {
    categories: &'a [Category],
    ops: Vec<Operation>,
}

impl Ledger<'_> {
    fn category(&self, name: &str) -> u32 {
        self.categories
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("catégorie {name}"))
            .id
    }

    fn add(
        &mut self,
        date: Date,
        mode: StandardMode,
        payee: &str,
        details: &str,
        category: &str,
        euros: f64,
    ) {
        let cents = (euros * 100.0).round() as i64;
        self.ops.push(Operation {
            uid: 0,
            date,
            mark_date: date,
            tack: Tack::None,
            mode: Mode::Standard(mode),
            payee: payee.to_string(),
            details: details.to_string(),
            category: Some(self.category(category)),
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        });
    }

    fn balance(&self) -> f64 {
        self.ops.iter().map(|op| op.amount.as_f64()).sum()
    }
}

const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

fn day(year: i32, month: u32, day: u32) -> Date {
    // Le 31 d'un mois de 30 jours devient le dernier jour du mois.
    (0..4)
        .find_map(|k| Date::from_ymd(year, month, day.saturating_sub(k).max(1)))
        .expect("date valide")
}

/// Le jour ouvré le plus proche avant (le salaire tombe en semaine).
fn business_day(date: Date) -> Date {
    let mut d = date;
    while d.to_naive().weekday().num_days_from_monday() >= 5 {
        d = Date(d.0 - 1);
    }
    d
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).expect("chemin de sortie");
    let categories = categories();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    let today = Date::today();
    let end = Date(today.0 + 35);
    let t = today.to_naive();
    let first = NaiveDate::from_ymd_opt(t.year() - 3, t.month(), 1).expect("date valide");
    let start = Date::from_naive(first);

    let mut current = Ledger {
        categories: &categories,
        ops: Vec::new(),
    };
    let mut joint = Ledger {
        categories: &categories,
        ops: Vec::new(),
    };
    let mut savings = Ledger {
        categories: &categories,
        ops: Vec::new(),
    };
    current.add(
        start,
        StandardMode::Versement,
        "Solde à l'ouverture",
        "",
        "Revenus",
        2_480.35,
    );
    joint.add(
        start,
        StandardMode::Versement,
        "Solde à l'ouverture",
        "",
        "Revenus",
        1_215.60,
    );
    savings.add(
        start,
        StandardMode::Versement,
        "Solde à l'ouverture",
        "",
        "Épargne",
        3_400.00,
    );

    let mut cheque = 4_512_300u32;
    let (mut year, mut month) = (first.year(), first.month());
    loop {
        let month_start = day(year, month, 1);
        if month_start > end {
            break;
        }
        let years = (year - first.year()) as f64 + (month as f64 - 1.0) / 12.0;
        let inflation = 1.02f64.powf(years);
        let in_range = |d: Date| d >= start && d <= end;
        let summer = month == 7 || month == 8;
        let december = month == 12;

        // ── Revenus et charges fixes du compte courant ──
        let salary_day = business_day(day(year, month, 27));
        if in_range(salary_day) {
            let salary = 3_050.0 * 1.025f64.powi(year - first.year());
            current.add(
                salary_day,
                StandardMode::VirementRecu,
                "Société Exemple SA",
                &format!("Salaire {} {year}", MONTHS[month as usize - 1]),
                "Salaire",
                (salary * 100.0).round() / 100.0,
            );
            if december {
                current.add(
                    salary_day,
                    StandardMode::VirementRecu,
                    "Société Exemple SA",
                    "Prime de fin d'année",
                    "Salaire",
                    (salary * 0.55).round(),
                );
            }
        }
        let fixed: &[(u32, StandardMode, &str, &str, &str, f64)] = &[
            (
                5,
                StandardMode::Prelevement,
                "Agence du Parc",
                "Loyer et charges",
                "Loyer",
                -1_190.0 * 1.03f64.powi(year - first.year()),
            ),
            (
                10,
                StandardMode::Prelevement,
                "EDF",
                "Mensualité électricité",
                "Électricité",
                -(74.0 * inflation).round(),
            ),
            (
                14,
                StandardMode::Prelevement,
                "Free",
                "Freebox",
                "Internet",
                -29.99,
            ),
            (
                18,
                StandardMode::Prelevement,
                "Bouygues Telecom",
                "Forfait mobile",
                "Téléphone",
                -15.99,
            ),
            (
                6,
                StandardMode::Prelevement,
                "Mutuelle Horizon",
                "Cotisation mensuelle",
                "Mutuelle",
                -41.80,
            ),
            (
                3,
                StandardMode::Prelevement,
                "Assurances du Centre",
                "Multirisque habitation",
                "Assurance habitation",
                -18.45,
            ),
            (
                2,
                StandardMode::Prelevement,
                "Navigo",
                "Forfait mensuel",
                "Transports en commun",
                -86.40,
            ),
            (
                12,
                StandardMode::Carte,
                "Netflix",
                "Abonnement",
                "Abonnements",
                -13.49,
            ),
            (
                21,
                StandardMode::Carte,
                "Spotify",
                "Abonnement",
                "Abonnements",
                -11.12,
            ),
            (
                8,
                StandardMode::Prelevement,
                "Banque Exemple",
                "Cotisation carte",
                "Frais bancaires",
                -2.95,
            ),
            (
                1,
                StandardMode::VirementEmis,
                "Compte joint",
                "Participation au foyer",
                "Courses",
                -330.0,
            ),
        ];
        for (d, mode, payee, details, category, amount) in fixed {
            let date = day(year, month, *d);
            if in_range(date) {
                current.add(date, *mode, payee, details, category, *amount);
            }
        }
        if month == 9 {
            let date = day(year, month, 20);
            if in_range(date) {
                current.add(
                    date,
                    StandardMode::Carte,
                    "Club de sport",
                    "Abonnement annuel",
                    "Sport",
                    -360.0,
                );
            }
        }

        // ── Épargne : versement mensuel, plus un surplus si le compte est bien garni ──
        let date = day(year, month, 2);
        if in_range(date) && date <= today {
            let mut amount = 200.0;
            if current.balance() > 20_000.0 {
                amount += 1_500.0;
            }
            current.add(
                date,
                StandardMode::VirementEmis,
                "Épargne",
                "Épargne mensuelle",
                "Épargne",
                -amount,
            );
            savings.add(
                date,
                StandardMode::VirementRecu,
                "Compte courant",
                "Épargne mensuelle",
                "Épargne",
                amount,
            );
        }
        if month == 12 {
            let date = day(year, 12, 31);
            if in_range(date) {
                let interest = (savings.balance() * 0.026 * 100.0).round() / 100.0;
                savings.add(
                    date,
                    StandardMode::VirementRecu,
                    "Banque Exemple",
                    &format!("Intérêts {year}"),
                    "Intérêts",
                    interest,
                );
            }
        }

        // ── Dépenses courantes, jour par jour ──
        let days = (0..31).map(|k| Date(month_start.0 + k)).filter(|d| {
            let n = d.to_naive();
            n.month() == month && *d <= today
        });
        for date in days.collect::<Vec<_>>() {
            let weekday = date.to_naive().weekday().num_days_from_monday();
            let weekend = weekday >= 5;
            if rng.chance(if weekend { 0.55 } else { 0.35 }) {
                let shop = rng.pick(&["Boulangerie Paul", "Maison Lenôtre", "Au Bon Pain"]);
                current.add(
                    date,
                    StandardMode::Carte,
                    shop,
                    "",
                    "Boulangerie",
                    -rng.range(1.2, 11.5),
                );
            }
            if rng.chance(if weekend { 0.30 } else { 0.10 }) {
                let shop = rng.pick(&["Monoprix", "Franprix", "Biocoop"]);
                current.add(
                    date,
                    StandardMode::Carte,
                    shop,
                    "",
                    "Courses",
                    -rng.range(9.0, 48.0) * inflation,
                );
            }
            if weekday == 5 && rng.chance(0.85) {
                let shop = rng.pick(&["Carrefour", "Leclerc", "Intermarché"]);
                joint.add(
                    date,
                    StandardMode::Carte,
                    shop,
                    "Courses de la semaine",
                    "Courses",
                    -rng.range(65.0, 165.0) * inflation,
                );
            }
            if rng.chance(if summer { 0.22 } else { 0.11 }) {
                let place = rng.pick(&[
                    "Le Petit Bistrot",
                    "Pizzeria Napoli",
                    "Sushi Bar",
                    "Café de la Gare",
                    "Brasserie du Marché",
                ]);
                current.add(
                    date,
                    StandardMode::Carte,
                    place,
                    "",
                    "Restaurant",
                    -rng.range(14.0, 68.0),
                );
            }
            if rng.chance(0.06) {
                current.add(
                    date,
                    StandardMode::Carte,
                    "TotalEnergies",
                    "",
                    "Carburant",
                    -rng.range(42.0, 78.0),
                );
            }
            if rng.chance(if december { 0.18 } else { 0.06 }) {
                let (shop, category) = *rng.pick(&[
                    ("Amazon", "High-tech"),
                    ("Fnac", "Cadeaux"),
                    ("Decathlon", "Sport"),
                    ("Uniqlo", "Vêtements"),
                    ("Zara", "Vêtements"),
                ]);
                let high = if december { 140.0 } else { 85.0 };
                current.add(
                    date,
                    StandardMode::Carte,
                    shop,
                    "",
                    category,
                    -rng.range(12.0, high),
                );
            }
            if weekend && rng.chance(0.18) {
                current.add(
                    date,
                    StandardMode::Carte,
                    "Cinéma Le Grand Rex",
                    "2 places",
                    "Cinéma",
                    -23.80,
                );
            }
            if rng.chance(0.035) {
                current.add(
                    date,
                    StandardMode::Carte,
                    "Pharmacie du Centre",
                    "",
                    "Pharmacie",
                    -rng.range(6.5, 38.0),
                );
            }
            if rng.chance(0.07) {
                let amount = *rng.pick(&[40.0, 50.0, 60.0, 80.0]);
                current.add(
                    date,
                    StandardMode::RetraitDab,
                    "Retrait DAB",
                    "Paris 11e",
                    "Retraits",
                    -amount,
                );
            }
            if rng.chance(0.012) {
                cheque += 1;
                current.add(
                    date,
                    StandardMode::ChequeEmis,
                    "Cabinet médical",
                    &format!("Chèque n° {cheque}"),
                    "Médecin",
                    -30.0,
                );
                let refund = Date(date.0 + rng.range(3.0, 8.0) as i32);
                if refund <= today {
                    current.add(
                        refund,
                        StandardMode::VirementRecu,
                        "CPAM",
                        "Remboursement soins",
                        "Remboursements",
                        21.0,
                    );
                }
            }
        }

        // ── Compte joint : participations et vacances ──
        let date = day(year, month, 1);
        if in_range(date) {
            joint.add(
                date,
                StandardMode::VirementRecu,
                "Compte courant",
                "Participation au foyer",
                "Revenus",
                330.0,
            );
            joint.add(
                date,
                StandardMode::VirementRecu,
                "Alex Martin",
                "Participation au foyer",
                "Revenus",
                330.0,
            );
        }
        if month == 7 {
            let date = day(year, 7, 4);
            if in_range(date) && date <= today {
                joint.add(
                    date,
                    StandardMode::Carte,
                    "SNCF Connect",
                    "Paris – Marseille",
                    "Train",
                    -rng.range(160.0, 240.0),
                );
                joint.add(
                    Date(date.0 + 2),
                    StandardMode::Carte,
                    "Location Les Pins",
                    "Vacances d'été",
                    "Vacances",
                    -rng.range(820.0, 1_150.0),
                );
            }
        }

        if month == 12 {
            year += 1;
            month = 1;
        } else {
            month += 1;
        }
    }

    // Pointage : rapproché au-delà de 45 jours, mélangé ensuite, rien dans le futur.
    let mut uid = 0u64;
    let mut finish = |ledger: Ledger, rng: &mut Rng| -> Vec<Operation> {
        let mut ops = ledger.ops;
        ops.sort_by_key(|op| op.date);
        for op in &mut ops {
            uid += 1;
            op.uid = uid;
            let age = today.0 - op.date.0;
            op.tack = if age > 45 {
                Tack::Reconciled
            } else if age > 10 && rng.chance(0.85) || age >= 0 && rng.chance(0.35) {
                Tack::Pointed
            } else {
                Tack::None
            };
        }
        ops
    };
    let current_ops = finish(current, &mut rng);
    let joint_ops = finish(joint, &mut rng);
    let savings_ops = finish(savings, &mut rng);

    let account = |name: &str, number: &str, iban: &str, ops: Vec<Operation>| Account {
        uid: bp_core::v5_text::new_account_uid(0),
        name: name.to_string(),
        bank_name: "Banque Exemple".into(),
        agency_name: "Agence Bastille".into(),
        agency_address: String::new(),
        number: number.into(),
        bank_code: "30001".into(),
        agency_code: "00011".into(),
        bic: String::new(),
        iban: iban.into(),
        logo: "Aucun".into(),
        currency: "€".into(),
        currency_rate: 1.0,
        secondary_currency: "F".into(),
        secondary_rate: 6.55957,
        is_virtual: false,
        disabled: false,
        operations: ops,
    };
    let document = Document {
        group_name: "Famille Martin".into(),
        guid: bp_core::v5_text::new_file_guid(),
        user_name: "Camille Martin".into(),
        custom_modes: Vec::new(),
        accounts: vec![
            account(
                "Compte courant",
                "00012345601",
                "FR76 3000 1000 1100 0123 4560 142",
                current_ops,
            ),
            account(
                "Compte joint",
                "00012345602",
                "FR76 3000 1000 1100 0123 4560 239",
                joint_ops,
            ),
            account(
                "Épargne",
                "00012345603",
                "FR76 3000 1000 1100 0123 4560 336",
                savings_ops,
            ),
        ],
        account_order: vec![0, 1, 2],
        default_account: 0,
        categories,
        payees: Vec::new(),
        feature_data: Vec::new(),
        unknown_header: Vec::new(),
    };
    bp_core::file::save(out.as_ref(), &document, &Protection::none().unwrap()).unwrap();

    for acc in &document.accounts {
        let total: f64 = acc.operations.iter().map(|op| op.amount.as_f64()).sum();
        let pending = acc
            .operations
            .iter()
            .filter(|op| op.tack == Tack::None)
            .count();
        println!(
            "{:<15} {:>5} opérations  solde {:>10.2} €  {pending} non pointées",
            acc.name,
            acc.operations.len(),
            total
        );
    }
}
