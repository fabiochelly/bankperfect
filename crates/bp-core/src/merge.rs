//! Fusion à trois versions d'un fichier de comptes modifié à deux endroits
//! (ordinateur et téléphone, ou deux ordinateurs sur un même dossier
//! Dropbox, Google Drive, OneDrive…).
//!
//! `base` est la version commune (celle de la dernière lecture ou du dernier
//! enregistrement), `ours` la version locale, `theirs` celle trouvée sur
//! disque. Chaque élément est rapproché par son identifiant (opérations et
//! comptes : `uid` ; catégories : `id` ; tiers, modes, données des
//! fonctionnalités : leur nom) puis chaque champ est fusionné séparément :
//! une modification faite d'un seul côté est reprise ; si les deux côtés ont
//! changé le même champ différemment, la version locale l'emporte et le
//! conflit est signalé. Un élément supprimé d'un côté et modifié de l'autre
//! est gardé (perdre une saisie est pire que garder une ligne en trop).
//!
//! Des deux côtés, les nouvelles opérations prennent le plus grand
//! identifiant + 1 : celles de `theirs` qui tombent sur un identifiant déjà
//! pris localement sont renumérotées, liens des virements et archives
//! compris. De même pour les catégories.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::archive::{self, Archives};
use crate::feature_store;
use crate::model::{Account, Category, Date, Document, Operation};
use crate::split::SISTER_LINE_KEY;
use crate::tr;

/// Désaccord résolu en faveur de la version locale (ou en gardant l'élément).
#[derive(Debug, Clone, PartialEq)]
pub enum Conflict {
    /// Même champ d'une opération modifié des deux côtés.
    Operation {
        account: String,
        date: Date,
        payee: String,
    },
    /// Opération supprimée d'un côté, modifiée de l'autre : gardée.
    DeletedOperation {
        account: String,
        date: Date,
        payee: String,
    },
    /// Informations d'un compte modifiées des deux côtés.
    Account {
        name: String,
    },
    /// Compte supprimé d'un côté, modifié de l'autre : gardé.
    DeletedAccount {
        name: String,
    },
    Category {
        name: String,
    },
    Payee {
        caption: String,
    },
    Mode {
        name: String,
    },
    /// Données d'une fonctionnalité (budget, échéancier…) modifiées des deux
    /// côtés : la version locale est gardée en entier.
    Feature {
        key: String,
    },
    /// Réglage du fichier.
    Setting {
        key: String,
    },
}

impl Conflict {
    pub fn describe(&self) -> String {
        match self {
            Self::Operation {
                account,
                date,
                payee,
            } => tr!(
                "{account} : opération du {date} « {payee} » modifiée des deux côtés, version de cet appareil gardée",
                account = account,
                date = date,
                payee = payee
            ),
            Self::DeletedOperation {
                account,
                date,
                payee,
            } => tr!(
                "{account} : opération du {date} « {payee} » supprimée d'un côté et modifiée de l'autre, gardée",
                account = account,
                date = date,
                payee = payee
            ),
            Self::Account { name } => tr!(
                "compte « {} » modifié des deux côtés, version de cet appareil gardée",
                name
            ),
            Self::DeletedAccount { name } => tr!(
                "compte « {} » supprimé d'un côté et modifié de l'autre, gardé",
                name
            ),
            Self::Category { name } => tr!(
                "catégorie « {} » modifiée des deux côtés, version de cet appareil gardée",
                name
            ),
            Self::Payee { caption } => tr!(
                "tiers mémorisé « {} » modifié des deux côtés, version de cet appareil gardée",
                caption
            ),
            Self::Mode { name } => tr!(
                "mode « {} » modifié des deux côtés, version de cet appareil gardée",
                name
            ),
            Self::Feature { key } => tr!(
                "données « {} » modifiées des deux côtés, version de cet appareil gardée",
                key
            ),
            Self::Setting { key } => tr!(
                "réglage « {} » modifié des deux côtés, version de cet appareil gardé",
                key
            ),
        }
    }
}

/// Résultat d'une fusion.
#[derive(Debug, Clone)]
pub struct Merged {
    pub document: Document,
    pub conflicts: Vec<Conflict>,
    /// Opérations et catégories venues de `theirs` renumérotées.
    pub renumbered: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MergeError {
    /// `theirs` n'est pas une version du même fichier (autre identifiant) :
    /// le fichier a été remplacé par un autre.
    #[error("{}", tr!("le fichier sur disque a été remplacé par un autre fichier de comptes"))]
    DifferentFile,
}

/// Fusionne `ours` (version locale) et `theirs` (version sur disque) à partir
/// de leur ancêtre commun `base`.
pub fn merge(base: &Document, ours: &Document, theirs: &Document) -> Result<Merged, MergeError> {
    if !base.guid.is_empty() && !theirs.guid.is_empty() && base.guid != theirs.guid {
        return Err(MergeError::DifferentFile);
    }
    let mut theirs = theirs.clone();
    let renumbered =
        renumber_categories(base, ours, &mut theirs) + renumber_operations(base, ours, &mut theirs);
    let theirs = &theirs;
    let mut conflicts = Vec::new();

    let accounts = merge_accounts(base, ours, theirs, &mut conflicts);
    let mut document = Document {
        group_name: pick_setting(
            &base.group_name,
            &ours.group_name,
            &theirs.group_name,
            tr!("nom du groupe de comptes"),
            &mut conflicts,
        ),
        guid: if ours.guid.is_empty() {
            theirs.guid.clone()
        } else {
            ours.guid.clone()
        },
        user_name: pick_setting(
            &base.user_name,
            &ours.user_name,
            &theirs.user_name,
            tr!("nom de l'utilisateur"),
            &mut conflicts,
        ),
        custom_modes: merge_list(
            &base.custom_modes,
            &ours.custom_modes,
            &theirs.custom_modes,
            |m| m.name.clone(),
            |b, o, t| {
                item(b, o, t, |m| {
                    conflicts.push(Conflict::Mode {
                        name: m.name.clone(),
                    })
                })
            },
        ),
        account_order: Vec::new(),
        default_account: 0,
        categories: merge_list(
            &base.categories,
            &ours.categories,
            &theirs.categories,
            |c| c.id,
            |b, o, t| {
                item(b, o, t, |c| {
                    conflicts.push(Conflict::Category {
                        name: c.name.clone(),
                    })
                })
            },
        ),
        payees: merge_list(
            &base.payees,
            &ours.payees,
            &theirs.payees,
            |p| p.caption.clone(),
            |b, o, t| {
                item(b, o, t, |p| {
                    conflicts.push(Conflict::Payee {
                        caption: p.caption.clone(),
                    })
                })
            },
        ),
        feature_data: merge_list(
            &base.feature_data,
            &ours.feature_data,
            &theirs.feature_data,
            |f| f.key.clone(),
            |b, o, t| {
                item(b, o, t, |f| {
                    conflicts.push(Conflict::Feature { key: f.key.clone() })
                })
            },
        ),
        unknown_header: merge_list(
            &base.unknown_header,
            &ours.unknown_header,
            &theirs.unknown_header,
            |(k, _)| k.clone(),
            |b, o, t| {
                item(b, o, t, |(k, _)| {
                    conflicts.push(Conflict::Setting { key: k.clone() })
                })
            },
        ),
        accounts,
    };
    merge_account_order(base, ours, theirs, &mut document, &mut conflicts);
    keep_used_categories(base, ours, theirs, &mut document);
    conflicts.dedup();
    Ok(Merged {
        document,
        conflicts,
        renumbered,
    })
}

// ---------------------------------------------------------------------------
// Renumérotation

/// Catégories créées des deux côtés avec le même identifiant : celles de
/// `theirs` en prennent un nouveau, et ses opérations et tiers suivent.
fn renumber_categories(base: &Document, ours: &Document, theirs: &mut Document) -> usize {
    let in_base: HashSet<u32> = base.categories.iter().map(|c| c.id).collect();
    let in_ours: HashMap<u32, &Category> = ours.categories.iter().map(|c| (c.id, c)).collect();
    let mut next = base
        .categories
        .iter()
        .chain(&ours.categories)
        .chain(&theirs.categories)
        .map(|c| c.id)
        .max()
        .unwrap_or(0)
        + 1;
    let mut map = HashMap::new();
    for category in &mut theirs.categories {
        if in_base.contains(&category.id)
            || in_ours.get(&category.id).is_none_or(|c| *c == category)
        {
            continue;
        }
        map.insert(category.id, next);
        category.id = next;
        next += 1;
    }
    if map.is_empty() {
        return 0;
    }
    let remap = |c: &mut Option<u32>| {
        if let Some(new) = c.and_then(|id| map.get(&id)) {
            *c = Some(*new);
        }
    };
    for op in theirs.accounts.iter_mut().flat_map(|a| &mut a.operations) {
        remap(&mut op.category);
    }
    for payee in &mut theirs.payees {
        remap(&mut payee.category);
    }
    map.len()
}

/// Opérations créées des deux côtés avec le même identifiant : celles de
/// `theirs` en prennent un nouveau ; les liens des virements (ligne sœur) et
/// les écritures de synthèse des archives suivent.
fn renumber_operations(base: &Document, ours: &Document, theirs: &mut Document) -> usize {
    let in_base: HashSet<u64> = all_operations(base).map(|(_, op)| op.uid).collect();
    let in_ours: HashMap<u64, (&str, &Operation)> = all_operations(ours)
        .map(|(a, op)| (op.uid, (a, op)))
        .collect();
    let mut next = base
        .next_operation_uid()
        .max(ours.next_operation_uid())
        .max(theirs.next_operation_uid());
    let mut map = HashMap::new();
    for account in &mut theirs.accounts {
        for op in &mut account.operations {
            if in_base.contains(&op.uid)
                || in_ours
                    .get(&op.uid)
                    .is_none_or(|(a, o)| *a == account.uid && *o == op)
            {
                continue;
            }
            map.insert(op.uid, next);
            op.uid = next;
            next += 1;
        }
    }
    if map.is_empty() {
        return 0;
    }
    for op in theirs.accounts.iter_mut().flat_map(|a| &mut a.operations) {
        for (key, value) in &mut op.extra {
            if key == SISTER_LINE_KEY
                && let Some(new) = value.parse::<u64>().ok().and_then(|uid| map.get(&uid))
            {
                *value = new.to_string();
            }
        }
    }
    if let Ok(mut archives) = Archives::load(theirs) {
        let mut changed = false;
        for entry in &mut archives.entries {
            if let Some(new) = map.get(&entry.synthesis) {
                entry.synthesis = *new;
                changed = true;
            }
        }
        if changed && archives.save(theirs).is_err() {
            // Archives illisibles d'un côté : elles restent telles quelles.
            feature_store::remove(theirs, archive::KEY);
        }
    }
    map.len()
}

fn all_operations(document: &Document) -> impl Iterator<Item = (&str, &Operation)> {
    document
        .accounts
        .iter()
        .flat_map(|a| a.operations.iter().map(move |op| (a.uid.as_str(), op)))
}

// ---------------------------------------------------------------------------
// Comptes et opérations

fn merge_accounts(
    base: &Document,
    ours: &Document,
    theirs: &Document,
    conflicts: &mut Vec<Conflict>,
) -> Vec<Account> {
    // Un compte supprimé d'un côté sans avoir été touché de l'autre
    // disparaît ; touché de l'autre côté, il est gardé avec ses opérations
    // (celles d'origine comptent alors comme présentes du côté qui l'a
    // supprimé).
    let unchanged = |b: &Account, x: &Account| b == x;
    let mut kept_for = (HashSet::new(), HashSet::new());
    let mut headers = merge_list(
        &base.accounts,
        &ours.accounts,
        &theirs.accounts,
        |a| a.uid.clone(),
        |b, o, t| match (b, o, t) {
            (Some(b), Some(o), None) if !unchanged(b, o) => {
                conflicts.push(Conflict::DeletedAccount {
                    name: o.name.clone(),
                });
                kept_for.1.insert(b.uid.clone());
                Some(o.clone_header())
            }
            (Some(b), None, Some(t)) if !unchanged(b, t) => {
                conflicts.push(Conflict::DeletedAccount {
                    name: t.name.clone(),
                });
                kept_for.0.insert(b.uid.clone());
                Some(t.clone_header())
            }
            (Some(_), Some(_), None) | (Some(_), None, Some(_)) => None,
            (b, Some(o), Some(t)) => {
                let b = b.unwrap_or(o);
                let mut clash = false;
                let header = merge_header(b, o, t, &mut clash);
                if clash {
                    conflicts.push(Conflict::Account {
                        name: o.name.clone(),
                    });
                }
                Some(header)
            }
            (_, Some(x), None) | (_, None, Some(x)) => Some(x.clone_header()),
            (_, None, None) => None,
        },
    );

    let restored = |doc: &Document, kept: &HashSet<String>| -> Vec<Account> {
        let mut accounts = doc.accounts.clone();
        accounts.extend(
            base.accounts
                .iter()
                .filter(|a| kept.contains(&a.uid))
                .cloned(),
        );
        accounts
    };
    let ours_accounts = restored(ours, &kept_for.0);
    let theirs_accounts = restored(theirs, &kept_for.1);

    let index = |accounts: &[Account]| -> HashMap<u64, (String, Operation)> {
        accounts
            .iter()
            .flat_map(|a| {
                a.operations
                    .iter()
                    .map(|op| (op.uid, (a.uid.clone(), op.clone())))
            })
            .collect()
    };
    let b_ops = index(&base.accounts);
    let o_ops = index(&ours_accounts);
    let t_ops = index(&theirs_accounts);
    let names: HashMap<&str, &str> = base
        .accounts
        .iter()
        .chain(&ours_accounts)
        .chain(&theirs_accounts)
        .map(|a| (a.uid.as_str(), a.name.as_str()))
        .collect();
    let describe = |(account, op): &(String, Operation)| {
        (
            names.get(account.as_str()).unwrap_or(&"").to_string(),
            op.date,
            op.payee.clone(),
        )
    };

    let mut merged: HashMap<u64, (String, Operation)> = HashMap::new();
    let uids: HashSet<u64> = b_ops
        .keys()
        .chain(o_ops.keys())
        .chain(t_ops.keys())
        .copied()
        .collect();
    for uid in uids {
        let result = match (b_ops.get(&uid), o_ops.get(&uid), t_ops.get(&uid)) {
            (b, Some(o), Some(t)) => {
                let b = b.unwrap_or(o);
                let mut clash = false;
                let account = pick(&b.0, &o.0, &t.0, &mut clash);
                let op = merge_operation(&b.1, &o.1, &t.1, &mut clash);
                if clash {
                    let (account, date, payee) = describe(o);
                    conflicts.push(Conflict::Operation {
                        account,
                        date,
                        payee,
                    });
                }
                Some((account, op))
            }
            (Some(b), Some(x), None) | (Some(b), None, Some(x)) => {
                if b == x {
                    None
                } else {
                    let (account, date, payee) = describe(x);
                    conflicts.push(Conflict::DeletedOperation {
                        account,
                        date,
                        payee,
                    });
                    Some(x.clone())
                }
            }
            (None, Some(x), None) | (None, None, Some(x)) => Some(x.clone()),
            (_, None, None) => None,
        };
        if let Some(result) = result {
            merged.insert(uid, result);
        }
    }

    // Opérations dont le compte a disparu (cas limite) : premier compte.
    let present: HashSet<String> = headers.iter().map(|a| a.uid.clone()).collect();
    let fallback = headers.first().map(|a| a.uid.clone()).unwrap_or_default();
    let mut by_account: HashMap<String, Vec<u64>> = HashMap::new();
    for (uid, (account, _)) in merged.iter_mut() {
        if !present.contains(account) {
            *account = fallback.clone();
        }
        by_account.entry(account.clone()).or_default().push(*uid);
    }

    let order_of = |accounts: &[Account], uid: &str| -> Vec<u64> {
        accounts
            .iter()
            .find(|a| a.uid == uid)
            .map(|a| a.operations.iter().map(|op| op.uid).collect())
            .unwrap_or_default()
    };
    for account in &mut headers {
        let Some(mut uids) = by_account.remove(&account.uid) else {
            continue;
        };
        let order = merge_order(
            &order_of(&base.accounts, &account.uid),
            &order_of(&ours_accounts, &account.uid),
            &order_of(&theirs_accounts, &account.uid),
        );
        let rank: HashMap<u64, usize> = order.iter().enumerate().map(|(i, u)| (*u, i)).collect();
        // Opérations déplacées vers ce compte : après les autres.
        uids.sort_by_key(|u| (rank.get(u).copied().unwrap_or(usize::MAX), *u));
        account.operations = uids
            .into_iter()
            .filter_map(|u| merged.remove(&u).map(|(_, op)| op))
            .collect();
    }
    headers
}

fn merge_header(b: &Account, o: &Account, t: &Account, clash: &mut bool) -> Account {
    Account {
        uid: o.uid.clone(),
        name: pick(&b.name, &o.name, &t.name, clash),
        bank_name: pick(&b.bank_name, &o.bank_name, &t.bank_name, clash),
        agency_name: pick(&b.agency_name, &o.agency_name, &t.agency_name, clash),
        agency_address: pick(
            &b.agency_address,
            &o.agency_address,
            &t.agency_address,
            clash,
        ),
        number: pick(&b.number, &o.number, &t.number, clash),
        bank_code: pick(&b.bank_code, &o.bank_code, &t.bank_code, clash),
        agency_code: pick(&b.agency_code, &o.agency_code, &t.agency_code, clash),
        bic: pick(&b.bic, &o.bic, &t.bic, clash),
        iban: pick(&b.iban, &o.iban, &t.iban, clash),
        logo: pick(&b.logo, &o.logo, &t.logo, clash),
        currency: pick(&b.currency, &o.currency, &t.currency, clash),
        currency_rate: pick(&b.currency_rate, &o.currency_rate, &t.currency_rate, clash),
        secondary_currency: pick(
            &b.secondary_currency,
            &o.secondary_currency,
            &t.secondary_currency,
            clash,
        ),
        secondary_rate: pick(
            &b.secondary_rate,
            &o.secondary_rate,
            &t.secondary_rate,
            clash,
        ),
        is_virtual: pick(&b.is_virtual, &o.is_virtual, &t.is_virtual, clash),
        disabled: pick(&b.disabled, &o.disabled, &t.disabled, clash),
        operations: Vec::new(),
    }
}

fn merge_operation(b: &Operation, o: &Operation, t: &Operation, clash: &mut bool) -> Operation {
    Operation {
        uid: o.uid,
        date: pick(&b.date, &o.date, &t.date, clash),
        mark_date: pick(&b.mark_date, &o.mark_date, &t.mark_date, clash),
        tack: pick(&b.tack, &o.tack, &t.tack, clash),
        mode: pick(&b.mode, &o.mode, &t.mode, clash),
        payee: pick(&b.payee, &o.payee, &t.payee, clash),
        details: pick(&b.details, &o.details, &t.details, clash),
        category: pick(&b.category, &o.category, &t.category, clash),
        amount: pick(&b.amount, &o.amount, &t.amount, clash),
        extra: merge_list(
            &b.extra,
            &o.extra,
            &t.extra,
            |(k, _)| k.clone(),
            |b, o, t| item(b, o, t, |_| *clash = true),
        ),
    }
}

/// Ordre d'affichage des comptes et compte par défaut, par identifiant.
fn merge_account_order(
    base: &Document,
    ours: &Document,
    theirs: &Document,
    document: &mut Document,
    conflicts: &mut Vec<Conflict>,
) {
    let uids = |doc: &Document| -> Vec<String> {
        doc.account_order
            .iter()
            .filter_map(|&i| doc.accounts.get(i).map(|a| a.uid.clone()))
            .collect()
    };
    let position: HashMap<&str, usize> = document
        .accounts
        .iter()
        .enumerate()
        .map(|(i, a)| (a.uid.as_str(), i))
        .collect();
    let mut order: Vec<usize> = merge_order(&uids(base), &uids(ours), &uids(theirs))
        .iter()
        .filter_map(|uid| position.get(uid.as_str()).copied())
        .collect();
    // Comptes absents de l'ordre (créés d'un côté, ou gardés après une
    // suppression) : à la fin.
    for i in 0..document.accounts.len() {
        if !order.contains(&i) {
            order.push(i);
        }
    }
    let default = |doc: &Document| doc.accounts.get(doc.default_account).map(|a| a.uid.clone());
    let mut clash = false;
    let chosen = pick(&default(base), &default(ours), &default(theirs), &mut clash);
    if clash {
        conflicts.push(Conflict::Setting {
            key: tr!("compte par défaut").into(),
        });
    }
    document.default_account = chosen
        .and_then(|uid| position.get(uid.as_str()).copied())
        .unwrap_or_else(|| order.first().copied().unwrap_or(0));
    document.account_order = order;
}

/// Catégorie supprimée d'un côté mais utilisée par une opération ou un tiers
/// de l'autre : remise (à la fin, au premier niveau).
fn keep_used_categories(
    base: &Document,
    ours: &Document,
    theirs: &Document,
    document: &mut Document,
) {
    let present: HashSet<u32> = document.categories.iter().map(|c| c.id).collect();
    let used: HashSet<u32> = document
        .accounts
        .iter()
        .flat_map(|a| &a.operations)
        .filter_map(|op| op.category)
        .chain(document.payees.iter().filter_map(|p| p.category))
        .filter(|id| !present.contains(id))
        .collect();
    let mut missing: Vec<u32> = used.into_iter().collect();
    missing.sort_unstable();
    for id in missing {
        if let Some(category) = [ours, theirs, base]
            .iter()
            .find_map(|d| d.categories.iter().find(|c| c.id == id))
        {
            document.categories.push(Category {
                is_child: false,
                ..category.clone()
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Outils génériques

/// Valeur fusionnée : le changement d'un seul côté l'emporte ; changé des
/// deux côtés différemment, la valeur locale est gardée et `clash` levé.
fn pick<T: Clone + PartialEq>(base: &T, ours: &T, theirs: &T, clash: &mut bool) -> T {
    if ours == theirs || theirs == base {
        ours.clone()
    } else if ours == base {
        theirs.clone()
    } else {
        *clash = true;
        ours.clone()
    }
}

fn pick_setting(
    base: &String,
    ours: &String,
    theirs: &String,
    key: &str,
    conflicts: &mut Vec<Conflict>,
) -> String {
    let mut clash = false;
    let value = pick(base, ours, theirs, &mut clash);
    if clash {
        conflicts.push(Conflict::Setting { key: key.into() });
    }
    value
}

/// Élément d'une liste, fusionné en entier. `clash` reçoit la version
/// gardée quand les deux côtés diffèrent.
fn item<T: Clone + PartialEq>(
    base: Option<&T>,
    ours: Option<&T>,
    theirs: Option<&T>,
    mut clash: impl FnMut(&T),
) -> Option<T> {
    match (base, ours, theirs) {
        (_, Some(o), Some(t)) if o == t => Some(o.clone()),
        (Some(b), Some(o), Some(t)) => {
            if o == b {
                Some(t.clone())
            } else {
                if t != b {
                    clash(o);
                }
                Some(o.clone())
            }
        }
        (None, Some(o), Some(_)) => {
            clash(o);
            Some(o.clone())
        }
        (Some(b), Some(x), None) | (Some(b), None, Some(x)) => {
            if x == b {
                None
            } else {
                clash(x);
                Some(x.clone())
            }
        }
        (None, Some(x), None) | (None, None, Some(x)) => Some(x.clone()),
        (_, None, None) => None,
    }
}

/// Fusionne trois versions d'une liste ordonnée dont les éléments sont
/// rapprochés par `key`. Les doublons de clé sont rapprochés dans leur ordre
/// d'apparition.
fn merge_list<T: Clone, K: Clone + Eq + Hash>(
    base: &[T],
    ours: &[T],
    theirs: &[T],
    key: impl Fn(&T) -> K,
    mut merge_item: impl FnMut(Option<&T>, Option<&T>, Option<&T>) -> Option<T>,
) -> Vec<T> {
    let keyed = |list: &[T]| -> Vec<(K, usize)> {
        let mut seen: HashMap<K, usize> = HashMap::new();
        list.iter()
            .map(|x| {
                let k = key(x);
                let n = seen.entry(k.clone()).or_insert(0);
                *n += 1;
                (k, *n)
            })
            .collect()
    };
    let (bk, ok, tk) = (keyed(base), keyed(ours), keyed(theirs));
    let lookup = |keys: &[(K, usize)]| -> HashMap<(K, usize), usize> {
        keys.iter()
            .cloned()
            .enumerate()
            .map(|(i, k)| (k, i))
            .collect()
    };
    let (bm, om, tm) = (lookup(&bk), lookup(&ok), lookup(&tk));
    merge_order(&bk, &ok, &tk)
        .into_iter()
        .filter_map(|k| {
            merge_item(
                bm.get(&k).map(|&i| &base[i]),
                om.get(&k).map(|&i| &ours[i]),
                tm.get(&k).map(|&i| &theirs[i]),
            )
        })
        .collect()
}

/// Ordre fusionné de trois listes de clés uniques. L'ordre d'un côté qui a
/// réordonné les éléments communs l'emporte (le local si les deux l'ont
/// fait) ; les clés présentes d'un seul côté sont placées après celle qui les
/// précède de ce côté-là. Les clés absentes des deux côtés sont omises
/// (l'appelant décide de leur suppression avec la clé dans les deux listes
/// restantes).
fn merge_order<K: Clone + Eq + Hash>(base: &[K], ours: &[K], theirs: &[K]) -> Vec<K> {
    let reordered = |side: &[K]| {
        let in_side: HashSet<&K> = side.iter().collect();
        let in_base: HashSet<&K> = base.iter().collect();
        !base
            .iter()
            .filter(|k| in_side.contains(k))
            .eq(side.iter().filter(|k| in_base.contains(k)))
    };
    let (spine, other) = if !reordered(ours) && reordered(theirs) {
        (theirs, ours)
    } else {
        (ours, theirs)
    };
    let in_spine: HashSet<&K> = spine.iter().collect();
    // Clés de `other` absentes de `spine`, groupées derrière leur
    // prédécesseur présent dans `spine` (`None` : en tête).
    let mut after: HashMap<Option<&K>, Vec<&K>> = HashMap::new();
    let mut anchor: Option<&K> = None;
    for k in other {
        if in_spine.contains(k) {
            anchor = Some(k);
        } else {
            after.entry(anchor).or_default().push(k);
        }
    }
    // Un groupe passe après les clés propres à `spine` qui suivent son
    // prédécesseur : les ajouts locaux d'abord, puis ceux de l'autre côté.
    let in_other: HashSet<&K> = other.iter().collect();
    let mut result = Vec::with_capacity(spine.len() + other.len());
    let mut pending = after.remove(&None).unwrap_or_default();
    for k in spine {
        if in_other.contains(k) {
            result.extend(pending.drain(..).cloned());
        }
        result.push(k.clone());
        if let Some(group) = after.remove(&Some(k)) {
            pending.extend(group);
        }
    }
    result.extend(pending.into_iter().cloned());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Amount, Tack};

    fn op(uid: u64, day: i32, payee: &str, cents: i64) -> Operation {
        Operation {
            uid,
            date: Date(45000 + day),
            mark_date: Date(45000 + day),
            tack: Tack::None,
            mode: Default::default(),
            payee: payee.into(),
            details: String::new(),
            category: None,
            amount: Amount(cents),
            extra: Vec::new(),
        }
    }

    fn account(uid: &str, name: &str, operations: Vec<Operation>) -> Account {
        Account {
            uid: uid.into(),
            name: name.into(),
            bank_name: String::new(),
            agency_name: String::new(),
            agency_address: String::new(),
            number: String::new(),
            bank_code: String::new(),
            agency_code: String::new(),
            bic: String::new(),
            iban: String::new(),
            logo: String::new(),
            currency: "EUR".into(),
            currency_rate: 1.0,
            secondary_currency: String::new(),
            secondary_rate: 1.0,
            is_virtual: false,
            disabled: false,
            operations,
        }
    }

    fn document() -> Document {
        Document {
            guid: "{GUID}".into(),
            accounts: vec![
                account(
                    "a",
                    "Courant",
                    vec![op(1, 1, "Loyer", -80000), op(2, 2, "Salaire", 250000)],
                ),
                account("b", "Livret", vec![op(3, 1, "Intérêts", 1200)]),
            ],
            account_order: vec![0, 1],
            categories: vec![
                Category {
                    id: 1,
                    name: "Logement".into(),
                    is_child: false,
                },
                Category {
                    id: 2,
                    name: "Revenus".into(),
                    is_child: false,
                },
            ],
            ..Default::default()
        }
    }

    fn ops(doc: &Document, account: usize) -> Vec<(u64, String)> {
        doc.accounts[account]
            .operations
            .iter()
            .map(|o| (o.uid, o.payee.clone()))
            .collect()
    }

    #[test]
    fn one_side_changed_gives_that_side() {
        let base = document();
        let mut changed = document();
        changed.accounts[0].operations[0].tack = Tack::Pointed;
        changed.accounts[1]
            .operations
            .push(op(4, 3, "Virement", 5000));
        changed.categories.push(Category {
            id: 3,
            name: "Loisirs".into(),
            is_child: false,
        });
        let merged = merge(&base, &base, &changed).unwrap();
        assert_eq!(merged.document, changed);
        assert!(merged.conflicts.is_empty());
        let merged = merge(&base, &changed, &base).unwrap();
        assert_eq!(merged.document, changed);
    }

    #[test]
    fn different_fields_of_one_operation_are_both_kept() {
        let base = document();
        let mut ours = document();
        ours.accounts[0].operations[0].category = Some(1);
        let mut theirs = document();
        theirs.accounts[0].operations[0].tack = Tack::Pointed;
        let merged = merge(&base, &ours, &theirs).unwrap();
        let op = &merged.document.accounts[0].operations[0];
        assert_eq!((op.category, op.tack), (Some(1), Tack::Pointed));
        assert!(merged.conflicts.is_empty());
    }

    #[test]
    fn same_field_changed_twice_keeps_ours_and_reports() {
        let base = document();
        let mut ours = document();
        ours.accounts[0].operations[0].amount = Amount(-81000);
        let mut theirs = document();
        theirs.accounts[0].operations[0].amount = Amount(-82000);
        let merged = merge(&base, &ours, &theirs).unwrap();
        assert_eq!(
            merged.document.accounts[0].operations[0].amount,
            Amount(-81000)
        );
        assert_eq!(merged.conflicts.len(), 1);
        assert!(
            matches!(&merged.conflicts[0], Conflict::Operation { payee, .. } if payee == "Loyer")
        );
    }

    #[test]
    fn new_operations_on_both_sides_are_all_kept() {
        let base = document();
        let mut ours = document();
        ours.accounts[0]
            .operations
            .push(op(4, 5, "Boulangerie", -350));
        let mut theirs = document();
        theirs.accounts[0]
            .operations
            .push(op(4, 6, "Pharmacie", -1290));
        theirs.accounts[0]
            .operations
            .push(op(5, 6, "Cinéma", -1100));
        let merged = merge(&base, &ours, &theirs).unwrap();
        // Seule la Pharmacie tombe sur un identifiant déjà pris (4).
        assert_eq!(merged.renumbered, 1);
        assert_eq!(
            ops(&merged.document, 0),
            [
                (1, "Loyer".into()),
                (2, "Salaire".into()),
                (4, "Boulangerie".into()),
                (6, "Pharmacie".into()),
                (5, "Cinéma".into()),
            ]
        );
    }

    #[test]
    fn renumbered_transfer_keeps_its_link() {
        let base = document();
        let mut ours = document();
        ours.accounts[0]
            .operations
            .push(op(4, 5, "Boulangerie", -350));
        let mut theirs = document();
        let mut out = op(4, 6, "Transfert", -10000);
        out.extra.push((SISTER_LINE_KEY.into(), "5".into()));
        let mut back = op(5, 6, "Transfert", 10000);
        back.extra.push((SISTER_LINE_KEY.into(), "4".into()));
        theirs.accounts[0].operations.push(out);
        theirs.accounts[1].operations.push(back);
        let merged = merge(&base, &ours, &theirs).unwrap();
        let out = merged.document.accounts[0].operations.last().unwrap();
        let back = merged.document.accounts[1].operations.last().unwrap();
        assert_eq!(
            out.extra(SISTER_LINE_KEY),
            Some(back.uid.to_string().as_str())
        );
        assert_eq!(
            back.extra(SISTER_LINE_KEY),
            Some(out.uid.to_string().as_str())
        );
        assert_ne!(out.uid, 4);
    }

    #[test]
    fn deletion_wins_over_nothing_but_not_over_a_change() {
        let base = document();
        let mut ours = document();
        ours.accounts[0].operations.remove(1);
        let theirs = document();
        let merged = merge(&base, &ours, &theirs).unwrap();
        assert_eq!(ops(&merged.document, 0), [(1, "Loyer".into())]);

        let mut theirs = document();
        theirs.accounts[0].operations[1].tack = Tack::Pointed;
        let merged = merge(&base, &ours, &theirs).unwrap();
        assert_eq!(merged.document.accounts[0].operations.len(), 2);
        assert!(matches!(
            merged.conflicts[0],
            Conflict::DeletedOperation { .. }
        ));
    }

    #[test]
    fn operation_moved_and_edited() {
        let base = document();
        let mut ours = document();
        let moved = ours.accounts[0].operations.remove(0);
        ours.accounts[1].operations.push(moved);
        let mut theirs = document();
        theirs.accounts[0].operations[0].details = "novembre".into();
        let merged = merge(&base, &ours, &theirs).unwrap();
        assert_eq!(ops(&merged.document, 0), [(2, "Salaire".into())]);
        let moved = merged.document.accounts[1].operations.last().unwrap();
        assert_eq!((moved.uid, moved.details.as_str()), (1, "novembre"));
    }

    #[test]
    fn categories_created_on_both_sides() {
        let base = document();
        let mut ours = document();
        ours.categories.push(Category {
            id: 3,
            name: "Loisirs".into(),
            is_child: false,
        });
        ours.accounts[0].operations[0].category = Some(3);
        let mut theirs = document();
        theirs.categories.push(Category {
            id: 3,
            name: "Santé".into(),
            is_child: false,
        });
        theirs.accounts[0].operations[1].category = Some(3);
        let merged = merge(&base, &ours, &theirs).unwrap();
        let doc = &merged.document;
        let names: Vec<&str> = doc.categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Logement", "Revenus", "Loisirs", "Santé"]);
        assert_eq!(
            doc.category_label(doc.accounts[0].operations[0].category.unwrap())
                .unwrap(),
            "Loisirs"
        );
        assert_eq!(
            doc.category_label(doc.accounts[0].operations[1].category.unwrap())
                .unwrap(),
            "Santé"
        );
    }

    #[test]
    fn deleted_category_still_used_comes_back() {
        let base = document();
        let mut ours = document();
        ours.categories.retain(|c| c.id != 2);
        let mut theirs = document();
        theirs.accounts[0].operations[1].category = Some(2);
        let merged = merge(&base, &ours, &theirs).unwrap();
        assert!(merged.document.category(2).is_some());
    }

    #[test]
    fn accounts_added_and_deleted() {
        let base = document();
        let mut ours = document();
        ours.accounts
            .push(account("c", "Joint", vec![op(10, 1, "Courses", -4000)]));
        ours.account_order.push(2);
        let mut theirs = document();
        theirs.accounts.remove(1);
        theirs.account_order = vec![0];
        let merged = merge(&base, &ours, &theirs).unwrap();
        let names: Vec<&str> = merged
            .document
            .account_order
            .iter()
            .map(|&i| merged.document.accounts[i].name.as_str())
            .collect();
        assert_eq!(names, ["Courant", "Joint"]);
    }

    #[test]
    fn deleted_account_used_elsewhere_is_kept_whole() {
        let base = document();
        let mut ours = document();
        ours.accounts[1]
            .operations
            .push(op(4, 4, "Versement", 10000));
        let mut theirs = document();
        theirs.accounts.remove(1);
        theirs.account_order = vec![0];
        let merged = merge(&base, &ours, &theirs).unwrap();
        assert_eq!(
            ops(&merged.document, 1),
            [(3, "Intérêts".into()), (4, "Versement".into())]
        );
        assert_eq!(merged.document.account_order, [0, 1]);
        assert!(matches!(
            merged.conflicts[0],
            Conflict::DeletedAccount { .. }
        ));
    }

    #[test]
    fn order_changes_are_kept() {
        let base = document();
        let mut ours = document();
        ours.payees.push(crate::model::Payee {
            caption: "EDF".into(),
            mode: Default::default(),
            category: None,
            payee: "EDF".into(),
            details: String::new(),
            amount: String::new(),
        });
        let mut theirs = document();
        theirs.account_order = vec![1, 0];
        theirs.default_account = 1;
        let merged = merge(&base, &ours, &theirs).unwrap();
        assert_eq!(merged.document.account_order, [1, 0]);
        assert_eq!(merged.document.default_account, 1);
        assert_eq!(merged.document.payees.len(), 1);
    }

    #[test]
    fn feature_data_changed_on_both_sides_keeps_ours() {
        let base = document();
        let mut ours = document();
        feature_store::set(&mut ours, "budget.v6", &1u32).unwrap();
        let mut theirs = document();
        feature_store::set(&mut theirs, "budget.v6", &2u32).unwrap();
        feature_store::set(&mut theirs, "scheduler.v6", &3u32).unwrap();
        let merged = merge(&base, &ours, &theirs).unwrap();
        assert_eq!(
            feature_store::get::<u32>(&merged.document, "budget.v6")
                .unwrap()
                .unwrap(),
            1
        );
        assert_eq!(
            feature_store::get::<u32>(&merged.document, "scheduler.v6")
                .unwrap()
                .unwrap(),
            3
        );
        assert_eq!(
            merged.conflicts,
            [Conflict::Feature {
                key: "budget.v6".into()
            }]
        );
    }

    #[test]
    fn another_file_is_refused() {
        let base = document();
        let mut theirs = document();
        theirs.guid = "{AUTRE}".into();
        assert_eq!(
            merge(&base, &base, &theirs).unwrap_err(),
            MergeError::DifferentFile
        );
    }

    #[test]
    fn order_merge() {
        let k = |s: &str| s.chars().map(String::from).collect::<Vec<_>>();
        let m = |b: &str, o: &str, t: &str| merge_order(&k(b), &k(o), &k(t)).concat();
        assert_eq!(m("abc", "abcd", "xabc"), "xabcd");
        assert_eq!(m("abc", "abc", "cba"), "cba");
        assert_eq!(m("abc", "acb", "cba"), "acb");
        assert_eq!(m("abc", "abxc", "abyc"), "abxyc");
        assert_eq!(m("abc", "ac", "abc"), "abc");
    }
}
