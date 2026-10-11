//! Gestion des référentiels : comptes, catégories, modes de paiement
//! (fenêtres « Gestion des comptes », « Gestion des catégories », « Modes de
//! paiement » de l'ancienne version, inventaire 01 §11, §12, §14).

use crate::model::{
    Account, Amount, Category, CustomMode, Date, Document, Mode, Operation, StandardMode, Tack,
};

// ─────────────────────────────── Comptes ───────────────────────────────

/// Crée un compte avec sa ligne « Solde initial » (rapprochée, Virement reçu,
/// datée du jour), l'ajoute en fin d'ordre d'affichage et renvoie son index.
pub fn add_account(document: &mut Document, name: &str, initial: Amount) -> usize {
    let index = document.accounts.len();
    let uid_line = document.next_operation_uid();
    let today = Date::today();
    let mode = if initial.is_negative() {
        StandardMode::Prelevement
    } else {
        StandardMode::VirementRecu
    };
    document.accounts.push(Account {
        uid: crate::v5_text::new_account_uid(index),
        name: clean(name),
        bank_name: String::new(),
        agency_name: String::new(),
        agency_address: String::new(),
        number: String::new(),
        bank_code: String::new(),
        agency_code: String::new(),
        bic: String::new(),
        iban: String::new(),
        logo: "Aucun".into(),
        currency: "€".into(),
        currency_rate: 1.0,
        secondary_currency: "F".into(),
        secondary_rate: 6.55957,
        is_virtual: false,
        disabled: false,
        operations: vec![Operation {
            uid: uid_line,
            date: today,
            mark_date: today,
            tack: Tack::Reconciled,
            mode: Mode::Standard(mode),
            payee: "Solde initial".into(),
            details: String::new(),
            category: None,
            amount: initial,
            extra: Vec::new(),
        }],
    });
    document.account_order.push(index);
    index
}

/// Supprime un compte et ses opérations ; refusé s'il n'en reste qu'un.
/// Renvoie l'index réel du compte à afficher ensuite.
pub fn delete_account(document: &mut Document, index: usize) -> Option<usize> {
    if document.accounts.len() <= 1 || index >= document.accounts.len() {
        return None;
    }
    document.accounts.remove(index);
    document.account_order.retain(|&i| i != index);
    for i in &mut document.account_order {
        if *i > index {
            *i -= 1;
        }
    }
    document.default_account = document.default_account.min(document.accounts.len() - 1);
    document.account_order.first().copied()
}

/// Déplace un compte d'un cran dans l'ordre d'affichage (`up` = vers le haut).
pub fn move_account(document: &mut Document, index: usize, up: bool) -> bool {
    let Some(pos) = document.account_order.iter().position(|&i| i == index) else {
        return false;
    };
    let target = if up {
        pos.checked_sub(1)
    } else {
        Some(pos + 1).filter(|&p| p < document.account_order.len())
    };
    match target {
        Some(t) => {
            document.account_order.swap(pos, t);
            true
        }
        None => false,
    }
}

/// Nom de compte unique (« Compte 2 », « Compte 3 »…) à partir d'une base.
pub fn unique_account_name(document: &Document, base: &str) -> String {
    let base = if base.trim().is_empty() {
        "Compte"
    } else {
        base.trim()
    };
    if !document.accounts.iter().any(|a| a.name == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !document.accounts.iter().any(|a| &a.name == n))
        .expect("nom libre")
}

/// Le format interdit le `;` dans les textes : remplacé par une espace.
pub fn clean(text: &str) -> String {
    text.replace([';', '\r', '\n'], " ")
}

// ─────────────────────────────── Catégories ───────────────────────────────

fn next_category_id(document: &Document) -> u32 {
    document
        .categories
        .iter()
        .map(|c| c.id + 1)
        .max()
        .unwrap_or(0)
}

/// Insère une catégorie avant la position `before` (ou à la fin) ; renvoie
/// son identifiant. La première ligne ne peut pas être une sous-catégorie.
pub fn insert_category(
    document: &mut Document,
    name: &str,
    child: bool,
    before: Option<usize>,
) -> u32 {
    let id = next_category_id(document);
    let pos = before
        .unwrap_or(document.categories.len())
        .min(document.categories.len());
    let is_child = child && pos > 0;
    document.categories.insert(
        pos,
        Category {
            id,
            name: clean(name).trim().to_string(),
            is_child,
        },
    );
    id
}

/// Positions occupées par une catégorie et, si c'est une mère, ses enfants.
fn group_range(document: &Document, pos: usize) -> std::ops::Range<usize> {
    let mut end = pos + 1;
    if !document.categories[pos].is_child {
        while end < document.categories.len() && document.categories[end].is_child {
            end += 1;
        }
    }
    pos..end
}

/// Supprime une catégorie ; une catégorie mère est supprimée avec ses
/// sous-catégories. Les opérations et tiers concernés perdent leur catégorie.
/// Renvoie le nombre d'opérations modifiées.
///
/// L'ancienne version rattachait silencieusement les sous-catégories à la
/// mère précédente et laissait des identifiants orphelins dans les opérations.
pub fn delete_category(document: &mut Document, pos: usize) -> usize {
    if pos >= document.categories.len() {
        return 0;
    }
    let removed: Vec<u32> = document
        .categories
        .drain(group_range(document, pos))
        .map(|c| c.id)
        .collect();
    if let Some(first) = document.categories.first_mut() {
        first.is_child = false;
    }
    let mut changed = 0;
    for op in document.accounts.iter_mut().flat_map(|a| &mut a.operations) {
        if op.category.is_some_and(|id| removed.contains(&id)) {
            op.category = None;
            changed += 1;
        }
    }
    for payee in &mut document.payees {
        if payee.category.is_some_and(|id| removed.contains(&id)) {
            payee.category = None;
        }
    }
    changed
}

/// Nombre d'opérations rattachées à une catégorie ou à ses enfants.
pub fn category_usage(document: &Document, pos: usize) -> usize {
    let ids: Vec<u32> = group_range(document, pos)
        .map(|p| document.categories[p].id)
        .collect();
    document
        .accounts
        .iter()
        .flat_map(|a| &a.operations)
        .filter(|op| op.category.is_some_and(|id| ids.contains(&id)))
        .count()
}

pub fn rename_category(document: &mut Document, pos: usize, name: &str) {
    if let Some(c) = document.categories.get_mut(pos) {
        let name = clean(name).trim().to_string();
        if !name.is_empty() {
            c.name = name;
        }
    }
}

/// Bascule catégorie ↔ sous-catégorie ; impossible sur la première ligne.
pub fn toggle_child(document: &mut Document, pos: usize) -> bool {
    if pos == 0 || pos >= document.categories.len() {
        return false;
    }
    let c = &mut document.categories[pos];
    c.is_child = !c.is_child;
    true
}

/// Déplace une catégorie (avec ses enfants si `with_children` et que c'est
/// une mère) au-dessus ou en dessous de la voisine (ou du groupe voisin).
/// Renvoie la nouvelle position.
pub fn move_category(
    document: &mut Document,
    pos: usize,
    up: bool,
    with_children: bool,
) -> Option<usize> {
    let cats = &mut document.categories;
    if pos >= cats.len() {
        return None;
    }
    let group = if with_children && !cats[pos].is_child {
        group_range(document, pos)
    } else {
        pos..pos + 1
    };
    let cats = &mut document.categories;
    let len = group.len();
    if up {
        if group.start == 0 {
            return None;
        }
        // Avec ses enfants, la mère passe au-dessus du groupe précédent entier.
        let mut target = group.start - 1;
        if with_children && !cats[group.start].is_child {
            while target > 0 && cats[target].is_child {
                target -= 1;
            }
        }
        let moved: Vec<Category> = cats.drain(group.clone()).collect();
        for (k, c) in moved.into_iter().enumerate() {
            cats.insert(target + k, c);
        }
        cats[0].is_child = false;
        Some(target)
    } else {
        if group.end >= cats.len() {
            return None;
        }
        let mut next_end = group.end + 1;
        if with_children && !cats[group.start].is_child && !cats[group.end].is_child {
            while next_end < cats.len() && cats[next_end].is_child {
                next_end += 1;
            }
        }
        let moved: Vec<Category> = cats.drain(group.clone()).collect();
        let insert_at = next_end - len;
        for (k, c) in moved.into_iter().enumerate() {
            cats.insert(insert_at + k, c);
        }
        cats[0].is_child = false;
        Some(insert_at)
    }
}

/// Trie les catégories mères par nom, et les enfants au sein de leur mère.
/// Les identifiants sont conservés.
pub fn sort_categories(document: &mut Document) {
    let mut groups: Vec<Vec<Category>> = Vec::new();
    for c in document.categories.drain(..) {
        match groups.last_mut() {
            Some(group) if c.is_child => group.push(c),
            _ => groups.push(vec![c]),
        }
    }
    let key = |c: &Category| crate::filter::normalize(&c.name);
    for group in &mut groups {
        group[1..].sort_by_key(key);
    }
    groups.sort_by_key(|g| key(&g[0]));
    document.categories = groups.into_iter().flatten().collect();
}

/// Export texte : une ligne `id=libellé` (sous-catégories préfixées de 5 espaces).
pub fn export_categories(document: &Document) -> String {
    document
        .categories
        .iter()
        .map(|c| {
            format!(
                "{}={}{}\r\n",
                c.id,
                if c.is_child { "     " } else { "" },
                c.name
            )
        })
        .collect()
}

/// Import texte (remplace la liste). Les opérations dont la catégorie n'existe
/// plus la perdent. Renvoie le nombre de catégories importées.
pub fn import_categories(document: &mut Document, text: &str) -> usize {
    let joined: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let parsed = crate::v5_text::parse_categories_text(&joined.join(";"));
    if parsed.is_empty() {
        return 0;
    }
    document.categories = parsed;
    let ids: std::collections::HashSet<u32> = document.categories.iter().map(|c| c.id).collect();
    for op in document.accounts.iter_mut().flat_map(|a| &mut a.operations) {
        if op.category.is_some_and(|id| !ids.contains(&id)) {
            op.category = None;
        }
    }
    document.categories.len()
}

// ─────────────────────────────── Modes de paiement ───────────────────────────────

/// Ajoute un mode personnalisé (nom unique) ; renvoie son nom effectif.
pub fn add_mode(document: &mut Document, name: &str, debit: bool) -> String {
    let base = clean(name).trim().to_string();
    let base = if base.is_empty() {
        "Nouveau mode".to_string()
    } else {
        base
    };
    let taken = |n: &str| {
        StandardMode::from_label(n).is_some()
            || document
                .custom_modes
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case(n))
    };
    let name = if taken(&base) {
        (2..)
            .map(|k| format!("{base} {k}"))
            .find(|n| !taken(n))
            .expect("nom libre")
    } else {
        base
    };
    document.custom_modes.push(CustomMode {
        name: name.clone(),
        debit,
    });
    name
}

/// Renomme un mode personnalisé ; les opérations et tiers qui l'utilisent
/// suivent (l'ancienne version les faisait redevenir « Carte »).
pub fn rename_mode(document: &mut Document, index: usize, new_name: &str) -> bool {
    let name = clean(new_name).trim().to_string();
    if name.is_empty()
        || StandardMode::from_label(&name).is_some()
        || document
            .custom_modes
            .iter()
            .enumerate()
            .any(|(i, m)| i != index && m.name.eq_ignore_ascii_case(&name))
    {
        return false;
    }
    let Some(mode) = document.custom_modes.get_mut(index) else {
        return false;
    };
    let old = std::mem::replace(&mut mode.name, name.clone());
    for op in document.accounts.iter_mut().flat_map(|a| &mut a.operations) {
        if op.mode == Mode::Custom(old.clone()) {
            op.mode = Mode::Custom(name.clone());
        }
    }
    for payee in &mut document.payees {
        if payee.mode == Mode::Custom(old.clone()) {
            payee.mode = Mode::Custom(name.clone());
        }
    }
    true
}

/// Nombre d'opérations utilisant un mode personnalisé.
pub fn mode_usage(document: &Document, index: usize) -> usize {
    let Some(mode) = document.custom_modes.get(index) else {
        return 0;
    };
    let target = Mode::Custom(mode.name.clone());
    document
        .accounts
        .iter()
        .flat_map(|a| &a.operations)
        .filter(|op| op.mode == target)
        .count()
}

/// Supprime un mode personnalisé ; ses opérations et tiers passent au mode
/// `replacement`. Renvoie le nombre d'opérations modifiées.
pub fn delete_mode(document: &mut Document, index: usize, replacement: Mode) -> usize {
    if index >= document.custom_modes.len() {
        return 0;
    }
    let removed = Mode::Custom(document.custom_modes.remove(index).name);
    let mut changed = 0;
    for op in document.accounts.iter_mut().flat_map(|a| &mut a.operations) {
        if op.mode == removed {
            op.mode = replacement.clone();
            changed += 1;
        }
    }
    for payee in &mut document.payees {
        if payee.mode == removed {
            payee.mode = replacement.clone();
        }
    }
    changed
}

/// Catégories d'un nouveau fichier (celles de l'ancienne version) ; un
/// retrait de cinq espaces marque une sous-catégorie.
const DEFAULT_CATEGORIES: &str = "Alimentation;     Epicerie;     Restaurant;Loisirs;     Sorties Spectacles;     Sports;     Hifi Vidéo;     Téléphonie;Habillement;Habitation;     EDF;     Téléphone;     Loyer;     Equipement;     Rangement/Déco;     Abonnements;     Assurance;Santé;     Médecine;     Pharmacie;Transport;     Essence;     Assurance;     Equipement/Réparations;     Public;     Parking;     Péage;Remboursements;     Rbt santé;     Autres;Salaire;     Paye;     Revenus annexes";

/// Nouveau fichier de comptes : un compte « Courant » avec son solde
/// initial et les catégories par défaut.
pub fn new_document(name: &str, initial: Amount) -> Document {
    let categories = DEFAULT_CATEGORIES
        .split(';')
        .filter(|c| !c.trim().is_empty())
        .enumerate()
        .map(|(id, c)| Category {
            id: id as u32,
            name: c.trim().to_string(),
            is_child: c.starts_with(' '),
        })
        .collect();
    let mut document = Document {
        group_name: clean(name),
        guid: crate::v5_text::new_file_guid(),
        user_name: String::new(),
        custom_modes: Vec::new(),
        accounts: Vec::new(),
        account_order: Vec::new(),
        default_account: 0,
        categories,
        payees: Vec::new(),
        feature_data: Vec::new(),
        unknown_header: Vec::new(),
    };
    add_account(&mut document, "Courant", initial);
    document
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn document() -> Document {
        let op = |category: Option<u32>, mode: Mode| Operation {
            uid: 1,
            date: Date(45000),
            mark_date: Date(45000),
            tack: Tack::None,
            mode,
            payee: "x".into(),
            details: String::new(),
            category,
            amount: Amount::from_cents(-100),
            extra: Vec::new(),
        };
        let mut doc = Document {
            group_name: String::new(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: vec![CustomMode {
                name: "Carte FNAC".into(),
                debit: true,
            }],
            accounts: Vec::new(),
            account_order: Vec::new(),
            default_account: 0,
            categories: vec![
                Category {
                    id: 0,
                    name: "Alimentation".into(),
                    is_child: false,
                },
                Category {
                    id: 1,
                    name: "Epicerie".into(),
                    is_child: true,
                },
                Category {
                    id: 2,
                    name: "Restaurant".into(),
                    is_child: true,
                },
                Category {
                    id: 5,
                    name: "Loisirs".into(),
                    is_child: false,
                },
                Category {
                    id: 6,
                    name: "Cinéma".into(),
                    is_child: true,
                },
            ],
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        };
        add_account(&mut doc, "Courant", Amount::from_cents(10_000));
        doc.accounts[0]
            .operations
            .push(op(Some(1), Mode::Custom("Carte FNAC".into())));
        doc.accounts[0]
            .operations
            .push(op(Some(6), Mode::Standard(StandardMode::Carte)));
        doc
    }

    #[test]
    fn accounts_lifecycle() {
        let mut doc = document();
        let i = add_account(&mut doc, "Livret; A", Amount::from_cents(-500));
        assert_eq!(doc.accounts[i].name, "Livret  A");
        let first = &doc.accounts[i].operations[0];
        assert_eq!(
            (first.tack, first.amount, first.payee.as_str()),
            (Tack::Reconciled, Amount::from_cents(-500), "Solde initial")
        );
        assert_eq!(doc.account_order, [0, 1]);
        assert!(move_account(&mut doc, 1, true));
        assert_eq!(doc.account_order, [1, 0]);
        assert_eq!(delete_account(&mut doc, 1), Some(0));
        assert_eq!(doc.account_order, [0]);
        assert_eq!(
            delete_account(&mut doc, 0),
            None,
            "le dernier compte ne peut pas être supprimé"
        );
        assert_eq!(unique_account_name(&doc, "Courant"), "Courant 2");
    }

    #[test]
    fn categories_edit() {
        let mut doc = document();
        let id = insert_category(&mut doc, "Fast-food", true, Some(3));
        assert_eq!(id, 7);
        assert_eq!(
            doc.category_label(7).as_deref(),
            Some("Alimentation > Fast-food")
        );
        assert!(!toggle_child(&mut doc, 0));
        assert_eq!(category_usage(&doc, 0), 1);
        // Supprimer « Alimentation » supprime aussi ses enfants et détache l'opération.
        assert_eq!(delete_category(&mut doc, 0), 1);
        assert_eq!(
            doc.categories
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["Loisirs", "Cinéma"]
        );
        assert_eq!(doc.accounts[0].operations[1].category, None);
    }

    #[test]
    fn categories_move_and_sort() {
        let mut doc = document();
        // Loisirs (+ Cinéma) passe au-dessus du groupe Alimentation.
        assert_eq!(move_category(&mut doc, 3, true, true), Some(0));
        let names: Vec<&str> = doc.categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Loisirs",
                "Cinéma",
                "Alimentation",
                "Epicerie",
                "Restaurant"
            ]
        );
        assert_eq!(move_category(&mut doc, 0, false, true), Some(3));
        sort_categories(&mut doc);
        let names: Vec<&str> = doc.categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Alimentation",
                "Epicerie",
                "Restaurant",
                "Loisirs",
                "Cinéma"
            ]
        );
        let text = export_categories(&doc);
        let mut copy = document();
        copy.categories.clear();
        assert_eq!(import_categories(&mut copy, &text), 5);
        assert_eq!(copy.categories, doc.categories);
    }

    #[test]
    fn modes_follow_rename_and_delete() {
        let mut doc = document();
        assert!(rename_mode(&mut doc, 0, "Carte Fnac+"));
        assert_eq!(
            doc.accounts[0].operations[1].mode,
            Mode::Custom("Carte Fnac+".into())
        );
        assert!(
            !rename_mode(&mut doc, 0, "Carte"),
            "nom d'un mode standard refusé"
        );
        assert_eq!(add_mode(&mut doc, "Carte Fnac+", true), "Carte Fnac+ 2");
        assert_eq!(mode_usage(&doc, 0), 1);
        assert_eq!(
            delete_mode(&mut doc, 0, Mode::Standard(StandardMode::Carte)),
            1
        );
        assert_eq!(
            doc.accounts[0].operations[1].mode,
            Mode::Standard(StandardMode::Carte)
        );
    }

    #[test]
    fn new_document_has_default_categories_and_account() {
        let doc = new_document("Famille", Amount::parse("150").unwrap());
        assert_eq!(doc.group_name, "Famille");
        assert_eq!(doc.accounts.len(), 1);
        assert_eq!(doc.accounts[0].name, "Courant");
        assert_eq!(doc.accounts[0].balance(), Amount::parse("150").unwrap());
        assert_eq!(doc.account_order, vec![0]);
        assert_eq!(doc.categories.len(), 33);
        assert!(!doc.categories[0].is_child && doc.categories[1].is_child);
        assert_eq!(
            doc.category_label(1).as_deref(),
            Some("Alimentation > Epicerie")
        );
    }
}
