//! Parsing of command-line input: coordinates, numbers, object-snap overrides and keyword options
//! (port of `src/engine/parse.js`).
use cadforge_core::geom::{pt, rad, Pt};

/// Scan an unsigned/signed decimal number at the start of `s`: `[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?`.
/// Returns the value and the number of bytes consumed.
fn scan_number(s: &str) -> Option<(f64, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(i);
    let mut j = i + int;
    if int > 0 {
        if j < b.len() && b[j] == b'.' {
            j += 1;
            j += digits(j);
        }
    } else if j < b.len() && b[j] == b'.' && digits(j + 1) > 0 {
        j += 1;
        j += digits(j);
    } else {
        return None;
    }
    if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        let d = digits(k);
        if d > 0 {
            j = k + d;
        }
    }
    s[..j].parse::<f64>().ok().map(|v| (v, j))
}

fn skip_ws(s: &str) -> &str {
    s.trim_start()
}

/// A whole-string decimal number.
fn full_number(s: &str) -> Option<f64> {
    match scan_number(s) {
        Some((v, n)) if n == s.len() => Some(v),
        _ => None,
    }
}

/// Parse a number or a fraction: `12.5`, `1/2`, `3-1/4`, `-2 1/8`.
pub fn parse_number(text: &str) -> Option<f64> {
    let t = text.trim();
    if let Some(v) = full_number(t) {
        return Some(v);
    }
    let (sign, rest) = match t.strip_prefix('-') {
        Some(r) => (-1.0, r),
        None => (1.0, t.strip_prefix('+').unwrap_or(t)),
    };
    let (num, den) = rest.rsplit_once('/')?;
    let den: f64 = digits_only(den)?;
    if den == 0.0 {
        return None;
    }
    let (whole, numer) = match num.find(|c: char| c == ' ' || c == '\t' || c == '-') {
        Some(i) => (digits_only(&num[..i])?, digits_only(&num[i + 1..])?),
        None => (0.0, digits_only(num)?),
    };
    Some(sign * (whole + numer / den))
}

fn digits_only(s: &str) -> Option<f64> {
    if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

// ---------------------------------------------------------------- object snaps

pub const OS_END: u32 = 1;
pub const OS_MID: u32 = 2;
pub const OS_CEN: u32 = 4;
pub const OS_NOD: u32 = 8;
pub const OS_QUA: u32 = 16;
pub const OS_INT: u32 = 32;
pub const OS_INS: u32 = 64;
pub const OS_PER: u32 = 128;
pub const OS_TAN: u32 = 256;
pub const OS_NEA: u32 = 512;
pub const OS_QUI: u32 = 1024;
pub const OS_APP: u32 = 2048;
pub const OS_EXT: u32 = 4096;
pub const OS_PAR: u32 = 8192;
/// OSMODE bit that suspends all running object snaps.
pub const OS_OFF: u32 = 16384;

pub const OS_BITS: [(&str, u32); 14] = [
    ("END", OS_END),
    ("MID", OS_MID),
    ("CEN", OS_CEN),
    ("NOD", OS_NOD),
    ("QUA", OS_QUA),
    ("INT", OS_INT),
    ("INS", OS_INS),
    ("PER", OS_PER),
    ("TAN", OS_TAN),
    ("NEA", OS_NEA),
    ("QUI", OS_QUI),
    ("APP", OS_APP),
    ("EXT", OS_EXT),
    ("PAR", OS_PAR),
];

pub fn os_bit(code: &str) -> u32 {
    OS_BITS.iter().find(|(c, _)| *c == code).map(|(_, b)| *b).unwrap_or(0)
}

pub fn os_full(code: &str) -> &'static str {
    match code {
        "END" => "Endpoint",
        "MID" => "Midpoint",
        "CEN" => "Center",
        "NOD" => "Node",
        "QUA" => "Quadrant",
        "INT" => "Intersection",
        "INS" => "Insertion",
        "PER" => "Perpendicular",
        "TAN" => "Tangent",
        "NEA" => "Nearest",
        "APP" => "Apparent Intersection",
        "EXT" => "Extension",
        "PAR" => "Parallel",
        _ => "",
    }
}

fn os_alias(t: &str) -> Option<&'static str> {
    Some(match t {
        "ENDPOINT" => "END",
        "MIDPOINT" => "MID",
        "CENTER" | "CENTRE" => "CEN",
        "NODE" => "NOD",
        "QUADRANT" => "QUA",
        "INTERSECTION" => "INT",
        "INSERTION" | "INSERT" => "INS",
        "PERPENDICULAR" => "PER",
        "TANGENT" => "TAN",
        "NEAREST" => "NEA",
        "APPINT" => "APP",
        "EXTENSION" => "EXT",
        "PARALLEL" => "PAR",
        _ => return None,
    })
}

/// A typed object-snap override or temporary-point helper.
#[derive(Clone, Debug, PartialEq)]
pub enum Override {
    Osnap(&'static str),
    From,
    Tt,
    None,
    M2p,
}

/// Recognise typed object-snap overrides such as END, MID, CEN, FROM, TT, NONE.
pub fn parse_override(text: &str) -> Option<Override> {
    let t = text.trim().to_uppercase();
    if t.len() >= 3 && t.is_char_boundary(3) {
        let three = &t[..3];
        if let Some((code, _)) = OS_BITS.iter().find(|(c, _)| *c == three) {
            if t.len() == 3 || os_alias(&t) == Some(code) {
                return Some(Override::Osnap(code));
            }
        }
    }
    if let Some(c) = os_alias(&t) {
        return Some(Override::Osnap(c));
    }
    match t.as_str() {
        "FROM" | "FRO" => Some(Override::From),
        "TT" | "TRACK" => Some(Override::Tt),
        "NON" | "NONE" => Some(Override::None),
        "M2P" | "MTP" => Some(Override::M2p),
        _ => None,
    }
}

// ---------------------------------------------------------------- numbers and points

/// Result of interpreting typed text as a point or a bare number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Point { pt: Pt, z: Option<f64>, polar: Option<(f64, f64)> },
    Number(f64),
}

#[derive(Clone, Copy, Default)]
pub struct InputOpts {
    pub base: Option<Pt>,
    /// Dynamic-input rule: a second point is relative unless prefixed with `#`.
    pub relative_default: bool,
}

/// Interpret typed text as a point (`x,y[,z]`, `d<a`, `@...`, `#...`) or a bare number.
pub fn parse_input(text: &str, o: InputOpts) -> Option<Input> {
    let mut t = text.trim();
    if t.is_empty() {
        return None;
    }
    let (mut force_abs, mut force_rel) = (false, false);
    if let Some(r) = t.strip_prefix('#') {
        force_abs = true;
        t = r.trim();
    } else if let Some(r) = t.strip_prefix('@') {
        force_rel = true;
        t = r.trim();
    }
    let rel = !force_abs && (force_rel || (o.relative_default && o.base.is_some()));
    let origin = if rel { o.base.unwrap_or(pt(0.0, 0.0)) } else { pt(0.0, 0.0) };
    if force_rel && t.is_empty() {
        return Some(Input::Point { pt: origin, z: None, polar: None }); // "@" alone = last point
    }
    // x,y[,z]
    if let Some((x, n)) = scan_number(t) {
        let rest = skip_ws(&t[n..]);
        if let Some(r) = rest.strip_prefix(',') {
            let r = skip_ws(r);
            if let Some((y, n2)) = scan_number(r) {
                let after = skip_ws(&r[n2..]);
                if after.is_empty() {
                    return Some(Input::Point { pt: pt(origin.x + x, origin.y + y), z: None, polar: None });
                }
                if let Some(zr) = after.strip_prefix(',') {
                    if let Some(z) = full_number(skip_ws(zr)) {
                        return Some(Input::Point { pt: pt(origin.x + x, origin.y + y), z: Some(z), polar: None });
                    }
                }
            }
        } else if let Some(r) = rest.strip_prefix('<') {
            if let Some(a) = full_number(skip_ws(r)) {
                let p = pt(origin.x + x * rad(a).cos(), origin.y + x * rad(a).sin());
                return Some(Input::Point { pt: p, z: None, polar: Some((x, rad(a))) });
            }
        }
    }
    match parse_number(t) {
        Some(n) if !force_abs && !force_rel => Some(Input::Number(n)),
        _ => None,
    }
}

// ---------------------------------------------------------------- keyword options

fn strip_paren(label: &str) -> &str {
    match label.find('(') {
        Some(i) if label.trim_end().ends_with(')') => label[..i].trim_end(),
        _ => label,
    }
}

/// Abbreviation of an option label: its capital letters and digits (`Ttr` -> `T`, `eXit` -> `X`, `3P` -> `3P`).
pub fn option_key(label: &str) -> String {
    let main = strip_paren(label);
    let caps: String = main.chars().filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit()).collect();
    if !caps.is_empty() {
        caps
    } else {
        main.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default()
    }
}

pub fn option_name(label: &str) -> &str {
    strip_paren(label)
}

/// Match typed text to one of the option labels; returns the matching label.
pub fn match_option<'a, S: AsRef<str>>(input: &str, options: &'a [S]) -> Option<&'a S> {
    let t = input.trim().to_lowercase();
    if t.is_empty() {
        return None;
    }
    if let Some(o) = options.iter().find(|o| option_name(o.as_ref()).to_lowercase() == t) {
        return Some(o);
    }
    if let Some(o) = options.iter().find(|o| option_key(o.as_ref()).to_lowercase() == t) {
        return Some(o);
    }
    if full_number(&t).is_some() && !t.contains('e') {
        return None; // a number is never an abbreviation ("2" is not 2Point)
    }
    let mut prefixed = options.iter().filter(|o| option_name(o.as_ref()).to_lowercase().starts_with(&t));
    match (prefixed.next(), prefixed.next()) {
        (Some(o), None) => Some(o),
        _ => None,
    }
}

/// `" [Close/Undo]"` suffix for a prompt.
pub fn options_text<S: AsRef<str>>(options: &[S]) -> String {
    if options.is_empty() {
        String::new()
    } else {
        format!(" [{}]", options.iter().map(|o| o.as_ref()).collect::<Vec<_>>().join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_fractions() {
        assert_eq!(parse_number("12.5"), Some(12.5));
        assert_eq!(parse_number("-.5e1"), Some(-5.0));
        assert_eq!(parse_number("1/2"), Some(0.5));
        assert_eq!(parse_number("3-1/4"), Some(3.25));
        assert_eq!(parse_number("-2 1/8"), Some(-2.125));
        assert_eq!(parse_number("1/0"), None);
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("1e"), None);
    }

    #[test]
    fn points() {
        let base = Some(pt(10.0, 10.0));
        assert_eq!(parse_input("1,2", InputOpts::default()), Some(Input::Point { pt: pt(1.0, 2.0), z: None, polar: None }));
        assert_eq!(parse_input("@5,5", InputOpts { base, relative_default: false }), Some(Input::Point { pt: pt(15.0, 15.0), z: None, polar: None }));
        assert_eq!(parse_input("5,5", InputOpts { base, relative_default: true }), Some(Input::Point { pt: pt(15.0, 15.0), z: None, polar: None }));
        assert_eq!(parse_input("#5,5", InputOpts { base, relative_default: true }), Some(Input::Point { pt: pt(5.0, 5.0), z: None, polar: None }));
        assert_eq!(parse_input("@", InputOpts { base, relative_default: false }), Some(Input::Point { pt: pt(10.0, 10.0), z: None, polar: None }));
        match parse_input("@10<90", InputOpts { base, relative_default: false }) {
            Some(Input::Point { pt: p, polar: Some((d, _)), .. }) => {
                assert!((p.x - 10.0).abs() < 1e-9 && (p.y - 20.0).abs() < 1e-9 && d == 10.0);
            }
            o => panic!("{o:?}"),
        }
        assert_eq!(parse_input("1, 2, 3", InputOpts::default()), Some(Input::Point { pt: pt(1.0, 2.0), z: Some(3.0), polar: None }));
        assert_eq!(parse_input("42", InputOpts::default()), Some(Input::Number(42.0)));
        assert_eq!(parse_input("#42", InputOpts::default()), None);
        assert_eq!(parse_input("hello", InputOpts::default()), None);
    }

    #[test]
    fn overrides() {
        assert_eq!(parse_override("end"), Some(Override::Osnap("END")));
        assert_eq!(parse_override("Midpoint"), Some(Override::Osnap("MID")));
        assert_eq!(parse_override("nea"), Some(Override::Osnap("NEA")));
        assert_eq!(parse_override("from"), Some(Override::From));
        assert_eq!(parse_override("none"), Some(Override::None));
        assert_eq!(parse_override("endx"), None);
    }

    #[test]
    fn options() {
        let opts = ["Close", "Undo", "Width", "2Point", "Ttr (tan tan radius)", "eXit"];
        assert_eq!(match_option("c", &opts), Some(&"Close"));
        assert_eq!(match_option("UNDO", &opts), Some(&"Undo"));
        assert_eq!(match_option("x", &opts), Some(&"eXit"));
        assert_eq!(match_option("wi", &opts), Some(&"Width"));
        assert_eq!(match_option("2", &opts), None);
        assert_eq!(match_option("2p", &opts), Some(&"2Point"));
        assert_eq!(match_option("t", &opts), Some(&"Ttr (tan tan radius)"));
        assert_eq!(option_key("Ttr (tan tan radius)"), "T");
        assert_eq!(options_text(&["A", "B"]), " [A/B]");
    }
}
