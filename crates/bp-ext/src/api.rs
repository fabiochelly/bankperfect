//! Table globale `bp` : enregistrement des commandes et crochets, accès aux
//! comptes et aux référentiels, stockage, réseau, fichiers.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use bp_core::model::{Amount, Date, Document};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value, Variadic};

use crate::date::{LuaDate, date_value, to_date};
use crate::objects::{
    AccountRef, OpRef, account_index, amount_from_value, amount_to_f64, category_table,
    new_operation, op_from_value, resolve_category, runtime, with_session,
};
use crate::{CommandInfo, Event, Needs, Services};

/// `-1 234,56 €` : espace fine insécable entre les milliers.
pub(crate) fn money_text(amount: Amount, currency: &str) -> String {
    let plain = amount.format(2, ',');
    let (sign, rest) = match plain.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", plain.as_str()),
    };
    let (int_part, frac) = rest.split_once(',').unwrap_or((rest, "00"));
    let mut grouped = String::new();
    for (i, c) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            grouped.push('\u{202f}');
        }
        grouped.push(c);
    }
    if currency.is_empty() {
        format!("{sign}{grouped},{frac}")
    } else {
        format!("{sign}{grouped},{frac}\u{a0}{currency}")
    }
}

/// Ce qu'une extension déclare en se chargeant.
#[derive(Default)]
pub(crate) struct Registry {
    pub loading: bool,
    pub name: Option<String>,
    pub description: String,
    pub version: String,
    pub author: String,
    pub permissions: Vec<String>,
    pub commands: Vec<(CommandInfo, Function)>,
    pub hooks: Vec<(Event, Function)>,
    pub log: Vec<String>,
}

/// Messages et demandes qui ne dépendent pas d'un fichier ouvert.
#[derive(Default)]
pub(crate) struct Outbox {
    pub toasts: Vec<String>,
    /// Fenêtres à fermer : (identifiant, valeur renvoyée).
    pub closes: Vec<(u64, Value)>,
}

const LOG_LINES: usize = 200;

pub(crate) struct ApiContext {
    pub id: String,
    pub dir: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub services: Rc<dyn Services>,
    pub registry: Rc<RefCell<Registry>>,
}

fn store_key(id: &str) -> String {
    format!("extension.{id}")
}

fn load_store(document: &Document, id: &str) -> serde_json::Map<String, serde_json::Value> {
    match bp_core::feature_store::get::<serde_json::Value>(document, &store_key(id)) {
        Some(Ok(serde_json::Value::Object(map))) => map,
        _ => serde_json::Map::new(),
    }
}

fn prefs_path(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join(format!("{id}.json"))
}

fn load_prefs(data_dir: &Path, id: &str) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read(prefs_path(data_dir, id))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Map<_, _>>(&b).ok())
        .unwrap_or_default()
}

fn to_json(lua: &Lua, value: Value) -> mlua::Result<serde_json::Value> {
    if matches!(value, Value::UserData(_)) {
        if let Ok(d) = to_date(lua, &value) {
            return Ok(serde_json::Value::String(d.to_string()));
        }
        return Err(runtime(
            "seules les données simples (texte, nombres, tables, dates) se stockent",
        ));
    }
    lua.from_value(value)
}

fn from_json(lua: &Lua, value: &serde_json::Value) -> mlua::Result<Value> {
    lua.to_value_with(
        value,
        mlua::serde::SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false),
    )
}

fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.trim_start_matches('\u{feff}').to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

fn filters(spec: Option<&Table>) -> mlua::Result<Vec<(String, Vec<String>)>> {
    let mut out = Vec::new();
    let Some(spec) = spec else { return Ok(out) };
    let Some(list) = spec.get::<Option<Table>>("filters")? else {
        return Ok(out);
    };
    for pair in list.pairs::<Value, Value>() {
        let (k, v) = pair?;
        match (k, v) {
            // filters = { CSV = { "csv", "txt" } }
            (Value::String(name), Value::Table(exts)) => out.push((
                name.to_str()?.to_string(),
                exts.sequence_values::<String>()
                    .collect::<mlua::Result<_>>()?,
            )),
            // filters = { { "CSV", { "csv" } } }
            (_, Value::Table(entry)) => out.push((
                entry.get::<String>(1)?,
                entry
                    .get::<Table>(2)?
                    .sequence_values::<String>()
                    .collect::<mlua::Result<_>>()?,
            )),
            _ => {}
        }
    }
    Ok(out)
}

pub(crate) fn install(lua: &Lua, ctx: ApiContext) -> mlua::Result<()> {
    let bp = lua.create_table()?;
    let registry = ctx.registry.clone();
    let id = ctx.id.clone();

    bp.set("version", env!("CARGO_PKG_VERSION"))?;
    bp.set("id", id.clone())?;

    // ─────────────────────── Déclarations ───────────────────────
    {
        let registry = registry.clone();
        bp.set(
            "extension",
            lua.create_function(move |_, spec: Table| {
                let mut r = registry.borrow_mut();
                r.name = spec.get("name")?;
                r.description = spec
                    .get::<Option<String>>("description")?
                    .unwrap_or_default();
                r.version = spec.get::<Option<String>>("version")?.unwrap_or_default();
                r.author = spec.get::<Option<String>>("author")?.unwrap_or_default();
                if let Some(p) = spec.get::<Option<Table>>("permissions")? {
                    r.permissions = p.sequence_values::<String>().collect::<mlua::Result<_>>()?;
                }
                Ok(())
            })?,
        )?;
    }
    {
        let registry = registry.clone();
        bp.set(
            "command",
            lua.create_function(move |_, spec: Table| {
                let mut r = registry.borrow_mut();
                if !r.loading {
                    return Err(runtime("bp.command se déclare au chargement de l'extension"));
                }
                let title: String = spec
                    .get::<Option<String>>("title")?
                    .ok_or_else(|| runtime("bp.command : title est obligatoire"))?;
                let run: Function = spec
                    .get::<Option<Function>>("run")?
                    .ok_or_else(|| runtime("bp.command : run (fonction) est obligatoire"))?;
                let needs = match spec.get::<Option<String>>("needs")?.as_deref() {
                    None | Some("file") => Needs::File,
                    Some("selection") => Needs::Selection,
                    Some("nothing" | "none") => Needs::Nothing,
                    Some(other) => {
                        return Err(runtime(format!(
                            "bp.command : needs « {other} » inconnu (\"file\", \"selection\" ou \"nothing\")"
                        )));
                    }
                };
                let mut menus: Vec<String> = match spec.get::<Value>("menu")? {
                    Value::Nil => Vec::new(),
                    Value::String(s) => vec![s.to_str()?.to_string()],
                    Value::Table(t) => t.sequence_values::<String>().collect::<mlua::Result<_>>()?,
                    _ => return Err(runtime("bp.command : menu est un texte ou une liste de textes")),
                };
                for m in &menus {
                    if !crate::MENUS.iter().any(|(id, _)| id == m) {
                        let known: Vec<&str> = crate::MENUS.iter().map(|(id, _)| *id).collect();
                        return Err(runtime(format!(
                            "bp.command : menu « {m} » inconnu ({})",
                            known.join(", ")
                        )));
                    }
                }
                menus.dedup();
                if menus.is_empty() {
                    menus.push("extensions".into());
                }
                let info = CommandInfo {
                    id: spec.get::<Option<String>>("id")?.unwrap_or_else(|| title.clone()),
                    title,
                    icon: spec.get("icon")?,
                    shortcut: spec.get("shortcut")?,
                    description: spec.get("description")?,
                    needs,
                    menus,
                };
                r.commands.push((info, run));
                Ok(())
            })?,
        )?;
    }
    {
        let registry = registry.clone();
        bp.set(
            "on",
            lua.create_function(move |_, (event, f): (String, Function)| {
                let event = match event.as_str() {
                    "file_opened" => Event::FileOpened,
                    "operation_added" => Event::OperationAdded,
                    "operation_changed" => Event::OperationChanged,
                    "before_save" => Event::BeforeSave,
                    other => return Err(runtime(format!("évènement inconnu : « {other} »"))),
                };
                registry.borrow_mut().hooks.push((event, f));
                Ok(())
            })?,
        )?;
    }
    {
        let registry = registry.clone();
        let print = lua.create_function(move |_, args: Variadic<Value>| {
            let line = args
                .iter()
                .map(|v| crate::ui::display(v).unwrap_or_else(|_| v.type_name().to_string()))
                .collect::<Vec<_>>()
                .join("\t");
            if cfg!(debug_assertions) {
                eprintln!("[extension] {line}");
            }
            let mut r = registry.borrow_mut();
            r.log.push(line);
            if r.log.len() > LOG_LINES {
                r.log.remove(0);
            }
            Ok(())
        })?;
        lua.globals().set("print", print.clone())?;
        bp.set("log", print)?;
    }

    // ─────────────────────── Comptes et opérations ───────────────────────
    bp.set(
        "accounts",
        lua.create_function(|lua, opts: Option<Table>| {
            let all = opts
                .map(|o| o.get::<Option<bool>>("all"))
                .transpose()?
                .flatten()
                .unwrap_or(false);
            let refs = with_session(lua, |s| {
                Ok(s.document
                    .account_order
                    .iter()
                    .copied()
                    .filter(|&i| all || !s.document.accounts[i].disabled)
                    .map(|index| AccountRef {
                        file: s.file_id,
                        index,
                    })
                    .collect::<Vec<_>>())
            })?;
            lua.create_sequence_from(refs)
        })?,
    )?;
    bp.set(
        "account",
        lua.create_function(|lua, name: Value| {
            with_session(lua, |s| {
                Ok(account_index(s, &name).ok().map(|index| AccountRef {
                    file: s.file_id,
                    index,
                }))
            })
        })?,
    )?;
    bp.set(
        "current_account",
        lua.create_function(|lua, ()| {
            with_session(lua, |s| {
                Ok(AccountRef {
                    file: s.file_id,
                    index: s.account,
                })
            })
        })?,
    )?;
    let lines = |which: fn(&crate::session::Session) -> &Vec<usize>| {
        move |lua: &Lua, ()| {
            let refs = with_session(lua, |s| {
                let ops = &s.document.accounts[s.account].operations;
                Ok(which(s)
                    .iter()
                    .filter_map(|&k| {
                        ops.get(k)
                            .map(|o| OpRef::new(s.file_id, s.account, o.uid, k))
                    })
                    .collect::<Vec<_>>())
            })?;
            lua.create_sequence_from(refs)
        }
    };
    bp.set("selection", lua.create_function(lines(|s| &s.selection))?)?;
    bp.set("visible", lua.create_function(lines(|s| &s.visible))?)?;
    bp.set(
        "add_account",
        lua.create_function(|lua, spec: Table| {
            let name: String = spec
                .get::<Option<String>>("name")?
                .ok_or_else(|| runtime("bp.add_account : name est obligatoire"))?;
            let initial = match spec.get::<Value>("initial")? {
                Value::Nil => Amount::ZERO,
                v => amount_from_value(&v)?,
            };
            let currency: Option<String> = spec.get("currency")?;
            with_session(lua, |s| {
                let name = bp_core::manage::unique_account_name(&s.document, &name);
                let uid = s.next_uid();
                let document = s.referentials_mut();
                let index = bp_core::manage::add_account(document, &name, initial);
                document.accounts[index].operations[0].uid = uid;
                if let Some(currency) = currency {
                    document.accounts[index].currency = currency;
                }
                Ok(AccountRef {
                    file: s.file_id,
                    index,
                })
            })
        })?,
    )?;
    bp.set(
        "transfer",
        lua.create_function(|lua, spec: Table| {
            let date = match spec.get::<Value>("date")? {
                Value::Nil => Date::today(),
                v => to_date(lua, &v)?,
            };
            let amount = amount_from_value(&spec.get::<Value>("amount")?)?;
            let details: String = spec.get::<Option<String>>("details")?.unwrap_or_default();
            let (first, second) = with_session(lua, |s| {
                let from = account_index(s, &spec.get::<Value>("from")?)?;
                let to = account_index(s, &spec.get::<Value>("to")?)?;
                let category = resolve_category(&s.document, &spec.get::<Value>("category")?)?;
                let request = bp_core::split::TransferRequest {
                    from,
                    to,
                    kind: bp_core::split::TransferKind::Transfer,
                    date,
                    amount,
                    details,
                    category,
                };
                s.operations_mut(from);
                s.operations_mut(to);
                let lines = bp_core::split::create_transfer(&mut s.document, &request)
                    .map_err(|e| runtime(e.to_string()))?;
                s.reset_uid();
                let make = |(a, k): (usize, usize)| {
                    OpRef::new(s.file_id, a, s.document.accounts[a].operations[k].uid, k)
                };
                Ok((make(lines.first), make(lines.second)))
            })?;
            Ok((first, second))
        })?,
    )?;
    bp.set(
        "select",
        lua.create_function(|lua, ops: Value| {
            let list: Vec<OpRef> = match &ops {
                Value::Table(t) => t
                    .sequence_values::<Value>()
                    .map(|v| op_from_value(&v?))
                    .collect::<mlua::Result<_>>()?,
                v => vec![op_from_value(v)?],
            };
            with_session(lua, |s| {
                let mut account = None;
                let mut uids = Vec::new();
                for op in &list {
                    let (a, _) = op.locate(s)?;
                    if *account.get_or_insert(a) == a {
                        uids.push(op.uid);
                    }
                }
                s.select = Some((account.unwrap_or(s.account), uids));
                Ok(())
            })
        })?,
    )?;
    bp.set(
        "find",
        lua.create_function(|lua, uid: u64| {
            with_session(lua, |s| {
                for (a, account) in s.document.accounts.iter().enumerate() {
                    if let Some(k) = account.operations.iter().position(|o| o.uid == uid) {
                        return Ok(Some(OpRef::new(s.file_id, a, uid, k)));
                    }
                }
                Ok(None)
            })
        })?,
    )?;
    bp.set(
        "operation",
        lua.create_function(|lua, spec: Table| {
            // Vérifie une description d'opération sans l'ajouter (formulaires).
            with_session(lua, |s| {
                new_operation(lua, &s.document, &spec).map(|_| true)
            })
        })?,
    )?;

    // ─────────────────────── Référentiels ───────────────────────
    bp.set(
        "categories",
        lua.create_function(|lua, ()| {
            with_session(lua, |s| {
                let list = lua.create_table()?;
                for c in &s.document.categories {
                    list.raw_push(category_table(lua, &s.document, c.id)?)?;
                }
                Ok(list)
            })
        })?,
    )?;
    bp.set(
        "category",
        lua.create_function(|lua, value: Value| {
            with_session(lua, |s| match resolve_category(&s.document, &value) {
                Ok(Some(id)) => category_table(lua, &s.document, id),
                _ => Ok(Value::Nil),
            })
        })?,
    )?;
    bp.set(
        "add_category",
        lua.create_function(|lua, (name, parent): (String, Option<Value>)| {
            with_session(lua, |s| {
                let parent_pos = match &parent {
                    None | Some(Value::Nil) => None,
                    Some(p) => {
                        let id = resolve_category(&s.document, p)?
                            .ok_or_else(|| runtime("catégorie mère inconnue"))?;
                        let pos = s
                            .document
                            .categories
                            .iter()
                            .position(|c| c.id == id)
                            .expect("catégorie");
                        if s.document.categories[pos].is_child {
                            return Err(runtime("une sous-catégorie ne peut pas avoir d'enfant"));
                        }
                        Some(pos)
                    }
                };
                let wanted = if let Some(pos) = parent_pos {
                    let parent = &s.document.categories[pos].name;
                    format!("{parent} > {name}")
                } else {
                    name.clone()
                };
                let wanted_value = Value::String(lua.create_string(&wanted)?);
                if let Ok(Some(id)) = resolve_category(&s.document, &wanted_value)
                    && s.document
                        .category_label(id)
                        .is_some_and(|l| l.to_lowercase() == wanted.to_lowercase())
                {
                    return category_table(lua, &s.document, id);
                }
                let document = s.referentials_mut();
                let id = match parent_pos {
                    Some(pos) => {
                        let end = document.categories[pos + 1..]
                            .iter()
                            .take_while(|c| c.is_child)
                            .count()
                            + pos
                            + 1;
                        bp_core::manage::insert_category(document, &name, true, Some(end))
                    }
                    None => bp_core::manage::insert_category(document, &name, false, None),
                };
                category_table(lua, &s.document, id)
            })
        })?,
    )?;
    bp.set(
        "rename_category",
        lua.create_function(|lua, (category, name): (Value, String)| {
            with_session(lua, |s| {
                let id = resolve_category(&s.document, &category)?
                    .ok_or_else(|| runtime("catégorie inconnue"))?;
                let pos = s
                    .document
                    .categories
                    .iter()
                    .position(|c| c.id == id)
                    .expect("catégorie");
                bp_core::manage::rename_category(s.referentials_mut(), pos, &name);
                Ok(())
            })
        })?,
    )?;
    bp.set(
        "delete_category",
        lua.create_function(|lua, category: Value| {
            with_session(lua, |s| {
                let id = resolve_category(&s.document, &category)?
                    .ok_or_else(|| runtime("catégorie inconnue"))?;
                let pos = s
                    .document
                    .categories
                    .iter()
                    .position(|c| c.id == id)
                    .expect("catégorie");
                s.touch_all_accounts();
                Ok(bp_core::manage::delete_category(s.referentials_mut(), pos))
            })
        })?,
    )?;
    bp.set(
        "modes",
        lua.create_function(|lua, ()| {
            with_session(lua, |s| {
                let list = lua.create_table()?;
                for m in s.document.mode_list() {
                    let t = lua.create_table()?;
                    t.set("name", m.label())?;
                    t.set("debit", bp_core::edit::mode_is_debit(&s.document, &m))?;
                    t.set("custom", matches!(m, bp_core::model::Mode::Custom(_)))?;
                    list.raw_push(t)?;
                }
                Ok(list)
            })
        })?,
    )?;
    bp.set(
        "add_mode",
        lua.create_function(|lua, (name, debit): (String, Option<bool>)| {
            with_session(lua, |s| {
                if let Ok(mode) = crate::objects::resolve_mode(&s.document, &name) {
                    return Ok(mode.label().to_string());
                }
                Ok(bp_core::manage::add_mode(
                    s.referentials_mut(),
                    &name,
                    debit.unwrap_or(true),
                ))
            })
        })?,
    )?;
    bp.set(
        "payees",
        lua.create_function(|lua, ()| {
            with_session(lua, |s| {
                let list = lua.create_table()?;
                for p in &s.document.payees {
                    let t = lua.create_table()?;
                    t.set("caption", p.caption.clone())?;
                    t.set("payee", p.payee.clone())?;
                    t.set("details", p.details.clone())?;
                    t.set("mode", p.mode.label())?;
                    t.set(
                        "category",
                        p.category.and_then(|id| s.document.category_label(id)),
                    )?;
                    t.set("amount", Amount::parse(&p.amount).map(amount_to_f64))?;
                    list.raw_push(t)?;
                }
                Ok(list)
            })
        })?,
    )?;
    bp.set(
        "file",
        lua.create_function(|lua, ()| {
            let info = lua.app_data_ref::<crate::session::Session>().map(|s| {
                (
                    s.path.display().to_string(),
                    s.path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    s.document.user_name.clone(),
                )
            });
            match info {
                None => Ok(Value::Nil),
                Some((path, name, user)) => {
                    let t = lua.create_table()?;
                    t.set("path", path)?;
                    t.set("name", name)?;
                    t.set("user", user)?;
                    Ok(Value::Table(t))
                }
            }
        })?,
    )?;

    // ─────────────────────── Dates, montants ───────────────────────
    bp.set(
        "date",
        lua.create_function(|lua, args: Variadic<Value>| match args.as_slice() {
            [] => date_value(lua, Date::today()),
            [v] => date_value(lua, to_date(lua, v)?),
            [y, m, d] => {
                let n = |v: &Value| {
                    crate::objects::number(v)
                        .map(|f| f as i64)
                        .ok_or_else(|| runtime("bp.date(année, mois, jour)"))
                };
                let date = Date::from_ymd(n(y)? as i32, n(m)? as u32, n(d)? as u32)
                    .ok_or_else(|| runtime("date inexistante"))?;
                date_value(lua, date)
            }
            _ => Err(runtime(
                "bp.date(année, mois, jour) ou bp.date(\"jj/mm/aaaa\")",
            )),
        })?,
    )?;
    bp.set(
        "today",
        lua.create_function(|lua, ()| date_value(lua, Date::today()))?,
    )?;
    bp.set(
        "is_date",
        lua.create_function(|_, v: Value| {
            Ok(matches!(&v, Value::UserData(ud) if ud.is::<LuaDate>()))
        })?,
    )?;
    bp.set(
        "money",
        lua.create_function(|_, (value, currency): (Value, Option<String>)| {
            Ok(money_text(
                amount_from_value(&value)?,
                currency.as_deref().unwrap_or("€"),
            ))
        })?,
    )?;
    bp.set(
        "round",
        lua.create_function(|_, (value, decimals): (f64, Option<i32>)| {
            let factor = 10f64.powi(decimals.unwrap_or(2));
            Ok((value * factor).round() / factor)
        })?,
    )?;
    bp.set(
        "parse_amount",
        lua.create_function(|_, text: String| Ok(Amount::parse(&text).map(amount_to_f64)))?,
    )?;

    // ─────────────────────── Stockage ───────────────────────
    let store = lua.create_table()?;
    {
        let id = id.clone();
        store.set(
            "get",
            lua.create_function(move |lua, key: String| {
                with_session(lua, |s| match load_store(&s.document, &id).get(&key) {
                    Some(v) => from_json(lua, v),
                    None => Ok(Value::Nil),
                })
            })?,
        )?;
    }
    {
        let id = id.clone();
        store.set(
            "set",
            lua.create_function(move |lua, (key, value): (String, Value)| {
                let json = to_json(lua, value)?;
                with_session(lua, |s| {
                    let mut map = load_store(&s.document, &id);
                    if json.is_null() {
                        map.remove(&key);
                    } else {
                        map.insert(key, json);
                    }
                    let value = serde_json::Value::Object(map);
                    let key = store_key(&id);
                    bp_core::feature_store::set(s.store_mut(&key), &key, &value)
                        .map_err(|e| runtime(e.to_string()))?;
                    Ok(())
                })
            })?,
        )?;
    }
    bp.set("store", store)?;
    let prefs = lua.create_table()?;
    {
        let (id, dir) = (id.clone(), ctx.data_dir.clone());
        prefs.set(
            "get",
            lua.create_function(
                move |lua, key: String| match load_prefs(&dir, &id).get(&key) {
                    Some(v) => from_json(lua, v),
                    None => Ok(Value::Nil),
                },
            )?,
        )?;
    }
    {
        let (id, dir) = (id.clone(), ctx.data_dir.clone());
        prefs.set(
            "set",
            lua.create_function(move |lua, (key, value): (String, Value)| {
                let json = to_json(lua, value)?;
                let mut map = load_prefs(&dir, &id);
                if json.is_null() {
                    map.remove(&key);
                } else {
                    map.insert(key, json);
                }
                std::fs::create_dir_all(&dir).map_err(|e| runtime(e.to_string()))?;
                let bytes = serde_json::to_vec_pretty(&map).map_err(|e| runtime(e.to_string()))?;
                bp_core::format_v6::write_atomic(&prefs_path(&dir, &id), &bytes)
                    .map_err(|e| runtime(e.to_string()))
            })?,
        )?;
    }
    bp.set("prefs", prefs)?;

    // ─────────────────────── JSON ───────────────────────
    let json = lua.create_table()?;
    json.set(
        "encode",
        lua.create_function(|lua, (value, pretty): (Value, Option<bool>)| {
            let v = to_json(lua, value)?;
            let text = if pretty.unwrap_or(false) {
                serde_json::to_string_pretty(&v)
            } else {
                serde_json::to_string(&v)
            };
            text.map_err(|e| runtime(e.to_string()))
        })?,
    )?;
    json.set(
        "decode",
        lua.create_function(|lua, text: String| {
            let v: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| runtime(format!("JSON invalide : {e}")))?;
            from_json(lua, &v)
        })?,
    )?;
    bp.set("json", json)?;

    // ─────────────────────── Réseau ───────────────────────
    let http = lua.create_table()?;
    let request = |method: &'static str, registry: Rc<RefCell<Registry>>| {
        move |lua: &Lua, (url, body, opts): (String, Option<String>, Option<Table>)| {
            if !registry.borrow().permissions.iter().any(|p| p == "network") {
                return Err(runtime(
                    "accès à Internet non déclaré : ajoutez permissions = { \"network\" } dans bp.extension{ … }",
                ));
            }
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err(runtime("adresse web invalide (http:// ou https://)"));
            }
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(std::time::Duration::from_secs(20)))
                .user_agent(format!("BankPerfect/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .into();
            let mut headers: Vec<(String, String)> = Vec::new();
            if let Some(h) = opts
                .as_ref()
                .map(|o| o.get::<Option<Table>>("headers"))
                .transpose()?
                .flatten()
            {
                for pair in h.pairs::<String, String>() {
                    headers.push(pair?);
                }
            }
            let result = if method == "GET" {
                let mut req = agent.get(&url);
                for (k, v) in &headers {
                    req = req.header(k, v);
                }
                req.call()
            } else {
                let mut req = agent.post(&url);
                for (k, v) in &headers {
                    req = req.header(k, v);
                }
                req.send(body.unwrap_or_default().as_bytes())
            };
            let mut response = result.map_err(|e| runtime(format!("requête impossible : {e}")))?;
            let status = response.status().as_u16();
            let bytes = response
                .body_mut()
                .with_config()
                .limit(32 << 20)
                .read_to_vec()
                .map_err(|e| runtime(format!("réponse illisible : {e}")))?;
            let t = lua.create_table()?;
            t.set("status", status)?;
            t.set("ok", (200..300).contains(&status))?;
            t.set("body", decode_text(&bytes))?;
            Ok(t)
        }
    };
    http.set(
        "get",
        lua.create_function({
            let f = request("GET", registry.clone());
            move |lua, (url, opts): (String, Option<Table>)| f(lua, (url, None, opts))
        })?,
    )?;
    http.set(
        "post",
        lua.create_function(request("POST", registry.clone()))?,
    )?;
    bp.set("http", http)?;

    // ─────────────────────── Fichiers ───────────────────────
    let files = lua.create_table()?;
    {
        let services = ctx.services.clone();
        files.set(
            "open",
            lua.create_function(move |lua, spec: Option<Table>| {
                let title = spec
                    .as_ref()
                    .map(|s| s.get::<Option<String>>("title"))
                    .transpose()?
                    .flatten()
                    .unwrap_or_else(|| "Ouvrir un fichier".into());
                let Some(path) = services.pick_open(&title, &filters(spec.as_ref())?) else {
                    return Ok(Value::Nil);
                };
                let bytes = std::fs::read(&path).map_err(|e| {
                    runtime(format!("lecture de {} impossible : {e}", path.display()))
                })?;
                let t = lua.create_table()?;
                t.set("path", path.display().to_string())?;
                t.set(
                    "name",
                    path.file_name().map(|n| n.to_string_lossy().into_owned()),
                )?;
                t.set("text", decode_text(&bytes))?;
                Ok(Value::Table(t))
            })?,
        )?;
    }
    {
        let services = ctx.services.clone();
        files.set(
            "save",
            lua.create_function(move |_, spec: Table| {
                let title = spec
                    .get::<Option<String>>("title")?
                    .unwrap_or_else(|| "Enregistrer".into());
                let name = spec.get::<Option<String>>("name")?.unwrap_or_default();
                let text: String = spec
                    .get::<Option<String>>("text")?
                    .ok_or_else(|| runtime("bp.files.save : text est obligatoire"))?;
                let Some(path) = services.pick_save(&title, &name, &filters(Some(&spec))?) else {
                    return Ok(None);
                };
                std::fs::write(&path, text.as_bytes())
                    .map_err(|e| runtime(format!("écriture impossible : {e}")))?;
                Ok(Some(path.display().to_string()))
            })?,
        )?;
    }
    bp.set("files", files)?;
    {
        let services = ctx.services.clone();
        bp.set(
            "open_url",
            lua.create_function(move |_, url: String| {
                if !(url.starts_with("https://") || url.starts_with("http://")) {
                    return Err(runtime("adresse web invalide"));
                }
                services.open_url(&url);
                Ok(())
            })?,
        )?;
    }

    // ─────────────────────── Interface ───────────────────────
    let ui = lua.create_table()?;
    ui.set(
        "toast",
        lua.create_function(|lua, message: Value| {
            let text = crate::ui::display(&message)?;
            if let Some(mut outbox) = lua.app_data_mut::<Outbox>() {
                outbox.toasts.push(text);
            }
            Ok(())
        })?,
    )?;
    bp.set("ui", ui)?;

    // ─────────────────────── Modules ───────────────────────
    {
        let dir = ctx.dir.clone();
        let cache = lua.create_table()?;
        lua.globals().set(
            "require",
            lua.create_function(move |lua, name: String| {
                if let Some(v) = cache.get::<Option<Value>>(name.as_str())? {
                    return Ok(v);
                }
                let dir = dir
                    .as_ref()
                    .ok_or_else(|| runtime("require : réservé aux extensions en dossier"))?;
                let relative = name.trim_start_matches("./").trim_end_matches(".luau");
                if relative.is_empty()
                    || relative
                        .split('/')
                        .any(|p| p.is_empty() || p == "." || p == "..")
                    || relative.contains('\\')
                {
                    return Err(runtime(format!("require : module invalide « {name} »")));
                }
                let path = dir.join(format!("{relative}.luau"));
                let source = std::fs::read_to_string(&path)
                    .map_err(|_| runtime(format!("require : module introuvable « {name} »")))?;
                let value: Value = lua
                    .load(source)
                    .set_name(format!("@{relative}.luau"))
                    .call(())?;
                let value = if value.is_nil() {
                    Value::Boolean(true)
                } else {
                    value
                };
                cache.set(name.as_str(), value.clone())?;
                Ok(value)
            })?,
        )?;
    }

    lua.globals().set("bp", bp)?;
    Ok(())
}
