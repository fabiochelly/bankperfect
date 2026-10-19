use super::*;
use crate::model::{Account, Date, Mode, StandardMode, Tack};

fn op(uid: u64, day: i32, payee: &str, cents: i64, category: Option<u32>) -> Operation {
    Operation {
        uid,
        date: Date(46000 + day),
        mark_date: Date(46000 + day),
        tack: Tack::None,
        mode: Mode::Standard(StandardMode::Carte),
        payee: payee.into(),
        details: String::new(),
        category,
        amount: Amount::from_cents(cents),
        extra: Vec::new(),
    }
}

/// Fichier aux catégories « maison », comme ceux de l'ancienne version.
fn doc(operations: Vec<Operation>) -> Document {
    let mut d = crate::manage::new_document("Test", Amount::ZERO);
    let names = [
        ("Alimentation", false),
        ("Epicerie", true),
        ("Restaurant", true),
        ("Habitation", false),
        ("EDF", true),
        ("Rangement déco", true),
        ("Santé", false),
        ("Pharmacie", true),
        ("Remboursements", false),
        ("Rbt santé", true),
        ("Transport", false),
        ("Essence", true),
    ];
    d.categories = names
        .iter()
        .enumerate()
        .map(|(i, (name, child))| Category {
            id: i as u32,
            name: name.to_string(),
            is_child: *child,
        })
        .collect();
    d.accounts[0] = Account {
        operations,
        ..d.accounts[0].clone()
    };
    d
}

fn best(c: &Categorizer, payee: &str, cents: i64) -> Option<(u32, f32, Source)> {
    let o = op(0, 0, payee, cents, None);
    c.suggest(&Query::of(&o))
        .best()
        .map(|b| (b.category, b.confidence, b.source))
}

#[test]
fn history_wins_and_follows_the_amount() {
    let mut ops = Vec::new();
    for i in 0..6 {
        ops.push(op(i, i as i32, "CB CARREFOUR 12/10", -6_000, Some(1)));
    }
    for i in 6..9 {
        ops.push(op(i, i as i32, "CB CARREFOUR 03/11", -45_000, Some(5)));
    }
    let c = Categorizer::new(&doc(ops));
    let (category, confidence, source) = best(&c, "CARTE CARREFOUR 01/12", -5_500).unwrap();
    assert_eq!((category, source), (1, Source::History));
    assert!(confidence >= AUTO_THRESHOLD, "{confidence}");
    // Un gros montant ressemble aux achats de rangement.
    assert_eq!(best(&c, "CB CARREFOUR", -48_000).unwrap().0, 5);
}

#[test]
fn dictionary_maps_to_the_users_own_names() {
    let c = Categorizer::new(&doc(Vec::new()));
    // Jamais vus : reconnus par le dictionnaire, rangés par ressemblance de nom.
    assert_eq!(
        best(&c, "CB LIDL 4521", -3_000).map(|b| (b.0, b.2)),
        Some((1, Source::Merchant))
    );
    assert_eq!(
        best(&c, "PRLV SEPA EDF clients particuliers", -9_000)
            .unwrap()
            .0,
        4
    );
    assert_eq!(best(&c, "CB ESSO EXPRESS", -6_000).unwrap().0, 11);
    assert_eq!(best(&c, "VIR SEPA CPAM DES YVELINES", 2_446).unwrap().0, 9);
    assert_eq!(best(&c, "CB PHARMACIE DU CENTRE", -1_290).unwrap().0, 7);
    // Une boulangerie, faute de catégorie dédiée, va dans l'épicerie.
    assert_eq!(best(&c, "CB BOULANGERIE PAUL", -420).unwrap().0, 1);
    assert_eq!(best(&c, "VIR INST CLEAR", 2_000_000), None);
}

#[test]
fn history_teaches_where_generic_categories_go() {
    // L'utilisateur range ses courses dans « Restaurant » (exprès) : le
    // dictionnaire suit ce choix pour une nouvelle enseigne.
    let ops = vec![
        op(1, 1, "CB CARREFOUR", -5_000, Some(2)),
        op(2, 2, "CB AUCHAN", -5_000, Some(2)),
        op(3, 3, "CB MONOPRIX", -5_000, Some(2)),
    ];
    let c = Categorizer::new(&doc(ops));
    assert_eq!(
        best(&c, "CB FRANPRIX", -2_000).map(|b| (b.0, b.2)),
        Some((2, Source::Merchant))
    );
}

#[test]
fn unknown_words_fall_back_on_seen_words() {
    let ops = vec![
        op(1, 1, "VIR INST DUPONT KINE", -6_000, Some(7)),
        op(2, 2, "PRLV ACME TRUC", -6_000, Some(4)),
    ];
    let c = Categorizer::new(&doc(ops));
    let found = best(&c, "VIR INST CABINET DUPONT", -6_000).unwrap();
    assert_eq!((found.0, found.2), (7, Source::Words));
    assert!(found.1 < AUTO_THRESHOLD);
}

#[test]
fn propose_skips_categorized_operations() {
    let ops = vec![
        op(1, 1, "CB LIDL", -3_000, Some(1)),
        op(2, 2, "CB LIDL", -3_200, None),
        op(3, 3, "XYZ", -3_200, None),
    ];
    let d = doc(ops);
    let c = Categorizer::new(&d);
    let proposals = propose(&d, &c, (0..3).map(|i| (0, i)));
    assert_eq!(proposals.len(), 2);
    assert_eq!(proposals[0].suggestion.confident(), Some(1));
    assert!(proposals[1].suggestion.is_empty());
}

#[test]
fn standard_list_maps_onto_itself() {
    let mut d = crate::manage::new_document("Test", Amount::ZERO);
    crate::manage::import_categories(&mut d, &taxonomy::standard_categories_text());
    let c = Categorizer::without_history(&d);
    for g in GENERICS {
        let (id, confidence) = c
            .category_for_generic(g.slug)
            .unwrap_or_else(|| panic!("{}", g.slug));
        let label = d.category_label(id).unwrap();
        assert!(label.ends_with(g.name), "{} → {label}", g.slug);
        assert!(confidence >= 0.95, "{} → {confidence}", g.slug);
    }
}

#[test]
fn words_seen_on_income_say_little_about_an_expense() {
    // Le nom de famille figure sur les salaires (recettes) : un virement émis
    // à un kiné du même nom va en santé, pas dans les revenus.
    let ops = (0..6)
        .map(|i| op(i, i as i32, "VIR SEPA SALAIRE MARTIN", 200_000, Some(8)))
        .collect();
    let c = Categorizer::new(&doc(ops));
    let o = Operation {
        details: "Kine".into(),
        ..op(0, 0, "VIR INST MARTIN DUPONT", -6_000, None)
    };
    let best = c.suggest(&Query::of(&o)).best().cloned().unwrap();
    assert_eq!(best.category, 6, "{best:?}");
}

mod english {
    use super::*;
    use crate::i18n::{Lang, with_lang};

    /// Nouveau fichier créé en anglais : catégories standard anglaises.
    fn english_doc() -> Document {
        with_lang(Lang::En, || {
            crate::manage::new_document("Test", Amount::ZERO)
        })
    }

    fn suggest(c: &Categorizer, payee: &str, mode: StandardMode, cents: i64) -> Option<Candidate> {
        let o = Operation {
            mode: Mode::Standard(mode),
            ..op(0, 0, payee, cents, None)
        };
        with_lang(Lang::En, || c.suggest(&Query::of(&o)).best().cloned())
    }

    #[test]
    fn english_statements_on_the_english_standard_list() {
        let d = english_doc();
        let c = with_lang(Lang::En, || Categorizer::new(&d));
        use StandardMode::*;
        let cases = [
            (
                "CARD PAYMENT TO TESCO STORES 2345 ON 12 MAR",
                Carte,
                -4_520,
                "Food > Groceries",
            ),
            (
                "DIRECT DEBIT BRITISH GAS",
                Prelevement,
                -8_500,
                "Housing > Electricity & gas",
            ),
            (
                "POS DEBIT STARBUCKS #1234 SEATTLE WA",
                Carte,
                -575,
                "Food > Restaurants",
            ),
            (
                "NETFLIX.COM",
                Carte,
                -1_549,
                "Subscriptions > Streaming & news",
            ),
            (
                "SALARY ACME LTD",
                VirementRecu,
                300_000,
                "Income > Salary & bonuses",
            ),
            (
                "HMRC SELF ASSESSMENT",
                VirementEmis,
                -150_000,
                "Taxes > Income tax",
            ),
            (
                "AMAZON MKTPLACE PMTS",
                Carte,
                -2_999,
                "Shopping > Other purchases",
            ),
            ("CHEVRON 0091234", Carte, -5_200, "Transportation > Fuel"),
            (
                "ROCKET MORTGAGE",
                Prelevement,
                -180_000,
                "Housing > Mortgage",
            ),
            ("WALGREENS #4421", Carte, -1_299, "Health > Pharmacy"),
            (
                "ATM WITHDRAWAL",
                RetraitDab,
                -6_000,
                "Banking & insurance > Cash withdrawals",
            ),
            (
                "IRS TREAS 310 TAX REF",
                VirementRecu,
                85_000,
                "Taxes > Income tax",
            ),
            (
                "INTEREST PAID",
                VirementRecu,
                312,
                "Income > Investment income",
            ),
        ];
        for (payee, mode, cents, expected) in cases {
            let best = suggest(&c, payee, mode, cents).unwrap_or_else(|| panic!("{payee}"));
            assert_eq!(
                d.category_label(best.category).as_deref(),
                Some(expected),
                "{payee}"
            );
            assert_eq!(best.source, Source::Merchant, "{payee}");
            // Enseigne reconnue, catégorie au nom standard : même confiance
            // qu'en français pour la même situation.
            assert!(
                (0.8..=0.9).contains(&best.confidence),
                "{payee} {}",
                best.confidence
            );
        }
        let reason = suggest(&c, "NETFLIX.COM", Carte, -1_549).unwrap().reason;
        assert_eq!(reason, "“netflix com” recognized");
    }

    #[test]
    fn english_history_is_explained_in_english() {
        let mut d = english_doc();
        let groceries = d
            .categories
            .iter()
            .find(|c| c.name == "Groceries")
            .unwrap()
            .id;
        d.accounts[0].operations = (0..4)
            .map(|i| {
                op(
                    i,
                    i as i32,
                    "CARD PAYMENT TO TESCO 12 MAR",
                    -3_000,
                    Some(groceries),
                )
            })
            .collect();
        let c = with_lang(Lang::En, || Categorizer::new(&d));
        let best = suggest(
            &c,
            "CARD PAYMENT TO TESCO ON 3 APR",
            StandardMode::Carte,
            -2_800,
        )
        .unwrap();
        assert_eq!((best.category, best.source), (groceries, Source::History));
        assert!(best.confidence >= AUTO_THRESHOLD);
        assert_eq!(best.reason, "4 transactions “tesco”");
        assert_eq!(with_lang(Lang::En, || best.source.label()), "history");
    }

    #[test]
    fn names_are_matched_in_the_language_of_the_file() {
        // Liste standard anglaise : chaque générique retrouve sa catégorie.
        let d = english_doc();
        let c = Categorizer::without_history(&d);
        for g in GENERICS {
            let (id, confidence) = c
                .category_for_generic(g.slug)
                .unwrap_or_else(|| panic!("{}", g.slug));
            let label = d.category_label(id).unwrap();
            assert!(label.ends_with(g.name_en), "{} → {label}", g.slug);
            assert!(confidence >= 0.95, "{} → {confidence}", g.slug);
        }
        // Catégories « maison » en anglais, à la Quicken.
        let mut custom = english_doc();
        let names = [
            ("Auto & Transport", false),
            ("Gas", true),
            ("Parking", true),
            ("Bills & Utilities", false),
            ("Mobile Phone", true),
            ("Food & Dining", false),
            ("Groceries", true),
            ("Coffee Shops", true),
            ("Paycheck", false),
        ];
        custom.categories = names
            .iter()
            .enumerate()
            .map(|(i, (name, child))| Category {
                id: i as u32,
                name: name.to_string(),
                is_child: *child,
            })
            .collect();
        let c = Categorizer::without_history(&custom);
        let mapped = |slug: &str| c.category_for_generic(slug).map(|m| m.0);
        assert_eq!(mapped("transports.carburant"), Some(1));
        assert_eq!(mapped("transports.peages"), Some(2));
        assert_eq!(mapped("abonnements.telecom"), Some(4));
        assert_eq!(mapped("alimentation.courses"), Some(6));
        assert_eq!(mapped("alimentation.restaurants"), Some(7));
        assert_eq!(mapped("revenus.salaires"), Some(8));
        // Un fichier français garde ses rapprochements, sans faux amis :
        // « Fuel » (fioul) n'y est pas du carburant.
        let mut french = doc(Vec::new());
        french.categories.push(Category {
            id: 12,
            name: "Fuel".into(),
            is_child: true,
        });
        let c = with_lang(Lang::En, || Categorizer::without_history(&french));
        assert_eq!(
            c.category_for_generic("transports.carburant").map(|m| m.0),
            Some(11)
        );
    }
}
