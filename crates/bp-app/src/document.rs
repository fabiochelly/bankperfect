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
    /// Mot de passe du fichier v5 d'origine, ou celui choisi pendant la
    /// session (pas encore forcément enregistré).
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
    /// Lignes rapprochées verrouillées : ni dépointées, ni modifiées, ni
    /// supprimées (réglage « Verrouiller les lignes rapprochées »).
    pub locked: bool,
    /// La colonne Date montre les dates de pointage (F11, pour la session).
    pub show_mark_dates: bool,
    /// Panneau des soldes et solde prévisionnel affichés (réglages).
    pub show_balance_panel: bool,
    pub show_forecast: bool,
    /// Niveaux d'annulation gardés.
    pub max_undo: usize,
    /// Pointage simple : non pointé ↔ pointé, sans rapprochement.
    pub disable_reconcile: bool,
    /// Soldes calculés sur toutes les lignes, même filtrées.
    ignore_filter_totals: bool,
    /// Message à afficher (refus d'une action), repris par l'application.
    pub notice: Option<String>,
    /// Opérations sélectionnées (indices dans le compte courant).
    pub selection: BTreeSet<usize>,
    /// Dernière ligne cliquée, pour les sélections avec Maj.
    pub anchor: Option<usize>,
    /// Extrémité mobile d'une sélection étendue au clavier (Maj).
    pub cursor: Option<usize>,
    pub scroll_to: Option<usize>,
    /// Identifiant de cette ouverture (les extensions refusent les objets
    /// d'un autre fichier).
    pub file_id: u64,
    /// Évènements à transmettre aux extensions (évènement, lignes, ajout à
    /// la dernière étape d'annulation).
    pub ext_events: Vec<ExtEvent>,

    view: View,
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    /// Incrémenté à chaque modification des opérations.
    revision: u64,
    /// Dernière étiquette d'étape d'annulation attribuée.
    undo_tag: u64,
}

/// Évènement pour les extensions : évènement, lignes (compte, identifiant),
/// ajout à la dernière étape d'annulation.
pub type ExtEvent = (bp_ext::Event, Vec<(usize, u64)>, bool);

static FILE_IDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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
    /// Compte, version, empreinte de l'échéancier et date du jour.
    series_key: Option<(usize, u64, u64, Date)>,
}

/// État d'un compte avant une action, pour Annuler/Refaire.
struct UndoEntry {
    label: String,
    /// Compte affiché après l'annulation.
    account: usize,
    /// Opérations des comptes touchés par l'action, avant elle.
    accounts: Vec<(usize, Vec<Operation>)>,
    /// Référentiels avant l'action, s'ils ont changé (extensions).
    referentials: Option<bp_core::model::Referentials>,
    /// Étiquette de l'étape : les crochets d'extension et les suites d'un
    /// même lancement s'ajoutent à l'étape qui porte la leur.
    tag: u64,
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
            locked: false,
            show_mark_dates: false,
            show_balance_panel: true,
            show_forecast: true,
            max_undo: 30,
            disable_reconcile: false,
            ignore_filter_totals: false,
            notice: None,
            selection: BTreeSet::new(),
            anchor: None,
            cursor: None,
            scroll_to: None,
            file_id: FILE_IDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ext_events: Vec::new(),
            view: View::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            undo_tag: 0,
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
    /// Numéro de version du contenu, incrémenté à chaque modification.
    pub fn revision(&self) -> u64 {
        self.revision
    }

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
            self.view.key = Some(key);
            self.view.filters = None;
        }
        let today = Date::today();
        // Courbe du solde, prolongée par les échéances à venir.
        let series_key = (
            self.account,
            self.revision,
            schedules_fingerprint(&self.document),
            today,
        );
        if self.view.series_key != Some(series_key) {
            self.view.insights.series = balance_series(&self.document, self.account, today);
            self.view.series_key = Some(series_key);
        }
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
        let all: Vec<usize>;
        let lines: &[usize] = if self.ignore_filter_totals {
            all = (0..ops.len()).collect();
            &all
        } else {
            &self.view.visible
        };
        for &i in lines {
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
    /// Une ligne sélectionnée est rapprochée alors que le verrou est actif.
    pub fn selection_locked(&self) -> bool {
        self.locked
            && self.selection.iter().any(|&i| {
                self.current_account()
                    .operations
                    .get(i)
                    .is_some_and(|op| op.tack == Tack::Reconciled)
            })
    }

    /// Change le pointage de la sélection ; refusé (faux) si des lignes
    /// rapprochées verrouillées devraient être dépointées.
    pub fn cycle_tack(&mut self) -> bool {
        let Some(&reference) = self.selection.iter().next() else {
            return true;
        };
        if self.locked && self.current_account().operations[reference].tack == Tack::Reconciled {
            self.notice = Some("Les lignes rapprochées sont verrouillées (menu Opérations)".into());
            return false;
        }
        self.snapshot("pointage");
        let ops = &mut self.document.accounts[self.account].operations;
        let from = ops[reference].tack;
        let to = match (from, self.disable_reconcile) {
            // Pointage simple : la ligne pointée est enregistrée « rapprochée »,
            // comme dans l'ancienne version.
            (Tack::None, true) => Tack::Reconciled,
            (_, true) => Tack::None,
            (Tack::None, false) => Tack::Pointed,
            (Tack::Pointed, false) => Tack::Reconciled,
            (Tack::Reconciled, false) => Tack::None,
        };
        let today = Date::today();
        for &i in &self.selection {
            if ops[i].tack == from {
                ops[i].tack = to;
                ops[i].mark_date = today;
            }
        }
        self.touch_tacks();
        true
    }

    /// Date de pointage manuelle des lignes sélectionnées (MARK-05).
    pub fn set_mark_date(&mut self, date: Date) {
        if self.selection.is_empty() {
            return;
        }
        self.snapshot("date de pointage");
        let ops = &mut self.document.accounts[self.account].operations;
        for &i in &self.selection {
            ops[i].mark_date = date;
        }
        self.touch_tacks();
    }

    /// Affiche (ou non) la date de pointage sous la date des lignes pointées.
    pub fn set_show_mark_dates(&mut self, on: bool) {
        self.show_mark_dates = on;
        if self.sort == SortColumn::MarkDate {
            self.sort = SortColumn::Date;
        }
    }

    /// Rapprocher toutes les lignes pointées affichées (MARK-03).
    pub fn reconcile_all_pointed(&mut self) {
        self.snapshot("rapprocher tout");
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
        self.snapshot(if target.is_some() {
            "modification"
        } else {
            "ajout"
        });
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
        self.apply_auto_rules(&[(self.account, index)]);
        let event = if target.is_some() {
            bp_ext::Event::OperationChanged
        } else {
            bp_ext::Event::OperationAdded
        };
        let uid = self.document.accounts[self.account].operations[index].uid;
        self.ext_events
            .push((event, vec![(self.account, uid)], true));
        let account = &self.document.accounts[self.account];
        self.view.index.update(&self.document, account, index);
        self.touch();
        self.refresh();
        self.selection.clear();
        self.selection.insert(index);
        self.anchor = Some(index);
        self.scroll_to = Some(index);
    }

    /// Modifie le document par `edit` sur les comptes `accounts` (annulable,
    /// sans trace si `edit` échoue).
    pub fn edit_accounts<R, E>(
        &mut self,
        label: &str,
        accounts: &[usize],
        edit: impl FnOnce(&mut bp_core::model::Document) -> Result<R, E>,
    ) -> Result<R, E> {
        self.snapshot_accounts(label, self.account, accounts);
        match edit(&mut self.document) {
            Ok(result) => {
                self.selection.clear();
                self.anchor = None;
                self.view.indexed_account = None;
                self.view.series_key = None;
                self.touch();
                self.refresh();
                Ok(result)
            }
            Err(e) => {
                self.undo.pop();
                Err(e)
            }
        }
    }

    /// Sélectionne des lignes du compte courant et fait défiler jusqu'à la
    /// première.
    pub fn select_lines(&mut self, lines: &[usize]) {
        self.selection = lines.iter().copied().collect();
        self.anchor = lines.first().copied();
        self.scroll_to = lines.first().copied();
    }

    /// Ligne sœur (compte, ligne) d'une ligne de transfert du compte courant.
    pub fn sister_of(&self, index: usize) -> Option<(usize, usize)> {
        let op = self.current_account().operations.get(index)?;
        bp_core::split::transfer_direction(&op.payee)?;
        bp_core::split::find_sister(&self.document, self.account, index)
            .filter(|(a, _)| *a != self.account)
    }

    /// Affiche le compte de la ligne sœur et la sélectionne.
    pub fn go_to_sister(&mut self, index: usize) -> bool {
        let Some((account, line)) = self.sister_of(index) else {
            return false;
        };
        self.select_account(account);
        self.refresh();
        self.selection.clear();
        self.selection.insert(line);
        self.anchor = Some(line);
        self.scroll_to = Some(line);
        true
    }

    /// Modifie une ligne de transfert et, si `both`, recopie date, détail,
    /// catégorie et montant dans sa ligne sœur (annulable sur les deux
    /// comptes).
    pub fn update_linked(&mut self, index: usize, op: Operation, both: bool) {
        let Some(sister) = self.sister_of(index).filter(|_| both) else {
            return self.upsert_operation(Some(index), op);
        };
        let account = self.account;
        self.snapshot_accounts("modification du transfert", account, &[account, sister.0]);
        let uid = self.current_account().operations[index].uid;
        self.document.accounts[account].operations[index] = Operation { uid, ..op };
        bp_core::split::copy_to_sister(&mut self.document, (account, index), sister);
        self.view.indexed_account = None;
        self.view.series_key = None;
        self.touch();
        self.refresh();
        self.selection.clear();
        self.selection.insert(index);
        self.scroll_to = Some(index);
    }

    /// Supprime la ligne de transfert sélectionnée : avec sa sœur si `both`,
    /// sinon la sœur est déliée (« Transfert » devient « Virement »).
    pub fn delete_linked(&mut self, index: usize, both: bool) {
        let Some(sister) = self.sister_of(index) else {
            return self.delete_selection();
        };
        let account = self.account;
        self.snapshot_accounts("suppression du transfert", account, &[account, sister.0]);
        if both {
            self.document.accounts[sister.0].operations.remove(sister.1);
        } else {
            bp_core::split::unlink_transfer(
                &mut self.document.accounts[sister.0].operations[sister.1],
            );
        }
        self.document.accounts[account].operations.remove(index);
        self.selection.clear();
        self.anchor = None;
        self.view.indexed_account = None;
        self.view.series_key = None;
        self.touch();
    }

    pub fn delete_selection(&mut self) {
        self.snapshot("suppression");
        let ops = &mut self.document.accounts[self.account].operations;
        for &i in self.selection.iter().rev() {
            ops.remove(i);
        }
        self.selection.clear();
        self.anchor = None;
        self.view.indexed_account = None;
        self.touch();
    }

    /// Applique des règles de calcul automatique aux lignes affichées du
    /// compte courant (annulable) : toutes les actives, ou la règle `only`.
    /// Renvoie le nombre de lignes modifiées et les erreurs rencontrées.
    pub fn apply_rules(
        &mut self,
        rules: &bp_core::autofields::AutoFieldRules,
        only: Option<usize>,
    ) -> (usize, Vec<String>) {
        let lines: Vec<usize> = self.view.visible.to_vec();
        self.snapshot("règles automatiques");
        let bp_core::model::Document {
            categories,
            accounts,
            ..
        } = &mut self.document;
        let account = &mut accounts[self.account];
        let (changed, errors) = match only {
            Some(k) => match rules
                .rules
                .get(k)
                .map(|r| r.apply(categories, account, &lines))
            {
                Some(Ok(outcome)) => (
                    outcome.changed.len(),
                    outcome.errors.iter().map(|e| e.message.clone()).collect(),
                ),
                Some(Err(e)) => (0, vec![e.to_string()]),
                None => (0, Vec::new()),
            },
            None => {
                let batch = rules.apply_active(categories, account, &lines);
                (
                    batch.changed.len(),
                    batch.errors.iter().map(|(_, e)| e.to_string()).collect(),
                )
            }
        };
        if changed == 0 {
            self.undo.pop();
        } else {
            self.view.indexed_account = None;
            self.view.series_key = None;
            self.touch();
        }
        (changed, errors)
    }

    /// Remplace des montants du compte courant (annulable). Les lignes
    /// rapprochées verrouillées sont laissées telles quelles. Renvoie le
    /// nombre de lignes modifiées.
    pub fn set_amounts(&mut self, changes: &[(usize, Amount)], label: &str) -> usize {
        if changes.is_empty() {
            return 0;
        }
        self.snapshot(label);
        let locked = self.locked;
        let operations = &mut self.document.accounts[self.account].operations;
        let mut changed = 0;
        for &(line, amount) in changes {
            if let Some(op) = operations.get_mut(line)
                && op.amount != amount
                && !(locked && op.tack == Tack::Reconciled)
            {
                op.amount = amount;
                changed += 1;
            }
        }
        if changed == 0 {
            self.undo.pop();
        } else {
            self.view.series_key = None;
            self.touch();
        }
        changed
    }

    /// Règles « appliquer automatiquement » sur des lignes ajoutées ou
    /// modifiées : (compte, opération).
    pub fn apply_auto_rules(&mut self, changes: &[(usize, usize)]) {
        if changes.is_empty() {
            return;
        }
        let Ok(rules) = bp_core::autofields::AutoFieldRules::load(
            &self.document,
            &bp_core::legacy_pickle::parse_py,
        ) else {
            return;
        };
        if !rules.rules.iter().any(|r| r.active && r.auto_apply) {
            return;
        }
        let results = rules.apply_to_new_lines_in_document(&mut self.document, changes);
        if results.iter().any(|(_, b)| !b.changed.is_empty()) {
            self.view.indexed_account = None;
            self.view.series_key = None;
            self.touch();
        }
    }

    /// Ajoute des opérations importées à un compte (annulable) ; les lignes
    /// importées du compte courant sont sélectionnées.
    pub fn import_into(&mut self, account: usize, operations: Vec<Operation>) -> usize {
        if operations.is_empty() {
            return 0;
        }
        self.snapshot_accounts("import", account, &[account]);
        let first = self.document.next_operation_uid();
        let target = &mut self.document.accounts[account].operations;
        let start = target.len();
        for (uid, mut op) in (first..).zip(operations) {
            op.uid = uid;
            target.push(op);
        }
        let added: Vec<usize> = (start..target.len()).collect();
        let n = added.len();
        let changes: Vec<(usize, usize)> = added.iter().map(|&l| (account, l)).collect();
        self.apply_auto_rules(&changes);
        let ops = &self.document.accounts[account].operations;
        let lines = added
            .iter()
            .filter_map(|&l| ops.get(l).map(|o| (account, o.uid)))
            .collect();
        self.ext_events
            .push((bp_ext::Event::OperationAdded, lines, true));
        self.view.indexed_account = None;
        self.view.series_key = None;
        self.touch();
        if account == self.account {
            self.refresh();
            self.selection = added.iter().copied().collect();
            self.anchor = added.first().copied();
            self.scroll_to = added.first().copied();
        }
        n
    }

    /// Copie des opérations sélectionnées (presse-papiers interne).
    pub fn copy_selection(&self) -> Vec<Operation> {
        let ops = &self.current_account().operations;
        self.selection
            .iter()
            .filter_map(|&i| ops.get(i).cloned())
            .collect()
    }

    /// Colle des opérations dans le compte courant et les sélectionne.
    pub fn paste(&mut self, operations: &[Operation]) -> usize {
        if operations.is_empty() {
            return 0;
        }
        self.snapshot("coller");
        let added = bp_core::edit::paste_operations(&mut self.document, self.account, operations);
        self.view.indexed_account = None;
        self.touch();
        self.refresh();
        self.selection = added.iter().copied().collect();
        self.anchor = added.first().copied();
        self.scroll_to = added.first().copied();
        added.len()
    }

    /// Lignes sélectionnées et affichées, dans l'ordre d'affichage.
    pub fn selected_visible(&self) -> Vec<usize> {
        self.view
            .visible
            .iter()
            .copied()
            .filter(|i| self.selection.contains(i))
            .collect()
    }

    /// Remplacer dans le tiers et le détail des lignes affichées.
    pub fn replace_in_visible(&mut self, find: &str, replacement: &str) -> usize {
        let lines: Vec<usize> = self.view.visible.to_vec();
        if bp_core::edit::count_matches(&self.document.accounts[self.account], &lines, find) == 0 {
            return 0;
        }
        self.snapshot("remplacer");
        let count = bp_core::edit::replace_text(
            &mut self.document.accounts[self.account],
            &lines,
            find,
            replacement,
        );
        self.view.indexed_account = None;
        self.touch();
        count
    }

    /// Modifier le groupe des lignes sélectionnées et affichées.
    pub fn apply_group_edit(&mut self, edit: &bp_core::edit::GroupEdit) -> usize {
        let lines = self.selected_visible();
        if lines.is_empty() {
            return 0;
        }
        self.snapshot("modifier le groupe");
        let document = self.document.clone_without_operations();
        let changed = bp_core::edit::apply_group_edit(
            &document,
            &mut self.document.accounts[self.account],
            &lines,
            edit,
        );
        self.view.indexed_account = None;
        self.touch();
        changed
    }

    /// Modifie les référentiels (comptes, catégories, tiers, modes). Comme
    /// dans l'ancienne version, l'historique d'annulation est vidé.
    pub fn edit_document<R>(&mut self, edit: impl FnOnce(&mut Document) -> R) -> R {
        let result = edit(&mut self.document);
        self.undo.clear();
        self.redo.clear();
        if self.account >= self.document.accounts.len() {
            self.account = self.document.account_order.first().copied().unwrap_or(0);
        }
        self.category_styles = crate::category_icons::styles(&self.document);
        self.selection.clear();
        self.anchor = None;
        self.cursor = None;
        self.view.indexed_account = None;
        self.view.series_key = None;
        self.touch();
        result
    }

    /// Remplace les champs supplémentaires d'une opération.
    pub fn set_extra(&mut self, index: usize, extra: Vec<(String, String)>) {
        if self
            .current_account()
            .operations
            .get(index)
            .is_none_or(|op| op.extra == extra)
        {
            return;
        }
        self.snapshot("champs supplémentaires");
        self.document.accounts[self.account].operations[index].extra = extra;
        self.view.indexed_account = None;
        self.touch();
    }

    /// Mémorise l'état du compte courant avant une action.
    fn snapshot(&mut self, label: &str) {
        self.snapshot_accounts(label, self.account, &[self.account]);
    }

    /// Mémorise l'état de plusieurs comptes avant une action (virements) ;
    /// `shown` est le compte affiché après une annulation.
    fn snapshot_accounts(&mut self, label: &str, shown: usize, accounts: &[usize]) {
        let mut saved: Vec<(usize, Vec<Operation>)> = Vec::new();
        for &a in accounts {
            if a < self.document.accounts.len() && !saved.iter().any(|(k, _)| *k == a) {
                saved.push((a, self.document.accounts[a].operations.clone()));
            }
        }
        self.undo_tag += 1;
        self.undo.push(UndoEntry {
            label: label.to_string(),
            account: shown,
            accounts: saved,
            referentials: None,
            tag: self.undo_tag,
        });
        while self.undo.len() > self.max_undo.max(1) {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Étiquette de la dernière étape d'annulation.
    pub fn last_undo_tag(&self) -> Option<u64> {
        self.undo.last().map(|e| e.tag)
    }

    /// Document prêté à une extension, avec le compte affiché, la sélection
    /// et les lignes affichées.
    pub fn extension_context(&mut self, merge_tag: Option<u64>) -> bp_ext::DocContext<'_> {
        let selection = self.selected_visible();
        let visible = self.view.visible.to_vec();
        bp_ext::DocContext {
            file_id: self.file_id,
            path: self.path.clone(),
            account: self.account,
            selection,
            visible,
            locked: self.locked,
            merge_tag,
            document: &mut self.document,
        }
    }

    /// Inscrit les modifications d'une extension dans l'historique (une
    /// étape par lancement) et met la vue à jour.
    pub fn apply_extension(&mut self, effects: &bp_ext::Effects) {
        let mut referentials = false;
        for changes in &effects.changes {
            referentials |= changes.referentials.is_some() || changes.accounts_added;
            match self.undo.last_mut() {
                Some(last) if last.tag == changes.tag => {
                    for (a, operations) in &changes.accounts {
                        if !last.accounts.iter().any(|(k, _)| k == a) {
                            last.accounts.push((*a, operations.clone()));
                        }
                    }
                    if last.referentials.is_none() {
                        last.referentials = changes.referentials.clone();
                    }
                }
                _ => {
                    self.undo.push(UndoEntry {
                        label: changes.label.clone(),
                        account: self.account,
                        accounts: changes.accounts.clone(),
                        referentials: changes.referentials.clone(),
                        tag: changes.tag,
                    });
                    while self.undo.len() > self.max_undo.max(1) {
                        self.undo.remove(0);
                    }
                }
            }
            self.redo.clear();
        }
        if self.account >= self.document.accounts.len() {
            self.account = self.document.account_order.first().copied().unwrap_or(0);
        }
        if referentials {
            self.category_styles = crate::category_icons::styles(&self.document);
        }
        if !effects.changes.is_empty() {
            self.selection.clear();
            self.anchor = None;
            self.cursor = None;
            self.view.indexed_account = None;
            self.view.series_key = None;
            self.touch();
        } else if effects.feature_changed {
            self.dirty = true;
        }
        if let Some((account, uids)) = &effects.select
            && *account < self.document.accounts.len()
        {
            if *account != self.account {
                self.select_account(*account);
            }
            self.refresh();
            let ops = &self.document.accounts[self.account].operations;
            let lines: Vec<usize> = uids
                .iter()
                .filter_map(|uid| ops.iter().position(|o| o.uid == *uid))
                .collect();
            self.select_lines(&lines);
        }
        self.refresh();
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }

    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.undo.pop() else {
            return false;
        };
        let reverse = self.swap_in(entry);
        self.redo.push(reverse);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(entry) = self.redo.pop() else {
            return false;
        };
        let reverse = self.swap_in(entry);
        self.undo.push(reverse);
        true
    }

    /// Remet les opérations mémorisées et renvoie l'état remplacé.
    fn swap_in(&mut self, entry: UndoEntry) -> UndoEntry {
        let mut current = Vec::with_capacity(entry.accounts.len());
        for (a, operations) in entry.accounts {
            if let Some(account) = self.document.accounts.get_mut(a) {
                current.push((a, std::mem::replace(&mut account.operations, operations)));
            }
        }
        let referentials = entry.referentials.map(|saved| {
            let current = self.document.swap_referentials(saved);
            self.category_styles = crate::category_icons::styles(&self.document);
            current
        });
        self.account = entry.account;
        self.selection.clear();
        self.anchor = None;
        self.cursor = None;
        self.view.indexed_account = None;
        self.touch();
        UndoEntry {
            label: entry.label,
            account: entry.account,
            accounts: current,
            referentials,
            tag: entry.tag,
        }
    }

    /// Modification qui change le tri ou les soldes : tout est recalculé.
    fn touch(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }

    pub fn ignore_filter_totals(&self) -> bool {
        self.ignore_filter_totals
    }

    /// Soldes sur toutes les lignes (option), recalculés si le réglage change.
    pub fn set_ignore_filter_totals(&mut self, on: bool) {
        if self.ignore_filter_totals != on {
            self.ignore_filter_totals = on;
            self.view.filters = None;
        }
    }

    /// Seul le pointage a changé : ni l'index, ni le tri, ni le solde
    /// progressif ne bougent, il suffit de refiltrer.
    fn touch_tacks(&mut self) {
        self.view.filters = None;
        self.dirty = true;
    }
}

/// Solde en fin de journée pour chaque jour ayant au moins une opération.
/// Horizon des échéances ajoutées à la courbe du solde (jours).
const SCHEDULE_HORIZON_DAYS: i32 = 92;

/// Empreinte des données de l'échéancier, pour recalculer la courbe quand
/// une échéance change.
fn schedules_fingerprint(document: &bp_core::model::Document) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for key in [
        bp_core::scheduler::FEATURE_KEY,
        bp_core::scheduler::LEGACY_KEY,
    ] {
        bp_core::legacy_pickle::feature_bytes(document, key).hash(&mut hasher);
    }
    hasher.finish()
}

/// Solde en fin de journée, date par date, du compte `account`, prolongé
/// par les occurrences des échéances des trois prochains mois (celles en
/// retard comptées demain).
fn balance_series(
    document: &bp_core::model::Document,
    account: usize,
    today: Date,
) -> Arc<[(Date, Amount)]> {
    let Some(acc) = document.accounts.get(account) else {
        return Arc::from([]);
    };
    let mut by_date: Vec<(Date, Amount)> = acc
        .operations
        .iter()
        .map(|op| (op.date, op.amount))
        .collect();
    if let Ok(loaded) = bp_core::scheduler::load(document)
        && !loaded.schedules.is_empty()
    {
        let last = by_date.iter().map(|(d, _)| *d).max().unwrap_or(today);
        let until = Date((today.0 + SCHEDULE_HORIZON_DAYS).max(last.0));
        by_date.extend(
            bp_core::scheduler::forecast(document, &loaded.schedules, account, until, today, false)
                .into_iter()
                .filter(|l| l.kind == bp_core::scheduler::ForecastKind::Schedule)
                // Échéances en retard : attendues dès demain, comme dans le
                // solde prévisionnel.
                .map(|l| (l.date.max(Date(today.0 + 1)), l.amount)),
        );
    }
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

    /// Document à deux comptes avec un transfert de 100 € de « Courant »
    /// vers « Livret ».
    fn doc_with_transfer() -> OpenDoc {
        let mut d = doc();
        let mut livret = d.document.accounts[0].clone_header();
        livret.uid = "b".into();
        livret.name = "Livret".into();
        d.document.accounts.push(livret);
        d.document.account_order.push(1);
        let request = bp_core::split::TransferRequest {
            date: Date(45010),
            kind: bp_core::split::TransferKind::Transfer,
            details: String::new(),
            category: None,
            amount: Amount::from_cents(10_000),
            from: 0,
            to: 1,
        };
        d.edit_accounts("transfert", &[0, 1], |doc| {
            bp_core::split::create_transfer(doc, &request)
        })
        .unwrap();
        d
    }

    #[test]
    fn linked_transfer_edit_delete_and_undo() {
        let mut d = doc_with_transfer();
        let line = d.current_account().operations.len() - 1;
        assert_eq!(d.sister_of(line), Some((1, 0)));
        // Modification des deux lignes.
        let mut op = d.operation(line).clone();
        op.amount = Amount::from_cents(-15_000);
        d.update_linked(line, op, true);
        assert_eq!(
            d.document.accounts[1].operations[0].amount,
            Amount::from_cents(15_000)
        );
        // Annulation : les deux comptes reviennent.
        assert!(d.undo());
        assert_eq!(
            d.document.accounts[0].operations[line].amount,
            Amount::from_cents(-10_000)
        );
        assert_eq!(
            d.document.accounts[1].operations[0].amount,
            Amount::from_cents(10_000)
        );
        // Suppression d'une seule ligne : l'autre devient un virement.
        d.delete_linked(line, false);
        assert_eq!(d.current_account().operations.len(), 4);
        assert!(
            d.document.accounts[1].operations[0]
                .payee
                .starts_with("Virement depuis")
        );
        assert!(d.undo());
        // Suppression des deux lignes.
        d.delete_linked(line, true);
        assert!(d.document.accounts[1].operations.is_empty());
        assert_eq!(d.current_account().operations.len(), 4);
        // Navigation vers la ligne sœur.
        assert!(d.undo());
        assert!(d.go_to_sister(line));
        assert_eq!(d.account, 1);
        assert_eq!(d.selection.iter().copied().collect::<Vec<_>>(), [0]);
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
    fn replace_then_undo_and_redo() {
        let mut d = doc();
        assert_eq!(d.replace_in_visible("Carrefour", "Monoprix"), 2);
        assert_eq!(d.undo_label(), Some("remplacer"));
        d.filters.payee = "monoprix".into();
        d.refresh();
        assert_eq!(
            d.visible().len(),
            2,
            "l'index de recherche voit le remplacement"
        );
        assert!(d.undo());
        d.refresh();
        assert!(d.visible().is_empty());
        assert_eq!(d.redo_label(), Some("remplacer"));
        assert!(d.redo());
        d.refresh();
        assert_eq!(d.visible().len(), 2);
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

    struct NoServices;

    impl bp_ext::Services for NoServices {
        fn pick_open(&self, _: &str, _: &[(String, Vec<String>)]) -> Option<PathBuf> {
            None
        }
        fn pick_save(&self, _: &str, _: &str, _: &[(String, Vec<String>)]) -> Option<PathBuf> {
            None
        }
        fn open_url(&self, _: &str) {}
    }

    fn engine(source: &str) -> bp_ext::Engine {
        let dir = std::env::temp_dir().join(format!(
            "bp-ext-doc-{}-{}",
            std::process::id(),
            source.len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("essai.luau"), source).unwrap();
        let mut engine =
            bp_ext::Engine::new(dir.clone(), dir.join("d"), std::rc::Rc::new(NoServices));
        engine.load_all(&[]);
        assert_eq!(engine.extensions[0].error, None);
        engine
    }

    #[test]
    fn extension_changes_are_one_undo_step_with_referentials() {
        let mut doc = doc();
        let mut engine = engine(
            r#"
            bp.command{ title = "Ranger…", run = function()
                local c = bp.add_category("Courses")
                for _, op in bp.current_account():operations{ payee = "carrefour" } do
                    if op.tack ~= "reconciled" then op.category = c end
                end
                bp.current_account():add{ payee = "Nouveau", amount = -3 }
            end }
            "#,
        );
        let fx = engine.run_command(0, 0, Some(&mut doc.extension_context(None)));
        assert!(fx.errors.is_empty(), "{:?}", fx.errors);
        doc.apply_extension(&fx);
        assert!(doc.dirty);
        assert_eq!(doc.document.categories.len(), 1);
        assert_eq!(doc.current_account().operations.len(), 5);
        assert_eq!(doc.current_account().operations[3].category, Some(0));
        assert_eq!(doc.undo_label(), Some("Ranger"));
        assert!(doc.undo());
        assert!(doc.document.categories.is_empty(), "catégorie retirée");
        assert_eq!(doc.current_account().operations.len(), 4);
        assert_eq!(doc.current_account().operations[3].category, None);
        assert!(doc.redo());
        assert_eq!(doc.document.categories.len(), 1);
        assert_eq!(doc.current_account().operations.len(), 5);
    }

    #[test]
    fn hooks_join_the_entry_step() {
        let mut doc = doc();
        let mut engine = engine(
            r#"
            bp.on("operation_added", function(op)
                op.details = "vu par l'extension"
            end)
            "#,
        );
        let mut new = op(0, 9, "Boulangerie", -450, Tack::None);
        new.uid = 0;
        doc.upsert_operation(None, new);
        let events = std::mem::take(&mut doc.ext_events);
        assert_eq!(events.len(), 1);
        let (event, lines, merge) = &events[0];
        assert!(*merge);
        let tag = doc.last_undo_tag();
        let fx = engine.fire(*event, lines, doc.extension_context(tag));
        assert!(fx.errors.is_empty(), "{:?}", fx.errors);
        doc.apply_extension(&fx);
        assert_eq!(
            doc.current_account().operations[4].details,
            "vu par l'extension"
        );
        assert_eq!(doc.undo_label(), Some("ajout"), "pas d'étape en plus");
        assert!(doc.undo());
        assert_eq!(doc.current_account().operations.len(), 4);
        assert!(doc.undo_label().is_none());
    }
}
