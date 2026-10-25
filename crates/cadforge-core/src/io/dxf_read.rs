//! DXF reader: tolerant ASCII parser for the entities CadForge understands (port of `src/io/dxf-read.js`).
//!
//! Supported: LINE CIRCLE ARC LWPOLYLINE POLYLINE ELLIPSE SPLINE POINT TEXT MTEXT INSERT HATCH SOLID/TRACE XLINE RAY
//! LEADER and DIMENSION (imported as the geometry of its anonymous block, so it looks exactly like the source drawing).
use crate::drawing::{Drawing, Layout, Paper};
use crate::entities::*;
use crate::geom::*;
use crate::hatch::pattern;
use crate::style::*;
use crate::text::{AttDef, HJust, TextData, VJust};
use std::collections::HashMap;

// ---------------------------------------------------------------- text decoding

/// DXF writes control characters as a caret and a letter (`^J` is a line feed) and a literal caret as `^ `.
fn uncaret(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains('^') {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '^' {
            match it.peek().copied() {
                Some(' ') => {
                    out.push('^');
                    it.next();
                }
                Some('J') => {
                    out.push('\n');
                    it.next();
                }
                Some('I') => {
                    out.push(' ');
                    it.next();
                }
                Some(n) if ('@'..='_').contains(&n) => {
                    it.next(); // other control characters are dropped
                }
                _ => out.push('^'),
            }
        } else {
            out.push(c);
        }
    }
    std::borrow::Cow::Owned(out)
}

/// Decode `%%c`, `%%d`, `%%p` and `\U+XXXX` escapes in TEXT / names.
pub fn decode_text(s: &str) -> String {
    let s = &*uncaret(s);
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

/// MTEXT contents with their formatting codes kept (text layout interprets them) and `\n` for paragraph breaks.
pub fn keep_mtext(src: &str) -> String {
    let src = &*uncaret(src);
    let s: Vec<char> = decode_text(src).chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i] == '\\' && i + 1 < s.len() {
            match s[i + 1] {
                'P' | 'p' => out.push('\n'),
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
            i += 2;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out.trim_end_matches('\n').to_string()
}

/// Strip MTEXT formatting codes, returning plain text with `\n` line breaks.
pub fn decode_mtext(src: &str) -> String {
    let src = &*uncaret(src);
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
        if let Ok(code) = norm[i].trim().parse::<i32>() {
            // text content keeps its spaces: long strings are split into 250-character chunks at any character
            let v = if matches!(code, 1 | 3) { norm[i + 1] } else { norm[i + 1].trim() };
            out.push((code, v.to_string()));
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
    // an MTEXT's "Embedded Object" (2018+ column data) repeats groups 10/11/40... with other meanings
    let mut embedded = false;
    for t in tokens {
        if t.0 == 0 {
            recs.push(Rec { ty: t.1.clone(), g: vec![], vertices: vec![] });
            embedded = false;
        } else if let Some(r) = recs.last_mut() {
            embedded |= t.0 == 101 && r.ty == "MTEXT";
            if !embedded {
                r.g.push(t.clone());
            }
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
            num(&mut i, 73); // closed flag (the order is 72, 73, 93)
            let nv = (num(&mut i, 93) as usize).min(cap);
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
    /// IMAGEDEF objects by handle: (path, width px, height px).
    image_defs: HashMap<String, (String, u32, u32)>,
    /// Picture names given to IMAGEDEF handles and the pictures themselves (their files are not part of a DXF).
    image_keys: HashMap<String, String>,
    images: indexmap::IndexMap<String, crate::picture::ImageDef>,
    /// Layer names by their LAYER table handle (viewports list their frozen layers by handle).
    layer_names: HashMap<String, String>,
    /// The group of each grouped entity, by the entity's handle (upper case).
    groups: HashMap<String, String>,
    /// Converting model space (polyface meshes there become 3D solids).
    model: bool,
    /// Polyface meshes read as solids, with the CadForge solid id a part of a split solid carries.
    solids: Vec<(Option<u64>, crate::solid::Solid)>,
}

struct BlockRaw {
    name: String,
    base: Pt,
    entities: Vec<Rec>,
}

fn convert(r: &Rec, ctx: &mut Ctx) -> Vec<Entity> {
    let mut out = convert_kind(r, ctx);
    if let Some(g) = r.first(5).and_then(|h| ctx.groups.get(&h.to_uppercase())) {
        for e in &mut out {
            e.grp = Some(g.clone());
        }
    }
    out
}

fn convert_kind(r: &Rec, ctx: &mut Ctx) -> Vec<Entity> {
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
            // group 43 is the width of every edge that has no widths of its own
            let constant = r.num(43, 0.0);
            let mut own: Vec<bool> = vec![];
            for (c, v) in &r.g {
                match *c {
                    10 => {
                        verts.push(Vertex::wide(parse_f(v), 0.0, 0.0, constant, constant));
                        own.push(false);
                    }
                    20 => {
                        if let Some(l) = verts.last_mut() {
                            l.y = parse_f(v);
                        }
                    }
                    40 | 41 => {
                        if let (Some(l), Some(o)) = (verts.last_mut(), own.last_mut()) {
                            if !*o {
                                // the first width given for this vertex replaces the constant; a missing one is 0
                                l.sw = 0.0;
                                l.ew = 0.0;
                                *o = true;
                            }
                            if *c == 40 {
                                l.sw = parse_f(v);
                            } else {
                                l.ew = parse_f(v);
                            }
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
            if flags & 64 != 0 {
                polyface_solid(r, ctx, &layer, color);
            } else if flags & 16 != 0 {
                *ctx.skipped.entry("POLYGON MESH".into()).or_insert(0) += 1;
            }
            if flags & (16 | 64) == 0 {
                // the header carries the default start / end widths of every vertex
                let (dsw, dew) = (r.num(40, 0.0), r.num(41, 0.0));
                let verts: Vec<Vertex> = r.vertices.iter().filter(|v| v.int(70, 0) & 16 == 0).map(|v| Vertex::wide(v.num(10, 0.0), v.num(20, 0.0), v.num(42, 0.0), v.num(40, dsw), v.num(41, dew))).collect();
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
        "SPLINE" | "HELIX" => {
            let flags = r.int(70, 0);
            let deg = r.int(71, 3).max(1) as usize;
            let fit = r.pts(11, 21);
            let ctrl = r.pts(10, 20);
            let knots = r.all_num(40);
            let weights = r.all_num(41);
            if fit.len() < 2 {
                if let Some((fit, closed)) = bezier_chain_fit(deg, &knots, &ctrl, &weights) {
                    push(Entity::spline(fit, closed), false, &mut out);
                    return out;
                }
            }
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
            let s = keep_mtext(&raw);
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
            if let Some(k) = cadforge_wrapped(r) {
                push(Entity::new(k), false, &mut out);
                return out;
            }
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
                push(Entity::leader(pts, r.first(LEADER_TEXT).unwrap_or(""), if h == 0.0 { 2.5 } else { h }), false, &mut out);
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
                    if r.int(450, 0) == 1 {
                        // gradient fill: two (or one tinted) true colours, the pattern name and the rotation
                        let cols: Vec<[u8; 3]> = r.all(421).iter().map(|v| {
                            let n = v.trim().parse::<i64>().unwrap_or(0) & 0xFFFFFF;
                            [(n >> 16) as u8, (n >> 8) as u8, n as u8]
                        }).collect();
                        let one = r.int(452, 0) == 1 || cols.len() < 2;
                        let c1 = cols.first().copied().unwrap_or([255, 255, 255]);
                        h.gradient = Some(GradientData {
                            c1,
                            c2: cols.get(1).copied().unwrap_or([255, 255, 255]),
                            angle: r.num(460, 0.0),
                            kind: r.first(470).map(|k| k.to_uppercase()).unwrap_or_else(|| "LINEAR".into()),
                            one_color: one,
                            tint: r.num(462, 0.5),
                        });
                    }
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
        "3DFACE" => {
            // a planar face: draw its outline in the XY plane
            let mut pts: Vec<Pt> = vec![r.pt(0), r.pt(1), r.pt(2)];
            if r.first(13).is_some() {
                pts.push(r.pt(3));
            }
            let mut uniq: Vec<Pt> = vec![];
            for q in pts {
                if uniq.last().map_or(true, |l| dist(*l, q) > 1e-12) {
                    uniq.push(q);
                }
            }
            if uniq.len() > 1 && dist(uniq[0], *uniq.last().expect("point")) < 1e-12 {
                uniq.pop();
            }
            if uniq.len() >= 3 {
                push(Entity::pline(uniq.iter().map(|q| Vertex::new(q.x, q.y, 0.0)).collect(), true), false, &mut out);
            }
        }
        "MLINE" => {
            // the STANDARD style's two elements; the vertices come as repeated group 11/21 points
            let verts: Vec<Pt> = r.pts(11, 21);
            let scale = r.num(40, 1.0);
            let (lo, hi) = match r.int(70, 1) {
                0 => (-scale, 0.0),
                2 => (0.0, scale),
                _ => (-scale / 2.0, scale / 2.0),
            };
            let closed = r.int(71, 0) & 2 != 0;
            if verts.len() >= 2 {
                let grp = format!("*M{}", ctx.skipped.len() + verts.len());
                for off in [lo, hi] {
                    let pts = offset_polyline(&verts, off, closed);
                    let mut e = Entity::pline(pts.iter().map(|q| Vertex::new(q.x, q.y, 0.0)).collect(), closed);
                    e.grp = Some(grp.clone());
                    push(e, false, &mut out);
                }
            }
        }
        "MULTILEADER" => {
            // the context data holds the text, then one LEADER{} per leader (landing point, dogleg direction and
            // length) holding one LEADER_LINE{} per line (its own vertices, usually just the arrow point)
            struct Root {
                landing: Option<Pt>,
                dir: Option<Pt>,
                dogleg: f64,
                lines: Vec<Vec<Pt>>,
            }
            let mut roots: Vec<Root> = vec![];
            let mut text = String::new();
            let mut text_h = 0.0;
            let mut in_line = false;
            let mut x: Option<f64> = None;
            let mut section = String::new();
            for (c, v) in &r.g {
                let root = roots.last_mut();
                match (*c, v.as_str(), root) {
                    (300, "CONTEXT_DATA{", _) => section = "ctx".into(),
                    (302, "LEADER{", _) => {
                        section = "leader".into();
                        roots.push(Root { landing: None, dir: None, dogleg: 0.0, lines: vec![] });
                    }
                    (303, "}", _) => section = "ctx".into(),
                    (304, "LEADER_LINE{", Some(root)) => {
                        in_line = true;
                        root.lines.push(vec![]);
                    }
                    (305, "}", _) => in_line = false,
                    (304, t, _) if section == "ctx" && !in_line => text = decode_mtext(t),
                    (41, t, _) if section == "ctx" && text_h == 0.0 => text_h = parse_f(t),
                    (10, t, _) if in_line || section == "leader" => x = Some(parse_f(t)),
                    (20, t, Some(root)) if in_line => {
                        if let (Some(px), Some(l)) = (x.take(), root.lines.last_mut()) {
                            l.push(pt(px, parse_f(t)));
                        }
                    }
                    (20, t, Some(root)) if section == "leader" && root.landing.is_none() => root.landing = x.take().map(|px| pt(px, parse_f(t))),
                    (11, t, _) if section == "leader" => x = Some(parse_f(t)),
                    (21, t, Some(root)) if section == "leader" && root.dir.is_none() => root.dir = x.take().map(|px| pt(px, parse_f(t))),
                    (40, t, Some(root)) if section == "leader" && !in_line && root.dogleg == 0.0 => root.dogleg = parse_f(t),
                    _ => {}
                }
            }
            let mut made = 0;
            for root in &roots {
                for (k, line) in root.lines.iter().enumerate() {
                    let mut line = line.clone();
                    if let (false, Some(l)) = (line.is_empty(), root.landing) {
                        if dist(*line.last().expect("point"), l) > 1e-9 {
                            line.push(l);
                        }
                        // the dogleg is drawn once per leader
                        if let (0, Some(d)) = (k, root.dir) {
                            if root.dogleg > 0.0 {
                                line.push(pt(l.x + d.x * root.dogleg, l.y + d.y * root.dogleg));
                            }
                        }
                    }
                    if line.len() >= 2 {
                        let t = if made == 0 { text.as_str() } else { "" };
                        push(Entity::leader(line, t, if text_h > 0.0 { text_h } else { 2.5 }), false, &mut out);
                        made += 1;
                    }
                }
            }
            if made == 0 {
                *ctx.skipped.entry("MULTILEADER".into()).or_insert(0) += 1;
            }
        }
        "WIPEOUT" => {
            // clip boundary in image pixel space: x from -0.5 to the pixel width, mapped with the u / v vectors (per pixel)
            let (o, u, v) = (r.pt(0), r.pt(1), r.pt(2));
            let raw: Vec<Pt> = r.pts(14, 24);
            let size = (r.num(13, 1.0), r.num(23, 1.0));
            let pts: Vec<Pt> = if r.int(71, 1) == 1 || raw.len() < 3 {
                vec![pt(-0.5, -0.5), pt(size.0 - 0.5, -0.5), pt(size.0 - 0.5, size.1 - 0.5), pt(-0.5, size.1 - 0.5)]
            } else {
                raw
            };
            // pixel y runs down from the top edge, as for images
            let world: Vec<Pt> = pts.iter().map(|q| (q.x + 0.5, size.1 - q.y - 0.5)).map(|(s, t)| pt(o.x + u.x * s + v.x * t, o.y + u.y * s + v.y * t)).collect();
            let n = world.len();
            let world = if n > 3 && dist(world[0], world[n - 1]) < 1e-9 { world[..n - 1].to_vec() } else { world };
            if world.len() >= 3 {
                push(Entity::new(Kind::Wipeout(WipeoutData { pts: world })), false, &mut out);
            }
        }
        "VIEWPORT" => {
            // id 1 is the sheet's own viewport, not a window onto model space
            let (w, h) = (r.num(40, 0.0), r.num(41, 0.0));
            if r.int(69, 0) != 1 && w > 0.0 && h > 0.0 {
                let mut vp = ViewportData::new(r.pt(0), w, h);
                let vh = r.num(45, 0.0);
                if vh > 0.0 {
                    vp.scale = Some(h / vh);
                }
                vp.vc = pt(r.num(17, 0.0) + r.num(12, 0.0), r.num(27, 0.0) + r.num(22, 0.0));
                vp.off = r.int(68, 1) == 0;
                vp.locked = r.int(90, 0) & 16384 != 0;
                for h in r.all(331) {
                    if let Some(name) = ctx.layer_names.get(&h.trim().to_uppercase()) {
                        vp.set_frozen(name, true);
                    }
                }
                push(Entity::new(Kind::Viewport(vp)), false, &mut out);
            }
        }
        "IMAGE" => {
            let (up, vp) = (r.pt(1), r.pt(2));
            let (mut wpx, mut hpx) = (r.num(13, 0.0), r.num(23, 0.0));
            let def = r.first(340).and_then(|h| ctx.image_defs.get(h)).cloned();
            if let Some((_, w, h)) = &def {
                if wpx <= 0.0 {
                    wpx = *w as f64;
                }
                if hpx <= 0.0 {
                    hpx = *h as f64;
                }
            }
            if wpx >= 1.0 && hpx >= 1.0 && wpx <= 1e6 && hpx <= 1e6 && (up.x != 0.0 || up.y != 0.0) && (vp.x != 0.0 || vp.y != 0.0) {
                let (path, hnd) = (def.map_or_else(|| "image".to_string(), |d| d.0), r.first(340).unwrap_or("").to_string());
                let key = match ctx.image_keys.get(&hnd) {
                    Some(k) if !hnd.is_empty() => k.clone(),
                    _ => {
                        let k = crate::picture::unique_key(&path, |k| ctx.images.contains_key(k));
                        ctx.images.insert(k.clone(), crate::picture::ImageDef::missing(&path, wpx as u32, hpx as u32));
                        ctx.image_keys.insert(hnd, k.clone());
                        k
                    }
                };
                let mut im = ImageData { name: key, p: r.pt(0), u: up * wpx, v: vp * hpx, clip: vec![], clip_off: false, brightness: r.int(281, 50).clamp(0, 100) as i32, contrast: r.int(282, 50).clamp(0, 100) as i32, fade: r.int(283, 0).clamp(0, 100) as i32 };
                // boundary vertices are in pixels from the top-left corner of the picture, y down
                let to_world = |x: f64, y: f64| im.p + up * (x + 0.5) + vp * (hpx - y - 0.5);
                let (xs, ys) = (r.all_num(14), r.all_num(24));
                let verts: Vec<Pt> = xs.iter().zip(&ys).map(|(x, y)| to_world(*x, *y)).collect();
                let active = r.int(280, 0) != 0;
                let poly = match (r.int(71, 1), verts.len()) {
                    (1, 2) if active => {
                        let (a, b) = (pt(xs[0], ys[0]), pt(xs[1], ys[1]));
                        vec![to_world(a.x, a.y), to_world(b.x, a.y), to_world(b.x, b.y), to_world(a.x, b.y)]
                    }
                    (2, n) if n >= 3 => {
                        let mut v = verts.clone();
                        if v.first() == v.last() {
                            v.pop();
                        }
                        v
                    }
                    _ => vec![],
                };
                if poly.len() >= 3 {
                    im.clip = poly;
                    im.clip_off = !active;
                }
                push(Entity::new(Kind::Image(im)), false, &mut out);
            }
        }
        "DIMENSION" => {
            if let Some(d) = cadforge_dim(r) {
                push(Entity::new(Kind::Dim(d)), false, &mut out);
                return out;
            }
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

/// A polyline offset by `d` to its left with mitred corners (used for multilines).
fn offset_polyline(pts: &[Pt], d: f64, closed: bool) -> Vec<Pt> {
    let n = pts.len();
    let seg_normal = |i: usize, j: usize| {
        let t = unit(pts[j] - pts[i]);
        pt(-t.y, t.x)
    };
    (0..n)
        .map(|i| {
            let prev = if i > 0 { Some(i - 1) } else if closed { Some(n - 1) } else { None };
            let next = if i + 1 < n { Some(i + 1) } else if closed { Some(0) } else { None };
            let n1 = prev.map(|p| seg_normal(p, i));
            let n2 = next.map(|q| seg_normal(i, q));
            let m = match (n1, n2) {
                (Some(a), Some(b)) => {
                    let k = 1.0 + dot(a, b);
                    if k.abs() < 1e-9 {
                        a
                    } else {
                        pt((a.x + b.x) / k, (a.y + b.y) / k)
                    }
                }
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => pt(0.0, 0.0),
            };
            pt(pts[i].x + m.x * d, pts[i].y + m.y * d)
        })
        .collect()
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
    leader_text(out)
}

/// CadForge writes a spline through fit points as the chain of cubic Béziers it draws (knots 0,0,0,0,1,1,1,...,n,n,n,n).
/// When the control points are exactly that chain for the Bézier joints, the joints are the fit points and the
/// spline reads back as it was (anything else, from other programs, is sampled).
fn bezier_chain_fit(deg: usize, knots: &[f64], ctrl: &[Pt], weights: &[f64]) -> Option<(Vec<Pt>, bool)> {
    if deg != 3 || ctrl.len() < 4 || !(ctrl.len() - 1).is_multiple_of(3) || weights.iter().any(|w| *w != 1.0) {
        return None;
    }
    let n = (ctrl.len() - 1) / 3;
    let mut want = vec![0.0; 4];
    for i in 1..n {
        want.extend([i as f64; 3]);
    }
    want.extend([n as f64; 4]);
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0);
    if knots.len() != want.len() || knots.iter().zip(&want).any(|(a, b)| !close(*a, *b)) {
        return None;
    }
    let joints: Vec<Pt> = ctrl.iter().step_by(3).copied().collect();
    let same = |p: Pt, q: Pt| close(p.x, q.x) && close(p.y, q.y);
    let candidates = [(joints.clone(), false), (joints[..n].to_vec(), true)];
    candidates.into_iter().find(|(fit, closed)| {
        let bz = spline_beziers(fit, *closed);
        (!closed || (n >= 3 && same(joints[0], joints[n])))
            && bz.len() == n
            && bz.iter().enumerate().all(|(i, b)| [b.p0, b.c1, b.c2, b.p1].iter().zip(&ctrl[3 * i..3 * i + 4]).all(|(p, q)| same(*p, *q)))
    })
}

/// A polyface mesh in model space as a 3D solid (triangles; four-sided faces are split). CadForge's own solids carry
/// their name and id in CADFORGE extended data, so the parts of a large one are joined again. Meshes elsewhere (in
/// blocks or layouts) are reported as skipped.
fn polyface_solid(r: &Rec, ctx: &mut Ctx, layer: &str, color: Option<Color>) {
    use crate::solid::{Solid, V3};
    if !ctx.model {
        *ctx.skipped.entry("POLYFACE MESH".into()).or_insert(0) += 1;
        return;
    }
    let (mut vertices, mut faces) = (vec![], vec![]);
    for v in &r.vertices {
        let f = v.int(70, 0);
        if f & 64 != 0 {
            vertices.push(V3::new(v.num(10, 0.0), v.num(20, 0.0), v.num(30, 0.0)));
        } else if f & 128 != 0 {
            let idx: Vec<u32> = [71, 72, 73, 74].iter().map(|c| v.int(*c, 0).unsigned_abs() as u32).filter(|i| *i > 0).map(|i| i - 1).collect();
            for k in 1..idx.len().saturating_sub(1) {
                faces.push([idx[0], idx[k], idx[k + 1]]);
            }
        }
    }
    faces.retain(|f: &[u32; 3]| f.iter().all(|i| (*i as usize) < vertices.len()));
    if faces.is_empty() {
        return;
    }
    let tag: Option<serde_json::Value> = serde_json::from_str(&xdata_strings(r, super::dxf_write::CADFORGE_APP).concat()).ok();
    let name = tag.as_ref().and_then(|t| t["solid"].as_str()).unwrap_or("Mesh").to_string();
    let id = tag.as_ref().and_then(|t| t["id"].as_u64());
    ctx.solids.push((id, Solid { id: 0, name, layer: layer.to_string(), color, vertices, faces, program: None }));
}

/// The solids read from polyface meshes, the parts of each CadForge solid joined again (with the CadForge id each had).
fn join_solids(parts: Vec<(Option<u64>, crate::solid::Solid)>) -> Vec<(Option<u64>, crate::solid::Solid)> {
    let mut out: Vec<(Option<u64>, crate::solid::Solid)> = vec![];
    for (id, s) in parts {
        match out.iter_mut().find(|(k, _)| id.is_some() && *k == id) {
            Some((_, t)) => {
                let base = t.vertices.len() as u32;
                t.vertices.extend(s.vertices);
                t.faces.extend(s.faces.iter().map(|f| [f[0] + base, f[1] + base, f[2] + base]));
            }
            None => out.push((id, s)),
        }
    }
    out
}

/// A table or region CadForge wrote as an insert of the block of its geometry: its CADFORGE extended data, when the
/// insert is still where CadForge put it (at the origin, unscaled, unrotated). Otherwise the insert stays an insert.
fn cadforge_wrapped(r: &Rec) -> Option<Kind> {
    if r.ty != "INSERT" {
        return None;
    }
    let json = xdata_strings(r, super::dxf_write::CADFORGE_APP).concat();
    let p = r.pt(0);
    if json.is_empty() || p.x != 0.0 || p.y != 0.0 || r.num(41, 1.0) != 1.0 || r.num(42, 1.0) != 1.0 || r.num(50, 0.0) != 0.0 {
        return None;
    }
    let k: Kind = serde_json::from_str(&json).ok()?;
    matches!(k, Kind::Table(_) | Kind::Region(_)).then_some(k)
}

/// The strings of an entity's extended data for one application (empty when it has none).
fn xdata_strings<'a>(r: &'a Rec, app: &str) -> Vec<&'a str> {
    let mut mine = false;
    let mut out = vec![];
    for (c, v) in &r.g {
        match *c {
            1001 => mine = v.trim().eq_ignore_ascii_case(app),
            1000 if mine => out.push(v.as_str()),
            _ => {}
        }
    }
    out
}

/// A DIMENSION that CadForge wrote, as CadForge's dimension: its CADFORGE extended data, when the dimension's
/// definition points and text still agree with it (a dimension changed by another program is read as drawn).
fn cadforge_dim(r: &Rec) -> Option<crate::dim::DimData> {
    use crate::dim::DimType;
    let json = xdata_strings(r, super::dxf_write::CADFORGE_APP).concat();
    if json.is_empty() {
        return None;
    }
    let mut d: crate::dim::DimData = serde_json::from_str(&json).ok()?;
    // links to other objects come from the CADFORGE XRECORD (by handle); ids in the dimension's own data are stale
    d.assoc.clear();
    let close = |a: Pt, b: Option<Pt>| b.is_some_and(|b| dist(a, b) <= 1e-9 * (1.0 + a.x.abs().max(a.y.abs())));
    let zero = Some(pt(0.0, 0.0));
    let points = close(r.pt(0), Some(super::dxf_write::dim_def_point(&d)))
        && match d.kind {
            DimType::Lin | DimType::Ali => close(r.pt(3), d.p1) && close(r.pt(4), d.p2),
            DimType::Ord => close(r.pt(3), d.pt) && close(r.pt(4), d.dp),
            DimType::Ang | DimType::Arc => close(r.pt(3), d.p1) && close(r.pt(4), d.p2) && close(r.pt(5), d.c.or(zero)),
            DimType::Rad | DimType::Jog | DimType::Dia => close(r.pt(5), d.pt),
        };
    let text = decode_text(r.first(1).unwrap_or("")) == d.txt.clone().unwrap_or_default();
    (points && text).then_some(d)
}

/// The CADFORGE XRECORD of the named object dictionary (see `dxf_write::cadforge_extras`), as JSON.
fn cadforge_extras(objects: &[Rec]) -> Option<serde_json::Value> {
    let app = super::dxf_write::CADFORGE_APP;
    let handle = objects.iter().filter(|r| r.ty == "DICTIONARY").find_map(|d| d.g.windows(2).find(|p| p[0].0 == 3 && p[0].1.trim() == app && p[1].0 == 350).map(|p| p[1].1.trim().to_uppercase()))?;
    let x = objects.iter().find(|r| r.ty == "XRECORD" && r.first(5).is_some_and(|h| h.trim().eq_ignore_ascii_case(&handle)))?;
    serde_json::from_str(&x.all(1).concat()).ok()
}

/// Put back what the CADFORGE XRECORD keeps: named views, the last view, the current and named UCSs, the current
/// colour / linetype / lineweight, the plot style table, which blocks are external references and the data of the
/// pictures the file names.
fn apply_extras(doc: &mut Drawing, x: &serde_json::Value) {
    fn get<T: serde::de::DeserializeOwned>(x: &serde_json::Value, k: &str) -> Option<T> {
        x.get(k).and_then(|v| serde_json::from_value(v.clone()).ok())
    }
    if let Some(v) = get(x, "views") {
        doc.views = v;
    }
    doc.view = get(x, "view").or(doc.view.take());
    if let Some(u) = get(x, "ucs") {
        doc.ucs = u;
    }
    if let Some(u) = get(x, "ucs_named") {
        doc.ucs_named = u;
    }
    if let Some(c) = get::<crate::drawing::Current>(x, "cur") {
        // (the current layer is $CLAYER)
        (doc.cur.color, doc.cur.ltype, doc.cur.lw) = (c.color, c.ltype, c.lw);
    }
    if let Some(p) = get::<crate::plotstyle::PlotStyleTable>(x, "plot_styles") {
        doc.plot_styles = Some(std::rc::Rc::new(p));
    }
    if let Some(s) = get(x, "standards") {
        doc.standards = s;
    }
    let st = &mut doc.settings;
    if let Some(v) = get::<indexmap::IndexMap<String, crate::style::DimStyle>>(x, "dim_styles").filter(|v| v.contains_key("Standard")) {
        st.dim_styles = v;
    }
    if let Some(v) = get(x, "cur_dim_style") {
        st.cur_dim_style = v;
    }
    if let Some(v) = get(x, "mleader_styles") {
        st.mleader_styles = v;
    }
    if let Some(v) = get(x, "cur_mleader_style") {
        st.cur_mleader_style = v;
    }
    if let Some(v) = get(x, "table_styles") {
        st.table_styles = v;
    }
    if let Some(v) = get(x, "cur_table_style") {
        st.cur_table_style = v;
    }
    if let Some(v) = get::<Vec<crate::annotative::AnnoScale>>(x, "scales").filter(|v| !v.is_empty()) {
        st.scales = v;
    }
    if let Some(v) = get(x, "cannoscale") {
        st.cannoscale = v;
    }
    let xrefs: indexmap::IndexMap<String, crate::drawing::XrefInfo> = get(x, "xrefs").unwrap_or_default();
    for (k, info) in xrefs {
        if let Some(b) = doc.blocks.get_mut(&k) {
            b.xref = Some(info);
        }
    }
    let pictures: indexmap::IndexMap<String, crate::picture::ImageDef> = get(x, "images").unwrap_or_default();
    for def in doc.images.values_mut().filter(|d| d.data.is_none()) {
        if let Some(p) = pictures.values().find(|p| p.path == def.path && p.data.is_some()) {
            *def = p.clone();
        }
    }
}

/// GROUP objects: the group name (its entry in the group dictionary) of each member entity, by entity handle.
fn groups_of(objects: &[Rec]) -> HashMap<String, String> {
    let mut names: HashMap<String, String> = HashMap::new();
    for d in objects.iter().filter(|r| r.ty == "DICTIONARY") {
        for p in d.g.windows(2).filter(|p| p[0].0 == 3 && p[1].0 == 350) {
            names.insert(p[1].1.trim().to_uppercase(), decode_text(&p[0].1));
        }
    }
    let mut out = HashMap::new();
    for g in objects.iter().filter(|r| r.ty == "GROUP") {
        let Some(name) = g.first(5).and_then(|h| names.get(&h.trim().to_uppercase())) else { continue };
        for m in g.all(340) {
            out.entry(m.trim().to_uppercase()).or_insert_with(|| name.clone());
        }
    }
    out
}

/// Group code (not used by DXF) carrying a leader's own text, taken from its annotation.
const LEADER_TEXT: i32 = -100;

/// A LEADER's MTEXT annotation (group 340) that sits where CadForge draws a leader's text (just right of the last
/// vertex by 0.4 text heights, middle left, unrotated, the leader's text height) becomes the leader's own text, so
/// a CadForge leader reads back as it was written. Annotations placed anywhere else stay separate texts.
fn leader_text(mut list: Vec<Rec>) -> Vec<Rec> {
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0);
    let mut merged: Vec<(usize, usize, String)> = vec![];
    for (i, r) in list.iter().enumerate().filter(|(_, r)| r.ty == "LEADER") {
        let (Some(h), Some(last)) = (r.first(340), r.pts(10, 20).last().copied()) else { continue };
        let Some(j) = list.iter().position(|m| m.ty == "MTEXT" && m.first(5).is_some_and(|x| x.eq_ignore_ascii_case(h))) else { continue };
        let (m, th) = (&list[j], r.num(40, 2.5));
        let at = m.pt(0);
        if m.int(71, 1) == 4 && close(m.num(40, 0.0), th) && close(at.x, last.x + th * 0.4) && close(at.y, last.y) && m.num(21, 0.0) == 0.0 && m.num(50, 0.0) == 0.0 {
            let mut raw = m.all(3).join("");
            raw.push_str(m.first(1).unwrap_or(""));
            merged.push((i, j, decode_mtext(&raw)));
        }
    }
    let drop: std::collections::HashSet<usize> = merged.iter().map(|(_, j, _)| *j).collect();
    for (i, _, s) in merged {
        list[i].g.push((LEADER_TEXT, s));
    }
    list.into_iter().enumerate().filter(|(k, _)| !drop.contains(k)).map(|(_, r)| r).collect()
}

// ---------------------------------------------------------------- entry point

/// Windows-1252 code points for the bytes 0x80..=0x9F (the rest of the range equals Latin-1).
const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{81}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{8D}', '\u{017D}', '\u{8F}', '\u{90}', '\u{2018}',
    '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{9D}', '\u{017E}', '\u{0178}',
];

/// Windows-1252 text from bytes.
pub(crate) fn cp1252_to_string(b: &[u8]) -> String {
    b.iter()
        .map(|&c| match c {
            0x80..=0x9F => CP1252_HIGH[(c - 0x80) as usize],
            _ => c as char,
        })
        .collect()
}

/// The DXF text in the bytes of a file: binary DXF is unpacked, anything else is UTF-8 (AutoCAD 2007 and later, with or
/// without BOM) or, failing that, the ANSI code page 1252 that older files and AutoCAD LT use.
pub fn dxf_text_from_bytes(bytes: &[u8]) -> Result<String, String> {
    if super::binary_dxf::is_binary_dxf(bytes) {
        return super::binary_dxf::binary_to_text(bytes);
    }
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    Ok(match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => cp1252_to_string(b),
    })
}

/// [`dxf_text_from_bytes`] for callers that cope with a damaged binary file by seeing no drawing in it (the reader then
/// reports that the text is not a DXF file).
pub fn decode_dxf_bytes(bytes: &[u8]) -> String {
    dxf_text_from_bytes(bytes).unwrap_or_default()
}

/// Parse the bytes of a DXF file, text or binary (see [`dxf_text_from_bytes`]).
pub fn read_dxf_bytes(bytes: &[u8], name: &str) -> Result<Drawing, String> {
    read_dxf(&dxf_text_from_bytes(bytes)?, name)
}

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
    doc.base = pt(hv("$INSBASE", 10, Some(0.0)).unwrap_or(0.0), hv("$INSBASE", 20, Some(0.0)).unwrap_or(0.0));
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
    // ---- the DIMSTYLE table: the named dimension styles (the header variables above are those of the current style,
    // which is Standard unless $DIMSTYLE names another)
    {
        let cur = header.get("$DIMSTYLE").and_then(|v| v.iter().find(|(c, _)| *c == 2)).map(|(_, v)| decode_text(v).trim().to_string()).unwrap_or_else(|| "Standard".into());
        let empty: Vec<Rec> = vec![];
        for r in secs.get("TABLES").unwrap_or(&empty).iter().filter(|x| x.ty == "DIMSTYLE") {
            let name = decode_text(r.first(2).unwrap_or("")).trim().to_string();
            if name.is_empty() || name.starts_with('*') || (name.eq_ignore_ascii_case("Standard") && cur.eq_ignore_ascii_case("Standard")) {
                continue;
            }
            let b = crate::style::DimStyle::default();
            let pos = |c: i32, d: f64| Some(r.num(c, d)).filter(|v| *v > 0.0 && v.is_finite()).unwrap_or(d);
            let post = decode_text(r.first(3).unwrap_or(""));
            let (pre, suf) = post.split_once("<>").map_or((String::new(), String::new()), |(a, z)| (a.to_string(), z.to_string()));
            let key = if name.eq_ignore_ascii_case("Standard") { "Standard".to_string() } else { name.clone() };
            let ds = crate::style::DimStyle {
                name: key.clone(),
                scale: pos(40, b.scale),
                asz: pos(41, b.asz),
                exo: r.num(42, b.exo),
                exe: r.num(44, b.exe),
                txt: pos(140, b.txt),
                cen: r.num(141, b.cen),
                lfac: Some(r.num(144, b.lfac)).filter(|v| *v != 0.0 && v.is_finite()).unwrap_or(1.0),
                gap: r.num(147, b.gap),
                dec: r.int(271, b.dec as i64).clamp(0, 8) as i32,
                tad: r.int(77, 1) != 0,
                zin: r.int(78, b.zin as i64) as i32,
                pre,
                suf,
                ..b
            };
            doc.settings.dim_styles.insert(key, ds);
        }
        if let Some(k) = doc.settings.dim_styles.keys().find(|k| k.eq_ignore_ascii_case(&cur)).cloned() {
            doc.settings.cur_dim_style = k;
        }
    }

    // ---- linetypes the file defines (simple dash / gap / dot ones; shapes and text are left out), so layers and
    // objects that use them keep their look
    let empty: Vec<Rec> = vec![];
    for r in secs.get("TABLES").unwrap_or(&empty).iter().filter(|x| x.ty == "LTYPE") {
        let name = decode_text(r.first(2).unwrap_or("")).trim().to_string();
        let upper = name.to_uppercase();
        if name.is_empty() || matches!(upper.as_str(), "BYBLOCK" | "BYLAYER") || LINETYPES.iter().any(|l| l.name.eq_ignore_ascii_case(&name)) {
            continue;
        }
        if r.all(74).iter().any(|v| v.trim() != "0") {
            continue;
        }
        let pattern: Vec<f64> = r.all(49).iter().map(|v| parse_f(v)).collect();
        if !pattern.is_empty() {
            let _ = doc.define_linetype(crate::defs::LinetypeDef { name, desc: decode_text(r.first(3).unwrap_or("")), pattern });
        }
    }

    // ---- layers
    doc.layers.clear();
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
    let layer_names: HashMap<String, String> = secs.get("TABLES").unwrap_or(&empty).iter().filter(|x| x.ty == "LAYER").filter_map(|r| Some((r.first(5)?.to_uppercase(), decode_text(r.first(2)?)))).collect();
    let mut ctx = Ctx { blocks: HashMap::new(), skipped: Default::default(), image_defs: HashMap::new(), image_keys: HashMap::new(), images: Default::default(), layer_names, groups: groups_of(secs.get("OBJECTS").unwrap_or(&empty)), model: false, solids: vec![] };
    // what CadForge keeps that DXF has no place for; loaded linetypes and patterns first, the entities use them
    let extras = cadforge_extras(secs.get("OBJECTS").unwrap_or(&empty));
    if let Some(x) = &extras {
        for def in x.get("ltypes").and_then(|v| serde_json::from_value::<Vec<crate::defs::LinetypeDef>>(v.clone()).ok()).unwrap_or_default() {
            let _ = doc.define_linetype(def);
        }
        for def in x.get("patterns").and_then(|v| serde_json::from_value::<Vec<crate::defs::PatternDef>>(v.clone()).ok()).unwrap_or_default() {
            let _ = doc.define_pattern(def);
        }
    }
    for r in secs.get("OBJECTS").unwrap_or(&empty).iter().filter(|x| x.ty == "IMAGEDEF") {
        if let Some(h) = r.first(5) {
            let px = |v: f64| if v.is_finite() && v > 0.0 { v.min(1e6) as u32 } else { 0 };
            ctx.image_defs.insert(h.to_string(), (decode_text(r.first(1).unwrap_or("")), px(r.num(10, 0.0)), px(r.num(20, 0.0))));
        }
    }
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
    // anonymous blocks that an imported INSERT refers to (dynamic block references *U, ...) are blocks like the
    // named ones; the others (dimension *D blocks, table *T blocks and what only they use) are not
    let space = |n: &str| {
        let l = n.to_lowercase();
        l.starts_with("*model_space") || l.starts_with("*paper_space")
    };
    let mut referenced: std::collections::HashSet<String> = Default::default();
    let mut todo: Vec<&Rec> = secs.get("ENTITIES").unwrap_or(&empty).iter().collect();
    todo.extend(ctx.blocks.iter().filter(|(n, _)| !n.starts_with('*') || space(n)).flat_map(|(_, b)| b.entities.iter()));
    while let Some(r) = todo.pop() {
        // (the blocks of CadForge's tables and regions are only their pictures for other programs)
        let Some(n) = r.first(2).filter(|_| r.ty == "INSERT" && cadforge_wrapped(r).is_none()).map(decode_text) else { continue };
        if n.starts_with('*') && !space(&n) && referenced.insert(n.clone()) {
            if let Some(b) = ctx.blocks.get(&n) {
                todo.extend(b.entities.iter());
            }
        }
    }
    for n in order.iter().filter(|n| !n.starts_with('*') || (referenced.contains(*n) && !space(n))) {
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

    // ---- layouts: LAYOUT objects name them and give the sheet size; BLOCK_RECORDs tie each paper-space block to one
    struct LayoutDef {
        name: String,
        tab: i64,
        paper: Paper,
    }
    let mut layout_defs: HashMap<String, LayoutDef> = HashMap::new(); // by LAYOUT handle
    for r in secs.get("OBJECTS").unwrap_or(&empty).iter().filter(|x| x.ty == "LAYOUT") {
        let Some(h) = r.first(5) else { continue };
        let Some(at) = r.g.iter().position(|(c, v)| *c == 100 && v == "AcDbLayout") else { continue };
        let after = &r.g[at..];
        let name = decode_text(after.iter().find(|(c, _)| *c == 1).map_or("", |(_, v)| v.as_str()));
        if name.is_empty() || name.eq_ignore_ascii_case("Model") {
            continue;
        }
        let tab = after.iter().find(|(c, _)| *c == 71).and_then(|(_, v)| v.trim().parse::<i64>().ok()).unwrap_or(0);
        let k = if r.int(72, 1) == 0 { 25.4 } else { 1.0 };
        let (mut w, mut hh) = (r.num(44, 297.0) * k, r.num(45, 210.0) * k);
        if matches!(r.int(73, 0), 1 | 3) {
            std::mem::swap(&mut w, &mut hh);
        }
        if !(w > 1.0 && hh > 1.0 && w.is_finite() && hh.is_finite()) {
            (w, hh) = (297.0, 210.0);
        }
        let pname = r.first(4).unwrap_or("");
        let known = ["A0", "A1", "A2", "A3", "A4", "Letter", "Legal", "Tabloid", "ANSI C", "ANSI D"].iter().find(|n| pname.eq_ignore_ascii_case(n)).map(|n| n.to_string());
        layout_defs.insert(h.to_string(), LayoutDef { name, tab, paper: Paper { w, h: hh, name: known.unwrap_or_else(|| "Custom".into()), landscape: w >= hh } });
    }
    let mut block_layout: HashMap<String, String> = HashMap::new(); // paper-space block name -> layout name
    for r in secs.get("TABLES").unwrap_or(&empty).iter().filter(|x| x.ty == "BLOCK_RECORD") {
        let bn = decode_text(r.first(2).unwrap_or(""));
        if let Some(def) = r.first(340).and_then(|h| layout_defs.get(h)) {
            block_layout.insert(bn.to_lowercase(), def.name.clone());
        }
    }
    let mut sheet_ents: HashMap<String, Vec<Entity>> = HashMap::new();
    let mut paper = 0usize;
    let mut fallback_names: Vec<String> = vec![];
    let layout_of = |bn: &str, fallback: &mut Vec<String>| -> String {
        if let Some(n) = block_layout.get(&bn.to_lowercase()) {
            return n.clone();
        }
        // files without LAYOUT objects: the first paper-space block becomes "Layout1", then "Layout2"...
        let key = bn.to_lowercase();
        if let Some(i) = fallback.iter().position(|k| *k == key) {
            return format!("Layout{}", i + 1);
        }
        fallback.push(key);
        format!("Layout{}", fallback.len())
    };

    // ---- model space (and the active layout's entities, stored in ENTITIES with group 67 = 1)
    // (the ids model-space objects get, by handle: solids' extrusion histories name their outlines by handle)
    let mut ids_by_handle: HashMap<String, u64> = HashMap::new();
    for r in group_entities(secs.get("ENTITIES").unwrap_or(&empty)) {
        if r.first(67) == Some("1") {
            let ln = layout_of("*Paper_Space", &mut fallback_names);
            for e in convert(&r, &mut ctx) {
                sheet_ents.entry(ln.clone()).or_default().push(e);
            }
            paper += 1;
            continue;
        }
        ctx.model = true;
        let ents = convert(&r, &mut ctx);
        ctx.model = false;
        for e in ents {
            let id = doc.add_to(e, "model");
            if let Some(h) = r.first(5) {
                ids_by_handle.entry(h.trim().to_uppercase()).or_insert(id);
            }
        }
    }
    // the other layouts live in their own `*Paper_SpaceN` blocks
    for n in order.iter().filter(|n| n.to_lowercase().starts_with("*paper_space")) {
        let recs = ctx.blocks[n].entities.clone();
        let ln = layout_of(n, &mut fallback_names);
        for r in &recs {
            for e in convert(r, &mut ctx) {
                sheet_ents.entry(ln.clone()).or_default().push(e);
            }
        }
    }
    let mut named: Vec<(i64, String, Paper)> = layout_defs.values().map(|d| (d.tab, d.name.clone(), d.paper.clone())).collect();
    for n in sheet_ents.keys() {
        if !named.iter().any(|(_, ln, _)| ln == n) {
            named.push((1000 + named.len() as i64, n.clone(), Paper::named("A3", true)));
        }
    }
    named.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    for (_, ln, pp) in named {
        let entities = sheet_ents.remove(&ln).unwrap_or_default();
        doc.layouts.push(Layout { name: ln.clone(), paper: pp, entities: vec![], fresh: false });
        for e in entities {
            doc.add_to(e, &ln);
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

    let mut solid_ids: HashMap<u64, u64> = HashMap::new();
    for (old, s) in join_solids(std::mem::take(&mut ctx.solids)) {
        if doc.layer(&s.layer).is_none() {
            doc.layers.push(Layer::new(&s.layer));
        }
        let id = doc.insert_solid_raw(s);
        if let Some(old) = old {
            solid_ids.insert(old, id);
        }
    }
    // extrusion histories: the outlines by their new ids
    for p in extras.as_ref().and_then(|x| x.get("programs")).and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let Some(id) = p["solid"].as_u64().and_then(|old| solid_ids.get(&old)) else { continue };
        let mut prog = p["program"].clone();
        let mapped = prog["features"].as_array_mut().is_some_and(|fs| {
            fs.iter_mut().all(|f| {
                let ids: Option<Vec<u64>> = f["profile"].as_array().map(|hs| hs.iter().filter_map(|h| h.as_str().and_then(|h| ids_by_handle.get(&h.to_uppercase()).copied())).collect());
                let all = ids.as_ref().zip(f["profile"].as_array()).is_some_and(|(i, hs)| i.len() == hs.len());
                f["profile"] = ids.unwrap_or_default().into();
                all
            })
        });
        if let (true, Ok(prog)) = (mapped, serde_json::from_value::<crate::solid::Program>(prog)) {
            if let Some(s) = doc.solids.iter_mut().find(|s| s.id == *id) {
                s.program = Some(prog);
            }
        }
    }
    // annotative objects, by the new ids
    for a in extras.as_ref().and_then(|x| x.get("annotative")).and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let id = a["h"].as_str().and_then(|h| ids_by_handle.get(&h.trim().to_uppercase()).copied());
        if let (Some(id), Ok(anno)) = (id, serde_json::from_value::<crate::annotative::Annotative>(a["anno"].clone())) {
            if let Some(e) = doc.by_id_mut(id) {
                e.anno = Some(anno);
            }
        }
    }
    // associative dimensions: their links, by the new ids of the objects
    for a in extras.as_ref().and_then(|x| x.get("dim_assoc")).and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let id_of = |h: &serde_json::Value| h.as_str().and_then(|h| ids_by_handle.get(&h.trim().to_uppercase()).copied());
        let Some(dim) = id_of(&a["dim"]) else { continue };
        let refs: Vec<crate::dim::DimRef> = a["refs"]
            .as_array()
            .map(|rs| {
                rs.iter()
                    .filter_map(|r| {
                        let mut r = r.clone();
                        r["ent"] = id_of(&r["ent"])?.into();
                        serde_json::from_value(r).ok()
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Some(Entity { kind: Kind::Dim(d), .. }) = doc.by_id_mut(dim) {
            d.assoc = refs;
        }
    }
    doc.images = std::mem::take(&mut ctx.images);
    if let Some(x) = &extras {
        apply_extras(&mut doc, x);
    }
    doc.skipped = ctx.skipped;
    doc.paper_skipped = 0;
    let _ = paper;
    doc.validate()?;
    doc.dirty = false;
    Ok(doc)
}
