//! Formats d'import fournis par les extensions (`bp.importer`) : le fichier
//! est lu par l'extension, l'aperçu, les doublons et le choix du compte sont
//! ceux de l'import de BankPerfect.

use bp_core::import::{ImportedAccount, ImportedOperation, ParsedImport, RejectReason, RejectedLine};
use bp_core::model::{Document, Mode};
use mlua::{Function, Lua, Table, Value};

use crate::date::to_date;
use crate::objects::{amount_from_value, number, parse_tack, resolve_mode, runtime};

/// Format déclaré par `bp.importer{ … }`.
#[derive(Clone)]
pub(crate) struct ImporterSpec {
    pub name: String,
    /// Extensions de fichier acceptées, en minuscules, sans le point.
    pub extensions: Vec<String>,
    pub detect: Option<Function>,
    pub parse: Function,
}

impl ImporterSpec {
    pub fn parse_spec(spec: &Table) -> mlua::Result<Self> {
        let name: String = spec
            .get::<Option<String>>("name")?
            .ok_or_else(|| runtime("bp.importer : name est obligatoire"))?;
        let extensions: Vec<String> = match spec.get::<Value>("extensions")? {
            Value::String(s) => vec![s.to_str()?.to_string()],
            Value::Table(t) => t.sequence_values::<String>().collect::<mlua::Result<_>>()?,
            _ => {
                return Err(runtime(
                    "bp.importer : extensions est obligatoire ({ \"csv\" }, { \"txt\", \"tsv\" }…)",
                ));
            }
        };
        let extensions: Vec<String> = extensions
            .iter()
            .map(|e| e.trim().trim_start_matches('.').to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        if extensions.is_empty() {
            return Err(runtime("bp.importer : extensions est vide"));
        }
        Ok(Self {
            name,
            extensions,
            detect: spec.get("detect")?,
            parse: spec
                .get::<Option<Function>>("parse")?
                .ok_or_else(|| runtime("bp.importer : parse (fonction) est obligatoire"))?,
        })
    }
}

/// Table `{ name, path, text, data }` passée à `detect` et `parse`.
pub(crate) fn file_table(lua: &Lua, path: &std::path::Path, data: &[u8]) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("path", path.display().to_string())?;
    t.set(
        "name",
        path.file_name().map(|n| n.to_string_lossy().into_owned()),
    )?;
    t.set("text", crate::api::decode_text(data))?;
    t.set("data", lua.create_string(data)?)?;
    Ok(t)
}

fn text(t: &Table, key: &str) -> mlua::Result<String> {
    Ok(match t.get::<Value>(key)? {
        Value::Nil => String::new(),
        Value::String(s) => s.to_str()?.trim().to_string(),
        v => crate::ui::display(&v)?,
    })
}

fn optional(t: &Table, key: &str) -> mlua::Result<Option<String>> {
    Ok(Some(text(t, key)?).filter(|s| !s.is_empty()))
}

/// Ce que renvoie `parse` : une liste d'opérations (un seul compte),
/// `{ name, number, currency, operations = { … } }` ou
/// `{ accounts = { { name, number, currency, operations }, … } }`.
pub(crate) fn parsed_import(lua: &Lua, document: &Document, value: Value) -> mlua::Result<ParsedImport> {
    let Value::Table(t) = value else {
        return Err(runtime(
            "parse doit renvoyer une liste d'opérations ou { accounts = … }",
        ));
    };
    let mut parsed = ParsedImport::default();
    let mut line = 0;
    let accounts: Vec<Table> = if let Some(list) = t.get::<Option<Table>>("accounts")? {
        list.sequence_values::<Table>().collect::<mlua::Result<_>>()?
    } else if t.contains_key("operations")? {
        vec![t]
    } else {
        let single = lua.create_table()?;
        single.set("operations", t)?;
        vec![single]
    };
    for a in accounts {
        let mut account = ImportedAccount {
            name: optional(&a, "name")?,
            account_id: optional(&a, "number")?,
            bank_id: optional(&a, "bank")?,
            branch_id: optional(&a, "branch")?,
            currency: optional(&a, "currency")?,
            ..Default::default()
        };
        let ops: Table = a
            .get::<Option<Table>>("operations")?
            .ok_or_else(|| runtime("parse : un compte sans liste operations"))?;
        for op in ops.sequence_values::<Table>() {
            let op = op?;
            line += 1;
            match imported_operation(lua, document, &op, line)? {
                Ok(o) => account.operations.push(o),
                Err(reason) => parsed.rejected.push(RejectedLine {
                    line,
                    text: [text(&op, "payee")?, text(&op, "details")?]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · "),
                    reason,
                }),
            }
        }
        if !account.operations.is_empty() {
            parsed.accounts.push(account);
        }
    }
    Ok(parsed)
}

fn imported_operation(
    lua: &Lua,
    document: &Document,
    t: &Table,
    line: usize,
) -> mlua::Result<Result<ImportedOperation, RejectReason>> {
    let date = match t.get::<Value>("date")? {
        Value::Nil => return Ok(Err(RejectReason::MissingDate)),
        v => match to_date(lua, &v) {
            Ok(d) => d,
            Err(_) => return Ok(Err(RejectReason::InvalidDate(crate::ui::display(&v)?))),
        },
    };
    let value_date = match t.get::<Value>("value_date")? {
        Value::Nil => None,
        v => to_date(lua, &v).ok(),
    };
    let amount = match t.get::<Value>("amount")? {
        Value::Nil => return Ok(Err(RejectReason::MissingAmount)),
        v => match amount_from_value(&v) {
            Ok(a) => a,
            Err(_) => return Ok(Err(RejectReason::InvalidAmount(crate::ui::display(&v)?))),
        },
    };
    if amount.0 == 0 {
        return Ok(Err(RejectReason::ZeroAmount));
    }
    let payee = text(t, "payee")?;
    let details = text(t, "details")?;
    let mode_text = text(t, "mode")?;
    let mut cheque_number = optional(t, "cheque")?;
    let mode = match resolve_mode(document, &mode_text) {
        Ok(mode) if !mode_text.is_empty() => mode,
        _ => {
            let (mode, number) = bp_core::import::guess_mode(&mode_text, &payee, &details, amount.is_negative());
            cheque_number = cheque_number.or(number);
            Mode::Standard(mode)
        }
    };
    let (category, subcategory) = match optional(t, "category")? {
        None => (None, None),
        Some(label) => match label.split_once('>') {
            Some((parent, child)) => (
                Some(parent.trim().to_string()),
                Some(child.trim().to_string()).filter(|s| !s.is_empty()),
            ),
            None => (Some(label), None),
        },
    };
    let tack = match optional(t, "tack")? {
        None => None,
        Some(text) => Some(parse_tack(&text)?),
    };
    let fitid = match t.get::<Value>("id")? {
        Value::Nil => None,
        v => Some(number(&v).map_or_else(|| crate::ui::display(&v), |n| Ok(format!("{n}")))?),
    };
    Ok(Ok(ImportedOperation {
        line,
        date,
        value_date,
        amount,
        mode,
        cheque_number,
        payee,
        details,
        category,
        subcategory,
        tack,
        fitid,
    }))
}
