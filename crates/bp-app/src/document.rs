//! Document ouvert : données, vue filtrée/triée du compte courant, sélection.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

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
    /// Icône et couleur de chaque catégorie (par identifiant).
    pub category_styles: std::collections::HashMap<u32, crate::category_icons::CategoryStyle>,
    /// Lignes de la grille affichées à l'écran (positions dans `visible`),
    /// relevées à chaque image pour Début/Fin et Page préc./suiv.
    pub on_screen: Option<(usize, usize)>,
    /// Fenêtre de temps de la courbe du solde.
    pub chart_window: ChartWindow,
    /// Opérations sélectionnées (indices dans le compte courant).
    pub selection: BTreeSet<usize>,
    /// Dernière ligne cliquée, pour les sélections avec Maj.
    pub anchor: Option<usize>,
    /// Extrémité mobile d'une sélection étendue au clavier (Maj).
    pub cursor: Option<usize>,
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
    /// Partagé pour que la grille ne le recopie pas à chaque image.
    visible: Arc<[usize]>,
    /// Totaux des cartes, recalculés seulement quand la vue change.
    totals: Totals,
    /// Solde de chaque compte (barre latérale).
    account_balances: Vec<Amount>,
    insights: Insights,
    /// Compte et révision pour lesquels la courbe du solde est à jour.
    series_key: Option<(usize, u64)>,
}

/// Destination d'un déplacement clavier dans la grille.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorTarget {
    Up,
    Down,
    PageUp,
    PageDown,
    /// Première ligne affichée à l'écran (Début).
    ScreenTop,
    /// Dernière ligne affichée à l'écran (Fin).
    ScreenBottom,
    /// Début de la liste (Ctrl+Début).
    First,
    /// Fin de la liste (Ctrl+Fin).
    Last,
}

/// Fenêtre de temps affichée par la courbe du solde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChartWindow {
    Month,
    SixMonths,
    #[default]
    Year,
    FiveYears,
    All,
}

impl ChartWindow {
    pub const ALL: [ChartWindow; 5] = [
        Self::Month,
        Self::SixMonths,
        Self::Year,
        Self::FiveYears,
        Self::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Month => "1M",
            Self::SixMonths => "6M",
            Self::Year => "1A",
            Self::FiveYears => "5A",
            Self::All => "Tout",
        }
    }

    /// Nombre de jours affichés, `None` pour tout l'historique.
    pub fn days(self) -> Option<i32> {
        match self {
            Self::Month => Some(31),
            Self::SixMonths => Some(183),
            Self::Year => Some(366),
            Self::FiveYears => Some(5 * 366),
            Self::All => None,
        }
    }
}

/// Données des mini-graphiques du tableau de bord.
#[derive(Default, Clone)]
pub struct Insights {
    /// Solde en fin de journée, par date croissante (jours ayant des opérations).
    pub series: Arc<[(Date, Amount)]>,
    /// Non pointées, pointées, rapprochées : nombre et somme (tout le compte).
    pub tack_counts: [usize; 3],
    pub tack_sums: [Amount; 3],
    /// Solde pointé (pointées et rapprochées) en fin de journée, par date croissante.
    pub pointed_series: Arc<[(Date, Amount)]>,
    /// Non pointées par semaine (lundi), de 8 semaines avant à 4 après aujourd'hui.
    pub pending_weeks: Vec<(Date, Amount, usize)>,
    /// Non pointées datées après aujourd'hui : somme et nombre.
    pub future_pending: (Amount, usize),
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
}

impl OpenDoc {
    pub fn new(path: PathBuf, opened: bp_core::file::OpenedFile) -> Self {
        let account = opened
            .document
            .default_account
            .min(opened.document.accounts.len().saturating_sub(1));
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
            category_styles: std::collections::HashMap::new(),
            on_screen: None,
            chart_window: ChartWindow::default(),
            selection: BTreeSet::new(),
            anchor: None,
            cursor: None,
            scroll_to: None,
            view: View::default(),
            revision: 0,
        };
        doc.category_styles = crate::category_icons::styles(&doc.document);
        doc.refresh();
        doc
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
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
            self.view.account_balances =
                self.document.accounts.iter().map(|a| a.balance()).collect();
            if self.view.series_key != Some((self.account, self.revision)) {
                self.view.insights.series = balance_series(account);
                self.view.series_key = Some((self.account, self.revision));
            }
            self.view.key = Some(key);
            self.view.filters = None;
        }
        let today = Date::today();
        if self.view.filters.as_ref() != Some(&(self.filters.clone(), today)) {
            let account = &self.document.accounts[self.account];
            self.view.visible = self
                .filters
                .compile(today)
                .apply(account, &self.view.index, &self.view.order)
                .into();
            self.view.filters = Some((self.filters.clone(), today));
            self.view.totals = self.compute_totals();
            self.compute_tack_insights();
        }
    }

    pub fn insights(&self) -> &Insights {
        &self.view.insights
    }

    /// Répartition du pointage (tout le compte, sans tenir compte des filtres,
    /// pour que le graphique reste stable).
    fn compute_tack_insights(&mut self) {
        let ops = &self.document.accounts[self.account].operations;
        let today = Date::today();
        let monday = |d: Date| {
            Date(d.0 - chrono::Datelike::weekday(&d.to_naive()).num_days_from_monday() as i32)
        };
        let first_week = Date(monday(today).0 - 8 * 7);
        let mut weeks: Vec<(Date, Amount, usize)> = (0..13)
            .map(|k| (Date(first_week.0 + 7 * k), Amount::ZERO, 0))
            .collect();
        let mut counts = [0usize; 3];
        let mut sums = [Amount::ZERO; 3];
        let mut future = (Amount::ZERO, 0usize);
        let mut pointed: Vec<(Date, Amount)> = Vec::new();
        for op in ops {
            let k = match op.tack {
                Tack::None => 0,
                Tack::Pointed => 1,
                Tack::Reconciled => 2,
            };
            counts[k] += 1;
            sums[k] = sums[k] + op.amount;
            if k == 0 {
                let w = (op.date.0 - first_week.0).div_euclid(7);
                if (0..13).contains(&w) {
                    let slot = &mut weeks[w as usize];
                    slot.1 = slot.1 + op.amount;
                    slot.2 += 1;
                }
                if op.date > today {
                    future = (future.0 + op.amount, future.1 + 1);
                }
            } else {
                pointed.push((op.date, op.amount));
            }
        }
        pointed.sort_unstable_by_key(|(d, _)| *d);
        let mut series: Vec<(Date, Amount)> = Vec::new();
        let mut balance = Amount::ZERO;
        for (date, amount) in pointed {
            balance = balance + amount;
            match series.last_mut() {
                Some(last) if last.0 == date => last.1 = balance,
                _ => series.push((date, balance)),
            }
        }
        let insights = &mut self.view.insights;
        insights.tack_counts = counts;
        insights.tack_sums = sums;
        insights.pointed_series = series.into();
        insights.pending_weeks = weeks;
        insights.future_pending = future;
    }

    pub fn visible(&self) -> Arc<[usize]> {
        Arc::clone(&self.view.visible)
    }

    pub fn visible_len(&self) -> usize {
        self.view.visible.len()
    }

    /// Solde d'un compte (mis en cache, recalculé à chaque modification).
    pub fn account_balance(&self, account: usize) -> Amount {
        self.view
            .account_balances
            .get(account)
            .copied()
            .unwrap_or_default()
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
        self.view.totals
    }

    fn compute_totals(&self) -> Totals {
        let ops = &self.current_account().operations;
        let mut totals = Totals::default();
        for &i in self.view.visible.iter() {
            let op = &ops[i];
            totals.balance = totals.balance + op.amount;
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
        totals.last_reconciled = ops
            .iter()
            .filter(|op| op.tack == Tack::Reconciled)
            .map(|op| op.date)
            .max();
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
        let Some(pos) = order.iter().position(|&i| i == self.account) else {
            return;
        };
        let next = if forward {
            (pos + 1) % order.len()
        } else {
            (pos + order.len() - 1) % order.len()
        };
        self.select_account(order[next]);
    }

    pub fn click_row(&mut self, op: usize, ctrl: bool, shift: bool) {
        if shift && let Some(anchor) = self.anchor {
            let visible = Arc::clone(&self.view.visible);
            if let (Some(a), Some(b)) = (
                visible.iter().position(|&i| i == anchor),
                visible.iter().position(|&i| i == op),
            ) {
                if !ctrl {
                    self.selection.clear();
                }
                self.selection.extend(&visible[a.min(b)..=a.max(b)]);
                self.cursor = Some(op);
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
        self.cursor = Some(op);
    }

    /// Déplacement clavier dans la grille.
    pub fn move_cursor(&mut self, target: CursorTarget, extend: bool) {
        let visible = Arc::clone(&self.view.visible);
        if visible.is_empty() {
            return;
        }
        let last = visible.len() - 1;
        let current = self
            .cursor
            .or(self.anchor)
            .and_then(|c| visible.iter().position(|&i| i == c));
        let page = self
            .on_screen
            .map_or(10, |(a, b)| b.saturating_sub(a).max(1));
        let from = current.unwrap_or(0);
        let pos = match target {
            CursorTarget::Up => from.saturating_sub(1),
            CursorTarget::Down => (from + 1).min(last),
            CursorTarget::PageUp => from.saturating_sub(page),
            CursorTarget::PageDown => (from + page).min(last),
            CursorTarget::ScreenTop => self.on_screen.map_or(0, |(a, _)| a.min(last)),
            CursorTarget::ScreenBottom => self.on_screen.map_or(last, |(_, b)| b.min(last)),
            CursorTarget::First => 0,
            CursorTarget::Last => last,
        };
        let op = visible[pos];
        let anchor_pos = self
            .anchor
            .and_then(|a| visible.iter().position(|&i| i == a));
        match (extend, anchor_pos) {
            (true, Some(a)) => {
                self.selection.clear();
                self.selection.extend(&visible[a.min(pos)..=a.max(pos)]);
            }
            _ => {
                self.selection.clear();
                self.selection.insert(op);
                self.anchor = Some(op);
            }
        }
        self.cursor = Some(op);
        self.scroll_to = Some(op);
    }

    /// Pointage (MARK-01) : non pointé → pointé → rapproché → non pointé. Seules
    /// les lignes dans le même état que la ligne de référence changent.
    pub fn cycle_tack(&mut self) {
        let Some(&reference) = self.selection.iter().next() else {
            return;
        };
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
        let visible = Arc::clone(&self.view.visible);
        let ops = &mut self.document.accounts[self.account].operations;
        for &i in visible.iter() {
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

/// Solde en fin de journée pour chaque jour ayant au moins une opération.
fn balance_series(account: &bp_core::model::Account) -> Arc<[(Date, Amount)]> {
    let mut by_date: Vec<(Date, Amount)> = account
        .operations
        .iter()
        .map(|op| (op.date, op.amount))
        .collect();
    by_date.sort_unstable_by_key(|(d, _)| *d);
    let mut series: Vec<(Date, Amount)> = Vec::new();
    let mut balance = Amount::ZERO;
    for (date, amount) in by_date {
        balance = balance + amount;
        match series.last_mut() {
            Some(last) if last.0 == date => last.1 = balance,
            _ => series.push((date, balance)),
        }
    }
    series.into()
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
        assert_eq!(*d.visible(), [3, 2, 1, 0]);
        assert_eq!(d.balance_of(3), Amount::from_cents(192_000));
        assert_eq!(d.balance_of(0), Amount::from_cents(-1_000));
    }

    #[test]
    fn filters_and_totals() {
        let mut d = doc();
        d.filters.payee = "carre".into();
        d.refresh();
        assert_eq!(*d.visible(), [3, 0]);
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
        assert_eq!(
            *d.visible(),
            [3],
            "le filtre de pointage voit le changement"
        );
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
        assert_eq!(
            *d.visible(),
            [4],
            "l'index de recherche connaît la nouvelle ligne"
        );

        d.delete_selection();
        d.refresh();
        assert!(d.visible().is_empty());
        assert_eq!(d.current_account().operations.len(), 4);
    }
}
