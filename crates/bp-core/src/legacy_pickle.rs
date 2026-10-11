//! Relecture des données persistantes des anciens scripts Python 2 (pickles).
//!
//! Les scripts de BankPerfect 7.x stockaient leurs réglages dans le fichier
//! `.bp` (lignes `bp_script_data_<clé>`, décodées par `SimpleDecode` dans
//! [`FeatureData::value`]) sous forme de **pickles Python 2**, presque toujours
//! en protocole 0 (texte). L'ancien fichier externe `Scripts/Scheduler/scheduler.dat`
//! est un pickle protocole 1 (binaire). Ce module fournit :
//!
//! - un petit interpréteur d'opcodes ([`parse`]) qui produit une [`Value`]
//!   générique ; il couvre les protocoles 0, 1 et 2 (et les quelques opcodes des
//!   protocoles 3 à 5 qui ne demandent rien de plus) ;
//! - des types serde fidèles aux structures des anciens scripts, avec une
//!   conversion tolérante depuis la valeur pickle (champ manquant ou de mauvais
//!   type → valeur par défaut du script d'origine).
//!
//! # Pourquoi un interpréteur maison plutôt que `serde-pickle`
//!
//! - Les `str` Python 2 sont des octets **CP1252** : il faut les décoder nous-mêmes
//!   (`serde-pickle` les rend en octets bruts ou les décode en UTF-8, ce qui
//!   échoue sur les accents).
//! - Python 2.3 écrit les booléens en protocole 0 sous la forme `I00`/`I01`, les
//!   chaînes `S'…'` avec les échappements de `repr()` (`\xe9`…) et les flottants
//!   spéciaux de Windows (`1.#INF`) : autant de cas à maîtriser précisément.
//! - Les données sont partagées par référence (mémo `p`/`g`) puis modifiées
//!   après coup (`(lp1` … `a`) : on travaille sur une arène de nœuds pour garder
//!   la sémantique exacte de Python.
//! - Le besoin est petit (environ 50 opcodes, lecture seule) et le code reste
//!   sans dépendance supplémentaire dans une crate qui manipule des données
//!   financières.
//!
//! # Données persistantes connues
//!
//! | Clé | Script | Structure (Python 2) |
//! |---|---|---|
//! | `fabiobpscheduler` | Scheduler (`scheduler.py`, `scheduler_add.py`, `scheduler_auto.py`, `previsions.py`) | liste de dicts « échéance » : `account` (index réel du compte), `transfer` (index ou -1), `tiers`, `details`, `mode` (0–8), `categ` (id ou -1), `montant` (flottant signé), `nextdate`/`enddate` (tuples `(a, m, j)`, `(0,0,0)` = pas de fin), `delta`, `deltaunit` (0 jours, 1 mois, 2 ans), `autoinsert` (0/1), `Postpone` (lettres `S`, `D`, `F`) → [`LegacyScheduleEntry`] |
//! | `echeancier` | ancienne version de l'échéancier | même structure, avec en plus `shift` (ancien nom de `Postpone`) ; ignorée par les scripts 7.x → [`LegacyScheduleEntry`] |
//! | `scheduler.dat` (fichier externe, protocole 1) | Scheduler avant la v2.4 | dict `{nom_du_fichier.bp: [échéances]}` → [`entries_from_scheduler_dat`] |
//! | `fabiobpbudget` | Budget | dict `{id_catégorie: montant mensuel signé}` → [`LegacyBudget`] |
//! | `fabiobpbudgetyear` | Budget annuel | liste de dicts `name`, `year`, `accounts` (liste d'index, facultatif), `lines` : liste de dicts `title`, `autocalc_real` (0/1), `categs` (ids), `estimated_values` et `real_values` (12 nombres) → [`LegacyBudgetYear`] |
//! | `fabiobpautofilters` | Filtres prédéfinis (et Budget en lecture) | dict `{nom: {dt1, dt2, mde, who, ctg, val: textes, mrk: 0–4}}` ; `mrk` : 4 toutes, 3 pointées et rapprochées, 2 rapprochées, 1 pointées, 0 non pointées → [`LegacyFilterRule`] |
//! | `fabiobpautofields` | Calcul automatique des champs | liste de dicts `active`, `field` (0 tiers, 1 détails, 2 les deux), `contains` (texte ou expression `=…`), `contains_or`, `do_mode`/`mode`, `do_third`/`third`, `do_info`/`info`, `do_categ`/`categ`, `emptycateg`, `do_date`/`date`, `withnewline` (booléens `I00`/`I01`) → [`LegacyAutoField`] |
//! | `fabiobpabbreviations` | Saisie multiple | dict `{abréviation: texte}` → [`LegacyAbbreviations`] |
//! | `fabiobptransfer` | Transfert de compte à compte | dict `mode` (0 virement, 1 chèque, 2 espèces), `categ`, `details`, `account1`, `account2` (derniers choix) → [`LegacyTransferSettings`] |
//! | `fabiobpviewall` | Relevé multicompte | dict `{"accounts": [index des comptes]}` → [`LegacyViewAll`] |
//! | `bpcashcards` | inconnu (aucun script fourni ne l'écrit) | dict `{numéro_de_carte: (texte, entier ×5)}`, ex. `('', 0, 2, 1, 18, 0)` ; signification non documentée → [`LegacyCashCards`] (valeurs brutes) |
//! | `fabiobpmailaddress` | Sauvegarde par e-mail | **pas un pickle** : l'adresse en texte brut → [`legacy_mail_address`] |

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::model::Document;

/// Valeur Python reconstruite à partir d'un pickle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    None,
    Bool(bool),
    Int(i64),
    /// Entier long ne tenant pas sur 64 bits, en texte décimal.
    BigInt(String),
    Float(f64),
    /// `str` Python 2 : octets décodés en CP1252.
    Str(String),
    /// `unicode` Python 2.
    Unicode(String),
    /// `bytes` Python 3 (protocoles 3 et plus).
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    /// Dictionnaire, dans l'ordre d'insertion du pickle.
    Dict(Vec<(Value, Value)>),
    /// Référence à une classe ou une fonction (`module.nom`).
    Global {
        module: String,
        name: String,
    },
    /// Objet reconstruit (`REDUCE`, `NEWOBJ`, `INST`, `OBJ`) : classe, arguments,
    /// état éventuel (`BUILD`). On ne l'interprète pas.
    Object {
        class: Box<Value>,
        args: Box<Value>,
        state: Option<Box<Value>>,
    },
}

impl Value {
    /// Valeur associée à une clé texte dans un dictionnaire.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_dict()?
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }

    pub fn as_dict(&self) -> Option<&[(Value, Value)]> {
        match self {
            Value::Dict(items) => Some(items),
            _ => None,
        }
    }

    /// Liste ou tuple.
    pub fn as_seq(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) | Value::Tuple(items) => Some(items),
            _ => None,
        }
    }

    /// Texte (`str` ou `unicode`).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Unicode(s) => Some(s),
            _ => None,
        }
    }

    /// Entier (un booléen vaut 0 ou 1, un flottant est tronqué comme `int()`).
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::Int(i) => Some(i),
            Value::Bool(b) => Some(b as i64),
            Value::Float(f) if f.is_finite() && f.abs() < 9.0e18 => Some(f.trunc() as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::Float(f) => Some(f),
            Value::Int(i) => Some(i as f64),
            Value::Bool(b) => Some(b as i64 as f64),
            _ => None,
        }
    }

    /// Valeur de vérité au sens de Python.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::None => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::BigInt(_) => true,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) | Value::Unicode(s) => !s.is_empty(),
            Value::Bytes(b) => !b.is_empty(),
            Value::List(v) | Value::Tuple(v) => !v.is_empty(),
            Value::Dict(d) => !d.is_empty(),
            Value::Global { .. } | Value::Object { .. } => true,
        }
    }
}

/// Erreur de lecture d'un pickle.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("pickle illisible (position {position}) : {message}")]
pub struct PickleError {
    pub position: usize,
    pub message: String,
}

/// Lit un pickle Python 2 (protocoles 0 à 2 ; quelques opcodes des protocoles
/// suivants sont aussi acceptés).
pub fn parse(data: &[u8]) -> Result<Value, PickleError> {
    Parser::new(data).run()
}

// ---------------------------------------------------------------------------
// Interpréteur
// ---------------------------------------------------------------------------

/// Profondeur maximale d'imbrication reconstruite (protège des structures
/// récursives).
const MAX_DEPTH: usize = 200;
/// Nombre maximal de valeurs reconstruites (protège des références partagées
/// démultipliées).
const MAX_NODES: usize = 2_000_000;

#[derive(Debug)]
enum Node {
    Leaf(Value),
    List(Vec<usize>),
    Tuple(Vec<usize>),
    Dict(Vec<(usize, usize)>),
    Object {
        class: usize,
        args: usize,
        state: Option<usize>,
    },
}

#[derive(Debug, Clone, Copy)]
enum Slot {
    Mark,
    Node(usize),
}

struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    /// Position de l'opcode en cours (pour les messages d'erreur).
    op_pos: usize,
    arena: Vec<Node>,
    stack: Vec<Slot>,
    memo: HashMap<u64, usize>,
}

impl<'a> Parser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            op_pos: 0,
            arena: Vec::new(),
            stack: Vec::new(),
            memo: HashMap::new(),
        }
    }

    fn error(&self, message: impl Into<String>) -> PickleError {
        PickleError {
            position: self.op_pos,
            message: message.into(),
        }
    }

    fn run(mut self) -> Result<Value, PickleError> {
        loop {
            self.op_pos = self.pos;
            let op = self.byte()?;
            match op {
                b'.' => {
                    let top = self.pop()?;
                    let mut budget = MAX_NODES;
                    return self.build(top, 0, &mut budget);
                }
                b'(' => self.stack.push(Slot::Mark),
                b'0' => {
                    self.pop_slot()?;
                }
                b'1' => {
                    self.pop_mark()?;
                }
                b'2' => {
                    let top = self.peek()?;
                    self.stack.push(Slot::Node(top));
                }
                // --- Valeurs simples, protocole 0 (texte) ---
                b'N' => self.push_leaf(Value::None),
                b'I' => {
                    let line = self.line()?;
                    let value = match line {
                        b"00" => Value::Bool(false),
                        b"01" => Value::Bool(true),
                        _ => self.parse_int_text(line)?,
                    };
                    self.push_leaf(value);
                }
                b'L' => {
                    let line = self.line()?;
                    let line = line.strip_suffix(b"L").unwrap_or(line);
                    let value = self.parse_int_text(line)?;
                    self.push_leaf(value);
                }
                b'F' => {
                    let line = self.line()?;
                    let value =
                        parse_float_text(line).ok_or_else(|| self.error("flottant invalide"))?;
                    self.push_leaf(Value::Float(value));
                }
                b'S' => {
                    let line = self.line()?;
                    let bytes = self.parse_quoted(line)?;
                    self.push_leaf(Value::Str(decode_cp1252(&bytes)));
                }
                b'V' => {
                    let line = self.line()?;
                    self.push_leaf(Value::Unicode(decode_raw_unicode_escape(line)));
                }
                // --- Valeurs simples, protocoles binaires ---
                b'J' => {
                    let v = i32::from_le_bytes(self.array::<4>()?);
                    self.push_leaf(Value::Int(v as i64));
                }
                b'K' => {
                    let v = self.byte()?;
                    self.push_leaf(Value::Int(v as i64));
                }
                b'M' => {
                    let v = u16::from_le_bytes(self.array::<2>()?);
                    self.push_leaf(Value::Int(v as i64));
                }
                0x8a => {
                    let n = self.byte()? as usize;
                    let bytes = self.take(n)?;
                    let value = self.decode_long(bytes)?;
                    self.push_leaf(value);
                }
                0x8b => {
                    let n = self.len32()?;
                    let bytes = self.take(n)?;
                    let value = self.decode_long(bytes)?;
                    self.push_leaf(value);
                }
                b'G' => {
                    let v = f64::from_be_bytes(self.array::<8>()?);
                    self.push_leaf(Value::Float(v));
                }
                0x88 => self.push_leaf(Value::Bool(true)),
                0x89 => self.push_leaf(Value::Bool(false)),
                b'T' => {
                    let n = self.len32()?;
                    let bytes = self.take(n)?;
                    self.push_leaf(Value::Str(decode_cp1252(bytes)));
                }
                b'U' => {
                    let n = self.byte()? as usize;
                    let bytes = self.take(n)?;
                    self.push_leaf(Value::Str(decode_cp1252(bytes)));
                }
                b'X' => {
                    let n = self.len32()?;
                    let bytes = self.take(n)?;
                    self.push_leaf(Value::Unicode(String::from_utf8_lossy(bytes).into_owned()));
                }
                0x8c => {
                    let n = self.byte()? as usize;
                    let bytes = self.take(n)?;
                    self.push_leaf(Value::Unicode(String::from_utf8_lossy(bytes).into_owned()));
                }
                0x8d => {
                    let n = self.len64()?;
                    let bytes = self.take(n)?;
                    self.push_leaf(Value::Unicode(String::from_utf8_lossy(bytes).into_owned()));
                }
                b'B' => {
                    let n = self.len32()?;
                    let bytes = self.take(n)?.to_vec();
                    self.push_leaf(Value::Bytes(bytes));
                }
                b'C' => {
                    let n = self.byte()? as usize;
                    let bytes = self.take(n)?.to_vec();
                    self.push_leaf(Value::Bytes(bytes));
                }
                0x8e => {
                    let n = self.len64()?;
                    let bytes = self.take(n)?.to_vec();
                    self.push_leaf(Value::Bytes(bytes));
                }
                // --- Conteneurs ---
                b']' => self.push_node(Node::List(Vec::new())),
                b'}' => self.push_node(Node::Dict(Vec::new())),
                b')' => self.push_node(Node::Tuple(Vec::new())),
                b'l' => {
                    let items = self.pop_mark()?;
                    self.push_node(Node::List(items));
                }
                b't' => {
                    let items = self.pop_mark()?;
                    self.push_node(Node::Tuple(items));
                }
                0x85..=0x87 => {
                    let n = (op - 0x84) as usize;
                    let mut items = Vec::with_capacity(n);
                    for _ in 0..n {
                        items.push(self.pop()?);
                    }
                    items.reverse();
                    self.push_node(Node::Tuple(items));
                }
                b'd' => {
                    let items = self.pop_mark()?;
                    let pairs = self.pairs(items)?;
                    self.push_node(Node::Dict(pairs));
                }
                b'a' => {
                    let value = self.pop()?;
                    let list = self.peek()?;
                    self.list_mut(list)?.push(value);
                }
                b'e' => {
                    let items = self.pop_mark()?;
                    let list = self.peek()?;
                    self.list_mut(list)?.extend(items);
                }
                b's' => {
                    let value = self.pop()?;
                    let key = self.pop()?;
                    let dict = self.peek()?;
                    self.dict_set(dict, key, value)?;
                }
                b'u' => {
                    let items = self.pop_mark()?;
                    let pairs = self.pairs(items)?;
                    let dict = self.peek()?;
                    for (key, value) in pairs {
                        self.dict_set(dict, key, value)?;
                    }
                }
                // --- Mémo ---
                b'p' => {
                    let line = self.line()?;
                    let index = self.parse_index(line)?;
                    let top = self.peek()?;
                    self.memo.insert(index, top);
                }
                b'q' => {
                    let index = self.byte()? as u64;
                    let top = self.peek()?;
                    self.memo.insert(index, top);
                }
                b'r' => {
                    let index = u32::from_le_bytes(self.array::<4>()?) as u64;
                    let top = self.peek()?;
                    self.memo.insert(index, top);
                }
                0x94 => {
                    let index = self.memo.len() as u64;
                    let top = self.peek()?;
                    self.memo.insert(index, top);
                }
                b'g' => {
                    let line = self.line()?;
                    let index = self.parse_index(line)?;
                    self.memo_get(index)?;
                }
                b'h' => {
                    let index = self.byte()? as u64;
                    self.memo_get(index)?;
                }
                b'j' => {
                    let index = u32::from_le_bytes(self.array::<4>()?) as u64;
                    self.memo_get(index)?;
                }
                // --- Objets (conservés sans interprétation) ---
                b'c' => {
                    let module = decode_cp1252(self.line()?);
                    let name = decode_cp1252(self.line()?);
                    self.push_leaf(Value::Global { module, name });
                }
                0x93 => {
                    let name = self.pop()?;
                    let module = self.pop()?;
                    let module = self.leaf_text(module)?;
                    let name = self.leaf_text(name)?;
                    self.push_leaf(Value::Global { module, name });
                }
                b'R' | 0x81 => {
                    let args = self.pop()?;
                    let class = self.pop()?;
                    self.push_node(Node::Object {
                        class,
                        args,
                        state: None,
                    });
                }
                b'b' => {
                    let state = self.pop()?;
                    let target = self.peek()?;
                    if let Node::Object { state: slot, .. } = &mut self.arena[target] {
                        *slot = Some(state);
                    }
                }
                b'i' => {
                    let module = decode_cp1252(self.line()?);
                    let name = decode_cp1252(self.line()?);
                    let items = self.pop_mark()?;
                    let class = self.alloc(Node::Leaf(Value::Global { module, name }));
                    let args = self.alloc(Node::Tuple(items));
                    self.push_node(Node::Object {
                        class,
                        args,
                        state: None,
                    });
                }
                b'o' => {
                    let mut items = self.pop_mark()?;
                    if items.is_empty() {
                        return Err(self.error("OBJ sans classe"));
                    }
                    let class = items.remove(0);
                    let args = self.alloc(Node::Tuple(items));
                    self.push_node(Node::Object {
                        class,
                        args,
                        state: None,
                    });
                }
                // --- En-têtes ---
                0x80 => {
                    let version = self.byte()?;
                    if version > 5 {
                        return Err(self.error(format!("protocole {version} non pris en charge")));
                    }
                }
                0x95 => {
                    self.array::<8>()?;
                }
                _ => return Err(self.error(format!("opcode inconnu 0x{op:02x}"))),
            }
        }
    }

    // --- Lecture d'octets ---

    fn byte(&mut self) -> Result<u8, PickleError> {
        let b = *self
            .data
            .get(self.pos)
            .ok_or_else(|| self.error("fin de données inattendue"))?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], PickleError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&end| end <= self.data.len())
            .ok_or_else(|| self.error("fin de données inattendue"))?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PickleError> {
        let slice = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    fn len32(&mut self) -> Result<usize, PickleError> {
        let n = i32::from_le_bytes(self.array::<4>()?);
        usize::try_from(n).map_err(|_| self.error("longueur négative"))
    }

    fn len64(&mut self) -> Result<usize, PickleError> {
        let n = u64::from_le_bytes(self.array::<8>()?);
        usize::try_from(n).map_err(|_| self.error("longueur trop grande"))
    }

    /// Ligne terminée par `\n` (un `\r` final, laissé par une écriture en mode
    /// texte sous Windows, est ignoré).
    fn line(&mut self) -> Result<&'a [u8], PickleError> {
        let rest = &self.data[self.pos..];
        let len = rest
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| self.error("fin de ligne manquante"))?;
        self.pos += len + 1;
        let line = &rest[..len];
        Ok(line.strip_suffix(b"\r").unwrap_or(line))
    }

    // --- Pile et arène ---

    fn alloc(&mut self, node: Node) -> usize {
        self.arena.push(node);
        self.arena.len() - 1
    }

    fn push_node(&mut self, node: Node) {
        let index = self.alloc(node);
        self.stack.push(Slot::Node(index));
    }

    fn push_leaf(&mut self, value: Value) {
        self.push_node(Node::Leaf(value));
    }

    fn pop_slot(&mut self) -> Result<Slot, PickleError> {
        self.stack.pop().ok_or_else(|| self.error("pile vide"))
    }

    fn pop(&mut self) -> Result<usize, PickleError> {
        match self.pop_slot()? {
            Slot::Node(index) => Ok(index),
            Slot::Mark => Err(self.error("marque inattendue")),
        }
    }

    fn peek(&self) -> Result<usize, PickleError> {
        match self.stack.last() {
            Some(Slot::Node(index)) => Ok(*index),
            Some(Slot::Mark) => Err(self.error("marque inattendue")),
            None => Err(self.error("pile vide")),
        }
    }

    /// Dépile jusqu'à la dernière marque ; renvoie les éléments dans l'ordre.
    fn pop_mark(&mut self) -> Result<Vec<usize>, PickleError> {
        let mark = self
            .stack
            .iter()
            .rposition(|slot| matches!(slot, Slot::Mark))
            .ok_or_else(|| self.error("marque absente"))?;
        let items = self.stack.split_off(mark + 1);
        self.stack.pop();
        Ok(items
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Node(index) => Some(index),
                Slot::Mark => None,
            })
            .collect())
    }

    fn pairs(&self, items: Vec<usize>) -> Result<Vec<(usize, usize)>, PickleError> {
        if !items.len().is_multiple_of(2) {
            return Err(self.error("nombre impair d'éléments pour un dictionnaire"));
        }
        Ok(items.chunks(2).map(|c| (c[0], c[1])).collect())
    }

    fn list_mut(&mut self, index: usize) -> Result<&mut Vec<usize>, PickleError> {
        let position = self.op_pos;
        match &mut self.arena[index] {
            Node::List(items) => Ok(items),
            _ => Err(PickleError {
                position,
                message: "ajout à un objet qui n'est pas une liste".into(),
            }),
        }
    }

    fn dict_set(&mut self, dict: usize, key: usize, value: usize) -> Result<(), PickleError> {
        // Une clé déjà présente est remplacée, comme en Python.
        let mut budget = MAX_NODES;
        let key_value = self.build(key, 0, &mut budget)?;
        let mut existing = None;
        if let Node::Dict(pairs) = &self.arena[dict] {
            for (i, (k, _)) in pairs.iter().enumerate() {
                let mut budget = MAX_NODES;
                if self.build(*k, 0, &mut budget)? == key_value {
                    existing = Some(i);
                    break;
                }
            }
        }
        let position = self.op_pos;
        match &mut self.arena[dict] {
            Node::Dict(pairs) => {
                match existing {
                    Some(i) => pairs[i].1 = value,
                    None => pairs.push((key, value)),
                }
                Ok(())
            }
            _ => Err(PickleError {
                position,
                message: "affectation dans un objet qui n'est pas un dictionnaire".into(),
            }),
        }
    }

    fn memo_get(&mut self, index: u64) -> Result<(), PickleError> {
        let node = *self
            .memo
            .get(&index)
            .ok_or_else(|| self.error(format!("référence au mémo {index} inconnue")))?;
        self.stack.push(Slot::Node(node));
        Ok(())
    }

    fn leaf_text(&self, index: usize) -> Result<String, PickleError> {
        match &self.arena[index] {
            Node::Leaf(v) => v
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| self.error("texte attendu")),
            _ => Err(self.error("texte attendu")),
        }
    }

    // --- Décodage des valeurs ---

    fn parse_int_text(&self, text: &[u8]) -> Result<Value, PickleError> {
        let text = std::str::from_utf8(text)
            .map_err(|_| self.error("entier invalide"))?
            .trim();
        if let Ok(v) = text.parse::<i64>() {
            return Ok(Value::Int(v));
        }
        let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            Ok(Value::BigInt(text.trim_start_matches('+').to_string()))
        } else {
            Err(self.error(format!("entier invalide : {text}")))
        }
    }

    fn parse_index(&self, text: &[u8]) -> Result<u64, PickleError> {
        std::str::from_utf8(text)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .ok_or_else(|| self.error("index de mémo invalide"))
    }

    /// Entier long binaire (complément à deux, petit-boutiste).
    fn decode_long(&self, bytes: &[u8]) -> Result<Value, PickleError> {
        if bytes.is_empty() {
            return Ok(Value::Int(0));
        }
        if bytes.len() > 16 {
            return Err(self.error("entier long trop grand"));
        }
        let negative = bytes[bytes.len() - 1] & 0x80 != 0;
        let mut buf = [if negative { 0xff } else { 0 }; 16];
        buf[..bytes.len()].copy_from_slice(bytes);
        let v = i128::from_le_bytes(buf);
        Ok(match i64::try_from(v) {
            Ok(v) => Value::Int(v),
            Err(_) => Value::BigInt(v.to_string()),
        })
    }

    /// Chaîne `S` : `'…'` ou `"…"` avec les échappements de `repr()`.
    fn parse_quoted(&self, line: &[u8]) -> Result<Vec<u8>, PickleError> {
        let quoted = line.len() >= 2
            && (line[0] == b'\'' || line[0] == b'"')
            && line[line.len() - 1] == line[0];
        if !quoted {
            return Err(self.error("chaîne sans guillemets"));
        }
        Ok(escape_decode(&line[1..line.len() - 1]))
    }

    // --- Reconstruction de l'arbre ---

    fn build(&self, index: usize, depth: usize, budget: &mut usize) -> Result<Value, PickleError> {
        if depth > MAX_DEPTH {
            return Err(self.error("structure trop profonde ou récursive"));
        }
        *budget = budget
            .checked_sub(1)
            .ok_or_else(|| self.error("structure trop volumineuse"))?;
        let child = |i: usize, budget: &mut usize| self.build(i, depth + 1, budget);
        Ok(match &self.arena[index] {
            Node::Leaf(v) => v.clone(),
            Node::List(items) => Value::List(
                items
                    .iter()
                    .map(|&i| child(i, budget))
                    .collect::<Result<_, _>>()?,
            ),
            Node::Tuple(items) => Value::Tuple(
                items
                    .iter()
                    .map(|&i| child(i, budget))
                    .collect::<Result<_, _>>()?,
            ),
            Node::Dict(pairs) => Value::Dict(
                pairs
                    .iter()
                    .map(|&(k, v)| Ok((child(k, budget)?, child(v, budget)?)))
                    .collect::<Result<_, PickleError>>()?,
            ),
            Node::Object { class, args, state } => Value::Object {
                class: Box::new(child(*class, budget)?),
                args: Box::new(child(*args, budget)?),
                state: match state {
                    Some(s) => Some(Box::new(child(*s, budget)?)),
                    None => None,
                },
            },
        })
    }
}

fn decode_cp1252(bytes: &[u8]) -> String {
    // WINDOWS_1252 (WHATWG) associe un caractère à chacun des 256 octets :
    // les octets non définis par Microsoft (0x81, 0x8D…) donnent U+0081…,
    // ce qui équivaut au repli latin-1 recommandé dans l'inventaire.
    encoding_rs::WINDOWS_1252
        .decode_without_bom_handling(bytes)
        .0
        .into_owned()
}

/// Flottant texte de `repr()` Python 2, y compris les formes Windows
/// (`1.#INF`, `-1.#IND`, `1.#QNAN`).
fn parse_float_text(text: &[u8]) -> Option<f64> {
    let text = std::str::from_utf8(text).ok()?.trim();
    if let Ok(v) = text.parse::<f64>() {
        return Some(v);
    }
    let upper = text.to_ascii_uppercase();
    let negative = upper.starts_with('-');
    if upper.contains("INF") {
        Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })
    } else if upper.contains("NAN") || upper.contains("IND") {
        Some(f64::NAN)
    } else {
        None
    }
}

/// Décodage `string_escape` de Python 2 (contenu d'une chaîne `S'…'`).
fn escape_decode(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c != b'\\' || i + 1 >= s.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let e = s[i + 1];
        i += 2;
        match e {
            b'\\' => out.push(b'\\'),
            b'\'' => out.push(b'\''),
            b'"' => out.push(b'"'),
            b'a' => out.push(7),
            b'b' => out.push(8),
            b'f' => out.push(12),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'v' => out.push(11),
            b'\n' => {}
            b'x' => {
                let hex = s.get(i..i + 2).and_then(|h| {
                    std::str::from_utf8(h)
                        .ok()
                        .and_then(|h| u8::from_str_radix(h, 16).ok())
                });
                match hex {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.extend_from_slice(b"\\x"),
                }
            }
            b'0'..=b'7' => {
                let mut value = (e - b'0') as u32;
                let mut n = 1;
                while n < 3 && i < s.len() && (b'0'..=b'7').contains(&s[i]) {
                    value = value * 8 + (s[i] - b'0') as u32;
                    i += 1;
                    n += 1;
                }
                out.push((value & 0xff) as u8);
            }
            other => {
                out.push(b'\\');
                out.push(other);
            }
        }
    }
    out
}

/// Décodage `raw-unicode-escape` (chaînes `V`) : octets latin-1, et `\uXXXX`
/// ou `\UXXXXXXXX` lorsqu'ils sont précédés d'un nombre impair de `\`.
fn decode_raw_unicode_escape(s: &[u8]) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut backslashes = 0usize;
    while i < s.len() {
        let c = s[i];
        if c == b'\\' && backslashes.is_multiple_of(2) {
            let width = match s.get(i + 1) {
                Some(b'u') => 4,
                Some(b'U') => 8,
                _ => 0,
            };
            if width > 0 {
                let decoded = s
                    .get(i + 2..i + 2 + width)
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .and_then(char::from_u32);
                if let Some(ch) = decoded {
                    out.push(ch);
                    i += 2 + width;
                    backslashes = 0;
                    continue;
                }
            }
        }
        backslashes = if c == b'\\' { backslashes + 1 } else { 0 };
        out.push(c as char);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Accès aux données d'un document
// ---------------------------------------------------------------------------

/// Octets bruts (déjà passés par `SimpleDecode`) de la donnée `key`.
///
/// Si la clé apparaît plusieurs fois, la première l'emporte, comme dans
/// `ObjectScriptsData.pas` (`GetData` parcourt la liste dans l'ordre).
pub fn feature_bytes<'a>(document: &'a Document, key: &str) -> Option<&'a [u8]> {
    document
        .feature_data
        .iter()
        .find(|f| f.key == key)
        .map(|f| f.value.as_slice())
}

/// Lit la donnée `key` comme un pickle. `None` si la clé est absente ou vide
/// (les scripts considèrent une chaîne vide comme « pas de donnée »).
pub fn feature_pickle(document: &Document, key: &str) -> Option<Result<Value, PickleError>> {
    let bytes = feature_bytes(document, key)?;
    if bytes.is_empty() {
        return None;
    }
    Some(parse(bytes))
}

// ---------------------------------------------------------------------------
// Aides de conversion tolérante
// ---------------------------------------------------------------------------

fn field_i64(v: &Value, key: &str, default: i64) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn field_f64(v: &Value, key: &str, default: f64) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn field_str(v: &Value, key: &str) -> String {
    v.get(key).map(text_of).unwrap_or_default()
}

fn field_bool(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).map(Value::is_truthy).unwrap_or(default)
}

/// Texte d'une valeur ; un nombre est converti comme le ferait `str()`.
fn text_of(v: &Value) -> String {
    match v {
        Value::Str(s) | Value::Unicode(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::BigInt(s) => s.clone(),
        Value::Float(f) => f.to_string(),
        _ => String::new(),
    }
}

fn i64_list(v: Option<&Value>) -> Vec<i64> {
    v.and_then(Value::as_seq)
        .map(|items| items.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

fn f64_list(v: Option<&Value>) -> Vec<f64> {
    v.and_then(Value::as_seq)
        .map(|items| items.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect())
        .unwrap_or_default()
}

/// Date `(a, m, j)` ; `(0, 0, 0)` si absente ou mal formée.
fn ymd(v: Option<&Value>) -> (i32, u32, u32) {
    let Some(items) = v.and_then(Value::as_seq) else {
        return (0, 0, 0);
    };
    let part = |i: usize| items.get(i).and_then(Value::as_i64).unwrap_or(0);
    let (y, m, d) = (part(0), part(1), part(2));
    (
        i32::try_from(y).unwrap_or(0),
        u32::try_from(m).unwrap_or(0),
        u32::try_from(d).unwrap_or(0),
    )
}

// ---------------------------------------------------------------------------
// Échéancier
// ---------------------------------------------------------------------------

/// Échéance telle que stockée par `scheduler.py` (clé `fabiobpscheduler`).
///
/// Les valeurs par défaut sont celles qu'appliquent les scripts lorsqu'une clé
/// manque (`Rec.get(…)` dans `edit_rec` et `scheduler_auto.py`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyScheduleEntry {
    /// Index réel du compte (-1 : aucun).
    pub account: i64,
    /// Index réel du compte lié pour un transfert, -1 sinon.
    pub transfer: i64,
    pub tiers: String,
    pub details: String,
    /// Index dans `["Carte", "Retrait DAB", "Chèque émis", "Prélèvement",
    /// "Virement émis", "TIP", "Versement", "Dépôt de chèque", "Virement reçu"]`.
    pub mode: i64,
    /// Identifiant de catégorie, -1 : sans catégorie.
    pub categ: i64,
    /// Montant signé.
    pub montant: f64,
    /// Prochaine échéance `(a, m, j)`.
    pub nextdate: (i32, u32, u32),
    /// Dernière échéance (incluse), `(0, 0, 0)` s'il n'y en a pas.
    pub enddate: (i32, u32, u32),
    pub delta: i64,
    /// 0 jours, 1 mois, autre valeur : années.
    pub deltaunit: i64,
    pub autoinsert: bool,
    /// Clé `Postpone` : lettres `S` (samedi), `D` (dimanche), `F` (férié).
    pub postpone: Option<String>,
    /// Clé `shift` des anciennes versions (même codage que `Postpone`).
    pub shift: Option<String>,
}

impl Default for LegacyScheduleEntry {
    fn default() -> Self {
        Self {
            account: -1,
            transfer: -1,
            tiers: String::new(),
            details: String::new(),
            mode: 3,
            categ: -1,
            montant: 0.0,
            nextdate: (0, 0, 0),
            enddate: (0, 0, 0),
            delta: 1,
            deltaunit: 1,
            autoinsert: true,
            postpone: None,
            shift: None,
        }
    }
}

impl LegacyScheduleEntry {
    pub fn from_pickle(v: &Value) -> Self {
        let d = Self::default();
        Self {
            account: field_i64(v, "account", d.account),
            transfer: field_i64(v, "transfer", d.transfer),
            tiers: field_str(v, "tiers"),
            details: field_str(v, "details"),
            mode: field_i64(v, "mode", d.mode),
            categ: field_i64(v, "categ", d.categ),
            montant: field_f64(v, "montant", d.montant),
            nextdate: ymd(v.get("nextdate")),
            enddate: ymd(v.get("enddate")),
            delta: field_i64(v, "delta", d.delta),
            deltaunit: field_i64(v, "deltaunit", d.deltaunit),
            autoinsert: field_bool(v, "autoinsert", d.autoinsert),
            postpone: v.get("Postpone").map(text_of),
            shift: v.get("shift").map(text_of),
        }
    }

    /// Lettres de report effectives : `Postpone`, à défaut l'ancien `shift`.
    pub fn effective_postpone(&self) -> &str {
        self.postpone
            .as_deref()
            .or(self.shift.as_deref())
            .unwrap_or("")
    }
}

/// Liste d'échéances (`fabiobpscheduler`, `echeancier`, fichiers `.bpsched`).
/// Les éléments qui ne sont pas des dictionnaires sont ignorés.
pub fn schedule_entries(v: &Value) -> Vec<LegacyScheduleEntry> {
    v.as_seq()
        .map(|items| {
            items
                .iter()
                .filter(|item| item.as_dict().is_some())
                .map(LegacyScheduleEntry::from_pickle)
                .collect()
        })
        .unwrap_or_default()
}

/// Échéances du fichier `file_name` (nom sans chemin, ex. `Exemple.bp`) dans
/// l'ancien fichier externe `scheduler.dat` (`{nom: [échéances]}`).
pub fn entries_from_scheduler_dat(
    data: &[u8],
    file_name: &str,
) -> Result<Vec<LegacyScheduleEntry>, PickleError> {
    let value = parse(data)?;
    Ok(value
        .get(file_name)
        .map(schedule_entries)
        .unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Autres fonctionnalités
// ---------------------------------------------------------------------------

/// Budget mensuel (`fabiobpbudget`) : montant signé par identifiant de catégorie.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LegacyBudget(pub BTreeMap<i64, f64>);

impl LegacyBudget {
    pub fn from_pickle(v: &Value) -> Self {
        Self(
            v.as_dict()
                .unwrap_or_default()
                .iter()
                .filter_map(|(k, v)| Some((k.as_i64()?, v.as_f64().unwrap_or(0.0))))
                .collect(),
        )
    }
}

/// Ligne d'un tableau de budget annuel.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyBudgetLine {
    pub title: String,
    /// Réalisé calculé à partir des catégories (sinon saisi à la main).
    pub autocalc_real: bool,
    pub categs: Vec<i64>,
    /// Budget de chaque mois (12 valeurs).
    pub estimated_values: Vec<f64>,
    /// Réalisé saisi de chaque mois (12 valeurs).
    pub real_values: Vec<f64>,
}

/// Tableau de budget annuel (`fabiobpbudgetyear` est une liste de tableaux).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyBudgetYear {
    pub name: String,
    pub year: i64,
    /// Index réels des comptes pris en compte ; `None` : compte courant.
    pub accounts: Option<Vec<i64>>,
    pub lines: Vec<LegacyBudgetLine>,
}

impl LegacyBudgetYear {
    pub fn from_pickle(v: &Value) -> Self {
        let pad12 = |mut values: Vec<f64>| {
            values.resize(12, 0.0);
            values
        };
        Self {
            name: field_str(v, "name"),
            year: field_i64(v, "year", 0),
            accounts: v.get("accounts").map(|a| i64_list(Some(a))),
            lines: v
                .get("lines")
                .and_then(Value::as_seq)
                .unwrap_or_default()
                .iter()
                .map(|line| LegacyBudgetLine {
                    title: field_str(line, "title"),
                    autocalc_real: field_bool(line, "autocalc_real", false),
                    categs: i64_list(line.get("categs")),
                    estimated_values: pad12(f64_list(line.get("estimated_values"))),
                    real_values: pad12(f64_list(line.get("real_values"))),
                })
                .collect(),
        }
    }

    /// Liste complète de la clé `fabiobpbudgetyear`.
    pub fn list_from_pickle(v: &Value) -> Vec<Self> {
        v.as_seq()
            .unwrap_or_default()
            .iter()
            .map(Self::from_pickle)
            .collect()
    }
}

/// Filtre prédéfini (`fabiobpautofilters` : dict nom → filtre).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyFilterRule {
    /// Date de début, avec les dates dynamiques (`1/m/a`, `dj/m-1/a`…).
    pub dt1: String,
    pub dt2: String,
    /// Mode, tiers, catégorie, montant : textes de filtre.
    pub mde: String,
    pub who: String,
    pub ctg: String,
    pub val: String,
    /// Pointage : 4 toutes, 3 pointées et rapprochées, 2 rapprochées,
    /// 1 pointées, 0 non pointées.
    pub mrk: i64,
}

impl LegacyFilterRule {
    pub fn from_pickle(v: &Value) -> Self {
        Self {
            dt1: field_str(v, "dt1"),
            dt2: field_str(v, "dt2"),
            mde: field_str(v, "mde"),
            who: field_str(v, "who"),
            ctg: field_str(v, "ctg"),
            val: field_str(v, "val"),
            mrk: field_i64(v, "mrk", 4),
        }
    }

    /// Dictionnaire complet, trié par nom.
    pub fn map_from_pickle(v: &Value) -> BTreeMap<String, Self> {
        v.as_dict()
            .unwrap_or_default()
            .iter()
            .map(|(k, v)| (text_of(k), Self::from_pickle(v)))
            .collect()
    }
}

/// Règle de calcul automatique des champs (`fabiobpautofields`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyAutoField {
    pub active: bool,
    /// Champ testé : 0 tiers, 1 détails, 2 tiers ou détails.
    pub field: i64,
    /// Texte recherché, ou expression Python si elle commence par `=`.
    pub contains: String,
    /// Plusieurs mots séparés : l'un ou l'autre suffit.
    pub contains_or: bool,
    pub do_mode: bool,
    /// Index de mode (0–9).
    pub mode: i64,
    pub do_third: bool,
    pub third: String,
    pub do_info: bool,
    pub info: String,
    pub do_categ: bool,
    pub categ: i64,
    /// N'affecte la catégorie que si la ligne n'en a pas.
    pub emptycateg: bool,
    pub do_date: bool,
    pub date: String,
    /// Appliquée à chaque nouvelle ligne saisie.
    pub withnewline: bool,
}

impl LegacyAutoField {
    pub fn from_pickle(v: &Value) -> Self {
        Self {
            active: field_bool(v, "active", false),
            field: field_i64(v, "field", 0),
            contains: field_str(v, "contains"),
            contains_or: field_bool(v, "contains_or", false),
            do_mode: field_bool(v, "do_mode", false),
            mode: field_i64(v, "mode", 0),
            do_third: field_bool(v, "do_third", false),
            third: field_str(v, "third"),
            do_info: field_bool(v, "do_info", false),
            info: field_str(v, "info"),
            do_categ: field_bool(v, "do_categ", false),
            categ: field_i64(v, "categ", -1),
            emptycateg: field_bool(v, "emptycateg", false),
            do_date: field_bool(v, "do_date", false),
            date: field_str(v, "date"),
            withnewline: field_bool(v, "withnewline", false),
        }
    }

    pub fn list_from_pickle(v: &Value) -> Vec<Self> {
        v.as_seq()
            .unwrap_or_default()
            .iter()
            .map(Self::from_pickle)
            .collect()
    }
}

/// Abréviations de la saisie multiple (`fabiobpabbreviations`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LegacyAbbreviations(pub BTreeMap<String, String>);

impl LegacyAbbreviations {
    pub fn from_pickle(v: &Value) -> Self {
        Self(
            v.as_dict()
                .unwrap_or_default()
                .iter()
                .map(|(k, v)| (text_of(k), text_of(v)))
                .collect(),
        )
    }
}

/// Derniers choix de la fenêtre de transfert (`fabiobptransfer`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyTransferSettings {
    /// 0 virement, 1 chèque, 2 espèces.
    pub mode: i64,
    pub categ: i64,
    pub details: String,
    /// Index réels des comptes source et destination (-1 : non choisi).
    pub account1: i64,
    pub account2: i64,
}

impl Default for LegacyTransferSettings {
    fn default() -> Self {
        Self {
            mode: 0,
            categ: -1,
            details: String::new(),
            account1: -1,
            account2: -1,
        }
    }
}

impl LegacyTransferSettings {
    pub fn from_pickle(v: &Value) -> Self {
        Self {
            mode: field_i64(v, "mode", 0),
            categ: field_i64(v, "categ", -1),
            details: field_str(v, "details"),
            account1: field_i64(v, "account1", -1),
            account2: field_i64(v, "account2", -1),
        }
    }
}

/// Comptes affichés par le relevé multicompte (`fabiobpviewall`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyViewAll {
    pub accounts: Vec<i64>,
}

impl LegacyViewAll {
    pub fn from_pickle(v: &Value) -> Self {
        Self {
            accounts: i64_list(v.get("accounts")),
        }
    }
}

/// Donnée `bpcashcards` : numéro de carte → valeurs brutes (sens inconnu).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LegacyCashCards(pub BTreeMap<String, Vec<Value>>);

impl LegacyCashCards {
    pub fn from_pickle(v: &Value) -> Self {
        Self(
            v.as_dict()
                .unwrap_or_default()
                .iter()
                .map(|(k, v)| (text_of(k), v.as_seq().unwrap_or_default().to_vec()))
                .collect(),
        )
    }
}

/// Adresse de `fabiobpmailaddress`, stockée en texte brut (pas en pickle).
pub fn legacy_mail_address(document: &Document) -> Option<String> {
    feature_bytes(document, "fabiobpmailaddress").map(decode_cp1252)
}

// ---------------------------------------------------------------------------
// Passerelle vers les valeurs génériques des fonctionnalités
// ---------------------------------------------------------------------------

impl Value {
    /// Conversion vers la valeur générique lue par les fonctionnalités
    /// (budget, filtres, règles) : chaînes `str`/`unicode` confondues, entiers
    /// longs en entier s'ils tiennent (sinon en texte), objets ignorés.
    pub fn to_py(&self) -> crate::pyvalue::PyValue {
        use crate::pyvalue::PyValue;
        match self {
            Value::None | Value::Global { .. } | Value::Object { .. } => PyValue::None,
            Value::Bool(b) => PyValue::Bool(*b),
            Value::Int(i) => PyValue::Int(*i),
            Value::BigInt(text) => text.parse().map_or_else(|_| PyValue::Str(text.clone()), PyValue::Int),
            Value::Float(f) => PyValue::Float(*f),
            Value::Str(s) | Value::Unicode(s) => PyValue::Str(s.clone()),
            Value::Bytes(b) => PyValue::Str(encoding_rs::WINDOWS_1252.decode(b).0.into_owned()),
            Value::List(items) => PyValue::List(items.iter().map(Value::to_py).collect()),
            Value::Tuple(items) => PyValue::Tuple(items.iter().map(Value::to_py).collect()),
            Value::Dict(pairs) => PyValue::Dict(pairs.iter().map(|(k, v)| (k.to_py(), v.to_py())).collect()),
        }
    }
}

/// Décode un ancien pickle en valeur générique (`None` s'il est illisible) :
/// c'est le `parse_pickle` attendu par les `load` des fonctionnalités.
pub fn parse_py(bytes: &[u8]) -> Option<crate::pyvalue::PyValue> {
    parse(bytes).ok().map(|v| v.to_py())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// `bp_script_data_echeancier` d'`Exemple.bp`, après `SimpleDecode`.
    pub(crate) const EXEMPLE_ECHEANCIER: &[u8] = b"(lp1\n(dp2\nS'categ'\np3\nI13\nsS'tiers'\np4\nS'Coheris'\np5\nsS'enddate'\np6\n(I0\nI0\nI0\ntp7\nsS'montant'\np8\nF-123\nsS'transfer'\np9\nI-1\nsS'delta'\np10\nI1\nsS'account'\np11\nI0\nsS'autoinsert'\np12\nI1\nsS'nextdate'\np13\n(I2009\nI4\nI17\ntp14\nsS'details'\np15\nS'paye supp'\np16\nsS'deltaunit'\np17\nI2\nsS'Postpone'\np18\nS'F'\nsS'shift'\np19\nS'DF'\np20\nsS'mode'\np21\nI3\nsa.";
    const EXEMPLE_SCHEDULER: &[u8] = b"(lp1\n.";
    const EXEMPLE_BUDGET: &[u8] = b"(dp1\nI24\nF-12.16\ns.";
    const EXEMPLE_AUTOFIELDS: &[u8] = b"(lp1\n(dp2\nS'info'\np3\nS''\nsS'categ'\np4\nI9\nsS'do_mode'\np5\nI00\nsS'third'\np6\nS''\nsS'do_third'\np7\nI00\nsS'contains'\np8\nS'=\"tresor public\" in tiers and montant < -70 and montant > -100'\np9\nsS'emptycateg'\np10\nI01\nsS'field'\np11\nI0\nsS'mode'\np12\nI0\nsS'do_categ'\np13\nI01\nsS'do_info'\np14\nI00\nsS'active'\np15\nI01\nsS'contains_or'\np16\nI00\nsS'withnewline'\np17\nI01\nsa(dp18\ng3\nS''\nsg4\nI5\nsg5\nI00\nsg6\nS''\nsg7\nI00\nsg8\nS'=\"gaz de france\" in tiers or \"electricite de france\" in tiers'\np19\nsg10\nI01\nsg11\nI0\nsg12\nI0\nsg13\nI01\nsg14\nI00\nsg15\nI01\nsg16\nI00\nsg17\nI01\nsa(dp20\nS'info'\np21\nS''\nsS'categ'\np22\nI-1\nsS'do_mode'\np23\nI00\nsS'third'\np24\nS\"=tiers.replace(mode[4:], '').strip()\"\np25\nsS'do_third'\np26\nI01\nsS'contains'\np27\nS'=\"chq \" in mode and mode[4:] in tiers'\np28\nsS'emptycateg'\np29\nI00\nsS'field'\np30\nI0\nsS'mode'\np31\nI0\nsS'do_categ'\np32\nI00\nsS'do_info'\np33\nI00\nsS'active'\np34\nI01\nsS'contains_or'\np35\nI00\nsS'withnewline'\np36\nI01\nsa(dp37\nS'info'\np38\nS'Pr\\xe9l\\xe8vement \\xe9lectricit\\xe9'\np39\nsS'categ'\np40\nI18\nsS'do_mode'\np41\nI00\nsS'third'\np42\nS''\nsS'do_third'\np43\nI00\nsS'contains'\np44\nS'EDF'\np45\nsS'emptycateg'\np46\nI01\nsS'field'\np47\nI0\nsS'mode'\np48\nI0\nsS'do_categ'\np49\nI01\nsS'do_info'\np50\nI01\nsS'active'\np51\nI01\nsS'contains_or'\np52\nI00\nsS'withnewline'\np53\nI01\nsa(dp54\nS'info'\np55\nS'SANS CATEGORIE'\np56\nsS'categ'\np57\nI-1\nsS'do_mode'\np58\nI00\nsS'do_date'\np59\nI01\nsS'third'\np60\nS''\nsS'do_third'\np61\nI00\nsS'contains'\np62\nS'=categ==\"\"'\np63\nsS'emptycateg'\np64\nI00\nsS'field'\np65\nI0\nsS'mode'\np66\nI0\nsS'do_categ'\np67\nI00\nsS'do_info'\np68\nI01\nsS'active'\np69\nI01\nsS'date'\np70\nS'13/10/2010'\np71\nsS'contains_or'\np72\nI00\nsS'withnewline'\np73\nI00\nsa.";
    const EXEMPLE_AUTOFILTERS: &[u8] = b"(dp1\n.";
    const EXEMPLE_TRANSFER: &[u8] = b"(dp1\nS'categ'\np2\nI6\nsS'account2'\np3\nI2\nsS'mode'\np4\nI1\nsS'account1'\np5\nI0\nsS'details'\np6\nS'test'\np7\ns.";
    const EXEMPLE_BUDGETYEAR: &[u8] = b"(lp1\n(dp2\nS'accounts'\np3\n(lp4\nI0\naI4\naI2\naI1\naI3\nasS'lines'\np5\n(lp6\n(dp7\nS'autocalc_real'\np8\nI1\nsS'categs'\np9\n(lp10\nI0\naI1\naI48\naI2\nasS'estimated_values'\np11\n(lp12\nF-550\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\nasS'real_values'\np13\n(lp14\nF-3.8100000000000001\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\nasS'title'\np15\nS'Alimentation'\np16\nsa(dp17\ng8\nI0\nsg9\n(lp18\nsg11\n(lp19\nF-400\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\nasg13\n(lp20\nF-350.36000000000001\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\naI0\nasg15\nS'Autres'\np21\nsasS'name'\np22\nS'Test de budget'\np23\nsS'year'\np24\nI2010\nsa.";
    const EXEMPLE_VIEWALL: &[u8] = b"(dp1\nS'accounts'\np2\n(lp3\nI1\naI3\nas.";
    const EXEMPLE_CASHCARDS: &[u8] =
        b"(dp1\nS'4974134605534941'\np2\n(S''\nI0\nI2\nI1\nI18\nI0\ntp3\ns.";

    fn s(text: &str) -> Value {
        Value::Str(text.to_string())
    }

    #[test]
    fn exemple_echeancier() {
        let value = parse(EXEMPLE_ECHEANCIER).unwrap();
        let entries = schedule_entries(&value);
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.account, 0);
        assert_eq!(e.transfer, -1);
        assert_eq!(e.tiers, "Coheris");
        assert_eq!(e.details, "paye supp");
        assert_eq!(e.mode, 3);
        assert_eq!(e.categ, 13);
        assert_eq!(e.montant, -123.0);
        assert_eq!(e.nextdate, (2009, 4, 17));
        assert_eq!(e.enddate, (0, 0, 0));
        assert_eq!(e.delta, 1);
        assert_eq!(e.deltaunit, 2);
        assert!(e.autoinsert);
        assert_eq!(e.postpone.as_deref(), Some("F"));
        assert_eq!(e.shift.as_deref(), Some("DF"));
        assert_eq!(e.effective_postpone(), "F");
    }

    #[test]
    fn exemple_scheduler_vide() {
        assert_eq!(parse(EXEMPLE_SCHEDULER).unwrap(), Value::List(vec![]));
        assert!(schedule_entries(&parse(EXEMPLE_SCHEDULER).unwrap()).is_empty());
        assert_eq!(parse(EXEMPLE_AUTOFILTERS).unwrap(), Value::Dict(vec![]));
    }

    #[test]
    fn exemple_budget() {
        let value = parse(EXEMPLE_BUDGET).unwrap();
        assert_eq!(
            value,
            Value::Dict(vec![(Value::Int(24), Value::Float(-12.16))])
        );
        let budget = LegacyBudget::from_pickle(&value);
        assert_eq!(budget.0.get(&24), Some(&-12.16));
    }

    #[test]
    fn exemple_autofields_booleens_memo_et_accents() {
        let value = parse(EXEMPLE_AUTOFIELDS).unwrap();
        let rules = LegacyAutoField::list_from_pickle(&value);
        assert_eq!(rules.len(), 5);
        // Le deuxième dict réutilise les clés par le mémo (`g3`, `g4`…).
        assert_eq!(rules[1].categ, 5);
        assert!(rules[1].contains.starts_with("=\"gaz de france\""));
        assert!(rules[1].active && rules[1].do_categ && !rules[1].do_mode);
        assert_eq!(rules[2].third, "=tiers.replace(mode[4:], '').strip()");
        assert_eq!(rules[3].info, "Prélèvement électricité");
        assert_eq!(rules[4].date, "13/10/2010");
        assert!(rules[4].do_date && !rules[4].withnewline);
        assert_eq!(
            value.as_seq().unwrap()[0].get("active"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn exemple_transfer_viewall_budgetyear_cashcards() {
        let transfer = LegacyTransferSettings::from_pickle(&parse(EXEMPLE_TRANSFER).unwrap());
        assert_eq!(
            transfer,
            LegacyTransferSettings {
                mode: 1,
                categ: 6,
                details: "test".into(),
                account1: 0,
                account2: 2
            }
        );

        let view = LegacyViewAll::from_pickle(&parse(EXEMPLE_VIEWALL).unwrap());
        assert_eq!(view.accounts, vec![1, 3]);

        let budgets = LegacyBudgetYear::list_from_pickle(&parse(EXEMPLE_BUDGETYEAR).unwrap());
        assert_eq!(budgets.len(), 1);
        let b = &budgets[0];
        assert_eq!(b.name, "Test de budget");
        assert_eq!(b.year, 2010);
        assert_eq!(b.accounts, Some(vec![0, 4, 2, 1, 3]));
        assert_eq!(b.lines.len(), 2);
        assert!(b.lines[0].autocalc_real);
        assert_eq!(b.lines[0].categs, vec![0, 1, 48, 2]);
        assert_eq!(b.lines[0].estimated_values[0], -550.0);
        assert_eq!(b.lines[0].real_values[0], -3.81);
        assert_eq!(b.lines[1].title, "Autres");
        assert!(b.lines[1].categs.is_empty());
        assert_eq!(b.lines[1].real_values.len(), 12);
        assert_eq!(b.lines[1].real_values[0], -350.36);

        let cards = LegacyCashCards::from_pickle(&parse(EXEMPLE_CASHCARDS).unwrap());
        assert_eq!(
            cards.0.get("4974134605534941"),
            Some(&vec![
                s(""),
                Value::Int(0),
                Value::Int(2),
                Value::Int(1),
                Value::Int(18),
                Value::Int(0)
            ])
        );
    }

    #[test]
    fn filtres_et_abreviations() {
        // Pickle protocole 0 écrit par Python 2 pour les valeurs par défaut de
        // setfilters.py (clé accentuée en CP1252) et un dict d'abréviations.
        let data = b"(dp1\nS'Mois pr\\xe9c\\xe9dent'\np2\n(dp3\nS'mrk'\np4\nI4\nsS'dt1'\np5\nS'1/m-1/a'\np6\nsS'dt2'\np7\nS'dj/m-1/a'\np8\nss.";
        let filters = LegacyFilterRule::map_from_pickle(&parse(data).unwrap());
        let rule = &filters["Mois précédent"];
        assert_eq!(rule.dt1, "1/m-1/a");
        assert_eq!(rule.dt2, "dj/m-1/a");
        assert_eq!(rule.mrk, 4);
        assert_eq!(rule.who, "");

        let data = b"(dp1\nS'edf'\np2\nS'\\xc9lectricit\\xe9 de France'\np3\ns.";
        let abbr = LegacyAbbreviations::from_pickle(&parse(data).unwrap());
        assert_eq!(abbr.0["edf"], "Électricité de France");
    }

    #[test]
    fn protocole_0_types_divers() {
        let data = b"(lp0\nNaI01\naI00\naL12345678901234567890L\naL42L\naF1.#INF\naF-1.5\naV\\u20ac \xe9\naS\"it's\"\naS'\\x80\\n\\t\\\\'\na(I1\nI2\ntp1\nag1\na.";
        let v = parse(data).unwrap();
        assert_eq!(
            v,
            Value::List(vec![
                Value::None,
                Value::Bool(true),
                Value::Bool(false),
                Value::BigInt("12345678901234567890".into()),
                Value::Int(42),
                Value::Float(f64::INFINITY),
                Value::Float(-1.5),
                Value::Unicode("€ \u{e9}".into()),
                s("it's"),
                s("€\n\t\\"),
                Value::Tuple(vec![Value::Int(1), Value::Int(2)]),
                Value::Tuple(vec![Value::Int(1), Value::Int(2)]),
            ])
        );
    }

    #[test]
    fn protocole_0_fins_de_ligne_windows() {
        // Fichier `.bpsched` écrit en mode texte sous Windows.
        let data = b"(lp1\r\n(dp2\r\nS'tiers'\r\np3\r\nS'Loyer'\r\np4\r\nsa.";
        let entries = schedule_entries(&parse(data).unwrap());
        assert_eq!(entries[0].tiers, "Loyer");
    }

    #[test]
    fn memo_partage_puis_modifie() {
        // La liste interne est mémorisée vide (`p1`) puis remplie : la
        // référence `g1` doit voir le contenu final, comme en Python.
        let data = b"(lp0\n(lp1\nI5\naag1\na.";
        let v = parse(data).unwrap();
        let inner = Value::List(vec![Value::Int(5)]);
        assert_eq!(v, Value::List(vec![inner.clone(), inner]));
    }

    #[test]
    fn protocole_1_binaire() {
        // Équivalent protocole 1 (cPickle Python 2, `scheduler.dat`) de
        // {'Exemple.bp': [{'tiers': 'Loyer', 'montant': -650.5, 'nextdate': (2011, 1, 31), 'delta': 1}]}
        let mut data = vec![b'}', b'q', 1, b'(', b'U', 10];
        data.extend_from_slice(b"Exemple.bp");
        data.extend_from_slice(&[b'q', 2, b']', b'q', 3, b'}', b'q', 4, b'(']);
        data.extend_from_slice(&[b'U', 5]);
        data.extend_from_slice(b"tiers");
        data.extend_from_slice(&[b'U', 5]);
        data.extend_from_slice(b"Loyer");
        data.extend_from_slice(&[b'U', 7]);
        data.extend_from_slice(b"montant");
        data.push(b'G');
        data.extend_from_slice(&(-650.5f64).to_be_bytes());
        data.extend_from_slice(&[b'U', 8]);
        data.extend_from_slice(b"nextdate");
        data.extend_from_slice(b"(M");
        data.extend_from_slice(&2011u16.to_le_bytes());
        data.extend_from_slice(&[b'K', 1, b'K', 31, b't']);
        data.extend_from_slice(&[b'U', 5]);
        data.extend_from_slice(b"delta");
        data.push(b'J');
        data.extend_from_slice(&1i32.to_le_bytes());
        data.extend_from_slice(b"uau.");

        let entries = entries_from_scheduler_dat(&data, "Exemple.bp").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tiers, "Loyer");
        assert_eq!(entries[0].montant, -650.5);
        assert_eq!(entries[0].nextdate, (2011, 1, 31));
        assert_eq!(entries[0].delta, 1);
        assert!(
            entries_from_scheduler_dat(&data, "Autre.bp")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn protocole_2_binaire() {
        // pickle.dumps((True, None, -1, 300, -256L), protocol=2) en Python 2 :
        // NEWTRUE, NONE, BININT, BININT2, LONG1 (complément à deux).
        let mut data = vec![0x80, 2, b'(', 0x88, b'N', b'J'];
        data.extend_from_slice(&(-1i32).to_le_bytes());
        data.extend_from_slice(&[b'M', 0x2c, 0x01, 0x8a, 2, 0x00, 0xff, b't', b'.']);
        assert_eq!(
            parse(&data).unwrap(),
            Value::Tuple(vec![
                Value::Bool(true),
                Value::None,
                Value::Int(-1),
                Value::Int(300),
                Value::Int(-256),
            ])
        );
        let mut d = vec![0x80, 2, b'K', 1, b'K', 2, 0x86, b'X'];
        d.extend_from_slice(&3u32.to_le_bytes());
        d.extend_from_slice("€".as_bytes());
        d.extend_from_slice(&[0x86, b'.']);
        assert_eq!(
            parse(&d).unwrap(),
            Value::Tuple(vec![
                Value::Tuple(vec![Value::Int(1), Value::Int(2)]),
                Value::Unicode("€".into())
            ])
        );
    }

    #[test]
    fn objets_conserves() {
        let data = b"cdatetime\ndate\np0\n(S'\\x07\\xdb\\x01\\x1f'\ntRp1\n.";
        let v = parse(data).unwrap();
        match v {
            Value::Object { class, args, .. } => {
                assert_eq!(
                    *class,
                    Value::Global {
                        module: "datetime".into(),
                        name: "date".into()
                    }
                );
                assert!(args.as_seq().is_some());
            }
            other => panic!("objet attendu : {other:?}"),
        }
    }

    #[test]
    fn erreurs_sans_panique() {
        for data in [
            &b""[..],
            b"(lp1\n",
            b"(",
            b"I12",
            b"S'abc\n.",
            b"a.",
            b"g9\n.",
            b"T\xff\xff\xff\xff",
            b"\x8a\x20",
            b"Z.",
            b"(dp1\nI1\ns.",
        ] {
            assert!(parse(data).is_err(), "{data:?}");
        }
        // Structure récursive : une liste qui se contient elle-même.
        assert!(parse(b"(lp0\ng0\na.").is_err());
    }

    #[test]
    fn conversion_tolerante() {
        // Champs manquants ou de mauvais type → valeurs par défaut du script.
        let data = b"(lp1\n(dp2\nS'tiers'\np3\nI5\nsS'montant'\np4\nI-40\nsS'nextdate'\np5\nS'hier'\nsaI3\na.";
        let entries = schedule_entries(&parse(data).unwrap());
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.tiers, "5");
        assert_eq!(e.montant, -40.0);
        assert_eq!(e.nextdate, (0, 0, 0));
        assert_eq!(e.account, -1);
        assert_eq!(e.transfer, -1);
        assert_eq!(e.mode, 3);
        assert_eq!(e.deltaunit, 1);
        assert!(e.autoinsert);
        assert_eq!(e.effective_postpone(), "");
        // Une valeur qui n'est pas une liste donne une liste vide.
        assert!(schedule_entries(&Value::Int(1)).is_empty());
    }

    #[test]
    fn serde_aller_retour() {
        let entries = schedule_entries(&parse(EXEMPLE_ECHEANCIER).unwrap());
        let json = serde_json::to_string(&entries).unwrap();
        let back: Vec<LegacyScheduleEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, entries);
        // Champs absents → défaut.
        let partial: LegacyScheduleEntry = serde_json::from_str(r#"{"tiers":"x"}"#).unwrap();
        assert_eq!(partial.account, -1);
        assert_eq!(partial.mode, 3);
    }

    #[test]
    fn donnees_du_document() {
        use crate::model::FeatureData;
        let mut doc = crate::scheduler::tests::sample_document();
        doc.feature_data = vec![
            FeatureData {
                key: "fabiobpmailaddress".into(),
                value: b"moi@exemple.fr".to_vec(),
            },
            FeatureData {
                key: "fabiobpviewall".into(),
                value: EXEMPLE_VIEWALL.to_vec(),
            },
            FeatureData {
                key: "vide".into(),
                value: Vec::new(),
            },
        ];
        assert_eq!(legacy_mail_address(&doc).as_deref(), Some("moi@exemple.fr"));
        let view = feature_pickle(&doc, "fabiobpviewall").unwrap().unwrap();
        assert_eq!(LegacyViewAll::from_pickle(&view).accounts, vec![1, 3]);
        assert!(feature_pickle(&doc, "vide").is_none());
        assert!(feature_pickle(&doc, "absente").is_none());
    }
}
