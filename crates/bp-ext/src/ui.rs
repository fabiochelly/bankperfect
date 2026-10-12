//! Fenêtres des extensions : description déclarative produite par Luau,
//! rendue par l'application avec ses propres composants.
//!
//! Une fenêtre garde un `state` (table Luau). Sa fonction `view(state)`
//! renvoie la liste des éléments à afficher ; les champs sont liés à une clé
//! de `state`. À chaque interaction, l'application envoie un [`UiEvent`] : la
//! valeur est écrite dans `state`, le gestionnaire éventuel est appelé, puis
//! `view` est rappelée pour reconstruire la fenêtre.

use std::collections::HashMap;

use bp_core::model::{Amount, Mode};
use mlua::{Function, Lua, Table, Value};

use crate::date::{LuaDate, date_value, parse_text};
use crate::objects::{
    AccountRef, OpRef, amount_from_value, amount_to_f64, resolve_category, resolve_mode, runtime,
    with_session,
};

/// Couleur sémantique (suit le thème) ou fixe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Accent,
    Positive,
    Negative,
    Warning,
    Muted,
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextStyle {
    #[default]
    Body,
    Muted,
    Strong,
    Heading,
    Small,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonStyle {
    #[default]
    Normal,
    Primary,
    Danger,
    Link,
}

/// Largeur d'une fenêtre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowSize {
    Small,
    Medium,
    Large,
    /// Fraction de la largeur de l'écran.
    Fraction(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputKind {
    Text {
        multiline: bool,
        password: bool,
    },
    Number {
        decimals: u32,
    },
    Amount,
    Date,
    Checkbox,
    Switch,
    /// Liste déroulante.
    Select {
        options: Vec<String>,
        selected: Option<usize>,
    },
    /// Contrôle segmenté.
    Choice {
        options: Vec<(String, Option<String>)>,
        selected: Option<usize>,
    },
    /// Compte (index réel).
    Account {
        selected: Option<usize>,
    },
    Category {
        selected: Option<u32>,
    },
    Mode {
        selected: Option<Mode>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub key: String,
    pub label: Option<String>,
    pub placeholder: Option<String>,
    /// Texte du champ (champs texte, nombre, montant, date) ou libellé de la
    /// case à cocher.
    pub text: String,
    pub checked: bool,
    pub kind: InputKind,
    pub enabled: bool,
    /// Aide sous le champ.
    pub hint: Option<String>,
    /// Le texte saisi n'est pas valide (nombre, montant, date).
    pub invalid: bool,
    pub width: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub text: String,
    pub color: Option<Color>,
    pub strong: bool,
    pub align: Option<Align>,
    /// Montant (aligné à droite, coloré selon le signe).
    pub amount: Option<f64>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub title: String,
    pub align: Align,
    pub width: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableView {
    /// Numéro du tableau dans la fenêtre (évènements).
    pub id: usize,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub selected: Vec<usize>,
    pub selectable: bool,
    pub multiple: bool,
    pub activable: bool,
    pub height: Option<f32>,
    pub empty: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub label: String,
    pub value: f64,
    pub text: String,
    pub color: Option<Color>,
}

/// Forme d'un graphique (`ui.chart`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartStyle {
    /// Courbe de la première série (évolution).
    Line,
    /// Colonnes groupées, une couleur par série.
    Columns,
    /// Barres horizontales groupées.
    Bars,
    /// Anneau des valeurs (absolues) de la première série.
    Donut,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Widget {
    Text {
        text: String,
        style: TextStyle,
        color: Option<Color>,
        align: Align,
    },
    Money {
        amount: f64,
        currency: String,
        style: TextStyle,
        colored: bool,
    },
    Section {
        title: Option<String>,
        icon: Option<String>,
        children: Vec<Widget>,
    },
    Row {
        children: Vec<Widget>,
        align: Align,
    },
    Columns(Vec<Widget>),
    Input(Input),
    Button {
        id: usize,
        label: String,
        icon: Option<String>,
        style: ButtonStyle,
        enabled: bool,
    },
    Table(TableView),
    Stat {
        label: String,
        value: String,
        icon: Option<String>,
        color: Option<Color>,
        hint: Option<String>,
    },
    Pill {
        text: String,
        color: Color,
    },
    Progress {
        fraction: f32,
        text: Option<String>,
        color: Option<Color>,
    },
    Bars(Vec<Bar>),
    Chart {
        style: ChartStyle,
        data: bp_core::reports::ChartData,
        currency: String,
        height: f32,
    },
    Notice {
        text: String,
        color: Color,
        icon: Option<String>,
    },
    Separator,
    Space(f32),
}

/// Interaction de l'utilisateur dans une fenêtre d'extension.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    /// Texte d'un champ (texte, nombre, montant, date).
    Text {
        key: String,
        text: String,
    },
    Bool {
        key: String,
        value: bool,
    },
    /// Choix dans une liste (index de l'option ; compte : index réel).
    Pick {
        key: String,
        index: Option<usize>,
    },
    Category {
        key: String,
        id: Option<u32>,
    },
    Mode {
        key: String,
        mode: Mode,
    },
    Click {
        button: usize,
    },
    TableSelect {
        table: usize,
        rows: Vec<usize>,
    },
    TableActivate {
        table: usize,
        row: usize,
    },
    /// Échap ou fermeture : la fenêtre renvoie `nil`.
    Dismiss,
}

/// Lignes d'un tableau : identifiants et liaison.
struct TableBinding {
    key: Option<String>,
    multiple: bool,
    ids: Vec<Value>,
    on_activate: Option<Function>,
    on_select: Option<Function>,
}

/// Fenêtre ouverte par une extension.
pub struct Window {
    pub id: u64,
    pub title: String,
    pub icon: Option<String>,
    pub subtitle: Option<String>,
    pub size: WindowSize,
    pub body: Vec<Widget>,
    pub footer: Vec<Widget>,
    pub(crate) state: Table,
    view: Option<Function>,
    static_body: Option<Value>,
    buttons: Option<Value>,
    pub(crate) on_change: Option<Function>,
    pub(crate) handlers: Vec<Function>,
    tables: Vec<TableBinding>,
    /// Valeurs des options des listes, par clé.
    options: HashMap<String, Vec<Value>>,
    kinds: HashMap<String, InputKind>,
    /// Texte en cours de saisie des champs typés (nombre, montant, date).
    buffers: HashMap<String, String>,
    pub(crate) dirty: bool,
    pub(crate) result: Option<Value>,
    pub(crate) closed: bool,
}

pub(crate) fn color(value: &Value) -> mlua::Result<Option<Color>> {
    let Value::String(s) = value else {
        return Ok(None);
    };
    let text = s.to_str()?.to_lowercase();
    Ok(Some(match text.as_str() {
        "accent" | "blue" | "bleu" => Color::Accent,
        "positive" | "green" | "vert" | "success" => Color::Positive,
        "negative" | "red" | "rouge" | "danger" => Color::Negative,
        "warning" | "orange" => Color::Warning,
        "muted" | "gray" | "grey" | "gris" => Color::Muted,
        hex => {
            let hex = hex.trim_start_matches('#');
            let channel = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or(""), 16);
            match (hex.len(), channel(0), channel(2), channel(4)) {
                (6, Ok(r), Ok(g), Ok(b)) => Color::Rgb(r, g, b),
                _ => return Err(runtime(format!("couleur inconnue : « {text} »"))),
            }
        }
    }))
}

fn align(value: Option<String>) -> Align {
    match value.as_deref() {
        Some("right" | "droite") => Align::Right,
        Some("center" | "centre") => Align::Center,
        _ => Align::Left,
    }
}

fn text_style(value: Option<String>) -> TextStyle {
    match value.as_deref() {
        Some("muted") => TextStyle::Muted,
        Some("strong" | "bold") => TextStyle::Strong,
        Some("heading" | "title") => TextStyle::Heading,
        Some("small") => TextStyle::Small,
        Some("mono" | "code") => TextStyle::Mono,
        _ => TextStyle::Body,
    }
}

/// Texte affiché d'une valeur Luau quelconque.
pub(crate) fn display(value: &Value) -> mlua::Result<String> {
    Ok(match value {
        Value::Nil => String::new(),
        Value::Boolean(true) => "✓".into(),
        Value::Boolean(false) => String::new(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}").replace('.', ",")
            }
        }
        Value::String(s) => s.to_str()?.to_string(),
        other => other.to_string()?,
    })
}

/// Clé de comparaison d'un identifiant de ligne.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RowKey {
    Int(i64),
    Num(u64),
    Text(String),
    Op(u64, u64),
    Account(u64, usize),
    Date(i32),
    Ptr(usize),
}

fn row_key(value: &Value) -> RowKey {
    match value {
        Value::Integer(i) => RowKey::Int(*i),
        Value::Number(n) if n.fract() == 0.0 => RowKey::Int(*n as i64),
        Value::Number(n) => RowKey::Num(n.to_bits()),
        Value::String(s) => RowKey::Text(s.to_string_lossy()),
        Value::UserData(ud) => {
            if let Ok(op) = ud.borrow::<OpRef>() {
                RowKey::Op(op.file, op.uid)
            } else if let Ok(a) = ud.borrow::<AccountRef>() {
                RowKey::Account(a.file, a.index)
            } else if let Ok(d) = ud.borrow::<LuaDate>() {
                RowKey::Date(d.0.0)
            } else {
                RowKey::Ptr(value.to_pointer() as usize)
            }
        }
        other => RowKey::Ptr(other.to_pointer() as usize),
    }
}

/// Éléments d'une table de widgets (clés entières, trous ignorés).
fn children(value: &Value) -> mlua::Result<Vec<Value>> {
    let Value::Table(t) = value else {
        return Ok(Vec::new());
    };
    let mut items: Vec<(i64, Value)> = Vec::new();
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair?;
        if let Some(i) = crate::objects::number(&k)
            .filter(|f| f.fract() == 0.0)
            .map(|f| f as i64)
        {
            items.push((i, v));
        }
    }
    items.sort_by_key(|(i, _)| *i);
    Ok(items.into_iter().map(|(_, v)| v).collect())
}

/// Enfants d'un conteneur : `children = { … }` ou ses éléments numérotés.
fn kids(t: &Table) -> mlua::Result<Value> {
    Ok(match t.get::<Value>("children")? {
        Value::Nil => Value::Table(t.clone()),
        v => v,
    })
}

fn number_text(value: f64, decimals: u32) -> String {
    if decimals == 0 && value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        let text = format!("{value:.*}", decimals.max(1) as usize);
        let text = if decimals == 0 {
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            text
        };
        text.replace('.', ",")
    }
}

fn parse_number(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{202f}' && *c != '\u{a0}')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    Amount::parse(&cleaned)
        .filter(|_| {
            cleaned
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '-' | ',' | '.'))
        })
        .map(amount_to_f64)
}

impl Window {
    /// Ouvre la fenêtre décrite par `spec` et construit son contenu.
    pub(crate) fn open(lua: &Lua, id: u64, spec: &Table) -> mlua::Result<Self> {
        let state = match spec.get::<Value>("state")? {
            Value::Table(t) => t,
            Value::Nil => lua.create_table()?,
            _ => return Err(runtime("state doit être une table")),
        };
        let size = match spec.get::<Value>("size")? {
            Value::String(s) => match s.to_str()?.as_ref() {
                "small" | "petite" => WindowSize::Small,
                "large" | "grande" => WindowSize::Large,
                _ => WindowSize::Medium,
            },
            Value::Number(n) => WindowSize::Fraction((n as f32).clamp(0.2, 0.95)),
            Value::Integer(n) => WindowSize::Fraction((n as f32 / 100.0).clamp(0.2, 0.95)),
            _ => WindowSize::Medium,
        };
        let (view, static_body) = match spec.get::<Value>("view")? {
            Value::Function(f) => (Some(f), None),
            Value::Nil => (None, Some(spec.get::<Value>("body")?)),
            Value::Table(t) => (None, Some(Value::Table(t))),
            _ => return Err(runtime("view doit être une fonction")),
        };
        let mut window = Self {
            id,
            title: spec.get::<Option<String>>("title")?.unwrap_or_default(),
            icon: spec.get("icon")?,
            subtitle: spec.get("subtitle")?,
            size,
            body: Vec::new(),
            footer: Vec::new(),
            state,
            view,
            static_body,
            buttons: Some(spec.get::<Value>("buttons")?).filter(|v| !v.is_nil()),
            on_change: spec.get("on_change")?,
            handlers: Vec::new(),
            tables: Vec::new(),
            options: HashMap::new(),
            kinds: HashMap::new(),
            buffers: HashMap::new(),
            dirty: true,
            result: None,
            closed: false,
        };
        window.rebuild(lua)?;
        Ok(window)
    }

    pub(crate) fn rebuild(&mut self, lua: &Lua) -> mlua::Result<()> {
        self.dirty = false;
        self.handlers.clear();
        self.tables.clear();
        self.options.clear();
        self.kinds.clear();
        let body = match (&self.view, &self.static_body) {
            (Some(view), _) => view.call::<Value>(self.state.clone())?,
            (None, Some(body)) => body.clone(),
            (None, None) => Value::Nil,
        };
        self.body = self.widgets(lua, &body)?;
        let buttons = match &self.buttons {
            Some(Value::Function(f)) => f.call::<Value>(self.state.clone())?,
            Some(v) => v.clone(),
            None => Value::Nil,
        };
        self.footer = self.widgets(lua, &buttons)?;
        if self.footer.is_empty() {
            let close = lua
                .globals()
                .get::<Table>("bp")?
                .get::<Table>("ui")?
                .get::<Function>("button")?
                .call::<Value>(lua.create_table_from([
                    (
                        Value::Integer(1),
                        Value::String(lua.create_string("Fermer")?),
                    ),
                    (
                        Value::String(lua.create_string("cancel")?),
                        Value::Boolean(true),
                    ),
                ])?)?;
            self.footer = self.widgets(lua, &lua.create_sequence_from([close])?.into_lua(lua)?)?;
        }
        Ok(())
    }

    fn widgets(&mut self, lua: &Lua, list: &Value) -> mlua::Result<Vec<Widget>> {
        let mut out = Vec::new();
        for item in children(list)? {
            if let Some(w) = self.widget(lua, &item)? {
                out.push(w);
            }
        }
        Ok(out)
    }

    fn handler(&mut self, value: Value) -> mlua::Result<Option<usize>> {
        match value {
            Value::Function(f) => {
                self.handlers.push(f);
                Ok(Some(self.handlers.len() - 1))
            }
            Value::Nil => Ok(None),
            _ => Err(runtime("on_click doit être une fonction")),
        }
    }

    fn widget(&mut self, lua: &Lua, value: &Value) -> mlua::Result<Option<Widget>> {
        let t = match value {
            Value::Nil | Value::Boolean(false) => return Ok(None),
            Value::Table(t) => t.clone(),
            other => {
                return Ok(Some(Widget::Text {
                    text: display(other)?,
                    style: TextStyle::Body,
                    color: None,
                    align: Align::Left,
                }));
            }
        };
        let kind: String = t
            .get::<Option<String>>("type")?
            .ok_or_else(|| runtime("élément de fenêtre sans type (utilisez bp.ui.*)"))?;
        let first = t.get::<Value>(1)?;
        Ok(Some(match kind.as_str() {
            "text" => Widget::Text {
                text: display(&first)?,
                style: text_style(t.get("style")?),
                color: color(&t.get("color")?)?,
                align: align(t.get("align")?),
            },
            "money" => Widget::Money {
                amount: amount_to_f64(amount_from_value(&t.get::<Value>("value")?)?),
                currency: t
                    .get::<Option<String>>("currency")?
                    .unwrap_or_else(|| "€".into()),
                style: text_style(t.get("style")?),
                colored: t.get::<Option<bool>>("colored")?.unwrap_or(true),
            },
            "section" | "card" => Widget::Section {
                title: t.get("title")?,
                icon: t.get("icon")?,
                children: self.widgets(lua, &kids(&t)?)?,
            },
            "row" => Widget::Row {
                children: self.widgets(lua, &kids(&t)?)?,
                align: align(t.get("align")?),
            },
            "columns" => Widget::Columns(self.widgets(lua, &kids(&t)?)?),
            "button" => {
                let id = self.handler(t.get("on_click")?)?;
                Widget::Button {
                    id: id.unwrap_or(usize::MAX),
                    label: display(&first)?,
                    icon: t.get("icon")?,
                    style: match t.get::<Option<String>>("style")?.as_deref() {
                        Some("primary") => ButtonStyle::Primary,
                        Some("danger") => ButtonStyle::Danger,
                        Some("link") => ButtonStyle::Link,
                        _ => ButtonStyle::Normal,
                    },
                    enabled: id.is_some() && t.get::<Option<bool>>("enabled")?.unwrap_or(true),
                }
            }
            "stat" => Widget::Stat {
                label: t.get::<Option<String>>("label")?.unwrap_or_default(),
                value: match t.get::<Value>("value")? {
                    Value::Table(m)
                        if m.get::<Option<String>>("type")?.as_deref() == Some("money") =>
                    {
                        let amount = amount_from_value(&m.get::<Value>("value")?)?;
                        let currency = m
                            .get::<Option<String>>("currency")?
                            .unwrap_or_else(|| "€".into());
                        crate::api::money_text(amount, &currency)
                    }
                    v => display(&v)?,
                },
                icon: t.get("icon")?,
                color: color(&t.get("color")?)?,
                hint: t.get("hint")?,
            },
            "pill" => Widget::Pill {
                text: display(&first)?,
                color: color(&t.get("color")?)?.unwrap_or(Color::Accent),
            },
            "notice" => Widget::Notice {
                text: display(&first)?,
                color: color(&t.get("color")?)?.unwrap_or(Color::Accent),
                icon: t.get("icon")?,
            },
            "progress" => Widget::Progress {
                fraction: t
                    .get::<Option<f64>>("value")?
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0) as f32,
                text: t.get("text")?,
                color: color(&t.get("color")?)?,
            },
            "bars" => {
                let mut bars = Vec::new();
                for item in children(&t.get::<Value>("data")?)? {
                    let Value::Table(b) = item else { continue };
                    let value: f64 = b.get::<Option<f64>>("value")?.unwrap_or(0.0);
                    bars.push(Bar {
                        label: display(&b.get::<Value>("label")?)?,
                        value,
                        text: match b.get::<Value>("text")? {
                            Value::Nil => crate::api::money_text(
                                Amount((value * Amount::SCALE as f64).round() as i64),
                                &t.get::<Option<String>>("currency")?
                                    .unwrap_or_else(|| "€".into()),
                            ),
                            v => display(&v)?,
                        },
                        color: color(&b.get("color")?)?,
                    });
                }
                Widget::Bars(bars)
            }
            "chart" => {
                use bp_core::reports::{ChartData, ChartPoint, ChartSeries, ChartUnit};
                let style = match t.get::<Option<String>>("style")?.as_deref() {
                    None | Some("columns") => ChartStyle::Columns,
                    Some("line") => ChartStyle::Line,
                    Some("bars") => ChartStyle::Bars,
                    Some("donut") => ChartStyle::Donut,
                    Some(other) => {
                        return Err(runtime(format!(
                            "ui.chart : style « {other} » inconnu (\"line\", \"columns\", \"bars\" ou \"donut\")"
                        )));
                    }
                };
                let mut series = Vec::new();
                if let Some(list) = t.get::<Option<Table>>("series")? {
                    // { labels = { … }, series = { { name, values = { … } }, … } }
                    let labels: Vec<String> = children(&t.get::<Value>("labels")?)?
                        .iter()
                        .map(display)
                        .collect::<mlua::Result<_>>()?;
                    for item in children(&Value::Table(list))? {
                        let Value::Table(serie) = item else { continue };
                        let values = children(&serie.get::<Value>("values")?)?;
                        series.push(ChartSeries {
                            name: serie.get::<Option<String>>("name")?.unwrap_or_default(),
                            points: values
                                .iter()
                                .enumerate()
                                .map(|(i, v)| ChartPoint {
                                    label: labels.get(i).cloned().unwrap_or_default(),
                                    value: crate::objects::number(v).unwrap_or(0.0),
                                })
                                .collect(),
                        });
                    }
                } else {
                    // { data = { { label, value }, … } } : une seule série.
                    let mut points = Vec::new();
                    for item in children(&t.get::<Value>("data")?)? {
                        let Value::Table(p) = item else { continue };
                        let label = match p.get::<Value>("label")? {
                            Value::Nil => p.get::<Value>(1)?,
                            v => v,
                        };
                        let value = match p.get::<Value>("value")? {
                            Value::Nil => p.get::<Value>(2)?,
                            v => v,
                        };
                        points.push(ChartPoint {
                            label: display(&label)?,
                            value: crate::objects::number(&value).unwrap_or(0.0),
                        });
                    }
                    series.push(ChartSeries {
                        name: t.get::<Option<String>>("name")?.unwrap_or_default(),
                        points,
                    });
                }
                Widget::Chart {
                    style,
                    data: ChartData {
                        unit: match t.get::<Option<String>>("unit")?.as_deref() {
                            Some("count") => ChartUnit::Count,
                            _ => ChartUnit::Amount,
                        },
                        series,
                    },
                    currency: t
                        .get::<Option<String>>("currency")?
                        .unwrap_or_else(|| "€".into()),
                    height: t
                        .get::<Option<f32>>("height")?
                        .unwrap_or(260.0)
                        .clamp(120.0, 900.0),
                }
            }
            "separator" => Widget::Separator,
            "space" => Widget::Space(t.get::<Option<f32>>("size")?.unwrap_or(12.0)),
            "table" => Widget::Table(self.table(lua, &t)?),
            "input" => Widget::Input(self.input(lua, &t)?),
            other => return Err(runtime(format!("élément de fenêtre inconnu : « {other} »"))),
        }))
    }

    fn input(&mut self, lua: &Lua, t: &Table) -> mlua::Result<Input> {
        let key: String = t
            .get::<Option<String>>(1)?
            .or(t.get("key")?)
            .ok_or_else(|| runtime("champ sans clé : bp.ui.input{ \"clé\", … }"))?;
        let field: String = t
            .get::<Option<String>>("field")?
            .unwrap_or_else(|| "text".into());
        let value = self.state.get::<Value>(key.as_str())?;
        let buffer = self.buffers.get(&key).cloned();
        let mut invalid = false;
        let mut checked = false;
        let mut text = String::new();
        let kind = match field.as_str() {
            "text" | "password" | "multiline" => {
                text = display(&value)?;
                InputKind::Text {
                    multiline: field == "multiline",
                    password: field == "password",
                }
            }
            "number" => {
                let decimals: u32 = t.get::<Option<u32>>("decimals")?.unwrap_or(0);
                let current = crate::objects::number(&value);
                text = match (&buffer, current) {
                    (Some(b), c) if parse_number(b) == c => b.clone(),
                    (_, Some(c)) => number_text(c, decimals),
                    (Some(b), None) => {
                        invalid = !b.trim().is_empty();
                        b.clone()
                    }
                    (None, None) => String::new(),
                };
                InputKind::Number { decimals }
            }
            "amount" => {
                let current = match &value {
                    Value::Nil => None,
                    v => Some(amount_from_value(v)?),
                };
                text = match (&buffer, current) {
                    (Some(b), c) if Amount::parse(b) == c => b.clone(),
                    (_, Some(c)) => c.format(2, ','),
                    (Some(b), None) => {
                        invalid = !b.trim().is_empty();
                        b.clone()
                    }
                    (None, None) => String::new(),
                };
                InputKind::Amount
            }
            "date" => {
                let current = match &value {
                    Value::Nil => None,
                    v => Some(crate::date::to_date(lua, v)?),
                };
                text = match (&buffer, current) {
                    (Some(b), c) if parse_text(b) == c => b.clone(),
                    (_, Some(c)) => c.to_string(),
                    (Some(b), None) => {
                        invalid = !b.trim().is_empty();
                        b.clone()
                    }
                    (None, None) => String::new(),
                };
                InputKind::Date
            }
            "checkbox" | "switch" => {
                checked = !matches!(value, Value::Nil | Value::Boolean(false));
                text = t.get::<Option<String>>("label")?.unwrap_or_default();
                if field == "switch" {
                    InputKind::Switch
                } else {
                    InputKind::Checkbox
                }
            }
            "select" | "choice" => {
                let mut values = Vec::new();
                let mut labels = Vec::new();
                for option in children(&t.get::<Value>("options")?)? {
                    match option {
                        Value::Table(o) => {
                            let v = o.get::<Value>("value")?;
                            let v = if v.is_nil() { o.get::<Value>(1)? } else { v };
                            let label = match o.get::<Value>("label")? {
                                Value::Nil => display(&o.get::<Value>(2)?)
                                    .ok()
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or(display(&v)?),
                                l => display(&l)?,
                            };
                            labels.push((label, o.get::<Option<String>>("icon")?));
                            values.push(v);
                        }
                        v => {
                            labels.push((display(&v)?, None));
                            values.push(v);
                        }
                    }
                }
                let mut selected = None;
                for (i, v) in values.iter().enumerate() {
                    if v.equals(&value)? {
                        selected = Some(i);
                        break;
                    }
                }
                self.options.insert(key.clone(), values);
                if field == "select" {
                    InputKind::Select {
                        options: labels.into_iter().map(|(l, _)| l).collect(),
                        selected,
                    }
                } else {
                    InputKind::Choice {
                        options: labels,
                        selected,
                    }
                }
            }
            "account" => InputKind::Account {
                selected: match &value {
                    Value::Nil => None,
                    v => with_session(lua, |s| crate::objects::account_index(s, v).map(Some))?,
                },
            },
            "category" => InputKind::Category {
                selected: with_session(lua, |s| resolve_category(&s.document, &value))?,
            },
            "mode" => InputKind::Mode {
                selected: match &value {
                    Value::String(m) => {
                        let m = m.to_str()?.to_string();
                        Some(with_session(lua, |s| resolve_mode(&s.document, &m))?)
                    }
                    _ => None,
                },
            },
            other => return Err(runtime(format!("type de champ inconnu : « {other} »"))),
        };
        self.kinds.insert(key.clone(), kind.clone());
        let is_check = matches!(kind, InputKind::Checkbox | InputKind::Switch);
        Ok(Input {
            label: if is_check { None } else { t.get("label")? },
            placeholder: t.get("placeholder")?,
            text,
            checked,
            kind,
            enabled: !t.get::<Option<bool>>("disabled")?.unwrap_or(false),
            hint: t.get("hint")?,
            invalid,
            width: t.get("width")?,
            key,
        })
    }

    fn table(&mut self, lua: &Lua, t: &Table) -> mlua::Result<TableView> {
        let mut columns = Vec::new();
        for c in children(&t.get::<Value>("columns")?)? {
            columns.push(match c {
                Value::Table(c) => Column {
                    title: display(&c.get::<Value>(1)?)
                        .ok()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(c.get::<Option<String>>("title")?.unwrap_or_default()),
                    align: align(c.get("align")?),
                    width: c.get("width")?,
                },
                v => Column {
                    title: display(&v)?,
                    align: Align::Left,
                    width: None,
                },
            });
        }
        let key: Option<String> = t.get("select")?;
        let multiple = t.get::<Option<bool>>("multiple")?.unwrap_or(false);
        let mut rows = Vec::new();
        let mut ids = Vec::new();
        for (n, row) in children(&t.get::<Value>("rows")?)?.into_iter().enumerate() {
            let Value::Table(r) = row else { continue };
            let mut cells = Vec::new();
            for cell in children(&Value::Table(r.clone()))? {
                cells.push(self.cell(lua, &cell)?);
            }
            let id = r.get::<Value>("id")?;
            ids.push(if id.is_nil() {
                Value::Integer(n as i64 + 1)
            } else {
                id
            });
            rows.push(cells);
        }
        let mut selected = Vec::new();
        if let Some(key) = &key {
            let current = self.state.get::<Value>(key.as_str())?;
            let wanted: std::collections::HashSet<RowKey> = if multiple {
                children(&current)?.iter().map(row_key).collect()
            } else if current.is_nil() {
                Default::default()
            } else {
                [row_key(&current)].into()
            };
            if !wanted.is_empty() {
                selected = ids
                    .iter()
                    .enumerate()
                    .filter(|(_, id)| wanted.contains(&row_key(id)))
                    .map(|(i, _)| i)
                    .collect();
            }
        }
        let on_activate: Option<Function> = t.get("on_activate")?;
        let on_select: Option<Function> = t.get("on_select")?;
        let id = self.tables.len();
        let view = TableView {
            id,
            columns,
            rows,
            selected,
            selectable: key.is_some() || on_select.is_some(),
            multiple,
            activable: on_activate.is_some(),
            height: t.get("height")?,
            empty: t
                .get::<Option<String>>("empty")?
                .unwrap_or_else(|| "Aucune ligne".into()),
        };
        self.tables.push(TableBinding {
            key,
            multiple,
            ids,
            on_activate,
            on_select,
        });
        Ok(view)
    }

    fn cell(&mut self, lua: &Lua, value: &Value) -> mlua::Result<Cell> {
        let plain = |text: String| Cell {
            text,
            color: None,
            strong: false,
            align: None,
            amount: None,
            icon: None,
        };
        let Value::Table(t) = value else {
            return Ok(plain(display(value)?));
        };
        match t.get::<Option<String>>("type")?.as_deref() {
            Some("money") => {
                let amount = amount_from_value(&t.get::<Value>("value")?)?;
                let currency = t
                    .get::<Option<String>>("currency")?
                    .unwrap_or_else(|| "€".into());
                Ok(Cell {
                    text: crate::api::money_text(amount, &currency),
                    color: None,
                    strong: t.get::<Option<String>>("style")?.as_deref() == Some("strong"),
                    align: Some(Align::Right),
                    amount: t
                        .get::<Option<bool>>("colored")?
                        .unwrap_or(true)
                        .then(|| amount_to_f64(amount)),
                    icon: None,
                })
            }
            Some("pill") | Some("text") | None => Ok(Cell {
                text: display(&t.get::<Value>(1)?)
                    .or_else(|_| display(&t.get::<Value>("text")?))?,
                color: color(&t.get("color")?)?,
                strong: matches!(
                    t.get::<Option<String>>("style")?.as_deref(),
                    Some("strong" | "bold")
                ) || t.get::<Option<bool>>("bold")?.unwrap_or(false),
                align: t.get::<Option<String>>("align")?.map(|a| align(Some(a))),
                amount: None,
                icon: t.get("icon")?,
            }),
            Some(other) => {
                let _ = lua;
                Err(runtime(format!(
                    "cellule de tableau invalide : « {other} »"
                )))
            }
        }
    }

    /// Applique une interaction : renvoie le gestionnaire à appeler et ses
    /// arguments supplémentaires (après `state`).
    pub(crate) fn apply(
        &mut self,
        lua: &Lua,
        event: UiEvent,
    ) -> mlua::Result<Option<(Function, Vec<Value>)>> {
        self.dirty = true;
        let change = |w: &Self, key: &str| -> Option<(Function, Vec<Value>)> {
            w.on_change
                .clone()
                .map(|f| (f, vec![Value::String(lua.create_string(key).expect("clé"))]))
        };
        match event {
            UiEvent::Text { key, text } => {
                let kind = self.kinds.get(&key).cloned();
                let value = match kind {
                    Some(InputKind::Number { .. }) => match parse_number(&text) {
                        Some(n) => Some(Value::Number(n)),
                        None if text.trim().is_empty() => Some(Value::Nil),
                        None => None,
                    },
                    Some(InputKind::Amount) => match Amount::parse(&text) {
                        Some(a) => Some(Value::Number(amount_to_f64(a))),
                        None if text.trim().is_empty() => Some(Value::Nil),
                        None => None,
                    },
                    Some(InputKind::Date) => match parse_text(&text) {
                        Some(d) => Some(date_value(lua, d)?),
                        None if text.trim().is_empty() => Some(Value::Nil),
                        None => None,
                    },
                    _ => Some(Value::String(lua.create_string(&text)?)),
                };
                self.buffers.insert(key.clone(), text);
                match value {
                    Some(v) => {
                        let before = self.state.get::<Value>(key.as_str())?;
                        if before.equals(&v)? {
                            return Ok(None);
                        }
                        self.state.set(key.as_str(), v)?;
                        Ok(change(self, &key))
                    }
                    None => Ok(None),
                }
            }
            UiEvent::Bool { key, value } => {
                self.state.set(key.as_str(), value)?;
                Ok(change(self, &key))
            }
            UiEvent::Pick { key, index } => {
                let value = match (self.kinds.get(&key), index) {
                    (_, None) => Value::Nil,
                    (Some(InputKind::Account { .. }), Some(i)) => {
                        let file = with_session(lua, |s| Ok(s.file_id))?;
                        Value::UserData(lua.create_userdata(AccountRef { file, index: i })?)
                    }
                    (_, Some(i)) => self
                        .options
                        .get(&key)
                        .and_then(|o| o.get(i))
                        .cloned()
                        .unwrap_or(Value::Nil),
                };
                self.state.set(key.as_str(), value)?;
                Ok(change(self, &key))
            }
            UiEvent::Category { key, id } => {
                let label = match id {
                    Some(id) => with_session(lua, |s| Ok(s.document.category_label(id)))?,
                    None => None,
                };
                self.state.set(key.as_str(), label)?;
                Ok(change(self, &key))
            }
            UiEvent::Mode { key, mode } => {
                self.state.set(key.as_str(), mode.label())?;
                Ok(change(self, &key))
            }
            UiEvent::Click { button } => {
                Ok(self.handlers.get(button).cloned().map(|f| (f, Vec::new())))
            }
            UiEvent::TableSelect { table, rows } => {
                let Some(binding) = self.tables.get(table) else {
                    return Ok(None);
                };
                let picked: Vec<Value> = rows
                    .iter()
                    .filter_map(|&r| binding.ids.get(r).cloned())
                    .collect();
                let value = if binding.multiple {
                    Value::Table(lua.create_sequence_from(picked)?)
                } else {
                    picked.into_iter().next().unwrap_or(Value::Nil)
                };
                if let Some(key) = &binding.key {
                    self.state.set(key.as_str(), value.clone())?;
                }
                if let Some(f) = binding.on_select.clone() {
                    return Ok(Some((f, vec![value])));
                }
                Ok(binding.key.clone().and_then(|k| change(self, &k)))
            }
            UiEvent::TableActivate { table, row } => {
                let Some(binding) = self.tables.get(table) else {
                    return Ok(None);
                };
                let id = binding.ids.get(row).cloned().unwrap_or(Value::Nil);
                Ok(binding.on_activate.clone().map(|f| (f, vec![id])))
            }
            UiEvent::Dismiss => {
                self.closed = true;
                self.result = None;
                Ok(None)
            }
        }
    }
}

use mlua::IntoLua;
