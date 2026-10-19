//! Évaluateur sûr d'expressions de style Python.
//!
//! Les anciens scripts évaluaient avec `eval()` les formules saisies par
//! l'utilisateur : conditions et actions du « Calcul automatique des champs »
//! (`="tresor public" in tiers and montant < -70`, `=tiers.replace(mode[4:], '')`),
//! dates dynamiques des filtres prédéfinis (`dj/m-1/a`), montants saisis
//! (`1200/12`). Ce module interprète le sous-ensemble de Python 2 réellement
//! utilisé, **sans jamais exécuter de code arbitraire** :
//!
//! - littéraux : entiers, flottants, chaînes `'…'` / `"…"` (préfixes `u`, `r`,
//!   échappements `\n`, `\xNN`…), `True`, `False`, `None`, listes `[…]` et
//!   n-uplets `(a, b)` ;
//! - opérateurs : `+ - * / // % **`, comparaisons enchaînables
//!   `== != <> < <= > >=`, `in`, `not in`, `and`, `or`, `not`, parenthèses ;
//! - indexation `s[0]` et tranches `s[4:]`, `s[:-2]` ;
//! - fonctions `len str int float abs round min max` et méthodes de chaîne
//!   `lower upper strip lstrip rstrip replace startswith endswith find rfind
//!   count title capitalize split join isdigit`.
//!
//! Comme en Python 2, `/` entre deux entiers est une division entière (vers
//! −∞) : les formules ont été écrites pour cette sémantique. Les tailles sont
//! bornées (longueur de la source, profondeur d'imbrication, longueur des
//! chaînes produites) pour qu'une formule ne puisse ni boucler ni épuiser la
//! mémoire.

use std::cmp::Ordering;
use std::fmt;

use crate::tr;

/// Longueur maximale d'une formule, en caractères.
pub const MAX_SOURCE_LEN: usize = 4_000;
/// Profondeur maximale d'imbrication (parenthèses, opérateurs unaires…).
const MAX_DEPTH: usize = 100;
/// Longueur maximale d'une chaîne produite par l'évaluation.
const MAX_STRING_LEN: usize = 100_000;

/// Valeur manipulée par une formule.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
}

impl Value {
    /// Vérité au sens de Python : `0`, `0.0`, `""`, `[]`, `None` et `False`
    /// sont faux.
    pub fn truthy(&self) -> bool {
        match self {
            Value::None => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
        }
    }

    /// Valeur entière (les booléens valent 0 ou 1).
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Bool(b) => Some(i64::from(*b)),
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_number(&self) -> Option<Number> {
        match self {
            Value::Bool(b) => Some(Number::Int(i64::from(*b))),
            Value::Int(i) => Some(Number::Int(*i)),
            Value::Float(f) => Some(Number::Float(*f)),
            _ => None,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Value::None => "None",
            Value::Bool(_) => tr!("booléen"),
            Value::Int(_) => tr!("entier"),
            Value::Float(_) => tr!("nombre décimal"),
            Value::Str(_) => tr!("chaîne"),
            Value::List(_) => tr!("liste"),
        }
    }

    /// Représentation `repr()` (chaînes entre apostrophes).
    fn repr(&self) -> String {
        match self {
            Value::Str(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
            other => other.to_string(),
        }
    }
}

/// Conversion en texte, comme `str()` en Python 2.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => f.write_str("None"),
            Value::Bool(true) => f.write_str("True"),
            Value::Bool(false) => f.write_str("False"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => f.write_str(&format_float(*x)),
            Value::Str(s) => f.write_str(s),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(Value::repr).collect();
                write!(f, "[{}]", parts.join(", "))
            }
        }
    }
}

/// `str(float)` de Python 2 : 12 chiffres significatifs, `.0` pour un entier.
fn format_float(x: f64) -> String {
    if !x.is_finite() {
        return if x.is_nan() {
            "nan".into()
        } else if x > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    if x == 0.0 {
        return "0.0".into();
    }
    let sci = format!("{x:.11e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if (-4..12).contains(&exponent) {
        let decimals = (11 - exponent).max(0) as usize;
        let fixed = format!("{x:.decimals$}");
        let trimmed = if fixed.contains('.') {
            fixed.trim_end_matches('0').trim_end_matches('.')
        } else {
            &fixed
        };
        if trimmed.contains('.') {
            trimmed.to_string()
        } else {
            format!("{trimmed}.0")
        }
    } else {
        let mantissa = if mantissa.contains('.') {
            mantissa.trim_end_matches('0').trim_end_matches('.')
        } else {
            mantissa
        };
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", exponent.abs())
    }
}

#[derive(Debug, Clone, Copy)]
enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    fn as_f64(self) -> f64 {
        match self {
            Number::Int(i) => i as f64,
            Number::Float(f) => f,
        }
    }
}

/// Erreur d'analyse ou d'évaluation d'une formule.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormulaError {
    Syntax { position: usize, message: String },
    Eval(String),
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { position, message } => f.write_str(&tr!(
                "erreur de syntaxe (caractère {position}) : {message}",
                position = position,
                message = message
            )),
            Self::Eval(message) => f.write_str(message),
        }
    }
}

fn syntax<T>(position: usize, message: impl Into<String>) -> Result<T, FormulaError> {
    Err(FormulaError::Syntax {
        position,
        message: message.into(),
    })
}

fn eval_err<T>(message: impl Into<String>) -> Result<T, FormulaError> {
    Err(FormulaError::Eval(message.into()))
}

/// Formule analysée, prête à être évaluée autant de fois que nécessaire.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    source: String,
    ast: Expr,
}

impl Formula {
    /// Analyse une formule (sans le `=` initial des règles).
    pub fn parse(source: &str) -> Result<Self, FormulaError> {
        if source.chars().count() > MAX_SOURCE_LEN {
            return syntax(MAX_SOURCE_LEN, tr!("formule trop longue"));
        }
        let tokens = tokenize(source)?;
        let mut parser = Parser {
            tokens,
            pos: 0,
            depth: 0,
        };
        let ast = parser.expression()?;
        if let Some(token) = parser.peek() {
            return syntax(
                token.position,
                tr!("élément inattendu après la fin de l'expression"),
            );
        }
        Ok(Self {
            source: source.to_string(),
            ast,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Variante où tous les entiers littéraux sont des flottants : `/` devient
    /// une vraie division (`100/3` = 33,33…). Sert à la saisie des montants,
    /// où la division entière de Python 2 n'avait rien de voulu.
    pub fn with_float_arithmetic(mut self) -> Self {
        self.ast.floatify();
        self
    }

    /// Noms des variables utilisées, sans doublon, dans l'ordre d'apparition.
    pub fn variables(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.ast.collect_variables(&mut out);
        out
    }

    /// Évalue la formule ; `variables` renvoie la valeur d'un nom, ou `None`
    /// s'il est inconnu.
    pub fn eval(&self, variables: &dyn Fn(&str) -> Option<Value>) -> Result<Value, FormulaError> {
        self.ast.eval(variables)
    }
}

// ---------------------------------------------------------------------------
// Analyse lexicale

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    Op(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    position: usize,
}

const OPERATORS: [&str; 24] = [
    "**", "//", "==", "!=", "<>", "<=", ">=", "(", ")", "[", "]", ",", ":", ".", "+", "-", "*",
    "/", "%", "<", ">", "{", "}", "=",
];

fn tokenize(source: &str) -> Result<Vec<Token>, FormulaError> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        // Chaîne, éventuellement préfixée (u, r, ur).
        let prefix_len = string_prefix_len(&chars[i..]);
        if let Some(prefix_len) = prefix_len {
            let prefix: String = chars[i..i + prefix_len].iter().collect::<String>();
            let prefix = prefix.to_lowercase();
            i += prefix_len;
            let (text, next) = lex_string(&chars, i, prefix.contains('u'), prefix.contains('r'))?;
            i = next;
            tokens.push(Token {
                kind: TokenKind::Str(text),
                position: start,
            });
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) {
            let (kind, next) = lex_number(&chars, i)?;
            i = next;
            tokens.push(Token {
                kind,
                position: start,
            });
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Ident(chars[start..i].iter().collect()),
                position: start,
            });
            continue;
        }
        let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
        match OPERATORS.iter().find(|op| rest.starts_with(**op)) {
            Some(op) if *op == "{" || *op == "}" || *op == "=" => {
                return syntax(start, tr!("caractère « {op} » non autorisé", op = op));
            }
            Some(op) => {
                i += op.chars().count();
                tokens.push(Token {
                    kind: TokenKind::Op(op),
                    position: start,
                });
            }
            None => return syntax(start, tr!("caractère « {c} » non autorisé", c = c)),
        }
    }
    Ok(tokens)
}

/// Longueur du préfixe de chaîne (`'`, `u'`, `r"`, `ur'`…) au début de `chars`.
fn string_prefix_len(chars: &[char]) -> Option<usize> {
    let mut n = 0;
    while n < 2
        && chars
            .get(n)
            .is_some_and(|c| matches!(c, 'u' | 'U' | 'r' | 'R'))
    {
        n += 1;
    }
    match chars.get(n) {
        Some('\'' | '"') => Some(n),
        _ => None,
    }
}

fn lex_string(
    chars: &[char],
    start: usize,
    unicode: bool,
    raw: bool,
) -> Result<(String, usize), FormulaError> {
    let quote = chars[start];
    if chars.get(start + 1) == Some(&quote) && chars.get(start + 2) == Some(&quote) {
        return syntax(
            start,
            tr!("les chaînes entre triples guillemets ne sont pas acceptées"),
        );
    }
    let mut out = String::new();
    let mut i = start + 1;
    loop {
        let Some(&c) = chars.get(i) else {
            return syntax(start, tr!("chaîne non terminée"));
        };
        if c == quote {
            return Ok((out, i + 1));
        }
        if c == '\\' {
            let Some(&next) = chars.get(i + 1) else {
                return syntax(start, tr!("chaîne non terminée"));
            };
            if raw {
                out.push('\\');
                out.push(next);
                i += 2;
                continue;
            }
            i += 2;
            match next {
                '\n' => {}
                '\\' => out.push('\\'),
                '\'' => out.push('\''),
                '"' => out.push('"'),
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'a' => out.push('\x07'),
                'b' => out.push('\x08'),
                'f' => out.push('\x0c'),
                'v' => out.push('\x0b'),
                'x' => {
                    let code = hex_value(chars, i, 2)
                        .ok_or_else(|| syntax_error(i, tr!("échappement \\x invalide")))?;
                    i += 2;
                    // Chaîne Python 2 : octet CP1252 ; chaîne unicode : point de code.
                    out.push(if unicode {
                        char::from_u32(code).unwrap_or('\u{fffd}')
                    } else {
                        cp1252_char(code as u8)
                    });
                }
                'u' if unicode => {
                    let code = hex_value(chars, i, 4)
                        .ok_or_else(|| syntax_error(i, tr!("échappement \\u invalide")))?;
                    i += 4;
                    out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                }
                '0'..='7' => {
                    let mut code = next.to_digit(8).unwrap_or(0);
                    let mut n = 1;
                    while n < 3 {
                        match chars.get(i).and_then(|c| c.to_digit(8)) {
                            Some(d) => {
                                code = code * 8 + d;
                                i += 1;
                                n += 1;
                            }
                            None => break,
                        }
                    }
                    out.push(if unicode {
                        char::from_u32(code).unwrap_or('\u{fffd}')
                    } else {
                        cp1252_char((code & 0xff) as u8)
                    });
                }
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
            continue;
        }
        if c == '\n' {
            return syntax(start, tr!("chaîne non terminée"));
        }
        out.push(c);
        i += 1;
    }
}

fn syntax_error(position: usize, message: &str) -> FormulaError {
    FormulaError::Syntax {
        position,
        message: message.into(),
    }
}

fn hex_value(chars: &[char], start: usize, len: usize) -> Option<u32> {
    let digits = chars.get(start..start + len)?;
    digits
        .iter()
        .try_fold(0u32, |acc, c| Some(acc * 16 + c.to_digit(16)?))
}

/// Caractère correspondant à un octet CP1252.
pub(crate) fn cp1252_char(byte: u8) -> char {
    let bytes = [byte];
    let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(&bytes);
    text.chars().next().unwrap_or('\u{fffd}')
}

fn lex_number(chars: &[char], start: usize) -> Result<(TokenKind, usize), FormulaError> {
    let mut i = start;
    let mut is_float = false;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if chars.get(i) == Some(&'.') {
        is_float = true;
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
    }
    if matches!(chars.get(i), Some('e' | 'E')) {
        let mut j = i + 1;
        if matches!(chars.get(j), Some('+' | '-')) {
            j += 1;
        }
        if chars.get(j).is_some_and(char::is_ascii_digit) {
            is_float = true;
            i = j;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    let text: String = chars[start..i].iter().collect();
    // Suffixe `L` des entiers longs de Python 2.
    if !is_float && matches!(chars.get(i), Some('L' | 'l')) {
        i += 1;
    }
    if chars
        .get(i)
        .is_some_and(|c| c.is_alphanumeric() || *c == '_')
    {
        return syntax(i, tr!("nombre mal formé"));
    }
    let kind = if is_float {
        TokenKind::Float(
            text.parse()
                .map_err(|_| syntax_error(start, tr!("nombre mal formé")))?,
        )
    } else {
        TokenKind::Int(
            text.parse()
                .map_err(|_| syntax_error(start, tr!("nombre trop grand")))?,
        )
    };
    Ok((kind, i))
}

// ---------------------------------------------------------------------------
// Analyse syntaxique

#[derive(Debug, Clone, Copy, PartialEq)]
enum UnaryOp {
    Neg,
    Pos,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Mod,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    NotIn,
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Const(Value),
    Var(String),
    List(Vec<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Compare(Box<Expr>, Vec<(CompareOp, Expr)>),
    Index(Box<Expr>, Box<Expr>),
    Slice(Box<Expr>, Option<Box<Expr>>, Option<Box<Expr>>),
    Call(String, Vec<Expr>),
    Method(Box<Expr>, String, Vec<Expr>),
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
}

/// Fonctions autorisées.
pub const FUNCTIONS: [&str; 8] = ["len", "str", "int", "float", "abs", "round", "min", "max"];

/// Méthodes de chaîne autorisées.
pub const METHODS: [&str; 16] = [
    "lower",
    "upper",
    "strip",
    "lstrip",
    "rstrip",
    "replace",
    "startswith",
    "endswith",
    "find",
    "rfind",
    "count",
    "title",
    "capitalize",
    "split",
    "join",
    "isdigit",
];

const KEYWORDS: [&str; 6] = ["and", "or", "not", "in", "is", "lambda"];

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn end_position(&self) -> usize {
        self.tokens
            .last()
            .map_or(0, |t| t.position + 1)
            .max(self.peek().map_or(0, |t| t.position))
    }

    fn position(&self) -> usize {
        self.peek()
            .map_or_else(|| self.end_position(), |t| t.position)
    }

    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Token { kind: TokenKind::Op(o), .. }) if *o == op)
    }

    fn is_keyword(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Token { kind: TokenKind::Ident(w), .. }) if w == word)
    }

    fn is_keyword_at(&self, offset: usize, word: &str) -> bool {
        matches!(self.tokens.get(self.pos + offset), Some(Token { kind: TokenKind::Ident(w), .. }) if w == word)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, op: &str) -> Result<(), FormulaError> {
        if self.eat_op(op) {
            Ok(())
        } else {
            syntax(self.position(), tr!("« {op} » attendu", op = op))
        }
    }

    fn enter(&mut self) -> Result<(), FormulaError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            syntax(self.position(), tr!("expression trop imbriquée"))
        } else {
            Ok(())
        }
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn expression(&mut self) -> Result<Expr, FormulaError> {
        self.enter()?;
        let result = self.or_expr();
        self.leave();
        result
    }

    fn or_expr(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.and_expr()?;
        let mut chain = 0;
        while self.is_keyword("or") {
            self.pos += 1;
            self.enter()?;
            chain += 1;
            let right = self.and_expr()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        self.depth -= chain;
        Ok(left)
    }

    fn and_expr(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.not_expr()?;
        let mut chain = 0;
        while self.is_keyword("and") {
            self.pos += 1;
            self.enter()?;
            chain += 1;
            let right = self.not_expr()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        self.depth -= chain;
        Ok(left)
    }

    fn not_expr(&mut self) -> Result<Expr, FormulaError> {
        if self.is_keyword("not") {
            self.pos += 1;
            self.enter()?;
            let operand = self.not_expr();
            self.leave();
            return Ok(Expr::Unary(UnaryOp::Not, Box::new(operand?)));
        }
        self.comparison()
    }

    fn comparison_op(&mut self) -> Option<CompareOp> {
        let op = match self.peek().map(|t| &t.kind) {
            Some(TokenKind::Op("==")) => CompareOp::Eq,
            Some(TokenKind::Op("!=" | "<>")) => CompareOp::Ne,
            Some(TokenKind::Op("<")) => CompareOp::Lt,
            Some(TokenKind::Op("<=")) => CompareOp::Le,
            Some(TokenKind::Op(">")) => CompareOp::Gt,
            Some(TokenKind::Op(">=")) => CompareOp::Ge,
            Some(TokenKind::Ident(w)) if w == "in" => CompareOp::In,
            Some(TokenKind::Ident(w)) if w == "not" && self.is_keyword_at(1, "in") => {
                self.pos += 1;
                CompareOp::NotIn
            }
            _ => return None,
        };
        self.pos += 1;
        Some(op)
    }

    fn comparison(&mut self) -> Result<Expr, FormulaError> {
        let first = self.arith()?;
        let mut rest = Vec::new();
        while let Some(op) = self.comparison_op() {
            rest.push((op, self.arith()?));
        }
        Ok(if rest.is_empty() {
            first
        } else {
            Expr::Compare(Box::new(first), rest)
        })
    }

    fn arith(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.term()?;
        let mut chain = 0;
        loop {
            let op = if self.eat_op("+") {
                BinaryOp::Add
            } else if self.eat_op("-") {
                BinaryOp::Sub
            } else {
                self.depth -= chain;
                return Ok(left);
            };
            self.enter()?;
            chain += 1;
            let right = self.term()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn term(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.factor()?;
        let mut chain = 0;
        loop {
            let op = if self.eat_op("*") {
                BinaryOp::Mul
            } else if self.eat_op("//") {
                BinaryOp::FloorDiv
            } else if self.eat_op("/") {
                BinaryOp::Div
            } else if self.eat_op("%") {
                BinaryOp::Mod
            } else {
                self.depth -= chain;
                return Ok(left);
            };
            self.enter()?;
            chain += 1;
            let right = self.factor()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn factor(&mut self) -> Result<Expr, FormulaError> {
        let op = if self.eat_op("-") {
            UnaryOp::Neg
        } else if self.eat_op("+") {
            UnaryOp::Pos
        } else {
            return self.power();
        };
        self.enter()?;
        let operand = self.factor();
        self.leave();
        Ok(Expr::Unary(op, Box::new(operand?)))
    }

    fn power(&mut self) -> Result<Expr, FormulaError> {
        let base = self.postfix()?;
        if self.eat_op("**") {
            self.enter()?;
            let exponent = self.factor();
            self.leave();
            return Ok(Expr::Binary(
                BinaryOp::Pow,
                Box::new(base),
                Box::new(exponent?),
            ));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Result<Expr, FormulaError> {
        let mut expr = self.atom()?;
        let mut chain = 0;
        loop {
            if self.is_op("[") || self.is_op(".") || self.is_op("(") {
                self.enter()?;
                chain += 1;
            }
            if self.eat_op("[") {
                expr = self.subscript(expr)?;
            } else if self.eat_op(".") {
                let position = self.position();
                let name = match self.peek().map(|t| t.kind.clone()) {
                    Some(TokenKind::Ident(name)) => name,
                    _ => return syntax(position, tr!("nom de méthode attendu")),
                };
                if !METHODS.contains(&name.as_str()) {
                    return syntax(position, tr!("méthode inconnue : « {name} »", name = name));
                }
                self.pos += 1;
                if !self.eat_op("(") {
                    return syntax(
                        self.position(),
                        tr!("« ( » attendu après le nom de méthode"),
                    );
                }
                let args = self.arguments()?;
                expr = Expr::Method(Box::new(expr), name, args);
            } else if self.is_op("(") {
                let position = self.position();
                let Expr::Var(name) = expr else {
                    return syntax(
                        position,
                        tr!("seules les fonctions connues peuvent être appelées"),
                    );
                };
                if !FUNCTIONS.contains(&name.as_str()) {
                    return syntax(position, tr!("fonction inconnue : « {name} »", name = name));
                }
                self.pos += 1;
                let args = self.arguments()?;
                expr = Expr::Call(name, args);
            } else {
                self.depth -= chain;
                return Ok(expr);
            }
        }
    }

    /// Après `[` : index ou tranche, jusqu'au `]`.
    fn subscript(&mut self, target: Expr) -> Result<Expr, FormulaError> {
        let start = if self.is_op(":") {
            None
        } else {
            Some(Box::new(self.expression()?))
        };
        if self.eat_op(":") {
            let end = if self.is_op("]") {
                None
            } else {
                Some(Box::new(self.expression()?))
            };
            self.expect_op("]")?;
            return Ok(Expr::Slice(Box::new(target), start, end));
        }
        self.expect_op("]")?;
        match start {
            Some(index) => Ok(Expr::Index(Box::new(target), index)),
            None => syntax(self.position(), tr!("index attendu")),
        }
    }

    /// Après `(` : arguments séparés par des virgules, jusqu'au `)`.
    fn arguments(&mut self) -> Result<Vec<Expr>, FormulaError> {
        let mut args = Vec::new();
        if self.eat_op(")") {
            return Ok(args);
        }
        loop {
            args.push(self.expression()?);
            if self.eat_op(")") {
                return Ok(args);
            }
            self.expect_op(",")?;
            if self.eat_op(")") {
                return Ok(args);
            }
        }
    }

    fn atom(&mut self) -> Result<Expr, FormulaError> {
        let position = self.position();
        let Some(token) = self.peek().cloned() else {
            return syntax(position, tr!("expression incomplète"));
        };
        self.pos += 1;
        match token.kind {
            TokenKind::Int(i) => Ok(Expr::Const(Value::Int(i))),
            TokenKind::Float(f) => Ok(Expr::Const(Value::Float(f))),
            TokenKind::Str(mut s) => {
                // Chaînes adjacentes concaténées, comme en Python.
                while let Some(Token {
                    kind: TokenKind::Str(next),
                    ..
                }) = self.peek()
                {
                    s.push_str(next);
                    self.pos += 1;
                }
                Ok(Expr::Const(Value::Str(s)))
            }
            TokenKind::Ident(name) => match name.as_str() {
                "True" => Ok(Expr::Const(Value::Bool(true))),
                "False" => Ok(Expr::Const(Value::Bool(false))),
                "None" => Ok(Expr::Const(Value::None)),
                w if KEYWORDS.contains(&w) => syntax(position, tr!("« {w} » inattendu", w = w)),
                _ => Ok(Expr::Var(name)),
            },
            TokenKind::Op("(") => {
                if self.eat_op(")") {
                    return Ok(Expr::List(Vec::new()));
                }
                let first = self.expression()?;
                if self.eat_op(")") {
                    return Ok(first);
                }
                // N-uplet : traité comme une liste.
                let mut items = vec![first];
                while self.eat_op(",") {
                    if self.is_op(")") {
                        break;
                    }
                    items.push(self.expression()?);
                }
                self.expect_op(")")?;
                Ok(Expr::List(items))
            }
            TokenKind::Op("[") => {
                let mut items = Vec::new();
                if self.eat_op("]") {
                    return Ok(Expr::List(items));
                }
                loop {
                    items.push(self.expression()?);
                    if self.eat_op("]") {
                        return Ok(Expr::List(items));
                    }
                    self.expect_op(",")?;
                    if self.eat_op("]") {
                        return Ok(Expr::List(items));
                    }
                }
            }
            TokenKind::Op(op) => syntax(position, tr!("« {op} » inattendu", op = op)),
        }
    }
}

// ---------------------------------------------------------------------------
// Évaluation

impl Expr {
    fn floatify(&mut self) {
        match self {
            Expr::Const(Value::Int(i)) => *self = Expr::Const(Value::Float(*i as f64)),
            Expr::Const(_) | Expr::Var(_) => {}
            Expr::List(items) | Expr::Call(_, items) => items.iter_mut().for_each(Expr::floatify),
            Expr::Unary(_, e) => e.floatify(),
            Expr::Binary(_, a, b) | Expr::And(a, b) | Expr::Or(a, b) | Expr::Index(a, b) => {
                a.floatify();
                b.floatify();
            }
            Expr::Compare(first, rest) => {
                first.floatify();
                rest.iter_mut().for_each(|(_, e)| e.floatify());
            }
            Expr::Slice(target, start, end) => {
                target.floatify();
                start.iter_mut().for_each(|e| e.floatify());
                end.iter_mut().for_each(|e| e.floatify());
            }
            Expr::Method(target, _, args) => {
                target.floatify();
                args.iter_mut().for_each(Expr::floatify);
            }
        }
    }

    fn collect_variables(&self, out: &mut Vec<String>) {
        match self {
            Expr::Const(_) => {}
            Expr::Var(name) => {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
            Expr::List(items) | Expr::Call(_, items) => {
                items.iter().for_each(|e| e.collect_variables(out));
            }
            Expr::Unary(_, e) => e.collect_variables(out),
            Expr::Binary(_, a, b) | Expr::And(a, b) | Expr::Or(a, b) | Expr::Index(a, b) => {
                a.collect_variables(out);
                b.collect_variables(out);
            }
            Expr::Compare(first, rest) => {
                first.collect_variables(out);
                rest.iter().for_each(|(_, e)| e.collect_variables(out));
            }
            Expr::Slice(target, start, end) => {
                target.collect_variables(out);
                start.iter().for_each(|e| e.collect_variables(out));
                end.iter().for_each(|e| e.collect_variables(out));
            }
            Expr::Method(target, _, args) => {
                target.collect_variables(out);
                args.iter().for_each(|e| e.collect_variables(out));
            }
        }
    }

    fn eval(&self, vars: &dyn Fn(&str) -> Option<Value>) -> Result<Value, FormulaError> {
        match self {
            Expr::Const(v) => Ok(v.clone()),
            Expr::Var(name) => match vars(name) {
                Some(v) => Ok(v),
                None => eval_err(tr!("nom inconnu : « {name} »", name = name)),
            },
            Expr::List(items) => Ok(Value::List(
                items
                    .iter()
                    .map(|e| e.eval(vars))
                    .collect::<Result<_, _>>()?,
            )),
            Expr::Unary(op, operand) => {
                let value = operand.eval(vars)?;
                match op {
                    UnaryOp::Not => Ok(Value::Bool(!value.truthy())),
                    UnaryOp::Pos => match value.as_number() {
                        Some(Number::Int(i)) => Ok(Value::Int(i)),
                        Some(Number::Float(f)) => Ok(Value::Float(f)),
                        None => eval_err(tr!("« + » impossible sur une {}", value.type_name())),
                    },
                    UnaryOp::Neg => match value.as_number() {
                        Some(Number::Int(i)) => i.checked_neg().map(Value::Int).ok_or_else(|| {
                            FormulaError::Eval(tr!("dépassement de capacité").into())
                        }),
                        Some(Number::Float(f)) => Ok(Value::Float(-f)),
                        None => eval_err(tr!("« - » impossible sur une {}", value.type_name())),
                    },
                }
            }
            Expr::Binary(op, a, b) => binary(*op, a.eval(vars)?, b.eval(vars)?),
            Expr::And(a, b) => {
                let left = a.eval(vars)?;
                if left.truthy() {
                    b.eval(vars)
                } else {
                    Ok(left)
                }
            }
            Expr::Or(a, b) => {
                let left = a.eval(vars)?;
                if left.truthy() {
                    Ok(left)
                } else {
                    b.eval(vars)
                }
            }
            Expr::Compare(first, rest) => {
                let mut left = first.eval(vars)?;
                for (op, expr) in rest {
                    let right = expr.eval(vars)?;
                    if !compare(*op, &left, &right)? {
                        return Ok(Value::Bool(false));
                    }
                    left = right;
                }
                Ok(Value::Bool(true))
            }
            Expr::Index(target, index) => index_value(target.eval(vars)?, index.eval(vars)?),
            Expr::Slice(target, start, end) => {
                let target = target.eval(vars)?;
                let start = start.as_ref().map(|e| e.eval(vars)).transpose()?;
                let end = end.as_ref().map(|e| e.eval(vars)).transpose()?;
                slice_value(target, start, end)
            }
            Expr::Call(name, args) => {
                let args: Vec<Value> = args
                    .iter()
                    .map(|e| e.eval(vars))
                    .collect::<Result<_, _>>()?;
                call_function(name, args)
            }
            Expr::Method(target, name, args) => {
                let target = target.eval(vars)?;
                let args: Vec<Value> = args
                    .iter()
                    .map(|e| e.eval(vars))
                    .collect::<Result<_, _>>()?;
                call_method(target, name, args)
            }
        }
    }
}

fn overflow() -> FormulaError {
    FormulaError::Eval(tr!("dépassement de capacité").into())
}

fn check_string(s: String) -> Result<Value, FormulaError> {
    if s.len() > MAX_STRING_LEN {
        eval_err(tr!("chaîne trop longue"))
    } else {
        Ok(Value::Str(s))
    }
}

/// Division entière de Python (arrondi vers −∞).
fn floor_div(a: i64, b: i64) -> Result<i64, FormulaError> {
    if b == 0 {
        return eval_err(tr!("division par zéro"));
    }
    let q = a.checked_div(b).ok_or_else(overflow)?;
    Ok(if a % b != 0 && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    })
}

/// Modulo de Python (du signe du diviseur).
fn py_mod(a: i64, b: i64) -> Result<i64, FormulaError> {
    if b == 0 {
        return eval_err(tr!("division par zéro"));
    }
    let r = a.checked_rem(b).ok_or_else(overflow)?;
    Ok(if r != 0 && ((r < 0) != (b < 0)) {
        r + b
    } else {
        r
    })
}

fn repeat(s: &str, n: i64) -> Result<Value, FormulaError> {
    let n = n.max(0) as usize;
    if s.len().saturating_mul(n) > MAX_STRING_LEN {
        return eval_err(tr!("chaîne trop longue"));
    }
    Ok(Value::Str(s.repeat(n)))
}

fn binary(op: BinaryOp, a: Value, b: Value) -> Result<Value, FormulaError> {
    match (op, &a, &b) {
        (BinaryOp::Add, Value::Str(x), Value::Str(y)) => return check_string(format!("{x}{y}")),
        (BinaryOp::Add, Value::List(x), Value::List(y)) => {
            return Ok(Value::List(x.iter().chain(y).cloned().collect()));
        }
        (BinaryOp::Mul, Value::Str(s), n) | (BinaryOp::Mul, n, Value::Str(s))
            if n.as_int().is_some() =>
        {
            return repeat(s, n.as_int().unwrap_or(0));
        }
        _ => {}
    }
    let (Some(x), Some(y)) = (a.as_number(), b.as_number()) else {
        return eval_err(tr!(
            "opération impossible entre une {} et une {}",
            a.type_name(),
            b.type_name()
        ));
    };
    if let (Number::Int(x), Number::Int(y)) = (x, y) {
        let result = match op {
            BinaryOp::Add => x.checked_add(y).ok_or_else(overflow)?,
            BinaryOp::Sub => x.checked_sub(y).ok_or_else(overflow)?,
            BinaryOp::Mul => x.checked_mul(y).ok_or_else(overflow)?,
            // Python 2 : division entière entre deux entiers.
            BinaryOp::Div | BinaryOp::FloorDiv => floor_div(x, y)?,
            BinaryOp::Mod => py_mod(x, y)?,
            BinaryOp::Pow => {
                if y < 0 {
                    return Ok(Value::Float((x as f64).powf(y as f64)));
                }
                let exp = u32::try_from(y).map_err(|_| overflow())?;
                x.checked_pow(exp).ok_or_else(overflow)?
            }
        };
        return Ok(Value::Int(result));
    }
    let (x, y) = (x.as_f64(), y.as_f64());
    let result = match op {
        BinaryOp::Add => x + y,
        BinaryOp::Sub => x - y,
        BinaryOp::Mul => x * y,
        BinaryOp::Div | BinaryOp::FloorDiv | BinaryOp::Mod if y == 0.0 => {
            return eval_err(tr!("division par zéro"));
        }
        BinaryOp::Div => x / y,
        BinaryOp::FloorDiv => (x / y).floor(),
        BinaryOp::Mod => x - y * (x / y).floor(),
        BinaryOp::Pow => x.powf(y),
    };
    if result.is_finite() {
        Ok(Value::Float(result))
    } else {
        Err(overflow())
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a.as_number(), b.as_number()) {
        (Some(Number::Int(x)), Some(Number::Int(y))) => x == y,
        (Some(x), Some(y)) => x.as_f64() == y.as_f64(),
        _ => match (a, b) {
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::None, Value::None) => true,
            (Value::List(x), Value::List(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(a, b)| values_equal(a, b))
            }
            _ => false,
        },
    }
}

fn order(a: &Value, b: &Value) -> Result<Ordering, FormulaError> {
    match (a.as_number(), b.as_number()) {
        (Some(Number::Int(x)), Some(Number::Int(y))) => return Ok(x.cmp(&y)),
        (Some(x), Some(y)) => {
            return x
                .as_f64()
                .partial_cmp(&y.as_f64())
                .ok_or_else(|| FormulaError::Eval(tr!("comparaison impossible").into()));
        }
        _ => {}
    }
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => Ok(x.cmp(y)),
        (Value::List(x), Value::List(y)) => {
            for (a, b) in x.iter().zip(y) {
                match order(a, b)? {
                    Ordering::Equal => {}
                    other => return Ok(other),
                }
            }
            Ok(x.len().cmp(&y.len()))
        }
        _ => eval_err(tr!(
            "comparaison impossible entre une {} et une {}",
            a.type_name(),
            b.type_name()
        )),
    }
}

fn compare(op: CompareOp, a: &Value, b: &Value) -> Result<bool, FormulaError> {
    Ok(match op {
        CompareOp::Eq => values_equal(a, b),
        CompareOp::Ne => !values_equal(a, b),
        CompareOp::Lt => order(a, b)? == Ordering::Less,
        CompareOp::Le => order(a, b)? != Ordering::Greater,
        CompareOp::Gt => order(a, b)? == Ordering::Greater,
        CompareOp::Ge => order(a, b)? != Ordering::Less,
        CompareOp::In | CompareOp::NotIn => {
            let found = match (a, b) {
                (Value::Str(needle), Value::Str(haystack)) => haystack.contains(needle.as_str()),
                (_, Value::List(items)) => items.iter().any(|item| values_equal(a, item)),
                _ => {
                    return eval_err(tr!(
                        "« in » impossible entre une {} et une {}",
                        a.type_name(),
                        b.type_name()
                    ));
                }
            };
            found == (op == CompareOp::In)
        }
    })
}

fn int_arg(value: &Value, what: &str) -> Result<i64, FormulaError> {
    value
        .as_int()
        .ok_or_else(|| FormulaError::Eval(tr!("{what} : entier attendu", what = what)))
}

/// Index Python (négatif = depuis la fin) ramené dans `0..len`.
fn resolve_index(index: i64, len: usize) -> Option<usize> {
    let len = len as i64;
    let i = if index < 0 { index + len } else { index };
    (0..len).contains(&i).then_some(i as usize)
}

fn index_value(target: Value, index: Value) -> Result<Value, FormulaError> {
    let i = int_arg(&index, "index")?;
    match target {
        Value::Str(s) => {
            let chars: Vec<char> = s.chars().collect();
            resolve_index(i, chars.len())
                .map(|i| Value::Str(chars[i].to_string()))
                .ok_or_else(|| FormulaError::Eval(tr!("index hors limites").into()))
        }
        Value::List(items) => resolve_index(i, items.len())
            .map(|i| items[i].clone())
            .ok_or_else(|| FormulaError::Eval(tr!("index hors limites").into())),
        other => eval_err(tr!("une {} ne peut pas être indexée", other.type_name())),
    }
}

/// Bornes d'une tranche Python, ramenées dans `0..=len`.
fn slice_bounds(
    start: Option<Value>,
    end: Option<Value>,
    len: usize,
) -> Result<(usize, usize), FormulaError> {
    let clamp = |v: Option<Value>, default: usize| -> Result<usize, FormulaError> {
        match v {
            None | Some(Value::None) => Ok(default),
            Some(v) => {
                let i = int_arg(&v, tr!("tranche"))?;
                let len = len as i64;
                let i = if i < 0 { i + len } else { i };
                Ok(i.clamp(0, len) as usize)
            }
        }
    };
    let start = clamp(start, 0)?;
    let end = clamp(end, len)?;
    Ok((start, end.max(start)))
}

fn slice_value(
    target: Value,
    start: Option<Value>,
    end: Option<Value>,
) -> Result<Value, FormulaError> {
    match target {
        Value::Str(s) => {
            let chars: Vec<char> = s.chars().collect();
            let (a, b) = slice_bounds(start, end, chars.len())?;
            Ok(Value::Str(chars[a..b].iter().collect()))
        }
        Value::List(items) => {
            let (a, b) = slice_bounds(start, end, items.len())?;
            Ok(Value::List(items[a..b].to_vec()))
        }
        other => eval_err(tr!("une {} ne peut pas être découpée", other.type_name())),
    }
}

fn arity(name: &str, args: &[Value], min: usize, max: usize) -> Result<(), FormulaError> {
    if args.len() < min || args.len() > max {
        eval_err(tr!("{name}() : nombre d'arguments incorrect", name = name))
    } else {
        Ok(())
    }
}

fn call_function(name: &str, args: Vec<Value>) -> Result<Value, FormulaError> {
    match name {
        "len" => {
            arity(name, &args, 1, 1)?;
            match &args[0] {
                Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                Value::List(l) => Ok(Value::Int(l.len() as i64)),
                other => eval_err(tr!("len() impossible sur une {}", other.type_name())),
            }
        }
        "str" => {
            arity(name, &args, 0, 1)?;
            Ok(Value::Str(
                args.first().map(Value::to_string).unwrap_or_default(),
            ))
        }
        "int" => {
            arity(name, &args, 1, 1)?;
            match &args[0] {
                Value::Str(s) => s.trim().parse().map(Value::Int).map_err(|_| {
                    FormulaError::Eval(tr!("int() : « {s} » n'est pas un entier", s = s))
                }),
                Value::Float(f) if f.is_finite() && f.abs() < 9.0e18 => {
                    Ok(Value::Int(f.trunc() as i64))
                }
                v => v.as_int().map(Value::Int).ok_or_else(|| {
                    FormulaError::Eval(tr!("int() impossible sur une {}", v.type_name()))
                }),
            }
        }
        "float" => {
            arity(name, &args, 1, 1)?;
            match &args[0] {
                Value::Str(s) => s.trim().parse().map(Value::Float).map_err(|_| {
                    FormulaError::Eval(tr!("float() : « {s} » n'est pas un nombre", s = s))
                }),
                v => v
                    .as_number()
                    .map(|n| Value::Float(n.as_f64()))
                    .ok_or_else(|| {
                        FormulaError::Eval(tr!("float() impossible sur une {}", v.type_name()))
                    }),
            }
        }
        "abs" => {
            arity(name, &args, 1, 1)?;
            match args[0].as_number() {
                Some(Number::Int(i)) => i.checked_abs().map(Value::Int).ok_or_else(overflow),
                Some(Number::Float(f)) => Ok(Value::Float(f.abs())),
                None => eval_err(tr!("abs() impossible sur une {}", args[0].type_name())),
            }
        }
        "round" => {
            arity(name, &args, 1, 2)?;
            let Some(x) = args[0].as_number() else {
                return eval_err(tr!("round() impossible sur une {}", args[0].type_name()));
            };
            let digits = match args.get(1) {
                Some(d) => int_arg(d, "round")?.clamp(-15, 15) as i32,
                None => 0,
            };
            // Python 2 : arrondi au plus loin de zéro, résultat flottant.
            let factor = 10f64.powi(digits);
            Ok(Value::Float((x.as_f64() * factor).round() / factor))
        }
        "min" | "max" => {
            let items = match args.as_slice() {
                [Value::List(items)] => items.clone(),
                _ => args,
            };
            let mut iter = items.into_iter();
            let Some(mut best) = iter.next() else {
                return eval_err(tr!("{name}() : aucun argument", name = name));
            };
            for item in iter {
                let ord = order(&item, &best)?;
                if (name == "min" && ord == Ordering::Less)
                    || (name == "max" && ord == Ordering::Greater)
                {
                    best = item;
                }
            }
            Ok(best)
        }
        _ => eval_err(tr!("fonction inconnue : « {name} »", name = name)),
    }
}

fn str_arg<'a>(args: &'a [Value], i: usize, method: &str) -> Result<&'a str, FormulaError> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s),
        _ => eval_err(tr!("{method}() : chaîne attendue", method = method)),
    }
}

fn char_index(haystack: &str, byte_index: Option<usize>) -> i64 {
    byte_index.map_or(-1, |b| haystack[..b].chars().count() as i64)
}

fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut previous_cased = false;
    for c in s.chars() {
        if c.is_alphabetic() {
            if previous_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            previous_cased = true;
        } else {
            out.push(c);
            previous_cased = false;
        }
    }
    out
}

fn strip_chars<'a>(
    s: &'a str,
    args: &[Value],
    method: &str,
    mode: u8,
) -> Result<&'a str, FormulaError> {
    let chars: Option<Vec<char>> = match args.first() {
        None | Some(Value::None) => None,
        Some(Value::Str(set)) => Some(set.chars().collect()),
        _ => return eval_err(tr!("{method}() : chaîne attendue", method = method)),
    };
    let pred = |c: char| match &chars {
        Some(set) => set.contains(&c),
        None => c.is_whitespace(),
    };
    Ok(match mode {
        0 => s.trim_matches(pred),
        1 => s.trim_start_matches(pred),
        _ => s.trim_end_matches(pred),
    })
}

fn call_method(target: Value, name: &str, args: Vec<Value>) -> Result<Value, FormulaError> {
    let Value::Str(s) = target else {
        return eval_err(tr!(
            "méthode « {name} » inconnue pour une {kind}",
            name = name,
            kind = target.type_name()
        ));
    };
    match name {
        "lower" => {
            arity(name, &args, 0, 0)?;
            Ok(Value::Str(s.to_lowercase()))
        }
        "upper" => {
            arity(name, &args, 0, 0)?;
            Ok(Value::Str(s.to_uppercase()))
        }
        "title" => {
            arity(name, &args, 0, 0)?;
            Ok(Value::Str(title_case(&s)))
        }
        "capitalize" => {
            arity(name, &args, 0, 0)?;
            let mut chars = s.chars();
            Ok(Value::Str(match chars.next() {
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.as_str().to_lowercase().chars())
                    .collect(),
                None => String::new(),
            }))
        }
        "strip" | "lstrip" | "rstrip" => {
            arity(name, &args, 0, 1)?;
            let mode = match name {
                "strip" => 0,
                "lstrip" => 1,
                _ => 2,
            };
            Ok(Value::Str(strip_chars(&s, &args, name, mode)?.to_string()))
        }
        "replace" => {
            arity(name, &args, 2, 3)?;
            let old = str_arg(&args, 0, name)?;
            let new = str_arg(&args, 1, name)?;
            let replaced = match args.get(2) {
                Some(count) => {
                    let count = int_arg(count, "replace")?;
                    if count < 0 {
                        s.replace(old, new)
                    } else {
                        s.replacen(old, new, count as usize)
                    }
                }
                None => {
                    let estimate = s
                        .matches(old)
                        .count()
                        .saturating_add(1)
                        .saturating_mul(new.len());
                    if estimate > MAX_STRING_LEN * 2 {
                        return eval_err(tr!("chaîne trop longue"));
                    }
                    s.replace(old, new)
                }
            };
            check_string(replaced)
        }
        "startswith" => {
            arity(name, &args, 1, 1)?;
            Ok(Value::Bool(s.starts_with(str_arg(&args, 0, name)?)))
        }
        "endswith" => {
            arity(name, &args, 1, 1)?;
            Ok(Value::Bool(s.ends_with(str_arg(&args, 0, name)?)))
        }
        "find" => {
            arity(name, &args, 1, 1)?;
            Ok(Value::Int(char_index(&s, s.find(str_arg(&args, 0, name)?))))
        }
        "rfind" => {
            arity(name, &args, 1, 1)?;
            Ok(Value::Int(char_index(
                &s,
                s.rfind(str_arg(&args, 0, name)?),
            )))
        }
        "count" => {
            arity(name, &args, 1, 1)?;
            let needle = str_arg(&args, 0, name)?;
            Ok(Value::Int(if needle.is_empty() {
                s.chars().count() as i64 + 1
            } else {
                s.matches(needle).count() as i64
            }))
        }
        "isdigit" => {
            arity(name, &args, 0, 0)?;
            Ok(Value::Bool(
                !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
            ))
        }
        "split" => {
            arity(name, &args, 0, 1)?;
            let parts: Vec<Value> = match args.first() {
                None | Some(Value::None) => {
                    s.split_whitespace().map(|p| Value::Str(p.into())).collect()
                }
                Some(Value::Str(sep)) if !sep.is_empty() => s
                    .split(sep.as_str())
                    .map(|p| Value::Str(p.into()))
                    .collect(),
                _ => return eval_err(tr!("split() : séparateur invalide")),
            };
            Ok(Value::List(parts))
        }
        "join" => {
            arity(name, &args, 1, 1)?;
            let Value::List(items) = &args[0] else {
                return eval_err(tr!("join() : liste attendue"));
            };
            let parts: Vec<&str> = items
                .iter()
                .map(|v| match v {
                    Value::Str(p) => Ok(p.as_str()),
                    _ => eval_err(tr!("join() : liste de chaînes attendue")),
                })
                .collect::<Result<_, _>>()?;
            check_string(parts.join(&s))
        }
        _ => eval_err(tr!("méthode inconnue : « {name} »", name = name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval_with(source: &str, vars: &[(&str, Value)]) -> Result<Value, FormulaError> {
        Formula::parse(source)?.eval(&|name| {
            vars.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
        })
    }

    fn eval(source: &str) -> Value {
        eval_with(source, &[]).unwrap_or_else(|e| panic!("{source} : {e}"))
    }

    #[test]
    fn arithmetic_python2() {
        assert_eq!(eval("1 + 2 * 3"), Value::Int(7));
        assert_eq!(eval("(1 + 2) * 3"), Value::Int(9));
        assert_eq!(eval("7 / 2"), Value::Int(3), "division entière Python 2");
        assert_eq!(eval("-7 / 2"), Value::Int(-4), "arrondi vers -inf");
        assert_eq!(eval("7 // 2"), Value::Int(3));
        assert_eq!(eval("-7 % 3"), Value::Int(2));
        assert_eq!(eval("7.0 / 2"), Value::Float(3.5));
        assert_eq!(eval("2 ** 10"), Value::Int(1024));
        assert_eq!(eval("-2 ** 2"), Value::Int(-4));
        assert_eq!(eval("10L - 1"), Value::Int(9));
        assert_eq!(eval("1e3"), Value::Float(1000.0));
        assert!(eval_with("1 / 0", &[]).is_err());
        let float = Formula::parse("100 / 3").map(Formula::with_float_arithmetic);
        assert_eq!(
            float.and_then(|f| f.eval(&|_| None)),
            Ok(Value::Float(100.0 / 3.0))
        );
        let chain = format!("{}1", "1+".repeat(150));
        assert!(
            Formula::parse(&chain).is_err(),
            "chaîne trop longue pour la récursion"
        );
        let chain = format!("{}1", "1+".repeat(50));
        assert_eq!(eval_with(&chain, &[]), Ok(Value::Int(51)));
        assert!(eval_with("9223372036854775807 + 1", &[]).is_err());
    }

    #[test]
    fn strings_and_methods() {
        assert_eq!(eval("'ab' + \"cd\""), Value::Str("abcd".into()));
        assert_eq!(eval("'ab' 'cd'"), Value::Str("abcd".into()));
        assert_eq!(eval("'abcdef'[2:]"), Value::Str("cdef".into()));
        assert_eq!(eval("'abcdef'[:-2]"), Value::Str("abcd".into()));
        assert_eq!(eval("'abcdef'[-1]"), Value::Str("f".into()));
        assert_eq!(eval("'abc'[10:]"), Value::Str(String::new()));
        assert_eq!(eval("'  Chq 12 '.strip()"), Value::Str("Chq 12".into()));
        assert_eq!(eval("'xxaxx'.strip('x')"), Value::Str("a".into()));
        assert_eq!(
            eval("'a-b-c'.replace('-', '+')"),
            Value::Str("a+b+c".into())
        );
        assert_eq!(
            eval("'a-b-c'.replace('-', '+', 1)"),
            Value::Str("a+b-c".into())
        );
        assert_eq!(
            eval("'élan vital'.title()"),
            Value::Str("Élan Vital".into())
        );
        assert_eq!(eval("'ÉCOLE'.lower()"), Value::Str("école".into()));
        assert_eq!(eval("'abc'.find('c')"), Value::Int(2));
        assert_eq!(
            eval("'éabc'.find('c')"),
            Value::Int(3),
            "index en caractères"
        );
        assert_eq!(eval("'abc'.find('z')"), Value::Int(-1));
        assert_eq!(eval("len('été')"), Value::Int(3));
        assert_eq!(
            eval("'a b  c'.split()"),
            Value::List(vec![
                Value::Str("a".into()),
                Value::Str("b".into()),
                Value::Str("c".into())
            ])
        );
        assert_eq!(eval("'-'.join(['a', 'b'])"), Value::Str("a-b".into()));
        assert_eq!(eval("str(12) + 'x'"), Value::Str("12x".into()));
        assert_eq!(eval("str(-12.5)"), Value::Str("-12.5".into()));
        assert_eq!(eval("str(3.0)"), Value::Str("3.0".into()));
        assert_eq!(
            eval("str(0.1 + 0.2)"),
            Value::Str("0.3".into()),
            "12 chiffres significatifs"
        );
        assert_eq!(eval("'ab' * 3"), Value::Str("ababab".into()));
        assert!(eval_with("'ab' * 10000000", &[]).is_err(), "taille bornée");
    }

    #[test]
    fn escapes() {
        assert_eq!(eval(r"'caf\xe9'"), Value::Str("café".into()));
        assert_eq!(eval(r"'\x80'"), Value::Str("€".into()), "octet CP1252");
        assert_eq!(eval(r"u'\u20ac'"), Value::Str("€".into()));
        assert_eq!(eval(r"'l\'eau'"), Value::Str("l'eau".into()));
        assert_eq!(eval(r"r'a\n'"), Value::Str("a\\n".into()));
        assert_eq!(eval(r"'a\qb'"), Value::Str("a\\qb".into()));
    }

    #[test]
    fn comparisons_and_logic() {
        assert_eq!(eval("1 < 2 < 3"), Value::Bool(true));
        assert_eq!(eval("1 < 3 < 2"), Value::Bool(false));
        assert_eq!(eval("1 <> 2"), Value::Bool(true));
        assert_eq!(eval("1 == 1.0"), Value::Bool(true));
        assert_eq!(eval("'a' in 'bac'"), Value::Bool(true));
        assert_eq!(eval("'z' not in 'bac'"), Value::Bool(true));
        assert_eq!(eval("2 in [1, 2]"), Value::Bool(true));
        assert_eq!(eval("2 in (1, 3)"), Value::Bool(false));
        assert_eq!(eval("not 0"), Value::Bool(true));
        assert_eq!(eval("'' or 'défaut'"), Value::Str("défaut".into()));
        assert_eq!(eval("0 and 1 / 0"), Value::Int(0), "court-circuit");
        assert_eq!(eval("True + 1"), Value::Int(2));
        assert!(eval_with("'a' < 1", &[]).is_err());
        assert!(eval_with("1 in 2", &[]).is_err());
    }

    #[test]
    fn variables() {
        let vars = [
            ("tiers", Value::Str("tresor public".into())),
            ("montant", Value::Float(-80.0)),
        ];
        let f = "\"tresor public\" in tiers and montant < -70 and montant > -100";
        assert_eq!(eval_with(f, &vars), Ok(Value::Bool(true)));
        assert_eq!(
            Formula::parse(f).map(|f| f.variables()),
            Ok(vec!["tiers".to_string(), "montant".to_string()])
        );
        assert!(matches!(
            eval_with("inconnu == 1", &vars),
            Err(FormulaError::Eval(_))
        ));
    }

    #[test]
    fn rejects_dangerous_or_malformed() {
        for source in [
            "__import__('os').system('ls')",
            "open('/etc/passwd')",
            "eval('1')",
            "tiers.__class__",
            "lambda: 1",
            "x = 1",
            "{}",
            "1 +",
            "(1",
            "'abc",
            "1 2",
            "a[",
            "a.b",
            "'''x'''",
            "",
            "f(x)(y)",
            "1 if 2 else 3",
            "a;b",
        ] {
            let result =
                Formula::parse(source).and_then(|f| f.eval(&|_| Some(Value::Str("x".into()))));
            assert!(result.is_err(), "{source:?} devrait échouer : {result:?}");
        }
        let deep = format!("{}1{}", "(".repeat(500), ")".repeat(500));
        assert!(Formula::parse(&deep).is_err());
        let negs = format!("{}1", "-".repeat(500));
        assert!(Formula::parse(&negs).is_err());
        assert!(Formula::parse(&"1+".repeat(3000)).is_err());
    }
}
