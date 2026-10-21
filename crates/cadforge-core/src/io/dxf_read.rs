//! DXF reader: tolerant ASCII parser for the entities CadForge understands (port of `src/io/dxf-read.js`).
//!
//! Supported: LINE CIRCLE ARC LWPOLYLINE POLYLINE ELLIPSE SPLINE POINT TEXT MTEXT INSERT HATCH SOLID/TRACE XLINE RAY
//! LEADER and DIMENSION (imported as the geometry of its anonymous block, so it looks exactly like the source drawing).
use crate::drawing::Drawing;
use crate::entities::*;
use crate::geom::*;
use crate::hatch::pattern;
use crate::style::*;
use crate::text::{AttDef, HJust, TextData, VJust};
use std::collections::HashMap;

// ---------------------------------------------------------------- text decoding

/// Decode `%%c`, `%%d`, `%%p` and `\U+XXXX` escapes in TEXT / names.
pub fn decode_text(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 7 <= chars.len() && chars[i + 1] == 'U' && chars[i + 2] == '+' {
            let hex: String = chars[i + 3..i + 7].iter().collect();
            if hex.chars().all(|h| h.is_ascii_hexdigit()) {
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                    i += 7;
                    continue;
                }
            }
        }
        if c == '%' && i + 2 < chars.len() && chars[i + 1] == '%' {
            match chars[i + 2] {
                'c' | 'C' => {
                    out.push('⌀');
                    i += 3;
                    continue;
                }
                'd' | 'D' => {
                    out.push('°');
                    i += 3;
                    continue;
                }
                'p' | 'P' => {
                    out.push('±');
                    i += 3;
                    continue;
                }
                'u' | 'U' | 'o' | 'O' => {
                    i += 3;
                    continue;
                }
                '%' => {
                    out.push('%');
                    i += 3;
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Strip MTEXT formatting codes, returning plain text with `\n` line breaks.
pub fn decode_mtext(src: &str) -> String {
    let s: Vec<char> = decode_text(src).chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c == '\\' {
            let n = s.get(i + 1).copied();
            match n {
                Some('P') | Some('p') => {
                    out.push('\n');
                    i += 1;
                }
                Some('~') => {
                    out.push(' ');
                    i += 1;
                }
                Some(x @ ('\\' | '{' | '}')) => {
                    out.push(x);
                    i += 1;
                }
                Some('L' | 'l' | 'O' | 'o' | 'K' | 'k') => i += 1,
                Some('S') => {
                    // stacked fraction \Sa^b; / \Sa#b;
                    if let Some(j) = s[i..].iter().position(|&ch| ch == ';') {
                        let body: String = s[i + 2..i + j].iter().collect();
                        let mut done = false;
                        let mut r = String::new();
                        for ch in body.chars() {
                            if !done && (ch == '^' || ch == '#') {
                                r.push('/');
                                done = true;
                            } else {
                                r.push(ch);
                            }
                        }
                        out.push_str(&r);
                        i += j;
                    } else {
                        out.push(c);
                    }
                }
                Some('f' | 'F' | 'C' | 'c' | 'H' | 'h' | 'W' | 'w' | 'Q' | 'q' | 'T' | 't' | 'A' | 'a' | 'X' | 'x') => match s[i..].iter().position(|&ch| ch == ';') {
                    Some(j) => i += j,
                    None => i = s.len(),
                },
                _ => out.push(c),
            }
        } else if c == '{' || c == '}' {
            // grouping
        } else {
            out.push(c);
        }
        i += 1;
    }
    out.trim_end_matches('\n').to_string()
}

// ---------------------------------------------------------------- tokenizer

type Tok = (i32, String);

fn tokenize(text: &str) -> Vec<Tok> {
    let lines: Vec<&str> = text.split(['\n', '\r']).collect();
    // `\r\n` produces an empty element between; normalise by splitting properly
    let mut norm: Vec<&str> = Vec::with_capacity(lines.len());
    {
        let bytes = text.as_bytes();
        let mut start = 0;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\n' || bytes[i] == b'\r' {
                norm.push(&text[start..i]);
                if bytes[i] == b'\r' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    i += 1;
                }
                start = i + 1;
            }
            i += 1;
        }
        norm.push(&text[start..]);
    }
    let mut out = vec![];
    let mut i = 0;
    while i + 1 < norm.len() {
        match norm[i].trim().parse::<i32>() {
            Ok(code) => out.push((code, norm[i + 1].trim().to_string())),
            Err(_) => {}
        }
        i += 2;
    }
    out
}

#[derive(Default, Clone)]
struct Rec {
    ty: String,
    g: Vec<Tok>,
    /// POLYLINE vertices.
    vertices: Vec<Rec>,
}

fn records(tokens: &[Tok]) -> Vec<Rec> {
    let mut recs: Vec<Rec> = vec![];
    for t in tokens {
        if t.0 == 0 {
            recs.push(Rec { ty: t.1.clone(), g: vec![], vertices: vec![] });
        } else if let Some(r) = recs.last_mut() {
            r.g.push(t.clone());
        }
    }
    recs
}

impl Rec {
    fn first(&self, code: i32) -> Option<&str> {
        self.g.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str())
    }
    fn num(&self, code: i32, d: f64) -> f64 {
        self.first(code).map(parse_f).unwrap_or(d)
    }
    fn int(&self, code: i32, d: i64) -> i64 {
        self.first(code).and_then(|v| v.trim().parse::<i64>().ok().or_else(|| v.trim().parse::<f64>().ok().map(|f| f as i64))).unwrap_or(d)
    }
    fn all(&self, code: i32) -> Vec<&str> {
        self.g.iter().filter(|(c, _)| *c == code).map(|(_, v)| v.as_str()).collect()
    }
    fn all_num(&self, code: i32) -> Vec<f64> {
        self.all(code).into_iter().map(parse_f).collect()
    }
    fn pt(&self, i: i32) -> Pt {
        pt(self.num(10 + i, 0.0), self.num(20 + i, 0.0))
    }
    /// Collect points given as repeated (10, 20) pairs (e.g. polyline vertices).
    fn pts(&self, cx: i32, cy: i32) -> Vec<Pt> {
        let mut v: Vec<Pt> = vec![];
        for (c, s) in &self.g {
            if *c == cx {
                v.push(pt(parse_f(s), 0.0));
            } else if *c == cy {
                if let Some(l) = v.last_mut() {
                    l.y = parse_f(s);
                }
            }
        }
        v
    }
}

/// C-style lenient float parse (`parseFloat`): reads the longest numeric prefix, NaN on failure.
fn parse_f(s: &str) -> f64 {
    let t = s.trim();
    if let Ok(v) = t.parse::<f64>() {
        return v;
    }
    let mut end = 0;
    for (i, ch) in t.char_indices() {
        let upto = i + ch.len_utf8();
        if t[..upto].parse::<f64>().is_ok() {
            end = upto;
        }
    }
    if end > 0 {
        t[..end].parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

fn sections(recs: Vec<Rec>) -> HashMap<String, Vec<Rec>> {
    let mut out: HashMap<String, Vec<Rec>> = HashMap::new();
    let mut cur: Option<String> = None;
    for r in recs {
        if r.ty == "SECTION" {
            cur = r.first(2).map(|s| s.to_string());
            if let Some(n) = &cur {
                out.entry(n.clone()).or_default();
            }
        } else if r.ty == "ENDSEC" {
            cur = None;
        } else if let Some(n) = &cur {
            out.get_mut(n).unwrap().push(r);
        }
    }
    out
}

fn parse_header(tokens: &[Tok]) -> HashMap<String, Vec<Tok>> {
    let mut vars: HashMap<String, Vec<Tok>> = HashMap::new();
    let mut in_header = false;
    let mut cur: Option<String> = None;
    let mut i = 0;
    while i < tokens.len() {
        let (c, v) = &tokens[i];
        if *c == 0 && v == "SECTION" && tokens.get(i + 1).map(|t| t.1.as_str()) == Some("HEADER") {
            in_header = true;
            i += 2;
            continue;
        }
        if *c == 0 && v == "ENDSEC" && in_header {
            break;
        }
        if in_header {
            if *c == 9 {
                vars.insert(v.clone(), vec![]);
                cur = Some(v.clone());
            } else if let Some(k) = &cur {
                vars.get_mut(k).unwrap().push((*c, v.clone()));
            }
        }
        i += 1;
    }
    vars
}

// ---------------------------------------------------------------- colour / properties

fn color_of(r: &Rec) -> Option<Color> {
    if let Some(tc) = r.first(420) {
        let n = tc.trim().parse::<i64>().unwrap_or(0) & 0xFFFFFF;
        return Some(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8));
    }
    let c = r.first(62)?;
    let n = c.trim().parse::<i64>().ok()?;
    match n {
        256 => None,
        0 => Some(Color::ByBlock),
        n if n.abs() <= 255 => Some(Color::Aci(n.unsigned_abs() as u8)),
        _ => None,
    }
}

fn ltype_of(name: Option<&str>) -> Option<String> {
    let n = name?;
    if n.is_empty() {
        return None;
    }
    let u = n.to_uppercase();
    if u == "BYLAYER" {
        return None;
    }
    if u == "BYBLOCK" {
        return Some("BYBLOCK".into());
    }
    ltype_key(n).map(|s| s.to_string())
}

fn lw_of(r: &Rec) -> Option<i32> {
    let v = r.first(370)?.trim().parse::<i32>().ok()?;
    if v == LW_BYLAYER {
        None
    } else {
        Some(v)
    }
}

// ---------------------------------------------------------------- B-spline evaluation

fn bspline_sample(deg: usize, knots: &[f64], ctrl: &[Pt], weights: Option<&[f64]>, per_span: usize) -> Vec<Pt> {
    let n = ctrl.len() - 1;
    let (t0, t1) = (knots[deg], knots[n + 1]);
    let mut uniq: Vec<f64> = vec![];
    for k in &knots[deg..=n + 1] {
        if !uniq.iter().any(|u| *u == *k) {
            uniq.push(*k);
        }
    }
    let spans = (uniq.len() as i64 - 1).max(1) as usize;
    let count = (spans * per_span).clamp(24, 2000);
    let w = |i: usize| weights.map_or(1.0, |ws| ws.get(i).copied().unwrap_or(1.0));
    let at = |t: f64| -> Pt {
        let mut k = deg;
        while k < n && t >= knots[k + 1] {
            k += 1;
        }
        let mut d: Vec<(f64, f64, f64)> = (0..=deg)
            .map(|i| {
                let idx = k - deg + i;
                (ctrl[idx].x * w(idx), ctrl[idx].y * w(idx), w(idx))
            })
            .collect();
        for r in 1..=deg {
            for j in (r..=deg).rev() {
                let den = knots[j + 1 + k - r] - knots[j + k - deg];
                let a = if den == 0.0 { 0.0 } else { (t - knots[j + k - deg]) / den };
                d[j] = ((1.0 - a) * d[j - 1].0 + a * d[j].0, (1.0 - a) * d[j - 1].1 + a * d[j].1, (1.0 - a) * d[j - 1].2 + a * d[j].2);
            }
        }
        pt(d[deg].0 / d[deg].2, d[deg].1 / d[deg].2)
    };
    let mut pts: Vec<Pt> = (0..count).map(|i| at(t0 + (t1 - t0) * i as f64 / count as f64)).collect();
    pts.push(ctrl[n]);
    pts
}

// ---------------------------------------------------------------- hatch boundaries

fn hatch_loops(r: &Rec) -> Vec<Vec<Vertex>> {
    let gs = &r.g;
    let mut loops: Vec<Vec<Vertex>> = vec![];
    let Some(mut i) = gs.iter().position(|(c, _)| *c == 91) else {
        return loops;
    };
    // counts come from the file: never trust them beyond the number of records that could back them
    let cap = gs.len();
    let n = (parse_f(&gs[i].1) as i64).clamp(0, cap as i64);
    i += 1;
    // read the next value with `code`, skipping everything in between
    let num = |i: &mut usize, code: i32| -> f64 {
        while *i < gs.len() && gs[*i].0 != code {
            *i += 1;
        }
        if *i < gs.len() {
            let v = parse_f(&gs[*i].1);
            *i += 1;
            v
        } else {
            f64::NAN
        }
    };
    for _ in 0..n {
        let flags = num(&mut i, 92);
        if flags.is_nan() {
            break;
        }
        if (flags as i64) & 2 != 0 {
            // polyline path
            let has_bulge = num(&mut i, 72);
            let nv = (num(&mut i, 93) as usize).min(cap);
            num(&mut i, 73);
            let mut verts = vec![];
            for _ in 0..nv {
                let x = num(&mut i, 10);
                let y = num(&mut i, 20);
                let mut b = 0.0;
                if has_bulge != 0.0 && i < gs.len() && gs[i].0 == 42 {
                    b = parse_f(&gs[i].1);
                    i += 1;
                }
                verts.push(Vertex::new(x, y, b));
            }
            if verts.len() > 2 {
                loops.push(verts);
            }
        } else {
            // edge path
            let ne = (num(&mut i, 93) as usize).min(cap);
            let mut edges: Vec<Seg> = vec![];
            for _ in 0..ne {
                let ty = num(&mut i, 72) as i64;
                match ty {
                    1 => {
                        let a = pt(num(&mut i, 10), num(&mut i, 20));
                        let b = pt(num(&mut i, 11), num(&mut i, 21));
                        edges.push(Seg::line(a, b));
                    }
                    2 => {
                        let c = pt(num(&mut i, 10), num(&mut i, 20));
                        let rr = num(&mut i, 40);
                        let a0 = rad(num(&mut i, 50));
                        let a1 = rad(num(&mut i, 51));
                        let ccw = num(&mut i, 73) != 0.0;
                        let mut sw = if ccw { sweep_ccw(a0, a1) } else { -sweep_ccw(a1, a0) };
                        if sw.abs() < 1e-12 {
                            sw = if ccw { TAU } else { -TAU };
                        }
                        edges.push(Seg::arc(c, rr, a0, sw));
                    }
                    3 => {
                        let c = pt(num(&mut i, 10), num(&mut i, 20));
                        let m = pt(num(&mut i, 11), num(&mut i, 21));
                        let ratio = num(&mut i, 40);
                        let a0 = rad(num(&mut i, 50));
                        let a1 = rad(num(&mut i, 51));
                        let ccw = num(&mut i, 73) != 0.0;
                        let sw = if ccw {
                            let s = sweep_ccw(a0, a1);
                            if s == 0.0 {
                                TAU
                            } else {
                                s
                            }
                        } else {
                            let s = sweep_ccw(a1, a0);
                            -(if s == 0.0 { TAU } else { s })
                        };
                        let steps = 48;
                        let pts: Vec<Pt> = (0..=steps)
                            .map(|s| {
                                let t = a0 + sw * s as f64 / steps as f64;
                                pt(c.x + m.x * t.cos() - m.y * ratio * t.sin(), c.y + m.y * t.cos() + m.x * ratio * t.sin())
                            })
                            .collect();
                        for s in 0..steps {
                            edges.push(Seg::line(pts[s], pts[s + 1]));
                        }
                    }
                    4 => {
                        // spline edge: approximate through its control polygon
                        let deg = (num(&mut i, 94) as usize).min(cap);
                        num(&mut i, 73);
                        num(&mut i, 74);
                        let nk = (num(&mut i, 95) as usize).min(cap);
                        let nc = (num(&mut i, 96) as usize).min(cap);
                        let knots: Vec<f64> = (0..nk).map(|_| num(&mut i, 40)).collect();
                        let ctrl: Vec<Pt> = (0..nc).map(|_| pt(num(&mut i, 10), num(&mut i, 20))).collect();
                        if ctrl.len() > deg && knots.len() == ctrl.len() + deg + 1 {
                            let pts = bspline_sample(deg, &knots, &ctrl, None, 10);
                            for s in 0..pts.len() - 1 {
                                edges.push(Seg::line(pts[s], pts[s + 1]));
                            }
                        } else {
                            for s in 0..ctrl.len().saturating_sub(1) {
                                edges.push(Seg::line(ctrl[s], ctrl[s + 1]));
                            }
                        }
                    }
                    _ => break,
                }
            }
            let lp = chain(&edges);
            if lp.len() > 2 {
                loops.push(lp);
            }
            // skip the source-object references so the next loop's 92 is found
            num(&mut i, 97);
        }
    }
    loops
}

fn rev(s: &Seg) -> Seg {
    match *s {
        Seg::L { a, b, approx } => Seg::L { a: b, b: a, approx },
        Seg::A { c, r, a0, sw, full } => Seg::A { c, r, a0: a0 + sw, sw: -sw, full },
    }
}

/// Join edges end-to-start into a closed vertex loop with bulges.
fn chain(edges: &[Seg]) -> Vec<Vertex> {
    if edges.is_empty() {
        return vec![];
    }
    let mut left: Vec<Seg> = edges.to_vec();
    let mut seq = vec![left.remove(0)];
    let scale = edges.iter().map(|e| e.start().x.abs() + e.start().y.abs()).fold(1.0, f64::max);
    let tol = 1e-5 * scale;
    while !left.is_empty() {
        let tail = seq.last().unwrap().end();
        let mut flip = false;
        let mut k = left.iter().position(|e| dist(e.start(), tail) <= tol);
        if k.is_none() {
            k = left.iter().position(|e| dist(e.end(), tail) <= tol);
            flip = true;
        }
        let k = match k {
            Some(k) => k,
            None => {
                flip = false;
                0
            }
        };
        let e = left.remove(k);
        seq.push(if flip { rev(&e) } else { e });
    }
    seq.iter()
        .map(|s| {
            let p = s.start();
            Vertex::new(p.x, p.y, if matches!(s, Seg::A { .. }) { s.bulge() } else { 0.0 })
        })
        .collect()
}

// ---------------------------------------------------------------- entity conversion

/// Extrusion (0,0,-1): the OCS x axis is mirrored.
fn mirror_x(e: &mut Entity) {
    let m = |p: Pt| pt(-p.x, p.y);
    match &mut e.kind {
        Kind::Circle { c, .. } => *c = m(*c),
        Kind::Arc { c, a0, a1, .. } => {
            *c = m(*c);
            let (n0, n1) = (PI - *a1, PI - *a0);
            *a0 = norm_angle(n0);
            *a1 = norm_angle(n1);
        }
        Kind::Pline { v, .. } => {
            for p in v.iter_mut() {
                *p = Vertex::new(-p.x, p.y, -p.b);
            }
        }
        Kind::Text(t) => t.p = m(t.p),
        _ => {}
    }
}

fn jh_of(code: i64) -> HJust {
    match code {
        1 | 4 => HJust::Center,
        2 => HJust::Right,
        _ => HJust::Left,
    }
}

fn attach_of(n: i64) -> (HJust, VJust) {
    let n = if (1..=9).contains(&n) { n } else { 1 };
    let col = [HJust::Left, HJust::Center, HJust::Right][((n - 1) % 3) as usize];
    let row = [VJust::Top, VJust::Middle, VJust::Bottom][((n - 1) / 3) as usize];
    (col, row)
}

struct Ctx {
    blocks: HashMap<String, BlockRaw>,
    skipped: std::collections::BTreeMap<String, usize>,
}

struct BlockRaw {
    name: String,
    base: Pt,
    entities: Vec<Rec>,
}

fn convert(r: &Rec, ctx: &mut Ctx) -> Vec<Entity> {
    let layer = decode_text(r.first(8).unwrap_or("0"));
    let color = color_of(r);
    let lt = ltype_of(r.first(6));
    let lw = lw_of(r);
    let flip = r.num(230, 1.0) < 0.0;
    let mut out: Vec<Entity> = vec![];
    let push = |mut e: Entity, orient: bool, out: &mut Vec<Entity>| {
        e.layer = layer.clone();
        e.color = color;
        e.ltype = lt.clone();
        e.lw = lw;
        if flip && orient {
            mirror_x(&mut e);
        }
        out.push(e);
    };
    match r.ty.as_str() {
        "LINE" => push(Entity::line(r.pt(0), r.pt(1)), false, &mut out),
        "CIRCLE" => {
            let rr = r.num(40, 0.0);
            if rr > 0.0 {
                push(Entity::circle(r.pt(0), rr), true, &mut out);
            }
        }
        "ARC" => {
            let rr = r.num(40, 0.0);
            if rr > 0.0 {
                push(Entity::arc(r.pt(0), rr, rad(r.num(50, 0.0)), rad(r.num(51, 0.0))), true, &mut out);
            }
        }
        "POINT" => push(Entity::point(r.pt(0)), false, &mut out),
        "LWPOLYLINE" => {
            let mut verts: Vec<Vertex> = vec![];
            for (c, v) in &r.g {
                match *c {
                    10 => verts.push(Vertex::new(parse_f(v), 0.0, 0.0)),
                    20 => {
                        if let Some(l) = verts.last_mut() {
                            l.y = parse_f(v);
                        }
                    }
                    42 => {
                        if let Some(l) = verts.last_mut() {
                            l.b = parse_f(v);
                        }
                    }
                    _ => {}
                }
            }
            if verts.len() > 1 {
                push(Entity::pline(verts, r.int(70, 0) & 1 == 1), true, &mut out);
            }
        }
        "POLYLINE" => {
            let flags = r.int(70, 0);
            if flags & (16 | 64) == 0 {
                let verts: Vec<Vertex> = r.vertices.iter().filter(|v| v.int(70, 0) & 16 == 0).map(|v| Vertex::new(v.num(10, 0.0), v.num(20, 0.0), v.num(42, 0.0))).collect();
                if verts.len() > 1 {
                    push(Entity::pline(verts, flags & 1 == 1), true, &mut out);
                }
            }
        }
        "ELLIPSE" => {
            let m = r.pt(1);
            let ratio = r.num(40, 1.0);
            if len(m) > 0.0 && ratio > 0.0 {
                push(Entity::ellipse(r.pt(0), m, ratio.min(1.0), r.num(41, 0.0), r.num(42, TAU)), false, &mut out);
            }
        }
        "SPLINE" => {
            let flags = r.int(70, 0);
            let deg = r.int(71, 3).max(1) as usize;
            let fit = r.pts(11, 21);
            let ctrl = r.pts(10, 20);
            let knots = r.all_num(40);
            let weights = r.all_num(41);
            let pts: Option<Vec<Pt>> = if fit.len() >= 2 {
                Some(fit.clone())
            } else if ctrl.len() > deg && knots.len() == ctrl.len() + deg + 1 {
                Some(bspline_sample(deg, &knots, &ctrl, if weights.len() == ctrl.len() { Some(&weights) } else { None }, 12))
            } else if ctrl.len() >= 2 {
                Some(ctrl.clone())
            } else {
                None
            };
            if let Some(mut pts) = pts {
                if pts.len() >= 2 {
                    let closed = flags & 1 == 1 && fit.len() >= 2;
                    if !closed && pts.len() > 2 && dist(pts[0], *pts.last().unwrap()) < 1e-9 && fit.len() < 2 {
                        pts.pop();
                        push(Entity::spline(pts, true), false, &mut out);
                    } else {
                        push(Entity::spline(pts, closed), false, &mut out);
                    }
                }
            }
        }
        "TEXT" | "ATTDEF" => {
            if let Some(mut t) = text_of_rec(r) {
                if r.ty == "ATTDEF" {
                    let f = r.int(70, 0);
                    t.att = Some(AttDef { tag: decode_text(r.first(2).unwrap_or("")), prompt: decode_text(r.first(3).unwrap_or("")), invisible: f & 1 != 0, constant: f & 2 != 0, verify: f & 4 != 0, preset: f & 8 != 0 });
                }
                push(Entity::new(Kind::Text(t)), true, &mut out);
            }
        }
        "MTEXT" => {
            let mut raw = r.all(3).join("");
            raw.push_str(r.first(1).unwrap_or(""));
            let s = decode_mtext(&raw);
            let hh = r.num(40, 2.5);
            if !s.is_empty() {
                let (jh, jv) = attach_of(r.int(71, 1));
                let rot = if r.first(11).is_some() { r.num(21, 0.0).atan2(r.num(11, 0.0)) } else { rad(r.num(50, 0.0)) };
                let mut e = Entity::mtext(r.pt(0), if hh > 0.0 { hh } else { 2.5 }, &s, r.num(41, 0.0));
                if let Kind::Text(t) = &mut e.kind {
                    t.rot = rot;
                    t.jh = jh;
                    t.jv = jv;
                }
                push(e, false, &mut out);
            }
        }
        "INSERT" => {
            let name = decode_text(r.first(2).unwrap_or(""));
            if !name.is_empty() {
                let mut e = Entity::insert(&name, r.pt(0));
                if let Kind::Insert(i) = &mut e.kind {
                    let sx = r.num(41, 1.0);
                    let sy = r.num(42, 1.0);
                    i.sx = if sx == 0.0 { 1.0 } else { sx };
                    i.sy = if sy == 0.0 { 1.0 } else { sy };
                    i.rot = rad(r.num(50, 0.0));
                    for a in &r.vertices {
                        if let Some(t) = text_of_rec(a) {
                            i.attribs.push(AttribData { tag: decode_text(a.first(2).unwrap_or("")), t, hidden: a.int(70, 0) & 1 != 0 });
                        }
                    }
                }
                push(e, false, &mut out);
            }
        }
        "XLINE" => {
            let d = r.pt(1);
            if len(d) > 0.0 {
                push(Entity::xline(r.pt(0), d), false, &mut out);
            }
        }
        "RAY" => {
            let d = r.pt(1);
            if len(d) > 0.0 {
                push(Entity::ray(r.pt(0), d), false, &mut out);
            }
        }
        "LEADER" => {
            let pts = r.pts(10, 20);
            if pts.len() >= 2 {
                let h = r.num(40, 2.5);
                push(Entity::leader(pts, "", if h == 0.0 { 2.5 } else { h }), false, &mut out);
            }
        }
        "HATCH" => {
            let loops = hatch_loops(r);
            if !loops.is_empty() {
                let name = r.first(2).unwrap_or("SOLID").to_uppercase();
                let solid = r.int(70, 0) == 1 || name == "SOLID";
                let pat = if solid {
                    "SOLID".to_string()
                } else if pattern(&name).is_some() {
                    name
                } else {
                    "ANSI31".to_string()
                };
                let mut e = Entity::hatch(loops, &pat);
                if let Kind::Hatch(h) = &mut e.kind {
                    let ps = r.num(41, 1.0);
                    h.ps = if ps == 0.0 { 1.0 } else { ps };
                    h.pa = rad(r.num(52, 0.0));
                }
                push(e, false, &mut out);
            }
        }
        "SOLID" | "TRACE" => {
            let p = [r.pt(0), r.pt(1), r.pt(3), r.pt(2)];
            let mut uniq: Vec<Pt> = vec![];
            for (i, q) in p.iter().enumerate() {
                if i == 0 || dist(*q, p[i - 1]) > 1e-12 {
                    uniq.push(*q);
                }
            }
            if uniq.len() >= 3 {
                let loop_: Vec<Vertex> = uniq.iter().map(|q| Vertex::new(q.x, q.y, 0.0)).collect();
                push(Entity::hatch(vec![loop_], "SOLID"), false, &mut out);
            }
        }
        "DIMENSION" => {
            let bname = r.first(2).unwrap_or("").to_string();
            let recs: Vec<Rec> = ctx.blocks.get(&bname).map(|b| b.entities.clone()).unwrap_or_default();
            for be in &recs {
                for mut e in convert(be, ctx) {
                    if matches!(e.kind, Kind::Point { .. }) {
                        continue; // definition points
                    }
                    e.layer = layer.clone(); // the dimension's layer, colour and linetype apply to ByBlock content
                    if e.color == Some(Color::ByBlock) || e.color.is_none() {
                        e.color = color;
                    }
                    if e.ltype.as_deref() == Some("BYBLOCK") {
                        e.ltype = lt.clone();
                    }
                    out.push(e);
                }
            }
        }
        other => {
            *ctx.skipped.entry(other.to_string()).or_insert(0) += 1;
        }
    }
    out
}

/// TEXT / ATTDEF / ATTRIB record to text data.
fn text_of_rec(r: &Rec) -> Option<TextData> {
    let hh = r.num(40, 0.0);
    let s = decode_text(r.first(1).unwrap_or(""));
    if !(hh > 0.0) || (s.is_empty() && r.ty == "TEXT") {
        return None;
    }
    let jh = r.int(72, 0);
    let jv = r.int(74, 0).max(r.int(73, 0));
    let anchor = if (jh != 0 || jv != 0) && r.first(11).is_some() { r.pt(1) } else { r.pt(0) };
    let vj = if jh == 4 {
        VJust::Middle
    } else {
        match jv {
            1 => VJust::Bottom,
            2 => VJust::Middle,
            3 => VJust::Top,
            _ => VJust::Baseline,
        }
    };
    Some(TextData::new(anchor, hh, &s).with_rot(rad(r.num(50, 0.0))).with_just(jh_of(jh), vj))
}

/// Merge POLYLINE/VERTEX/SEQEND and INSERT/ATTRIB/SEQEND runs into single records.
fn group_entities(list: &[Rec]) -> Vec<Rec> {
    let mut out: Vec<Rec> = vec![];
    let mut in_poly = false;
    let mut in_ins = false;
    for r in list {
        match r.ty.as_str() {
            "POLYLINE" => {
                out.push(Rec { vertices: vec![], ..r.clone() });
                in_poly = true;
                in_ins = false;
            }
            "VERTEX" if in_poly => out.last_mut().unwrap().vertices.push(r.clone()),
            "INSERT" => {
                out.push(Rec { vertices: vec![], ..r.clone() });
                in_poly = false;
                in_ins = r.int(66, 0) == 1;
            }
            "ATTRIB" if in_ins => out.last_mut().unwrap().vertices.push(r.clone()),
            "SEQEND" => {
                in_poly = false;
                in_ins = false;
            }
            "ATTRIB" => {}
            _ => {
                in_poly = false;
                in_ins = false;
                out.push(r.clone());
            }
        }
    }
    out
}

// ---------------------------------------------------------------- entry point

/// Parse DXF text into a drawing.
pub fn read_dxf(text: &str, name: &str) -> Result<Drawing, String> {
    let head: String = text.chars().take(2000).collect();
    if !head.contains("SECTION") {
        return Err("This does not look like an ASCII DXF file.".into());
    }
    let tokens = tokenize(text);
    let secs = sections(records(&tokens));
    let header = parse_header(&tokens);
    let mut doc = Drawing::new(name);

    // ---- header
    let hv = |k: &str, code: i32, d: Option<f64>| -> Option<f64> { header.get(k).and_then(|v| v.iter().find(|(c, _)| *c == code)).map(|(_, v)| parse_f(v)).or(d) };
    let units = hv("$INSUNITS", 70, Some(0.0)).unwrap_or(0.0) as i32;
    doc.settings.units.ins_units = if [0, 1, 2, 4, 5, 6].contains(&units) { units } else { 0 };
    doc.settings.units.precision = (hv("$LUPREC", 70, Some(4.0)).unwrap_or(4.0) as i32).clamp(0, 8);
    let lts = hv("$LTSCALE", 40, Some(1.0)).unwrap_or(1.0);
    doc.settings.ltscale = if lts == 0.0 || lts.is_nan() { 1.0 } else { lts };
    doc.settings.pdmode = hv("$PDMODE", 70, Some(0.0)).unwrap_or(0.0) as i32;
    doc.settings.pdsize = hv("$PDSIZE", 40, Some(0.0)).unwrap_or(0.0);
    if let Some(th) = hv("$TEXTSIZE", 40, Some(2.5)) {
        if th > 0.0 {
            doc.settings.text_height = th;
        }
    }
    {
        let ds = doc.settings.dim_styles.get_mut("Standard").expect("standard dim style");
        for (k, f) in [("$DIMTXT", 0), ("$DIMASZ", 1), ("$DIMEXO", 2), ("$DIMEXE", 3), ("$DIMGAP", 4), ("$DIMCEN", 5), ("$DIMSCALE", 6)] {
            if let Some(v) = hv(k, 40, None) {
                if v > 0.0 {
                    match f {
                        0 => ds.txt = v,
                        1 => ds.asz = v,
                        2 => ds.exo = v,
                        3 => ds.exe = v,
                        4 => ds.gap = v,
                        5 => ds.cen = v,
                        _ => ds.scale = v,
                    }
                }
            }
        }
        if let Some(dd) = hv("$DIMDEC", 70, None) {
            if (0.0..=8.0).contains(&dd) {
                ds.dec = dd as i32;
            }
        }
    }

    // ---- layers
    doc.layers.clear();
    let empty: Vec<Rec> = vec![];
    for r in secs.get("TABLES").unwrap_or(&empty).iter().filter(|x| x.ty == "LAYER") {
        let lname = decode_text(r.first(2).unwrap_or(""));
        if lname.is_empty() || doc.layer(&lname).is_some() {
            continue;
        }
        let c62 = r.int(62, 7);
        let flags = r.int(70, 0);
        let color = match r.first(420) {
            Some(tc) => {
                let n = tc.trim().parse::<i64>().unwrap_or(0) & 0xFFFFFF;
                Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)
            }
            None => Color::Aci(if c62 == 0 { 7 } else { c62.unsigned_abs().min(255) as u8 }),
        };
        let lt = ltype_key(r.first(6).unwrap_or("Continuous")).unwrap_or("Continuous");
        let lwv = r.int(370, 25);
        let mut l = Layer::new(&lname).with_color(color).with_ltype(lt).with_lw(if lwv >= 0 { lwv as i32 } else { 25 });
        l.on = c62 >= 0;
        l.frozen = flags & 1 != 0;
        l.locked = flags & 4 != 0;
        l.plot = r.first(290) != Some("0");
        doc.layers.push(l);
    }
    if doc.layer("0").is_none() {
        doc.layers.insert(0, Layer::new("0"));
    }

    // ---- blocks (including anonymous dimension blocks, used for DIMENSION import)
    let mut ctx = Ctx { blocks: HashMap::new(), skipped: Default::default() };
    let mut order: Vec<String> = vec![];
    {
        let mut cur: Option<BlockRaw> = None;
        let mut raw: Vec<Rec> = vec![];
        for r in secs.get("BLOCKS").unwrap_or(&empty) {
            match r.ty.as_str() {
                "BLOCK" => {
                    cur = Some(BlockRaw { name: decode_text(r.first(2).unwrap_or("")), base: r.pt(0), entities: vec![] });
                    raw.clear();
                }
                "ENDBLK" => {
                    if let Some(mut b) = cur.take() {
                        b.entities = group_entities(&raw);
                        order.push(b.name.clone());
                        ctx.blocks.insert(b.name.clone(), b);
                    }
                }
                _ => {
                    if cur.is_some() {
                        raw.push(r.clone());
                    }
                }
            }
        }
    }
    for n in order.iter().filter(|n| !n.starts_with('*')) {
        let (base, recs) = {
            let b = &ctx.blocks[n];
            (b.base, b.entities.clone())
        };
        let mut ents = vec![];
        for r in &recs {
            ents.extend(convert(r, &mut ctx));
        }
        doc.define_block(n, base, ents);
    }

    // ---- model space
    let mut paper = 0;
    for r in group_entities(secs.get("ENTITIES").unwrap_or(&empty)) {
        if r.first(67) == Some("1") {
            paper += 1;
            continue;
        }
        for e in convert(&r, &mut ctx) {
            doc.add_to(e, "model");
        }
    }

    // layers referenced by entities but missing from the table
    let missing: Vec<String> = {
        let mut m: Vec<String> = vec![];
        for e in doc.all_entities() {
            if !e.layer.is_empty() && doc.layer(&e.layer).is_none() && !m.contains(&e.layer) {
                m.push(e.layer.clone());
            }
        }
        m
    };
    for n in missing {
        doc.layers.push(Layer::new(&n));
    }
    let cl = decode_text(header.get("$CLAYER").and_then(|v| v.iter().find(|(c, _)| *c == 8)).map(|(_, v)| v.as_str()).unwrap_or("0"));
    doc.cur.layer = doc.layer(&cl).map(|l| l.name.clone()).unwrap_or_else(|| doc.layers[0].name.clone());

    doc.skipped = ctx.skipped;
    doc.paper_skipped = paper;
    doc.validate()?;
    doc.dirty = false;
    Ok(doc)
}
