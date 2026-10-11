//! Calculatrice scientifique (ex-script `calc.py`) : évaluateur d'expressions
//! sûr, sans `eval`.
//!
//! Syntaxe : nombres (virgule ou point décimal, notation `1E3`), `+ - * / ^ %`,
//! `²`, parenthèses, constantes `pi` et `e`, fonctions `sqrt sin cos tan asin
//! acos atan sinh cosh tanh ln log deg rad` (angles en radians), et la saisie
//! en degrés-minutes-secondes `12° 30' 15"`.

/// Résultat d'un calcul : nombre, et format DMS si l'expression en contenait.
pub fn evaluate(expression: &str) -> Result<(f64, bool), String> {
    let dms = expression.contains(['°', '\'', '"']);
    let tokens = tokenize(expression)?;
    let mut parser = Parser { tokens, pos: 0 };
    let value = parser.expression()?;
    if parser.pos != parser.tokens.len() {
        return Err("expression incomplète".into());
    }
    if !value.is_finite() {
        return Err("résultat indéfini".into());
    }
    Ok((value, dms))
}

/// Affichage : DMS `12° 30' 15"` si demandé, sinon nombre décimal à virgule.
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
    text.replace('.', ",")
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
                let literal: String = chars[start..i].iter().collect::<String>().replace(',', ".");
                let value = literal
                    .parse::<f64>()
                    .map_err(|_| format!("nombre invalide « {literal} »"))?;
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
            other => return Err(format!("caractère inattendu « {other} »")),
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
                    return Err("division par zéro".into());
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
                    _ => Err("parenthèse fermante manquante".into()),
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
                    "sqrt" if arg < 0.0 => return Err("racine d'un nombre négatif".into()),
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
                    other => return Err(format!("fonction inconnue « {other} »")),
                };
                Ok(v)
            }
            _ => Err("expression incomplète".into()),
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
}
