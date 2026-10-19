//! Filtres prédéfinis (ex-script `Filters/setfilters.py`).
//!
//! Un filtre prédéfini est un jeu nommé de critères de la grille (dates, mode,
//! tiers, catégorie, montant, pointage). Ses dates peuvent être **dynamiques** :
//! `jour/mois/année` où chaque partie est une petite expression arithmétique
//! utilisant les constantes `j` (jour courant), `m` (mois courant), `a` (année
//! courante) et `dj` (dernier jour du mois obtenu), par exemple `1/m/a`,
//! `dj/m-1/a` ou `j-50/m/a`. Les dépassements se reportent sur le mois ou
//! l'année voisins (`0/m/a` = dernier jour du mois précédent, `1/13/a` = 1er
//! janvier suivant), exactement comme dans `fdate()`.
//!
//! Les filtres servent aussi de périodes de référence au budget mensuel
//! ([`crate::budget`]).

use std::cmp::Ordering;

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::feature_store::{self, FeatureError};
use crate::filter::{Filters, TackFilter, normalize};
use crate::formula::{Formula, Value};
use crate::model::{Date, Document};
use crate::pyvalue::{LegacyError, PyValue};
use crate::tr;

/// Erreur de saisie d'un filtre prédéfini.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AutoFilterError {
    EmptyName,
    InvalidDate { expr: String, message: String },
}

impl std::fmt::Display for AutoFilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => f.write_str(tr!("le nom du filtre est vide")),
            Self::InvalidDate { expr, message } => f.write_str(&tr!(
                "date « {expr} » invalide : {message}",
                expr = expr,
                message = message
            )),
        }
    }
}

/// Nombre de jours du mois `month` (1 à 12) de l'année `year`.
pub fn last_day(month: u32, year: i32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn date_error(expr: &str, message: impl Into<String>) -> AutoFilterError {
    AutoFilterError::InvalidDate {
        expr: expr.to_string(),
        message: message.into(),
    }
}

/// Évalue une partie de date : nombre entier, ou expression utilisant les
/// constantes autorisées (`vars`).
fn eval_part(expr: &str, part: &str, vars: &[(&str, i64)]) -> Result<i64, AutoFilterError> {
    let part = part.trim();
    if !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()) {
        return part
            .parse()
            .map_err(|_| date_error(expr, tr!("nombre trop grand")));
    }
    let formula = Formula::parse(part).map_err(|e| date_error(expr, e.to_string()))?;
    let value = formula
        .eval(&|name| {
            vars.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| Value::Int(*v))
        })
        .map_err(|e| date_error(expr, e.to_string()))?;
    match value {
        Value::Int(i) => Ok(i),
        _ => Err(date_error(expr, tr!("un nombre entier est attendu"))),
    }
}

/// Décalage maximal accepté sur le jour (environ mille ans), pour borner les
/// reports de mois en mois.
const MAX_DAY_OFFSET: i64 = 400_000;

/// Calcule une date dynamique (`1/m-1/a`, `dj/m/a`, `j-50/m/a`, `15/03/2024`…)
/// par rapport à `today`. Une expression vide donne `Ok(None)` (pas de borne).
pub fn eval_dynamic_date(expr: &str, today: Date) -> Result<Option<Date>, AutoFilterError> {
    if expr.trim().is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = expr.split('/').collect();
    let [day_part, month_part, year_part] = parts[..] else {
        return Err(date_error(expr, tr!("format jour/mois/année attendu")));
    };
    let now = today.to_naive();
    let year = eval_part(expr, year_part, &[("a", i64::from(now.year()))])?;
    let month = eval_part(expr, month_part, &[("m", i64::from(now.month()))])?;
    // Report des mois hors de 1..12 sur l'année.
    let mut year = year
        .checked_add((month - 1).div_euclid(12))
        .ok_or_else(|| date_error(expr, tr!("année hors limites")))?;
    let mut month = (month - 1).rem_euclid(12) + 1;
    if !(1..=9999).contains(&year) {
        return Err(date_error(expr, tr!("année hors limites")));
    }
    let mut last = i64::from(last_day(month as u32, year as i32));
    let mut day = eval_part(expr, day_part, &[("dj", last), ("j", i64::from(now.day()))])?;
    if day.abs() > MAX_DAY_OFFSET {
        return Err(date_error(expr, tr!("décalage de jours trop grand")));
    }
    while day < 1 {
        month -= 1;
        if month == 0 {
            year -= 1;
            month = 12;
        }
        last = i64::from(last_day(month as u32, year as i32));
        day += last;
    }
    while day > last {
        month += 1;
        if month == 13 {
            year += 1;
            month = 1;
        }
        day -= last;
        last = i64::from(last_day(month as u32, year as i32));
    }
    if !(1..=9999).contains(&year) {
        return Err(date_error(expr, tr!("année hors limites")));
    }
    Date::from_ymd(year as i32, month as u32, day as u32)
        .map(Some)
        .ok_or_else(|| date_error(expr, tr!("date impossible")))
}

/// Code de pointage des anciens scripts (`mrk`) : 4 = toutes, 3 = pointées et
/// rapprochées, 2 = rapprochées, 1 = pointées, 0 = non pointées.
pub fn tack_from_legacy(code: i64) -> TackFilter {
    match code {
        0 => TackFilter::NotPointed,
        1 => TackFilter::Pointed,
        2 => TackFilter::Reconciled,
        3 => TackFilter::PointedOrReconciled,
        _ => TackFilter::All,
    }
}

/// Inverse de [`tack_from_legacy`].
pub fn tack_to_legacy(tack: TackFilter) -> i64 {
    match tack {
        TackFilter::NotPointed => 0,
        TackFilter::Pointed => 1,
        TackFilter::Reconciled => 2,
        TackFilter::PointedOrReconciled => 3,
        TackFilter::All => 4,
    }
}

/// Filtre prédéfini nommé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoFilter {
    pub name: String,
    /// Date de début, éventuellement dynamique (vide : pas de borne).
    #[serde(default)]
    pub date_from: String,
    /// Date de fin, éventuellement dynamique (vide : pas de borne).
    #[serde(default)]
    pub date_to: String,
    /// Filtre texte du mode (syntaxe de [`crate::filter`]).
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub payee: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub amount: String,
    #[serde(default)]
    pub tack: TackFilter,
}

impl AutoFilter {
    /// Filtre sans critère.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            date_from: String::new(),
            date_to: String::new(),
            mode: String::new(),
            payee: String::new(),
            category: String::new(),
            amount: String::new(),
            tack: TackFilter::All,
        }
    }

    /// Bornes de dates calculées pour `today`. Comme l'original, une date
    /// invalide est ignorée (pas de borne) ; [`AutoFilter::validate`] permet de
    /// la refuser à la saisie.
    pub fn date_range(&self, today: Date) -> (Option<Date>, Option<Date>) {
        (
            eval_dynamic_date(&self.date_from, today).ok().flatten(),
            eval_dynamic_date(&self.date_to, today).ok().flatten(),
        )
    }

    /// Filtres de la grille correspondant à ce filtre prédéfini.
    pub fn to_filters(&self, today: Date) -> Filters {
        let (date_from, date_to) = self.date_range(today);
        Filters {
            mode: self.mode.clone(),
            payee: self.payee.clone(),
            category: self.category.clone(),
            amount: self.amount.clone(),
            tack: self.tack,
            date_from,
            date_to,
            hide_old_lines: false,
        }
    }

    /// Vérifie le nom et les expressions de dates.
    pub fn validate(&self) -> Result<(), AutoFilterError> {
        if clean_name(&self.name).is_empty() {
            return Err(AutoFilterError::EmptyName);
        }
        // N'importe quelle date de référence convient pour vérifier la syntaxe.
        let reference = Date::from_ymd(2000, 1, 31).unwrap_or(Date(36556));
        eval_dynamic_date(&self.date_from, reference)?;
        eval_dynamic_date(&self.date_to, reference)?;
        Ok(())
    }

    /// Phrase décrivant les opérations retenues (libellé du budget mensuel).
    pub fn description(&self, today: Date) -> String {
        let (from, to) = self.date_range(today);
        let tack = match self.tack {
            TackFilter::All => String::new(),
            TackFilter::PointedOrReconciled => tr!("pointées et rapprochées").into(),
            TackFilter::Reconciled => tr!("rapprochées").into(),
            TackFilter::Pointed => tr!("pointées").into(),
            TackFilter::NotPointed => tr!("non pointées").into(),
        };
        let dates = match (from, to) {
            (Some(a), Some(b)) => tr!("du {a} au {b}", a = a, b = b),
            (Some(a), None) => tr!("à partir du {a}", a = a),
            (None, Some(b)) => tr!("jusqu'au {b}", b = b),
            (None, None) => String::new(),
        };
        let quoted = |text: &str, template: &str| {
            if text.is_empty() {
                String::new()
            } else {
                crate::i18n::format(crate::i18n::tr(template), &[&text], &[])
            }
        };
        let parts = [
            tack,
            dates,
            quoted(
                &self.mode,
                crate::tr_mark!("ayant « {} » comme mode de paiement"),
            ),
            quoted(
                &self.payee,
                crate::tr_mark!("ayant « {} » dans le champ tiers"),
            ),
            quoted(
                &self.category,
                crate::tr_mark!("ventilées dans la catégorie « {} »"),
            ),
            quoted(&self.amount, crate::tr_mark!("ayant pour montant « {} »")),
        ];
        let details: Vec<&str> = parts
            .iter()
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .collect();
        if details.is_empty() {
            tr!("Opérations concernées : toutes").into()
        } else {
            tr!(
                "Opérations concernées : toutes les lignes {}",
                details.join(", ")
            )
        }
    }

    /// Conversion depuis un élément de l'ancien dictionnaire
    /// `{"dt1", "dt2", "mde", "who", "ctg", "val", "mrk"}`.
    pub fn from_legacy(name: &str, value: &PyValue) -> Result<Self, LegacyError> {
        if value.as_dict().is_none() {
            return Err(LegacyError::Structure(tr!(
                "filtre « {name} » : dictionnaire attendu",
                name = name
            )));
        }
        Ok(Self {
            name: name.to_string(),
            date_from: value.text("dt1").to_string(),
            date_to: value.text("dt2").to_string(),
            mode: value.text("mde").to_string(),
            payee: value.text("who").to_string(),
            category: value.text("ctg").to_string(),
            amount: value.text("val").to_string(),
            tack: tack_from_legacy(value.get("mrk").and_then(PyValue::as_i64).unwrap_or(4)),
        })
    }
}

/// Nom nettoyé comme à la saisie de l'original (`;` et `&` retirés, espaces
/// de bord supprimés) : `;` sépare les entrées du menu déroulant et `&` y
/// marquerait un raccourci.
pub fn clean_name(name: &str) -> String {
    name.replace([';', '&'], "").trim().to_string()
}

fn compare_names(a: &str, b: &str) -> Ordering {
    normalize(a).cmp(&normalize(b)).then_with(|| a.cmp(b))
}

/// Liste des filtres prédéfinis du document, triée par nom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoFilters {
    pub filters: Vec<AutoFilter>,
}

impl Default for AutoFilters {
    fn default() -> Self {
        Self::defaults()
    }
}

impl AutoFilters {
    /// Clé de rangement dans le nouveau format.
    pub const KEY: &'static str = "autofilters.v6";
    /// Clé de l'ancien script (pickle d'un dict `{nom: {...}}`).
    pub const LEGACY_KEY: &'static str = "fabiobpautofilters";

    /// Filtres livrés par défaut : « Mois actuel » et « Mois précédent ».
    pub fn defaults() -> Self {
        let month = |name: &str, from: &str, to: &str| AutoFilter {
            date_from: from.into(),
            date_to: to.into(),
            ..AutoFilter::new(name)
        };
        Self {
            filters: vec![
                // Noms dans la langue de l'interface (les dates gardent la
                // syntaxe j/m/a des filtres).
                month(crate::tr!("Mois actuel"), "1/m/a", "dj/m/a"),
                month(crate::tr!("Mois précédent"), "1/m-1/a", "dj/m-1/a"),
            ],
        }
    }

    pub fn get(&self, name: &str) -> Option<&AutoFilter> {
        self.filters.iter().find(|f| f.name == name)
    }

    /// Noms dans l'ordre d'affichage.
    pub fn names(&self) -> Vec<&str> {
        self.filters.iter().map(|f| f.name.as_str()).collect()
    }

    fn sort(&mut self) {
        self.filters.sort_by(|a, b| compare_names(&a.name, &b.name));
    }

    /// Ajoute le filtre, ou remplace celui de même nom. Le nom est nettoyé
    /// ([`clean_name`]) ; les dates sont vérifiées.
    pub fn upsert(&mut self, mut filter: AutoFilter) -> Result<(), AutoFilterError> {
        filter.name = clean_name(&filter.name);
        filter.validate()?;
        match self.filters.iter_mut().find(|f| f.name == filter.name) {
            Some(existing) => *existing = filter,
            None => self.filters.push(filter),
        }
        self.sort();
        Ok(())
    }

    /// Supprime le filtre `name` ; renvoie vrai s'il existait.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.filters.len();
        self.filters.retain(|f| f.name != name);
        self.filters.len() != before
    }

    /// Conversion depuis l'ancien pickle (dict `{nom: {"dt1", …}}`).
    ///
    /// L'ancien script remplaçait un dictionnaire vide par les filtres par
    /// défaut : on fait de même ici, pour retrouver ce que l'utilisateur
    /// voyait. Dans le nouveau format, une liste vide reste vide.
    pub fn from_legacy(value: &PyValue) -> Result<Self, LegacyError> {
        let Some(items) = value.as_dict() else {
            return Err(LegacyError::Structure(
                tr!("filtres prédéfinis : dictionnaire attendu").into(),
            ));
        };
        if items.is_empty() {
            return Ok(Self::defaults());
        }
        let mut filters = Vec::new();
        for (key, value) in items {
            let Some(name) = key.as_str() else {
                return Err(LegacyError::Structure(
                    tr!("filtres prédéfinis : nom de filtre non textuel").into(),
                ));
            };
            filters.push(AutoFilter::from_legacy(name, value)?);
        }
        let mut result = Self { filters };
        result.sort();
        Ok(result)
    }

    /// Lit les filtres du document : nouvelle clé, sinon ancien pickle
    /// (décodé par `parse_pickle`), sinon filtres par défaut.
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
        .unwrap_or_else(|| Ok(Self::defaults()))
    }

    /// Range les filtres dans le document (et retire l'ancienne clé).
    pub fn save(&self, document: &mut Document) -> Result<(), FeatureError> {
        feature_store::set(document, Self::KEY, self)?;
        feature_store::remove(document, Self::LEGACY_KEY);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature_store::tests::empty_document;

    fn ymd(y: i32, m: u32, d: u32) -> Date {
        Date::from_ymd(y, m, d).expect("date valide")
    }

    fn dyn_date(expr: &str, today: Date) -> Option<Date> {
        eval_dynamic_date(expr, today).expect("expression valide")
    }

    #[test]
    fn default_filters_month_boundaries() {
        let today = ymd(2026, 10, 11);
        assert_eq!(dyn_date("1/m/a", today), Some(ymd(2026, 10, 1)));
        assert_eq!(dyn_date("dj/m/a", today), Some(ymd(2026, 10, 31)));
        assert_eq!(dyn_date("1/m-1/a", today), Some(ymd(2026, 9, 1)));
        assert_eq!(dyn_date("dj/m-1/a", today), Some(ymd(2026, 9, 30)));
        assert_eq!(dyn_date("j/m/a", today), Some(today));
        assert_eq!(dyn_date("15/03/2024", today), Some(ymd(2024, 3, 15)));
        assert_eq!(dyn_date("", today), None);
        assert_eq!(dyn_date("  ", today), None);
    }

    #[test]
    fn year_end_and_january() {
        let january = ymd(2027, 1, 15);
        assert_eq!(dyn_date("1/m-1/a", january), Some(ymd(2026, 12, 1)));
        assert_eq!(dyn_date("dj/m-1/a", january), Some(ymd(2026, 12, 31)));
        assert_eq!(dyn_date("1/m-13/a", january), Some(ymd(2025, 12, 1)));
        let december = ymd(2026, 12, 31);
        assert_eq!(dyn_date("1/m+1/a", december), Some(ymd(2027, 1, 1)));
        assert_eq!(dyn_date("j+1/m/a", december), Some(ymd(2027, 1, 1)));
        assert_eq!(dyn_date("1/1/a+1", december), Some(ymd(2027, 1, 1)));
        assert_eq!(dyn_date("1/13/a", december), Some(ymd(2027, 1, 1)));
        assert_eq!(dyn_date("1/0/a", december), Some(ymd(2025, 12, 1)));
    }

    #[test]
    fn february_and_leap_years() {
        let march_2024 = ymd(2024, 3, 31);
        assert_eq!(dyn_date("dj/m-1/a", march_2024), Some(ymd(2024, 2, 29)));
        let march_2023 = ymd(2023, 3, 31);
        assert_eq!(dyn_date("dj/m-1/a", march_2023), Some(ymd(2023, 2, 28)));
        // Jour fixe trop grand pour le mois : report sur le mois suivant.
        assert_eq!(dyn_date("31/2/a", march_2023), Some(ymd(2023, 3, 3)));
        assert_eq!(dyn_date("31/2/a", march_2024), Some(ymd(2024, 3, 2)));
        assert_eq!(
            dyn_date("30/02/2100", march_2024),
            Some(ymd(2100, 3, 2)),
            "2100 non bissextile"
        );
        assert_eq!(dyn_date("dj/2/2000", march_2024), Some(ymd(2000, 2, 29)));
        // Jour nul ou négatif : report sur le mois précédent.
        assert_eq!(dyn_date("0/3/a", march_2024), Some(ymd(2024, 2, 29)));
        assert_eq!(
            dyn_date("j-50/m/a", ymd(2024, 3, 10)),
            Some(ymd(2024, 1, 20))
        );
        assert_eq!(
            dyn_date("j-365/m/a", ymd(2024, 3, 10)),
            Some(ymd(2023, 3, 11))
        );
        assert_eq!(
            dyn_date("dj-6/m/a", ymd(2024, 2, 10)),
            Some(ymd(2024, 2, 23))
        );
    }

    #[test]
    fn arithmetic_in_parts() {
        let today = ymd(2026, 10, 11);
        assert_eq!(dyn_date("2*7/m/a", today), Some(ymd(2026, 10, 14)));
        assert_eq!(dyn_date("dj-(j-1)/m/a", today), Some(ymd(2026, 10, 21)));
        assert_eq!(
            dyn_date(" 1 / m - 1 / a - 1 ", today),
            Some(ymd(2025, 9, 1))
        );
        assert_eq!(
            dyn_date("1/(m-1)%12+1/a", ymd(2026, 12, 3)),
            Some(ymd(2026, 12, 1))
        );
    }

    #[test]
    fn malformed_dates_rejected() {
        let today = ymd(2026, 10, 11);
        for expr in [
            "1/m",
            "1/m/a/b",
            "x/m/a",
            "j/a/a",
            "1/m/m",
            "1/m/a+",
            "1.5/m/a",
            "__import__('os')/m/a",
            "1/m/0",
            "1/m/10000",
            "j-9999999/m/a",
            "'1'/m/a",
        ] {
            assert!(eval_dynamic_date(expr, today).is_err(), "{expr:?}");
        }
        let mut filter = AutoFilter::new("Test");
        filter.date_from = "1/m".into();
        assert!(matches!(
            filter.validate(),
            Err(AutoFilterError::InvalidDate { .. })
        ));
        assert_eq!(
            filter.date_range(today),
            (None, None),
            "ignorée comme l'original"
        );
    }

    #[test]
    fn filter_to_grid_filters() {
        let today = ymd(2026, 1, 5);
        let filter = AutoFilter {
            payee: "edf".into(),
            amount: "<0".into(),
            tack: tack_from_legacy(3),
            date_from: "1/m-1/a".into(),
            date_to: "dj/m-1/a".into(),
            ..AutoFilter::new("EDF du mois dernier")
        };
        let filters = filter.to_filters(today);
        assert_eq!(filters.date_from, Some(ymd(2025, 12, 1)));
        assert_eq!(filters.date_to, Some(ymd(2025, 12, 31)));
        assert_eq!(filters.tack, TackFilter::PointedOrReconciled);
        assert_eq!(filters.payee, "edf");
        assert!(filters.is_active());
        assert_eq!(
            filter.description(today),
            "Opérations concernées : toutes les lignes pointées et rapprochées, du 01/12/2025 au 31/12/2025, ayant « edf » dans le champ tiers, ayant pour montant « <0 »"
        );
        assert_eq!(
            AutoFilter::new("x").description(today),
            "Opérations concernées : toutes"
        );
        for code in 0..=4 {
            assert_eq!(tack_to_legacy(tack_from_legacy(code)), code);
        }
        assert_eq!(tack_from_legacy(17), TackFilter::All);
    }

    #[test]
    fn list_management() {
        let mut filters = AutoFilters::default();
        assert_eq!(filters.names(), ["Mois actuel", "Mois précédent"]);
        filters
            .upsert(AutoFilter::new("  année; &courante "))
            .expect("valide");
        assert_eq!(
            filters.names(),
            ["année courante", "Mois actuel", "Mois précédent"]
        );
        let mut replaced = AutoFilter::new("Mois actuel");
        replaced.payee = "x".into();
        filters.upsert(replaced).expect("valide");
        assert_eq!(filters.filters.len(), 3);
        assert_eq!(
            filters.get("Mois actuel").map(|f| f.payee.as_str()),
            Some("x")
        );
        assert_eq!(
            filters.upsert(AutoFilter::new(" ;& ")),
            Err(AutoFilterError::EmptyName)
        );
        assert!(filters.remove("Mois actuel"));
        assert!(!filters.remove("Mois actuel"));
    }

    #[test]
    fn legacy_and_persistence() {
        let legacy = crate::pyvalue::parse_repr(
            "{'Courses': {'dt1': 'j-30/m/a', 'dt2': '', 'mde': 'carte', 'who': 'super|hyper', 'ctg': '', 'val': '', 'mrk': 1}, 'A': {'dt1': '1/1/a'}}",
        )
        .expect("littéral valide");
        let filters = AutoFilters::from_legacy(&legacy).expect("structure valide");
        assert_eq!(filters.names(), ["A", "Courses"]);
        let courses = filters.get("Courses").expect("présent");
        assert_eq!(courses.date_from, "j-30/m/a");
        assert_eq!(courses.mode, "carte");
        assert_eq!(courses.payee, "super|hyper");
        assert_eq!(courses.tack, TackFilter::Pointed);
        assert_eq!(filters.get("A").map(|f| f.tack), Some(TackFilter::All));

        // Dictionnaire vide (cas d'Exemple.bp) : filtres par défaut.
        assert_eq!(
            AutoFilters::from_legacy(&PyValue::Dict(Vec::new())),
            Ok(AutoFilters::defaults())
        );
        assert!(AutoFilters::from_legacy(&PyValue::List(Vec::new())).is_err());

        let mut doc = empty_document();
        let no_pickle = |_: &[u8]| None;
        assert_eq!(
            AutoFilters::load(&doc, &no_pickle).ok(),
            Some(AutoFilters::defaults())
        );
        doc.feature_data.push(crate::model::FeatureData {
            key: AutoFilters::LEGACY_KEY.into(),
            value: b"pickle".to_vec(),
        });
        let parse = |_: &[u8]| Some(legacy.clone());
        assert_eq!(AutoFilters::load(&doc, &parse).ok(), Some(filters.clone()));

        // Une liste vidée volontairement reste vide après enregistrement.
        let empty = AutoFilters {
            filters: Vec::new(),
        };
        empty.save(&mut doc).expect("enregistrable");
        assert!(feature_store::raw(&doc, AutoFilters::LEGACY_KEY).is_none());
        assert_eq!(AutoFilters::load(&doc, &parse).ok(), Some(empty));
    }
}
