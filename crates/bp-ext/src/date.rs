//! Dates côté Luau : `bp.date(2026, 10, 12)`, `bp.today() + 30`, `d.month`…

use bp_core::model::Date;
use bp_core::tr;
use chrono::{Datelike, Months, NaiveDate};
use mlua::{Lua, MetaMethod, UserData, UserDataFields, UserDataMethods, Value};

const WEEKDAYS: [&str; 7] = [
    "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
];
const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LuaDate(pub Date);

impl LuaDate {
    fn naive(self) -> NaiveDate {
        self.0.to_naive()
    }
}

/// Ajoute des mois en restant dans le mois visé (31/01 + 1 mois = 28/02).
fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let result = if months >= 0 {
        date.checked_add_months(Months::new(months as u32))
    } else {
        date.checked_sub_months(Months::new(months.unsigned_abs()))
    };
    result.unwrap_or(date)
}

impl UserData for LuaDate {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("day", |_, d| Ok(d.naive().day()));
        fields.add_field_method_get("month", |_, d| Ok(d.naive().month()));
        fields.add_field_method_get("year", |_, d| Ok(d.naive().year()));
        // 1 = lundi … 7 = dimanche.
        fields.add_field_method_get("weekday", |_, d| {
            Ok(d.naive().weekday().number_from_monday())
        });
        fields.add_field_method_get("weekday_name", |_, d| {
            Ok(WEEKDAYS[d.naive().weekday().num_days_from_monday() as usize])
        });
        fields.add_field_method_get("month_name", |_, d| Ok(MONTHS[d.naive().month0() as usize]));
        fields.add_field_method_get("serial", |_, d| Ok(d.0.0));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("add_days", |_, d, n: i32| Ok(LuaDate(Date(d.0.0 + n))));
        methods.add_method("add_months", |_, d, n: i32| {
            Ok(LuaDate(Date::from_naive(add_months(d.naive(), n))))
        });
        methods.add_method("add_years", |_, d, n: i32| {
            Ok(LuaDate(Date::from_naive(add_months(d.naive(), n * 12))))
        });
        methods.add_method("start_of_month", |_, d, ()| {
            Ok(LuaDate(Date::from_naive(
                d.naive().with_day(1).expect("jour 1"),
            )))
        });
        methods.add_method("end_of_month", |_, d, ()| {
            let first = d.naive().with_day(1).expect("jour 1");
            let next = add_months(first, 1);
            Ok(LuaDate(Date(Date::from_naive(next).0 - 1)))
        });
        methods.add_method("start_of_year", |_, d, ()| {
            Ok(LuaDate(
                Date::from_ymd(d.naive().year(), 1, 1).expect("1er janvier"),
            ))
        });
        methods.add_method("end_of_year", |_, d, ()| {
            Ok(LuaDate(
                Date::from_ymd(d.naive().year(), 12, 31).expect("31 décembre"),
            ))
        });
        methods.add_method("days_until", |lua, d, other: Value| {
            Ok(to_date(lua, &other)?.0 - d.0.0)
        });
        methods.add_method("format", |_, d, pattern: Option<String>| {
            let n = d.naive();
            Ok(match pattern {
                None => d.0.to_string(),
                Some(p) => p
                    .replace("aaaa", &format!("{:04}", n.year()))
                    .replace("aa", &format!("{:02}", n.year() % 100))
                    .replace("mm", &format!("{:02}", n.month()))
                    .replace("jj", &format!("{:02}", n.day()))
                    .replace("mois", MONTHS[n.month0() as usize]),
            })
        });
        methods.add_method("iso", |_, d, ()| {
            Ok(d.naive().format("%Y-%m-%d").to_string())
        });
        methods.add_meta_method(MetaMethod::ToString, |_, d, ()| Ok(d.0.to_string()));
        methods.add_meta_method(MetaMethod::Eq, |lua, d, other: Value| {
            Ok(to_date(lua, &other).is_ok_and(|o| o == d.0))
        });
        methods.add_meta_function(MetaMethod::Lt, |lua, (a, b): (Value, Value)| {
            Ok(to_date(lua, &a)? < to_date(lua, &b)?)
        });
        methods.add_meta_function(MetaMethod::Le, |lua, (a, b): (Value, Value)| {
            Ok(to_date(lua, &a)? <= to_date(lua, &b)?)
        });
        methods.add_meta_function(MetaMethod::Add, |lua, (a, b): (Value, Value)| {
            let (date, days) = match (&a, &b) {
                (Value::UserData(_), _) => (to_date(lua, &a)?, b),
                _ => (to_date(lua, &b)?, a),
            };
            let days = crate::objects::number(&days)
                .map(|f| f.round() as i64)
                .ok_or_else(|| {
                    mlua::Error::runtime(tr!("une date s'additionne avec un nombre de jours"))
                })?;
            Ok(LuaDate(Date(date.0 + days as i32)))
        });
        methods.add_meta_function(MetaMethod::Sub, |lua, (a, b): (Value, Value)| {
            let date = to_date(lua, &a)?;
            if let Some(days) = crate::objects::number(&b).map(|f| f.round() as i64) {
                return Ok(Value::UserData(
                    lua.create_userdata(LuaDate(Date(date.0 - days as i32)))?,
                ));
            }
            Ok(Value::Integer((date.0 - to_date(lua, &b)?.0) as i64))
        });
    }
}

/// Date passée par une extension : objet date, ou texte `jj/mm/aaaa`
/// (`aaaa-mm-jj` accepté).
pub(crate) fn to_date(_lua: &Lua, value: &Value) -> mlua::Result<Date> {
    match value {
        Value::UserData(ud) => ud
            .borrow::<LuaDate>()
            .map(|d| d.0)
            .map_err(|_| mlua::Error::runtime(tr!("une date est attendue"))),
        Value::String(s) => {
            let text = s.to_str()?.to_string();
            parse_text(&text)
                .ok_or_else(|| mlua::Error::runtime(tr!("date invalide : « {} »", text)))
        }
        _ => Err(mlua::Error::runtime(tr!(
            "une date est attendue (reçu : {})",
            value.type_name()
        ))),
    }
}

pub(crate) fn parse_text(text: &str) -> Option<Date> {
    let text = text.trim();
    if let Ok(d) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Some(Date::from_naive(d));
    }
    Date::parse_fr(text, Date::today())
}

pub(crate) fn date_value(lua: &Lua, date: Date) -> mlua::Result<Value> {
    Ok(Value::UserData(lua.create_userdata(LuaDate(date))?))
}
