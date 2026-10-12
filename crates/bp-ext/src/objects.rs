//! Comptes et opérations côté Luau : des références (compte, identifiant de
//! ligne) lues et modifiées directement dans le document de la session.

use std::cell::Cell;

use bp_core::model::{Amount, Date, Document, Mode, Operation, StandardMode, Tack};
use mlua::{Lua, MetaMethod, Table, UserData, UserDataFields, UserDataMethods, Value};

use crate::date::{date_value, to_date};
use crate::session::Session;

pub(crate) fn runtime(message: impl Into<String>) -> mlua::Error {
    mlua::Error::runtime(message.into())
}

/// Exécute `f` avec la session de l'appel en cours.
pub(crate) fn with_session<R>(
    lua: &Lua,
    f: impl FnOnce(&mut Session) -> mlua::Result<R>,
) -> mlua::Result<R> {
    let mut session = lua
        .try_app_data_mut::<Session>()
        .map_err(|_| runtime("accès au fichier impossible ici"))?
        .ok_or_else(|| runtime("aucun fichier de comptes n'est ouvert"))?;
    f(&mut session)
}

/// Nombre Luau (entier ou non).
pub(crate) fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(i) => Some(*i as f64),
        Value::Number(n) => Some(*n),
        _ => None,
    }
}

pub(crate) fn amount_to_f64(amount: Amount) -> f64 {
    amount.0 as f64 / Amount::SCALE as f64
}

pub(crate) fn amount_from_value(value: &Value) -> mlua::Result<Amount> {
    match value {
        Value::Integer(i) => Ok(Amount(i * Amount::SCALE)),
        Value::Number(n) if n.is_finite() => Ok(Amount((n * Amount::SCALE as f64).round() as i64)),
        Value::String(s) => {
            let text = s.to_str()?.to_string();
            Amount::parse(&text).ok_or_else(|| runtime(format!("montant invalide : « {text} »")))
        }
        _ => Err(runtime(format!(
            "un montant est attendu (reçu : {})",
            value.type_name()
        ))),
    }
}

/// Texte d'une zone de l'opération : sur une ligne, sans `;`.
fn clean_text(text: &str) -> String {
    text.replace([';', '\r', '\n', '\t'], " ")
}

pub(crate) fn tack_name(tack: Tack) -> &'static str {
    match tack {
        Tack::None => "none",
        Tack::Pointed => "pointed",
        Tack::Reconciled => "reconciled",
    }
}

pub(crate) fn parse_tack(text: &str) -> mlua::Result<Tack> {
    match text.to_lowercase().as_str() {
        "none" | "" | "aucun" | "non pointée" => Ok(Tack::None),
        "pointed" | "p" | "pointée" => Ok(Tack::Pointed),
        "reconciled" | "r" | "rapprochée" => Ok(Tack::Reconciled),
        _ => Err(runtime(format!(
            "pointage inconnu « {text} » (attendu : \"none\", \"pointed\" ou \"reconciled\")"
        ))),
    }
}

pub(crate) fn resolve_mode(document: &Document, name: &str) -> mlua::Result<Mode> {
    if let Some(mode) = StandardMode::from_label(name.trim()) {
        return Ok(Mode::Standard(mode));
    }
    let lower = name.trim().to_lowercase();
    document
        .custom_modes
        .iter()
        .find(|m| m.name.to_lowercase() == lower)
        .map(|m| Mode::Custom(m.name.clone()))
        .ok_or_else(|| runtime(format!("mode de paiement inconnu : « {name} »")))
}

fn normalize_label(text: &str) -> String {
    text.split('>')
        .map(|part| part.trim().to_lowercase())
        .collect::<Vec<_>>()
        .join(" > ")
}

/// Catégorie désignée par son libellé complet (`Parent > Enfant`), son nom
/// (s'il est unique), son identifiant ou une table `{ id = … }`.
pub(crate) fn resolve_category(document: &Document, value: &Value) -> mlua::Result<Option<u32>> {
    match value {
        Value::Nil => Ok(None),
        Value::Boolean(false) => Ok(None),
        Value::Integer(id) => document
            .category(*id as u32)
            .map(|c| Some(c.id))
            .ok_or_else(|| runtime(format!("catégorie inconnue : n°{id}"))),
        Value::Number(id) => resolve_category(document, &Value::Integer(*id as i64)),
        Value::Table(t) => resolve_category(document, &t.get::<Value>("id")?),
        Value::String(s) => {
            let text = s.to_str()?.to_string();
            if text.trim().is_empty() {
                return Ok(None);
            }
            let wanted = normalize_label(&text);
            if let Some(c) = document.categories.iter().find(|c| {
                document
                    .category_label(c.id)
                    .is_some_and(|l| normalize_label(&l) == wanted)
            }) {
                return Ok(Some(c.id));
            }
            let named: Vec<u32> = document
                .categories
                .iter()
                .filter(|c| c.name.trim().to_lowercase() == wanted)
                .map(|c| c.id)
                .collect();
            match named.as_slice() {
                [id] => Ok(Some(*id)),
                [] => Err(runtime(format!("catégorie inconnue : « {text} »"))),
                _ => Err(runtime(format!(
                    "plusieurs catégories s'appellent « {text} » : précisez « Parent > {text} »"
                ))),
            }
        }
        other => Err(runtime(format!(
            "une catégorie est attendue (reçu : {})",
            other.type_name()
        ))),
    }
}

/// Table décrivant une catégorie : `{ id, name, label, parent, is_child }`.
pub(crate) fn category_table(lua: &Lua, document: &Document, id: u32) -> mlua::Result<Value> {
    let Some(pos) = document.categories.iter().position(|c| c.id == id) else {
        return Ok(Value::Nil);
    };
    let c = &document.categories[pos];
    let t = lua.create_table()?;
    t.set("id", c.id)?;
    t.set("name", c.name.clone())?;
    t.set("label", document.category_label(c.id))?;
    t.set("is_child", c.is_child)?;
    if c.is_child
        && let Some(parent) = document.categories[..pos]
            .iter()
            .rev()
            .find(|p| !p.is_child)
    {
        t.set("parent", parent.name.clone())?;
    }
    Ok(Value::Table(t))
}

// ─────────────────────────────── Comptes ───────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct AccountRef {
    pub file: u64,
    pub index: usize,
}

impl AccountRef {
    pub fn check(&self, s: &Session) -> mlua::Result<usize> {
        if self.file != s.file_id {
            return Err(runtime("ce compte appartient à un autre fichier"));
        }
        if self.index >= s.document.accounts.len() {
            return Err(runtime("ce compte n'existe plus"));
        }
        Ok(self.index)
    }
}

pub(crate) fn account_from_value(value: &Value) -> mlua::Result<AccountRef> {
    match value {
        Value::UserData(ud) => ud
            .borrow::<AccountRef>()
            .map(|a| a.clone())
            .map_err(|_| runtime("un compte est attendu")),
        _ => Err(runtime(format!(
            "un compte est attendu (reçu : {})",
            value.type_name()
        ))),
    }
}

/// Compte désigné par un objet compte ou par son nom.
pub(crate) fn account_index(s: &Session, value: &Value) -> mlua::Result<usize> {
    if let Value::String(name) = value {
        let name = name.to_str()?.to_string();
        let lower = name.trim().to_lowercase();
        return s
            .document
            .accounts
            .iter()
            .position(|a| a.name.trim().to_lowercase() == lower)
            .ok_or_else(|| runtime(format!("compte inconnu : « {name} »")));
    }
    account_from_value(value)?.check(s)
}

impl UserData for AccountRef {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        macro_rules! text_field {
            ($name:literal, $field:ident) => {
                fields.add_field_method_get($name, |lua, a| {
                    with_session(lua, |s| Ok(s.document.accounts[a.check(s)?].$field.clone()))
                });
                fields.add_field_method_set($name, |lua, a, value: String| {
                    with_session(lua, |s| {
                        let i = a.check(s)?;
                        s.referentials_mut().accounts[i].$field =
                            clean_text(&value).trim().to_string();
                        Ok(())
                    })
                });
            };
        }
        text_field!("name", name);
        text_field!("bank", bank_name);
        text_field!("agency", agency_name);
        text_field!("number", number);
        text_field!("iban", iban);
        text_field!("bic", bic);
        text_field!("currency", currency);
        fields.add_field_method_get("uid", |lua, a| {
            with_session(lua, |s| Ok(s.document.accounts[a.check(s)?].uid.clone()))
        });
        fields.add_field_method_get("balance", |lua, a| {
            with_session(lua, |s| {
                Ok(amount_to_f64(s.document.accounts[a.check(s)?].balance()))
            })
        });
        fields.add_field_method_get("disabled", |lua, a| {
            with_session(lua, |s| Ok(s.document.accounts[a.check(s)?].disabled))
        });
        fields.add_field_method_get("virtual", |lua, a| {
            with_session(lua, |s| {
                Ok(s.document.accounts[a.check(s)?].counts_as_virtual())
            })
        });
        fields.add_field_method_get("count", |lua, a| {
            with_session(lua, |s| {
                Ok(s.document.accounts[a.check(s)?].operations.len())
            })
        });
        fields.add_field_method_get("current", |lua, a| {
            with_session(lua, |s| Ok(a.check(s)? == s.account))
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("operations", |lua, a, filter: Option<Table>| {
            let filter = filter
                .map(|t| OperationFilter::parse(lua, &t))
                .transpose()?;
            let refs = with_session(lua, |s| {
                let i = a.check(s)?;
                let account = &s.document.accounts[i];
                let mut order: Vec<usize> = (0..account.operations.len())
                    .filter(|&k| {
                        filter
                            .as_ref()
                            .is_none_or(|f| f.matches(&s.document, &account.operations[k]))
                    })
                    .collect();
                order.sort_by_key(|&k| account.operations[k].date);
                Ok(order
                    .into_iter()
                    .map(|k| OpRef::new(s.file_id, i, account.operations[k].uid, k))
                    .collect::<Vec<_>>())
            })?;
            let list = lua.create_table_with_capacity(refs.len(), 0)?;
            for r in refs {
                list.raw_push(r)?;
            }
            Ok(list)
        });
        methods.add_method("add", |lua, a, spec: Table| {
            let op = with_session(lua, |s| {
                let i = a.check(s)?;
                let mut op = new_operation(lua, &s.document, &spec)?;
                op.uid = s.next_uid();
                let (file, uid) = (s.file_id, op.uid);
                let ops = s.operations_mut(i);
                ops.push(op);
                Ok(OpRef::new(file, i, uid, ops.len() - 1))
            })?;
            Ok(op)
        });
        methods.add_method("find", |lua, a, uid: u64| {
            with_session(lua, |s| {
                let i = a.check(s)?;
                Ok(s.document.accounts[i]
                    .operations
                    .iter()
                    .position(|o| o.uid == uid)
                    .map(|k| OpRef::new(s.file_id, i, uid, k)))
            })
        });
        methods.add_method("balance_at", |lua, a, date: Value| {
            let date = to_date(lua, &date)?;
            with_session(lua, |s| {
                let i = a.check(s)?;
                Ok(amount_to_f64(
                    s.document.accounts[i]
                        .operations
                        .iter()
                        .filter(|o| o.date <= date)
                        .map(|o| o.amount)
                        .sum(),
                ))
            })
        });
        methods.add_method("show", |lua, a, ()| {
            with_session(lua, |s| {
                let i = a.check(s)?;
                s.select = Some((i, Vec::new()));
                Ok(())
            })
        });
        methods.add_meta_method(MetaMethod::Eq, |_, a, other: Value| {
            Ok(account_from_value(&other).is_ok_and(|o| o.index == a.index && o.file == a.file))
        });
        methods.add_meta_method(MetaMethod::ToString, |lua, a, ()| {
            with_session(lua, |s| Ok(s.document.accounts[a.check(s)?].name.clone()))
        });
    }
}

/// Filtre de `compte:operations{ … }`.
struct OperationFilter {
    from: Option<Date>,
    to: Option<Date>,
    text: Option<String>,
    payee: Option<String>,
    categories: Option<Vec<Option<u32>>>,
    mode: Option<Mode>,
    tack: Option<Tack>,
    min: Option<Amount>,
    max: Option<Amount>,
}

impl OperationFilter {
    fn parse(lua: &Lua, t: &Table) -> mlua::Result<Self> {
        let date = |key: &str| -> mlua::Result<Option<Date>> {
            match t.get::<Value>(key)? {
                Value::Nil => Ok(None),
                v => to_date(lua, &v).map(Some),
            }
        };
        let amount = |key: &str| -> mlua::Result<Option<Amount>> {
            match t.get::<Value>(key)? {
                Value::Nil => Ok(None),
                v => amount_from_value(&v).map(Some),
            }
        };
        let lower = |key: &str| -> mlua::Result<Option<String>> {
            Ok(t.get::<Option<String>>(key)?
                .map(|s| bp_core::filter::normalize(&s)))
        };
        let (categories, mode) = with_session(lua, |s| {
            let categories = match t.get::<Value>("category")? {
                Value::Nil => None,
                Value::Boolean(false) => Some(vec![None]),
                v => {
                    let id = resolve_category(&s.document, &v)?;
                    let mut ids = vec![id];
                    // Une catégorie mère inclut ses sous-catégories.
                    if let Some(pos) = s.document.categories.iter().position(|c| Some(c.id) == id)
                        && !s.document.categories[pos].is_child
                    {
                        ids.extend(
                            s.document.categories[pos + 1..]
                                .iter()
                                .take_while(|c| c.is_child)
                                .map(|c| Some(c.id)),
                        );
                    }
                    Some(ids)
                }
            };
            let mode = t
                .get::<Option<String>>("mode")?
                .map(|m| resolve_mode(&s.document, &m))
                .transpose()?;
            Ok((categories, mode))
        })?;
        Ok(Self {
            from: date("from")?,
            to: date("to")?,
            text: lower("text")?,
            payee: lower("payee")?,
            categories,
            mode,
            tack: t
                .get::<Option<String>>("tack")?
                .map(|v| parse_tack(&v))
                .transpose()?,
            min: amount("min")?,
            max: amount("max")?,
        })
    }

    fn matches(&self, _document: &Document, op: &Operation) -> bool {
        use bp_core::filter::normalize;
        self.from.is_none_or(|d| op.date >= d)
            && self.to.is_none_or(|d| op.date <= d)
            && self.min.is_none_or(|m| op.amount >= m)
            && self.max.is_none_or(|m| op.amount <= m)
            && self.tack.is_none_or(|t| op.tack == t)
            && self.mode.as_ref().is_none_or(|m| &op.mode == m)
            && self
                .categories
                .as_ref()
                .is_none_or(|ids| ids.contains(&op.category))
            && self
                .payee
                .as_ref()
                .is_none_or(|p| normalize(&op.payee).contains(p.as_str()))
            && self.text.as_ref().is_none_or(|p| {
                normalize(&op.payee).contains(p.as_str())
                    || normalize(&op.details).contains(p.as_str())
            })
    }
}

/// Nouvelle opération décrite par une table `{ date, mode, payee, … }`
/// (identifiant à attribuer).
pub(crate) fn new_operation(
    lua: &Lua,
    document: &Document,
    spec: &Table,
) -> mlua::Result<Operation> {
    let amount = match spec.get::<Value>("amount")? {
        Value::Nil => return Err(runtime("le montant (amount) est obligatoire")),
        v => amount_from_value(&v)?,
    };
    let date = match spec.get::<Value>("date")? {
        Value::Nil => Date::today(),
        v => to_date(lua, &v)?,
    };
    let mode = match spec.get::<Option<String>>("mode")? {
        Some(m) => resolve_mode(document, &m)?,
        None if amount.is_negative() => Mode::Standard(StandardMode::Carte),
        None => Mode::Standard(StandardMode::VirementRecu),
    };
    let tack = spec
        .get::<Option<String>>("tack")?
        .map(|t| parse_tack(&t))
        .transpose()?
        .unwrap_or_default();
    let mark_date = match spec.get::<Value>("mark_date")? {
        Value::Nil => date,
        v => to_date(lua, &v)?,
    };
    let mut op = Operation {
        uid: 0,
        date,
        mark_date,
        tack,
        mode,
        payee: clean_text(&spec.get::<Option<String>>("payee")?.unwrap_or_default()),
        details: clean_text(&spec.get::<Option<String>>("details")?.unwrap_or_default()),
        category: resolve_category(document, &spec.get::<Value>("category")?)?,
        amount,
        extra: Vec::new(),
    };
    if let Some(fields) = spec.get::<Option<Table>>("fields")? {
        for pair in fields.pairs::<String, Value>() {
            let (key, value) = pair?;
            bp_core::split::set_extra(&mut op, &key, &value_text(&value)?);
        }
    }
    Ok(op)
}

fn value_text(value: &Value) -> mlua::Result<String> {
    Ok(match value {
        Value::Nil => String::new(),
        Value::String(s) => s.to_str()?.to_string(),
        other => other.to_string()?,
    })
}

// ────────────────────────────── Opérations ──────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct OpRef {
    pub file: u64,
    pub account: Cell<usize>,
    pub uid: u64,
    /// Dernière position connue dans le compte (vérifiée à chaque accès).
    pub hint: Cell<usize>,
}

impl OpRef {
    pub fn new(file: u64, account: usize, uid: u64, position: usize) -> Self {
        Self {
            file,
            account: Cell::new(account),
            uid,
            hint: Cell::new(position),
        }
    }

    /// (compte, position) de l'opération dans le document.
    pub fn locate(&self, s: &Session) -> mlua::Result<(usize, usize)> {
        if self.file != s.file_id {
            return Err(runtime("cette opération appartient à un autre fichier"));
        }
        let account = self.account.get();
        let ops = &s
            .document
            .accounts
            .get(account)
            .ok_or_else(|| runtime("le compte de cette opération n'existe plus"))?
            .operations;
        let hint = self.hint.get();
        if ops.get(hint).is_some_and(|o| o.uid == self.uid) {
            return Ok((account, hint));
        }
        let position = ops
            .iter()
            .position(|o| o.uid == self.uid)
            .ok_or_else(|| runtime("cette opération a été supprimée"))?;
        self.hint.set(position);
        Ok((account, position))
    }

    fn get<R>(
        &self,
        lua: &Lua,
        f: impl FnOnce(&Session, &Operation) -> mlua::Result<R>,
    ) -> mlua::Result<R> {
        with_session(lua, |s| {
            let (a, k) = self.locate(s)?;
            f(s, &s.document.accounts[a].operations[k])
        })
    }

    /// Modifie l'opération (refusé sur une ligne rapprochée verrouillée).
    fn update(
        &self,
        lua: &Lua,
        f: impl FnOnce(&Document, &mut Operation) -> mlua::Result<()>,
    ) -> mlua::Result<()> {
        with_session(lua, |s| {
            let (a, k) = self.locate(s)?;
            check_unlocked(s, &s.document.accounts[a].operations[k])?;
            let mut op = s.document.accounts[a].operations[k].clone();
            f(&s.document, &mut op)?;
            if op != s.document.accounts[a].operations[k] {
                s.operations_mut(a)[k] = op;
            }
            Ok(())
        })
    }
}

/// Copie d'une opération en table simple (état d'avant une modification,
/// opération supprimée) : mêmes champs que l'objet opération, en lecture.
pub(crate) fn operation_copy(
    lua: &Lua,
    file: u64,
    account: usize,
    op: &Operation,
) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("uid", op.uid)?;
    t.set("account", AccountRef {
        file,
        index: account,
    })?;
    t.set("date", date_value(lua, op.date)?)?;
    t.set("mark_date", date_value(lua, op.mark_date)?)?;
    t.set("payee", op.payee.clone())?;
    t.set("details", op.details.clone())?;
    t.set("amount", amount_to_f64(op.amount))?;
    t.set("mode", op.mode.label().to_string())?;
    let category = op.category.and_then(|id| {
        lua.app_data_ref::<Session>()
            .and_then(|s| s.document.category_label(id))
    });
    t.set("category", category)?;
    t.set("category_id", op.category)?;
    t.set("tack", tack_name(op.tack))?;
    t.set("cheque", op.cheque_number().map(str::to_string))?;
    t.set(
        "is_transfer",
        bp_core::split::transfer_direction(&op.payee).is_some(),
    )?;
    let fields = lua.create_table()?;
    for (k, v) in op.extra.iter().filter(|(k, _)| !k.starts_with('#')) {
        fields.set(k.clone(), v.clone())?;
    }
    t.set("fields", fields)?;
    Ok(t)
}

/// `{ index, count, total, payee }` d'une ligne issue d'une ventilation
/// (tiers « [2/3] Tiers », détail « … (Total: -120,00) »), ou `nil`.
fn split_info(lua: &Lua, op: &Operation) -> mlua::Result<Value> {
    let payee = op.payee.as_str();
    let rest = bp_core::view::payee_without_ventilation(payee);
    if rest.len() == payee.len() {
        return Ok(Value::Nil);
    }
    let prefix = &payee[1..payee.len() - rest.len() - 1];
    let Some((index, count)) = prefix
        .split_once('/')
        .and_then(|(i, n)| Some((i.trim().parse::<u32>().ok()?, n.trim().parse::<u32>().ok()?)))
    else {
        return Ok(Value::Nil);
    };
    let total = op
        .details
        .rfind("(Total: ")
        .and_then(|start| {
            let text = &op.details[start + 8..];
            Amount::parse(text.trim_end_matches(')'))
        })
        .map(amount_to_f64);
    let t = lua.create_table()?;
    t.set("index", index)?;
    t.set("count", count)?;
    t.set("total", total)?;
    t.set("payee", rest.trim_start().to_string())?;
    Ok(Value::Table(t))
}

/// Parts de `op:split{ … }` : `{ amount, category, details }` ; une part
/// sans montant reçoit le reste. Les montants sont pris en valeur absolue.
fn split_parts(
    document: &Document,
    original: &Operation,
    parts: &Table,
) -> mlua::Result<Vec<bp_core::split::SplitPart>> {
    let mut out = Vec::new();
    let mut rest_at = None;
    for (i, part) in parts.sequence_values::<Table>().enumerate() {
        let part = part.map_err(|_| runtime("op:split : chaque part est une table { amount, category, details }"))?;
        let amount = match part.get::<Value>("amount")? {
            Value::Nil => {
                if rest_at.replace(i).is_some() {
                    return Err(runtime("op:split : une seule part peut omettre son montant"));
                }
                Amount::ZERO
            }
            v => Amount(amount_from_value(&v)?.0.abs()),
        };
        out.push(bp_core::split::SplitPart {
            details: part.get::<Option<String>>("details")?.unwrap_or_default(),
            category: resolve_category(document, &part.get::<Value>("category")?)?,
            amount,
        });
    }
    if let Some(i) = rest_at {
        out[i].amount = bp_core::split::remaining_to_split(original.amount, &out);
    }
    Ok(out)
}

fn check_unlocked(s: &Session, op: &Operation) -> mlua::Result<()> {
    if s.locked && op.tack == Tack::Reconciled {
        return Err(runtime(
            "les lignes rapprochées sont verrouillées (menu Opérations) : modification refusée",
        ));
    }
    Ok(())
}

pub(crate) fn op_from_value(value: &Value) -> mlua::Result<OpRef> {
    match value {
        Value::UserData(ud) => ud
            .borrow::<OpRef>()
            .map(|o| o.clone())
            .map_err(|_| runtime("une opération est attendue")),
        _ => Err(runtime(format!(
            "une opération est attendue (reçu : {})",
            value.type_name()
        ))),
    }
}

impl UserData for OpRef {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("uid", |_, o| Ok(o.uid));
        fields.add_field_method_get("account", |lua, o| {
            with_session(lua, |s| {
                let (a, _) = o.locate(s)?;
                Ok(AccountRef {
                    file: s.file_id,
                    index: a,
                })
            })
        });
        fields.add_field_method_get("date", |lua, o| {
            let date = o.get(lua, |_, op| Ok(op.date))?;
            date_value(lua, date)
        });
        fields.add_field_method_set("date", |lua, o, value: Value| {
            let date = to_date(lua, &value)?;
            o.update(lua, |_, op| {
                if op.tack == Tack::None {
                    op.mark_date = date;
                }
                op.date = date;
                Ok(())
            })
        });
        fields.add_field_method_get("mark_date", |lua, o| {
            let date = o.get(lua, |_, op| Ok(op.mark_date))?;
            date_value(lua, date)
        });
        fields.add_field_method_set("mark_date", |lua, o, value: Value| {
            let date = to_date(lua, &value)?;
            o.update(lua, |_, op| {
                op.mark_date = date;
                Ok(())
            })
        });
        fields.add_field_method_get("payee", |lua, o| o.get(lua, |_, op| Ok(op.payee.clone())));
        fields.add_field_method_set("payee", |lua, o, value: String| {
            o.update(lua, |_, op| {
                op.payee = clean_text(&value);
                Ok(())
            })
        });
        fields.add_field_method_get("details", |lua, o| {
            o.get(lua, |_, op| Ok(op.details.clone()))
        });
        fields.add_field_method_set("details", |lua, o, value: String| {
            o.update(lua, |_, op| {
                op.details = clean_text(&value);
                Ok(())
            })
        });
        fields.add_field_method_get("amount", |lua, o| {
            o.get(lua, |_, op| Ok(amount_to_f64(op.amount)))
        });
        fields.add_field_method_set("amount", |lua, o, value: Value| {
            let amount = amount_from_value(&value)?;
            o.update(lua, |_, op| {
                op.amount = amount;
                Ok(())
            })
        });
        fields.add_field_method_get("mode", |lua, o| {
            o.get(lua, |_, op| Ok(op.mode.label().to_string()))
        });
        fields.add_field_method_set("mode", |lua, o, value: String| {
            o.update(lua, |document, op| {
                op.mode = resolve_mode(document, &value)?;
                Ok(())
            })
        });
        fields.add_field_method_get("category", |lua, o| {
            o.get(lua, |s, op| {
                Ok(op.category.and_then(|id| s.document.category_label(id)))
            })
        });
        fields.add_field_method_get("category_id", |lua, o| o.get(lua, |_, op| Ok(op.category)));
        fields.add_field_method_set("category", |lua, o, value: Value| {
            o.update(lua, |document, op| {
                op.category = resolve_category(document, &value)?;
                Ok(())
            })
        });
        fields.add_field_method_get("tack", |lua, o| o.get(lua, |_, op| Ok(tack_name(op.tack))));
        fields.add_field_method_set("tack", |lua, o, value: String| {
            let tack = parse_tack(&value)?;
            o.update(lua, |_, op| {
                if op.tack != tack {
                    op.tack = tack;
                    op.mark_date = Date::today();
                }
                Ok(())
            })
        });
        fields.add_field_method_get("cheque", |lua, o| {
            o.get(lua, |_, op| Ok(op.cheque_number().map(str::to_string)))
        });
        fields.add_field_method_get("is_transfer", |lua, o| {
            o.get(lua, |_, op| {
                Ok(bp_core::split::transfer_direction(&op.payee).is_some())
            })
        });
        fields.add_field_method_get("split_info", |lua, o| {
            let op = o.get(lua, |_, op| Ok(op.clone()))?;
            split_info(lua, &op)
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("field", |lua, o, key: String| {
            o.get(lua, |_, op| {
                Ok(op
                    .extra
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&key))
                    .map(|(_, v)| v.clone()))
            })
        });
        methods.add_method("set_field", |lua, o, (key, value): (String, Value)| {
            if key.starts_with('#') {
                return Err(runtime(format!(
                    "le champ « {key} » est réservé à BankPerfect"
                )));
            }
            let text = value_text(&value)?;
            o.update(lua, |_, op| {
                if !bp_core::split::set_extra(op, &key, &text) {
                    return Err(runtime(format!("nom de champ refusé : « {key} »")));
                }
                Ok(())
            })
        });
        methods.add_method("fields", |lua, o, ()| {
            let extra = o.get(lua, |_, op| Ok(op.extra.clone()))?;
            let t = lua.create_table()?;
            for (k, v) in extra.into_iter().filter(|(k, _)| !k.starts_with('#')) {
                t.set(k, v)?;
            }
            Ok(t)
        });
        methods.add_method("delete", |lua, o, ()| {
            with_session(lua, |s| {
                let (a, k) = o.locate(s)?;
                check_unlocked(s, &s.document.accounts[a].operations[k])?;
                s.operations_mut(a).remove(k);
                Ok(())
            })
        });
        methods.add_method("move_to", |lua, o, target: Value| {
            with_session(lua, |s| {
                let to = account_index(s, &target)?;
                let (a, k) = o.locate(s)?;
                if a == to {
                    return Ok(());
                }
                check_unlocked(s, &s.document.accounts[a].operations[k])?;
                let op = s.operations_mut(a).remove(k);
                let ops = s.operations_mut(to);
                ops.push(op);
                o.account.set(to);
                o.hint.set(ops.len() - 1);
                Ok(())
            })
        });
        methods.add_method("copy_to", |lua, o, target: Value| {
            with_session(lua, |s| {
                let to = account_index(s, &target)?;
                let (a, k) = o.locate(s)?;
                let mut op = s.document.accounts[a].operations[k].clone();
                op.uid = s.next_uid();
                // Une copie n'est pas liée à la ligne sœur d'un transfert.
                op.extra.retain(|(k, _)| !k.starts_with("#sister"));
                let (file, uid) = (s.file_id, op.uid);
                let ops = s.operations_mut(to);
                ops.push(op);
                Ok(OpRef::new(file, to, uid, ops.len() - 1))
            })
        });
        methods.add_method("split", |lua, o, parts: Table| {
            let refs = with_session(lua, |s| {
                let (a, k) = o.locate(s)?;
                let original = s.document.accounts[a].operations[k].clone();
                check_unlocked(s, &original)?;
                let parts = split_parts(&s.document, &original, &parts)?;
                s.operations_mut(a);
                let lines = bp_core::split::split_operation(&mut s.document, a, k, &parts)
                    .map_err(|e| runtime(format!("ventilation impossible : {e}")))?;
                s.reset_uid();
                let ops = &s.document.accounts[a].operations;
                Ok(lines
                    .into_iter()
                    .map(|l| OpRef::new(s.file_id, a, ops[l].uid, l))
                    .collect::<Vec<_>>())
            })?;
            lua.create_sequence_from(refs)
        });
        methods.add_method("sister", |lua, o, ()| {
            with_session(lua, |s| {
                let (a, k) = o.locate(s)?;
                Ok(bp_core::split::find_sister(&s.document, a, k)
                    .filter(|(sa, _)| *sa != a)
                    .map(|(sa, sk)| {
                        let uid = s.document.accounts[sa].operations[sk].uid;
                        OpRef::new(s.file_id, sa, uid, sk)
                    }))
            })
        });
        methods.add_meta_method(MetaMethod::Eq, |_, o, other: Value| {
            Ok(op_from_value(&other).is_ok_and(|p| p.uid == o.uid && p.file == o.file))
        });
        methods.add_meta_method(MetaMethod::ToString, |lua, o, ()| {
            o.get(lua, |_, op| {
                Ok(format!(
                    "{} {} {}",
                    op.date,
                    op.payee,
                    op.amount.format(2, ',')
                ))
            })
        });
    }
}
