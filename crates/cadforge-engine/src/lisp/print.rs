//! Printing values and the number formats of RTOS / ANGTOS / DISTOF.
use super::value::*;

/// A real as AutoLISP prints it: six significant digits, always with a decimal point or an exponent.
pub fn fmt_real(x: f64) -> String {
    if !x.is_finite() {
        return if x.is_nan() { "nan".into() } else if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return "0.0".into();
    }
    let a = x.abs();
    if !(1e-4..1e10).contains(&a) {
        let s = format!("{:.5e}", x); // d.ddddde[-]x
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = m.trim_end_matches('0');
        let m = if m.ends_with('.') { format!("{m}0") } else { m.to_string() };
        let e: i32 = e.parse().unwrap_or(0);
        return format!("{m}e{}{:03}", if e < 0 { '-' } else { '+' }, e.abs());
    }
    // six significant digits
    let digits = 6 - 1 - a.log10().floor() as i32;
    let s = format!("{:.*}", digits.clamp(0, 15) as usize, x);
    let s = if s.contains('.') { s.trim_end_matches('0').to_string() } else { s };
    if s.ends_with('.') {
        format!("{s}0")
    } else if s.contains('.') {
        s
    } else {
        format!("{s}.0")
    }
}

fn escape(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn show(v: &Value, quote: bool, out: &mut String) {
    match v {
        Value::Nil => out.push_str("nil"),
        Value::T => out.push('T'),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Real(r) => out.push_str(&fmt_real(*r)),
        Value::Str(s) => {
            if quote {
                out.push_str(&escape(s))
            } else {
                out.push_str(s)
            }
        }
        Value::Sym(s) => out.push_str(s),
        Value::Ename(e) => out.push_str(&format!("<Entity name: {e:x}>")),
        Value::Sel(s) => out.push_str(&format!("<Selection set: {}>", s.borrow().len())),
        Value::Fun(f) => out.push_str(&format!("#<SUBR @{:p} {}>", std::rc::Rc::as_ptr(f), f.name.as_deref().unwrap_or("LAMBDA"))),
        Value::Cons(_) => {
            out.push('(');
            let mut cur = v.clone();
            let mut first = true;
            loop {
                match cur {
                    Value::Cons(c) => {
                        if !first {
                            out.push(' ');
                        }
                        first = false;
                        show(&c.0, quote, out);
                        cur = c.1.clone();
                    }
                    Value::Nil => break,
                    tail => {
                        out.push_str(" . ");
                        show(&tail, quote, out);
                        break;
                    }
                }
            }
            out.push(')');
        }
    }
}

/// As `princ` prints it (strings without quotes).
pub fn princ_str(v: &Value) -> String {
    let mut s = String::new();
    show(v, false, &mut s);
    s
}

/// As `prin1` / `print` prints it (strings quoted).
pub fn prin1_str(v: &Value) -> String {
    let mut s = String::new();
    show(v, true, &mut s);
    s
}

fn group_decimals(x: f64, prec: usize) -> String {
    format!("{:.*}", prec, x)
}

fn fraction(x: f64, denom: u32) -> (i64, u32, u32) {
    // whole part, numerator, denominator reduced
    let sign = if x < 0.0 { -1 } else { 1 };
    let a = x.abs();
    let mut whole = a.floor() as i64;
    let mut num = ((a - whole as f64) * denom as f64).round() as u32;
    if num == denom {
        whole += 1;
        num = 0;
    }
    let (mut n, mut d) = (num, denom);
    while n > 0 && n % 2 == 0 && d % 2 == 0 {
        n /= 2;
        d /= 2;
    }
    (sign * whole, if num == 0 { 0 } else { n }, d)
}

/// RTOS: `mode` 1 scientific, 2 decimal, 3 engineering, 4 architectural, 5 fractional.
pub fn rtos(x: f64, mode: i64, prec: usize) -> String {
    match mode {
        1 => {
            let s = format!("{:.*e}", prec, x);
            let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
            let e: i32 = e.parse().unwrap_or(0);
            format!("{m}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
        }
        3 => {
            let feet = (x / 12.0).trunc();
            let inches = x - feet * 12.0;
            format!("{}'-{}\"", feet as i64, group_decimals(inches, prec))
        }
        4 => {
            let denom = 1u32 << prec.min(8);
            let neg = x < 0.0;
            let a = x.abs();
            let mut feet = (a / 12.0).trunc() as i64;
            let rest = a - feet as f64 * 12.0;
            let (mut whole, n, d) = fraction(rest, denom);
            if whole >= 12 {
                feet += 1;
                whole -= 12;
            }
            let inches = if n == 0 {
                format!("{whole}")
            } else if whole > 0 {
                format!("{whole}-{n}/{d}")
            } else {
                format!("{n}/{d}")
            };
            format!("{}{feet}'-{inches}\"", if neg { "-" } else { "" })
        }
        5 => {
            let (w, n, d) = fraction(x, 1u32 << prec.min(8));
            if n == 0 {
                format!("{w}")
            } else if w == 0 {
                format!("{}{n}/{d}", if x < 0.0 { "-" } else { "" })
            } else {
                format!("{w}-{n}/{d}")
            }
        }
        _ => group_decimals(x, prec),
    }
}

/// ANGTOS (angle in radians): 0 decimal degrees, 1 degrees/minutes/seconds, 2 grads, 3 radians, 4 surveyor's units.
pub fn angtos(a: f64, mode: i64, prec: usize) -> String {
    let deg = a.to_degrees().rem_euclid(360.0);
    match mode {
        1 => {
            let mut d = deg.trunc();
            let mut m = ((deg - d) * 60.0).trunc();
            let sec_val = ((deg - d) * 60.0 - m) * 60.0;
            let mut sec = (sec_val * 10f64.powi(prec.saturating_sub(4) as i32)).round() / 10f64.powi(prec.saturating_sub(4) as i32);
            if sec >= 60.0 {
                sec -= 60.0;
                m += 1.0;
            }
            if m >= 60.0 {
                m -= 60.0;
                d += 1.0;
            }
            let mut s = format!("{}d", d as i64);
            if prec >= 1 {
                s.push_str(&format!("{}'", m as i64));
            }
            if prec >= 3 {
                s.push_str(&format!("{}\"", if prec > 4 { format!("{:.*}", prec - 4, sec) } else { format!("{}", sec as i64) }));
            }
            s
        }
        2 => format!("{}g", group_decimals(deg * 400.0 / 360.0, prec)),
        3 => format!("{}r", group_decimals(a.rem_euclid(std::f64::consts::TAU), prec)),
        4 => {
            let (ns, ew, ang) = if deg <= 90.0 {
                ("N", "E", 90.0 - deg)
            } else if deg < 180.0 {
                ("S", "E", deg - 90.0)
            } else if deg <= 270.0 {
                ("S", "W", 270.0 - deg)
            } else {
                ("N", "W", deg - 270.0)
            };
            if deg == 0.0 {
                return "E".into();
            }
            if deg == 90.0 {
                return "N".into();
            }
            if deg == 180.0 {
                return "W".into();
            }
            if deg == 270.0 {
                return "S".into();
            }
            format!("{ns}{}{ew}", angtos(ang.to_radians(), 1, prec))
        }
        _ => format!("{}", group_decimals(deg, prec)),
    }
}

/// DISTOF: parse a distance written in one of the RTOS modes.
pub fn distof(s: &str, mode: i64) -> Option<f64> {
    let t = s.trim();
    match mode {
        1 | 2 => {
            let t = t.replace(['E', 'e'], "e");
            t.parse().ok()
        }
        3 | 4 => {
            let neg = t.starts_with('-');
            let t = t.trim_start_matches('-');
            let (feet, inch) = t.split_once('\'').map_or((0.0, t), |(f, r)| (f.parse().unwrap_or(0.0), r.trim_start_matches('-')));
            let inch = inch.trim_end_matches('"').trim();
            let v = parse_mixed(inch)?;
            Some((if neg { -1.0 } else { 1.0 }) * (feet * 12.0 + v))
        }
        5 => parse_mixed(t),
        _ => None,
    }
}

fn parse_mixed(t: &str) -> Option<f64> {
    let t = t.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    let mut total = 0.0;
    for part in t.split([' ', '-']).filter(|p| !p.is_empty()) {
        total += match part.split_once('/') {
            Some((n, d)) => n.parse::<f64>().ok()? / d.parse::<f64>().ok()?,
            None => part.parse().ok()?,
        };
    }
    Some(total)
}
