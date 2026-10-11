//! Document ouvert : données, vue filtrée/triée du compte courant, sélection.

use std::collections::BTreeSet;
use std::path::PathBuf;

use bp_core::FileFormat;
use bp_core::filter::{Filters, SearchIndex};
use bp_core::format_v6::Protection;
use bp_core::model::{Amount, Date, Document, Operation, Tack};
use bp_core::view::{SortColumn, running_balances, sorted_order};

pub struct OpenDoc {
    pub path: PathBuf,
    pub document: Document,
    pub format: FileFormat,
    pub protection: Option<Protection>,
    pub legacy_password: Option<String>,
    pub dirty: bool,
    pub warnings: Vec<String>,

    /// Compte affiché (index réel dans `document.accounts`).
    pub account: usize,
    pub filters: Filters,
    pub sort: SortColumn,
    pub ascending: bool,
    pub two_lines: bool,
    /// Opérations sélectionnées (indices dans le compte courant).
    pub selection: BTreeSet<usize>,
    /// Dernière ligne cliquée, pour les sélections avec Maj.
    pub anchor: Option<usize>,
    pub scroll_to: Option<usize>,

    view: View,
    /// Incrémenté à chaque modification des opérations.
    revision: u64,
}

/// Vue calculée du compte courant. L'index de recherche, le tri et les soldes
/// ne sont recalculés que si le compte, le tri ou les données changent ;
/// changer un filtre ne fait que refiltrer (quelques millisecondes).
#[derive(Default)]
struct View {
    key: Option<(usize, SortColumn, bool, u64)>,
    filters: Option<(Filters, Date)>,
    /// Compte pour lequel l'index de recherche est à jour.
    indexed_account: Option<usize>,
    index: SearchIndex,
    order: Vec<usize>,
    /// Solde progressif, indexé par opération.
    balance_of: Vec<Amount>,
    visible: Vec<usize>,
}

/// Totaux du panneau des soldes.
#[derive(Default, Clone, Copy)]
pub struct Totals {
    pub balance: Amount,
    pub pointed: Amount,
    pub pending: Amount,
    pub pending_count: usize,
    pub reconciled: Amount,
    pub last_reconciled: Option<Date>,
    /// Solde au jour près 30 jours plus tôt, pour la tendance.
    pub balance_30_days_ago: Amount,
    pub month_income: Amount,
    pub month_expenses: Amount,
}

impl OpenDoc {
    pub fn new(path: PathBuf, opened: bp_core::file::OpenedFile) -> Self {
        let account = opened.document.default_account.min(opened.document.accounts.len().saturating_sub(1));
        let mut doc = Self {
            path,
            format: opened.format,
            protection: opened.protection,
            legacy_password: opened.legacy_password,
            warnings: opened.warnings,
            document: opened.document,
            dirty: false,
            account,
            filters: Filters::default(),
            sort: SortColumn::Date,
            ascending: false,
            two_lines: false,
            selection: BTreeSet::new(),
            anchor: None,
            scroll_to: None,
            view: View::default(),
            revision: 0,
        };
        doc.refresh();
        doc
    }

    pub fn file_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// Met la vue à jour si nécessaire. Appelé à chaque image : ne coûte rien
    /// quand rien n'a changé.
    pub fn refresh(&mut self) {
        let key = (self.account, self.sort, self.ascending, self.revision);
        if self.view.key != Some(key) {
            let account = &self.document.accounts[self.account];
            if self.view.indexed_account != Some(self.account) {
                self.view.index = SearchIndex::build(&self.document, account);
                self.view.indexed_account = Some(self.account);
            }
            self.view.order = sorted_order(&self.document, account, self.sort, self.ascending);
            let balances = running_balances(account, &self.view.order, self.ascending);
            self.view.balance_of = vec![Amount::ZERO; account.operations.len()];
            for (pos, &op) in self.view.order.iter().enumerate() {
                self.view.balance_of[op] = balances[pos];
            }
            self.view.key = Some(key);
            self.view.filters = None;
        }
        let today = Date::today();
        if self.view.filters.as_ref() != Some(&(self.filters.clone(), today)) {
            let account = &self.document.accounts[self.account];
            self.view.visible = self.filters.compile(today).apply(account, &self.view.index, &self.view.order);
            self.view.filters = Some((self.filters.clone(), today));
        }
    }

    pub fn visible(&self) -> &[usize] {
        &self.view.visible
    }

    pub fn total_count(&self) -> usize {
        self.view.order.len()
    }

    pub fn balance_of(&self, op: usize) -> Amount {
        self.view.balance_of[op]
    }

    pub fn current_account(&self) -> &bp_core::model::Account {
        &self.document.accounts[self.account]
    }

    pub fn operation(&self, index: usize) -> &Operation {
        &self.current_account().operations[index]
    }

    /// Soldes calculés sur les lignes affichées (comportement par défaut de
    /// l'ancienne version).
    pub fn totals(&self) -> Totals {
        let ops = &self.current_account().operations;
        let today = Date::today();
        let month_ago = Date(today.0 - 30);
        let first_of_month = {
            let d = today.to_naive();
            Date::from_ymd(chrono::Datelike::year(&d), chrono::Datelike::month(&d), 1).unwrap_or(today)
        };
        let mut totals = Totals::default();
        for &i in &self.view.visible {
            let op = &ops[i];
            totals.balance = totals.balance + op.amount;
            if op.date <= month_ago {
                totals.balance_30_days_ago = totals.balance_30_days_ago + op.amount;
            }
            if op.date >= first_of_month && op.date <= today {
                if op.amount.is_negative() {
                    totals.month_expenses = totals.month_expenses + op.amount;
                } else {
                    totals.month_income = totals.month_income + op.amount;
                }
            }
            match op.tack {
                Tack::None => {
                    totals.pending = totals.pending + op.amount;
                    totals.pending_count += 1;
                }
                Tack::Pointed => totals.pointed = totals.pointed + op.amount,
                Tack::Reconciled => {
                    totals.pointed = totals.pointed + op.amount;
                    totals.reconciled = totals.reconciled + op.amount;
                }
            }
        }
        totals.last_reconciled = ops.iter().filter(|op| op.tack == Tack::Reconciled).map(|op| op.date).max();
        totals
    }

    pub fn select_account(&mut self, account: usize) {
        if account != self.account && account < self.document.accounts.len() {
            self.account = account;
            self.selection.clear();
            self.anchor = None;
            self.refresh();
        }
    }

    /// Compte suivant/précédent dans l'ordre d'affichage, en sautant les comptes désactivés.
    pub fn cycle_account(&mut self, forward: bool) {
        let order: Vec<usize> = self
            .document
            .account_order
            .iter()
            .copied()
            .filter(|&i| !self.document.accounts[i].disabled || i == self.account)
            .collect();
        let Some(pos) = order.iter().position(|&i| i == self.account) else { return };
        let next = if forward { (pos + 1) % order.len() } else { (pos + order.len() - 1) % order.len() };
        self.select_account(order[next]);
    }

    pub fn click_row(&mut self, op: usize, ctrl: bool, shift: bool) {
        if shift && let Some(anchor) = self.anchor {
            let visible = &self.view.visible;
            if let (Some(a), Some(b)) =
                (visible.iter().position(|&i| i == anchor), visible.iter().position(|&i| i == op))
            {
                if !ctrl {
                    self.selection.clear();
                }
                self.selection.extend(&visible[a.min(b)..=a.max(b)]);
                return;
            }
        }
        if ctrl {
            if !self.selection.remove(&op) {
                self.selection.insert(op);
            }
        } else {
            self.selection.clear();
            self.selection.insert(op);
        }
        self.anchor = Some(op);
    }

    /// Déplace la sélection d'une ligne vers le haut ou le bas.
    pub fn move_selection(&mut self, delta: isize) {
        let visible = &self.view.visible;
        if visible.is_empty() {
            return;
        }
        let current = self.anchor.and_then(|a| visible.iter().position(|&i| i == a));
        let pos = match current {
            Some(p) => (p as isize + delta).clamp(0, visible.len() as isize - 1) as usize,
            None => 0,
        };
        let op = visible[pos];
        self.selection.clear();
        self.selection.insert(op);
        self.anchor = Some(op);
        self.scroll_to = Some(op);
    }

    /// Pointage (MARK-01) : non pointé → pointé → rapproché → non pointé. Seules
    /// les lignes dans le même état que la ligne de référence changent.
    pub fn cycle_tack(&mut self) {
        let Some(&reference) = self.selection.iter().next() else { return };
        let ops = &mut self.document.accounts[self.account].operations;
        let from = ops[reference].tack;
        let to = match from {
            Tack::None => Tack::Pointed,
            Tack::Pointed => Tack::Reconciled,
            Tack::Reconciled => Tack::None,
        };
        let today = Date::today();
        for &i in &self.selection {
            if ops[i].tack == from {
                ops[i].tack = to;
                ops[i].mark_date = today;
            }
        }
        self.touch_tacks();
    }

    /// Rapprocher toutes les lignes pointées affichées (MARK-03).
    pub fn reconcile_all_pointed(&mut self) {
        let visible = self.view.visible.clone();
        let ops = &mut self.document.accounts[self.account].operations;
        for i in visible {
            if ops[i].tack == Tack::Pointed {
                ops[i].tack = Tack::Reconciled;
            }
        }
        self.touch_tacks();
    }

    /// Ajoute ou remplace une opération et la sélectionne.
    pub fn upsert_operation(&mut self, target: Option<usize>, mut op: Operation) {
        let index = match target {
            Some(i) => {
                op.uid = self.current_account().operations[i].uid;
                self.document.accounts[self.account].operations[i] = op;
                i
            }
            None => {
                op.uid = self.document.next_operation_uid();
                let ops = &mut self.document.accounts[self.account].operations;
                ops.push(op);
                ops.len() - 1
            }
        };
        let account = &self.document.accounts[self.account];
        self.view.index.update(&self.document, account, index);
        self.touch();
        self.refresh();
        self.selection.clear();
        self.selection.insert(index);
        self.anchor = Some(index);
        self.scroll_to = Some(index);
    }

    pub fn delete_selection(&mut self) {
        let ops = &mut self.document.accounts[self.account].operations;
        for &i in self.selection.iter().rev() {
            ops.remove(i);
        }
        self.selection.clear();
        self.anchor = None;
        self.view.indexed_account = None;
        self.touch();
    }

    /// Modification qui change le tri ou les soldes : tout est recalculé.
    fn touch(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }

    /// Seul le pointage a changé : ni l'index, ni le tri, ni le solde
    /// progressif ne bougent, il suffit de refiltrer.
    fn touch_tacks(&mut self) {
        self.view.filters = None;
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bp_core::model::{Account, Mode, StandardMode};

    fn op(uid: u64, day: i32, payee: &str, cents: i64, tack: Tack) -> Operation {
        Operation {
            uid,
            date: Date(45000 + day),
            mark_date: Date(45000 + day),
            tack,
            mode: Mode::Standard(StandardMode::Carte),
            payee: payee.into(),
            details: String::new(),
            category: None,
            amount: Amount::from_cents(cents),
            extra: Vec::new(),
        }
    }

    fn doc() -> OpenDoc {
        let account = Account {
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
            secondary_currency: "F".into(),
            secondary_rate: 6.55957,
            is_virtual: false,
            disabled: false,
            operations: vec![
                op(1, 1, "Carrefour", -1_000, Tack::Reconciled),
                op(2, 2, "EDF", -5_000, Tack::None),
                op(3, 3, "Salaire", 200_000, Tack::Pointed),
                op(4, 4, "Carrefour", -2_000, Tack::None),
            ],
        };
        let document = Document {
            group_name: "Test".into(),
            guid: String::new(),
            user_name: String::new(),
            custom_modes: Vec::new(),
            accounts: vec![account],
            account_order: vec![0],
            default_account: 0,
            categories: Vec::new(),
            payees: Vec::new(),
            feature_data: Vec::new(),
            unknown_header: Vec::new(),
        };
        OpenDoc::new(
            PathBuf::from("test.bp"),
            bp_core::file::OpenedFile {
                document,
                format: FileFormat::V6,
                protection: None,
                legacy_password: None,
                warnings: Vec::new(),
            },
        )
    }

    #[test]
    fn sorted_newest_first_with_running_balance() {
        let d = doc();
        assert_eq!(d.visible(), [3, 2, 1, 0]);
        assert_eq!(d.balance_of(3), Amount::from_cents(192_000));
        assert_eq!(d.balance_of(0), Amount::from_cents(-1_000));
    }

    #[test]
    fn filters_and_totals() {
        let mut d = doc();
        d.filters.payee = "carre".into();
        d.refresh();
        assert_eq!(d.visible(), [3, 0]);
        let totals = d.totals();
        assert_eq!(totals.balance, Amount::from_cents(-3_000));
        assert_eq!(totals.pending, Amount::from_cents(-2_000));
        assert_eq!(totals.reconciled, Amount::from_cents(-1_000));
    }

    #[test]
    fn tack_cycle_changes_only_lines_in_reference_state() {
        let mut d = doc();
        d.click_row(1, false, false);
        d.click_row(2, true, false);
        d.cycle_tack();
        let ops = &d.current_account().operations;
        assert_eq!((ops[1].tack, ops[2].tack), (Tack::Pointed, Tack::Pointed));
        assert_eq!(ops[1].mark_date, Date::today());
        assert!(d.dirty);
        d.filters.tack = bp_core::filter::TackFilter::NotPointed;
        d.refresh();
        assert_eq!(d.visible(), [3], "le filtre de pointage voit le changement");
    }

    #[test]
    fn add_and_delete_operations() {
        let mut d = doc();
        d.upsert_operation(None, op(0, 10, "Fnac", -3_000, Tack::None));
        assert_eq!(d.current_account().operations[4].uid, 5);
        assert_eq!(d.visible()[0], 4, "la plus récente en haut");
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [4]);
        d.filters.payee = "fnac".into();
        d.refresh();
        assert_eq!(d.visible(), [4], "l'index de recherche connaît la nouvelle ligne");

        d.delete_selection();
        d.refresh();
        assert!(d.visible().is_empty());
        assert_eq!(d.current_account().operations.len(), 4);
    }
}
