//! Échéancier : modèle des échéances et moteur de calcul.
//!
//! Reprend les anciens scripts `Scheduler/scheduler.py` (liste et application
//! manuelle), `scheduler_auto.py` (insertion automatique à l'ouverture du
//! fichier), `scheduler_add.py` (création d'une échéance depuis une opération)
//! et `previsions.py` (solde prévisionnel).
//!
//! # Stockage
//!
//! Les échéances sont stockées dans [`Document::feature_data`] sous la clé
//! [`FEATURE_KEY`] (`scheduler.v6`), en JSON (`{"version": 1, "schedules": […]}`).
//! Si cette clé est absente, on relit l'ancienne donnée pickle
//! [`LEGACY_KEY`] (`fabiobpscheduler`) sans la modifier ; la première
//! sauvegarde écrit la nouvelle clé, qui l'emporte ensuite. La très ancienne
//! clé [`LEGACY_OLD_KEY`] (`echeancier`), ignorée par les scripts 7.x, n'est
//! pas relue automatiquement : la réactiver insérerait des échéances oubliées
//! depuis des années.
//!
//! # Écarts avec les scripts d'origine
//!
//! Voir `docs/corrections.md` (C32 à C41). En plus des corrections, le
//! nouveau modèle accepte tous les modes de paiement (y compris « Titre » et
//! les modes personnalisés, l'ancien était limité aux 9 premiers modes
//! standard) et une périodicité en semaines.

use chrono::{Datelike, Weekday};
use serde::{Deserialize, Serialize};

use crate::edit::mode_is_debit;
use crate::legacy_pickle::{self, LegacyScheduleEntry};
use crate::model::{Amount, Date, Document, FeatureData, Mode, Operation, StandardMode, Tack};
use crate::{Error, Result};

/// Clé de stockage des échéances dans le nouveau format.
pub const FEATURE_KEY: &str = "scheduler.v6";
/// Clé de l'ancien script (pickle Python 2), relue si la nouvelle est absente.
pub const LEGACY_KEY: &str = "fabiobpscheduler";
/// Donnée du tout premier échéancier (même structure), jamais relue
/// d'office : ses échéances datent souvent de plusieurs années.
pub const OLDEST_KEY: &str = "echeancier";
/// Clé d'une version plus ancienne de l'échéancier, jamais relue automatiquement.
pub const LEGACY_OLD_KEY: &str = "echeancier";
/// Préfixe ajouté au tiers des opérations insérées (hors transferts).
pub const AUTO_PREFIX: &str = "[auto] ";
/// Version du format JSON stocké sous [`FEATURE_KEY`].
pub const STORAGE_VERSION: u32 = 1;

/// Modes de l'ancien échéancier, dans l'ordre de leur index (`mode` 0 à 8).
pub const LEGACY_MODES: [StandardMode; 9] = [
    StandardMode::Carte,
    StandardMode::RetraitDab,
    StandardMode::ChequeEmis,
    StandardMode::Prelevement,
    StandardMode::VirementEmis,
    StandardMode::Tip,
    StandardMode::Versement,
    StandardMode::DepotCheque,
    StandardMode::VirementRecu,
];

/// Nombre maximal d'occurrences calculées en une fois pour une échéance
/// (garde-fou : une échéance quotidienne oubliée depuis longtemps reprend à la
/// prochaine insertion là où elle s'est arrêtée).
pub const MAX_OCCURRENCES: usize = 100_000;

/// Plus petite et plus grande dates gérées (01/01/0001 et 31/12/9999).
const MIN_DATE: i64 = -693_593;
const MAX_DATE: i64 = 2_958_465;

// ---------------------------------------------------------------------------
// Modèle
// ---------------------------------------------------------------------------

/// Unité de la périodicité.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PeriodUnit {
    Days,
    /// Nouveauté : l'ancien échéancier ne connaissait que jours, mois et ans.
    Weeks,
    #[default]
    Months,
    Years,
}

/// Périodicité : « tous les `every` `unit` ».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Period {
    /// Intervalle ; 0 est invalide (l'échéance est alors refusée).
    pub every: u32,
    pub unit: PeriodUnit,
}

impl Default for Period {
    fn default() -> Self {
        Self {
            every: 1,
            unit: PeriodUnit::Months,
        }
    }
}

/// Report de la date d'insertion lorsqu'elle tombe un jour non ouvré
/// (option « Postpone » de l'ancien échéancier : lettres `S`, `D`, `F`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Postpone {
    pub saturday: bool,
    pub sunday: bool,
    /// Jours fériés français.
    pub holiday: bool,
}

impl Postpone {
    /// Lit le codage de l'ancien échéancier (`"SDF"`, `"F"`…).
    pub fn from_letters(letters: &str) -> Self {
        Self {
            saturday: letters.contains('S'),
            sunday: letters.contains('D'),
            holiday: letters.contains('F'),
        }
    }

    /// Codage de l'ancien échéancier.
    pub fn to_letters(self) -> String {
        [
            (self.saturday, 'S'),
            (self.sunday, 'D'),
            (self.holiday, 'F'),
        ]
        .into_iter()
        .filter_map(|(on, c)| on.then_some(c))
        .collect()
    }

    pub fn is_empty(self) -> bool {
        !(self.saturday || self.sunday || self.holiday)
    }
}

/// Une échéance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    /// Identifiant (`Account::uid`) du compte de l'échéance. L'ancien
    /// échéancier stockait l'index du compte, faux dès qu'un compte était
    /// supprimé.
    pub account: String,
    /// Compte lié si l'échéance est un transfert.
    pub transfer: Option<String>,
    /// Tiers.
    pub payee: String,
    /// Détail.
    pub details: String,
    pub mode: Mode,
    pub category: Option<u32>,
    /// Montant ; à l'insertion, son signe est imposé par le mode.
    pub amount: Amount,
    /// Prochaine échéance (avant report éventuel).
    pub next_date: Date,
    /// Dernière échéance, incluse.
    pub end_date: Option<Date>,
    /// Jour du mois voulu (1 à 31) pour les périodicités en mois et en années :
    /// 31 = toujours le dernier jour du mois. `None` : jour de `next_date`.
    pub anchor_day: Option<u8>,
    pub period: Period,
    /// Insertion automatique à l'ouverture du fichier.
    pub auto_insert: bool,
    pub postpone: Postpone,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            account: String::new(),
            transfer: None,
            payee: String::new(),
            details: String::new(),
            // Valeurs par défaut de la fenêtre d'édition d'origine.
            mode: Mode::Standard(StandardMode::Prelevement),
            category: None,
            amount: Amount::ZERO,
            next_date: Date(0),
            end_date: None,
            anchor_day: None,
            period: Period::default(),
            auto_insert: true,
            postpone: Postpone::default(),
        }
    }
}

/// Une occurrence d'échéance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Occurrence {
    /// Date théorique de l'échéance.
    pub due: Date,
    /// Date d'insertion, après report éventuel (week-end, férié).
    pub date: Date,
}

impl Schedule {
    /// Date de l'occurrence qui suit `date`.
    pub fn following(&self, date: Date) -> Option<Date> {
        add_period(date, self.period, self.anchor_day, 1)
    }

    /// Date de l'occurrence qui précède `date`.
    pub fn preceding(&self, date: Date) -> Option<Date> {
        add_period(date, self.period, self.anchor_day, -1)
    }

    /// Avance la prochaine échéance d'une période (bouton « en avant »).
    pub fn advance(&mut self) -> bool {
        match self.following(self.next_date) {
            Some(date) if self.period.every > 0 => {
                self.next_date = date;
                true
            }
            _ => false,
        }
    }

    /// Recule la prochaine échéance d'une période (bouton « en arrière »).
    pub fn rewind(&mut self) -> bool {
        match self.preceding(self.next_date) {
            Some(date) if self.period.every > 0 => {
                self.next_date = date;
                true
            }
            _ => false,
        }
    }

    /// « Dernière échéance atteinte » : la date de fin est dépassée.
    pub fn is_finished(&self) -> bool {
        self.end_date.is_some_and(|end| end < self.next_date)
    }

    /// Dernière date à prendre en compte pour une insertion jusqu'à `until`.
    fn limit(&self, until: Date) -> Date {
        match self.end_date {
            Some(end) if end < until => end,
            _ => until,
        }
    }

    /// Occurrences dont la date théorique est comprise entre `next_date` et
    /// `until` (borné par la date de fin), dans l'ordre. Vide si l'intervalle
    /// est nul.
    pub fn occurrences(&self, until: Date) -> Vec<Occurrence> {
        let mut out = Vec::new();
        if self.period.every == 0 {
            return out;
        }
        let limit = self.limit(until);
        let mut due = self.next_date;
        while due <= limit && out.len() < MAX_OCCURRENCES {
            out.push(Occurrence {
                due,
                date: postponed_date(due, self.postpone),
            });
            match self.following(due) {
                Some(next) if next > due => due = next,
                _ => break,
            }
        }
        out
    }

    /// Libellé de la colonne « Intervalle » (`Tous les mois`, `Tous les 3 jours`…).
    pub fn interval_label(&self) -> String {
        let every = self.period.every;
        match (self.period.unit, every) {
            (PeriodUnit::Days, 1) => "Tous les jours".into(),
            (PeriodUnit::Weeks, 1) => "Toutes les semaines".into(),
            (PeriodUnit::Months, 1) => "Tous les mois".into(),
            (PeriodUnit::Years, 1) => "Tous les ans".into(),
            (PeriodUnit::Days, n) => format!("Tous les {n} jours"),
            (PeriodUnit::Weeks, n) => format!("Toutes les {n} semaines"),
            (PeriodUnit::Months, n) => format!("Tous les {n} mois"),
            (PeriodUnit::Years, n) => format!("Tous les {n} ans"),
        }
    }
}

// ---------------------------------------------------------------------------
// Calcul des dates
// ---------------------------------------------------------------------------

pub fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn checked_date(serial: i64) -> Option<Date> {
    (MIN_DATE..=MAX_DATE)
        .contains(&serial)
        .then_some(Date(serial as i32))
}

/// Date calendaire, si `date` est dans la plage gérée (une date venue d'un
/// fichier abîmé ne doit pas faire paniquer `Date::to_naive`).
fn naive(date: Date) -> Option<chrono::NaiveDate> {
    checked_date(date.0 as i64).map(Date::to_naive)
}

/// `jj/mm/aaaa`, ou `??/??/????` hors de la plage gérée.
fn format_date(date: Date) -> String {
    match checked_date(date.0 as i64) {
        Some(d) => d.to_string(),
        None => "??/??/????".into(),
    }
}

/// Ajoute (ou retire) des jours.
pub fn add_days(date: Date, days: i64) -> Option<Date> {
    checked_date((date.0 as i64).checked_add(days)?)
}

/// Ajoute (ou retire) des mois. Le jour obtenu est `anchor_day` (à défaut le
/// jour de `date`), ramené au dernier jour du mois s'il le dépasse : le
/// 31/01 + 1 mois donne le 28/02 (29/02 les années bissextiles), et avec
/// `anchor_day = 31` le mois suivant redonne le 31/03.
pub fn add_months(date: Date, months: i64, anchor_day: Option<u8>) -> Option<Date> {
    let d = naive(date)?;
    let total = (d.year() as i64 * 12 + d.month0() as i64).checked_add(months)?;
    let year = i32::try_from(total.div_euclid(12)).ok()?;
    let month = (total.rem_euclid(12) + 1) as u32;
    if !(1..=9999).contains(&year) {
        return None;
    }
    let wanted = anchor_day.map_or(d.day(), u32::from).clamp(1, 31);
    Date::from_ymd(year, month, wanted.min(days_in_month(year, month)))
}

/// Ajoute `count` périodes (négatif : en arrière).
pub fn add_period(date: Date, period: Period, anchor_day: Option<u8>, count: i64) -> Option<Date> {
    let n = (period.every as i64).checked_mul(count)?;
    match period.unit {
        PeriodUnit::Days => add_days(date, n),
        PeriodUnit::Weeks => add_days(date, n.checked_mul(7)?),
        PeriodUnit::Months => add_months(date, n, anchor_day),
        PeriodUnit::Years => add_months(date, n.checked_mul(12)?, anchor_day),
    }
}

/// Date du dimanche de Pâques (calendrier grégorien).
///
/// Même algorithme que `easter_date` des scripts d'origine (méthode d'Oudin),
/// dont les `/` sont des divisions entières Python 2 : ici des divisions
/// entières explicites (opérandes positifs pour toute année ≥ 1583).
pub fn easter_sunday(year: i32) -> Option<Date> {
    if !(1583..=9999).contains(&year) {
        return None;
    }
    let y = year;
    let g = y % 19;
    let c = y / 100;
    let h = (19 * g + c - c / 4 - (8 * c + 13) / 25 + 15).rem_euclid(30);
    let i = ((h / 28) * (29 / (h + 1)) * ((21 - g) / 11) - 1) * (h / 28) + h;
    let r = 28 + i - ((y / 4 + y + i + 2 + c / 4 - c).rem_euclid(7));
    if r <= 31 {
        Date::from_ymd(y, 3, r as u32)
    } else {
        Date::from_ymd(y, 4, (r - 31) as u32)
    }
}

/// Jour férié français, selon la liste de l'ancien échéancier : 1er janvier,
/// 1er mai, 8 mai, 14 juillet, 15 août, 1er novembre, 11 novembre, 25 décembre,
/// lundi de Pâques, Ascension, lundi de Pentecôte.
pub fn is_french_holiday(date: Date) -> bool {
    let Some(d) = naive(date) else {
        return false;
    };
    const FIXED: [(u32, u32); 8] = [
        (1, 1),
        (5, 1),
        (5, 8),
        (7, 14),
        (8, 15),
        (11, 1),
        (11, 11),
        (12, 25),
    ];
    if FIXED.contains(&(d.month(), d.day())) {
        return true;
    }
    easter_sunday(d.year()).is_some_and(|easter| {
        [1, 39, 50]
            .iter()
            .any(|&offset| add_days(easter, offset) == Some(date))
    })
}

/// Jour non ouvré au sens de l'option de report.
pub fn is_non_working_day(date: Date, postpone: Postpone) -> bool {
    let Some(d) = naive(date) else {
        return false;
    };
    match d.weekday() {
        Weekday::Sat if postpone.saturday => true,
        Weekday::Sun if postpone.sunday => true,
        _ => postpone.holiday && is_french_holiday(date),
    }
}

/// Premier jour ouvré à partir de `date` (incluse), selon l'option de report.
pub fn postponed_date(date: Date, postpone: Postpone) -> Date {
    let mut current = date;
    // Au plus quelques jours (samedi, dimanche, lundi férié…) ; la borne est
    // un simple garde-fou.
    for _ in 0..31 {
        if !is_non_working_day(current, postpone) {
            break;
        }
        match add_days(current, 1) {
            Some(next) => current = next,
            None => break,
        }
    }
    current
}

// ---------------------------------------------------------------------------
// Opérations produites par une échéance
// ---------------------------------------------------------------------------

/// Mode de la ligne réciproque d'un transfert (`OtherMode` de l'original).
pub fn reciprocal_mode(document: &Document, mode: &Mode) -> Mode {
    use StandardMode::*;
    let standard = match mode {
        Mode::Standard(m) => match m {
            Carte | Prelevement | VirementEmis | Tip => VirementRecu,
            RetraitDab => Versement,
            ChequeEmis => DepotCheque,
            Versement => RetraitDab,
            DepotCheque => ChequeEmis,
            VirementRecu => Prelevement,
            Titre => Titre,
        },
        Mode::Custom(_) if mode_is_debit(document, mode) => VirementRecu,
        Mode::Custom(_) => Prelevement,
    };
    Mode::Standard(standard)
}

/// Montant d'une ligne : le signe est imposé par le mode (`LineAdd`).
fn line_amount(document: &Document, mode: &Mode, amount: Amount) -> Amount {
    let abs = Amount(amount.0.saturating_abs());
    if mode_is_debit(document, mode) {
        -abs
    } else {
        abs
    }
}

fn account_index(document: &Document, uid: &str) -> Option<usize> {
    if uid.is_empty() {
        return None;
    }
    document.accounts.iter().position(|a| a.uid == uid)
}

/// Ligne à produire pour une occurrence.
#[derive(Debug, Clone, PartialEq)]
struct PlannedLine {
    account: usize,
    mode: Mode,
    payee: String,
    amount: Amount,
}

/// Raison du refus d'une échéance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    /// Le compte de l'échéance n'existe pas (ou plus).
    UnknownAccount,
    /// Le compte lié du transfert n'existe pas (ou plus).
    UnknownTransferAccount,
    /// Transfert d'un compte vers lui-même.
    SameAccount,
    /// Intervalle nul.
    NullPeriod,
}

impl RefusalReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::UnknownAccount => "compte inexistant",
            Self::UnknownTransferAccount => "compte du transfert inexistant",
            Self::SameAccount => "transfert vers le même compte",
            Self::NullPeriod => "intervalle nul",
        }
    }
}

/// Lignes produites par une occurrence de l'échéance : une ligne, ou deux pour
/// un transfert (« Transfert vers/depuis le compte X », mode réciproque).
/// `with_prefix` ajoute `[auto] ` au tiers d'une opération simple.
fn planned_lines(
    document: &Document,
    schedule: &Schedule,
    with_prefix: bool,
) -> std::result::Result<Vec<PlannedLine>, RefusalReason> {
    if schedule.period.every == 0 {
        return Err(RefusalReason::NullPeriod);
    }
    let acc1 = account_index(document, &schedule.account).ok_or(RefusalReason::UnknownAccount)?;
    let Some(transfer) = &schedule.transfer else {
        let payee = if with_prefix {
            format!("{AUTO_PREFIX}{}", schedule.payee)
        } else {
            schedule.payee.clone()
        };
        return Ok(vec![PlannedLine {
            account: acc1,
            mode: schedule.mode.clone(),
            payee,
            amount: line_amount(document, &schedule.mode, schedule.amount),
        }]);
    };
    let acc2 = account_index(document, transfer).ok_or(RefusalReason::UnknownTransferAccount)?;
    if acc1 == acc2 {
        return Err(RefusalReason::SameAccount);
    }
    let mode1 = schedule.mode.clone();
    let mode2 = reciprocal_mode(document, &mode1);
    let (sens1, sens2) = if mode_is_debit(document, &mode1) {
        ("vers", "depuis")
    } else {
        ("depuis", "vers")
    };
    let name1 = &document.accounts[acc1].name;
    let name2 = &document.accounts[acc2].name;
    Ok(vec![
        PlannedLine {
            account: acc1,
            payee: format!("Transfert {sens1} le compte {name2}"),
            amount: line_amount(document, &mode1, schedule.amount),
            mode: mode1,
        },
        PlannedLine {
            account: acc2,
            payee: format!("Transfert {sens2} le compte {name1}"),
            amount: line_amount(document, &mode2, -schedule.amount),
            mode: mode2,
        },
    ])
}

// ---------------------------------------------------------------------------
// Insertion
// ---------------------------------------------------------------------------

/// Échéances concernées par une insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// L'échéance d'index donné (« La ligne sélectionnée »).
    One(usize),
    /// « Les lignes qui ne s'insèrent pas automatiquement ».
    NotAutomatic,
    /// Les échéances à insertion automatique (ouverture du fichier).
    Automatic,
    /// « Toutes les lignes ».
    All,
}

impl Selection {
    fn matches(self, index: usize, schedule: &Schedule) -> bool {
        match self {
            Self::One(i) => i == index,
            Self::NotAutomatic => !schedule.auto_insert,
            Self::Automatic => schedule.auto_insert,
            Self::All => true,
        }
    }
}

/// Opération insérée.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertedLine {
    /// Index de l'échéance.
    pub schedule: usize,
    /// Index réel du compte.
    pub account: usize,
    pub uid: u64,
    pub date: Date,
    pub payee: String,
    pub amount: Amount,
}

/// Échéance refusée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal {
    pub schedule: usize,
    pub reason: RefusalReason,
}

/// Bilan d'une insertion.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InsertionReport {
    pub inserted: Vec<InsertedLine>,
    pub refused: Vec<Refusal>,
}

impl InsertionReport {
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty() && self.refused.is_empty()
    }

    /// Message récapitulatif, au format de `scheduler_auto.py`.
    pub fn summary(&self, document: &Document, schedules: &[Schedule]) -> String {
        let account_name = |i: usize| document.accounts.get(i).map_or("---", |a| a.name.as_str());
        let mut text = String::from("Echéancier :\n\n");
        if !self.inserted.is_empty() {
            text.push_str("Lignes insérées :\n");
            for line in &self.inserted {
                text.push_str(&format!(
                    "Compte {} : échéance \"{}\" du {} (montant : {})\n",
                    account_name(line.account),
                    line.payee,
                    format_date(line.date),
                    line.amount.format(2, ',')
                ));
            }
        }
        if !self.refused.is_empty() {
            text.push_str("Lignes refusées :\n");
            for refusal in &self.refused {
                let Some(s) = schedules.get(refusal.schedule) else {
                    continue;
                };
                let account = account_index(document, &s.account).map_or("---", account_name);
                text.push_str(&format!(
                    "Compte {account} : échéance \"{}\" du {} (montant : {})... {}\n",
                    s.payee,
                    format_date(s.next_date),
                    s.amount.format(2, ','),
                    refusal.reason.label()
                ));
            }
        }
        text
    }
}

/// Insère les occurrences des échéances `selection` dont la date théorique
/// est antérieure ou égale à `until` (et à la date de fin de chacune).
///
/// Les lignes sont ajoutées à la fin des comptes, non pointées, avec la date
/// reportée selon l'option de report ; le tiers d'une opération simple reçoit
/// le préfixe `[auto] `, un transfert produit deux lignes liées
/// (`#sisterAccUid`, `#sisterLineUid`). Une catégorie inconnue est retirée.
/// Sauf en `simulate` (« simulation » de l'original), la prochaine échéance
/// de chaque échéance traitée est avancée.
pub fn insert_due(
    document: &mut Document,
    schedules: &mut [Schedule],
    until: Date,
    selection: Selection,
    simulate: bool,
) -> InsertionReport {
    let mut report = InsertionReport::default();
    let mut next_uid = document.next_operation_uid();
    for (index, schedule) in schedules.iter_mut().enumerate() {
        if !selection.matches(index, schedule) {
            continue;
        }
        let lines = match planned_lines(document, schedule, true) {
            Ok(lines) => lines,
            Err(reason) => {
                report.refused.push(Refusal {
                    schedule: index,
                    reason,
                });
                continue;
            }
        };
        let category = schedule
            .category
            .filter(|&id| document.category(id).is_some());
        let occurrences = schedule.occurrences(until);
        for occurrence in &occurrences {
            let first_uid = next_uid;
            for (n, line) in lines.iter().enumerate() {
                let mut operation = Operation {
                    uid: next_uid,
                    date: occurrence.date,
                    mark_date: occurrence.date,
                    tack: Tack::None,
                    mode: line.mode.clone(),
                    payee: line.payee.clone(),
                    details: schedule.details.clone(),
                    category,
                    amount: line.amount,
                    extra: Vec::new(),
                };
                if lines.len() == 2 {
                    let other = &lines[1 - n];
                    let other_uid = if n == 0 { first_uid + 1 } else { first_uid };
                    operation.extra = vec![
                        (
                            "#sisterAccUid".into(),
                            document.accounts[other.account].uid.clone(),
                        ),
                        ("#sisterLineUid".into(), other_uid.to_string()),
                    ];
                }
                document.accounts[line.account].operations.push(operation);
                report.inserted.push(InsertedLine {
                    schedule: index,
                    account: line.account,
                    uid: next_uid,
                    date: occurrence.date,
                    payee: line.payee.clone(),
                    amount: line.amount,
                });
                next_uid += 1;
            }
        }
        if !simulate
            && let Some(last) = occurrences.last()
            && let Some(next) = schedule.following(last.due)
        {
            schedule.next_date = next;
        }
    }
    report
}

/// Insertion automatique à l'ouverture d'un fichier (`scheduler_auto.py`) :
/// charge les échéances, insère celles à insertion automatique jusqu'à
/// `today`, puis enregistre les échéances si des lignes ont été insérées.
pub fn auto_insert(document: &mut Document, today: Date) -> Result<InsertionReport> {
    let mut schedules = load(document)?.schedules;
    let report = insert_due(document, &mut schedules, today, Selection::Automatic, false);
    if !report.inserted.is_empty() {
        save(document, &schedules)?;
    }
    Ok(report)
}

// ---------------------------------------------------------------------------
// Solde prévisionnel
// ---------------------------------------------------------------------------

/// Nature d'une ligne du prévisionnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastKind {
    /// « Solde actuel » : lignes pointées ou datées jusqu'à aujourd'hui.
    CurrentBalance,
    /// Occurrence d'une échéance.
    Schedule,
    /// Opération non pointée à venir.
    Pending,
    /// Regroupement mensuel (« Une seule ligne par mois »).
    Month,
}

/// Ligne du prévisionnel.
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastLine {
    pub date: Date,
    pub payee: String,
    pub amount: Amount,
    /// Solde après la ligne.
    pub balance: Amount,
    pub kind: ForecastKind,
    /// Nombre de lignes regroupées (1 hors regroupement mensuel).
    pub count: usize,
    /// Solde en millièmes du plus grand solde absolu (barre de progression).
    pub progress: i32,
}

/// Prévisionnel du compte `account` (index réel) jusqu'à `until`
/// (`previsions.py`).
///
/// Première ligne : le solde actuel (lignes pointées ou datées jusqu'à
/// `today`), datée de la plus récente de ces lignes. Puis, dans l'ordre
/// chronologique : les opérations non pointées à venir jusqu'à `until`, et les
/// occurrences des échéances du compte (y compris les transferts reçus d'un
/// autre compte) jusqu'à `until`, à leur date reportée. Avec
/// `one_line_per_month`, les lignes à venir sont regroupées par mois, datées
/// du dernier jour du mois et portant le tiers de la dernière ligne du mois.
pub fn forecast(
    document: &Document,
    schedules: &[Schedule],
    account: usize,
    until: Date,
    today: Date,
    one_line_per_month: bool,
) -> Vec<ForecastLine> {
    let Some(acc) = document.accounts.get(account) else {
        return Vec::new();
    };
    let mut total = Amount::ZERO;
    let mut last_date: Option<Date> = None;
    let mut future: Vec<(Date, String, Amount, ForecastKind)> = Vec::new();
    for op in &acc.operations {
        if op.tack != Tack::None || op.date <= today {
            total = total + op.amount;
            last_date = last_date.max(Some(op.date));
        } else if op.date <= until {
            future.push((op.date, op.payee.clone(), op.amount, ForecastKind::Pending));
        }
    }
    for schedule in schedules {
        let Ok(lines) = planned_lines(document, schedule, false) else {
            continue;
        };
        for line in lines.iter().filter(|l| l.account == account) {
            for occurrence in schedule.occurrences(until) {
                future.push((
                    occurrence.date,
                    line.payee.clone(),
                    line.amount,
                    ForecastKind::Schedule,
                ));
            }
        }
    }
    future.sort_by(|a, b| (a.0, &a.1, a.2).cmp(&(b.0, &b.1, b.2)));

    let mut lines = vec![ForecastLine {
        date: last_date.unwrap_or(today),
        payee: "Solde actuel".into(),
        amount: total,
        balance: total,
        kind: ForecastKind::CurrentBalance,
        count: 1,
        progress: 0,
    }];
    if one_line_per_month {
        let mut grouped: Vec<(Date, String, Amount, usize)> = Vec::new();
        for (date, payee, amount, _) in future {
            let end = naive(date)
                .and_then(|d| {
                    Date::from_ymd(d.year(), d.month(), days_in_month(d.year(), d.month()))
                })
                .unwrap_or(date);
            match grouped.last_mut() {
                Some(last) if last.0 == end => {
                    last.1 = payee;
                    last.2 = last.2 + amount;
                    last.3 += 1;
                }
                _ => grouped.push((end, payee, amount, 1)),
            }
        }
        for (date, payee, amount, count) in grouped {
            total = total + amount;
            lines.push(ForecastLine {
                date,
                payee,
                amount,
                balance: total,
                kind: ForecastKind::Month,
                count,
                progress: 0,
            });
        }
    } else {
        for (date, payee, amount, kind) in future {
            total = total + amount;
            lines.push(ForecastLine {
                date,
                payee,
                amount,
                balance: total,
                kind,
                count: 1,
                progress: 0,
            });
        }
    }
    let max = lines
        .iter()
        .map(|l| l.balance.0.unsigned_abs())
        .max()
        .unwrap_or(0);
    if max > 0 {
        for line in &mut lines {
            line.progress = (line.balance.0 as i128 * 1000 / max as i128) as i32;
        }
    }
    lines
}

/// Solde prévisionnel du compte `account` à la date `until`.
pub fn forecast_balance(
    document: &Document,
    schedules: &[Schedule],
    account: usize,
    until: Date,
    today: Date,
) -> Amount {
    forecast(document, schedules, account, until, today, false)
        .last()
        .map_or(Amount::ZERO, |l| l.balance)
}

// ---------------------------------------------------------------------------
// Création depuis une opération
// ---------------------------------------------------------------------------

/// Échéance mensuelle créée depuis une opération (`scheduler_add.py`) :
/// prochaine échéance un mois après l'opération, même jour du mois (le 31
/// reste en fin de mois), insertion automatique, sans date de fin.
/// `None` si `account` n'est pas un index de compte valide.
pub fn schedule_from_operation(
    document: &Document,
    account: usize,
    operation: &Operation,
) -> Option<Schedule> {
    let acc = document.accounts.get(account)?;
    let day = naive(operation.date)?.day() as u8;
    let anchor_day = Some(day);
    Some(Schedule {
        account: acc.uid.clone(),
        transfer: None,
        // Une opération déjà insérée par l'échéancier porte le préfixe
        // « [auto] » : il est retiré, sinon il serait doublé à l'insertion.
        payee: operation
            .payee
            .strip_prefix(AUTO_PREFIX)
            .unwrap_or(&operation.payee)
            .to_string(),
        details: operation.details.clone(),
        mode: operation.mode.clone(),
        category: operation.category,
        amount: operation.amount,
        next_date: add_months(operation.date, 1, anchor_day)?,
        end_date: None,
        anchor_day,
        period: Period {
            every: 1,
            unit: PeriodUnit::Months,
        },
        auto_insert: true,
        postpone: Postpone::default(),
    })
}

// ---------------------------------------------------------------------------
// Conversion des anciennes données
// ---------------------------------------------------------------------------

/// Date `(a, m, j)` de l'ancien format ; le jour est ramené dans le mois.
fn legacy_date((y, m, d): (i32, u32, u32)) -> Option<Date> {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 {
        return None;
    }
    Date::from_ymd(y, m, d.min(days_in_month(y, m)))
}

/// Convertit une échéance de l'ancien échéancier.
///
/// Les index de comptes deviennent des identifiants de compte (index inconnu
/// → identifiant vide, l'échéance sera refusée à l'insertion, comme
/// l'original refusait un « compte inexistant »). Le jour d'ancrage
/// reproduit l'ancien calcul : en mois, une prochaine échéance au dernier jour
/// du mois reste en fin de mois.
pub fn schedule_from_legacy(
    document: &Document,
    entry: &LegacyScheduleEntry,
) -> std::result::Result<Schedule, String> {
    let next_date = legacy_date(entry.nextdate).ok_or_else(|| {
        format!(
            "échéance « {} » : prochaine échéance invalide {:?}",
            entry.tiers, entry.nextdate
        )
    })?;
    let account_uid = |index: i64| {
        usize::try_from(index)
            .ok()
            .and_then(|i| document.accounts.get(i))
            .map(|a| a.uid.clone())
            .unwrap_or_default()
    };
    let mode = usize::try_from(entry.mode)
        .ok()
        .and_then(|i| LEGACY_MODES.get(i).copied())
        .unwrap_or(if entry.montant > 0.0 {
            StandardMode::VirementRecu
        } else {
            StandardMode::Prelevement
        });
    let unit = match entry.deltaunit {
        0 => PeriodUnit::Days,
        1 => PeriodUnit::Months,
        _ => PeriodUnit::Years,
    };
    let next = next_date.to_naive();
    let anchor_day = match unit {
        PeriodUnit::Months if next.day() == days_in_month(next.year(), next.month()) => Some(31),
        PeriodUnit::Months | PeriodUnit::Years => Some(next.day() as u8),
        PeriodUnit::Days | PeriodUnit::Weeks => None,
    };
    let amount = if entry.montant.is_finite() {
        Amount((entry.montant * Amount::SCALE as f64).round() as i64)
    } else {
        Amount::ZERO
    };
    Ok(Schedule {
        account: account_uid(entry.account),
        transfer: (entry.transfer != -1).then(|| account_uid(entry.transfer)),
        payee: entry.tiers.clone(),
        details: entry.details.clone(),
        mode: Mode::Standard(mode),
        category: u32::try_from(entry.categ).ok(),
        amount,
        next_date,
        end_date: if entry.enddate == (0, 0, 0) {
            None
        } else {
            legacy_date(entry.enddate)
        },
        anchor_day,
        period: Period {
            every: u32::try_from(entry.delta.max(0)).unwrap_or(u32::MAX),
            unit,
        },
        auto_insert: entry.autoinsert,
        postpone: Postpone::from_letters(entry.effective_postpone()),
    })
}

/// Convertit une liste d'anciennes échéances ; les échéances inutilisables
/// sont écartées et signalées.
pub fn schedules_from_legacy(
    document: &Document,
    entries: &[LegacyScheduleEntry],
) -> (Vec<Schedule>, Vec<String>) {
    let mut schedules = Vec::with_capacity(entries.len());
    let mut warnings = Vec::new();
    for entry in entries {
        match schedule_from_legacy(document, entry) {
            Ok(s) => schedules.push(s),
            Err(w) => warnings.push(w),
        }
    }
    (schedules, warnings)
}

/// Échéances du tout premier échéancier ([`OLDEST_KEY`]), à reprendre sur
/// demande ; `None` si le fichier n'en contient pas.
pub fn oldest_schedules(document: &Document) -> Option<(Vec<Schedule>, Vec<String>)> {
    match legacy_pickle::feature_pickle(document, OLDEST_KEY)? {
        Ok(value) => {
            let entries = legacy_pickle::schedule_entries(&value);
            (!entries.is_empty()).then(|| schedules_from_legacy(document, &entries))
        }
        Err(e) => Some((
            Vec::new(),
            vec![format!("ancien échéancier illisible : {e}")],
        )),
    }
}

// ---------------------------------------------------------------------------
// Stockage
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    #[serde(default)]
    schedules: Vec<Schedule>,
}

/// Provenance des échéances chargées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Nouvelle clé [`FEATURE_KEY`].
    V6,
    /// Ancienne donnée pickle [`LEGACY_KEY`].
    Legacy,
    /// Aucune donnée.
    Empty,
}

/// Résultat de [`load`].
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub schedules: Vec<Schedule>,
    pub source: Source,
    /// Anomalies rencontrées dans l'ancienne donnée.
    pub warnings: Vec<String>,
}

/// Sérialise les échéances en JSON.
pub fn to_json(schedules: &[Schedule]) -> Result<Vec<u8>> {
    serde_json::to_vec(&Stored {
        version: STORAGE_VERSION,
        schedules: schedules.to_vec(),
    })
    .map_err(|e| invalid(FEATURE_KEY, e.to_string()))
}

/// Relit les échéances stockées en JSON (champs manquants → valeurs par défaut).
pub fn from_json(data: &[u8]) -> Result<Vec<Schedule>> {
    let stored: Stored =
        serde_json::from_slice(data).map_err(|e| invalid(FEATURE_KEY, e.to_string()))?;
    if stored.version > STORAGE_VERSION {
        return Err(invalid(
            FEATURE_KEY,
            format!("version {} non prise en charge", stored.version),
        ));
    }
    Ok(stored.schedules)
}

fn invalid(key: &str, message: String) -> Error {
    Error::InvalidFeatureData {
        key: key.to_string(),
        message,
    }
}

/// Charge les échéances du document : nouvelle clé, sinon ancienne donnée
/// pickle. Une nouvelle donnée illisible est une erreur (on ne retombe pas
/// sur l'ancienne, qui réinsérerait des échéances déjà traitées) ; une
/// ancienne donnée illisible donne une liste vide, comme dans les scripts.
pub fn load(document: &Document) -> Result<Loaded> {
    if let Some(data) = legacy_pickle::feature_bytes(document, FEATURE_KEY) {
        return Ok(Loaded {
            schedules: from_json(data)?,
            source: Source::V6,
            warnings: Vec::new(),
        });
    }
    match legacy_pickle::feature_pickle(document, LEGACY_KEY) {
        None => Ok(Loaded {
            schedules: Vec::new(),
            source: Source::Empty,
            warnings: Vec::new(),
        }),
        Some(Err(e)) => Ok(Loaded {
            schedules: Vec::new(),
            source: Source::Legacy,
            warnings: vec![format!("échéancier de l'ancienne version illisible : {e}")],
        }),
        Some(Ok(value)) => {
            let entries = legacy_pickle::schedule_entries(&value);
            let (schedules, warnings) = schedules_from_legacy(document, &entries);
            Ok(Loaded {
                schedules,
                source: Source::Legacy,
                warnings,
            })
        }
    }
}

/// Enregistre les échéances sous [`FEATURE_KEY`] (l'ancienne donnée est
/// conservée telle quelle).
pub fn save(document: &mut Document, schedules: &[Schedule]) -> Result<()> {
    let value = to_json(schedules)?;
    match document
        .feature_data
        .iter_mut()
        .find(|f| f.key == FEATURE_KEY)
    {
        Some(feature) => feature.value = value,
        None => document.feature_data.push(FeatureData {
            key: FEATURE_KEY.to_string(),
            value,
        }),
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{Account, Category, CustomMode};

    fn d(y: i32, m: u32, day: u32) -> Date {
        Date::from_ymd(y, m, day).unwrap()
    }

    fn account(uid: &str, name: &str) -> Account {
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
            currency: "€".into(),
            currency_rate: 1.0,
            secondary_currency: String::new(),
            secondary_rate: 1.0,
            is_virtual: false,
            disabled: false,
            operations: Vec::new(),
        }
    }

    pub(crate) fn sample_document() -> Document {
        Document {
            group_name: "Exemple".into(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: vec![CustomMode {
                name: "Espèces".into(),
                debit: true,
            }],
            accounts: vec![
                account("uid-courant", "Courant"),
                account("uid-livret", "Livret"),
            ],
            account_order: vec![0, 1],
            default_account: 0,
            categories: vec![
                Category {
                    id: 13,
                    name: "Salaire".into(),
                    is_child: false,
                },
                Category {
                    id: 0,
                    name: "Alimentation".into(),
                    is_child: false,
                },
            ],
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        }
    }

    fn op(uid: u64, date: Date, tack: Tack, payee: &str, cents: i64) -> Operation {
        Operation {
            uid,
            date,
            mark_date: date,
            tack,
            mode: Mode::Standard(StandardMode::Carte),
            payee: payee.into(),
            details: String::new(),
            category: None,
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        }
    }

    fn monthly(next: Date, anchor: Option<u8>) -> Schedule {
        Schedule {
            account: "uid-courant".into(),
            payee: "Loyer".into(),
            amount: Amount::from_cents(-65_000),
            next_date: next,
            anchor_day: anchor,
            ..Schedule::default()
        }
    }

    // --- Dates ---

    #[test]
    fn bornes_des_dates() {
        assert_eq!(Date::from_ymd(1, 1, 1), Some(Date(MIN_DATE as i32)));
        assert_eq!(Date::from_ymd(9999, 12, 31), Some(Date(MAX_DATE as i32)));
        assert_eq!(add_days(Date(MAX_DATE as i32), 1), None);
        assert_eq!(add_months(d(9999, 12, 1), 1, None), None);
        assert_eq!(add_months(Date(i32::MAX), 1, None), None);
    }

    #[test]
    fn paques_2024_2030() {
        let expected = [
            (2024, 3, 31),
            (2025, 4, 20),
            (2026, 4, 5),
            (2027, 3, 28),
            (2028, 4, 16),
            (2029, 4, 1),
            (2030, 4, 21),
        ];
        for (y, m, day) in expected {
            assert_eq!(easter_sunday(y), Some(d(y, m, day)), "{y}");
        }
        // Dates extrêmes connues : 22 mars 1818 et 25 avril 1943.
        assert_eq!(easter_sunday(1818), Some(d(1818, 3, 22)));
        assert_eq!(easter_sunday(1943), Some(d(1943, 4, 25)));
        assert_eq!(easter_sunday(1500), None);
    }

    /// Algorithme « anonyme grégorien » (Meeus/Jones/Butcher), indépendant.
    fn easter_meeus(y: i32) -> Date {
        let a = y % 19;
        let b = y / 100;
        let c = y % 100;
        let dd = b / 4;
        let e = b % 4;
        let f = (b + 8) / 25;
        let g = (b - f + 1) / 3;
        let h = (19 * a + b - dd - g + 15) % 30;
        let i = c / 4;
        let k = c % 4;
        let l = (32 + 2 * e + 2 * i - h - k) % 7;
        let m = (a + 11 * h + 22 * l) / 451;
        let month = (h + l - 7 * m + 114) / 31;
        let day = (h + l - 7 * m + 114) % 31 + 1;
        d(y, month as u32, day as u32)
    }

    #[test]
    fn paques_comparaison_1600_2600() {
        for y in 1600..=2600 {
            assert_eq!(easter_sunday(y), Some(easter_meeus(y)), "{y}");
        }
    }

    #[test]
    fn jours_feries() {
        for (y, m, day) in [
            (2025, 1, 1),
            (2025, 4, 21), // lundi de Pâques
            (2025, 5, 1),
            (2025, 5, 8),
            (2025, 5, 29), // Ascension
            (2025, 6, 9),  // lundi de Pentecôte
            (2025, 7, 14),
            (2025, 8, 15),
            (2025, 11, 1),
            (2025, 11, 11),
            (2025, 12, 25),
            (2024, 4, 1),  // lundi de Pâques 2024
            (2024, 5, 9),  // Ascension 2024
            (2024, 5, 20), // Pentecôte 2024
        ] {
            assert!(is_french_holiday(d(y, m, day)), "{day}/{m}/{y}");
        }
        for (y, m, day) in [(2025, 4, 20), (2025, 6, 8), (2025, 12, 26), (2025, 4, 22)] {
            assert!(!is_french_holiday(d(y, m, day)), "{day}/{m}/{y}");
        }
    }

    #[test]
    fn report_week_end_et_feries() {
        let all = Postpone::from_letters("SDF");
        let sat = Postpone::from_letters("S");
        let weekend = Postpone::from_letters("SD");
        let holiday = Postpone::from_letters("F");
        // 01/05/2027 est un samedi férié.
        assert_eq!(postponed_date(d(2027, 5, 1), sat), d(2027, 5, 2));
        assert_eq!(postponed_date(d(2027, 5, 1), weekend), d(2027, 5, 3));
        assert_eq!(postponed_date(d(2027, 5, 1), holiday), d(2027, 5, 2));
        // Noël 2027, samedi : lundi 27.
        assert_eq!(postponed_date(d(2027, 12, 25), all), d(2027, 12, 27));
        // Samedi 19/04/2025 + dimanche de Pâques + lundi de Pâques → mardi 22.
        assert_eq!(postponed_date(d(2025, 4, 19), all), d(2025, 4, 22));
        // Sans option, aucun report.
        assert_eq!(
            postponed_date(d(2027, 5, 1), Postpone::default()),
            d(2027, 5, 1)
        );
        // Jour ouvré : inchangé.
        assert_eq!(postponed_date(d(2025, 4, 23), all), d(2025, 4, 23));
        assert_eq!(all.to_letters(), "SDF");
        assert!(Postpone::from_letters("").is_empty());
    }

    #[test]
    fn ajout_de_mois_fin_de_mois() {
        // 31/01 + 1 mois → fin février, puis retour au 31 grâce à l'ancrage.
        let mut s = monthly(d(2023, 1, 31), Some(31));
        assert!(s.advance());
        assert_eq!(s.next_date, d(2023, 2, 28));
        assert!(s.advance());
        assert_eq!(s.next_date, d(2023, 3, 31));
        assert!(s.advance());
        assert_eq!(s.next_date, d(2023, 4, 30));
        assert!(s.rewind());
        assert_eq!(s.next_date, d(2023, 3, 31));

        // Année bissextile.
        assert_eq!(add_months(d(2024, 1, 31), 1, None), Some(d(2024, 2, 29)));
        assert_eq!(
            add_months(d(2024, 1, 31), 1, Some(31)),
            Some(d(2024, 2, 29))
        );
        assert_eq!(add_months(d(2100, 1, 31), 1, None), Some(d(2100, 2, 28)));
        assert_eq!(add_months(d(2000, 1, 31), 1, None), Some(d(2000, 2, 29)));

        // Le 30 reste le 30 (l'original dérivait vers la fin de mois).
        let mut s = monthly(d(2023, 4, 30), Some(30));
        s.advance();
        assert_eq!(s.next_date, d(2023, 5, 30));
        let mut s = monthly(d(2023, 1, 30), Some(30));
        s.advance();
        s.advance();
        assert_eq!(s.next_date, d(2023, 3, 30));

        // Changement d'année, sens négatif.
        assert_eq!(add_months(d(2023, 12, 15), 1, None), Some(d(2024, 1, 15)));
        assert_eq!(add_months(d(2024, 1, 15), -1, None), Some(d(2023, 12, 15)));
        assert_eq!(
            add_months(d(2024, 3, 31), -1, Some(31)),
            Some(d(2024, 2, 29))
        );
        assert_eq!(add_months(d(2024, 1, 10), -25, None), Some(d(2021, 12, 10)));
    }

    #[test]
    fn ajout_jours_semaines_annees() {
        let days = |n| Period {
            every: n,
            unit: PeriodUnit::Days,
        };
        let weeks = Period {
            every: 2,
            unit: PeriodUnit::Weeks,
        };
        let years = Period {
            every: 1,
            unit: PeriodUnit::Years,
        };
        assert_eq!(
            add_period(d(2024, 2, 28), days(1), None, 1),
            Some(d(2024, 2, 29))
        );
        assert_eq!(
            add_period(d(2023, 2, 28), days(1), None, 1),
            Some(d(2023, 3, 1))
        );
        assert_eq!(
            add_period(d(2023, 12, 31), days(1), None, 1),
            Some(d(2024, 1, 1))
        );
        assert_eq!(
            add_period(d(2024, 1, 1), days(10), None, -1),
            Some(d(2023, 12, 22))
        );
        assert_eq!(
            add_period(d(2024, 2, 20), weeks, None, 1),
            Some(d(2024, 3, 5))
        );
        // 29/02 annuel : 28/02 les années non bissextiles, 29/02 sinon.
        assert_eq!(
            add_period(d(2024, 2, 29), years, Some(29), 1),
            Some(d(2025, 2, 28))
        );
        assert_eq!(
            add_period(d(2025, 2, 28), years, Some(29), 1),
            Some(d(2026, 2, 28))
        );
        assert_eq!(
            add_period(d(2027, 2, 28), years, Some(29), 1),
            Some(d(2028, 2, 29))
        );
        assert_eq!(
            add_period(d(2024, 2, 29), years, Some(29), 4),
            Some(d(2028, 2, 29))
        );
    }

    #[test]
    fn libelles_intervalle() {
        let mut s = Schedule::default();
        assert_eq!(s.interval_label(), "Tous les mois");
        s.period = Period {
            every: 3,
            unit: PeriodUnit::Days,
        };
        assert_eq!(s.interval_label(), "Tous les 3 jours");
        s.period.unit = PeriodUnit::Weeks;
        assert_eq!(s.interval_label(), "Toutes les 3 semaines");
        s.period = Period {
            every: 1,
            unit: PeriodUnit::Years,
        };
        assert_eq!(s.interval_label(), "Tous les ans");
    }

    // --- Ancien format ---

    #[test]
    fn conversion_exemple_bp() {
        let doc = sample_document();
        let value = legacy_pickle::parse(legacy_pickle::tests::EXEMPLE_ECHEANCIER).unwrap();
        let entries = legacy_pickle::schedule_entries(&value);
        let (schedules, warnings) = schedules_from_legacy(&doc, &entries);
        assert!(warnings.is_empty());
        let s = &schedules[0];
        assert_eq!(s.account, "uid-courant");
        assert_eq!(s.transfer, None);
        assert_eq!(s.payee, "Coheris");
        assert_eq!(s.details, "paye supp");
        assert_eq!(s.mode, Mode::Standard(StandardMode::Prelevement));
        assert_eq!(s.category, Some(13));
        assert_eq!(s.amount, Amount(-1_230_000));
        assert_eq!(s.next_date, d(2009, 4, 17));
        assert_eq!(s.end_date, None);
        assert_eq!(
            s.period,
            Period {
                every: 1,
                unit: PeriodUnit::Years
            }
        );
        assert_eq!(s.anchor_day, Some(17));
        assert!(s.auto_insert);
        assert_eq!(s.postpone, Postpone::from_letters("F"));
    }

    #[test]
    fn conversion_cas_limites() {
        let doc = sample_document();
        let base = LegacyScheduleEntry {
            nextdate: (2023, 2, 28),
            tiers: "X".into(),
            ..LegacyScheduleEntry::default()
        };
        // Fin de mois en mensuel : reste en fin de mois, comme l'original.
        let s = schedule_from_legacy(&doc, &base).unwrap();
        assert_eq!(s.anchor_day, Some(31));
        assert_eq!(s.following(s.next_date), Some(d(2023, 3, 31)));
        // Compte inconnu, transfert vers un compte inconnu, mode hors liste.
        let e = LegacyScheduleEntry {
            account: 7,
            transfer: 1,
            mode: 12,
            montant: 15.5,
            categ: -1,
            delta: 0,
            deltaunit: 0,
            enddate: (2024, 2, 31),
            shift: Some("SD".into()),
            ..base.clone()
        };
        let s = schedule_from_legacy(&doc, &e).unwrap();
        assert_eq!(s.account, "");
        assert_eq!(s.transfer.as_deref(), Some("uid-livret"));
        assert_eq!(s.mode, Mode::Standard(StandardMode::VirementRecu));
        assert_eq!(s.category, None);
        assert_eq!(s.period.every, 0);
        assert_eq!(s.period.unit, PeriodUnit::Days);
        assert_eq!(s.anchor_day, None);
        assert_eq!(s.end_date, Some(d(2024, 2, 29)));
        assert_eq!(s.postpone, Postpone::from_letters("SD"));
        assert_eq!(s.amount, Amount(155_000));
        // Prochaine échéance inutilisable : écartée avec un avertissement.
        let bad = LegacyScheduleEntry {
            nextdate: (0, 0, 0),
            ..base
        };
        let (schedules, warnings) = schedules_from_legacy(&doc, &[bad]);
        assert!(schedules.is_empty());
        assert_eq!(warnings.len(), 1);
    }

    // --- Insertion ---

    #[test]
    fn insertion_mensuelle_fin_de_mois() {
        let mut doc = sample_document();
        let mut schedules = vec![Schedule {
            category: Some(13),
            details: "janvier".into(),
            ..monthly(d(2024, 1, 31), Some(31))
        }];
        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 4, 15),
            Selection::All,
            false,
        );
        assert!(report.refused.is_empty());
        let ops = &doc.accounts[0].operations;
        let dates: Vec<Date> = ops.iter().map(|o| o.date).collect();
        assert_eq!(dates, vec![d(2024, 1, 31), d(2024, 2, 29), d(2024, 3, 31)]);
        assert!(ops.iter().all(|o| o.payee == "[auto] Loyer"));
        assert!(ops.iter().all(|o| o.amount == Amount::from_cents(-65_000)));
        assert!(
            ops.iter()
                .all(|o| o.tack == Tack::None && o.mark_date == o.date)
        );
        assert!(
            ops.iter()
                .all(|o| o.category == Some(13) && o.details == "janvier")
        );
        assert_eq!(ops.iter().map(|o| o.uid).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(schedules[0].next_date, d(2024, 4, 30));
        assert_eq!(report.inserted.len(), 3);
        // Rien de plus à insérer à la même date.
        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 4, 15),
            Selection::All,
            false,
        );
        assert!(report.is_empty());
    }

    #[test]
    fn insertion_signe_categorie_et_report() {
        let mut doc = sample_document();
        doc.accounts[0]
            .operations
            .push(op(41, d(2024, 1, 1), Tack::None, "x", 100));
        let mut schedules = vec![Schedule {
            // Montant positif mais mode de dépense : le signe suit le mode.
            amount: Amount::from_cents(2_000),
            mode: Mode::Custom("Espèces".into()),
            category: Some(99), // catégorie inexistante → retirée
            postpone: Postpone::from_letters("SDF"),
            period: Period {
                every: 1,
                unit: PeriodUnit::Years,
            },
            ..monthly(d(2027, 12, 25), Some(25))
        }];
        insert_due(
            &mut doc,
            &mut schedules,
            d(2028, 1, 1),
            Selection::All,
            false,
        );
        let new = &doc.accounts[0].operations[1];
        assert_eq!(new.uid, 42);
        assert_eq!(new.date, d(2027, 12, 27));
        assert_eq!(new.amount, Amount::from_cents(-2_000));
        assert_eq!(new.category, None);
        assert_eq!(new.mode, Mode::Custom("Espèces".into()));
        // La prochaine échéance part de la date théorique, pas de la date reportée.
        assert_eq!(schedules[0].next_date, d(2028, 12, 25));
    }

    #[test]
    fn insertion_transfert() {
        let mut doc = sample_document();
        let mut schedules = vec![Schedule {
            transfer: Some("uid-livret".into()),
            mode: Mode::Standard(StandardMode::VirementEmis),
            amount: Amount::from_cents(-10_000),
            details: "épargne".into(),
            ..monthly(d(2024, 5, 5), Some(5))
        }];
        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 5, 31),
            Selection::All,
            false,
        );
        assert_eq!(report.inserted.len(), 2);
        let a = &doc.accounts[0].operations[0];
        let b = &doc.accounts[1].operations[0];
        assert_eq!(a.payee, "Transfert vers le compte Livret");
        assert_eq!(b.payee, "Transfert depuis le compte Courant");
        assert_eq!(a.mode, Mode::Standard(StandardMode::VirementEmis));
        assert_eq!(b.mode, Mode::Standard(StandardMode::VirementRecu));
        assert_eq!(a.amount, Amount::from_cents(-10_000));
        assert_eq!(b.amount, Amount::from_cents(10_000));
        assert_eq!(a.details, "épargne");
        assert_eq!(a.extra("#sisterAccUid"), Some("uid-livret"));
        assert_eq!(b.extra("#sisterAccUid"), Some("uid-courant"));
        assert_eq!(a.extra("#sisterLineUid"), Some(b.uid.to_string().as_str()));
        assert_eq!(b.extra("#sisterLineUid"), Some(a.uid.to_string().as_str()));
        assert_ne!(a.uid, b.uid);

        // Transfert en recette : sens inversés.
        let mut doc = sample_document();
        let mut schedules = vec![Schedule {
            transfer: Some("uid-livret".into()),
            mode: Mode::Standard(StandardMode::Versement),
            amount: Amount::from_cents(500),
            ..monthly(d(2024, 5, 5), Some(5))
        }];
        insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 5, 5),
            Selection::All,
            false,
        );
        let a = &doc.accounts[0].operations[0];
        let b = &doc.accounts[1].operations[0];
        assert_eq!(a.payee, "Transfert depuis le compte Livret");
        assert_eq!(b.payee, "Transfert vers le compte Courant");
        assert_eq!(b.mode, Mode::Standard(StandardMode::RetraitDab));
        assert_eq!(
            (a.amount, b.amount),
            (Amount::from_cents(500), Amount::from_cents(-500))
        );
    }

    #[test]
    fn modes_reciproques() {
        let doc = sample_document();
        use StandardMode::*;
        let pairs = [
            (Carte, VirementRecu),
            (RetraitDab, Versement),
            (ChequeEmis, DepotCheque),
            (Prelevement, VirementRecu),
            (VirementEmis, VirementRecu),
            (Tip, VirementRecu),
            (Versement, RetraitDab),
            (DepotCheque, ChequeEmis),
            (VirementRecu, Prelevement),
        ];
        for (from, to) in pairs {
            assert_eq!(
                reciprocal_mode(&doc, &Mode::Standard(from)),
                Mode::Standard(to)
            );
        }
    }

    #[test]
    fn selection_lignes_non_automatiques() {
        // L'original testait la clé inexistante `auto_insert` : rien n'était choisi.
        let mut doc = sample_document();
        let mut schedules = vec![
            Schedule {
                payee: "auto".into(),
                ..monthly(d(2024, 1, 10), Some(10))
            },
            Schedule {
                payee: "manuelle".into(),
                auto_insert: false,
                ..monthly(d(2024, 1, 10), Some(10))
            },
        ];
        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 1, 31),
            Selection::NotAutomatic,
            false,
        );
        assert_eq!(report.inserted.len(), 1);
        assert_eq!(doc.accounts[0].operations[0].payee, "[auto] manuelle");
        assert_eq!(schedules[0].next_date, d(2024, 1, 10));
        assert_eq!(schedules[1].next_date, d(2024, 2, 10));

        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 1, 31),
            Selection::One(0),
            false,
        );
        assert_eq!(report.inserted.len(), 1);
        assert_eq!(schedules[0].next_date, d(2024, 2, 10));
        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 1, 31),
            Selection::One(9),
            false,
        );
        assert!(report.is_empty());
    }

    #[test]
    fn simulation_et_date_de_fin() {
        let mut doc = sample_document();
        let mut schedules = vec![Schedule {
            end_date: Some(d(2024, 3, 10)),
            ..monthly(d(2024, 1, 10), Some(10))
        }];
        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 12, 31),
            Selection::All,
            true,
        );
        assert_eq!(report.inserted.len(), 3);
        assert_eq!(schedules[0].next_date, d(2024, 1, 10));
        assert!(!schedules[0].is_finished());
        insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 12, 31),
            Selection::All,
            false,
        );
        assert_eq!(schedules[0].next_date, d(2024, 4, 10));
        assert!(schedules[0].is_finished());
        assert_eq!(doc.accounts[0].operations.len(), 6);
    }

    #[test]
    fn refus() {
        let mut doc = sample_document();
        let mut schedules = vec![
            Schedule {
                account: "disparu".into(),
                ..monthly(d(2024, 1, 10), None)
            },
            Schedule {
                transfer: Some("disparu".into()),
                ..monthly(d(2024, 1, 10), None)
            },
            Schedule {
                transfer: Some("uid-courant".into()),
                ..monthly(d(2024, 1, 10), None)
            },
            Schedule {
                period: Period {
                    every: 0,
                    unit: PeriodUnit::Days,
                },
                ..monthly(d(2024, 1, 10), None)
            },
        ];
        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 2, 1),
            Selection::All,
            false,
        );
        assert!(report.inserted.is_empty());
        let reasons: Vec<_> = report.refused.iter().map(|r| r.reason).collect();
        assert_eq!(
            reasons,
            vec![
                RefusalReason::UnknownAccount,
                RefusalReason::UnknownTransferAccount,
                RefusalReason::SameAccount,
                RefusalReason::NullPeriod
            ]
        );
        assert!(schedules.iter().all(|s| s.next_date == d(2024, 1, 10)));
        let text = report.summary(&doc, &schedules);
        assert!(text.contains("Lignes refusées"));
        assert!(text.contains("compte inexistant"));
        assert!(text.contains("intervalle nul"));
    }

    #[test]
    fn insertion_automatique_a_l_ouverture() {
        let mut doc = sample_document();
        // Ancienne donnée : l'échéance d'Exemple.bp (annuelle depuis le 17/04/2009).
        doc.feature_data.push(FeatureData {
            key: LEGACY_KEY.into(),
            value: legacy_pickle::tests::EXEMPLE_ECHEANCIER.to_vec(),
        });
        let report = auto_insert(&mut doc, d(2012, 1, 1)).unwrap();
        let ops = &doc.accounts[0].operations;
        assert_eq!(
            ops.iter().map(|o| o.date).collect::<Vec<_>>(),
            vec![d(2009, 4, 17), d(2010, 4, 17), d(2011, 4, 17)]
        );
        assert!(ops.iter().all(|o| o.payee == "[auto] Coheris"));
        assert!(ops.iter().all(|o| o.amount == Amount(-1_230_000)));
        assert!(ops.iter().all(|o| o.category == Some(13)));
        assert!(
            ops.iter()
                .all(|o| o.mode == Mode::Standard(StandardMode::Prelevement))
        );
        assert_eq!(report.inserted.len(), 3);
        let summary = report.summary(&doc, &[]);
        assert!(summary.contains(
            "Compte Courant : échéance \"[auto] Coheris\" du 17/04/2009 (montant : -123,00)"
        ));

        // La nouvelle clé est écrite, l'ancienne conservée.
        let loaded = load(&doc).unwrap();
        assert_eq!(loaded.source, Source::V6);
        assert_eq!(loaded.schedules[0].next_date, d(2012, 4, 17));
        assert!(legacy_pickle::feature_bytes(&doc, LEGACY_KEY).is_some());

        // Seconde ouverture le même jour : rien à faire.
        let report = auto_insert(&mut doc, d(2012, 1, 1)).unwrap();
        assert!(report.is_empty());
        assert_eq!(doc.accounts[0].operations.len(), 3);
    }

    #[test]
    fn insertion_automatique_ignore_les_echeances_manuelles() {
        let mut doc = sample_document();
        save(
            &mut doc,
            &[Schedule {
                auto_insert: false,
                ..monthly(d(2024, 1, 10), None)
            }],
        )
        .unwrap();
        let report = auto_insert(&mut doc, d(2024, 6, 1)).unwrap();
        assert!(report.is_empty());
        assert!(doc.accounts[0].operations.is_empty());
    }

    // --- Prévisionnel ---

    #[test]
    fn solde_previsionnel() {
        let mut doc = sample_document();
        let today = d(2024, 6, 15);
        doc.accounts[0].operations = vec![
            op(1, d(2024, 6, 1), Tack::Pointed, "Salaire", 200_000),
            op(2, d(2024, 6, 10), Tack::None, "Courses", -10_000),
            // Pointée mais future : dans le solde actuel.
            op(3, d(2024, 6, 20), Tack::Reconciled, "Avance", 5_000),
            op(4, d(2024, 7, 2), Tack::None, "Assurance", -3_000),
            // Au-delà de la date choisie : ignorée.
            op(5, d(2024, 12, 1), Tack::None, "Lointain", -1_000),
        ];
        let schedules = vec![
            monthly(d(2024, 7, 1), Some(1)),
            // Transfert du livret vers le compte courant.
            Schedule {
                account: "uid-livret".into(),
                transfer: Some("uid-courant".into()),
                mode: Mode::Standard(StandardMode::VirementEmis),
                amount: Amount::from_cents(-20_000),
                ..monthly(d(2024, 7, 5), Some(5))
            },
            // Échéance d'un autre compte : ignorée.
            Schedule {
                account: "uid-livret".into(),
                ..monthly(d(2024, 7, 1), Some(1))
            },
        ];
        let lines = forecast(&doc, &schedules, 0, d(2024, 8, 3), today, false);
        let summary: Vec<_> = lines
            .iter()
            .map(|l| {
                (
                    l.date,
                    l.payee.as_str(),
                    l.amount.0 / 100,
                    l.balance.0 / 100,
                    l.kind,
                )
            })
            .collect();
        use ForecastKind::{CurrentBalance, Pending};
        assert_eq!(
            summary,
            vec![
                (
                    d(2024, 6, 20),
                    "Solde actuel",
                    195_000,
                    195_000,
                    CurrentBalance
                ),
                (
                    d(2024, 7, 1),
                    "Loyer",
                    -65_000,
                    130_000,
                    ForecastKind::Schedule
                ),
                (d(2024, 7, 2), "Assurance", -3_000, 127_000, Pending),
                (
                    d(2024, 7, 5),
                    "Transfert depuis le compte Livret",
                    20_000,
                    147_000,
                    ForecastKind::Schedule
                ),
                (
                    d(2024, 8, 1),
                    "Loyer",
                    -65_000,
                    82_000,
                    ForecastKind::Schedule
                ),
            ]
        );
        assert_eq!(lines[0].progress, 1000);
        assert_eq!(lines[4].progress, 82_000 * 1000 / 195_000);
        assert_eq!(
            forecast_balance(&doc, &schedules, 0, d(2024, 8, 3), today),
            Amount::from_cents(82_000)
        );
        assert_eq!(
            forecast_balance(&doc, &schedules, 0, d(2024, 6, 30), today),
            Amount::from_cents(195_000)
        );

        let monthly_lines = forecast(&doc, &schedules, 0, d(2024, 8, 3), today, true);
        let summary: Vec<_> = monthly_lines
            .iter()
            .skip(1)
            .map(|l| (l.date, l.payee.as_str(), l.amount.0 / 100, l.count))
            .collect();
        assert_eq!(
            summary,
            vec![
                (
                    d(2024, 7, 31),
                    "Transfert depuis le compte Livret",
                    -48_000,
                    3
                ),
                (d(2024, 8, 31), "Loyer", -65_000, 1),
            ]
        );
        assert_eq!(monthly_lines[2].balance, Amount::from_cents(82_000));
        assert!(forecast(&doc, &schedules, 9, today, today, false).is_empty());
    }

    #[test]
    fn previsionnel_vide_sans_division_par_zero() {
        let doc = sample_document();
        let today = d(2024, 6, 15);
        let lines = forecast(&doc, &[], 0, d(2024, 12, 31), today, false);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].date, today);
        assert_eq!(lines[0].balance, Amount::ZERO);
        assert_eq!(lines[0].progress, 0);
    }

    // --- Création depuis une opération ---

    #[test]
    fn echeance_depuis_une_operation() {
        let doc = sample_document();
        let mut operation = op(7, d(2024, 1, 31), Tack::Pointed, "EDF", -4_500);
        operation.mode = Mode::Standard(StandardMode::Prelevement);
        operation.category = Some(0);
        operation.details = "contrat".into();
        let s = schedule_from_operation(&doc, 1, &operation).unwrap();
        assert_eq!(s.account, "uid-livret");
        assert_eq!(s.next_date, d(2024, 2, 29));
        assert_eq!(s.following(s.next_date), Some(d(2024, 3, 31)));
        assert_eq!(s.payee, "EDF");
        assert_eq!(s.details, "contrat");
        assert_eq!(s.category, Some(0));
        assert_eq!(s.amount, Amount::from_cents(-4_500));
        assert_eq!(s.mode, Mode::Standard(StandardMode::Prelevement));
        assert!(s.auto_insert);
        assert_eq!(s.end_date, None);
        assert_eq!(s.transfer, None);
        assert_eq!(s.period, Period::default());
        assert!(schedule_from_operation(&doc, 5, &operation).is_none());
    }

    // --- Stockage ---

    #[test]
    fn stockage_json() {
        let mut doc = sample_document();
        let schedules = vec![
            Schedule {
                transfer: Some("uid-livret".into()),
                end_date: Some(d(2025, 1, 1)),
                postpone: Postpone::from_letters("SF"),
                mode: Mode::Custom("Espèces".into()),
                category: Some(13),
                ..monthly(d(2024, 1, 31), Some(31))
            },
            Schedule::default(),
        ];
        save(&mut doc, &schedules).unwrap();
        save(&mut doc, &schedules).unwrap();
        assert_eq!(
            doc.feature_data
                .iter()
                .filter(|f| f.key == FEATURE_KEY)
                .count(),
            1
        );
        let loaded = load(&doc).unwrap();
        assert_eq!(loaded.source, Source::V6);
        assert_eq!(loaded.schedules, schedules);

        // Champs manquants → défauts.
        let partial = from_json(br#"{"version":1,"schedules":[{"payee":"X"}]}"#).unwrap();
        assert_eq!(partial[0].payee, "X");
        assert!(partial[0].auto_insert);
        assert_eq!(partial[0].period, Period::default());
        // Version future ou JSON corrompu : erreur.
        assert!(from_json(br#"{"version":99}"#).is_err());
        assert!(from_json(b"{").is_err());
    }

    #[test]
    fn priorite_et_erreurs_de_chargement() {
        let mut doc = sample_document();
        assert_eq!(load(&doc).unwrap().source, Source::Empty);

        // L'ancienne clé `echeancier` n'est pas relue.
        doc.feature_data.push(FeatureData {
            key: LEGACY_OLD_KEY.into(),
            value: legacy_pickle::tests::EXEMPLE_ECHEANCIER.to_vec(),
        });
        assert_eq!(load(&doc).unwrap().source, Source::Empty);

        // `fabiobpscheduler` vide d'Exemple.bp.
        doc.feature_data.push(FeatureData {
            key: LEGACY_KEY.into(),
            value: b"(lp1\n.".to_vec(),
        });
        let loaded = load(&doc).unwrap();
        assert_eq!(loaded.source, Source::Legacy);
        assert!(loaded.schedules.is_empty());

        // Ancienne donnée illisible : liste vide et avertissement.
        doc.feature_data.last_mut().unwrap().value = b"(lp1\nZ".to_vec();
        let loaded = load(&doc).unwrap();
        assert!(loaded.schedules.is_empty());
        assert_eq!(loaded.warnings.len(), 1);

        // La nouvelle clé l'emporte, même vide.
        save(&mut doc, &[]).unwrap();
        let loaded = load(&doc).unwrap();
        assert_eq!(loaded.source, Source::V6);

        // Nouvelle donnée corrompue : erreur, pas de repli sur l'ancienne.
        doc.feature_data
            .iter_mut()
            .find(|f| f.key == FEATURE_KEY)
            .unwrap()
            .value = b"pas du json".to_vec();
        assert!(load(&doc).is_err());
        assert!(auto_insert(&mut doc, d(2024, 1, 1)).is_err());
    }

    #[test]
    fn dates_hors_plage_sans_panique() {
        let mut doc = sample_document();
        let mut schedules = from_json(
            br#"{"version":1,"schedules":[{"account":"uid-courant","next_date":2147483647},
                {"account":"inconnu","next_date":-2147483648,"period":{"every":1,"unit":"Days"}}]}"#,
        )
        .unwrap();
        let report = insert_due(
            &mut doc,
            &mut schedules,
            d(2024, 1, 1),
            Selection::All,
            false,
        );
        assert!(report.inserted.is_empty());
        assert!(report.summary(&doc, &schedules).contains("??/??/????"));
        assert!(!is_french_holiday(Date(i32::MAX)));
        assert_eq!(
            postponed_date(Date(i32::MIN), Postpone::from_letters("SDF")),
            Date(i32::MIN)
        );
        let mut operation = op(1, Date(i32::MAX), Tack::None, "x", 1);
        assert!(schedule_from_operation(&doc, 0, &operation).is_none());
        operation.date = d(2024, 1, 1);
        doc.accounts[0].operations.push(Operation {
            date: Date(i32::MAX),
            ..operation
        });
        let lines = forecast(&doc, &schedules, 0, Date(i32::MAX), d(2024, 1, 1), true);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn occurrences_bornees() {
        let s = Schedule {
            period: Period {
                every: 1,
                unit: PeriodUnit::Days,
            },
            ..monthly(d(1900, 1, 1), None)
        };
        let occ = s.occurrences(d(9999, 12, 31));
        assert_eq!(occ.len(), MAX_OCCURRENCES);
        let s = monthly(d(9999, 12, 15), Some(15));
        assert_eq!(s.occurrences(d(9999, 12, 31)).len(), 1);
    }

    #[test]
    fn schedule_from_inserted_operation_drops_auto_prefix() {
        let doc = crate::manage::new_document("Test", Amount::ZERO);
        let mut op = doc.accounts[0].operations[0].clone();
        op.payee = format!("{AUTO_PREFIX}Loyer");
        let schedule = schedule_from_operation(&doc, 0, &op).unwrap();
        assert_eq!(schedule.payee, "Loyer");
    }
}
