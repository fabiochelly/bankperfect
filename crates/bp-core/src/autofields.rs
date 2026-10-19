//! Calcul automatique des champs (ex-script `Auto Fields/auto_fields.py`).
//!
//! Une règle teste chaque opération puis, si elle s'applique, impose certains
//! champs :
//!
//! - **condition** : sur le tiers, le détail ou les deux, une liste de mots
//!   séparés par `+` (tous présents ; `!mot` = absent ; option « + = OU » :
//!   l'un des mots suffit), sans distinction de casse ; ou bien une **formule**
//!   commençant par `=`, par exemple
//!   `="tresor public" in tiers and montant < -70 and montant > -100`,
//!   évaluée par l'interpréteur sûr de [`crate::formula`] ;
//! - **actions** : mode (seulement si le signe du montant correspond), tiers,
//!   détail, date (valeur fixe ou formule `=…`) et catégorie, avec l'option
//!   « uniquement les opérations sans catégorie ».
//!
//! Variables des formules (comme l'original) : `date` (`jj-mm-aaaa`), `mode`,
//! `tiers`, `details` (en minuscules dans les conditions, telles quelles dans
//! les actions), `montant`/`amount`/`value`, `categ`/`categorie`/`category`
//! (nom de la catégorie, vide si aucune), `categ_id` (−1 si aucune), et les
//! versions non converties en minuscules `paymode`, `thirdparty`/`third`/
//! `people`, `detail`.
//!
//! Les règles actives marquées « appliquer automatiquement » s'appliquent aux
//! opérations ajoutées ou modifiées ([`AutoFieldRules::apply_to_new_lines`]).
//! Import et export reprennent le format texte de l'original (une règle par
//! ligne, `repr()` Python d'un dict), relu sans `eval`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::feature_store::{self, FeatureError};
use crate::formula::{Formula, FormulaError, Value};
use crate::model::{Account, Category, Date, Document, Mode, Operation, StandardMode};
use crate::pyvalue::{LegacyError, PyValue, parse_repr, to_repr};
use crate::tr;

/// Variables utilisables dans les formules.
pub const VARIABLES: [&str; 16] = [
    "date",
    "mode",
    "tiers",
    "details",
    "montant",
    "categ",
    "categorie",
    "category",
    "categ_id",
    "paymode",
    "thirdparty",
    "third",
    "people",
    "amount",
    "value",
    "detail",
];

/// Champ testé par une condition « contient ».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RuleField {
    #[default]
    Payee,
    Details,
    PayeeOrDetails,
}

impl RuleField {
    pub const ALL: [RuleField; 3] = [Self::Payee, Self::Details, Self::PayeeOrDetails];

    pub fn label(self) -> &'static str {
        match self {
            Self::Payee => tr!("Tiers"),
            Self::Details => tr!("Détails"),
            Self::PayeeOrDetails => tr!("Tiers ou Détails"),
        }
    }

    fn legacy_index(self) -> i64 {
        match self {
            Self::Payee => 0,
            Self::Details => 1,
            Self::PayeeOrDetails => 2,
        }
    }
}

/// Changement de catégorie imposé par une règle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CategoryChange {
    /// « - Aucune - » : retire la catégorie.
    Clear,
    Set(u32),
}

/// Règle de calcul automatique.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub field: RuleField,
    /// Mots cherchés (`a+b`, `!x`) ou formule commençant par `=`.
    pub contains: String,
    /// « Considérer le caractère "+" comme un "OU" ».
    #[serde(default)]
    pub plus_is_or: bool,
    /// Mode à appliquer (modes standard seulement, comme l'original).
    #[serde(default)]
    pub set_mode: Option<StandardMode>,
    /// Tiers à appliquer : texte, ou formule commençant par `=`.
    #[serde(default)]
    pub set_payee: Option<String>,
    /// Détail à appliquer : texte, ou formule commençant par `=`.
    #[serde(default)]
    pub set_details: Option<String>,
    /// Date à appliquer (`JJ/MM/AAAA`), ou formule commençant par `=`.
    #[serde(default)]
    pub set_date: Option<String>,
    #[serde(default)]
    pub set_category: Option<CategoryChange>,
    /// « Modifier uniquement les opérations sans catégorie ».
    #[serde(default)]
    pub only_uncategorized: bool,
    /// « Activer la règle ».
    #[serde(default)]
    pub active: bool,
    /// « Appliquer automatiquement à chaque nouvelle ligne ».
    #[serde(default)]
    pub auto_apply: bool,
}

/// Partie d'une règle concernée par une erreur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulePart {
    Condition,
    Payee,
    Details,
    Date,
}

impl RulePart {
    pub fn label(self) -> &'static str {
        match self {
            Self::Condition => tr!("condition"),
            Self::Payee => tr!("tiers"),
            Self::Details => tr!("détail"),
            Self::Date => tr!("date"),
        }
    }
}

/// Règle inutilisable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    EmptyCondition,
    Formula { part: RulePart, error: FormulaError },
    UnknownVariable { part: RulePart, name: String },
    UnknownCategory(u32),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCondition => f.write_str(tr!("la valeur cherchée est vide")),
            Self::Formula { part, error } => f.write_str(&tr!(
                "formule de {part} invalide : {error}",
                part = part.label(),
                error = error
            )),
            Self::UnknownVariable { part, name } => f.write_str(&tr!(
                "formule de {part} : variable inconnue « {name} »",
                part = part.label(),
                name = name
            )),
            Self::UnknownCategory(_) => f.write_str(tr!("la catégorie à appliquer n'existe pas")),
        }
    }
}

/// Erreur d'évaluation sur une opération (la ligne n'est alors pas retenue,
/// ou le champ concerné n'est pas modifié).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineError {
    /// Indice de l'opération dans le compte.
    pub line: usize,
    pub part: RulePart,
    pub message: String,
}

/// Résultat de l'application d'une règle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleOutcome {
    /// Opérations vérifiant la condition (et sans catégorie si l'option est
    /// cochée).
    pub matched: Vec<usize>,
    /// Opérations effectivement modifiées.
    pub changed: Vec<usize>,
    pub errors: Vec<LineError>,
}

enum Condition {
    Words { terms: Vec<String>, any: bool },
    Formula(Formula),
}

enum Template {
    Text(String),
    Formula(Formula),
}

/// Règle analysée, prête à être appliquée.
pub struct CompiledRule<'a> {
    rule: &'a Rule,
    condition: Condition,
    payee: Option<Template>,
    details: Option<Template>,
    date: Option<Template>,
}

fn compile_formula(source: &str, part: RulePart) -> Result<Formula, RuleError> {
    let formula = Formula::parse(source).map_err(|error| RuleError::Formula { part, error })?;
    if let Some(name) = formula
        .variables()
        .into_iter()
        .find(|v| !VARIABLES.contains(&v.as_str()))
    {
        return Err(RuleError::UnknownVariable { part, name });
    }
    Ok(formula)
}

fn compile_template(value: &Option<String>, part: RulePart) -> Result<Option<Template>, RuleError> {
    Ok(match value {
        None => None,
        Some(text) => Some(match text.strip_prefix('=') {
            Some(source) => Template::Formula(compile_formula(source, part)?),
            None => Template::Text(text.clone()),
        }),
    })
}

/// Nom de la catégorie (sans parent, espaces de bord retirés).
fn category_name(categories: &[Category], id: Option<u32>) -> String {
    id.and_then(|id| categories.iter().find(|c| c.id == id))
        .map(|c| c.name.trim().to_string())
        .unwrap_or_default()
}

/// Libellé complet `Parent > Enfant`, comme la liste de l'original.
fn category_label(categories: &[Category], id: u32) -> Option<String> {
    let pos = categories.iter().position(|c| c.id == id)?;
    let category = &categories[pos];
    let name = category.name.trim();
    if !category.is_child {
        return Some(name.to_string());
    }
    Some(match categories[..pos].iter().rev().find(|c| !c.is_child) {
        Some(parent) => format!("{} > {}", parent.name.trim(), name),
        None => name.to_string(),
    })
}

/// Valeur d'une variable de formule pour `op`.
fn variable(op: &Operation, categories: &[Category], name: &str, condition: bool) -> Option<Value> {
    let lower = |s: &str| {
        if condition {
            s.to_lowercase()
        } else {
            s.to_string()
        }
    };
    Some(match name {
        "date" => {
            let d = op.date.to_string();
            Value::Str(d.replace('/', "-"))
        }
        "mode" => Value::Str(lower(&op.mode_name())),
        "tiers" => Value::Str(lower(&op.payee)),
        "details" => Value::Str(lower(&op.details)),
        "montant" | "amount" | "value" => Value::Float(op.amount.as_f64()),
        "categ" | "categorie" | "category" => Value::Str(category_name(categories, op.category)),
        "categ_id" => Value::Int(op.category.map_or(-1, i64::from)),
        "paymode" => Value::Str(op.mode_name()),
        "thirdparty" | "third" | "people" => Value::Str(op.payee.clone()),
        "detail" => Value::Str(op.details.clone()),
        _ => return None,
    })
}

/// Analyse une date imposée : `JJ/MM/AAAA` (ou `JJ-MM-AAAA`, format de la
/// variable `date`), chiffres sur 2, 2 et 4 positions.
fn parse_rule_date(text: &str) -> Option<Date> {
    let text = text.trim();
    let parts: Vec<&str> = text.split(['/', '-']).collect();
    let [d, m, y] = parts[..] else {
        return None;
    };
    let digits = |s: &str, n: usize| s.len() == n && s.chars().all(|c| c.is_ascii_digit());
    if !(digits(d, 2) && digits(m, 2) && digits(y, 4)) {
        return None;
    }
    Date::from_ymd(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
}

impl Rule {
    /// Règle vide (valeurs par défaut de la fenêtre « Ajouter »).
    pub fn new(contains: impl Into<String>) -> Self {
        Self {
            field: RuleField::Payee,
            contains: contains.into(),
            plus_is_or: false,
            set_mode: None,
            set_payee: None,
            set_details: None,
            set_date: None,
            set_category: None,
            only_uncategorized: false,
            active: false,
            auto_apply: false,
        }
    }

    pub fn is_formula(&self) -> bool {
        self.contains.starts_with('=')
    }

    /// Analyse la condition et les formules d'actions ; une règle mal formée
    /// est refusée avant toute modification.
    pub fn compile(&self) -> Result<CompiledRule<'_>, RuleError> {
        if self.contains.is_empty() {
            return Err(RuleError::EmptyCondition);
        }
        let condition = match self.contains.strip_prefix('=') {
            Some(source) => Condition::Formula(compile_formula(source, RulePart::Condition)?),
            None => {
                let lower = self.contains.to_lowercase();
                Condition::Words {
                    terms: lower.split('+').map(str::to_string).collect(),
                    any: self.plus_is_or,
                }
            }
        };
        Ok(CompiledRule {
            rule: self,
            condition,
            payee: compile_template(&self.set_payee, RulePart::Payee)?,
            details: compile_template(&self.set_details, RulePart::Details)?,
            date: compile_template(&self.set_date, RulePart::Date)?,
        })
    }

    /// Vérifie la règle (syntaxe des formules, catégorie existante).
    pub fn validate(&self, categories: &[Category]) -> Result<(), RuleError> {
        self.compile()?.check_category(categories)
    }

    /// Opérations de `lines` qui vérifient la règle (pour la confirmation
    /// « Il y a N lignes qui vérifient la règle… »).
    pub fn find_matches(
        &self,
        categories: &[Category],
        account: &Account,
        lines: &[usize],
    ) -> Result<RuleOutcome, RuleError> {
        let compiled = self.compile()?;
        let mut outcome = RuleOutcome::default();
        for &line in lines {
            if let Some(op) = account.operations.get(line)
                && compiled.accepts(op, categories, line, &mut outcome.errors)
            {
                outcome.matched.push(line);
            }
        }
        Ok(outcome)
    }

    /// Applique la règle aux opérations `lines` du compte.
    pub fn apply(
        &self,
        categories: &[Category],
        account: &mut Account,
        lines: &[usize],
    ) -> Result<RuleOutcome, RuleError> {
        let compiled = self.compile()?;
        compiled.check_category(categories)?;
        Ok(compiled.apply(categories, account, lines))
    }

    /// Texte des deux colonnes de la liste des règles : « Si je trouve » et
    /// « Je modifie les champs ».
    pub fn summary(&self, categories: &[Category]) -> (String, String) {
        let condition = if self.is_formula() {
            self.contains.clone()
        } else {
            format!("{}: {}", self.field.label(), self.contains)
        };
        let mut actions = Vec::new();
        if let Some(mode) = self.set_mode {
            actions.push(tr!("Mode: {}", mode.label()));
        }
        if let Some(payee) = &self.set_payee {
            actions.push(tr!("Tiers: {}", payee));
        }
        if let Some(details) = &self.set_details {
            actions.push(tr!("Détails: {}", details));
        }
        if let Some(change) = self.set_category {
            let label = match change {
                CategoryChange::Clear => tr!("- Aucune -").to_string(),
                CategoryChange::Set(id) => {
                    category_label(categories, id).unwrap_or_else(|| tr!("- Aucune -").into())
                }
            };
            actions.push(tr!("Categ: {}", label));
        }
        if let Some(date) = &self.set_date {
            actions.push(tr!("Date: {}", date));
        }
        (condition, actions.join(", "))
    }

    /// Conversion depuis un dict de l'ancien script (clés `field`, `contains`,
    /// `do_mode`, `mode`, `do_third`, `third`, `do_info`, `info`, `do_categ`,
    /// `categ`, `active`, `withnewline`, `contains_or`, `emptycateg`,
    /// `do_date`, `date` ; les cinq dernières sont facultatives).
    pub fn from_legacy(value: &PyValue) -> Result<Self, LegacyError> {
        if value.as_dict().is_none() {
            return Err(LegacyError::Structure(
                tr!("règle : dictionnaire attendu").into(),
            ));
        }
        let Some(contains) = value.get("contains").and_then(PyValue::as_str) else {
            return Err(LegacyError::Structure(
                tr!("règle : valeur cherchée absente").into(),
            ));
        };
        let field = match value.get("field").map_or(Some(0), PyValue::as_i64) {
            Some(0) => RuleField::Payee,
            Some(1) => RuleField::Details,
            Some(2) => RuleField::PayeeOrDetails,
            _ => {
                return Err(LegacyError::Structure(
                    tr!("règle : champ testé invalide").into(),
                ));
            }
        };
        let text_if = |flag: &str, key: &str| value.flag(flag).then(|| value.text(key).to_string());
        let set_mode = if value.flag("do_mode") {
            let index = value.get("mode").and_then(PyValue::as_i64).unwrap_or(-1);
            usize::try_from(index)
                .ok()
                .and_then(|i| StandardMode::ALL.get(i).copied())
                .map(Some)
                .ok_or_else(|| LegacyError::Structure(tr!("règle : mode invalide").into()))?
        } else {
            None
        };
        let set_category = if value.flag("do_categ") {
            match value.get("categ").and_then(PyValue::as_i64) {
                Some(-1) | None => Some(CategoryChange::Clear),
                Some(id) => Some(CategoryChange::Set(u32::try_from(id).map_err(|_| {
                    LegacyError::Structure(tr!("règle : catégorie invalide ({id})", id = id))
                })?)),
            }
        } else {
            None
        };
        Ok(Self {
            field,
            contains: contains.to_string(),
            plus_is_or: value.flag("contains_or"),
            set_mode,
            set_payee: text_if("do_third", "third"),
            set_details: text_if("do_info", "info"),
            set_date: text_if("do_date", "date"),
            set_category,
            only_uncategorized: value.flag("emptycateg"),
            active: value.flag("active"),
            auto_apply: value.flag("withnewline"),
        })
    }

    /// Dict au format de l'ancien script (export `.txt`).
    pub fn to_legacy(&self) -> PyValue {
        let s = |k: &str| PyValue::Str(k.to_string());
        let text = |v: &Option<String>| PyValue::Str(v.clone().unwrap_or_default());
        let flag = |b: bool| PyValue::Bool(b);
        let mode = self.set_mode.map_or(0, |m| m.index() as i64);
        let categ = match self.set_category {
            Some(CategoryChange::Set(id)) => i64::from(id),
            _ => -1,
        };
        PyValue::Dict(vec![
            (s("field"), PyValue::Int(self.field.legacy_index())),
            (s("contains"), PyValue::Str(self.contains.clone())),
            (s("do_mode"), flag(self.set_mode.is_some())),
            (s("mode"), PyValue::Int(mode)),
            (s("do_third"), flag(self.set_payee.is_some())),
            (s("third"), text(&self.set_payee)),
            (s("do_info"), flag(self.set_details.is_some())),
            (s("info"), text(&self.set_details)),
            (s("do_categ"), flag(self.set_category.is_some())),
            (s("categ"), PyValue::Int(categ)),
            (s("active"), flag(self.active)),
            (s("withnewline"), flag(self.auto_apply)),
            (s("contains_or"), flag(self.plus_is_or)),
            (s("emptycateg"), flag(self.only_uncategorized)),
            (s("do_date"), flag(self.set_date.is_some())),
            (s("date"), text(&self.set_date)),
        ])
    }
}

impl CompiledRule<'_> {
    fn check_category(&self, categories: &[Category]) -> Result<(), RuleError> {
        match self.rule.set_category {
            Some(CategoryChange::Set(id)) if !categories.iter().any(|c| c.id == id) => {
                Err(RuleError::UnknownCategory(id))
            }
            _ => Ok(()),
        }
    }

    /// La condition est-elle vérifiée ? Une erreur d'évaluation rend la ligne
    /// non retenue (l'original se rabattait alors sur une recherche du texte
    /// de la formule).
    fn condition_holds(
        &self,
        op: &Operation,
        categories: &[Category],
    ) -> Result<bool, FormulaError> {
        match &self.condition {
            Condition::Formula(formula) => formula
                .eval(&|name| variable(op, categories, name, true))
                .map(|v| v.truthy()),
            Condition::Words { terms, any } => {
                let text = match self.rule.field {
                    RuleField::Payee => op.payee.to_lowercase(),
                    RuleField::Details => op.details.to_lowercase(),
                    RuleField::PayeeOrDetails => {
                        format!("{} {}", op.payee, op.details).to_lowercase()
                    }
                };
                let mut terms = terms.iter().filter(|t| !t.is_empty());
                Ok(if *any {
                    // `!` n'a pas de sens particulier en mode OU (comme l'original).
                    terms.any(|t| text.contains(t.as_str()))
                } else {
                    terms.all(|t| match t.strip_prefix('!') {
                        Some(absent) => !text.contains(absent),
                        None => text.contains(t.as_str()),
                    })
                })
            }
        }
    }

    fn accepts(
        &self,
        op: &Operation,
        categories: &[Category],
        line: usize,
        errors: &mut Vec<LineError>,
    ) -> bool {
        if self.rule.only_uncategorized && op.category.is_some() {
            return false;
        }
        match self.condition_holds(op, categories) {
            Ok(holds) => holds,
            Err(e) => {
                errors.push(LineError {
                    line,
                    part: RulePart::Condition,
                    message: e.to_string(),
                });
                false
            }
        }
    }

    /// Valeur d'une action : texte fixe, ou formule évaluée sur l'opération
    /// telle qu'elle était avant les actions de la règle.
    fn render(
        template: &Template,
        before: &Operation,
        categories: &[Category],
    ) -> Result<String, FormulaError> {
        match template {
            Template::Text(text) => Ok(text.clone()),
            Template::Formula(formula) => formula
                .eval(&|name| variable(before, categories, name, false))
                .map(|v| v.to_string()),
        }
    }

    fn apply(
        &self,
        categories: &[Category],
        account: &mut Account,
        lines: &[usize],
    ) -> RuleOutcome {
        let mut outcome = RuleOutcome::default();
        let mut seen = BTreeSet::new();
        for &line in lines {
            if !seen.insert(line) {
                continue;
            }
            let Some(op) = account.operations.get_mut(line) else {
                continue;
            };
            if !self.accepts(op, categories, line, &mut outcome.errors) {
                continue;
            }
            outcome.matched.push(line);
            let before = op.clone();
            let mut fail = |part: RulePart, message: String| {
                outcome.errors.push(LineError {
                    line,
                    part,
                    message,
                });
            };
            if let Some(mode) = self.rule.set_mode {
                // Le mode n'est imposé que si le signe du montant correspond.
                let amount = before.amount.0;
                if (mode.is_debit() && amount < 0) || (!mode.is_debit() && amount > 0) {
                    op.mode = Mode::Standard(mode);
                }
            }
            if let Some(template) = &self.payee {
                match Self::render(template, &before, categories) {
                    Ok(text) => op.payee = text.replace(';', " "),
                    Err(e) => fail(RulePart::Payee, e.to_string()),
                }
            }
            if let Some(template) = &self.details {
                match Self::render(template, &before, categories) {
                    Ok(text) => op.details = text.replace(';', " "),
                    Err(e) => fail(RulePart::Details, e.to_string()),
                }
            }
            if let Some(template) = &self.date {
                match Self::render(template, &before, categories) {
                    Ok(text) => match parse_rule_date(&text) {
                        Some(date) => op.date = date,
                        None => fail(
                            RulePart::Date,
                            tr!("« {text} » n'est pas une date JJ/MM/AAAA", text = text),
                        ),
                    },
                    Err(e) => fail(RulePart::Date, e.to_string()),
                }
            }
            match self.rule.set_category {
                Some(CategoryChange::Set(id)) => op.category = Some(id),
                Some(CategoryChange::Clear) => op.category = None,
                None => {}
            }
            if *op != before {
                outcome.changed.push(line);
            }
        }
        outcome
    }
}

/// Résultat de l'application de plusieurs règles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BatchOutcome {
    /// Opérations modifiées par au moins une règle (sans doublon, triées).
    pub changed: BTreeSet<usize>,
    /// Résultat de chaque règle appliquée : (indice de la règle, résultat).
    pub rules: Vec<(usize, RuleOutcome)>,
    /// Règles inutilisables : (indice de la règle, erreur).
    pub errors: Vec<(usize, RuleError)>,
}

/// Erreur d'import d'un fichier de règles.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    Empty,
    Line { line: usize, message: String },
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str(tr!("le fichier ne contient pas de règles à importer")),
            Self::Line { line, message } => f.write_str(&tr!(
                "ligne {line} : {message}",
                line = line,
                message = message
            )),
        }
    }
}

/// Liste ordonnée des règles du document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoFieldRules {
    pub rules: Vec<Rule>,
}

impl AutoFieldRules {
    /// Clé de rangement dans le nouveau format.
    pub const KEY: &'static str = "autofields.v6";
    /// Clé de l'ancien script (pickle d'une liste de dicts).
    pub const LEGACY_KEY: &'static str = "fabiobpautofields";

    fn apply_where(
        &self,
        categories: &[Category],
        account: &mut Account,
        lines: &[usize],
        keep: impl Fn(&Rule) -> bool,
    ) -> BatchOutcome {
        let mut batch = BatchOutcome::default();
        for (index, rule) in self.rules.iter().enumerate().filter(|(_, r)| keep(r)) {
            match rule.apply(categories, account, lines) {
                Ok(outcome) => {
                    batch.changed.extend(outcome.changed.iter().copied());
                    batch.rules.push((index, outcome));
                }
                Err(e) => batch.errors.push((index, e)),
            }
        }
        batch
    }

    /// « Appliquer actives » : toutes les règles actives, dans l'ordre, sur
    /// les opérations `lines` (les lignes visibles).
    pub fn apply_active(
        &self,
        categories: &[Category],
        account: &mut Account,
        lines: &[usize],
    ) -> BatchOutcome {
        self.apply_where(categories, account, lines, |r| r.active)
    }

    /// Mode automatique : règles actives et marquées « appliquer
    /// automatiquement », sur les opérations ajoutées ou modifiées.
    pub fn apply_to_new_lines(
        &self,
        categories: &[Category],
        account: &mut Account,
        lines: &[usize],
    ) -> BatchOutcome {
        self.apply_where(categories, account, lines, |r| r.active && r.auto_apply)
    }

    /// Mode automatique sur un document : `changes` = (compte, opération).
    /// Renvoie le résultat par compte touché.
    pub fn apply_to_new_lines_in_document(
        &self,
        document: &mut Document,
        changes: &[(usize, usize)],
    ) -> Vec<(usize, BatchOutcome)> {
        let accounts: BTreeSet<usize> = changes.iter().map(|&(a, _)| a).collect();
        let mut results = Vec::new();
        for a in accounts {
            let lines: Vec<usize> = changes
                .iter()
                .filter(|&&(acc, _)| acc == a)
                .map(|&(_, l)| l)
                .collect();
            let Document {
                categories,
                accounts,
                ..
            } = document;
            if let Some(account) = accounts.get_mut(a) {
                results.push((a, self.apply_to_new_lines(categories, account, &lines)));
            }
        }
        results
    }

    /// Conversion depuis l'ancien pickle (liste de dicts), aussi utilisée pour
    /// les fichiers `.bprules` et les anciens `Scripts\Auto Fields\*.dat`.
    pub fn from_legacy(value: &PyValue) -> Result<Self, LegacyError> {
        let items = value
            .as_list()
            .ok_or_else(|| LegacyError::Structure(tr!("règles : liste attendue").into()))?;
        Ok(Self {
            rules: items
                .iter()
                .map(Rule::from_legacy)
                .collect::<Result<_, _>>()?,
        })
    }

    /// Lit les règles du document (nouvelle clé, sinon ancien pickle décodé
    /// par `parse_pickle`, sinon aucune règle).
    pub fn load(
        document: &Document,
        parse_pickle: &dyn Fn(&[u8]) -> Option<PyValue>,
    ) -> Result<Self, FeatureError> {
        feature_store::load_with_legacy(
            document,
            Self::KEY,
            Self::LEGACY_KEY,
            parse_pickle,
            Self::from_legacy,
        )
        .unwrap_or_else(|| Ok(Self::default()))
    }

    /// Range les règles dans le document (et retire l'ancienne clé).
    pub fn save(&self, document: &mut Document) -> Result<(), FeatureError> {
        feature_store::set(document, Self::KEY, self)?;
        feature_store::remove(document, Self::LEGACY_KEY);
        Ok(())
    }

    /// Export au format texte de l'original : une règle par ligne, `repr()`
    /// Python d'un dict, texte ASCII (accents en octets CP1252 échappés).
    pub fn export_text(&self) -> String {
        self.rules
            .iter()
            .map(|r| to_repr(&r.to_legacy()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Import d'un fichier texte exporté (par l'original ou par
    /// [`AutoFieldRules::export_text`]). Les lignes sont relues comme des
    /// littéraux Python, **sans `eval`** ; une seule ligne invalide fait
    /// échouer l'import (aucune règle ajoutée), comme l'original.
    pub fn import_text(text: &str) -> Result<Vec<Rule>, ImportError> {
        let mut rules = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() {
                continue;
            }
            let error = |message: String| ImportError::Line {
                line: n + 1,
                message,
            };
            let value = parse_repr(line).map_err(|e| error(e.to_string()))?;
            rules.push(Rule::from_legacy(&value).map_err(|e| error(e.to_string()))?);
        }
        if rules.is_empty() {
            Err(ImportError::Empty)
        } else {
            Ok(rules)
        }
    }

    /// Comme [`AutoFieldRules::import_text`], depuis les octets du fichier
    /// (UTF-8, sinon CP1252).
    pub fn import_bytes(bytes: &[u8]) -> Result<Vec<Rule>, ImportError> {
        match std::str::from_utf8(bytes) {
            Ok(text) => Self::import_text(text),
            Err(_) => {
                let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
                Self::import_text(&text)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Tack;

    fn categories() -> Vec<Category> {
        let cat = |id, name: &str, is_child| Category {
            id,
            name: name.into(),
            is_child,
        };
        vec![
            cat(1, "Logement", false),
            cat(5, "     Energie", true),
            cat(18, "     Électricité", true),
            cat(9, "Impôts", false),
        ]
    }

    fn op(mode: Mode, payee: &str, details: &str, cents: i64, category: Option<u32>) -> Operation {
        let date = Date::from_ymd(2010, 10, 5).expect("date valide");
        Operation {
            uid: 1,
            date,
            mark_date: date,
            tack: Tack::None,
            mode,
            payee: payee.into(),
            details: details.into(),
            category,
            amount: crate::model::Amount::from_cents(cents),
            extra: Vec::new(),
        }
    }

    fn std(mode: StandardMode) -> Mode {
        Mode::Standard(mode)
    }

    fn account(operations: Vec<Operation>) -> Account {
        Account {
            uid: "a".into(),
            name: "Courant".into(),
            bank_name: String::new(),
            agency_name: String::new(),
            agency_address: String::new(),
            number: String::new(),
            bank_code: String::new(),
            agency_code: String::new(),
            bic: String::new(),
            iban: String::new(),
            logo: String::new(),
            currency: "€".into(),
            currency_rate: 1.0,
            secondary_currency: String::new(),
            secondary_rate: 1.0,
            is_virtual: false,
            disabled: false,
            operations,
        }
    }

    /// Les cinq règles réelles d'Exemple.bp (`fabiobpautofields`), telles que
    /// le pickle les décode.
    const EXEMPLE_RULES: &str = r#"{'info': '', 'categ': 9, 'do_mode': False, 'third': '', 'do_third': False, 'contains': '="tresor public" in tiers and montant < -70 and montant > -100', 'emptycateg': True, 'field': 0, 'mode': 0, 'do_categ': True, 'do_info': False, 'active': True, 'contains_or': False, 'withnewline': True}
{'info': '', 'categ': 5, 'do_mode': False, 'third': '', 'do_third': False, 'contains': '="gaz de france" in tiers or "electricite de france" in tiers', 'emptycateg': True, 'field': 0, 'mode': 0, 'do_categ': True, 'do_info': False, 'active': True, 'contains_or': False, 'withnewline': True}
{'info': '', 'categ': -1, 'do_mode': False, 'third': "=tiers.replace(mode[4:], '').strip()", 'do_third': True, 'contains': '="chq " in mode and mode[4:] in tiers', 'emptycateg': False, 'field': 0, 'mode': 0, 'do_categ': False, 'do_info': False, 'active': True, 'contains_or': False, 'withnewline': True}
{'info': 'Pr\xe9l\xe8vement \xe9lectricit\xe9', 'categ': 18, 'do_mode': False, 'third': '', 'do_third': False, 'contains': 'EDF', 'emptycateg': True, 'field': 0, 'mode': 0, 'do_categ': True, 'do_info': True, 'active': True, 'contains_or': False, 'withnewline': True}
{'info': 'SANS CATEGORIE', 'categ': -1, 'do_mode': False, 'do_date': True, 'third': '', 'do_third': False, 'contains': '=categ==""', 'emptycateg': False, 'field': 0, 'mode': 0, 'do_categ': False, 'do_info': True, 'active': True, 'date': '13/10/2010', 'contains_or': False, 'withnewline': False}"#;

    fn exemple_rules() -> AutoFieldRules {
        AutoFieldRules {
            rules: AutoFieldRules::import_text(EXEMPLE_RULES).expect("règles d'Exemple.bp valides"),
        }
    }

    #[test]
    fn exemple_rules_decoded() {
        let rules = exemple_rules().rules;
        assert_eq!(rules.len(), 5);
        assert!(rules[0].is_formula());
        assert_eq!(rules[0].set_category, Some(CategoryChange::Set(9)));
        assert!(rules[0].only_uncategorized && rules[0].active && rules[0].auto_apply);
        assert_eq!(
            rules[2].set_payee.as_deref(),
            Some("=tiers.replace(mode[4:], '').strip()")
        );
        assert_eq!(rules[2].set_category, None);
        assert_eq!(
            rules[3].set_details.as_deref(),
            Some("Prélèvement électricité")
        );
        assert_eq!(rules[4].set_date.as_deref(), Some("13/10/2010"));
        assert!(!rules[4].auto_apply);
        for rule in &rules {
            rule.validate(&categories()).expect("règle valide");
        }
        let (condition, actions) = rules[3].summary(&categories());
        assert_eq!(condition, "Tiers: EDF");
        assert_eq!(
            actions,
            "Détails: Prélèvement électricité, Categ: Logement > Électricité"
        );
        assert_eq!(rules[0].summary(&categories()).1, "Categ: Impôts");
    }

    #[test]
    fn exemple_formula_conditions() {
        let rules = exemple_rules().rules;
        let cats = categories();
        let mut acc = account(vec![
            op(
                std(StandardMode::Prelevement),
                "TRESOR PUBLIC 75",
                "",
                -8_000,
                None,
            ),
            op(
                std(StandardMode::Prelevement),
                "Tresor public",
                "",
                -12_000,
                None,
            ),
            op(
                std(StandardMode::Prelevement),
                "Tresor public",
                "",
                -8_000,
                Some(1),
            ),
            op(
                std(StandardMode::Prelevement),
                "Gaz de France",
                "",
                -5_000,
                None,
            ),
            op(
                std(StandardMode::Prelevement),
                "ELECTRICITE DE FRANCE",
                "",
                -5_000,
                None,
            ),
        ]);
        let all: Vec<usize> = (0..acc.operations.len()).collect();
        let outcome = rules[0].apply(&cats, &mut acc, &all).expect("règle valide");
        assert_eq!(
            outcome.matched,
            [0],
            "montant entre -100 et -70, sans catégorie"
        );
        assert_eq!(acc.operations[0].category, Some(9));
        assert_eq!(acc.operations[1].category, None, "-120 hors bornes");
        assert_eq!(acc.operations[2].category, Some(1), "déjà catégorisée");
        let outcome = rules[1].apply(&cats, &mut acc, &all).expect("règle valide");
        assert_eq!(outcome.matched, [3, 4]);
        assert_eq!(outcome.changed, [3, 4]);
        assert!(outcome.errors.is_empty());
    }

    #[test]
    fn exemple_cheque_number_removed_from_payee() {
        let rules = exemple_rules().rules;
        let cats = categories();
        let mut acc = account(vec![
            op(
                std(StandardMode::ChequeEmis),
                "Garage 1234567",
                "n°1234567",
                -20_000,
                None,
            ),
            op(
                std(StandardMode::ChequeEmis),
                "Garage",
                "n°1234567",
                -20_000,
                None,
            ),
            op(
                std(StandardMode::Carte),
                "Garage 1234567",
                "",
                -20_000,
                None,
            ),
        ]);
        let outcome = rules[2]
            .apply(&cats, &mut acc, &[0, 1, 2])
            .expect("règle valide");
        assert_eq!(outcome.matched, [0]);
        assert_eq!(
            acc.operations[0].payee, "Garage",
            "numéro retiré, espaces supprimés"
        );
        assert_eq!(acc.operations[2].payee, "Garage 1234567");
    }

    #[test]
    fn exemple_words_and_details() {
        let rules = exemple_rules().rules;
        let cats = categories();
        let mut acc = account(vec![
            op(
                std(StandardMode::Prelevement),
                "Prlv EDF Clients",
                "",
                -6_000,
                None,
            ),
            op(
                std(StandardMode::Prelevement),
                "edf",
                "ancien",
                -6_000,
                Some(9),
            ),
            op(std(StandardMode::Prelevement), "GDF", "", -6_000, None),
        ]);
        let outcome = rules[3]
            .apply(&cats, &mut acc, &[0, 1, 2])
            .expect("règle valide");
        assert_eq!(outcome.matched, [0]);
        assert_eq!(acc.operations[0].details, "Prélèvement électricité");
        assert_eq!(acc.operations[0].category, Some(18));
        assert_eq!(
            acc.operations[1].details, "ancien",
            "« uniquement sans catégorie » : aucune action sur une ligne catégorisée"
        );
        assert_eq!(acc.operations[1].category, Some(9));
    }

    #[test]
    fn exemple_uncategorized_gets_marker_and_date() {
        let rules = exemple_rules().rules;
        let cats = categories();
        let mut acc = account(vec![
            op(std(StandardMode::Carte), "X", "", -100, None),
            op(std(StandardMode::Carte), "Y", "", -100, Some(5)),
        ]);
        let outcome = rules[4]
            .apply(&cats, &mut acc, &[0, 1])
            .expect("règle valide");
        assert_eq!(outcome.matched, [0]);
        assert_eq!(acc.operations[0].details, "SANS CATEGORIE");
        assert_eq!(
            acc.operations[0].date,
            Date::from_ymd(2010, 10, 13).expect("valide")
        );
        assert_eq!(acc.operations[1].details, "");
    }

    #[test]
    fn automatic_mode_on_new_lines() {
        let rules = exemple_rules();
        let mut doc = crate::feature_store::tests::empty_document();
        doc.categories = categories();
        doc.accounts = vec![
            account(vec![
                op(std(StandardMode::Prelevement), "EDF", "", -6_000, None),
                op(std(StandardMode::Prelevement), "EDF", "", -6_000, None),
            ]),
            account(vec![op(
                std(StandardMode::Carte),
                "Boulangerie",
                "",
                -300,
                None,
            )]),
        ];
        let results =
            rules.apply_to_new_lines_in_document(&mut doc, &[(0, 1), (1, 0), (7, 0), (0, 99)]);
        assert_eq!(results.len(), 2);
        assert_eq!(
            doc.accounts[0].operations[0].category, None,
            "ligne non signalée intacte"
        );
        assert_eq!(doc.accounts[0].operations[1].category, Some(18));
        assert_eq!(
            doc.accounts[1].operations[0].details, "",
            "la règle 5 n'est pas automatique"
        );
        let changed: Vec<usize> = results[0].1.changed.iter().copied().collect();
        assert_eq!(changed, [1]);

        // « Appliquer actives » applique aussi la règle 5.
        let lines = [0];
        let batch = rules.apply_active(&doc.categories, &mut doc.accounts[1], &lines);
        assert_eq!(doc.accounts[1].operations[0].details, "SANS CATEGORIE");
        assert_eq!(batch.changed.len(), 1);
        assert!(batch.errors.is_empty());
    }

    #[test]
    fn words_matching() {
        let cats = categories();
        let check = |contains: &str, field: RuleField, or: bool, payee: &str, details: &str| {
            let rule = Rule {
                field,
                plus_is_or: or,
                ..Rule::new(contains)
            };
            let acc = account(vec![op(
                std(StandardMode::Carte),
                payee,
                details,
                -100,
                None,
            )]);
            !rule
                .find_matches(&cats, &acc, &[0])
                .expect("valide")
                .matched
                .is_empty()
        };
        use RuleField::*;
        assert!(
            check("cinéma", Payee, false, "CINÉMA Rex", ""),
            "casse ignorée, accents compris"
        );
        assert!(
            !check("cinema", Payee, false, "Cinéma", ""),
            "pas de pliage des accents (comme l'original)"
        );
        assert!(check("salaire+!camille", Payee, false, "Salaire Alex", ""));
        assert!(!check(
            "salaire+!camille",
            Payee,
            false,
            "Salaire Camille",
            ""
        ));
        assert!(!check("picard+monoprix", Payee, false, "Picard", ""));
        assert!(check("picard+monoprix", Payee, true, "Picard", ""));
        assert!(!check("picard+monoprix", Payee, true, "Carrefour", ""));
        assert!(check("loyer", Details, false, "SCI", "Loyer mars"));
        assert!(!check("loyer", Payee, false, "SCI", "Loyer mars"));
        assert!(check(
            "sci+loyer",
            PayeeOrDetails,
            false,
            "SCI",
            "Loyer mars"
        ));
        assert!(
            check("+", Payee, false, "x", ""),
            "aucun mot : tout correspond (AND)"
        );
        assert!(
            !check("+", Payee, true, "x", ""),
            "aucun mot : rien ne correspond (OU)"
        );
    }

    #[test]
    fn actions_formulas_and_mode_sign() {
        let cats = categories();
        let rule = Rule {
            set_mode: Some(StandardMode::Prelevement),
            set_payee: Some("=tiers.title()".into()),
            set_details: Some(
                "=details[:3] + ' ; ' + str(montant) + ' ' + categ + ' ' + date".into(),
            ),
            ..Rule::new("=montant != 0")
        };
        let mut acc = account(vec![
            op(
                std(StandardMode::Carte),
                "SUPER U",
                "abcdef",
                -1_250,
                Some(9),
            ),
            op(
                std(StandardMode::VirementRecu),
                "employeur",
                "paie",
                100_000,
                None,
            ),
        ]);
        let outcome = rule.apply(&cats, &mut acc, &[0, 1, 0]).expect("valide");
        assert_eq!(outcome.matched, [0, 1], "doublons ignorés");
        let ops = &acc.operations;
        assert_eq!(ops[0].mode, std(StandardMode::Prelevement));
        assert_eq!(
            ops[1].mode,
            std(StandardMode::VirementRecu),
            "recette : mode de dépense non imposé"
        );
        assert_eq!(ops[0].payee, "Super U");
        assert_eq!(
            ops[0].details, "abc   -12.5 Impôts 05-10-2010",
            "« ; » remplacé par une espace"
        );
        assert_eq!(ops[1].details, "pai   1000.0  05-10-2010");
    }

    #[test]
    fn evaluation_errors_do_not_modify() {
        let cats = categories();
        let mut acc = account(vec![op(
            std(StandardMode::Carte),
            "Tiers",
            "Détail",
            -100,
            None,
        )]);
        // Condition : comparaison impossible → ligne non retenue, erreur signalée.
        let rule = Rule {
            set_details: Some("modifié".into()),
            ..Rule::new("=tiers < 3")
        };
        let outcome = rule.apply(&cats, &mut acc, &[0]).expect("syntaxe valide");
        assert!(outcome.matched.is_empty());
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(outcome.errors[0].part, RulePart::Condition);
        assert_eq!(acc.operations[0].details, "Détail");

        // Action : erreur → champ inchangé (l'original écrivait le texte de la
        // formule), les autres actions s'appliquent.
        let rule = Rule {
            set_payee: Some("=tiers[100]".into()),
            set_details: Some("ok".into()),
            set_date: Some("=date".into()),
            ..Rule::new("tiers")
        };
        let outcome = rule.apply(&cats, &mut acc, &[0]).expect("syntaxe valide");
        assert_eq!(outcome.changed, [0]);
        assert_eq!(acc.operations[0].payee, "Tiers");
        assert_eq!(acc.operations[0].details, "ok");
        assert_eq!(outcome.errors.len(), 1, "la date jj-mm-aaaa est acceptée");
        assert_eq!(outcome.errors[0].part, RulePart::Payee);

        let rule = Rule {
            set_date: Some("31/02/2010".into()),
            ..Rule::new("tiers")
        };
        let outcome = rule.apply(&cats, &mut acc, &[0]).expect("syntaxe valide");
        assert_eq!(outcome.errors[0].part, RulePart::Date);
        assert!(outcome.changed.is_empty());
    }

    #[test]
    fn malformed_rules_rejected() {
        let cats = categories();
        let mut acc = account(vec![op(std(StandardMode::Carte), "x", "", -100, None)]);
        type Expectation = fn(&RuleError) -> bool;
        let cases: Vec<(Rule, Expectation)> = vec![
            (Rule::new(""), |e| matches!(e, RuleError::EmptyCondition)),
            (Rule::new("="), |e| {
                matches!(
                    e,
                    RuleError::Formula {
                        part: RulePart::Condition,
                        ..
                    }
                )
            }),
            (Rule::new("=tiers ==="), |e| {
                matches!(e, RuleError::Formula { .. })
            }),
            (Rule::new("=__import__('os').system('rm')"), |e| {
                matches!(
                    e,
                    RuleError::Formula {
                        part: RulePart::Condition,
                        ..
                    }
                )
            }),
            (Rule::new("=open('/etc/passwd')"), |e| {
                matches!(e, RuleError::Formula { .. })
            }),
            (Rule::new("=tiers.__class__()"), |e| {
                matches!(e, RuleError::Formula { .. })
            }),
            (
                Rule::new("=os"),
                |e| matches!(e, RuleError::UnknownVariable { name, .. } if name == "os"),
            ),
            (
                Rule::new("=solde > 0"),
                |e| matches!(e, RuleError::UnknownVariable { name, .. } if name == "solde"),
            ),
            (
                Rule {
                    set_payee: Some("=tiers +".into()),
                    ..Rule::new("x")
                },
                |e| {
                    matches!(
                        e,
                        RuleError::Formula {
                            part: RulePart::Payee,
                            ..
                        }
                    )
                },
            ),
            (
                Rule {
                    set_date: Some("=exec('x')".into()),
                    ..Rule::new("x")
                },
                |e| {
                    matches!(
                        e,
                        RuleError::Formula {
                            part: RulePart::Date,
                            ..
                        }
                    )
                },
            ),
            (
                Rule {
                    set_category: Some(CategoryChange::Set(404)),
                    ..Rule::new("x")
                },
                |e| matches!(e, RuleError::UnknownCategory(404)),
            ),
        ];
        for (rule, expected) in cases {
            let before = acc.clone();
            let result = rule.apply(&cats, &mut acc, &[0]);
            match result {
                Err(e) => assert!(
                    expected(&e),
                    "{:?} : erreur inattendue {e:?}",
                    rule.contains
                ),
                Ok(_) => panic!("{:?} aurait dû être refusée", rule.contains),
            }
            assert_eq!(acc, before, "aucune modification");
        }
        let rules = AutoFieldRules {
            rules: vec![
                Rule::new("="),
                Rule {
                    active: true,
                    ..Rule::new("=")
                },
            ],
        };
        let batch = rules.apply_active(&cats, &mut acc, &[0]);
        assert_eq!(
            batch.errors.len(),
            1,
            "seules les règles actives sont tentées"
        );
        assert_eq!(batch.errors[0].0, 1);
    }

    #[test]
    fn category_clear_and_out_of_range_lines() {
        let cats = categories();
        let mut acc = account(vec![op(std(StandardMode::Carte), "x", "", -100, Some(5))]);
        let rule = Rule {
            set_category: Some(CategoryChange::Clear),
            ..Rule::new("x")
        };
        let outcome = rule.apply(&cats, &mut acc, &[0, 3]).expect("valide");
        assert_eq!(outcome.changed, [0]);
        assert_eq!(acc.operations[0].category, None);
    }

    #[test]
    fn export_import_round_trip() {
        let rules = exemple_rules();
        let text = rules.export_text();
        assert!(text.is_ascii());
        assert_eq!(text.lines().count(), 5);
        assert!(
            text.contains(r"'Pr\xe9l\xe8vement \xe9lectricit\xe9'"),
            "{text}"
        );
        assert_eq!(AutoFieldRules::import_text(&text), Ok(rules.rules.clone()));
        let mut cp1252 = b"{'contains': 'caf".to_vec();
        cp1252.push(0xe9);
        cp1252.extend_from_slice(b"', 'field': 1}\r\n\r\n");
        let imported = AutoFieldRules::import_bytes(&cp1252).expect("valide");
        assert_eq!(imported[0].contains, "café");
        assert_eq!(imported[0].field, RuleField::Details);
        let utf8 = "\u{feff}{'contains': 'café'}".as_bytes();
        assert_eq!(
            AutoFieldRules::import_bytes(utf8).map(|r| r[0].contains.clone()),
            Ok("café".into())
        );
    }

    #[test]
    fn import_rejects_bad_files() {
        assert_eq!(AutoFieldRules::import_text(""), Err(ImportError::Empty));
        assert_eq!(
            AutoFieldRules::import_text("\n  \n"),
            Err(ImportError::Empty)
        );
        for bad in [
            "__import__('os').system('rm -rf ~')",
            "{'contains': 'a'}\n{'contains': 1}",
            "{'field': 0}",
            "{'contains': 'a', 'field': 7}",
            "{'contains': 'a', 'do_mode': 1, 'mode': 12}",
            "{'contains': 'a', 'do_categ': 1, 'categ': -5}",
            "[1, 2]",
            "{'contains': 'a'",
        ] {
            assert!(
                matches!(
                    AutoFieldRules::import_text(bad),
                    Err(ImportError::Line { .. })
                ),
                "{bad:?}"
            );
        }
        assert!(matches!(
            AutoFieldRules::import_text("{'contains': 'a'}\n{'contains': 1}"),
            Err(ImportError::Line { line: 2, .. })
        ));
    }

    #[test]
    fn legacy_pickle_structure_and_persistence() {
        let value = parse_repr(&format!(
            "[{}]",
            EXEMPLE_RULES.lines().collect::<Vec<_>>().join(", ")
        ))
        .expect("valide");
        let rules = AutoFieldRules::from_legacy(&value).expect("structure valide");
        assert_eq!(rules, exemple_rules());
        assert!(AutoFieldRules::from_legacy(&PyValue::Dict(Vec::new())).is_err());
        // Anciennes versions : cases à cocher en entiers, clés facultatives absentes.
        let old = parse_repr("[{'field': 2, 'contains': 'x', 'do_mode': 1, 'mode': 6, 'do_third': 0, 'third': '', 'do_info': 0, 'info': '', 'do_categ': 0, 'categ': -1, 'active': 1}]")
            .expect("valide");
        let old = AutoFieldRules::from_legacy(&old).expect("valide");
        assert_eq!(old.rules[0].set_mode, Some(StandardMode::Versement));
        assert_eq!(old.rules[0].field, RuleField::PayeeOrDetails);
        assert!(
            !old.rules[0].auto_apply && !old.rules[0].plus_is_or && old.rules[0].set_date.is_none()
        );

        let mut doc = crate::feature_store::tests::empty_document();
        let parse = |_: &[u8]| Some(value.clone());
        assert_eq!(
            AutoFieldRules::load(&doc, &parse).ok(),
            Some(AutoFieldRules::default())
        );
        doc.feature_data.push(crate::model::FeatureData {
            key: AutoFieldRules::LEGACY_KEY.into(),
            value: b"(lp1".to_vec(),
        });
        assert_eq!(AutoFieldRules::load(&doc, &parse).ok(), Some(rules.clone()));
        rules.save(&mut doc).expect("enregistrable");
        let none = |_: &[u8]| None;
        assert_eq!(AutoFieldRules::load(&doc, &none).ok(), Some(rules));
    }
}
