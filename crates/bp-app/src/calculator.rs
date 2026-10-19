//! Calculatrice scientifique (ex-script `calc.py`) : évaluateur d'expressions
//! sûr, sans `eval`.
//!
//! Syntaxe : nombres (virgule ou point décimal, notation `1E3`), `+ - * / ^ %`,
//! `²`, parenthèses, constantes `pi` et `e`, fonctions `sqrt sin cos tan asin
//! acos atan sinh cosh tanh ln log deg rad` (angles en radians), et la saisie
//! en degrés-minutes-secondes `12° 30' 15"`.

use bp_core::i18n::{self, Lang};
use bp_core::tr;

/// Résultat d'un calcul : nombre, et format DMS si l'expression en contenait.
pub fn evaluate(expression: &str) -> Result<(f64, bool), String> {
    let dms = expression.contains(['°', '\'', '"']);
    let tokens = tokenize(expression)?;
    let mut parser = Parser { tokens, pos: 0 };
    let value = parser.expression()?;
    if parser.pos != parser.tokens.len() {
        return Err(tr!("expression incomplète").into());
    }
    if !value.is_finite() {
        return Err(tr!("résultat indéfini").into());
    }
    Ok((value, dms))
}

/// Affichage : DMS `12° 30' 15"` si demandé, sinon nombre décimal (virgule
/// en français, point en anglais).
pub fn format(value: f64, dms: bool) -> String {
    if dms {
        let sign = if value < 0.0 { "-" } else { "" };
        let v = value.abs();
        let d = v.trunc();
        let m = ((v - d) * 60.0).trunc();
        let s = ((v - d) * 60.0 - m) * 60.0;
        return format!("{sign}{d}° {m}' {:.0}\"", s.round());
    }
    let text = if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        let t = format!("{value:.12}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    text.replace('.', &i18n::decimal_separator().to_string())
}

/// Nombre tapé, au format de Rust (point décimal, sans séparateur des
/// milliers). En français, la virgule est décimale. En anglais, le point
/// est décimal et la virgule sépare les milliers (« 1,234.5 ») ; une
/// virgule seule reste décimale (« 2,5 ») sauf suivie de trois chiffres
/// après un entier non nul (« 1,234 »).
fn number_literal(raw: &str) -> String {
    if i18n::lang() == Lang::En {
        let thousands = raw.contains('.')
            || raw.matches(',').count() > 1
            || raw.split_once(',').is_some_and(|(int, frac)| {
                frac.len() == 3
                    && frac.chars().all(|c| c.is_ascii_digit())
                    && (1..=3).contains(&int.len())
                    && !int.starts_with('0')
            });
        if thousands {
            return raw.replace(',', "");
        }
    }
    raw.replace(',', ".")
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Op(char),
    Open,
    Close,
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '0'..='9' | '.' | ',' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == ',')
                {
                    i += 1;
                }
                // Exposant : 1E3, 2,5E-4
                if i < chars.len() && (chars[i] == 'E') {
                    i += 1;
                    if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
                        i += 1;
                    }
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let raw: String = chars[start..i].iter().collect();
                let value = number_literal(&raw)
                    .parse::<f64>()
                    .map_err(|_| tr!("nombre invalide « {} »", raw))?;
                tokens.push(Token::Number(value));
            }
            'a'..='z' | 'A'..='Z' | 'π' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == 'π') {
                    i += 1;
                }
                tokens.push(Token::Ident(
                    chars[start..i].iter().collect::<String>().to_lowercase(),
                ));
            }
            '+' | '-' | '–' | '*' | '×' | '/' | '÷' | '^' | '%' | '²' | '°' | '\'' | '"' => {
                let op = match c {
                    '–' => '-',
                    '×' => '*',
                    '÷' => '/',
                    other => other,
                };
                tokens.push(Token::Op(op));
                i += 1;
            }
            '(' => {
                tokens.push(Token::Open);
                i += 1;
            }
            ')' => {
                tokens.push(Token::Close);
                i += 1;
            }
            '√' => {
                tokens.push(Token::Ident("sqrt".into()));
                i += 1;
            }
            other => return Err(tr!("caractère inattendu « {} »", other)),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn eat_op(&mut self, ops: &[char]) -> Option<char> {
        match self.peek() {
            Some(Token::Op(c)) if ops.contains(c) => {
                let c = *c;
                self.pos += 1;
                Some(c)
            }
            _ => None,
        }
    }

    /// Somme de termes. Les parties DMS (`12° 30' 15"`) s'additionnent.
    fn expression(&mut self) -> Result<f64, String> {
        let mut value = self.term()?;
        loop {
            match self.eat_op(&['+', '-']) {
                Some('+') => value += self.term()?,
                Some(_) => value -= self.term()?,
                None => {
                    // Partie DMS qui suit sans opérateur : 12° 30'
                    if matches!(self.peek(), Some(Token::Number(_))) && self.last_was_dms() {
                        value += self.term()?;
                    } else {
                        return Ok(value);
                    }
                }
            }
        }
    }

    fn last_was_dms(&self) -> bool {
        matches!(
            self.tokens.get(self.pos.wrapping_sub(1)),
            Some(Token::Op('°' | '\'' | '"'))
        )
    }

    fn term(&mut self) -> Result<f64, String> {
        let mut value = self.power()?;
        while let Some(op) = self.eat_op(&['*', '/']) {
            let rhs = self.power()?;
            if op == '*' {
                value *= rhs;
            } else {
                if rhs == 0.0 {
                    return Err(tr!("division par zéro").into());
                }
                value /= rhs;
            }
        }
        Ok(value)
    }

    fn power(&mut self) -> Result<f64, String> {
        let base = self.unary()?;
        if self.eat_op(&['^']).is_some() {
            return Ok(base.powf(self.power()?));
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<f64, String> {
        match self.eat_op(&['-', '+']) {
            Some('-') => Ok(-self.unary()?),
            Some(_) => self.unary(),
            None => self.postfix(),
        }
    }

    /// Suffixes : `²`, `%`, et unités DMS `°` `'` `"`.
    fn postfix(&mut self) -> Result<f64, String> {
        let mut value = self.atom()?;
        while let Some(op) = self.eat_op(&['²', '%', '°', '\'', '"']) {
            value = match op {
                '²' => value * value,
                '%' => value / 100.0,
                '°' => value,
                '\'' => value / 60.0,
                _ => value / 3600.0,
            };
        }
        Ok(value)
    }

    fn atom(&mut self) -> Result<f64, String> {
        match self.tokens.get(self.pos).cloned() {
            Some(Token::Number(v)) => {
                self.pos += 1;
                Ok(v)
            }
            Some(Token::Open) => {
                self.pos += 1;
                let v = self.expression()?;
                match self.peek() {
                    Some(Token::Close) => {
                        self.pos += 1;
                        Ok(v)
                    }
                    _ => Err(tr!("parenthèse fermante manquante").into()),
                }
            }
            Some(Token::Ident(name)) => {
                self.pos += 1;
                match name.as_str() {
                    "pi" | "π" => return Ok(std::f64::consts::PI),
                    "e" if !matches!(self.peek(), Some(Token::Open)) => {
                        return Ok(std::f64::consts::E);
                    }
                    _ => {}
                }
                let arg = self.atom()?;
                let v = match name.as_str() {
                    "sqrt" if arg < 0.0 => {
                        return Err(tr!("racine d'un nombre négatif").into());
                    }
                    "sqrt" => arg.sqrt(),
                    "sin" => arg.sin(),
                    "cos" => arg.cos(),
                    "tan" => arg.tan(),
                    "asin" => arg.asin(),
                    "acos" => arg.acos(),
                    "atan" => arg.atan(),
                    "sinh" => arg.sinh(),
                    "cosh" => arg.cosh(),
                    "tanh" => arg.tanh(),
                    "ln" => arg.ln(),
                    "log" => arg.log10(),
                    "deg" => arg.to_degrees(),
                    "rad" => arg.to_radians(),
                    other => return Err(tr!("fonction inconnue « {} »", other)),
                };
                Ok(v)
            }
            _ => Err(tr!("expression incomplète").into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(s: &str) -> f64 {
        evaluate(s).unwrap().0
    }

    #[test]
    fn arithmetic() {
        assert_eq!(eval("1+2*3"), 7.0);
        assert_eq!(eval("(1+2)*3"), 9.0);
        assert_eq!(eval("2^3^2"), 512.0);
        assert_eq!(eval("-3²"), -9.0);
        assert_eq!(eval("50%*200"), 100.0);
        assert_eq!(eval("1,5+2.5"), 4.0);
        assert_eq!(eval("2E3"), 2000.0);
        assert!(evaluate("1/0").is_err());
        assert!(evaluate("(1+2").is_err());
        assert!(evaluate("1+").is_err());
    }

    #[test]
    fn functions_and_constants() {
        assert!((eval("sin(pi/2)") - 1.0).abs() < 1e-12);
        assert!((eval("deg(pi)") - 180.0).abs() < 1e-9);
        assert!((eval("ln(e)") - 1.0).abs() < 1e-12);
        assert_eq!(eval("log(1000)"), 3.0);
        assert_eq!(eval("sqrt(16)"), 4.0);
    }

    #[test]
    fn degrees_minutes_seconds() {
        let (v, dms) = evaluate("12° 30' 0\" + 1° 45'").unwrap();
        assert!(dms);
        assert!((v - 14.25).abs() < 1e-9);
        assert_eq!(format(v, true), "14° 15' 0\"");
        assert_eq!(format(2.5, false), "2,5");
        assert_eq!(format(3.0, false), "3");
    }

    #[test]
    fn english_numbers() {
        i18n::with_lang(Lang::En, || {
            assert_eq!(eval("1,234.5+0.5"), 1235.0);
            assert_eq!(eval("1,234*2"), 2468.0);
            assert_eq!(eval("1,000,000/4"), 250_000.0);
            assert_eq!(eval("2,5*2"), 5.0);
            assert_eq!(eval("0,125*8"), 1.0);
            assert_eq!(format(2.5, false), "2.5");
            assert!(evaluate("1/0").unwrap_err().contains("zero"));
        });
        assert_eq!(eval("1,234*1000"), 1234.0);
    }
}
