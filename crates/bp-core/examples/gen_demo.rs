//! Génère un fichier de démonstration réaliste (foyer fictif, trois ans
//! d'historique jusqu'à un mois après aujourd'hui), pour les captures d'écran
//! de la documentation. Aucune donnée personnelle.
//!
//! Usage : cargo run -p bp-core --example gen_demo -- [--lang en] <sortie.bp>
//!
//! - français (par défaut) : foyer Martin à Paris, comptes en euros ;
//! - `--lang en` : foyer américain (Miller, Chicago), comptes en dollars et
//!   catégories standard anglaises, pour les captures de la version anglaise.
//!
//! Les deux variantes suivent le même scénario (mêmes tirages) et remplissent
//! les mêmes écrans : opérations pointées et rapprochées, échéancier, budget
//! mensuel, budget annuel et règles automatiques.

use bp_core::autofields::{AutoFieldRules, CategoryChange, Rule, RuleField};
use bp_core::budget::{AnnualBudget, AnnualBudgets, AnnualLine, MonthlyBudget};
use bp_core::format_v6::Protection;
use bp_core::i18n::Lang;
use bp_core::model::*;
use bp_core::scheduler::{self, Period, PeriodUnit};
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

/// Catégories de la démo française : parent suivi de ses sous-catégories.
const CATEGORIES_FR: &[(&str, &[&str])] = &[
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

/// Catégories : la liste de la démo française, ou la liste standard anglaise
/// (celle que reçoit un nouveau fichier créé en anglais).
fn categories(lang: Lang) -> Vec<Category> {
    if lang == Lang::En {
        return bp_core::i18n::with_lang(
            Lang::En,
            bp_core::categorize::taxonomy::standard_categories,
        );
    }
    let mut out = Vec::new();
    for (parent, children) in CATEGORIES_FR {
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

fn category_id(categories: &[Category], name: &str) -> u32 {
    categories
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("catégorie {name}"))
        .id
}

/// Opérations d'un compte en cours de construction.
struct Ledger<'a> {
    categories: &'a [Category],
    ops: Vec<Operation>,
}

impl Ledger<'_> {
    fn add(
        &mut self,
        date: Date,
        mode: StandardMode,
        payee: &str,
        details: &str,
        category: &str,
        amount: f64,
    ) {
        let cents = (amount * 100.0).round() as i64;
        self.ops.push(Operation {
            uid: 0,
            date,
            mark_date: date,
            tack: Tack::None,
            mode: Mode::Standard(mode),
            payee: payee.to_string(),
            details: details.to_string(),
            category: Some(category_id(self.categories, category)),
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        });
    }

    fn balance(&self) -> f64 {
        self.ops.iter().map(|op| op.amount.as_f64()).sum()
    }
}

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

/// Arguments : `[--lang fr|en] <sortie.bp>`.
fn parse_args() -> (Lang, String) {
    let mut lang = Lang::Fr;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let code = match arg.strip_prefix("--lang") {
            Some("") => args.next(),
            Some(rest) => rest.strip_prefix('=').map(str::to_string),
            None => {
                out = Some(arg);
                continue;
            }
        };
        lang = code
            .as_deref()
            .and_then(Lang::from_code)
            .unwrap_or_else(|| panic!("langue inconnue : {code:?} (fr ou en)"));
    }
    (
        lang,
        out.expect("usage : gen_demo [--lang fr|en] <sortie.bp>"),
    )
}

fn main() {
    let (lang, out) = parse_args();
    let en = lang == Lang::En;
    // Texte (ou valeur) de la variante choisie : `s(français, anglais)`.
    let s = |fr: &'static str, english: &'static str| if en { english } else { fr };
    let n = |fr: f64, english: f64| if en { english } else { fr };
    let categories = categories(lang);
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
    let opening = s("Solde à l'ouverture", "Opening balance");
    let income = s("Revenus", "Income");
    current.add(
        start,
        StandardMode::Versement,
        opening,
        "",
        income,
        n(2_480.35, 3_215.40),
    );
    joint.add(
        start,
        StandardMode::Versement,
        opening,
        "",
        income,
        n(1_215.60, 1_540.25),
    );
    savings.add(
        start,
        StandardMode::Versement,
        opening,
        "",
        s("Épargne", "Savings accounts"),
        n(3_400.00, 6_800.00),
    );

    let mut cheque: u32 = if en { 1_040 } else { 4_512_300 };
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
        let month_name =
            bp_core::i18n::with_lang(lang, || bp_core::i18n::month_name(month as usize - 1));

        // ── Revenus et charges fixes du compte courant ──
        let salary_day = business_day(day(year, month, 27));
        if in_range(salary_day) {
            let salary = n(3_050.0, 4_860.0) * 1.025f64.powi(year - first.year());
            let employer = s("Société Exemple SA", "Example Corp");
            current.add(
                salary_day,
                StandardMode::VirementRecu,
                employer,
                &if en {
                    format!("Payroll {month_name} {year}")
                } else {
                    format!("Salaire {month_name} {year}")
                },
                s("Salaire", "Salary & bonuses"),
                (salary * 100.0).round() / 100.0,
            );
            if december {
                current.add(
                    salary_day,
                    StandardMode::VirementRecu,
                    employer,
                    s("Prime de fin d'année", "Year-end bonus"),
                    s("Salaire", "Salary & bonuses"),
                    (salary * 0.55).round(),
                );
            }
        }
        let fixed: &[(u32, StandardMode, &str, &str, &str, f64)] = &[
            (
                5,
                StandardMode::Prelevement,
                s("Agence du Parc", "Lakeview Apartments"),
                s("Loyer et charges", "Monthly rent"),
                s("Loyer", "Rent"),
                -n(1_190.0, 1_850.0) * 1.03f64.powi(year - first.year()),
            ),
            (
                10,
                StandardMode::Prelevement,
                s("EDF", "ComEd"),
                s("Mensualité électricité", "Electric bill"),
                s("Électricité", "Electricity & gas"),
                -(n(74.0, 92.0) * inflation).round(),
            ),
            (
                14,
                StandardMode::Prelevement,
                s("Free", "Xfinity"),
                s("Freebox", "Home internet"),
                s("Internet", "Internet & phone"),
                -n(29.99, 65.00),
            ),
            (
                18,
                StandardMode::Prelevement,
                s("Bouygues Telecom", "Verizon Wireless"),
                s("Forfait mobile", "Mobile plan"),
                s("Téléphone", "Internet & phone"),
                -n(15.99, 70.00),
            ),
            (
                6,
                StandardMode::Prelevement,
                s("Mutuelle Horizon", "Blue Cross Blue Shield"),
                s("Cotisation mensuelle", "Monthly premium"),
                s("Mutuelle", "Health insurance"),
                -n(41.80, 186.40),
            ),
            (
                3,
                StandardMode::Prelevement,
                s("Assurances du Centre", "State Farm"),
                s("Multirisque habitation", "Renters insurance"),
                s("Assurance habitation", "Home insurance"),
                -n(18.45, 16.25),
            ),
            (
                2,
                StandardMode::Prelevement,
                s("Navigo", "Ventra"),
                s("Forfait mensuel", "30-day pass"),
                s("Transports en commun", "Public transit"),
                -n(86.40, 75.00),
            ),
            (
                12,
                StandardMode::Carte,
                "Netflix",
                s("Abonnement", "Subscription"),
                s("Abonnements", "Streaming & news"),
                -n(13.49, 15.49),
            ),
            (
                21,
                StandardMode::Carte,
                "Spotify",
                s("Abonnement", "Subscription"),
                s("Abonnements", "Streaming & news"),
                -n(11.12, 11.99),
            ),
            (
                8,
                StandardMode::Prelevement,
                s("Banque Exemple", "Example Bank"),
                s("Cotisation carte", "Monthly service fee"),
                s("Frais bancaires", "Bank fees"),
                -n(2.95, 5.00),
            ),
            (
                1,
                StandardMode::VirementEmis,
                s("Compte joint", "Joint Checking"),
                s("Participation au foyer", "Household contribution"),
                s("Courses", "Internal transfers"),
                -n(330.0, 450.0),
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
                    s("Club de sport", "Planet Fitness"),
                    s("Abonnement annuel", "Annual membership"),
                    s("Sport", "Sports"),
                    -n(360.0, 299.0),
                );
            }
        }

        // ── Épargne : versement mensuel, plus un surplus si le compte est bien garni ──
        let date = day(year, month, 2);
        if in_range(date) && date <= today {
            let mut amount = n(200.0, 400.0);
            if current.balance() > n(20_000.0, 25_000.0) {
                amount += n(1_500.0, 2_000.0);
            }
            let category = s("Épargne", "Savings accounts");
            current.add(
                date,
                StandardMode::VirementEmis,
                s("Épargne", "Savings"),
                s("Épargne mensuelle", "Monthly savings"),
                category,
                -amount,
            );
            savings.add(
                date,
                StandardMode::VirementRecu,
                s("Compte courant", "Checking"),
                s("Épargne mensuelle", "Monthly savings"),
                category,
                amount,
            );
        }
        if month == 12 {
            let date = day(year, 12, 31);
            if in_range(date) {
                let interest = (savings.balance() * n(0.026, 0.0385) * 100.0).round() / 100.0;
                savings.add(
                    date,
                    StandardMode::VirementRecu,
                    s("Banque Exemple", "Example Bank"),
                    &if en {
                        format!("Interest {year}")
                    } else {
                        format!("Intérêts {year}")
                    },
                    s("Intérêts", "Investment income"),
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
                let shop = rng.pick(if en {
                    &["Starbucks", "Dunkin'", "Corner Bakery Cafe"]
                } else {
                    &["Boulangerie Paul", "Maison Lenôtre", "Au Bon Pain"]
                });
                current.add(
                    date,
                    StandardMode::Carte,
                    shop,
                    "",
                    s("Boulangerie", "Bakery & market"),
                    -rng.range(1.2, 11.5),
                );
            }
            if rng.chance(if weekend { 0.30 } else { 0.10 }) {
                let shop = rng.pick(if en {
                    &["Trader Joe's", "Whole Foods Market", "Walgreens"]
                } else {
                    &["Monoprix", "Franprix", "Biocoop"]
                });
                current.add(
                    date,
                    StandardMode::Carte,
                    shop,
                    "",
                    s("Courses", "Groceries"),
                    -rng.range(9.0, 48.0) * inflation,
                );
            }
            if weekday == 5 && rng.chance(0.85) {
                let shop = rng.pick(if en {
                    &["Costco", "Jewel-Osco", "Target"]
                } else {
                    &["Carrefour", "Leclerc", "Intermarché"]
                });
                joint.add(
                    date,
                    StandardMode::Carte,
                    shop,
                    s("Courses de la semaine", "Weekly groceries"),
                    s("Courses", "Groceries"),
                    -rng.range(65.0, 165.0) * n(1.0, 1.3) * inflation,
                );
            }
            if rng.chance(if summer { 0.22 } else { 0.11 }) {
                let place = rng.pick(if en {
                    &[
                        "Chipotle",
                        "Lou Malnati's Pizzeria",
                        "Sushi Station",
                        "Shake Shack",
                        "The Corner Diner",
                    ]
                } else {
                    &[
                        "Le Petit Bistrot",
                        "Pizzeria Napoli",
                        "Sushi Bar",
                        "Café de la Gare",
                        "Brasserie du Marché",
                    ]
                });
                current.add(
                    date,
                    StandardMode::Carte,
                    place,
                    "",
                    s("Restaurant", "Restaurants"),
                    -rng.range(14.0, 68.0),
                );
            }
            if rng.chance(0.06) {
                current.add(
                    date,
                    StandardMode::Carte,
                    s("TotalEnergies", "Shell"),
                    "",
                    s("Carburant", "Fuel"),
                    -rng.range(42.0, 78.0),
                );
            }
            if rng.chance(if december { 0.18 } else { 0.06 }) {
                let (shop, category) = *rng.pick(if en {
                    &[
                        ("Amazon", "Electronics & appliances"),
                        ("Barnes & Noble", "Gifts"),
                        ("REI", "Sports"),
                        ("Old Navy", "Clothing & shoes"),
                        ("Target", "Clothing & shoes"),
                    ]
                } else {
                    &[
                        ("Amazon", "High-tech"),
                        ("Fnac", "Cadeaux"),
                        ("Decathlon", "Sport"),
                        ("Uniqlo", "Vêtements"),
                        ("Zara", "Vêtements"),
                    ]
                });
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
                    s("Cinéma Le Grand Rex", "AMC Theatres"),
                    s("2 places", "2 tickets"),
                    s("Cinéma", "Entertainment & events"),
                    -n(23.80, 31.50),
                );
            }
            if rng.chance(0.035) {
                current.add(
                    date,
                    StandardMode::Carte,
                    s("Pharmacie du Centre", "CVS Pharmacy"),
                    "",
                    s("Pharmacie", "Pharmacy"),
                    -rng.range(6.5, 38.0),
                );
            }
            if rng.chance(0.07) {
                let amount = *rng.pick(&[40.0, 50.0, 60.0, 80.0]);
                current.add(
                    date,
                    StandardMode::RetraitDab,
                    s("Retrait DAB", "ATM withdrawal"),
                    s("Paris 11e", "Chicago, IL"),
                    s("Retraits", "Cash withdrawals"),
                    -amount,
                );
            }
            if rng.chance(0.012) {
                cheque += 1;
                current.add(
                    date,
                    StandardMode::ChequeEmis,
                    s("Cabinet médical", "Lakeshore Medical Group"),
                    &if en {
                        format!("Check #{cheque}")
                    } else {
                        format!("Chèque n° {cheque}")
                    },
                    s("Médecin", "Doctors & tests"),
                    -n(30.0, 40.0),
                );
                let refund = Date(date.0 + rng.range(3.0, 8.0) as i32);
                if refund <= today {
                    current.add(
                        refund,
                        StandardMode::VirementRecu,
                        s("CPAM", "Blue Cross Blue Shield"),
                        s("Remboursement soins", "Claim reimbursement"),
                        s("Remboursements", "Health reimbursements"),
                        n(21.0, 28.0),
                    );
                }
            }
        }

        // ── Compte joint : participations et vacances ──
        let date = day(year, month, 1);
        if in_range(date) {
            let contribution = n(330.0, 450.0);
            let details = s("Participation au foyer", "Household contribution");
            joint.add(
                date,
                StandardMode::VirementRecu,
                s("Compte courant", "Checking"),
                details,
                s("Revenus", "Internal transfers"),
                contribution,
            );
            joint.add(
                date,
                StandardMode::VirementRecu,
                s("Alex Martin", "Taylor Miller"),
                details,
                s("Revenus", "Other income"),
                contribution,
            );
        }
        if month == 7 {
            let date = day(year, 7, 4);
            if in_range(date) && date <= today {
                joint.add(
                    date,
                    StandardMode::Carte,
                    s("SNCF Connect", "United Airlines"),
                    s("Paris – Marseille", "Chicago – Denver"),
                    s("Train", "Trains, flights & car rental"),
                    -rng.range(160.0, 240.0) * n(1.0, 2.2),
                );
                joint.add(
                    Date(date.0 + 2),
                    StandardMode::Carte,
                    s("Location Les Pins", "Airbnb"),
                    s("Vacances d'été", "Summer vacation"),
                    s("Vacances", "Lodging"),
                    -rng.range(820.0, 1_150.0) * n(1.0, 1.2),
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

    let account = |index: usize, name: &str, number: &str, iban: &str, ops: Vec<Operation>| {
        let mut account = Account {
            uid: bp_core::v5_text::new_account_uid(index),
            name: name.to_string(),
            bank_name: s("Banque Exemple", "Example Bank").into(),
            agency_name: s("Agence Bastille", "Lincoln Park branch").into(),
            agency_address: String::new(),
            number: number.into(),
            bank_code: s("30001", "").into(),
            agency_code: s("00011", "").into(),
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
        if en {
            // Dollar, avec l'euro en devise secondaire (taux indicatif).
            account.currency = "$".into();
            account.secondary_currency = "€".into();
            account.secondary_rate = 0.86;
        }
        account
    };
    let mut document = Document {
        group_name: s("Famille Martin", "The Miller Family").into(),
        guid: bp_core::v5_text::new_file_guid(),
        user_name: s("Camille Martin", "Jordan Miller").into(),
        custom_modes: Vec::new(),
        accounts: vec![
            account(
                0,
                s("Compte courant", "Checking"),
                s("00012345601", "4401 2345 6601"),
                s("FR76 3000 1000 1100 0123 4560 142", ""),
                current_ops,
            ),
            account(
                1,
                s("Compte joint", "Joint Checking"),
                s("00012345602", "4401 2345 6602"),
                s("FR76 3000 1000 1100 0123 4560 239", ""),
                joint_ops,
            ),
            account(
                2,
                s("Épargne", "Savings"),
                s("00012345603", "4401 2345 6603"),
                s("FR76 3000 1000 1100 0123 4560 336", ""),
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
    add_features(&mut document, lang, today, end);
    bp_core::file::save(out.as_ref(), &document, &Protection::none().unwrap()).unwrap();

    for acc in &document.accounts {
        let total: f64 = acc.operations.iter().map(|op| op.amount.as_f64()).sum();
        let pending = acc
            .operations
            .iter()
            .filter(|op| op.tack == Tack::None)
            .count();
        println!(
            "{:<15} {:>5} opérations  solde {:>10.2} {}  {pending} non pointées",
            acc.name,
            acc.operations.len(),
            total,
            acc.currency
        );
    }
}

/// Échéancier, budgets et règles automatiques, tirés des opérations générées.
fn add_features(document: &mut Document, lang: Lang, today: Date, end: Date) {
    let en = lang == Lang::En;
    let s = |fr: &'static str, english: &'static str| if en { english } else { fr };
    let categories = document.categories.clone();
    let id = |name: &str| category_id(&categories, name);

    // ── Échéancier : les charges fixes et le salaire, reprises de leur
    // dernière opération ; prochaine échéance après les opérations à venir
    // déjà présentes (rien n'est inséré en double à l'ouverture).
    let recurring: &[(usize, &str, PeriodUnit)] = &[
        (
            0,
            s("Société Exemple SA", "Example Corp"),
            PeriodUnit::Months,
        ),
        (
            0,
            s("Agence du Parc", "Lakeview Apartments"),
            PeriodUnit::Months,
        ),
        (0, s("EDF", "ComEd"), PeriodUnit::Months),
        (0, s("Free", "Xfinity"), PeriodUnit::Months),
        (
            0,
            s("Bouygues Telecom", "Verizon Wireless"),
            PeriodUnit::Months,
        ),
        (
            0,
            s("Mutuelle Horizon", "Blue Cross Blue Shield"),
            PeriodUnit::Months,
        ),
        (0, "Netflix", PeriodUnit::Months),
        (0, "Spotify", PeriodUnit::Months),
        (0, s("Épargne", "Savings"), PeriodUnit::Months),
        (0, s("Club de sport", "Planet Fitness"), PeriodUnit::Years),
        (1, s("Alex Martin", "Taylor Miller"), PeriodUnit::Months),
    ];
    let mut schedules = Vec::new();
    for &(account, payee, unit) in recurring {
        let Some(last) = document.accounts[account]
            .operations
            .iter()
            .filter(|op| op.payee == payee && op.amount.0.abs() < 500_000)
            .next_back()
        else {
            continue;
        };
        let mut schedule =
            scheduler::schedule_from_operation(document, account, last).expect("compte valide");
        schedule.period = Period { every: 1, unit };
        schedule.next_date = last.date;
        while schedule.next_date <= end {
            schedule.next_date =
                scheduler::add_period(schedule.next_date, schedule.period, schedule.anchor_day, 1)
                    .expect("date valide");
        }
        schedules.push(schedule);
    }
    scheduler::save(document, &schedules).unwrap();

    // ── Budget mensuel ──
    let monthly: &[(&str, f64)] = if en {
        &[
            ("Salary & bonuses", 5_150.0),
            ("Groceries", -620.0),
            ("Bakery & market", -110.0),
            ("Restaurants", -240.0),
            ("Rent", -1_960.0),
            ("Electricity & gas", -100.0),
            ("Internet & phone", -135.0),
            ("Public transit", -75.0),
            ("Fuel", -60.0),
            ("Streaming & news", -28.0),
            ("Entertainment & events", -50.0),
            ("Clothing & shoes", -80.0),
            ("Health insurance", -186.4),
            ("Pharmacy", -20.0),
            ("Cash withdrawals", -150.0),
            ("Savings accounts", -400.0),
        ]
    } else {
        &[
            ("Salaire", 3_200.0),
            ("Courses", -450.0),
            ("Boulangerie", -110.0),
            ("Restaurant", -180.0),
            ("Loyer", -1_270.0),
            ("Électricité", -80.0),
            ("Internet", -30.0),
            ("Téléphone", -16.0),
            ("Transports en commun", -86.4),
            ("Carburant", -60.0),
            ("Abonnements", -25.0),
            ("Cinéma", -40.0),
            ("Vêtements", -60.0),
            ("Mutuelle", -41.8),
            ("Pharmacie", -20.0),
            ("Retraits", -120.0),
            ("Épargne", -200.0),
        ]
    };
    let mut budget = MonthlyBudget::default();
    for &(name, amount) in monthly {
        budget.set_signed(Some(id(name)), Amount::from_cents((amount * 100.0) as i64));
    }
    budget.save(document).unwrap();

    // ── Budget annuel de l'année en cours ──
    let year = today.to_naive().year();
    let lines: &[(&str, &[&str], f64)] = if en {
        &[
            ("Salary", &["Salary & bonuses"], 5_150.0),
            (
                "Housing",
                &["Rent", "Electricity & gas", "Home insurance"],
                -2_080.0,
            ),
            (
                "Food",
                &["Groceries", "Bakery & market", "Restaurants"],
                -970.0,
            ),
            ("Getting around", &["Public transit", "Fuel"], -140.0),
            (
                "Health",
                &["Health insurance", "Pharmacy", "Doctors & tests"],
                -230.0,
            ),
            (
                "Leisure",
                &["Streaming & news", "Entertainment & events", "Sports"],
                -110.0,
            ),
        ]
    } else {
        &[
            ("Salaire", &["Salaire"], 3_200.0),
            (
                "Logement",
                &["Loyer", "Électricité", "Assurance habitation"],
                -1_370.0,
            ),
            (
                "Alimentation",
                &["Courses", "Boulangerie", "Restaurant"],
                -740.0,
            ),
            ("Transports", &["Transports en commun", "Carburant"], -150.0),
            ("Santé", &["Mutuelle", "Pharmacie", "Médecin"], -80.0),
            ("Loisirs", &["Abonnements", "Cinéma", "Sport"], -95.0),
        ]
    };
    let mut table = AnnualBudget::new(format!("Budget {year}"), year);
    for &(title, names, amount) in lines {
        table.lines.push(AnnualLine {
            title: title.to_string(),
            auto_real: true,
            categories: names.iter().map(|name| Some(id(name))).collect(),
            budget: [Amount::from_cents((amount * 100.0) as i64); 12],
            real: [Amount::ZERO; 12],
        });
    }
    AnnualBudgets {
        tables: vec![table],
    }
    .save(document)
    .unwrap();

    // ── Règles automatiques ──
    let rule = |contains: &str, mode: Option<StandardMode>, category: &str| Rule {
        field: RuleField::Payee,
        contains: contains.to_string(),
        plus_is_or: true,
        set_mode: mode,
        set_payee: None,
        set_details: None,
        set_date: None,
        set_category: Some(CategoryChange::Set(id(category))),
        only_uncategorized: true,
        active: true,
        auto_apply: true,
    };
    let rules = if en {
        vec![
            rule(
                "Netflix+Spotify",
                Some(StandardMode::Carte),
                "Streaming & news",
            ),
            rule("ATM", Some(StandardMode::RetraitDab), "Cash withdrawals"),
            rule("Shell", Some(StandardMode::Carte), "Fuel"),
            rule("CVS+Walgreens", None, "Pharmacy"),
        ]
    } else {
        vec![
            rule("Netflix+Spotify", Some(StandardMode::Carte), "Abonnements"),
            rule("Retrait DAB", Some(StandardMode::RetraitDab), "Retraits"),
            rule("TotalEnergies", Some(StandardMode::Carte), "Carburant"),
            rule("Pharmacie", None, "Pharmacie"),
        ]
    };
    AutoFieldRules { rules }.save(document).unwrap();
}
