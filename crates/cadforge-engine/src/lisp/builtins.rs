//! The functions that need no drawing: arithmetic, lists, strings, geometry helpers.
use super::print::*;
use super::value::*;
use super::{bad_arg, Interp};

fn arg(a: &[Value], i: usize) -> LResult<&Value> {
    a.get(i)
        .ok_or_else(|| LErr::Msg("too few arguments".into()))
}

fn num(a: &[Value], i: usize) -> LResult<f64> {
    let v = arg(a, i)?;
    v.as_f64().ok_or_else(|| bad_arg("numberp", v))
}

fn int(a: &[Value], i: usize) -> LResult<i64> {
    let v = arg(a, i)?;
    v.as_i64().ok_or_else(|| bad_arg("numberp", v))
}

fn string<'a>(a: &'a [Value], i: usize) -> LResult<&'a str> {
    let v = arg(a, i)?;
    v.as_str().ok_or_else(|| bad_arg("stringp", v))
}

fn list(a: &[Value], i: usize) -> LResult<Vec<Value>> {
    let v = arg(a, i)?;
    v.to_vec().ok_or_else(|| bad_arg("listp", v))
}

fn point(a: &[Value], i: usize) -> LResult<(f64, f64, f64)> {
    let v = arg(a, i)?;
    let l = v.to_vec().ok_or_else(|| bad_arg("listp", v))?;
    let f = |k: usize| l.get(k).and_then(Value::as_f64);
    match (f(0), f(1)) {
        (Some(x), Some(y)) if l.len() <= 3 => Ok((x, y, f(2).unwrap_or(0.0))),
        _ => Err(bad_arg("point list", v)),
    }
}

fn pt3(x: f64, y: f64, z: f64) -> Value {
    Value::list(vec![Value::Real(x), Value::Real(y), Value::Real(z)])
}

fn all_int(a: &[Value]) -> bool {
    a.iter().all(|v| matches!(v, Value::Int(_)))
}

/// Case-sensitive AutoLISP wildcard match (`wcmatch`): `#` digit, `@` letter, `.` non-alphanumeric, `*`, `?`,
/// `[abc]`, `[~abc]`, a leading `~` negates, `,` separates alternatives and `` ` `` escapes.
pub fn wcmatch(s: &str, pattern: &str, ignore_case: bool) -> bool {
    let (s, pattern) = if ignore_case {
        (s.to_uppercase(), pattern.to_uppercase())
    } else {
        (s.to_string(), pattern.to_string())
    };
    let mut alts: Vec<String> = vec![String::new()];
    let mut esc = false;
    for ch in pattern.chars() {
        if esc {
            alts.last_mut().expect("an alternative").push('`');
            alts.last_mut().expect("an alternative").push(ch);
            esc = false;
        } else if ch == '`' {
            esc = true;
        } else if ch == ',' {
            alts.push(String::new());
        } else {
            alts.last_mut().expect("an alternative").push(ch);
        }
    }
    let sc: Vec<char> = s.chars().collect();
    alts.iter().any(|alt| {
        let (neg, body) = match alt.strip_prefix('~') {
            Some(b) => (true, b),
            None => (false, alt.as_str()),
        };
        let pc: Vec<char> = body.chars().collect();
        wc_here(&sc, &pc) != neg
    })
}

fn wc_here(s: &[char], p: &[char]) -> bool {
    let Some(&first) = p.first() else {
        return s.is_empty();
    };
    match first {
        '*' => (0..=s.len()).any(|k| wc_here(&s[k..], &p[1..])),
        '`' => !s.is_empty() && p.get(1) == s.first() && wc_here(&s[1..], &p[2..]),
        '[' => {
            let Some(end) = p.iter().position(|c| *c == ']') else {
                return !s.is_empty() && s[0] == '[' && wc_here(&s[1..], &p[1..]);
            };
            let set = &p[1..end];
            let (neg, set) = if set.first() == Some(&'~') {
                (true, &set[1..])
            } else {
                (false, set)
            };
            let Some(&c) = s.first() else { return false };
            let mut hit = false;
            let mut i = 0;
            while i < set.len() {
                if i + 2 < set.len() && set[i + 1] == '-' {
                    hit |= (set[i]..=set[i + 2]).contains(&c);
                    i += 3;
                } else {
                    hit |= set[i] == c;
                    i += 1;
                }
            }
            hit != neg && wc_here(&s[1..], &p[end + 1..])
        }
        '?' => !s.is_empty() && wc_here(&s[1..], &p[1..]),
        '#' => s.first().is_some_and(|c| c.is_ascii_digit()) && wc_here(&s[1..], &p[1..]),
        '@' => s.first().is_some_and(|c| c.is_alphabetic()) && wc_here(&s[1..], &p[1..]),
        '.' => s.first().is_some_and(|c| !c.is_alphanumeric()) && wc_here(&s[1..], &p[1..]),
        c => s.first() == Some(&c) && wc_here(&s[1..], &p[1..]),
    }
}

fn cxr(name: &str, v: &Value) -> LResult {
    // c[ad]+r: apply the letters from the right
    let letters: Vec<char> = name[1..name.len() - 1].chars().collect();
    let mut cur = v.clone();
    for l in letters.iter().rev() {
        cur = match l {
            'A' => cur.car(),
            _ => cur.cdr(),
        }
        .ok_or_else(|| bad_arg("listp", &cur))?;
    }
    Ok(cur)
}

fn is_cxr(name: &str) -> bool {
    name.len() >= 4
        && name.len() <= 6
        && name.starts_with('C')
        && name.ends_with('R')
        && name[1..name.len() - 1]
            .chars()
            .all(|c| c == 'A' || c == 'D')
}

fn parse_leading_number(s: &str) -> Option<f64> {
    let t = s.trim_start();
    let mut end = 0;
    let b = t.as_bytes();
    let mut seen_dot = false;
    let mut seen_e = false;
    while end < b.len() {
        let c = b[end] as char;
        let ok = c.is_ascii_digit()
            || ((c == '-' || c == '+') && (end == 0 || matches!(b[end - 1] as char, 'e' | 'E')))
            || (c == '.' && !seen_dot && !seen_e)
            || ((c == 'e' || c == 'E') && !seen_e && end > 0);
        if !ok {
            break;
        }
        seen_dot |= c == '.';
        seen_e |= c == 'e' || c == 'E';
        end += 1;
    }
    let mut cand = &t[..end];
    while !cand.is_empty() {
        if let Ok(v) = cand.parse::<f64>() {
            return Some(v);
        }
        cand = &cand[..cand.len() - 1];
    }
    None
}

pub fn call(it: &Interp, name: &str, a: &[Value]) -> Option<LResult> {
    if is_cxr(name) {
        return Some(arg(a, 0).and_then(|v| cxr(name, v)));
    }
    Some(match name {
        // ---------------------------------------------------------------- arithmetic
        "+" | "-" | "*" => (|| {
            if a.is_empty() {
                return Ok(if name == "*" {
                    Value::Int(1)
                } else {
                    Value::Int(0)
                });
            }
            for v in a {
                if v.as_f64().is_none() {
                    return Err(bad_arg("numberp", v));
                }
            }
            if all_int(a) {
                let xs: Vec<i64> = a.iter().filter_map(Value::as_i64).collect();
                let r = match name {
                    "+" => xs.iter().fold(0i64, |s, x| s.wrapping_add(*x)),
                    "*" => xs.iter().fold(1i64, |s, x| s.wrapping_mul(*x)),
                    _ if xs.len() == 1 => xs[0].wrapping_neg(),
                    _ => xs[1..].iter().fold(xs[0], |s, x| s.wrapping_sub(*x)),
                };
                Ok(Value::Int(r))
            } else {
                let xs: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
                let r = match name {
                    "+" => xs.iter().sum(),
                    "*" => xs.iter().product(),
                    _ if xs.len() == 1 => -xs[0],
                    _ => xs[1..].iter().fold(xs[0], |s, x| s - x),
                };
                Ok(Value::Real(r))
            }
        })(),
        "/" => (|| {
            let first = arg(a, 0)?;
            if a.len() == 1 {
                return if let Value::Int(i) = first {
                    Ok(Value::Int(*i))
                } else {
                    Ok(first.clone())
                };
            }
            if all_int(a) {
                let mut r = int(a, 0)?;
                for i in 1..a.len() {
                    let d = int(a, i)?;
                    if d == 0 {
                        return Err(LErr::Msg("divide by zero".into()));
                    }
                    r = r.wrapping_div(d);
                }
                Ok(Value::Int(r))
            } else {
                let mut r = num(a, 0)?;
                for i in 1..a.len() {
                    let d = num(a, i)?;
                    if d == 0.0 {
                        return Err(LErr::Msg("divide by zero".into()));
                    }
                    r /= d;
                }
                Ok(Value::Real(r))
            }
        })(),
        "1+" => arg(a, 0).and_then(|v| match v {
            Value::Int(i) => Ok(Value::Int(i + 1)),
            Value::Real(r) => Ok(Value::Real(r + 1.0)),
            o => Err(bad_arg("numberp", o)),
        }),
        "1-" => arg(a, 0).and_then(|v| match v {
            Value::Int(i) => Ok(Value::Int(i - 1)),
            Value::Real(r) => Ok(Value::Real(r - 1.0)),
            o => Err(bad_arg("numberp", o)),
        }),
        "ABS" => arg(a, 0).and_then(|v| match v {
            Value::Int(i) => Ok(Value::Int(i.abs())),
            Value::Real(r) => Ok(Value::Real(r.abs())),
            o => Err(bad_arg("numberp", o)),
        }),
        "SQRT" => num(a, 0).and_then(|x| {
            if x < 0.0 {
                Err(LErr::Msg("function undefined for argument".into()))
            } else {
                Ok(Value::Real(x.sqrt()))
            }
        }),
        "SIN" => num(a, 0).map(|x| Value::Real(x.sin())),
        "COS" => num(a, 0).map(|x| Value::Real(x.cos())),
        "ATAN" => (|| {
            let y = num(a, 0)?;
            Ok(Value::Real(if a.len() > 1 {
                y.atan2(num(a, 1)?)
            } else {
                y.atan()
            }))
        })(),
        "EXP" => num(a, 0).map(|x| Value::Real(x.exp())),
        "LOG" => num(a, 0).and_then(|x| {
            if x <= 0.0 {
                Err(LErr::Msg("function undefined for argument".into()))
            } else {
                Ok(Value::Real(x.ln()))
            }
        }),
        "EXPT" => (|| {
            let (b, e) = (arg(a, 0)?, arg(a, 1)?);
            match (b, e) {
                (Value::Int(x), Value::Int(y)) if *y >= 0 => {
                    Ok(Value::Int(x.wrapping_pow((*y).min(u32::MAX as i64) as u32)))
                }
                _ => Ok(Value::Real(num(a, 0)?.powf(num(a, 1)?))),
            }
        })(),
        "FIX" => num(a, 0).map(|x| Value::Int(x.trunc() as i64)),
        "FLOAT" => num(a, 0).map(Value::Real),
        "MIN" | "MAX" => (|| {
            if a.is_empty() {
                return Err(LErr::Msg("too few arguments".into()));
            }
            let mut best = num(a, 0)?;
            for i in 1..a.len() {
                let x = num(a, i)?;
                best = if name == "MIN" {
                    best.min(x)
                } else {
                    best.max(x)
                };
            }
            Ok(if all_int(a) {
                Value::Int(best as i64)
            } else {
                Value::Real(best)
            })
        })(),
        "REM" => (|| {
            if all_int(a) && a.len() == 2 {
                let d = int(a, 1)?;
                if d == 0 {
                    return Err(LErr::Msg("divide by zero".into()));
                }
                return Ok(Value::Int(int(a, 0)? % d));
            }
            let d = num(a, 1)?;
            if d == 0.0 {
                return Err(LErr::Msg("divide by zero".into()));
            }
            Ok(Value::Real(num(a, 0)? % d))
        })(),
        "GCD" => (|| {
            let (mut x, mut y) = (int(a, 0)?.abs(), int(a, 1)?.abs());
            while y != 0 {
                (x, y) = (y, x % y);
            }
            Ok(Value::Int(x))
        })(),
        "~" => int(a, 0).map(|x| Value::Int(!x)),
        "LOGAND" => a
            .iter()
            .try_fold(-1i64, |s, v| {
                v.as_i64()
                    .map(|x| s & x)
                    .ok_or_else(|| bad_arg("integerp", v))
            })
            .map(Value::Int),
        "LOGIOR" => a
            .iter()
            .try_fold(0i64, |s, v| {
                v.as_i64()
                    .map(|x| s | x)
                    .ok_or_else(|| bad_arg("integerp", v))
            })
            .map(Value::Int),
        "LSH" => (|| {
            let (x, n) = (int(a, 0)?, int(a, 1)?);
            Ok(Value::Int(if n >= 0 {
                x << n.min(63)
            } else {
                x >> (-n).min(63)
            }))
        })(),
        // ---------------------------------------------------------------- comparison and logic
        "=" | "/=" | "<" | ">" | "<=" | ">=" => (|| {
            if a.len() < 2 && name != "/=" {
                return Err(LErr::Msg("too few arguments".into()));
            }
            let cmp = |x: &Value, y: &Value| -> LResult<std::cmp::Ordering> {
                match (x, y) {
                    (Value::Str(p), Value::Str(q)) => Ok(p.cmp(q)),
                    _ => {
                        let (p, q) = (
                            x.as_f64().ok_or_else(|| bad_arg("numberp", x))?,
                            y.as_f64().ok_or_else(|| bad_arg("numberp", y))?,
                        );
                        Ok(p.partial_cmp(&q).unwrap_or(std::cmp::Ordering::Less))
                    }
                }
            };
            if name == "=" || name == "/=" {
                // `=` on lists / atoms of other types is equality; numbers and strings by value
                let eq = |x: &Value, y: &Value| match (x, y) {
                    (Value::Int(_) | Value::Real(_), Value::Int(_) | Value::Real(_))
                    | (Value::Str(_), Value::Str(_)) => cmp(x, y).map(|o| o.is_eq()),
                    _ => Ok(x.equal(y)),
                };
                if name == "=" {
                    for w in a.windows(2) {
                        if !eq(&w[0], &w[1])? {
                            return Ok(Value::Nil);
                        }
                    }
                    return Ok(Value::T);
                }
                if a.len() == 1 {
                    return Ok(Value::T);
                }
                for w in a.windows(2) {
                    if eq(&w[0], &w[1])? {
                        return Ok(Value::Nil);
                    }
                }
                return Ok(Value::T);
            }
            for w in a.windows(2) {
                let o = cmp(&w[0], &w[1])?;
                let ok = match name {
                    "<" => o.is_lt(),
                    ">" => o.is_gt(),
                    "<=" => o.is_le(),
                    _ => o.is_ge(),
                };
                if !ok {
                    return Ok(Value::Nil);
                }
            }
            Ok(Value::T)
        })(),
        "EQ" => arg(a, 0).and_then(|x| Ok(Value::bool(x.eq(arg(a, 1)?)))),
        "EQUAL" => (|| {
            let (x, y) = (arg(a, 0)?, arg(a, 1)?);
            if let Some(f) = a.get(2).and_then(Value::as_f64) {
                if let (Some(p), Some(q)) = (x.as_f64(), y.as_f64()) {
                    return Ok(Value::bool((p - q).abs() <= f));
                }
                if let (Some(p), Some(q)) = (x.to_vec(), y.to_vec()) {
                    return Ok(Value::bool(
                        p.len() == q.len()
                            && p.iter()
                                .zip(&q)
                                .all(|(m, n)| match (m.as_f64(), n.as_f64()) {
                                    (Some(m), Some(n)) => (m - n).abs() <= f,
                                    _ => m.equal(n),
                                }),
                    ));
                }
            }
            Ok(Value::bool(x.equal(y)))
        })(),
        "NOT" | "NULL" => arg(a, 0).map(|v| Value::bool(v.is_nil())),
        "ATOM" => arg(a, 0).map(|v| Value::bool(v.is_atom())),
        "LISTP" => arg(a, 0).map(|v| Value::bool(matches!(v, Value::Cons(_) | Value::Nil))),
        "NUMBERP" => arg(a, 0).map(|v| Value::bool(v.as_f64().is_some())),
        "ZEROP" => num(a, 0).map(|x| Value::bool(x == 0.0)),
        "MINUSP" => num(a, 0).map(|x| Value::bool(x < 0.0)),
        "TYPE" => arg(a, 0).map(|v| {
            if v.is_nil() {
                Value::Nil
            } else {
                Value::sym(v.type_name())
            }
        }),
        "BOUNDP" => arg(a, 0).and_then(|v| match v {
            Value::Sym(s) => Ok(Value::bool(it.is_bound(s))),
            o => Err(bad_arg("symbolp", o)),
        }),
        // ---------------------------------------------------------------- lists
        "LIST" => Ok(Value::list(a.to_vec())),
        "CONS" => arg(a, 0).and_then(|x| Ok(Value::cons(x.clone(), arg(a, 1)?.clone()))),
        "CAR" => arg(a, 0).and_then(|v| v.car().ok_or_else(|| bad_arg("listp", v))),
        "CDR" => arg(a, 0).and_then(|v| v.cdr().ok_or_else(|| bad_arg("listp", v))),
        "LENGTH" => list(a, 0).map(|l| Value::Int(l.len() as i64)),
        "NTH" => (|| {
            let n = int(a, 0)?;
            let l = list(a, 1)?;
            Ok(if n < 0 {
                Value::Nil
            } else {
                l.get(n as usize).cloned().unwrap_or(Value::Nil)
            })
        })(),
        "LAST" => list(a, 0).map(|l| l.last().cloned().unwrap_or(Value::Nil)),
        "REVERSE" => list(a, 0).map(|mut l| {
            l.reverse();
            Value::list(l)
        }),
        "APPEND" => (|| {
            let mut out = vec![];
            for (i, v) in a.iter().enumerate() {
                if v.is_nil() {
                    continue;
                }
                if i + 1 == a.len() && !matches!(v, Value::Cons(_)) {
                    return Ok(out
                        .into_iter()
                        .rev()
                        .fold(v.clone(), |t, x| Value::cons(x, t)));
                }
                out.extend(v.to_vec().ok_or_else(|| bad_arg("listp", v))?);
            }
            Ok(Value::list(out))
        })(),
        "MEMBER" => (|| {
            let x = arg(a, 0)?;
            let l = list(a, 1)?;
            Ok(match l.iter().position(|e| e.equal(x)) {
                Some(i) => Value::list(l[i..].to_vec()),
                None => Value::Nil,
            })
        })(),
        "ASSOC" => (|| {
            let key = arg(a, 0)?;
            for e in list(a, 1)? {
                if let Value::Cons(c) = &e {
                    if c.0.equal(key) {
                        return Ok(e.clone());
                    }
                }
            }
            Ok(Value::Nil)
        })(),
        "SUBST" => (|| {
            let (new, old) = (arg(a, 0)?, arg(a, 1)?);
            Ok(Value::list(
                list(a, 2)?
                    .into_iter()
                    .map(|e| if e.equal(old) { new.clone() } else { e })
                    .collect(),
            ))
        })(),
        "VL-REMOVE" => (|| {
            let x = arg(a, 0)?;
            Ok(Value::list(
                list(a, 1)?.into_iter().filter(|e| !e.equal(x)).collect(),
            ))
        })(),
        "VL-POSITION" => (|| {
            let x = arg(a, 0)?;
            Ok(list(a, 1)?
                .iter()
                .position(|e| e.equal(x))
                .map_or(Value::Nil, |i| Value::Int(i as i64)))
        })(),
        "VL-LIST-LENGTH" => list(a, 0).map(|l| Value::Int(l.len() as i64)),
        "ACAD_STRLSORT" => list(a, 0).and_then(|l| {
            let mut s: Vec<String> = l
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(String::from)
                        .ok_or_else(|| bad_arg("stringp", v))
                })
                .collect::<LResult<_>>()?;
            s.sort_by_key(|x| x.to_lowercase());
            Ok(Value::list(s.iter().map(|x| Value::str(x)).collect()))
        }),
        // ---------------------------------------------------------------- strings
        "STRCAT" => (|| {
            let mut s = String::new();
            for (i, v) in a.iter().enumerate() {
                s.push_str(v.as_str().ok_or_else(|| bad_arg("stringp", &a[i]))?);
            }
            Ok(Value::str(&s))
        })(),
        "STRLEN" => (|| {
            let mut n = 0;
            for i in 0..a.len() {
                n += string(a, i)?.chars().count();
            }
            Ok(Value::Int(n as i64))
        })(),
        "SUBSTR" => (|| {
            let s: Vec<char> = string(a, 0)?.chars().collect();
            let start = int(a, 1)?;
            if start < 1 {
                return Err(LErr::Msg("invalid argument".into()));
            }
            let from = (start as usize - 1).min(s.len());
            let to = match a.get(2) {
                Some(_) => (from + int(a, 2)?.max(0) as usize).min(s.len()),
                None => s.len(),
            };
            Ok(Value::str(&s[from..to].iter().collect::<String>()))
        })(),
        "STRCASE" => (|| {
            let s = string(a, 0)?;
            Ok(Value::str(&if a.get(1).is_some_and(Value::is_true) {
                s.to_lowercase()
            } else {
                s.to_uppercase()
            }))
        })(),
        "CHR" => int(a, 0).map(|n| {
            Value::str(
                &char::from_u32(n.clamp(0, 0x10ffff) as u32)
                    .unwrap_or('?')
                    .to_string(),
            )
        }),
        "ASCII" => string(a, 0).map(|s| Value::Int(s.chars().next().map_or(0, |c| c as i64))),
        "ITOA" => int(a, 0).map(|n| Value::str(&n.to_string())),
        "ATOI" => string(a, 0)
            .map(|s| Value::Int(parse_leading_number(s).map_or(0, |x| x.trunc() as i64))),
        "ATOF" => string(a, 0).map(|s| Value::Real(parse_leading_number(s).unwrap_or(0.0))),
        "RTOS" => (|| {
            let x = num(a, 0)?;
            let mode = if a.len() > 1 { int(a, 1)? } else { 2 };
            let prec = if a.len() > 2 {
                int(a, 2)?.clamp(0, 16) as usize
            } else {
                4
            };
            Ok(Value::str(&rtos(x, mode, prec)))
        })(),
        "ANGTOS" => (|| {
            let x = num(a, 0)?;
            let mode = if a.len() > 1 { int(a, 1)? } else { 0 };
            let prec = if a.len() > 2 {
                int(a, 2)?.clamp(0, 8) as usize
            } else {
                4
            };
            Ok(Value::str(&angtos(x, mode, prec)))
        })(),
        "DISTOF" => (|| {
            let mode = if a.len() > 1 { int(a, 1)? } else { 2 };
            Ok(distof(string(a, 0)?, mode).map_or(Value::Nil, Value::Real))
        })(),
        "READ" => (|| match super::reader::read_first(string(a, 0)?) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Ok(Value::Nil),
            Err(e) => Err(LErr::Msg(e)),
        })(),
        "WCMATCH" => (|| Ok(Value::bool(wcmatch(string(a, 0)?, string(a, 1)?, false))))(),
        "VL-PRINC-TO-STRING" => arg(a, 0).map(|v| Value::str(&princ_str(v))),
        "VL-PRIN1-TO-STRING" => arg(a, 0).map(|v| Value::str(&prin1_str(v))),
        "VL-STRING-SEARCH" => (|| {
            let (pat, s) = (string(a, 0)?, string(a, 1)?);
            let start = if a.len() > 2 {
                int(a, 2)?.max(0) as usize
            } else {
                0
            };
            let sc: Vec<char> = s.chars().collect();
            let pc: Vec<char> = pat.chars().collect();
            if pc.is_empty() || pc.len() > sc.len() {
                return Ok(Value::Nil);
            }
            Ok((start..=sc.len() - pc.len())
                .find(|i| sc[*i..*i + pc.len()] == pc[..])
                .map_or(Value::Nil, |i| Value::Int(i as i64)))
        })(),
        "VL-STRING-POSITION" => (|| {
            let (code, s) = (int(a, 0)?, string(a, 1)?);
            let start = if a.len() > 2 {
                int(a, 2)?.max(0) as usize
            } else {
                0
            };
            Ok(s.chars()
                .enumerate()
                .skip(start)
                .find(|(_, c)| *c as i64 == code)
                .map_or(Value::Nil, |(i, _)| Value::Int(i as i64)))
        })(),
        "VL-STRING-SUBST" => (|| {
            let (new, pat, s) = (string(a, 0)?, string(a, 1)?, string(a, 2)?);
            let start = if a.len() > 3 {
                int(a, 3)?.max(0) as usize
            } else {
                0
            };
            let byte = s.char_indices().nth(start).map_or(s.len(), |(b, _)| b);
            Ok(Value::str(&match s[byte..].find(pat) {
                Some(p) if !pat.is_empty() => format!(
                    "{}{}{}{}",
                    &s[..byte],
                    &s[byte..byte + p],
                    new,
                    &s[byte + p + pat.len()..]
                ),
                _ => s.to_string(),
            }))
        })(),
        "VL-STRING-TRIM" | "VL-STRING-LEFT-TRIM" | "VL-STRING-RIGHT-TRIM" => (|| {
            let (chars, s) = (string(a, 0)?, string(a, 1)?);
            let set: Vec<char> = chars.chars().collect();
            let t = match name {
                "VL-STRING-LEFT-TRIM" => s.trim_start_matches(|c| set.contains(&c)),
                "VL-STRING-RIGHT-TRIM" => s.trim_end_matches(|c| set.contains(&c)),
                _ => s.trim_matches(|c| set.contains(&c)),
            };
            Ok(Value::str(t))
        })(),
        "VL-STRING->LIST" => {
            string(a, 0).map(|s| Value::list(s.chars().map(|c| Value::Int(c as i64)).collect()))
        }
        "VL-LIST->STRING" => list(a, 0).and_then(|l| {
            let mut s = String::new();
            for v in &l {
                s.push(
                    char::from_u32(v.as_i64().ok_or_else(|| bad_arg("integerp", v))? as u32)
                        .unwrap_or('?'),
                );
            }
            Ok(Value::str(&s))
        }),
        // ---------------------------------------------------------------- geometry
        "DISTANCE" => (|| {
            let ((x1, y1, z1), (x2, y2, z2)) = (point(a, 0)?, point(a, 1)?);
            Ok(Value::Real(
                ((x2 - x1).powi(2) + (y2 - y1).powi(2) + (z2 - z1).powi(2)).sqrt(),
            ))
        })(),
        "ANGLE" => (|| {
            let ((x1, y1, _), (x2, y2, _)) = (point(a, 0)?, point(a, 1)?);
            Ok(Value::Real(
                (y2 - y1).atan2(x2 - x1).rem_euclid(std::f64::consts::TAU),
            ))
        })(),
        "POLAR" => (|| {
            let ((x, y, z), ang, d) = (point(a, 0)?, num(a, 1)?, num(a, 2)?);
            Ok(pt3(x + d * ang.cos(), y + d * ang.sin(), z))
        })(),
        "INTERS" => (|| {
            let ((x1, y1, _), (x2, y2, _), (x3, y3, _), (x4, y4, _)) =
                (point(a, 0)?, point(a, 1)?, point(a, 2)?, point(a, 3)?);
            let bounded = a.get(4).is_none_or(Value::is_true);
            let den = (x2 - x1) * (y4 - y3) - (y2 - y1) * (x4 - x3);
            if den.abs() < 1e-12 {
                return Ok(Value::Nil);
            }
            let t = ((x3 - x1) * (y4 - y3) - (y3 - y1) * (x4 - x3)) / den;
            let u = ((x3 - x1) * (y2 - y1) - (y3 - y1) * (x2 - x1)) / den;
            if bounded && !((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)) {
                return Ok(Value::Nil);
            }
            Ok(pt3(x1 + t * (x2 - x1), y1 + t * (y2 - y1), 0.0))
        })(),
        // ---------------------------------------------------------------- environment
        "VL-LOAD-COM" => Ok(Value::T),
        "VER" => Ok(Value::str("CadForge LISP 1.0")),
        "GETENV" => Ok(Value::Nil),
        "SETENV" => arg(a, 1).cloned(),
        "FINDFILE" => Ok(Value::Nil),
        "VL-BB-SET" => (|| {
            let Value::Sym(s) = arg(a, 0)? else {
                return Err(bad_arg("symbolp", &a[0]));
            };
            it.set_global(&format!("%BB:{s}"), arg(a, 1)?.clone());
            Ok(a[1].clone())
        })(),
        "VL-BB-REF" => (|| {
            let Value::Sym(s) = arg(a, 0)? else {
                return Err(bad_arg("symbolp", &a[0]));
            };
            Ok(it.lookup(&format!("%BB:{s}")))
        })(),
        "EXIT" | "QUIT" => Err(LErr::Exit),
        _ => return None,
    })
}
