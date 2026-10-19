//! Fonctionnalités rangées dans le fichier : échéancier (`bp.schedules`…),
//! budget mensuel (`bp.budget`) et règles automatiques (`bp.rules`).
//!
//! Comme `bp.store`, leurs modifications marquent le fichier comme modifié,
//! sont défaites si la commande échoue et ne s'annulent pas avec Ctrl+Z
//! (comme dans les fenêtres de BankPerfect). Seules les opérations modifiées
//! par `bp.rules.apply` rejoignent l'étape d'annulation de la commande.

use bp_core::autofields::{AutoFieldRules, CategoryChange, Rule, RuleField};
use bp_core::budget::{BudgetOptions, BudgetPeriod, BudgetTotal, MonthlyBudget};
use bp_core::model::{Amount, Date, Document, Mode};
use bp_core::scheduler::{self, PeriodUnit, Schedule};
use bp_core::tr;
use mlua::{Lua, Table, Value};

use crate::date::{date_value, to_date};
use crate::objects::{
    AccountRef, OpRef, account_index, amount_from_value, amount_to_f64, op_from_value,
    resolve_category, resolve_mode, runtime, with_session,
};
use crate::session::Session;

fn parse_py(bytes: &[u8]) -> Option<bp_core::pyvalue::PyValue> {
    bp_core::legacy_pickle::parse_py(bytes)
}

// ─────────────────────────────── Échéancier ───────────────────────────────

fn load_schedules(document: &Document) -> mlua::Result<Vec<Schedule>> {
    scheduler::load(document)
        .map(|l| l.schedules)
        .map_err(|e| runtime(tr!("échéancier illisible : {}", e)))
}

fn save_schedules(s: &mut Session, schedules: &[Schedule]) -> mlua::Result<()> {
    let document = s.store_mut(scheduler::FEATURE_KEY);
    scheduler::save(document, schedules).map_err(|e| runtime(e.to_string()))
}

fn account_by_uid(document: &Document, uid: &str) -> Option<usize> {
    document.accounts.iter().position(|a| a.uid == uid)
}

fn unit_name(unit: PeriodUnit) -> &'static str {
    match unit {
        PeriodUnit::Days => "days",
        PeriodUnit::Weeks => "weeks",
        PeriodUnit::Months => "months",
        PeriodUnit::Years => "years",
    }
}

fn parse_unit(text: &str) -> mlua::Result<PeriodUnit> {
    Ok(match text {
        "days" | "day" | "jours" => PeriodUnit::Days,
        "weeks" | "week" | "semaines" => PeriodUnit::Weeks,
        "months" | "month" | "mois" => PeriodUnit::Months,
        "years" | "year" | "ans" => PeriodUnit::Years,
        other => {
            return Err(runtime(tr!(
                "unité inconnue « {} » (\"days\", \"weeks\", \"months\" ou \"years\")",
                other
            )));
        }
    })
}

fn schedule_table(lua: &Lua, s: &Session, index: usize, sc: &Schedule) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let account = |uid: &str| {
        account_by_uid(&s.document, uid).map(|index| AccountRef {
            file: s.file_id,
            index,
        })
    };
    t.set("index", index + 1)?;
    t.set("account", account(&sc.account))?;
    t.set("transfer", sc.transfer.as_deref().and_then(account))?;
    t.set("payee", sc.payee.clone())?;
    t.set("details", sc.details.clone())?;
    t.set("mode", sc.mode.name().to_string())?;
    t.set(
        "category",
        sc.category.and_then(|id| s.document.category_label(id)),
    )?;
    t.set("amount", amount_to_f64(sc.amount))?;
    t.set("next_date", date_value(lua, sc.next_date)?)?;
    t.set(
        "end_date",
        sc.end_date.map(|d| date_value(lua, d)).transpose()?,
    )?;
    t.set("every", sc.period.every)?;
    t.set("unit", unit_name(sc.period.unit))?;
    t.set("day", sc.anchor_day)?;
    t.set("auto", sc.auto_insert)?;
    let postpone = lua.create_table()?;
    postpone.set("saturday", sc.postpone.saturday)?;
    postpone.set("sunday", sc.postpone.sunday)?;
    postpone.set("holiday", sc.postpone.holiday)?;
    t.set("postpone", postpone)?;
    t.set("interval", sc.interval_label())?;
    t.set("finished", sc.is_finished())?;
    Ok(t)
}

/// Applique les champs présents de `t` à l'échéance.
fn apply_schedule(lua: &Lua, s: &Session, sc: &mut Schedule, t: &Table) -> mlua::Result<()> {
    let present = |key: &str| -> mlua::Result<Option<Value>> {
        Ok(match t.get::<Value>(key)? {
            Value::Nil => None,
            v => Some(v),
        })
    };
    if let Some(v) = present("account")? {
        let a = account_index(s, &v)?;
        sc.account = s.document.accounts[a].uid.clone();
    }
    if let Some(v) = present("transfer")? {
        sc.transfer = match v {
            Value::Boolean(false) => None,
            v => {
                let a = account_index(s, &v)?;
                Some(s.document.accounts[a].uid.clone())
            }
        };
    }
    if let Some(v) = present("payee")? {
        sc.payee = crate::ui::data_text(&v)?.trim().to_string();
    }
    if let Some(v) = present("details")? {
        sc.details = crate::ui::data_text(&v)?.trim().to_string();
    }
    if let Some(v) = present("mode")? {
        sc.mode = resolve_mode(&s.document, &crate::ui::data_text(&v)?)?;
    }
    if let Some(v) = present("category")? {
        sc.category = resolve_category(&s.document, &v)?;
    }
    if let Some(v) = present("amount")? {
        sc.amount = amount_from_value(&v)?;
    }
    if let Some(v) = present("next_date")? {
        sc.next_date = to_date(lua, &v)?;
    }
    if let Some(v) = present("end_date")? {
        sc.end_date = match v {
            Value::Boolean(false) => None,
            v => Some(to_date(lua, &v)?),
        };
    }
    if let Some(v) = present("every")? {
        let every = crate::objects::number(&v)
            .filter(|n| *n >= 1.0 && n.fract() == 0.0)
            .ok_or_else(|| runtime(tr!("every : un nombre entier positif est attendu")))?;
        sc.period.every = every as u32;
    }
    if let Some(v) = present("unit")? {
        sc.period.unit = parse_unit(&crate::ui::data_text(&v)?)?;
    }
    if let Some(v) = present("day")? {
        sc.anchor_day = match v {
            Value::Boolean(false) => None,
            v => Some(
                crate::objects::number(&v)
                    .filter(|n| (1.0..=31.0).contains(n))
                    .ok_or_else(|| runtime(tr!("day : un jour de 1 à 31 est attendu")))?
                    as u8,
            ),
        };
    }
    if let Some(v) = present("auto")? {
        sc.auto_insert = matches!(v, Value::Boolean(true));
    }
    if let Some(Value::Table(p)) = present("postpone")? {
        let flag = |k: &str| -> mlua::Result<Option<bool>> { p.get(k) };
        if let Some(b) = flag("saturday")? {
            sc.postpone.saturday = b;
        }
        if let Some(b) = flag("sunday")? {
            sc.postpone.sunday = b;
        }
        if let Some(b) = flag("holiday")? {
            sc.postpone.holiday = b;
        }
    }
    Ok(())
}

fn schedule_index(list: &[Schedule], index: usize) -> mlua::Result<usize> {
    index
        .checked_sub(1)
        .filter(|&i| i < list.len())
        .ok_or_else(|| runtime(tr!("échéance n°{} inexistante", index)))
}

// ─────────────────────────────── Budget ───────────────────────────────

fn load_budget(document: &Document) -> mlua::Result<MonthlyBudget> {
    MonthlyBudget::load(document, &parse_py).map_err(|e| runtime(tr!("budget illisible : {}", e)))
}

fn total_table(lua: &Lua, t: &BudgetTotal) -> mlua::Result<Table> {
    let out = lua.create_table()?;
    out.set("budget", amount_to_f64(t.budget))?;
    out.set("real", amount_to_f64(t.real))?;
    out.set("difference", amount_to_f64(t.difference))?;
    out.set("percent", t.percent)?;
    Ok(out)
}

// ─────────────────────────────── Règles ───────────────────────────────

fn load_rules(document: &Document) -> mlua::Result<AutoFieldRules> {
    AutoFieldRules::load(document, &parse_py).map_err(|e| runtime(tr!("règles illisibles : {}", e)))
}

fn field_name(field: RuleField) -> &'static str {
    match field {
        RuleField::Payee => "payee",
        RuleField::Details => "details",
        RuleField::PayeeOrDetails => "both",
    }
}

fn text_option(t: &Table, key: &str) -> mlua::Result<Option<String>> {
    Ok(match t.get::<Value>(key)? {
        Value::Nil => None,
        v => Some(crate::ui::data_text(&v)?).filter(|s| !s.is_empty()),
    })
}

pub(crate) fn install(lua: &Lua, bp: &Table) -> mlua::Result<()> {
    // Échéancier.
    bp.set(
        "schedules",
        lua.create_function(|lua, ()| {
            with_session(lua, |s| {
                let list = lua.create_table()?;
                for (i, sc) in load_schedules(&s.document)?.iter().enumerate() {
                    list.raw_push(schedule_table(lua, s, i, sc)?)?;
                }
                Ok(list)
            })
        })?,
    )?;
    bp.set(
        "add_schedule",
        lua.create_function(|lua, spec: Table| {
            with_session(lua, |s| {
                let mut list = load_schedules(&s.document)?;
                let mut sc = Schedule {
                    account: s.document.accounts[s.account].uid.clone(),
                    next_date: Date::today(),
                    ..Default::default()
                };
                apply_schedule(lua, s, &mut sc, &spec)?;
                if sc.payee.is_empty() {
                    return Err(runtime(tr!("bp.add_schedule : payee est obligatoire")));
                }
                list.push(sc);
                save_schedules(s, &list)?;
                let i = list.len() - 1;
                schedule_table(lua, s, i, &list[i])
            })
        })?,
    )?;
    bp.set(
        "update_schedule",
        lua.create_function(|lua, (index, spec): (usize, Table)| {
            with_session(lua, |s| {
                let mut list = load_schedules(&s.document)?;
                let i = schedule_index(&list, index)?;
                let mut sc = list[i].clone();
                apply_schedule(lua, s, &mut sc, &spec)?;
                list[i] = sc;
                save_schedules(s, &list)?;
                schedule_table(lua, s, i, &list[i])
            })
        })?,
    )?;
    bp.set(
        "delete_schedule",
        lua.create_function(|lua, index: usize| {
            with_session(lua, |s| {
                let mut list = load_schedules(&s.document)?;
                let i = schedule_index(&list, index)?;
                list.remove(i);
                save_schedules(s, &list)
            })
        })?,
    )?;
    bp.set(
        "forecast",
        lua.create_function(
            |lua, (account, until, opts): (Value, Value, Option<Table>)| {
                let until = to_date(lua, &until)?;
                let monthly = opts
                    .map(|o| o.get::<Option<bool>>("monthly"))
                    .transpose()?
                    .flatten()
                    .unwrap_or(false);
                with_session(lua, |s| {
                    let a = account_index(s, &account)?;
                    let schedules = load_schedules(&s.document)?;
                    let lines = scheduler::forecast(
                        &s.document,
                        &schedules,
                        a,
                        until,
                        Date::today(),
                        monthly,
                    );
                    let list = lua.create_table()?;
                    for line in lines {
                        let t = lua.create_table()?;
                        t.set("date", date_value(lua, line.date)?)?;
                        t.set("payee", line.payee)?;
                        t.set("amount", amount_to_f64(line.amount))?;
                        t.set("balance", amount_to_f64(line.balance))?;
                        t.set(
                            "kind",
                            match line.kind {
                                scheduler::ForecastKind::CurrentBalance => "balance",
                                scheduler::ForecastKind::Schedule => "schedule",
                                scheduler::ForecastKind::Pending => "pending",
                                scheduler::ForecastKind::Month => "month",
                            },
                        )?;
                        t.set("count", line.count)?;
                        list.raw_push(t)?;
                    }
                    Ok(list)
                })
            },
        )?,
    )?;

    // Budget mensuel.
    let budget = lua.create_table()?;
    budget.set(
        "get",
        lua.create_function(|lua, category: Value| {
            with_session(lua, |s| {
                let id = resolve_category(&s.document, &category)?;
                Ok(load_budget(&s.document)?.get(id).map(amount_to_f64))
            })
        })?,
    )?;
    budget.set(
        "set",
        lua.create_function(|lua, (category, amount): (Value, Value)| {
            with_session(lua, |s| {
                let id = resolve_category(&s.document, &category)?;
                let amount = match amount {
                    Value::Nil | Value::Boolean(false) => Amount::ZERO,
                    v => amount_from_value(&v)?,
                };
                let mut b = load_budget(&s.document)?;
                b.set_signed(id, amount);
                s.store_mut(MonthlyBudget::LEGACY_KEY);
                let document = s.store_mut(MonthlyBudget::KEY);
                b.save(document).map_err(|e| runtime(e.to_string()))
            })
        })?,
    )?;
    budget.set(
        "entries",
        lua.create_function(|lua, ()| {
            with_session(lua, |s| {
                let list = lua.create_table()?;
                for e in load_budget(&s.document)?.entries {
                    let t = lua.create_table()?;
                    t.set(
                        "category",
                        e.category.and_then(|id| s.document.category_label(id)),
                    )?;
                    t.set("amount", amount_to_f64(e.amount))?;
                    list.raw_push(t)?;
                }
                Ok(list)
            })
        })?,
    )?;
    budget.set(
        "report",
        lua.create_function(|lua, opts: Option<Table>| {
            let today = Date::today();
            let mut options = BudgetOptions::new(today);
            options.show_unbudgeted = false;
            options.show_zero = false;
            if let Some(o) = &opts {
                if let Some(v) = o.get::<Option<Value>>("month")? {
                    let d = to_date(lua, &v)?.to_naive();
                    use chrono::Datelike;
                    options.period = BudgetPeriod::Month {
                        year: d.year(),
                        month: d.month(),
                    };
                } else if let Some(year) = o.get::<Option<i32>>("year")? {
                    options.period = BudgetPeriod::Year(year);
                } else if o.get::<Option<bool>>("all")?.unwrap_or(false) {
                    options.period = BudgetPeriod::All;
                }
                if let Some(b) = o.get::<Option<bool>>("unbudgeted")? {
                    options.show_unbudgeted = b;
                }
                if let Some(b) = o.get::<Option<bool>>("subcategories")? {
                    options.show_subcategories = b;
                }
            }
            with_session(lua, |s| {
                if let Some(list) = opts
                    .as_ref()
                    .map(|o| o.get::<Option<Table>>("accounts"))
                    .transpose()?
                    .flatten()
                {
                    let mut accounts = Vec::new();
                    for v in list.sequence_values::<Value>() {
                        accounts.push(account_index(s, &v?)?);
                    }
                    options.accounts = Some(accounts);
                }
                let report = load_budget(&s.document)?.report(&s.document, &options, today);
                let out = lua.create_table()?;
                let rows = lua.create_table()?;
                for r in &report.rows {
                    let t = lua.create_table()?;
                    t.set(
                        "category",
                        r.category.and_then(|id| s.document.category_label(id)),
                    )?;
                    t.set("label", r.label.clone())?;
                    t.set("is_child", r.is_child)?;
                    t.set("budget", r.budget.map(amount_to_f64))?;
                    t.set("real", amount_to_f64(r.real))?;
                    t.set("difference", r.difference.map(amount_to_f64))?;
                    t.set("percent", r.percent)?;
                    rows.raw_push(t)?;
                }
                out.set("rows", rows)?;
                out.set("expenses", total_table(lua, &report.expenses)?)?;
                out.set("income", total_table(lua, &report.income)?)?;
                out.set("total", total_table(lua, &report.total)?)?;
                out.set("description", report.description)?;
                Ok(out)
            })
        })?,
    )?;
    bp.set("budget", budget)?;

    // Règles automatiques.
    let rules = lua.create_table()?;
    rules.set(
        "list",
        lua.create_function(|lua, ()| {
            with_session(lua, |s| {
                let list = lua.create_table()?;
                for (i, r) in load_rules(&s.document)?.rules.iter().enumerate() {
                    let t = lua.create_table()?;
                    let (condition, actions) = r.summary(&s.document.categories);
                    t.set("index", i + 1)?;
                    t.set("contains", r.contains.clone())?;
                    t.set("field", field_name(r.field))?;
                    t.set("active", r.active)?;
                    t.set("auto", r.auto_apply)?;
                    t.set("condition", condition)?;
                    t.set("actions", actions)?;
                    list.raw_push(t)?;
                }
                Ok(list)
            })
        })?,
    )?;
    rules.set(
        "add",
        lua.create_function(|lua, spec: Table| {
            with_session(lua, |s| {
                let contains = text_option(&spec, "contains")?
                    .ok_or_else(|| runtime(tr!("bp.rules.add : contains est obligatoire")))?;
                let mut rule = Rule::new(contains);
                rule.field = match text_option(&spec, "field")?.as_deref() {
                    None | Some("payee") => RuleField::Payee,
                    Some("details") => RuleField::Details,
                    Some("both") => RuleField::PayeeOrDetails,
                    Some(other) => {
                        return Err(runtime(tr!(
                            "field « {} » inconnu (\"payee\", \"details\" ou \"both\")",
                            other
                        )));
                    }
                };
                rule.plus_is_or = spec.get::<Option<bool>>("any")?.unwrap_or(false);
                rule.set_payee = text_option(&spec, "payee")?;
                rule.set_details = text_option(&spec, "details")?;
                rule.set_date = match spec.get::<Value>("date")? {
                    Value::Nil => None,
                    v => Some(to_date(lua, &v)?.to_string()),
                };
                rule.set_mode = match text_option(&spec, "mode")? {
                    None => None,
                    Some(label) => Some(match resolve_mode(&s.document, &label)? {
                        Mode::Standard(m) => m,
                        Mode::Custom(_) => {
                            return Err(runtime(tr!(
                                "une règle n'applique que les modes standard"
                            )));
                        }
                    }),
                };
                rule.set_category = match spec.get::<Value>("category")? {
                    Value::Nil => None,
                    Value::Boolean(false) => Some(CategoryChange::Clear),
                    v => resolve_category(&s.document, &v)?.map(CategoryChange::Set),
                };
                rule.only_uncategorized = spec
                    .get::<Option<bool>>("only_uncategorized")?
                    .unwrap_or(false);
                rule.active = spec.get::<Option<bool>>("active")?.unwrap_or(true);
                rule.auto_apply = spec.get::<Option<bool>>("auto")?.unwrap_or(false);
                rule.validate(&s.document.categories)
                    .map_err(|e| runtime(tr!("règle refusée : {}", e)))?;
                let mut all = load_rules(&s.document)?;
                all.rules.push(rule);
                s.store_mut(AutoFieldRules::LEGACY_KEY);
                let document = s.store_mut(AutoFieldRules::KEY);
                all.save(document).map_err(|e| runtime(e.to_string()))?;
                Ok(all.rules.len())
            })
        })?,
    )?;
    rules.set(
        "remove",
        lua.create_function(|lua, index: usize| {
            with_session(lua, |s| {
                let mut all = load_rules(&s.document)?;
                let i = index
                    .checked_sub(1)
                    .filter(|&i| i < all.rules.len())
                    .ok_or_else(|| runtime(tr!("règle n°{} inexistante", index)))?;
                all.rules.remove(i);
                s.store_mut(AutoFieldRules::LEGACY_KEY);
                let document = s.store_mut(AutoFieldRules::KEY);
                all.save(document).map_err(|e| runtime(e.to_string()))
            })
        })?,
    )?;
    rules.set(
        "apply",
        lua.create_function(|lua, ops: Option<Value>| {
            let list: Option<Vec<OpRef>> = match &ops {
                None | Some(Value::Nil) => None,
                Some(Value::Table(t)) => Some(
                    t.sequence_values::<Value>()
                        .map(|v| op_from_value(&v?))
                        .collect::<mlua::Result<_>>()?,
                ),
                Some(v) => Some(vec![op_from_value(v)?]),
            };
            with_session(lua, |s| {
                // (compte, lignes) : les opérations données, sinon celles
                // affichées dans la grille.
                let mut targets: Vec<(usize, Vec<usize>)> = Vec::new();
                match list {
                    None => targets.push((s.account, s.visible.clone())),
                    Some(list) => {
                        for op in list {
                            let (a, k) = op.locate(s)?;
                            match targets.iter_mut().find(|(t, _)| *t == a) {
                                Some((_, lines)) => lines.push(k),
                                None => targets.push((a, vec![k])),
                            }
                        }
                    }
                }
                let all = load_rules(&s.document)?;
                let mut changed = 0;
                for (a, mut lines) in targets {
                    if s.locked {
                        let ops = &s.document.accounts[a].operations;
                        lines.retain(|&k| ops[k].tack != bp_core::model::Tack::Reconciled);
                    }
                    let mut account = s.document.accounts[a].clone();
                    let outcome = all.apply_active(&s.document.categories, &mut account, &lines);
                    if !outcome.changed.is_empty() {
                        changed += outcome.changed.len();
                        *s.operations_mut(a) = account.operations;
                    }
                }
                Ok(changed)
            })
        })?,
    )?;
    bp.set("rules", rules)?;

    Ok(())
}
