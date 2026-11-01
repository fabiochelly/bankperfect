//! Portefeuille de titres d'un compte.
//!
//! Chaque ligne « Titre » est un lot (voir [`crate::quotes`]) : la quantité et
//! le prix d'achat restent dans le tiers, comme dans l'original, et la valeur
//! dans le montant. Ce module regroupe les lots par titre et y ajoute ce que
//! l'original ne savait pas faire : prix de revient, plus-values, ventes,
//! dividendes et historique de la valeur.
//!
//! Une vente retire les titres en commençant par les plus anciens (FIFO) et
//! ajoute au compte le produit de la vente. Ventes et dividendes sont des
//! opérations ordinaires, reconnues par des champs supplémentaires masqués
//! (`#…`) : les supprimer ou les annuler les retire aussi du journal.

use std::collections::HashMap;

use chrono::Datelike;

use crate::model::{Amount, Date, Document, Mode, Operation, StandardMode, Tack};
use crate::quotes::{self, StockLine, positive_number};

/// Nature du mouvement : [`KIND_SALE`] ou [`KIND_DIVIDEND`].
const KEY_KIND: &str = "#titre";
const KEY_CODE: &str = "#titre_code";
const KEY_QUANTITY: &str = "#titre_quantite";
const KEY_GAIN: &str = "#titre_gain";
const KIND_SALE: &str = "vente";
const KIND_DIVIDEND: &str = "dividende";

/// Écart toléré sur les quantités, qui peuvent être fractionnaires.
const EPSILON: f64 = 1e-9;

/// Titre détenu : tous les lots d'un même code.
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub code: String,
    pub name: String,
    /// Index des opérations du compte.
    pub lines: Vec<usize>,
    pub quantity: f64,
    /// Coût total, `None` si un lot n'a pas de prix d'achat lisible.
    pub cost: Option<f64>,
    pub value: f64,
    /// Un lot a une quantité illisible : la position est incomplète.
    pub incomplete: bool,
}

impl Position {
    /// Prix de revient unitaire.
    pub fn unit_cost(&self) -> Option<f64> {
        self.cost
            .filter(|_| self.quantity > 0.0)
            .map(|cost| cost / self.quantity)
    }

    /// Plus-value latente.
    pub fn gain(&self) -> Option<f64> {
        self.cost.map(|cost| self.value - cost)
    }

    pub fn gain_percent(&self) -> Option<f64> {
        self.cost
            .filter(|cost| *cost > 0.0)
            .map(|cost| (self.value - cost) / cost * 100.0)
    }
}

/// Regroupe les lots par titre, des plus grosses positions aux plus petites.
/// `price` donne le cours d'un code dans la devise du compte ; sans cours, la
/// valeur est celle enregistrée sur la ligne.
pub fn positions(lines: &[StockLine], price: &dyn Fn(&str) -> Option<f64>) -> Vec<Position> {
    let mut list: Vec<Position> = Vec::new();
    for lot in lines {
        let k = match list.iter().position(|p| p.code == lot.identifier) {
            Some(k) => k,
            None => {
                list.push(Position {
                    code: lot.identifier.clone(),
                    name: lot.name.clone(),
                    lines: Vec::new(),
                    quantity: 0.0,
                    cost: Some(0.0),
                    value: 0.0,
                    incomplete: false,
                });
                list.len() - 1
            }
        };
        let position = &mut list[k];
        position.lines.push(lot.line);
        position.quantity += lot.quantity.unwrap_or(0.0);
        position.incomplete |= lot.quantity.is_none();
        position.cost = match (position.cost, lot.quantity, lot.unit_cost) {
            (Some(total), Some(quantity), Some(unit)) => Some(total + quantity * unit),
            _ => None,
        };
        position.value += match (price(&lot.identifier), lot.quantity) {
            (Some(price), Some(quantity)) => price * quantity,
            _ => lot.value.as_f64(),
        };
    }
    list.sort_by(|a, b| b.value.total_cmp(&a.value));
    list
}

/// Totaux du portefeuille.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Summary {
    pub value: f64,
    /// Coût des positions dont le prix d'achat est connu.
    pub cost: f64,
    /// Plus-value latente de ces mêmes positions.
    pub gain: f64,
    /// Nombre de positions sans prix d'achat, exclues du coût et de la
    /// plus-value.
    pub uncosted: usize,
}

impl Summary {
    pub fn gain_percent(&self) -> Option<f64> {
        (self.cost > 0.0).then(|| self.gain / self.cost * 100.0)
    }
}

pub fn summary(positions: &[Position]) -> Summary {
    let mut total = Summary::default();
    for position in positions {
        total.value += position.value;
        match position.cost {
            Some(cost) => {
                total.cost += cost;
                total.gain += position.value - cost;
            }
            None => total.uncosted += 1,
        }
    }
    total
}

/// Quantité telle qu'écrite en tête du tiers.
fn format_quantity(quantity: f64) -> String {
    let rounded = (quantity * 1e6).round() / 1e6;
    format!("{rounded}").replace('.', &crate::i18n::decimal_separator().to_string())
}

fn to_amount(value: f64) -> Amount {
    Amount::from_cents((value * 100.0).round() as i64)
}

/// Opération de trésorerie liée à un titre.
fn cash_operation(mode: StandardMode, date: Date, payee: String, code: &str) -> Operation {
    Operation {
        uid: 0,
        date,
        mark_date: date,
        tack: Tack::None,
        mode: Mode::Standard(mode),
        payee,
        details: code.to_string(),
        category: None,
        amount: Amount::ZERO,
        extra: Vec::new(),
    }
}

fn push(document: &mut Document, account: usize, mut operation: Operation) {
    operation.uid = document.next_operation_uid();
    document.accounts[account].operations.push(operation);
}

/// Ajoute un lot au compte. Avec `pay`, l'achat est aussi débité du compte
/// (cas d'un compte-titres qui porte ses liquidités).
pub fn add_purchase(document: &mut Document, account: usize, lot: Operation, pay: bool) {
    let payment = (pay && lot.amount.0 > 0).then(|| {
        let name = lot.payee.split(" acheté ").next().unwrap_or(&lot.payee);
        let mut payment = cash_operation(
            StandardMode::VirementEmis,
            lot.date,
            crate::tr!("Achat {}", name),
            &lot.details,
        );
        payment.amount = Amount(-lot.amount.0);
        payment
    });
    push(document, account, lot);
    if let Some(payment) = payment {
        push(document, account, payment);
    }
}

/// Vente saisie, en devise du compte.
#[derive(Debug, Clone, PartialEq)]
pub struct Sale {
    pub code: String,
    pub quantity: f64,
    pub price: f64,
    pub fees: f64,
    pub date: Date,
}

/// Résultat d'une vente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaleResult {
    /// Produit net, ajouté au compte.
    pub proceeds: Amount,
    /// Plus-value réalisée, `None` si un lot vendu n'a pas de prix d'achat.
    pub gain: Option<f64>,
}

/// Lots d'un code, du plus ancien au plus récent.
fn lots_of(document: &Document, account: usize, code: &str) -> Vec<StockLine> {
    let mut lots: Vec<StockLine> = quotes::stock_lines(&document.accounts[account])
        .into_iter()
        .filter(|lot| lot.identifier == code)
        .collect();
    lots.sort_by_key(|lot| (lot.date, lot.line));
    lots
}

/// Refuse de toucher à une ligne rapprochée quand le rapprochement est
/// verrouillé.
fn check_unlocked(
    document: &Document,
    account: usize,
    lines: &[usize],
    locked: bool,
) -> Result<(), String> {
    let operations = &document.accounts[account].operations;
    if locked
        && lines
            .iter()
            .any(|&line| operations[line].tack == Tack::Reconciled)
    {
        return Err(crate::tr!("Une ligne de ce titre est rapprochée et verrouillée.").into());
    }
    Ok(())
}

/// Vend des titres, les plus anciens d'abord. Le document n'est modifié que
/// si la vente est possible.
pub fn sell(
    document: &mut Document,
    account: usize,
    sale: &Sale,
    locked: bool,
) -> Result<SaleResult, String> {
    let lots = lots_of(document, account, &sale.code);
    if lots.iter().any(|lot| lot.quantity.is_none()) {
        return Err(crate::tr!("Une ligne de ce titre a une quantité illisible.").into());
    }
    let held: f64 = lots.iter().filter_map(|lot| lot.quantity).sum();
    if sale.quantity.is_nan() || sale.quantity <= 0.0 || sale.quantity > held + EPSILON {
        return Err(crate::tr!(
            "Quantité à vendre invalide : vous détenez {}.",
            format_quantity(held)
        ));
    }
    if !sale.price.is_finite() || sale.price <= 0.0 || sale.fees < 0.0 {
        return Err(crate::tr!("Prix de vente illisible.").into());
    }

    let mut left = sale.quantity;
    let mut sold_cost = Some(0.0);
    let mut emptied = Vec::new();
    let mut reduced = Vec::new();
    let mut touched = Vec::new();
    for lot in &lots {
        if left <= EPSILON {
            break;
        }
        let quantity = lot.quantity.unwrap_or(0.0);
        let taken = quantity.min(left);
        left -= taken;
        touched.push(lot.line);
        sold_cost = sold_cost.zip(lot.unit_cost).map(|(c, u)| c + taken * u);
        if quantity - taken <= EPSILON {
            emptied.push(lot.line);
        } else {
            reduced.push((lot.line, quantity, quantity - taken));
        }
    }
    check_unlocked(document, account, &touched, locked)?;

    let name = lots.first().map_or("", |lot| lot.name.as_str()).to_string();
    let operations = &mut document.accounts[account].operations;
    for (line, before, after) in reduced {
        let op = &mut operations[line];
        let rest = op
            .payee
            .split_once(char::is_whitespace)
            .map_or("", |(_, rest)| rest);
        op.payee = format!("{} {rest}", format_quantity(after))
            .trim()
            .to_string();
        op.amount = Amount((op.amount.0 as f64 * after / before).round() as i64);
    }
    emptied.sort_unstable();
    for line in emptied.into_iter().rev() {
        operations.remove(line);
    }

    let proceeds = to_amount(sale.quantity * sale.price - sale.fees);
    let gain = sold_cost.map(|cost| proceeds.as_f64() - cost);
    let mut cash = cash_operation(
        StandardMode::VirementRecu,
        sale.date,
        crate::tr!(
            "Vente {} {} à {}",
            format_quantity(sale.quantity),
            name,
            format_quantity(sale.price)
        ),
        &sale.code,
    );
    cash.amount = proceeds;
    cash.extra = vec![
        (KEY_KIND.into(), KIND_SALE.into()),
        (KEY_CODE.into(), sale.code.clone()),
        (KEY_QUANTITY.into(), sale.quantity.to_string()),
    ];
    if let Some(gain) = gain {
        cash.extra.push((KEY_GAIN.into(), format!("{gain:.2}")));
    }
    push(document, account, cash);
    Ok(SaleResult { proceeds, gain })
}

/// Dividende encaissé sur un titre.
pub fn dividend_operation(
    code: &str,
    name: &str,
    amount: &str,
    date: Date,
) -> Result<Operation, String> {
    let net = positive_number(amount).ok_or(crate::tr!("Montant du dividende illisible."))?;
    let mut operation = cash_operation(
        StandardMode::VirementRecu,
        date,
        crate::tr!("Dividende {}", name),
        code,
    );
    operation.amount = to_amount(net);
    operation.extra = vec![
        (KEY_KIND.into(), KIND_DIVIDEND.into()),
        (KEY_CODE.into(), code.to_string()),
    ];
    Ok(operation)
}

/// Retire toutes les lignes d'un titre (saisie erronée). Le journal des
/// ventes et dividendes n'est pas touché.
pub fn remove_position(
    document: &mut Document,
    account: usize,
    code: &str,
    locked: bool,
) -> Result<usize, String> {
    let mut lines: Vec<usize> = lots_of(document, account, code)
        .iter()
        .map(|lot| lot.line)
        .collect();
    check_unlocked(document, account, &lines, locked)?;
    lines.sort_unstable();
    let operations = &mut document.accounts[account].operations;
    for &line in lines.iter().rev() {
        operations.remove(line);
    }
    Ok(lines.len())
}

/// Nature d'un mouvement du journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementKind {
    Sale,
    Dividend,
}

/// Vente ou dividende enregistré dans le compte.
#[derive(Debug, Clone, PartialEq)]
pub struct Movement {
    pub date: Date,
    pub kind: MovementKind,
    pub code: String,
    pub label: String,
    pub amount: Amount,
    /// Plus-value réalisée (ventes dont le coût est connu).
    pub gain: Option<f64>,
}

/// Journal des ventes et dividendes, du plus récent au plus ancien.
pub fn movements(operations: &[Operation]) -> Vec<Movement> {
    let mut list: Vec<Movement> = operations
        .iter()
        .filter_map(|op| {
            let kind = match op.extra(KEY_KIND)? {
                KIND_SALE => MovementKind::Sale,
                KIND_DIVIDEND => MovementKind::Dividend,
                _ => return None,
            };
            Some(Movement {
                date: op.date,
                kind,
                code: op.extra(KEY_CODE).unwrap_or(&op.details).to_string(),
                label: op.payee.clone(),
                amount: op.amount,
                gain: op.extra(KEY_GAIN).and_then(|g| g.parse().ok()),
            })
        })
        .collect();
    list.sort_by_key(|m| std::cmp::Reverse(m.date));
    list
}

/// Plus-values réalisées et dividendes d'une année.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YearTotals {
    pub year: i32,
    pub gain: f64,
    pub dividends: f64,
}

/// Totaux par année, de la plus récente à la plus ancienne.
pub fn year_totals(movements: &[Movement]) -> Vec<YearTotals> {
    let mut years: Vec<YearTotals> = Vec::new();
    for movement in movements {
        let year = movement.date.to_naive().year();
        let k = match years.iter().position(|y| y.year == year) {
            Some(k) => k,
            None => {
                years.push(YearTotals {
                    year,
                    gain: 0.0,
                    dividends: 0.0,
                });
                years.len() - 1
            }
        };
        match movement.kind {
            MovementKind::Sale => years[k].gain += movement.gain.unwrap_or(0.0),
            MovementKind::Dividend => years[k].dividends += movement.amount.as_f64(),
        }
    }
    years.sort_by_key(|y| std::cmp::Reverse(y.year));
    years
}

/// Valeur du portefeuille à une date.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HistoryPoint {
    pub date: Date,
    pub value: f64,
    /// Coût des lots détenus à cette date (prix d'achat connus).
    pub invested: f64,
}

/// Valeur passée des lots actuels, chacun à partir de sa date d'achat.
/// `prices` donne, par code, les cours datés et triés, en devise du compte.
pub fn value_history(
    lines: &[StockLine],
    prices: &HashMap<String, Vec<(Date, f64)>>,
) -> Vec<HistoryPoint> {
    let mut dates: Vec<Date> = prices.values().flatten().map(|(date, _)| *date).collect();
    dates.sort_unstable();
    dates.dedup();
    let mut points = Vec::new();
    for date in dates {
        let mut point = HistoryPoint {
            date,
            value: 0.0,
            invested: 0.0,
        };
        for lot in lines.iter().filter(|lot| lot.date <= date) {
            let (Some(quantity), Some(series)) = (lot.quantity, prices.get(&lot.identifier)) else {
                continue;
            };
            // Dernier cours connu à cette date (places fermées, jours fériés).
            let known = series.partition_point(|(day, _)| *day <= date);
            if known > 0 {
                point.value += quantity * series[known - 1].1;
                point.invested += quantity * lot.unit_cost.unwrap_or(0.0);
            }
        }
        if point.value > 0.0 {
            points.push(point);
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quotes::new_stock_operation;

    fn document(lots: &[(&str, &str, &str, &str, i32)]) -> Document {
        let mut document = crate::manage::new_document("Test", Amount::ZERO);
        document.accounts[0].operations.clear();
        for &(code, quantity, name, price, day) in lots {
            let op = new_stock_operation(code, quantity, name, price, Date(day)).unwrap();
            add_purchase(&mut document, 0, op, false);
        }
        document
    }

    fn lines(document: &Document) -> Vec<StockLine> {
        quotes::stock_lines(&document.accounts[0])
    }

    fn sale(code: &str, quantity: f64, price: f64, fees: f64) -> Sale {
        Sale {
            code: code.into(),
            quantity,
            price,
            fees,
            date: Date(46000),
        }
    }

    #[test]
    fn groups_lots_and_computes_gains() {
        let doc = document(&[
            ("ORA.PA", "10", "Orange", "10", 45000),
            ("ORA.PA", "10", "Orange", "20", 45100),
            ("AAPL", "1", "Apple", "", 45000),
        ]);
        let list = positions(&lines(&doc), &|code| (code == "ORA.PA").then_some(18.0));
        assert_eq!(list.len(), 2);
        let orange = &list[0];
        assert_eq!(orange.code, "ORA.PA");
        assert_eq!(orange.quantity, 20.0);
        assert_eq!(orange.unit_cost(), Some(15.0));
        assert_eq!(orange.value, 360.0);
        assert_eq!(orange.gain(), Some(60.0));
        assert_eq!(orange.gain_percent(), Some(20.0));
        assert_eq!(list[1].cost, None);
        let total = summary(&list);
        assert_eq!(total.cost, 300.0);
        assert_eq!(total.gain, 60.0);
        assert_eq!(total.uncosted, 1);
        assert_eq!(total.gain_percent(), Some(20.0));
    }

    #[test]
    fn sells_the_oldest_lots_first() {
        let mut doc = document(&[
            ("ORA.PA", "10", "Orange", "20", 45100),
            ("ORA.PA", "10", "Orange", "10", 45000),
        ]);
        let result = sell(&mut doc, 0, &sale("ORA.PA", 15.0, 30.0, 5.0), false).unwrap();
        // 10 à 10 puis 5 à 20 : coût 200, produit 445.
        assert_eq!(result.proceeds, Amount::from_cents(44500));
        assert_eq!(result.gain, Some(245.0));
        let left = lines(&doc);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].quantity, Some(5.0));
        assert_eq!(left[0].unit_cost, Some(20.0));
        assert_eq!(left[0].date, Date(45100));
        assert_eq!(left[0].value, Amount::from_cents(10000));

        let journal = movements(&doc.accounts[0].operations);
        assert_eq!(journal.len(), 1);
        assert_eq!(journal[0].kind, MovementKind::Sale);
        assert_eq!(journal[0].code, "ORA.PA");
        assert_eq!(journal[0].gain, Some(245.0));
        // Le produit de la vente n'est pas pris pour un titre.
        assert_eq!(
            doc.accounts[0].operations.last().unwrap().amount,
            result.proceeds
        );
    }

    #[test]
    fn refuses_impossible_sales_without_changing_anything() {
        let mut doc = document(&[("ORA.PA", "10", "Orange", "10", 45000)]);
        let before = doc.accounts[0].operations.clone();
        assert!(sell(&mut doc, 0, &sale("ORA.PA", 11.0, 30.0, 0.0), false).is_err());
        assert!(sell(&mut doc, 0, &sale("ORA.PA", 0.0, 30.0, 0.0), false).is_err());
        assert!(sell(&mut doc, 0, &sale("ORA.PA", 1.0, 0.0, 0.0), false).is_err());
        assert!(sell(&mut doc, 0, &sale("AAPL", 1.0, 30.0, 0.0), false).is_err());
        doc.accounts[0].operations[0].tack = Tack::Reconciled;
        assert!(sell(&mut doc, 0, &sale("ORA.PA", 1.0, 30.0, 0.0), true).is_err());
        assert!(remove_position(&mut doc, 0, "ORA.PA", true).is_err());
        doc.accounts[0].operations[0].tack = Tack::None;
        assert_eq!(doc.accounts[0].operations, before);
    }

    #[test]
    fn a_sale_without_purchase_price_has_no_gain() {
        let mut doc = document(&[("AAPL", "2", "Apple", "", 45000)]);
        let result = sell(&mut doc, 0, &sale("AAPL", 2.0, 100.0, 0.0), false).unwrap();
        assert_eq!(result.gain, None);
        assert!(lines(&doc).is_empty());
        assert_eq!(movements(&doc.accounts[0].operations)[0].gain, None);
    }

    #[test]
    fn totals_movements_by_year() {
        let mut doc = document(&[("ORA.PA", "10", "Orange", "10", 45000)]);
        let date = Date::from_ymd(2024, 6, 1).unwrap();
        let mut sold = sale("ORA.PA", 5.0, 12.0, 0.0);
        sold.date = date;
        sell(&mut doc, 0, &sold, false).unwrap();
        let later = Date::from_ymd(2025, 6, 1).unwrap();
        for amount in ["3,50", "1,50"] {
            let op = dividend_operation("ORA.PA", "Orange", amount, later).unwrap();
            add_purchase(&mut doc, 0, op, false);
        }
        assert!(dividend_operation("ORA.PA", "Orange", "abc", later).is_err());
        let journal = movements(&doc.accounts[0].operations);
        assert_eq!(journal.len(), 3);
        assert_eq!(
            year_totals(&journal),
            vec![
                YearTotals {
                    year: 2025,
                    gain: 0.0,
                    dividends: 5.0
                },
                YearTotals {
                    year: 2024,
                    gain: 10.0,
                    dividends: 0.0
                },
            ]
        );
    }

    #[test]
    fn the_journal_survives_saving_the_file() {
        let mut doc = document(&[("ORA.PA", "10", "Orange", "10", 45000)]);
        sell(&mut doc, 0, &sale("ORA.PA", 4.0, 12.0, 0.0), false).unwrap();
        let bytes =
            crate::file::to_bytes(&doc, &crate::format_v6::Protection::none().unwrap()).unwrap();
        let reopened = crate::file::open_bytes(&bytes, crate::format_v6::Unlock::None).unwrap();
        let operations = &reopened.document.accounts[0].operations;
        let journal = movements(operations);
        assert_eq!(journal.len(), 1);
        assert_eq!(journal[0].gain, Some(8.0));
        assert_eq!(journal[0].code, "ORA.PA");
    }

    #[test]
    fn a_paid_purchase_is_debited() {
        let mut doc = document(&[]);
        let lot = new_stock_operation("ORA.PA", "10", "Orange", "10", Date(45000)).unwrap();
        add_purchase(&mut doc, 0, lot, true);
        let operations = &doc.accounts[0].operations;
        assert_eq!(operations.len(), 2);
        assert_eq!(operations[1].payee, "Achat 10 Orange");
        assert_eq!(operations[0].amount.0 + operations[1].amount.0, 0);
        assert_ne!(operations[0].uid, operations[1].uid);
        assert_eq!(lines(&doc).len(), 1);
    }

    #[test]
    fn removes_a_position() {
        let mut doc = document(&[
            ("TYPO", "1", "Erreur", "", 45000),
            ("ORA.PA", "10", "Orange", "10", 45000),
            ("TYPO", "2", "Erreur", "", 45000),
        ]);
        assert_eq!(remove_position(&mut doc, 0, "TYPO", false), Ok(2));
        let left = lines(&doc);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].identifier, "ORA.PA");
    }

    #[test]
    fn builds_the_value_history_from_purchase_dates() {
        let doc = document(&[
            ("ORA.PA", "10", "Orange", "10", 45001),
            ("AAPL", "1", "Apple", "100", 45002),
        ]);
        let mut prices = HashMap::new();
        prices.insert(
            "ORA.PA".to_string(),
            vec![(Date(45000), 9.0), (Date(45001), 11.0), (Date(45003), 12.0)],
        );
        prices.insert("AAPL".to_string(), vec![(Date(45002), 110.0)]);
        let history = value_history(&lines(&doc), &prices);
        let summary: Vec<(i32, f64, f64)> = history
            .iter()
            .map(|p| (p.date.0, p.value, p.invested))
            .collect();
        assert_eq!(
            summary,
            vec![
                // Avant le premier achat : rien.
                (45001, 110.0, 100.0),
                // Orange reste à son dernier cours connu.
                (45002, 220.0, 200.0),
                (45003, 230.0, 200.0),
            ]
        );
    }
}
