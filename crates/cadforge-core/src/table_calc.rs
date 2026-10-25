//! Table formulas and data links. A cell whose text starts with `=` shows the value of its formula: numbers, cell
//! references (`B2`), ranges (`A1:A5`), `+ - * / ^`, parentheses and the functions SUM, AVERAGE, COUNT, MIN, MAX,
//! ABS, ROUND and SQRT. Text cells count as zero in arithmetic and are left out of SUM, AVERAGE, COUNT, MIN and MAX.
//! A formula that cannot be worked out shows an error code (`#REF!`, `#DIV/0!`, `#NAME?`, `#CYCLE!`, `#ERROR!`).
//!
//! A data link ties a table to a CSV file: [`fill_from_csv`] puts the file's cells into the table and
//! [`to_csv`] writes the table back.
use crate::entities::TableData;

/// What each cell of the table shows: its text, or the value of its formula.
pub fn display(t: &TableData) -> Vec<Vec<String>> {
    let mut e = Eval { t, visiting: vec![] };
    t.cells
        .iter()
        .enumerate()
        .map(|(r, row)| {
            row.iter()
                .enumerate()
                .map(|(c, s)| match formula(s) {
                    Some(_) => match e.cell(r, c) {
                        Ok(Some(v)) => format_value(v),
                        Ok(None) => String::new(),
                        Err(code) => code.to_string(),
                    },
                    None => s.clone(),
                })
                .collect()
        })
        .collect()
}

/// The value of one cell (a number, a formula, or `None` for text and empty cells).
pub fn value(t: &TableData, r: usize, c: usize) -> Result<Option<f64>, &'static str> {
    Eval { t, visiting: vec![] }.cell(r, c)
}

fn formula(s: &str) -> Option<&str> {
    s.trim_start().strip_prefix('=')
}

/// Up to six decimals, without trailing zeros.
pub fn format_value(v: f64) -> String {
    let r = (v * 1e6).round() / 1e6;
    let s = format!("{r:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// `"B12"` -> (row 11, column 1).
pub fn parse_ref(s: &str) -> Option<(usize, usize)> {
    let s = s.trim().to_ascii_uppercase();
    let letters: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = &s[letters.len()..];
    if letters.is_empty() || letters.len() > 3 || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let col = letters.bytes().fold(0usize, |n, b| n * 26 + (b - b'A' + 1) as usize) - 1;
    let row = digits.parse::<usize>().ok()?.checked_sub(1)?;
    Some((row, col))
}

struct Eval<'a> {
    t: &'a TableData,
    visiting: Vec<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ref(usize, usize),
    Name(String),
    Op(char),
}

fn tokens(s: &str) -> Result<Vec<Tok>, &'static str> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < cs.len() {
        let ch = cs[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() || (ch == '.' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let st = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                i += 1;
            }
            if i < cs.len() && (cs[i] == 'e' || cs[i] == 'E') && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == '-' || *d == '+') {
                i += 2;
                while i < cs.len() && cs[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let t: String = cs[st..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| "#ERROR!")?));
        } else if ch.is_ascii_alphabetic() {
            let st = i;
            while i < cs.len() && cs[i].is_ascii_alphanumeric() {
                i += 1;
            }
            let w: String = cs[st..i].iter().collect();
            match parse_ref(&w) {
                Some((r, c)) => out.push(Tok::Ref(r, c)),
                None => out.push(Tok::Name(w.to_ascii_uppercase())),
            }
        } else if "+-*/^(),:".contains(ch) {
            out.push(Tok::Op(ch));
            i += 1;
        } else {
            return Err("#ERROR!");
        }
    }
    Ok(out)
}

/// One argument of a function: a single value, or every value of a range (text and empty cells left out).
enum Arg {
    One(f64),
    Many(Vec<f64>),
}

struct P<'e, 'a> {
    e: &'e mut Eval<'a>,
    toks: Vec<Tok>,
    i: usize,
}

impl P<'_, '_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }
    fn eat(&mut self, op: char) -> bool {
        if self.peek() == Some(&Tok::Op(op)) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<f64, &'static str> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                return Ok(v);
            }
        }
    }
    fn term(&mut self) -> Result<f64, &'static str> {
        let mut v = self.power()?;
        loop {
            if self.eat('*') {
                v *= self.power()?;
            } else if self.eat('/') {
                let d = self.power()?;
                if d == 0.0 {
                    return Err("#DIV/0!");
                }
                v /= d;
            } else {
                return Ok(v);
            }
        }
    }
    fn power(&mut self) -> Result<f64, &'static str> {
        let b = self.unary()?;
        if self.eat('^') {
            let x = self.power()?;
            let v = b.powf(x);
            return if v.is_finite() { Ok(v) } else { Err("#ERROR!") };
        }
        Ok(b)
    }
    fn unary(&mut self) -> Result<f64, &'static str> {
        if self.eat('-') {
            return Ok(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        self.primary()
    }
    fn primary(&mut self) -> Result<f64, &'static str> {
        match self.toks.get(self.i).cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(n)
            }
            Some(Tok::Ref(r, c)) => {
                self.i += 1;
                if self.peek() == Some(&Tok::Op(':')) {
                    return Err("#ERROR!"); // a range outside a function
                }
                Ok(self.e.cell(r, c)?.unwrap_or(0.0))
            }
            Some(Tok::Op('(')) => {
                self.i += 1;
                let v = self.expr()?;
                if !self.eat(')') {
                    return Err("#ERROR!");
                }
                Ok(v)
            }
            Some(Tok::Name(f)) => {
                self.i += 1;
                if !self.eat('(') {
                    return Err("#NAME?");
                }
                let mut args = vec![];
                if !self.eat(')') {
                    loop {
                        args.push(self.arg()?);
                        if self.eat(')') {
                            break;
                        }
                        if !self.eat(',') {
                            return Err("#ERROR!");
                        }
                    }
                }
                call(&f, args)
            }
            _ => Err("#ERROR!"),
        }
    }
    fn arg(&mut self) -> Result<Arg, &'static str> {
        if let (Some(Tok::Ref(r0, c0)), Some(Tok::Op(':')), Some(Tok::Ref(r1, c1))) = (self.toks.get(self.i).cloned(), self.toks.get(self.i + 1).cloned(), self.toks.get(self.i + 2).cloned()) {
            self.i += 3;
            let mut vals = vec![];
            for r in r0.min(r1)..=r0.max(r1) {
                for c in c0.min(c1)..=c0.max(c1) {
                    if let Some(v) = self.e.cell(r, c)? {
                        vals.push(v);
                    }
                }
            }
            return Ok(Arg::Many(vals));
        }
        if let (Some(Tok::Ref(r, c)), next) = (self.toks.get(self.i).cloned(), self.toks.get(self.i + 1).cloned()) {
            // a lone reference to a text or empty cell is left out, like a range
            if matches!(next, Some(Tok::Op(',')) | Some(Tok::Op(')'))) {
                self.i += 1;
                return Ok(Arg::Many(self.e.cell(r, c)?.into_iter().collect()));
            }
        }
        Ok(Arg::One(self.expr()?))
    }
}

fn call(f: &str, args: Vec<Arg>) -> Result<f64, &'static str> {
    let flat: Vec<f64> = args.iter().flat_map(|a| match a {
        Arg::One(v) => vec![*v],
        Arg::Many(v) => v.clone(),
    }).collect();
    let one = |i: usize| match args.get(i) {
        Some(Arg::One(v)) => Ok(*v),
        Some(Arg::Many(v)) if v.len() == 1 => Ok(v[0]),
        _ => Err("#ERROR!"),
    };
    match f {
        "SUM" => Ok(flat.iter().sum()),
        "AVERAGE" | "AVG" => {
            if flat.is_empty() {
                Err("#DIV/0!")
            } else {
                Ok(flat.iter().sum::<f64>() / flat.len() as f64)
            }
        }
        "COUNT" => Ok(flat.len() as f64),
        "MIN" => flat.iter().copied().reduce(f64::min).ok_or("#ERROR!"),
        "MAX" => flat.iter().copied().reduce(f64::max).ok_or("#ERROR!"),
        "ABS" if args.len() == 1 => Ok(one(0)?.abs()),
        "SQRT" if args.len() == 1 => {
            let v = one(0)?;
            if v < 0.0 {
                Err("#ERROR!")
            } else {
                Ok(v.sqrt())
            }
        }
        "ROUND" if (1..=2).contains(&args.len()) => {
            let v = one(0)?;
            let n = if args.len() == 2 { one(1)?.round() } else { 0.0 };
            let k = 10f64.powf(n);
            Ok((v * k).round() / k)
        }
        _ => Err("#NAME?"),
    }
}

impl Eval<'_> {
    /// The number a cell holds: its formula's value, its text as a number, or `None` (text or empty).
    fn cell(&mut self, r: usize, c: usize) -> Result<Option<f64>, &'static str> {
        let Some(s) = self.t.cells.get(r).and_then(|row| row.get(c)) else { return Err("#REF!") };
        match formula(s) {
            None => Ok(s.trim().parse::<f64>().ok().filter(|v| v.is_finite())),
            Some(f) => {
                if self.visiting.contains(&(r, c)) {
                    return Err("#CYCLE!");
                }
                self.visiting.push((r, c));
                let toks = tokens(f);
                let v = toks.and_then(|toks| {
                    let mut p = P { e: self, toks, i: 0 };
                    let v = p.expr()?;
                    if p.i != p.toks.len() {
                        return Err("#ERROR!");
                    }
                    Ok(v)
                });
                self.visiting.pop();
                v.map(Some)
            }
        }
    }
}

/// The cells of a CSV file (quoted fields, doubled quotes, line breaks inside quotes; a `;` file is read too).
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let delim = if first.matches(';').count() > first.matches(',').count() { ';' } else { ',' };
    let mut rows = vec![];
    let mut row = vec![];
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(ch);
            }
        } else if ch == '"' && field.is_empty() {
            quoted = true;
        } else if ch == delim {
            row.push(std::mem::take(&mut field));
        } else if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(ch);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    // drop trailing empty lines
    while rows.last().is_some_and(|r: &Vec<String>| r.iter().all(String::is_empty)) {
        rows.pop();
    }
    rows
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The table as CSV: what each cell shows (formula values, not formulas).
pub fn to_csv(t: &TableData) -> String {
    let mut out: String = display(t).iter().map(|r| r.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join("\r\n");
    out.push_str("\r\n");
    out
}

/// Put the cells of a CSV file into the table, starting at the top-left cell: rows and columns are added (with the
/// size of the last one) or removed to fit, and merges that no longer fit are dropped. A title row stays merged.
pub fn fill_from_csv(t: &mut TableData, cells: &[Vec<String>]) {
    let nr = cells.len().max(1);
    let nc = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let (rh, cw) = (t.rows.last().copied().unwrap_or(t.h * 2.0), t.cols.last().copied().unwrap_or(t.h * 8.0));
    t.rows.resize(nr, rh);
    t.cols.resize(nc, cw);
    t.cells = (0..nr).map(|r| (0..nc).map(|c| cells.get(r).and_then(|row| row.get(c)).cloned().unwrap_or_default()).collect()).collect();
    t.merges.retain(|m| m[2] < nr && m[3] < nc);
    t.title = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::pt;

    fn table(cells: &[&[&str]]) -> TableData {
        let nc = cells.iter().map(|r| r.len()).max().unwrap_or(1);
        TableData {
            p: pt(0.0, 0.0),
            cols: vec![10.0; nc],
            rows: vec![5.0; cells.len()],
            cells: cells.iter().map(|r| r.iter().map(|s| s.to_string()).collect()).collect(),
            h: 2.5,
            merges: vec![],
            style: String::new(),
            title: false,
            rot: 0.0,
            link: None,
        }
    }

    #[test]
    fn references_and_ranges() {
        assert_eq!(parse_ref("A1"), Some((0, 0)));
        assert_eq!(parse_ref("c12"), Some((11, 2)));
        assert_eq!(parse_ref("AA3"), Some((2, 26)));
        assert_eq!(parse_ref("A0"), None);
        assert_eq!(parse_ref("SUM"), None);
    }

    #[test]
    fn formulas_show_their_values() {
        let t = table(&[
            &["Item", "Qty", "Price", "Total"],
            &["Door", "2", "150", "=B2*C2"],
            &["Window", "3", "99.5", "=B3*C3"],
            &["Sum", "=SUM(B2:B3)", "=AVERAGE(C2:C3)", "=SUM(D2:D3)"],
            &["", "=COUNT(A1:D3)", "=MAX(C2:C3)-MIN(C2,C3)", "=ROUND(D4/7, 2)"],
            &["", "=(1+2)*3^2", "=-B2 + ABS(-4)", "=SQRT(16)/0"],
        ]);
        let d = display(&t);
        assert_eq!(d[1][3], "300");
        assert_eq!(d[2][3], "298.5");
        assert_eq!((d[3][1].as_str(), d[3][2].as_str(), d[3][3].as_str()), ("5", "124.75", "598.5"));
        assert_eq!(d[4][1], "6", "COUNT counts the numbers (formulas included), not the text");
        assert_eq!(d[4][2], "50.5");
        assert_eq!(d[4][3], "85.5");
        assert_eq!((d[5][1].as_str(), d[5][2].as_str(), d[5][3].as_str()), ("27", "2", "#DIV/0!"));
        assert_eq!(d[0][0], "Item", "text stays text");
    }

    #[test]
    fn errors_are_shown_as_codes() {
        let t = table(&[&["=B1", "=A1", "=Z9", "=FOO(1)"], &["=1+", "=SUM(1, 2", "=A1:B1", "=2*(3"]]);
        let d = display(&t);
        assert_eq!(&d[0], &["#CYCLE!", "#CYCLE!", "#REF!", "#NAME?"]);
        assert_eq!(&d[1], &["#ERROR!", "#ERROR!", "#ERROR!", "#ERROR!"]);
        assert_eq!(value(&table(&[&["text", "=A1+1"]]), 0, 1), Ok(Some(1.0)), "text counts as zero in arithmetic");
    }

    #[test]
    fn csv_in_and_out() {
        let cells = parse_csv("Item,Note\r\nDoor,\"2 x \"\"steel\"\", painted\"\nWindow,\"line one\nline two\"\n\n");
        assert_eq!(cells, vec![vec!["Item", "Note"], vec!["Door", "2 x \"steel\", painted"], vec!["Window", "line one\nline two"]]);
        assert_eq!(parse_csv("a;b;c\n1;2;3"), vec![vec!["a", "b", "c"], vec!["1", "2", "3"]]);
        let mut t = table(&[&["x", "y"], &["1", "=A2*2"]]);
        t.merges.push([0, 0, 0, 1]);
        assert_eq!(to_csv(&t), "x,y\r\n1,2\r\n", "values, not formulas");
        fill_from_csv(&mut t, &cells);
        assert_eq!((t.rows.len(), t.cols.len(), t.cells[2][1].as_str()), (3, 2, "line one\nline two"));
        assert_eq!(t.merges.len(), 1, "a merge that still fits stays");
        fill_from_csv(&mut t, &[vec!["only".into()]]);
        assert_eq!((t.rows.len(), t.cols.len(), t.merges.len()), (1, 1, 0));
    }
}
