//! Valeur Python 2 générique, pour relire les données des anciens scripts.
//!
//! Les anciens scripts rangeaient leurs données dans le `.bp` sous forme de
//! pickles Python 2 (`bp_script_data_<clé>`), et le « Calcul automatique des
//! champs » exportait ses règles avec `repr(dict)`, une par ligne. Ce module
//! définit le sous-ensemble de valeurs rencontré ([`PyValue`]) :
//!
//! - le lecteur de pickles (`legacy_pickle`) produit (ou se convertit en) un
//!   [`PyValue`] ; les conversions vers les types natifs (`budget`,
//!   `autofilters`, `autofields`) partent toutes de là ;
//! - [`parse_repr`] relit un littéral Python (`{'field': 0, 'contains': 'caf\xe9'}`)
//!   **sans `eval`** ; [`to_repr`] écrit l'inverse, lisible par l'ancienne
//!   version.
//!
//! Les `str` Python 2 sont des octets CP1252 : les échappements `\xNN` sont
//! décodés en CP1252 (`\x80` → `€`), et l'écriture réencode en CP1252.

use crate::formula::cp1252_char;

/// Valeur Python 2 (sous-ensemble des pickles et `repr` des anciens scripts).
#[derive(Debug, Clone, PartialEq)]
pub enum PyValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    /// `str` (octets CP1252 déjà décodés) ou `unicode`.
    Str(String),
    List(Vec<PyValue>),
    Tuple(Vec<PyValue>),
    /// Dictionnaire, dans l'ordre de lecture.
    Dict(Vec<(PyValue, PyValue)>),
}

impl PyValue {
    /// Valeur associée à une clé chaîne d'un dictionnaire.
    pub fn get(&self, key: &str) -> Option<&PyValue> {
        match self {
            PyValue::Dict(items) => items
                .iter()
                .find(|(k, _)| matches!(k, PyValue::Str(s) if s == key))
                .map(|(_, v)| v),
            _ => None,
        }
    }

    /// Entier (booléens : 0/1 ; flottant entier accepté).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            PyValue::Bool(b) => Some(i64::from(*b)),
            PyValue::Int(i) => Some(*i),
            PyValue::Float(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => Some(*f as i64),
            _ => None,
        }
    }

    /// Nombre (entier ou flottant).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            PyValue::Bool(b) => Some(f64::from(u8::from(*b))),
            PyValue::Int(i) => Some(*i as f64),
            PyValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PyValue::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Vérité au sens de Python (les cases à cocher valaient 0/1 ou
    /// `False`/`True` selon les versions).
    pub fn truthy(&self) -> bool {
        match self {
            PyValue::None => false,
            PyValue::Bool(b) => *b,
            PyValue::Int(i) => *i != 0,
            PyValue::Float(f) => *f != 0.0,
            PyValue::Str(s) => !s.is_empty(),
            PyValue::List(l) | PyValue::Tuple(l) => !l.is_empty(),
            PyValue::Dict(d) => !d.is_empty(),
        }
    }

    /// Éléments d'une liste ou d'un n-uplet.
    pub fn as_list(&self) -> Option<&[PyValue]> {
        match self {
            PyValue::List(l) | PyValue::Tuple(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&[(PyValue, PyValue)]> {
        match self {
            PyValue::Dict(d) => Some(d),
            _ => None,
        }
    }

    /// Champ booléen facultatif d'un dictionnaire (absent = faux), comme
    /// `d.get(clé, 0)`.
    pub fn flag(&self, key: &str) -> bool {
        self.get(key).is_some_and(PyValue::truthy)
    }

    /// Champ texte facultatif d'un dictionnaire (absent = chaîne vide).
    pub fn text(&self, key: &str) -> &str {
        self.get(key).and_then(PyValue::as_str).unwrap_or("")
    }
}

/// Erreur de lecture d'une donnée héritée.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyError {
    #[error("littéral Python invalide (caractère {position}) : {message}")]
    Syntax { position: usize, message: String },
    #[error("structure inattendue : {0}")]
    Structure(String),
}

/// Profondeur maximale d'imbrication acceptée dans un littéral.
const MAX_DEPTH: usize = 64;

/// Lit un littéral Python 2 (`repr()` d'un dict, d'une liste…), sans `eval`.
pub fn parse_repr(text: &str) -> Result<PyValue, LegacyError> {
    let mut parser = ReprParser {
        chars: text.chars().collect(),
        pos: 0,
        depth: 0,
    };
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos < parser.chars.len() {
        return parser.error("contenu inattendu après la valeur");
    }
    Ok(value)
}

struct ReprParser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
}

impl ReprParser {
    fn error<T>(&self, message: &str) -> Result<T, LegacyError> {
        Err(LegacyError::Syntax {
            position: self.pos,
            message: message.into(),
        })
    }

    fn skip_ws(&mut self) {
        while self.chars.get(self.pos).is_some_and(|c| c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<PyValue, LegacyError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return self.error("imbrication trop profonde");
        }
        let result = self.value_inner();
        self.depth -= 1;
        result
    }

    /// Éléments séparés par des virgules jusqu'à `close` (virgule finale admise).
    fn items(&mut self, close: char) -> Result<Vec<PyValue>, LegacyError> {
        let mut items = Vec::new();
        loop {
            if self.eat(close) {
                return Ok(items);
            }
            items.push(self.value()?);
            if self.eat(close) {
                return Ok(items);
            }
            if !self.eat(',') {
                return self.error("« , » attendue");
            }
        }
    }

    fn value_inner(&mut self) -> Result<PyValue, LegacyError> {
        self.skip_ws();
        let Some(c) = self.peek() else {
            return self.error("valeur attendue");
        };
        match c {
            '{' => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    if self.eat('}') {
                        return Ok(PyValue::Dict(items));
                    }
                    let key = self.value()?;
                    if !self.eat(':') {
                        return self.error("« : » attendu");
                    }
                    let value = self.value()?;
                    items.push((key, value));
                    if self.eat('}') {
                        return Ok(PyValue::Dict(items));
                    }
                    if !self.eat(',') {
                        return self.error("« , » attendue");
                    }
                }
            }
            '[' => {
                self.pos += 1;
                Ok(PyValue::List(self.items(']')?))
            }
            '(' => {
                self.pos += 1;
                Ok(PyValue::Tuple(self.items(')')?))
            }
            '\'' | '"' => self.string(false),
            'u' | 'U' if matches!(self.chars.get(self.pos + 1), Some('\'' | '"')) => {
                self.pos += 1;
                self.string(true)
            }
            '-' | '+' | '.' | '0'..='9' => self.number(),
            _ => {
                let start = self.pos;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    self.pos += 1;
                }
                let word: String = self.chars[start..self.pos].iter().collect();
                match word.as_str() {
                    "True" => Ok(PyValue::Bool(true)),
                    "False" => Ok(PyValue::Bool(false)),
                    "None" => Ok(PyValue::None),
                    _ => {
                        self.pos = start;
                        self.error("valeur non reconnue")
                    }
                }
            }
        }
    }

    fn number(&mut self) -> Result<PyValue, LegacyError> {
        let start = self.pos;
        if matches!(self.peek(), Some('-' | '+')) {
            self.pos += 1;
        }
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E'))
            || (matches!(self.peek(), Some('-' | '+'))
                && matches!(self.chars.get(self.pos - 1), Some('e' | 'E')))
        {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if matches!(self.peek(), Some('L' | 'l')) {
            self.pos += 1;
        }
        if let Ok(i) = text.parse::<i64>() {
            return Ok(PyValue::Int(i));
        }
        match text.parse::<f64>() {
            Ok(f) => Ok(PyValue::Float(f)),
            Err(_) => {
                self.pos = start;
                self.error("nombre invalide")
            }
        }
    }

    fn string(&mut self, unicode: bool) -> Result<PyValue, LegacyError> {
        let quote = self.chars[self.pos];
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return self.error("chaîne non terminée");
            };
            self.pos += 1;
            if c == quote {
                return Ok(PyValue::Str(out));
            }
            if c != '\\' {
                out.push(c);
                continue;
            }
            let Some(e) = self.peek() else {
                return self.error("chaîne non terminée");
            };
            self.pos += 1;
            match e {
                '\\' | '\'' | '"' => out.push(e),
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'x' | 'u' | 'U' if e == 'x' || unicode => {
                    let len = match e {
                        'x' => 2,
                        'u' => 4,
                        _ => 8,
                    };
                    let digits: String = self
                        .chars
                        .get(self.pos..self.pos + len)
                        .map(|d| d.iter().collect())
                        .unwrap_or_default();
                    let Ok(code) = u32::from_str_radix(&digits, 16) else {
                        return self.error("échappement invalide");
                    };
                    self.pos += len;
                    out.push(if unicode {
                        char::from_u32(code).unwrap_or('\u{fffd}')
                    } else {
                        cp1252_char(code as u8)
                    });
                }
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
        }
    }
}

/// Écrit `value` comme `repr()` de Python 2 : texte ASCII, caractères
/// accentués en octets CP1252 échappés (`\xe9`), relisible par l'ancienne
/// version (`eval`) comme par [`parse_repr`].
pub fn to_repr(value: &PyValue) -> String {
    let mut out = String::new();
    write_repr(value, &mut out);
    out
}

fn write_repr(value: &PyValue, out: &mut String) {
    match value {
        PyValue::None => out.push_str("None"),
        PyValue::Bool(true) => out.push_str("True"),
        PyValue::Bool(false) => out.push_str("False"),
        PyValue::Int(i) => out.push_str(&i.to_string()),
        PyValue::Float(f) => out.push_str(&float_repr(*f)),
        PyValue::Str(s) => write_str_repr(s, out),
        PyValue::List(items) => {
            out.push('[');
            write_items(items, out);
            out.push(']');
        }
        PyValue::Tuple(items) => {
            out.push('(');
            write_items(items, out);
            if items.len() == 1 {
                out.push(',');
            }
            out.push(')');
        }
        PyValue::Dict(items) => {
            out.push('{');
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_repr(k, out);
                out.push_str(": ");
                write_repr(v, out);
            }
            out.push('}');
        }
    }
}

fn write_items(items: &[PyValue], out: &mut String) {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_repr(item, out);
    }
}

fn float_repr(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e16 {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

fn write_str_repr(s: &str, out: &mut String) {
    // Une chaîne non représentable en CP1252 devient un littéral unicode.
    let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(s);
    if unmappable {
        out.push('u');
    }
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    out.push(quote);
    if unmappable {
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                c if c == quote => {
                    out.push('\\');
                    out.push(c);
                }
                ' '..='~' => out.push(c),
                c if (c as u32) < 0x100 => out.push_str(&format!("\\x{:02x}", c as u32)),
                c if (c as u32) < 0x10000 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push_str(&format!("\\U{:08x}", c as u32)),
            }
        }
    } else {
        for &b in bytes.iter() {
            match b {
                b'\\' => out.push_str("\\\\"),
                b'\n' => out.push_str("\\n"),
                b'\t' => out.push_str("\\t"),
                b'\r' => out.push_str("\\r"),
                b if b == quote as u8 => {
                    out.push('\\');
                    out.push(quote);
                }
                b' '..=b'~' => out.push(b as char),
                b => out.push_str(&format!("\\x{b:02x}")),
            }
        }
    }
    out.push(quote);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_python2_repr() {
        let text = r#"{'info': 'Pr\xe9l\xe8vement \x80', 'categ': 18, 'do_mode': False, 'n': None, 'contains': '="gaz" in tiers', 'l': [1, -2.5, (3,)], 'x': 5L, u'u': u'\u20ac'}"#;
        let v = parse_repr(text).expect("littéral valide");
        assert_eq!(v.text("info"), "Prélèvement €");
        assert_eq!(v.get("categ").and_then(PyValue::as_i64), Some(18));
        assert!(!v.flag("do_mode"));
        assert!(!v.flag("absent"));
        assert_eq!(v.get("n"), Some(&PyValue::None));
        assert_eq!(v.text("contains"), "=\"gaz\" in tiers");
        assert_eq!(
            v.get("l"),
            Some(&PyValue::List(vec![
                PyValue::Int(1),
                PyValue::Float(-2.5),
                PyValue::Tuple(vec![PyValue::Int(3)])
            ]))
        );
        assert_eq!(v.get("x").and_then(PyValue::as_i64), Some(5));
        assert_eq!(v.text("u"), "€");
    }

    #[test]
    fn rejects_code() {
        for text in [
            "__import__('os').system('rm -rf /')",
            "{'a': open('x')}",
            "{'a': 1",
            "[1 2]",
            "'abc",
            "{'a' 1}",
            "1 + 1",
            "",
        ] {
            assert!(parse_repr(text).is_err(), "{text:?}");
        }
        assert!(parse_repr(&"[".repeat(1000)).is_err());
    }

    #[test]
    fn repr_round_trip() {
        let value = PyValue::Dict(vec![
            (
                PyValue::Str("info".into()),
                PyValue::Str("Prélèvement l'été €".into()),
            ),
            (PyValue::Str("q".into()), PyValue::Str("a\"b'c\\\n".into())),
            (PyValue::Str("n".into()), PyValue::Float(-12.0)),
            (
                PyValue::Str("t".into()),
                PyValue::Tuple(vec![PyValue::Int(1)]),
            ),
            (PyValue::Str("j".into()), PyValue::Str("日本 🎉".into())),
        ]);
        let text = to_repr(&value);
        assert!(text.is_ascii(), "{text}");
        assert!(
            text.contains(r#""Pr\xe9l\xe8vement l'\xe9t\xe9 \x80""#),
            "{text}"
        );
        assert!(text.contains("-12.0"));
        assert!(text.contains("(1,)"));
        assert!(text.contains(r"u'\u65e5\u672c \U0001f389'"), "{text}");
        assert_eq!(parse_repr(&text), Ok(value));
    }
}
