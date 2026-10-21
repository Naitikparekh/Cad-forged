//! The AutoLISP reader: text → values.
use super::value::*;

struct Rd<'a> {
    s: &'a [char],
    i: usize,
}

/// Parse every expression of `src`.
pub fn read_all(src: &str) -> Result<Vec<Value>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut r = Rd { s: &chars, i: 0 };
    let mut out = vec![];
    loop {
        r.skip_ws()?;
        if r.i >= r.s.len() {
            return Ok(out);
        }
        out.push(r.expr()?);
    }
}

/// Parse the first expression of `src` (used by `read`); `Ok(None)` when it holds only blanks / comments.
pub fn read_first(src: &str) -> Result<Option<Value>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut r = Rd { s: &chars, i: 0 };
    r.skip_ws()?;
    if r.i >= r.s.len() {
        return Ok(None);
    }
    r.expr().map(Some)
}

/// True when `src` has more opening than closing parentheses (an unfinished expression).
pub fn unbalanced(src: &str) -> bool {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut prev = ' ';
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            if c == '"' && prev != '\\' {
                in_str = false;
            }
        } else {
            match c {
                '"' => in_str = true,
                ';' => {
                    for n in chars.by_ref() {
                        if n == '\n' {
                            break;
                        }
                    }
                }
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        prev = if prev == '\\' && c == '\\' { ' ' } else { c };
    }
    depth > 0 || in_str
}

impl Rd<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) -> Result<(), String> {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.i += 1;
            } else if c == ';' {
                if self.s.get(self.i + 1) == Some(&'|') {
                    // ;| block comment |;
                    self.i += 2;
                    loop {
                        match self.peek() {
                            None => return Err("unterminated block comment".into()),
                            Some('|') if self.s.get(self.i + 1) == Some(&';') => {
                                self.i += 2;
                                break;
                            }
                            _ => self.i += 1,
                        }
                    }
                } else {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.i += 1;
                    }
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    fn expr(&mut self) -> Result<Value, String> {
        self.skip_ws()?;
        match self.peek() {
            None => Err("unexpected end of input".into()),
            Some('(') => {
                self.i += 1;
                self.list()
            }
            Some(')') => Err("extra right paren".into()),
            Some('\'') => {
                self.i += 1;
                let x = self.expr()?;
                Ok(Value::list(vec![Value::sym("QUOTE"), x]))
            }
            Some('"') => {
                self.i += 1;
                self.string()
            }
            Some(_) => Ok(self.atom()),
        }
    }

    fn list(&mut self) -> Result<Value, String> {
        let mut items: Vec<Value> = vec![];
        let mut tail = Value::Nil;
        loop {
            self.skip_ws()?;
            match self.peek() {
                None => return Err("missing right paren".into()),
                Some(')') => {
                    self.i += 1;
                    break;
                }
                Some('.') if self.is_dot() => {
                    self.i += 1;
                    tail = self.expr()?;
                    self.skip_ws()?;
                    if self.peek() != Some(')') {
                        return Err("malformed dotted pair".into());
                    }
                    self.i += 1;
                    break;
                }
                _ => items.push(self.expr()?),
            }
        }
        Ok(items.into_iter().rev().fold(tail, |t, x| Value::cons(x, t)))
    }

    /// A lone `.` (dotted-pair marker), not the start of a number like `.5`.
    fn is_dot(&self) -> bool {
        self.s.get(self.i + 1).is_none_or(|c| c.is_whitespace() || *c == '(' || *c == ')' || *c == '"' || *c == '\'')
    }

    fn string(&mut self) -> Result<Value, String> {
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated string".into()),
                Some('"') => {
                    self.i += 1;
                    return Ok(Value::str(&out));
                }
                Some('\\') => {
                    self.i += 1;
                    match self.peek() {
                        Some('n') => out.push('\n'),
                        Some('t') => out.push('\t'),
                        Some('r') => out.push('\r'),
                        Some('e') => out.push('\u{1b}'),
                        Some('"') => out.push('"'),
                        Some('\\') => out.push('\\'),
                        Some(c) if c.is_ascii_digit() => {
                            // \nnn: octal character code
                            let mut n = 0u32;
                            let mut k = 0;
                            while let Some(d) = self.peek().and_then(|c| c.to_digit(8)) {
                                n = n * 8 + d;
                                self.i += 1;
                                k += 1;
                                if k == 3 {
                                    break;
                                }
                            }
                            out.push(char::from_u32(n).unwrap_or('?'));
                            continue;
                        }
                        Some(c) => {
                            out.push('\\');
                            out.push(c);
                        }
                        None => return Err("unterminated string".into()),
                    }
                    self.i += 1;
                }
                Some(c) => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn atom(&mut self) -> Value {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | ';') {
                break;
            }
            self.i += 1;
        }
        let tok: String = self.s[start..self.i].iter().collect();
        if let Ok(n) = tok.parse::<i64>() {
            return Value::Int(n);
        }
        let looks_numeric = tok.chars().next().is_some_and(|c| c.is_ascii_digit() || ((c == '-' || c == '+' || c == '.') && tok.len() > 1 && tok.chars().nth(1).is_some_and(|d| d.is_ascii_digit() || d == '.')));
        if looks_numeric {
            if let Ok(r) = tok.parse::<f64>() {
                return Value::Real(r);
            }
        }
        match tok.to_uppercase().as_str() {
            "NIL" => Value::Nil,
            "T" => Value::T,
            _ => Value::sym(&tok),
        }
    }
}
