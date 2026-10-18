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
