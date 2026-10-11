//! Génère un fichier v6 synthétique volumineux et mesure la vitesse des filtres.
//! Usage : cargo run --release --example gen_large -- <sortie.bp> [opérations]

use std::time::Instant;

use bp_core::filter::{Filters, SearchIndex};
use bp_core::format_v6::Protection;
use bp_core::model::*;
use bp_core::view::{SortColumn, running_balances, sorted_order};

const PAYEES: &[&str] = &[
    "Carrefour",
    "Monoprix",
    "Franprix",
    "EDF",
    "Gaz de France",
    "SNCF",
    "Fnac",
    "Amazon.fr",
    "Leroy Merlin",
    "Pharmacie Centrale",
    "Boulangerie Paul",
    "Station Total",
    "Péage A6",
    "Orange",
    "SFR",
    "Trésor Public",
    "Mutuelle Bleue",
    "Salaire ACME",
    "Loyer",
    "Cinéma UGC",
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).expect("chemin de sortie");
    let count: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(160_000);

    let categories: Vec<Category> = [
        "Alimentation",
        "Epicerie",
        "Restaurant",
        "Habitation",
        "EDF",
        "Loyer",
        "Transport",
        "Essence",
        "Salaire",
    ]
    .iter()
    .enumerate()
    .map(|(i, name)| Category {
        id: i as u32,
        name: name.to_string(),
        is_child: ![0, 3, 6, 8].contains(&i),
    })
    .collect();

    let start = Date::from_ymd(1986, 1, 1).unwrap().0;
    let end = Date::today().0 + 60;
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut rand = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut operations = Vec::with_capacity(count);
    for i in 0..count {
        let date = Date(start + ((end - start) as u64 * i as u64 / count as u64) as i32);
        let payee = PAYEES[rand() as usize % PAYEES.len()];
        let mode = StandardMode::ALL[rand() as usize % 10];
        let cents = (rand() % if mode.is_debit() { 9_000 } else { 21_000 }) as i64 + 1;
        let amount = Amount::from_cents(if mode.is_debit() { -cents } else { cents });
        let tack = if date.0 < end - 90 {
            Tack::Reconciled
        } else if rand() % 2 == 0 {
            Tack::Pointed
        } else {
            Tack::None
        };
        operations.push(Operation {
            uid: i as u64 + 1,
            date,
            mark_date: date,
            tack,
            mode: Mode::Standard(mode),
            payee: payee.to_string(),
            details: if mode == StandardMode::ChequeEmis {
                format!("n°{}", 1_000_000 + i)
            } else {
                String::new()
            },
            category: Some(rand() as u32 % categories.len() as u32),
            amount,
            extra: Vec::new(),
        });
    }

    let account = |name: &str, ops: Vec<Operation>| Account {
        uid: bp_core::v5_text::new_account_uid(0),
        name: name.to_string(),
        bank_name: "Banque Exemple".into(),
        agency_name: String::new(),
        agency_address: String::new(),
        number: "00012345678".into(),
        bank_code: "30003".into(),
        agency_code: "03030".into(),
        bic: String::new(),
        iban: "FR76 3000 3030 3000 0123 4567 890".into(),
        logo: "Aucun".into(),
        currency: "€".into(),
        currency_rate: 1.0,
        secondary_currency: "F".into(),
        secondary_rate: 6.55957,
        is_virtual: false,
        disabled: false,
        operations: ops,
    };
    let small: Vec<Operation> = operations
        .iter()
        .step_by(40)
        .cloned()
        .enumerate()
        .map(|(i, mut op)| {
            op.uid = count as u64 + i as u64 + 1;
            op
        })
        .collect();
    let document = Document {
        group_name: "Démo 40 ans".into(),
        guid: bp_core::v5_text::new_file_guid(),
        user_name: "Démo".into(),
        custom_modes: vec![CustomMode {
            name: "Espèces".into(),
            debit: true,
        }],
        accounts: vec![
            account("Compte courant", operations),
            account("Livret A", small),
        ],
        account_order: vec![0, 1],
        default_account: 0,
        categories,
        payees: Vec::new(),
        feature_data: Vec::new(),
        unknown_header: Vec::new(),
    };

    let t = Instant::now();
    bp_core::file::save(out.as_ref(), &document, &Protection::none().unwrap()).unwrap();
    println!("enregistrement de {count} opérations : {:.1?}", t.elapsed());

    let t = Instant::now();
    let opened = bp_core::file::open(out.as_ref(), None).unwrap();
    println!("ouverture                          : {:.1?}", t.elapsed());
    let doc = opened.document;
    let acc = &doc.accounts[0];

    let t = Instant::now();
    let index = SearchIndex::build(&doc, acc);
    println!("index de recherche                 : {:.1?}", t.elapsed());
    let t = Instant::now();
    let order = sorted_order(&doc, acc, SortColumn::Date, false);
    let _ = running_balances(acc, &order, false);
    println!("tri + solde progressif             : {:.1?}", t.elapsed());

    for (label, filters) in [
        (
            "tiers « carre »",
            Filters {
                payee: "carre".into(),
                ..Default::default()
            },
        ),
        (
            "tiers « edf|gaz /total »",
            Filters {
                payee: "edf|gaz /total".into(),
                ..Default::default()
            },
        ),
        (
            "montant -50>50 + non pointées",
            Filters {
                amount: "-50>50".into(),
                tack: bp_core::filter::TackFilter::NotPointed,
                ..Default::default()
            },
        ),
        (
            "catégorie « transport »",
            Filters {
                category: "transport".into(),
                ..Default::default()
            },
        ),
    ] {
        let t = Instant::now();
        let visible = filters.compile(Date::today()).apply(acc, &index, &order);
        println!(
            "filtre {label:<30}: {:>7.2?} → {} lignes",
            t.elapsed(),
            visible.len()
        );
    }
}
