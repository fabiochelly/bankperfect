//! Ce qui se prévoit : budget du mois, échéancier, virements entre comptes.
//!
//! Le budget et l'échéancier se définissent sur ordinateur ; le téléphone les
//! consulte, insère une échéance quand elle tombe et crée des virements. Les
//! échéances à insertion automatique ne sont pas insérées ici à l'ouverture :
//! l'ordinateur le fait déjà, et les deux appareils inséreraient la même
//! occurrence.

use bp_core::budget::{BudgetKind, BudgetOptions, BudgetPeriod, MonthlyBudget};
use bp_core::edit::mode_is_debit;
use bp_core::model::{Amount, Date};
use bp_core::scheduler::{self, Selection};
use bp_core::split::{self, TransferKind, TransferRequest};
use chrono::Datelike;

use crate::{BpError, Session, money};

/// Jauge d'une catégorie budgétée en dépense.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BudgetLine {
    pub label: String,
    /// Dépensé ce mois-ci.
    pub spent: String,
    /// Prévu pour le mois.
    pub planned: String,
    /// Reste (ou dépassement, sans signe).
    pub left: String,
    /// Dépensé / prévu (peut dépasser 1).
    pub ratio: f64,
    pub over: bool,
}

/// Budget mensuel d'un mois : jauges des dépenses prévues.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BudgetStatus {
    pub lines: Vec<BudgetLine>,
    pub spent: String,
    pub planned: String,
    pub left: String,
    pub ratio: f64,
    pub over: bool,
    /// Part du mois écoulée (entre 0 et 1) : repère sur les jauges.
    pub elapsed: f64,
}

/// Une échéance de l'échéancier.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ScheduleItem {
    /// Position dans l'échéancier.
    pub index: u32,
    pub payee: String,
    pub account: String,
    /// Compte crédité si l'échéance est un virement, sinon vide.
    pub transfer_to: String,
    pub amount: String,
    pub negative: bool,
    /// Prochaine date (après report éventuel), `jj/mm/aaaa`.
    pub date: String,
    /// Jours avant la prochaine date (négatif : en retard).
    pub days: i32,
    /// `Tous les mois`.
    pub interval: String,
    /// Insérée d'office par BankPerfect sur ordinateur.
    pub automatic: bool,
}

/// Virement en cours de saisie.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TransferDraft {
    /// Index réels des comptes débité et crédité.
    pub from: u32,
    pub to: u32,
    /// `jj/mm/aaaa`.
    pub date: String,
    pub amount: String,
    pub details: String,
}

fn month_bounds(today: Date, offset: i32) -> (i32, u32, Date, Date) {
    let d = today.to_naive();
    let months = d.year() * 12 + d.month0() as i32 + offset;
    let (year, month) = (months.div_euclid(12), months.rem_euclid(12) as u32 + 1);
    let first = Date::from_ymd(year, month, 1).unwrap_or(today);
    let next = months + 1;
    let last = Date::from_ymd(next.div_euclid(12), next.rem_euclid(12) as u32 + 1, 1)
        .map_or(first, |n| Date(n.0 - 1));
    (year, month, first, last)
}

#[uniffi::export]
impl Session {
    /// Budget du mois (`offset` : 0 = mois en cours), `None` si le fichier
    /// n'a aucune dépense budgétée.
    pub fn budget(&self, offset: i32) -> Option<BudgetStatus> {
        let inner = self.lock();
        let document = &inner.document;
        let budget = MonthlyBudget::load(document, &bp_core::legacy_pickle::parse_py).ok()?;
        let today = Date::today();
        let (year, month, first, last) = month_bounds(today, offset);
        let mut options = BudgetOptions::new(today);
        options.period = BudgetPeriod::Month { year, month };
        options.show_unbudgeted = false;
        options.show_subcategories = false;
        let report = budget.report(document, &options, today);
        let currency = crate::dashboard::currency(document);
        let gauge = |spent: Amount, planned: Amount| {
            let ratio = if planned.0 > 0 {
                spent.0 as f64 / planned.0 as f64
            } else {
                0.0
            };
            let left = Amount((planned.0 - spent.0).abs());
            (ratio, spent.0 > planned.0, money(left, &currency))
        };
        let mut lines: Vec<(f64, BudgetLine)> = report
            .rows
            .iter()
            .filter(|r| r.kind == Some(BudgetKind::Expense))
            .map(|r| {
                // Dépenses négatives : un remboursement net compte pour zéro.
                let spent = Amount((-r.real.0).max(0));
                let planned = Amount(-r.budget.unwrap_or_default().0);
                let (ratio, over, left) = gauge(spent, planned);
                (
                    ratio,
                    BudgetLine {
                        label: r.label.clone(),
                        spent: money(spent, &currency),
                        planned: money(planned, &currency),
                        left,
                        ratio,
                        over,
                    },
                )
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        // Les plus entamées d'abord.
        lines.sort_by(|a, b| b.0.total_cmp(&a.0));
        let spent = Amount((-report.expenses.real.0).max(0));
        let planned = Amount(-report.expenses.budget.0);
        let (ratio, over, left) = gauge(spent, planned);
        let days = f64::from(last.0 - first.0 + 1);
        let elapsed = if today > last {
            1.0
        } else if today < first {
            0.0
        } else {
            f64::from(today.0 - first.0 + 1) / days
        };
        Some(BudgetStatus {
            lines: lines.into_iter().map(|(_, l)| l).collect(),
            spent: money(spent, &currency),
            planned: money(planned, &currency),
            left,
            ratio,
            over,
            elapsed,
        })
    }

    /// Échéances en cours, de la plus proche à la plus lointaine.
    pub fn schedules(&self) -> Vec<ScheduleItem> {
        let inner = self.lock();
        let document = &inner.document;
        let Ok(loaded) = scheduler::load(document) else {
            return Vec::new();
        };
        let today = Date::today();
        let name = |uid: &str| {
            document
                .accounts
                .iter()
                .find(|a| a.uid == uid)
                .map(|a| a.name.clone())
        };
        let mut items: Vec<(Date, ScheduleItem)> = loaded
            .schedules
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_finished() && s.period.every > 0)
            .filter_map(|(i, s)| {
                let account = name(&s.account)?;
                let date = scheduler::postponed_date(s.next_date, s.postpone);
                let amount = s.amount_on(s.next_date).abs();
                let amount = if mode_is_debit(document, &s.mode) {
                    -amount
                } else {
                    amount
                };
                let currency = document
                    .accounts
                    .iter()
                    .find(|a| a.uid == s.account)
                    .map(|a| a.currency.as_str())
                    .unwrap_or("");
                Some((
                    date,
                    ScheduleItem {
                        index: i as u32,
                        payee: s.payee.clone(),
                        account,
                        transfer_to: s.transfer.as_deref().and_then(name).unwrap_or_default(),
                        amount: money(amount, currency),
                        negative: amount.is_negative(),
                        date: date.to_string(),
                        days: date.0 - today.0,
                        interval: s.interval_label(),
                        automatic: s.auto_insert,
                    },
                ))
            })
            .collect();
        items.sort_by_key(|(d, _)| *d);
        items.into_iter().map(|(_, s)| s).collect()
    }

    /// Insère la prochaine occurrence de l'échéance `index` et avance
    /// l'échéance. Renvoie un message de confirmation.
    pub fn insert_schedule(&self, index: u32) -> Result<String, BpError> {
        let mut inner = self.lock();
        let mut schedules = scheduler::load(&inner.document)
            .map_err(|e| BpError::Failed {
                reason: e.to_string(),
            })?
            .schedules;
        let schedule = schedules
            .get(index as usize)
            .ok_or_else(|| BpError::invalid("Échéance introuvable (modifiée ailleurs ?)"))?;
        if schedule.is_finished() {
            return Err(BpError::invalid("Cette échéance est terminée"));
        }
        let until = schedule.next_date;
        let report = scheduler::insert_due(
            &mut inner.document,
            &mut schedules,
            until,
            Selection::One(index as usize),
            false,
        );
        if let Some(refusal) = report.refused.first() {
            return Err(BpError::invalid(format!(
                "Échéance refusée : {}",
                refusal.reason.label()
            )));
        }
        let Some(line) = report.inserted.first() else {
            return Err(BpError::invalid("Rien à insérer"));
        };
        let message = format!(
            "« {} » du {} saisie",
            schedules[index as usize].payee, line.date
        );
        scheduler::save(&mut inner.document, &schedules).map_err(|e| BpError::Failed {
            reason: e.to_string(),
        })?;
        inner.changed();
        Ok(message)
    }

    /// Virement d'un compte à l'autre : deux lignes liées, comme sur ordinateur.
    pub fn transfer(&self, draft: TransferDraft) -> Result<(), BpError> {
        let mut inner = self.lock();
        let from = inner.account(draft.from)?;
        let to = inner.account(draft.to)?;
        if from == to {
            return Err(BpError::invalid("Choisissez deux comptes différents"));
        }
        let date = Date::parse_fr(&draft.date, Date::today())
            .ok_or_else(|| BpError::invalid("Date invalide (jj/mm/aaaa)"))?;
        let amount = Amount::parse(draft.amount.trim())
            .filter(|a| a.0 != 0)
            .ok_or_else(|| BpError::invalid("Saisissez un montant"))?;
        let request = TransferRequest {
            date,
            kind: TransferKind::Transfer,
            details: draft.details.trim().to_string(),
            category: None,
            amount,
            from,
            to,
        };
        split::create_transfer(&mut inner.document, &request)
            .map_err(|e| BpError::invalid(e.to_string()))?;
        inner.changed();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bp_core::budget::{BudgetKind, MonthlyBudget};
    use bp_core::manage;
    use bp_core::model::{Amount, Date, Document};
    use bp_core::scheduler::{self, Schedule};

    use crate::tests::{file, sample};
    use crate::*;

    fn two_accounts() -> Document {
        let (mut document, _) = sample();
        manage::add_account(&mut document, "Livret", Amount::ZERO);
        document
    }

    fn open(document: &Document) -> Arc<Session> {
        Session::open(file(document, &Protection::none().unwrap()), None).unwrap()
    }

    #[test]
    fn transfer_creates_two_linked_lines() {
        let session = open(&two_accounts());
        let draft = TransferDraft {
            from: 0,
            to: 1,
            date: Date::today().to_string(),
            amount: "250".into(),
            details: "Épargne".into(),
        };
        session.transfer(draft.clone()).unwrap();
        let accounts = session.accounts();
        assert_eq!(accounts[0].balance, "-150,00\u{A0}€");
        assert_eq!(accounts[1].balance, "250,00\u{A0}€");
        assert!(session.is_dirty());
        let same = TransferDraft { to: 0, ..draft };
        assert!(session.transfer(same).is_err());
    }

    #[test]
    fn transfer_lines_stay_together() {
        let session = open(&two_accounts());
        session
            .transfer(TransferDraft {
                from: 0,
                to: 1,
                date: Date::today().to_string(),
                amount: "40".into(),
                details: String::new(),
            })
            .unwrap();
        let uid = session.lock().document.accounts[0]
            .operations
            .last()
            .unwrap()
            .uid;
        // Modifier une ligne reporte le montant sur l'autre.
        let mut draft = session.draft_of(uid).unwrap();
        draft.amount = "55".into();
        session.save_operation(0, Some(uid), draft).unwrap();
        assert_eq!(session.accounts()[1].balance, "55,00\u{A0}€");
        // Supprimer l'une supprime l'autre.
        session.delete_operation(uid).unwrap();
        assert_eq!(session.accounts()[1].balance, "0,00\u{A0}€");
        assert_eq!(session.accounts()[0].balance, "100,00\u{A0}€");
    }

    #[test]
    fn budget_gauges() {
        let (mut document, _) = sample();
        assert!(open(&document).budget(0).is_none());
        let parent = document.categories.iter().find(|c| !c.is_child).unwrap();
        let (category, name) = (parent.id, parent.name.trim().to_string());
        let mut budget = MonthlyBudget::default();
        budget.set(
            Some(category),
            BudgetKind::Expense,
            Some(Amount::from_cents(20_000)),
        );
        budget.save(&mut document).unwrap();
        let mut op = document.accounts[0].operations[0].clone();
        op.uid = document.next_operation_uid();
        op.date = Date::today();
        op.category = Some(category);
        op.amount = Amount::from_cents(-15_000);
        document.accounts[0].operations.push(op);
        let status = open(&document).budget(0).unwrap();
        assert_eq!(status.lines.len(), 1);
        assert_eq!(status.lines[0].label, name);
        assert!((status.lines[0].ratio - 0.75).abs() < 1e-9);
        assert_eq!(status.lines[0].left, "50,00\u{A0}€");
        assert!(!status.over);
        assert!(status.elapsed > 0.0 && status.elapsed <= 1.0);
    }

    #[test]
    fn schedule_insertion_advances_it() {
        let mut document = two_accounts();
        let today = Date::today();
        let schedule = Schedule {
            account: document.accounts[0].uid.clone(),
            payee: "Loyer".into(),
            amount: Amount::from_cents(70_000),
            next_date: Date(today.0 - 2),
            auto_insert: false,
            ..Schedule::default()
        };
        scheduler::save(&mut document, &[schedule]).unwrap();
        let session = open(&document);
        let items = session.schedules();
        assert_eq!(items.len(), 1);
        assert!(items[0].negative);
        assert_eq!(items[0].account, "Courant");
        let message = session.insert_schedule(0).unwrap();
        assert!(message.contains("Loyer"), "{message}");
        assert_eq!(session.accounts()[0].balance, "-600,00\u{A0}€");
        // L'échéance est passée au mois suivant.
        let next = &session.schedules()[0];
        assert!(next.days > 20, "{next:?}");
    }
}
