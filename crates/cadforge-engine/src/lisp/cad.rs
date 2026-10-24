//! Lisp functions that talk to the drawing session: output, system variables, `command`, the `get...` functions.
use super::print::*;
use super::value::*;
use super::{bad_arg, Interp};
use crate::ctx::*;
use crate::runtime::{CmdError, PromptSpec};
use cadforge_core::geom::{pt, Pt};

fn arg(a: &[Value], i: usize) -> LResult<&Value> {
    a.get(i)
        .ok_or_else(|| LErr::Msg("too few arguments".into()))
}

fn string<'a>(a: &'a [Value], i: usize) -> LResult<&'a str> {
    let v = arg(a, i)?;
    v.as_str().ok_or_else(|| bad_arg("stringp", v))
}

impl From<CmdError> for LErr {
    fn from(e: CmdError) -> LErr {
        match e {
            CmdError::Cancel => LErr::Cancel,
            CmdError::Msg(m) => LErr::Msg(m),
        }
    }
}

/// Write text to the command history; `\n` ends a line. Whatever follows the last newline stays buffered until the next
/// output or [`flush`].
pub fn out(it: &Interp, c: &Ctx, text: &str) {
    if text.is_empty() {
        return;
    }
    *it.last_printed.borrow_mut() = Some(text.to_string());
    let mut buf = it.out_buf.borrow_mut();
    buf.push_str(text);
    while let Some(i) = buf.find('\n') {
        let line: String = buf.drain(..=i).collect();
        c.log(line.trim_end_matches('\n'));
    }
}

pub fn flush(it: &Interp, c: &Ctx) {
    let rest = std::mem::take(&mut *it.out_buf.borrow_mut());
    if !rest.is_empty() {
        c.log(&rest);
    }
}

/// A point value in the user coordinate system → the world point the engine uses.
fn to_wcs(c: &Ctx, p: (f64, f64)) -> Pt {
    c.app(|a| a.ucs().to_wcs(pt(p.0, p.1)))
}

fn from_wcs(c: &Ctx, p: Pt) -> Value {
    let u = c.app(|a| a.ucs().from_wcs(p));
    Value::point(u.x, u.y)
}

/// One `command` argument as the text the command line would receive; `None` stands for PAUSE.
fn command_token(v: &Value) -> LResult<Option<String>> {
    Ok(Some(match v {
        Value::Str(s) if &**s == "\\" => return Ok(None),
        Value::Str(s) => s.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Real(r) => format!("{r}"),
        Value::Nil => String::new(),
        Value::T => "T".into(),
        Value::Sym(s) => s.to_string(),
        Value::Ename(e) => format!("<ID:{e}>"),
        Value::Sel(s) => format!(
            "<ID:{}>",
            s.borrow()
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Cons(_) => {
            let (x, y) = v.as_point().ok_or_else(|| bad_arg("point list", v))?;
            let z = v
                .to_vec()
                .and_then(|l| l.get(2).and_then(Value::as_f64))
                .unwrap_or(0.0);
            if z != 0.0 {
                format!("{x},{y},{z}")
            } else {
                format!("{x},{y}")
            }
        }
        Value::Fun(_) => return Err(bad_arg("command argument", v)),
    }))
}

/// `(command "name" input...)`: each string / number / point is typed at the prompts of the command; `PAUSE` leaves one
/// answer to the user. Several commands may follow each other in one call.
async fn command(it: &Interp, c: &Ctx, a: &[Value]) -> LResult {
    let mut tokens: Vec<Option<String>> = vec![];
    for v in a {
        tokens.push(command_token(v)?);
    }
    flush(it, c);
    let mut i = 0;
    while i < tokens.len() {
        let Some(name) = tokens[i].clone() else {
            i += 1;
            continue;
        };
        i += 1;
        let name = name
            .trim()
            .trim_start_matches('_')
            .trim_start_matches('.')
            .to_string();
        if name.is_empty() {
            continue;
        }
        let rest: Vec<Option<String>> = tokens[i..].to_vec();
        c.app(|app| app.lisp_input = rest.into_iter().collect());
        c.sub_forced(&name, crate::runtime::Args::none()).await;
        // whatever the command did not consume belongs to the next command name
        let left = c.app(|app| app.lisp_input.len());
        i = tokens.len() - left;
    }
    c.app(|app| app.lisp_input.clear());
    Ok(Value::Nil)
}

/// Prompt text of a `get...` call: the string argument without leading newlines.
fn prompt_of(a: &[Value], from: usize) -> String {
    a.iter()
        .skip(from)
        .find_map(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches(['\n', '\r'])
        .trim_end()
        .to_string()
}

fn spec(it: &Interp, a: &[Value], from: usize) -> PromptSpec {
    let ig = it.init_get.borrow().clone();
    let msg = prompt_of(a, from);
    let mut s = PromptSpec::new(if msg.is_empty() {
        "Enter a value"
    } else {
        &msg
    });
    if !ig.keywords.is_empty() {
        s = s.options_vec(ig.keywords.clone());
    }
    if ig.bits & 1 == 0 {
        s = s.allow_enter();
    }
    s
}

fn reset_init(it: &Interp) {
    *it.init_get.borrow_mut() = Default::default();
}

pub async fn call(it: &Interp, c: &Ctx, name: &str, a: &[Value]) -> Option<LResult> {
    Some(match name {
        "PRINC" => {
            let s = a.first().map(princ_str).unwrap_or_default();
            out(it, c, &s);
            Ok(a.first().cloned().unwrap_or_else(|| Value::sym("")))
        }
        "PRIN1" => {
            let s = a.first().map(prin1_str).unwrap_or_default();
            out(it, c, &s);
            Ok(a.first().cloned().unwrap_or_else(|| Value::sym("")))
        }
        "PRINT" => {
            let s = a.first().map(prin1_str).unwrap_or_default();
            out(it, c, &format!("\n{s} "));
            Ok(a.first().cloned().unwrap_or(Value::Nil))
        }
        "TERPRI" => {
            out(it, c, "\n");
            Ok(Value::Nil)
        }
        "PROMPT" => match string(a, 0) {
            Ok(s) => {
                out(it, c, s);
                Ok(Value::Nil)
            }
            Err(e) => Err(e),
        },
        "ALERT" => match string(a, 0) {
            Ok(s) => {
                flush(it, c);
                c.log(s);
                Ok(Value::Nil)
            }
            Err(e) => Err(e),
        },
        "GETVAR" => (|| {
            let n = string(a, 0)?;
            let ty = c.app(|app| app.sysvars.var_type(n));
            match c.var(n) {
                Ok(crate::sysvars::Value::Num(x)) => {
                    Ok(if matches!(ty, crate::sysvars::VarType::Int) {
                        Value::Int(x as i64)
                    } else {
                        Value::Real(x)
                    })
                }
                Ok(crate::sysvars::Value::Str(s)) => Ok(Value::str(&s)),
                Err(_) => Ok(Value::Nil),
            }
        })(),
        "SETVAR" => (|| {
            let n = string(a, 0)?;
            let v = arg(a, 1)?;
            let r = match v {
                Value::Int(i) => c.setvar(n, *i),
                Value::Real(x) => c.setvar(n, *x),
                Value::Str(s) => c.setvar(n, s.to_string()),
                other => return Err(bad_arg("numberp", other)),
            };
            r.map_err(LErr::Msg)?;
            Ok(v.clone())
        })(),
        "COMMAND" | "COMMAND-S" => command(it, c, a).await,
        "VL-CMDF" => command(it, c, a).await.map(|_| Value::T),
        "INITGET" => (|| {
            let mut ig = super::InitGet::default();
            let mut rest = a;
            if let Some(Value::Int(b)) = a.first() {
                ig.bits = *b;
                rest = &a[1..];
            }
            if let Some(Value::Str(k)) = rest.first() {
                ig.keywords = k
                    .split_whitespace()
                    .map(|w| w.split(',').next().unwrap_or(w).to_string())
                    .collect();
            }
            *it.init_get.borrow_mut() = ig;
            Ok(Value::Nil)
        })(),
        "GETPOINT" | "GETCORNER" => {
            let base = a.first().and_then(Value::as_point).map(|p| to_wcs(c, p));
            let sp = spec(it, a, 0);
            reset_init(it);
            let r = if name == "GETCORNER" {
                match base {
                    Some(b) => c.corner(b, sp).await,
                    None => Err(CmdError::Msg("getcorner needs a base point".into())),
                }
            } else {
                c.point(match base {
                    Some(b) => sp.base(b),
                    None => sp,
                })
                .await
            };
            match r {
                Ok(PointR::Pt(p)) => Ok(from_wcs(c, p)),
                Ok(PointR::Kw(k)) => Ok(Value::str(&k)),
                Ok(PointR::Enter(_)) | Ok(PointR::Num { .. }) => Ok(Value::Nil),
                Err(e) => Err(e.into()),
            }
        }
        "GETDIST" | "GETANGLE" | "GETORIENT" => {
            let base = a.first().and_then(Value::as_point).map(|p| to_wcs(c, p));
            let mut sp = spec(it, a, 0);
            if let Some(b) = base {
                sp = sp.base(b);
            }
            reset_init(it);
            let r = if name == "GETDIST" {
                c.distance(sp).await
            } else {
                c.angle(sp).await
            };
            match r {
                Ok(MeasR::Value { value, .. }) => Ok(Value::Real(value)),
                Ok(MeasR::Kw(k)) => Ok(Value::str(&k)),
                Ok(MeasR::Enter(_)) => Ok(Value::Nil),
                Err(e) => Err(e.into()),
            }
        }
        "GETREAL" | "GETINT" => {
            let ig = it.init_get.borrow().clone();
            let mut sp = spec(it, a, 0);
            if ig.bits & 2 != 0 {
                sp = sp.nonzero();
            }
            if ig.bits & 4 != 0 {
                sp = sp.min(0.0);
            }
            reset_init(it);
            match c.number(sp).await {
                Ok(NumR::Value(x)) => Ok(if name == "GETINT" {
                    Value::Int(x.trunc() as i64)
                } else {
                    Value::Real(x)
                }),
                Ok(NumR::Kw(k)) => Ok(Value::str(&k)),
                Ok(NumR::Enter(_)) => Ok(Value::Nil),
                Err(e) => Err(e.into()),
            }
        }
        "GETSTRING" => {
            let spaces = a
                .first()
                .is_some_and(|v| v.is_true() && v.as_str().is_none());
            let mut sp = spec(it, a, 0);
            if spaces {
                sp = sp.raw();
            }
            reset_init(it);
            match c.string(sp).await {
                Ok(r) => Ok(if r.text.is_empty() && r.entered {
                    Value::str("")
                } else {
                    Value::str(&r.text)
                }),
                Err(e) => Err(e.into()),
            }
        }
        "GETKWORD" => {
            let ig = it.init_get.borrow().clone();
            if ig.keywords.is_empty() {
                reset_init(it);
                return Some(Ok(Value::Nil));
            }
            let sp = spec(it, a, 0);
            reset_init(it);
            match c.keyword(sp).await {
                Ok(r) => Ok(r.kw().map_or(Value::Nil, Value::str)),
                Err(e) => Err(e.into()),
            }
        }
        "ENTSEL" | "NENTSEL" => {
            let sp = spec(it, a, 0);
            reset_init(it);
            match c.entity(sp).await {
                Ok(EntR::Entity { id, pt: p, .. }) => {
                    Ok(Value::list(vec![Value::Ename(id), from_wcs(c, p)]))
                }
                Ok(EntR::Kw(k)) => Ok(Value::str(&k)),
                Ok(EntR::Enter) => Ok(Value::Nil),
                Err(e) => Err(e.into()),
            }
        }
        "TRANS" => (|| {
            let (x, y) = arg(a, 0)?
                .as_point()
                .ok_or_else(|| bad_arg("point list", &a[0]))?;
            let (from, to) = (
                arg(a, 1)?.as_i64().unwrap_or(0),
                arg(a, 2)?.as_i64().unwrap_or(0),
            );
            let disp = a.get(3).is_some_and(Value::is_true);
            let u = c.app(|app| app.ucs());
            let p = pt(x, y);
            let w = match (from, disp) {
                (1, false) => u.to_wcs(p),
                (1, true) => u.vec_to_wcs(p),
                _ => p,
            };
            let r = match (to, disp) {
                (1, false) => u.from_wcs(w),
                (1, true) => u.vec_from_wcs(w),
                _ => w,
            };
            let z = arg(a, 0)?
                .to_vec()
                .and_then(|l| l.get(2).and_then(Value::as_f64))
                .unwrap_or(0.0);
            Ok(Value::list(vec![
                Value::Real(r.x),
                Value::Real(r.y),
                Value::Real(z),
            ]))
        })(),
        "REDRAW" | "GRAPHSCR" | "TEXTSCR" | "VL-PROMPT" => {
            c.emit();
            Ok(Value::Nil)
        }
        "MENUCMD" | "SETFUNHELP" | "VLAX-RELEASE-OBJECT" => Ok(Value::Nil),
        _ if name.starts_with("VLAX-") || name.starts_with("VLA-") || name.starts_with("VLR-") => {
            Err(LErr::Msg(format!(
                "{name} is not supported: the ActiveX / Reactor API is not available in CadForge"
            )))
        }
        _ => return None,
    })
}
