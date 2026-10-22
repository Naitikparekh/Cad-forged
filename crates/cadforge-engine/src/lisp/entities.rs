//! Lisp functions on drawing objects: `entlast`, `entget`, `entmod`, `entmake`, `ssget` and friends. Entities are described
//! as DXF-style association lists (`((0 . "LINE") (8 . "0") (10 0.0 0.0 0.0) (11 10.0 0.0 0.0))`).
use super::builtins::wcmatch;
use super::value::*;
use super::{bad_arg, Interp};
use crate::ctx::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::Color;
use cadforge_core::text::{HJust, TextData, VJust};
use std::cell::RefCell;
use std::rc::Rc;

fn arg(a: &[Value], i: usize) -> LResult<&Value> {
    a.get(i).ok_or_else(|| LErr::Msg("too few arguments".into()))
}

fn ename(a: &[Value], i: usize) -> LResult<u64> {
    match arg(a, i)? {
        Value::Ename(e) => Ok(*e),
        other => Err(bad_arg("enamep", other)),
    }
}

fn dxf_name(k: &Kind) -> &'static str {
    match k {
        Kind::Line { .. } => "LINE",
        Kind::Circle { .. } => "CIRCLE",
        Kind::Arc { .. } => "ARC",
        Kind::Pline { .. } => "LWPOLYLINE",
        Kind::Ellipse(_) => "ELLIPSE",
        Kind::Spline { .. } => "SPLINE",
        Kind::Point { .. } => "POINT",
        Kind::Text(t) if t.att.is_some() => "ATTDEF",
        Kind::Text(t) if t.multi => "MTEXT",
        Kind::Text(_) => "TEXT",
        Kind::Dim(_) => "DIMENSION",
        Kind::Leader { .. } => "LEADER",
        Kind::Hatch(_) => "HATCH",
        Kind::Insert(_) => "INSERT",
        Kind::Xline { .. } => "XLINE",
        Kind::Ray { .. } => "RAY",
        Kind::Viewport(_) => "VIEWPORT",
        Kind::Table(_) => "ACAD_TABLE",
        Kind::Wipeout(_) => "WIPEOUT",
        Kind::Region(_) => "REGION",
        Kind::Image(_) => "IMAGE",
    }
}

fn p3(p: Pt) -> Value {
    Value::point(p.x, p.y)
}

fn p2(p: Pt) -> Value {
    Value::list(vec![Value::Real(p.x), Value::Real(p.y)])
}

fn pair(code: i64, v: Value) -> Value {
    Value::cons(Value::Int(code), v)
}

/// A group with a point value: `(10 x y z)`.
fn point_group(code: i64, p: Pt) -> Value {
    Value::cons(Value::Int(code), p3(p))
}

fn rad(a: f64) -> Value {
    Value::Real(a)
}

fn entity_list(e: &Entity, doc: &cadforge_core::drawing::Drawing) -> Value {
    let mut g: Vec<Value> = vec![pair(-1, Value::Ename(e.id)), pair(0, Value::str(dxf_name(&e.kind))), pair(5, Value::str(&format!("{:X}", e.id))), pair(100, Value::str("AcDbEntity")), pair(8, Value::str(if e.layer.is_empty() { "0" } else { &e.layer }))];
    if let Some(lt) = &e.ltype {
        g.push(pair(6, Value::str(if lt == "BYBLOCK" { "ByBlock" } else { lt })));
    }
    match e.color {
        Some(Color::Aci(n)) => g.push(pair(62, Value::Int(n as i64))),
        Some(Color::ByBlock) => g.push(pair(62, Value::Int(0))),
        Some(Color::Rgb(r, gg, b)) => {
            g.push(pair(62, Value::Int(cadforge_core::style::rgb_to_aci([r, gg, b]) as i64)));
            g.push(pair(420, Value::Int(((r as i64) << 16) | ((gg as i64) << 8) | b as i64)));
        }
        _ => {}
    }
    if let Some(lw) = e.lw {
        g.push(pair(370, Value::Int(lw as i64)));
    }
    match &e.kind {
        Kind::Line { a, b } => {
            g.push(point_group(10, *a));
            g.push(point_group(11, *b));
        }
        Kind::Circle { c, r } => {
            g.push(point_group(10, *c));
            g.push(pair(40, Value::Real(*r)));
        }
        Kind::Arc { c, r, a0, a1 } => {
            g.push(point_group(10, *c));
            g.push(pair(40, Value::Real(*r)));
            g.push(pair(50, rad(*a0)));
            g.push(pair(51, rad(*a1)));
        }
        Kind::Pline { v, closed } => {
            g.push(pair(100, Value::str("AcDbPolyline")));
            g.push(pair(90, Value::Int(v.len() as i64)));
            g.push(pair(70, Value::Int(*closed as i64)));
            for q in v {
                g.push(Value::cons(Value::Int(10), p2(pt(q.x, q.y))));
                if q.b != 0.0 {
                    g.push(pair(42, Value::Real(q.b)));
                }
            }
        }
        Kind::Point { p } => g.push(point_group(10, *p)),
        Kind::Text(t) => {
            g.push(pair(1, Value::str(&t.s)));
            g.push(point_group(10, t.p));
            g.push(pair(40, Value::Real(t.h)));
            g.push(pair(50, rad(t.rot)));
            if let Some(w) = t.w {
                g.push(pair(41, Value::Real(w)));
            }
            g.push(pair(7, Value::str(&doc.settings.text_style)));
        }
        Kind::Ellipse(el) => {
            g.push(point_group(10, el.c));
            g.push(point_group(11, el.m));
            g.push(pair(40, Value::Real(el.ratio)));
            g.push(pair(41, rad(el.t0)));
            g.push(pair(42, rad(el.t1)));
        }
        Kind::Insert(i) => {
            g.push(pair(2, Value::str(&i.name)));
            g.push(point_group(10, i.p));
            g.push(pair(41, Value::Real(i.sx)));
            g.push(pair(42, Value::Real(i.sy)));
            g.push(pair(50, rad(i.rot)));
            if !i.attribs.is_empty() {
                g.push(pair(66, Value::Int(1)));
            }
        }
        Kind::Hatch(h) => {
            g.push(pair(2, Value::str(&h.pattern)));
            g.push(pair(41, Value::Real(h.ps)));
            g.push(pair(52, rad(h.pa)));
            g.push(pair(91, Value::Int(h.loops.len() as i64)));
        }
        Kind::Spline { fit, closed } => {
            g.push(pair(70, Value::Int(*closed as i64)));
            g.push(pair(74, Value::Int(fit.len() as i64)));
            for q in fit {
                g.push(point_group(11, *q));
            }
        }
        Kind::Xline { p, d } | Kind::Ray { p, d } => {
            g.push(point_group(10, *p));
            g.push(point_group(11, *d));
        }
        Kind::Leader { pts, s, h } => {
            g.push(pair(1, Value::str(s)));
            g.push(pair(40, Value::Real(*h)));
            for q in pts {
                g.push(point_group(10, *q));
            }
        }
        Kind::Image(im) => {
            g.push(point_group(10, im.p));
            g.push(pair(2, Value::str(&im.name)));
        }
        _ => {}
    }
    Value::list(g)
}

/// `(code . value)` groups of an association list, in order. The value of `(10 x y)` is the list `(x y)`.
fn groups(v: &Value) -> LResult<Vec<(i64, Value)>> {
    let items = v.to_vec().ok_or_else(|| bad_arg("listp", v))?;
    let mut out = vec![];
    for it in items {
        let Value::Cons(c) = &it else { return Err(bad_arg("association list", &it)) };
        let code = c.0.as_i64().ok_or_else(|| bad_arg("integerp", &c.0))?;
        out.push((code, c.1.clone()));
    }
    Ok(out)
}

fn gpoint(v: &Value) -> Option<Pt> {
    v.as_point().map(|(x, y)| pt(x, y))
}

fn gnum(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn aci_color(n: i64) -> Option<Color> {
    match n {
        0 => Some(Color::ByBlock),
        256 => None,
        1..=255 => Some(Color::Aci(n as u8)),
        _ => None,
    }
}

/// Apply the groups of an association list to `e` (entmod). Returns false when a value is not acceptable.
fn apply_groups(e: &mut Entity, gs: &[(i64, Value)], doc: &cadforge_core::drawing::Drawing) -> bool {
    // polyline vertices come as repeated 10 / 42 groups
    if let Kind::Pline { v, closed } = &mut e.kind {
        let mut nv: Vec<Vertex> = vec![];
        for (code, val) in gs {
            match code {
                10 => match gpoint(val) {
                    Some(p) => nv.push(Vertex::new(p.x, p.y, 0.0)),
                    None => return false,
                },
                42 => {
                    if let (Some(last), Some(b)) = (nv.last_mut(), gnum(val)) {
                        last.b = b;
                    }
                }
                70 => *closed = gnum(val).is_some_and(|f| (f as i64) & 1 != 0),
                _ => {}
            }
        }
        if nv.len() >= 2 {
            *v = nv;
        }
    }
    if let Kind::Spline { fit, .. } = &mut e.kind {
        let pts: Vec<Pt> = gs.iter().filter(|(c, _)| *c == 11).filter_map(|(_, v)| gpoint(v)).collect();
        if pts.len() >= 2 {
            *fit = pts;
        }
    }
    let is_pline = matches!(e.kind, Kind::Pline { .. } | Kind::Spline { .. });
    for (code, val) in gs {
        match code {
            8 => {
                let Some(n) = val.as_str() else { return false };
                match doc.layer(n) {
                    Some(l) => e.layer = l.name.clone(),
                    None => return false,
                }
            }
            6 => {
                let Some(n) = val.as_str() else { return false };
                e.ltype = match n.to_uppercase().as_str() {
                    "BYLAYER" => None,
                    "BYBLOCK" => Some("BYBLOCK".into()),
                    u => cadforge_core::style::ltype_key(u).map(String::from),
                };
            }
            62 => e.color = gnum(val).and_then(|n| aci_color(n as i64)),
            420 => {
                if let Some(n) = gnum(val) {
                    let n = n as i64;
                    e.color = Some(Color::Rgb(((n >> 16) & 255) as u8, ((n >> 8) & 255) as u8, (n & 255) as u8));
                }
            }
            370 => e.lw = gnum(val).map(|n| n as i32),
            _ => {}
        }
        if is_pline {
            continue;
        }
        match (&mut e.kind, code) {
            (Kind::Line { a, .. }, 10) => {
                if let Some(p) = gpoint(val) {
                    *a = p
                }
            }
            (Kind::Line { b, .. }, 11) => {
                if let Some(p) = gpoint(val) {
                    *b = p
                }
            }
            (Kind::Circle { c, .. } | Kind::Arc { c, .. }, 10) => {
                if let Some(p) = gpoint(val) {
                    *c = p
                }
            }
            (Kind::Circle { r, .. } | Kind::Arc { r, .. }, 40) => {
                if let Some(x) = gnum(val).filter(|x| *x > 0.0) {
                    *r = x
                }
            }
            (Kind::Arc { a0, .. }, 50) => {
                if let Some(x) = gnum(val) {
                    *a0 = x
                }
            }
            (Kind::Arc { a1, .. }, 51) => {
                if let Some(x) = gnum(val) {
                    *a1 = x
                }
            }
            (Kind::Point { p }, 10) => {
                if let Some(q) = gpoint(val) {
                    *p = q
                }
            }
            (Kind::Text(t), 10) => {
                if let Some(q) = gpoint(val) {
                    t.p = q
                }
            }
            (Kind::Text(t), 1) => {
                if let Some(s) = val.as_str() {
                    t.s = s.to_string()
                }
            }
            (Kind::Text(t), 40) => {
                if let Some(x) = gnum(val).filter(|x| *x > 0.0) {
                    t.h = x
                }
            }
            (Kind::Text(t), 50) => {
                if let Some(x) = gnum(val) {
                    t.rot = x
                }
            }
            (Kind::Ellipse(el), 10) => {
                if let Some(q) = gpoint(val) {
                    el.c = q
                }
            }
            (Kind::Ellipse(el), 11) => {
                if let Some(q) = gpoint(val) {
                    el.m = q
                }
            }
            (Kind::Ellipse(el), 40) => {
                if let Some(x) = gnum(val).filter(|x| *x > 0.0 && *x <= 1.0) {
                    el.ratio = x
                }
            }
            (Kind::Insert(i), 10) => {
                if let Some(q) = gpoint(val) {
                    i.p = q
                }
            }
            (Kind::Insert(i), 41) => {
                if let Some(x) = gnum(val).filter(|x| *x != 0.0) {
                    i.sx = x
                }
            }
            (Kind::Insert(i), 42) => {
                if let Some(x) = gnum(val).filter(|x| *x != 0.0) {
                    i.sy = x
                }
            }
            (Kind::Insert(i), 50) => {
                if let Some(x) = gnum(val) {
                    i.rot = x
                }
            }
            (Kind::Xline { p, .. } | Kind::Ray { p, .. }, 10) => {
                if let Some(q) = gpoint(val) {
                    *p = q
                }
            }
            (Kind::Xline { d, .. } | Kind::Ray { d, .. }, 11) => {
                if let Some(q) = gpoint(val) {
                    *d = q
                }
            }
            _ => {}
        }
    }
    true
}

/// Build an entity from the groups of `entmake`.
fn make_entity(gs: &[(i64, Value)]) -> Option<Entity> {
    let ty = gs.iter().find(|(c, _)| *c == 0)?.1.as_str()?.to_uppercase();
    let first = |code: i64| gs.iter().find(|(c, _)| *c == code).map(|(_, v)| v);
    let point = |code: i64| first(code).and_then(gpoint);
    let kind = match ty.as_str() {
        "LINE" => Kind::Line { a: point(10)?, b: point(11)? },
        "CIRCLE" => Kind::Circle { c: point(10)?, r: first(40).and_then(gnum).filter(|r| *r > 0.0)? },
        "ARC" => Kind::Arc { c: point(10)?, r: first(40).and_then(gnum).filter(|r| *r > 0.0)?, a0: first(50).and_then(gnum).unwrap_or(0.0), a1: first(51).and_then(gnum).unwrap_or(std::f64::consts::PI) },
        "POINT" => Kind::Point { p: point(10)? },
        "XLINE" => Kind::Xline { p: point(10)?, d: point(11)? },
        "RAY" => Kind::Ray { p: point(10)?, d: point(11)? },
        "ELLIPSE" => Kind::Ellipse(EllipseData { c: point(10)?, m: point(11)?, ratio: first(40).and_then(gnum).filter(|r| *r > 0.0 && *r <= 1.0)?, t0: first(41).and_then(gnum).unwrap_or(0.0), t1: first(42).and_then(gnum).unwrap_or(std::f64::consts::TAU) }),
        "TEXT" | "MTEXT" => {
            let s = first(1)?.as_str()?.to_string();
            Kind::Text(TextData { p: point(10)?, h: first(40).and_then(gnum).filter(|h| *h > 0.0).unwrap_or(2.5), s, rot: first(50).and_then(gnum).unwrap_or(0.0), jh: HJust::Left, jv: VJust::Baseline, w: first(41).and_then(gnum), multi: ty == "MTEXT", att: None })
        }
        "INSERT" => Kind::Insert(InsertData { name: first(2)?.as_str()?.to_string(), p: point(10)?, sx: first(41).and_then(gnum).filter(|x| *x != 0.0).unwrap_or(1.0), sy: first(42).and_then(gnum).filter(|x| *x != 0.0).unwrap_or(1.0), rot: first(50).and_then(gnum).unwrap_or(0.0), attribs: vec![], clip: vec![], clip_off: false }),
        "LWPOLYLINE" => {
            let mut v: Vec<Vertex> = vec![];
            for (code, val) in gs {
                match code {
                    10 => {
                        let p = gpoint(val)?;
                        v.push(Vertex::new(p.x, p.y, 0.0));
                    }
                    42 => {
                        if let (Some(l), Some(b)) = (v.last_mut(), gnum(val)) {
                            l.b = b;
                        }
                    }
                    _ => {}
                }
            }
            if v.len() < 2 {
                return None;
            }
            Kind::Pline { v, closed: first(70).and_then(gnum).is_some_and(|f| (f as i64) & 1 != 0) }
        }
        _ => return None,
    };
    Some(Entity::new(kind))
}

/// Does `e` satisfy the filter groups of `ssget`?
fn matches_filter(e: &Entity, filter: &[(i64, Value)]) -> bool {
    filter.iter().all(|(code, val)| match code {
        0 => val.as_str().is_some_and(|p| wcmatch(dxf_name(&e.kind), p, true)),
        8 => val.as_str().is_some_and(|p| wcmatch(if e.layer.is_empty() { "0" } else { &e.layer }, p, true)),
        2 => matches!(&e.kind, Kind::Insert(i) if val.as_str().is_some_and(|p| wcmatch(&i.name, p, true))),
        1 => matches!(&e.kind, Kind::Text(t) if val.as_str().is_some_and(|p| wcmatch(&t.s, p, false))),
        6 => val.as_str().is_some_and(|p| wcmatch(e.ltype.as_deref().unwrap_or("BYLAYER"), p, true)),
        62 => {
            let have = match e.color {
                Some(Color::Aci(n)) => n as i64,
                Some(Color::ByBlock) => 0,
                _ => 256,
            };
            val.as_i64() == Some(have)
        }
        _ => true,
    })
}

fn selset(ids: Vec<u64>) -> Value {
    if ids.is_empty() {
        Value::Nil
    } else {
        Value::Sel(Rc::new(RefCell::new(ids)))
    }
}

fn sel_of<'a>(a: &'a [Value], i: usize) -> LResult<&'a Rc<RefCell<Vec<u64>>>> {
    match arg(a, i)? {
        Value::Sel(s) => Ok(s),
        other => Err(bad_arg("PICKSETp", other)),
    }
}

pub async fn call(_it: &Interp, c: &Ctx, name: &str, a: &[Value]) -> Option<LResult> {
    Some(match name {
        "ENTLAST" => Ok(c.doc(|d| d.entities().last().map(|e| Value::Ename(e.id))).unwrap_or(Value::Nil)),
        "ENTNEXT" => (|| {
            let ids: Vec<u64> = c.doc(|d| d.entities().iter().map(|e| e.id).collect());
            Ok(match a.first() {
                None | Some(Value::Nil) => ids.first().map_or(Value::Nil, |i| Value::Ename(*i)),
                Some(Value::Ename(e)) => ids.iter().position(|i| i == e).and_then(|p| ids.get(p + 1)).map_or(Value::Nil, |i| Value::Ename(*i)),
                Some(other) => return Err(bad_arg("enamep", other)),
            })
        })(),
        "HANDENT" => (|| {
            let h = arg(a, 0)?.as_str().ok_or_else(|| bad_arg("stringp", &a[0]))?;
            let id = u64::from_str_radix(h.trim(), 16).ok();
            Ok(id.filter(|i| c.doc(|d| d.by_id(*i).is_some())).map_or(Value::Nil, Value::Ename))
        })(),
        "ENTGET" => (|| {
            let id = ename(a, 0)?;
            Ok(c.doc(|d| d.by_id(id).map(|e| entity_list(e, d))).unwrap_or(Value::Nil))
        })(),
        "ENTMOD" => (|| {
            let gs = groups(arg(a, 0)?)?;
            let Some((_, Value::Ename(id))) = gs.iter().find(|(c, _)| *c == -1) else { return Ok(Value::Nil) };
            let id = *id;
            let ok = c.doc_mut(|d| {
                let Some(mut e) = d.by_id(id).cloned() else { return false };
                if !apply_groups(&mut e, &gs, d) {
                    return false;
                }
                d.modify(id, |slot| *slot = e);
                true
            });
            c.emit();
            Ok(if ok { a[0].clone() } else { Value::Nil })
        })(),
        "ENTUPD" => {
            c.emit();
            Ok(a.first().cloned().unwrap_or(Value::Nil))
        }
        "ENTMAKE" => (|| {
            let gs = groups(arg(a, 0)?)?;
            let Some(mut e) = make_entity(&gs) else { return Ok(Value::Nil) };
            let ok = c.doc(|d| {
                let mut probe = e.clone();
                apply_groups(&mut probe, &gs, d).then_some(probe)
            });
            let Some(probed) = ok else { return Ok(Value::Nil) };
            e = probed;
            c.add(e);
            Ok(a[0].clone())
        })(),
        "ENTDEL" => (|| {
            let id = ename(a, 0)?;
            let had = c.doc_mut(|d| {
                if d.by_id(id).is_some() {
                    d.change(|d| d.remove(id));
                    true
                } else {
                    false
                }
            });
            c.emit();
            Ok(if had { a[0].clone() } else { Value::Nil })
        })(),
        // ---------------------------------------------------------------- selection sets
        "SSGET" => ssget(c, a).await,
        "SSLENGTH" => sel_of(a, 0).map(|s| Value::Int(s.borrow().len() as i64)),
        "SSNAME" => (|| {
            let s = sel_of(a, 0)?;
            let i = arg(a, 1)?.as_i64().ok_or_else(|| bad_arg("integerp", &a[1]))?;
            Ok(if i < 0 { Value::Nil } else { s.borrow().get(i as usize).map_or(Value::Nil, |e| Value::Ename(*e)) })
        })(),
        "SSADD" => (|| {
            let id = ename(a, 0)?;
            let set = match a.get(1) {
                Some(Value::Sel(s)) => s.clone(),
                _ => Rc::new(RefCell::new(vec![])),
            };
            if !set.borrow().contains(&id) {
                set.borrow_mut().push(id);
            }
            Ok(Value::Sel(set))
        })(),
        "SSDEL" => (|| {
            let id = ename(a, 0)?;
            let s = sel_of(a, 1)?;
            s.borrow_mut().retain(|e| *e != id);
            Ok(a[1].clone())
        })(),
        "SSMEMB" => (|| {
            let id = ename(a, 0)?;
            let s = sel_of(a, 1)?;
            Ok(if s.borrow().contains(&id) { a[0].clone() } else { Value::Nil })
        })(),
        "SSSETFIRST" => (|| {
            let ids = match a.get(1) {
                Some(Value::Sel(s)) => s.borrow().clone(),
                _ => vec![],
            };
            c.ed(|e| {
                e.sel.clear();
                e.select(ids);
            });
            c.emit();
            Ok(Value::Nil)
        })(),
        "SSGETFIRST" => {
            let ids: Vec<u64> = c.ed(|e| e.sel.iter().copied().collect());
            Ok(Value::list(vec![Value::Nil, selset(ids)]))
        }
        // ---------------------------------------------------------------- tables
        "TBLSEARCH" => (|| {
            let table = arg(a, 0)?.as_str().ok_or_else(|| bad_arg("stringp", &a[0]))?.to_uppercase();
            let n = arg(a, 1)?.as_str().ok_or_else(|| bad_arg("stringp", &a[1]))?;
            Ok(c.doc(|d| table_entry(d, &table, n)).unwrap_or(Value::Nil))
        })(),
        _ => return None,
    })
}

fn table_entry(d: &cadforge_core::drawing::Drawing, table: &str, name: &str) -> Option<Value> {
    match table {
        "LAYER" => {
            let l = d.layer(name)?;
            let flags = (l.frozen as i64) | if l.locked { 4 } else { 0 };
            let aci = match l.color {
                Color::Aci(n) => n as i64,
                Color::Rgb(r, g, b) => cadforge_core::style::rgb_to_aci([r, g, b]) as i64,
                _ => 7,
            };
            Some(Value::list(vec![pair(0, Value::str("LAYER")), pair(2, Value::str(&l.name)), pair(70, Value::Int(flags)), pair(62, Value::Int(if l.on { aci } else { -aci })), pair(6, Value::str(&l.ltype))]))
        }
        "BLOCK" => {
            let b = d.blocks.get(name).or_else(|| d.blocks.values().find(|b| b.name.eq_ignore_ascii_case(name)))?;
            Some(Value::list(vec![pair(0, Value::str("BLOCK")), pair(2, Value::str(&b.name)), pair(70, Value::Int(0)), point_group(10, b.base)]))
        }
        "LTYPE" => {
            let k = cadforge_core::style::ltype_key(name)?;
            Some(Value::list(vec![pair(0, Value::str("LTYPE")), pair(2, Value::str(k)), pair(70, Value::Int(0))]))
        }
        "STYLE" => (d.settings.text_style.eq_ignore_ascii_case(name)).then(|| Value::list(vec![pair(0, Value::str("STYLE")), pair(2, Value::str(&d.settings.text_style)), pair(70, Value::Int(0))])),
        _ => None,
    }
}

async fn ssget(c: &Ctx, a: &[Value]) -> LResult {
    let filter_of = |v: Option<&Value>| -> LResult<Vec<(i64, Value)>> {
        match v {
            Some(v) if matches!(v, Value::Cons(_)) => groups(v),
            _ => Ok(vec![]),
        }
    };
    let mode = a.first().and_then(Value::as_str).map(|s| s.trim_start_matches('_').to_uppercase());
    // `(ssget filter)` and `(ssget)` ask the user
    if mode.is_none() {
        let filter = filter_of(a.first())?;
        let f2 = filter.clone();
        let mut sp = crate::runtime::PromptSpec::new("Select objects");
        if !filter.is_empty() {
            sp = sp.filter(move |e| matches_filter(e, &f2));
        }
        let sel = c.select(sp).await?;
        return Ok(selset(sel.ents));
    }
    let mode = mode.unwrap_or_default();
    let in_space = |keep: &dyn Fn(&Entity, &cadforge_core::drawing::Drawing) -> bool| -> Vec<u64> { c.doc(|d| d.entities().iter().filter(|e| !d.layer_of(e).frozen && keep(e, d)).map(|e| e.id).collect()) };
    let pt_arg = |i: usize| -> LResult<Pt> { a.get(i).and_then(Value::as_point).map(|(x, y)| pt(x, y)).ok_or_else(|| bad_arg("point list", a.get(i).unwrap_or(&Value::Nil))) };
    match mode.as_str() {
        "X" => {
            let filter = filter_of(a.get(1))?;
            Ok(selset(in_space(&|e, _| matches_filter(e, &filter))))
        }
        "W" | "C" | "WP" | "CP" => {
            let filter = filter_of(a.get(if mode.len() == 1 { 3 } else { 2 }))?;
            let poly: Vec<Pt> = if mode.len() == 1 {
                let (p, q) = (pt_arg(1)?, pt_arg(2)?);
                vec![pt(p.x.min(q.x), p.y.min(q.y)), pt(p.x.max(q.x), p.y.min(q.y)), pt(p.x.max(q.x), p.y.max(q.y)), pt(p.x.min(q.x), p.y.max(q.y))]
            } else {
                a.get(1).and_then(Value::to_vec).unwrap_or_default().iter().filter_map(|v| v.as_point().map(|(x, y)| pt(x, y))).collect()
            };
            if poly.len() < 3 {
                return Err(LErr::Msg("bad argument: at least three points are needed".into()));
            }
            let cross = mode.starts_with('C');
            Ok(selset(in_space(&|e, d| {
                if !matches_filter(e, &filter) {
                    return false;
                }
                let Some(b) = bounds_of(e, d) else { return false };
                let corners = [pt(b.min_x, b.min_y), pt(b.max_x, b.min_y), pt(b.max_x, b.max_y), pt(b.min_x, b.max_y)];
                if cross {
                    // crossing: the bounds touch the polygon (corner inside, or an edge of the polygon crosses the bounds)
                    corners.iter().any(|q| point_in_polygon(*q, &poly)) || poly.iter().any(|q| q.x >= b.min_x && q.x <= b.max_x && q.y >= b.min_y && q.y <= b.max_y) || (0..poly.len()).any(|i| {
                        let (p0, p1) = (poly[i], poly[(i + 1) % poly.len()]);
                        (0..4).any(|k| !seg_intersect(&Seg::line(p0, p1), &Seg::line(corners[k], corners[(k + 1) % 4]), false, false).is_empty())
                    })
                } else {
                    corners.iter().all(|q| point_in_polygon(*q, &poly))
                }
            })))
        }
        "L" => Ok(c.doc(|d| d.entities().last().map(|e| selset(vec![e.id]))).unwrap_or(Value::Nil)),
        "I" | "P" => {
            let ids: Vec<u64> = c.ed(|e| e.sel.iter().copied().collect());
            Ok(selset(ids))
        }
        _ => {
            // modes like ":S" (single) behave like the plain interactive form
            let sel = c.select(crate::runtime::PromptSpec::new("Select objects")).await?;
            Ok(selset(sel.ents))
        }
    }
}
