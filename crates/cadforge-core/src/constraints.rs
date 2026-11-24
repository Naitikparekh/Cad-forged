//! Parametric constraints (GEOMCONSTRAINT, DIMCONSTRAINT, AUTOCONSTRAIN, PARAMETERS).
//!
//! A constraint ties points, lines and circles of model-space objects together: geometric ones (horizontal,
//! parallel, tangent, coincident...) and dimensional ones (a distance, radius, diameter or angle with a name and an
//! expression, such as `d2 = d1 / 2`). Whenever a command changes constrained objects, [`solve`] moves the geometry
//! the least it can so that every constraint holds again; what the user just edited moves least of all.
//!
//! The constraints are kept with model space ([`crate::drawing::Space`]), so they are saved in CadForge drawings and
//! undone with the geometry. DWG and DXF files do not keep them.
use crate::dim::{DimData, DimType};
use crate::drawing::Drawing;
use crate::entities::*;
use crate::geom::*;
use crate::text::TextData;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

/// Which part of an object a constraint uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum At {
    /// The object as a whole: a line (or polyline segment `i`), a circle or an arc.
    #[default]
    Whole,
    Start,
    End,
    Mid,
    Center,
    /// Polyline vertex `i`.
    Vertex,
}

/// A reference to part of an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PRef {
    pub ent: u64,
    #[serde(default)]
    pub at: At,
    /// The polyline vertex (At::Vertex) or segment (At::Whole on a polyline).
    #[serde(default)]
    pub i: usize,
}

impl PRef {
    pub fn new(ent: u64, at: At, i: usize) -> PRef {
        PRef { ent, at, i }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CKind {
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    Coincident,
    Collinear,
    Concentric,
    Equal,
    Tangent,
    Fix,
    Symmetric,
    /// Dimensional: the distance between two points along their line (aligned).
    Distance,
    /// Dimensional: the horizontal distance between two points.
    HDistance,
    /// Dimensional: the vertical distance between two points.
    VDistance,
    Radius,
    Diameter,
    /// Dimensional: the angle between two lines, in degrees.
    Angle,
}

impl CKind {
    pub fn dimensional(self) -> bool {
        matches!(self, CKind::Distance | CKind::HDistance | CKind::VDistance | CKind::Radius | CKind::Diameter | CKind::Angle)
    }

    pub fn label(self) -> &'static str {
        match self {
            CKind::Horizontal => "Horizontal",
            CKind::Vertical => "Vertical",
            CKind::Parallel => "Parallel",
            CKind::Perpendicular => "Perpendicular",
            CKind::Coincident => "Coincident",
            CKind::Collinear => "Collinear",
            CKind::Concentric => "Concentric",
            CKind::Equal => "Equal",
            CKind::Tangent => "Tangent",
            CKind::Fix => "Fix",
            CKind::Symmetric => "Symmetric",
            CKind::Distance => "Aligned",
            CKind::HDistance => "Horizontal distance",
            CKind::VDistance => "Vertical distance",
            CKind::Radius => "Radius",
            CKind::Diameter => "Diameter",
            CKind::Angle => "Angular",
        }
    }

    /// The symbol drawn in the constraint bar next to the geometry.
    fn glyph(self) -> &'static str {
        match self {
            CKind::Horizontal => "H",
            CKind::Vertical => "V",
            CKind::Parallel => "\u{2225}",
            CKind::Perpendicular => "\u{22a5}",
            CKind::Coincident => "\u{25aa}",
            CKind::Collinear => "\u{2261}",
            CKind::Concentric => "\u{25ce}",
            CKind::Equal => "=",
            CKind::Tangent => "T",
            CKind::Fix => "F",
            CKind::Symmetric => "[ ]",
            _ => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: u64,
    pub kind: CKind,
    pub refs: Vec<PRef>,
    /// Dimensional constraints: the parameter name (`d1`) and its expression (`50`, `d1*2`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expr: String,
    /// The value the expression gave when it was last worked out (degrees for angles).
    #[serde(default)]
    pub value: f64,
    /// Fix: where the fixed point (or the object's defining values) must stay.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hold: Vec<f64>,
    /// Dimensional constraints: how far the shown dimension sits from the geometry (drawing units, signed).
    #[serde(default)]
    pub off: f64,
}

/// A user parameter (PARAMETERS New): a name and an expression other expressions can use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserParam {
    pub name: String,
    pub expr: String,
    #[serde(default)]
    pub value: f64,
}

/// The constraints of a space.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Constraints {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<Constraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<UserParam>,
    /// CONSTRAINTBAR Hide / DCDISPLAY Hide.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_bars: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_dims: bool,
}

impl Constraints {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty() && self.params.is_empty()
    }

    pub fn next_id(&self) -> u64 {
        self.list.iter().map(|c| c.id).max().unwrap_or(0) + 1
    }

    /// A parameter name not used yet: `d1`, `d2`... (`rad1`, `dia1`, `ang1` for those kinds, as AutoCAD names them).
    pub fn next_name(&self, kind: CKind) -> String {
        let prefix = match kind {
            CKind::Radius => "rad",
            CKind::Diameter => "dia",
            CKind::Angle => "ang",
            _ => "d",
        };
        (1..).map(|i| format!("{prefix}{i}")).find(|n| !self.has_name(n)).expect("a free name")
    }

    pub fn has_name(&self, n: &str) -> bool {
        self.list.iter().any(|c| c.name.eq_ignore_ascii_case(n)) || self.params.iter().any(|p| p.name.eq_ignore_ascii_case(n))
    }

    /// The constraints that use object `id`.
    pub fn on(&self, id: u64) -> impl Iterator<Item = &Constraint> {
        self.list.iter().filter(move |c| c.refs.iter().any(|r| r.ent == id))
    }
}

// ---------------------------------------------------------------- expressions

/// Work out an expression of numbers, parameter names, + - * / ^, parentheses and the functions sin, cos, tan
/// (degrees), sqrt, abs, round, min, max and the constant pi.
pub fn eval_expr(s: &str, vars: &HashMap<String, f64>) -> Result<f64, String> {
    struct P<'a> {
        c: Vec<char>,
        i: usize,
        vars: &'a HashMap<String, f64>,
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self.c.get(self.i).is_some_and(|c| c.is_whitespace()) {
                self.i += 1;
            }
        }
        fn eat(&mut self, ch: char) -> bool {
            self.ws();
            if self.c.get(self.i) == Some(&ch) {
                self.i += 1;
                true
            } else {
                false
            }
        }
        fn expr(&mut self) -> Result<f64, String> {
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
        fn term(&mut self) -> Result<f64, String> {
            let mut v = self.power()?;
            loop {
                if self.eat('*') {
                    v *= self.power()?;
                } else if self.eat('/') {
                    let d = self.power()?;
                    if d == 0.0 {
                        return Err("division by zero".into());
                    }
                    v /= d;
                } else {
                    return Ok(v);
                }
            }
        }
        fn power(&mut self) -> Result<f64, String> {
            let b = self.unary()?;
            if self.eat('^') {
                return Ok(b.powf(self.power()?));
            }
            Ok(b)
        }
        fn unary(&mut self) -> Result<f64, String> {
            if self.eat('-') {
                return Ok(-self.unary()?);
            }
            if self.eat('+') {
                return self.unary();
            }
            self.primary()
        }
        fn primary(&mut self) -> Result<f64, String> {
            self.ws();
            if self.eat('(') {
                let v = self.expr()?;
                if !self.eat(')') {
                    return Err("a ) is missing".into());
                }
                return Ok(v);
            }
            let start = self.i;
            let c = *self.c.get(self.i).ok_or("the expression ends too soon")?;
            if c.is_ascii_digit() || c == '.' {
                while self.c.get(self.i).is_some_and(|c| c.is_ascii_digit() || *c == '.' || *c == 'e' || *c == 'E') {
                    self.i += 1;
                }
                let t: String = self.c[start..self.i].iter().collect();
                return t.parse::<f64>().map_err(|_| format!("{t} is not a number"));
            }
            if c.is_alphabetic() || c == '_' {
                while self.c.get(self.i).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
                    self.i += 1;
                }
                let name: String = self.c[start..self.i].iter().collect();
                let lower = name.to_ascii_lowercase();
                if self.eat('(') {
                    let mut args = vec![self.expr()?];
                    while self.eat(',') {
                        args.push(self.expr()?);
                    }
                    if !self.eat(')') {
                        return Err("a ) is missing".into());
                    }
                    let a = args[0];
                    let rad = a.to_radians();
                    return Ok(match (lower.as_str(), args.len()) {
                        ("sin", 1) => rad.sin(),
                        ("cos", 1) => rad.cos(),
                        ("tan", 1) => rad.tan(),
                        ("sqrt", 1) if a >= 0.0 => a.sqrt(),
                        ("abs", 1) => a.abs(),
                        ("round", 1) => a.round(),
                        ("min", _) => args.iter().copied().fold(f64::INFINITY, f64::min),
                        ("max", _) => args.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                        _ => return Err(format!("{name}() is not a function CadForge knows")),
                    });
                }
                if lower == "pi" {
                    return Ok(PI);
                }
                return self.vars.iter().find(|(k, _)| k.eq_ignore_ascii_case(&name)).map(|(_, v)| *v).ok_or_else(|| format!("{name} is not a parameter"));
            }
            Err(format!("unexpected {c}"))
        }
    }
    let mut p = P { c: s.chars().collect(), i: 0, vars };
    let v = p.expr()?;
    p.ws();
    if p.i < p.c.len() {
        return Err(format!("unexpected {}", p.c[p.i]));
    }
    if !v.is_finite() {
        return Err("the value is not a number".into());
    }
    Ok(v)
}

/// Work out every parameter's expression (in dependency order); an error names the first one that cannot be.
pub fn evaluate_params(cs: &mut Constraints) -> Result<(), String> {
    let total = cs.list.iter().filter(|c| c.kind.dimensional()).count() + cs.params.len();
    let mut vars: HashMap<String, f64> = HashMap::new();
    let mut done: HashSet<String> = HashSet::new();
    for _ in 0..=total {
        let mut progress = false;
        let mut last_err = None;
        for p in cs.params.iter_mut() {
            if done.contains(&p.name.to_ascii_lowercase()) {
                continue;
            }
            match eval_expr(&p.expr, &vars) {
                Ok(v) => {
                    p.value = v;
                    vars.insert(p.name.clone(), v);
                    done.insert(p.name.to_ascii_lowercase());
                    progress = true;
                }
                Err(e) => last_err = Some(format!("{}: {e}", p.name)),
            }
        }
        for c in cs.list.iter_mut().filter(|c| c.kind.dimensional()) {
            if done.contains(&c.name.to_ascii_lowercase()) {
                continue;
            }
            match eval_expr(&c.expr, &vars) {
                Ok(v) => {
                    c.value = v;
                    vars.insert(c.name.clone(), v);
                    done.insert(c.name.to_ascii_lowercase());
                    progress = true;
                }
                Err(e) => last_err = Some(format!("{}: {e}", c.name)),
            }
        }
        if done.len() == total {
            return Ok(());
        }
        if !progress {
            return Err(last_err.unwrap_or_else(|| "the parameters refer to each other in a circle".into()));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- the geometry as variables

#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Line,
    Circle,
    Arc,
    Pline { n: usize, closed: bool },
    Point,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    shape: Shape,
    off: usize,
    len: usize,
}

/// The constrained objects' defining values laid out in one vector.
struct Vars {
    slots: HashMap<u64, Slot>,
    x: Vec<f64>,
}

fn shape_of(e: &Entity) -> Option<(Shape, Vec<f64>)> {
    Some(match &e.kind {
        Kind::Line { a, b } => (Shape::Line, vec![a.x, a.y, b.x, b.y]),
        Kind::Circle { c, r } => (Shape::Circle, vec![c.x, c.y, *r]),
        Kind::Arc { c, r, a0, a1 } => (Shape::Arc, vec![c.x, c.y, *r, *a0, *a1]),
        Kind::Pline { v, closed } if v.len() >= 2 => (Shape::Pline { n: v.len(), closed: *closed }, v.iter().flat_map(|p| [p.x, p.y]).collect()),
        Kind::Point { p } => (Shape::Point, vec![p.x, p.y]),
        _ => return None,
    })
}

/// Whether constraints can use object `e`.
pub fn constrainable(e: &Entity) -> bool {
    shape_of(e).is_some()
}

fn store(e: &mut Entity, x: &[f64]) {
    match &mut e.kind {
        Kind::Line { a, b } => {
            *a = pt(x[0], x[1]);
            *b = pt(x[2], x[3]);
        }
        Kind::Circle { c, r } => {
            *c = pt(x[0], x[1]);
            *r = x[2].abs();
        }
        Kind::Arc { c, r, a0, a1 } => {
            *c = pt(x[0], x[1]);
            *r = x[2].abs();
            *a0 = norm_angle(x[3]);
            *a1 = norm_angle(x[4]);
        }
        Kind::Pline { v, .. } => {
            for (i, p) in v.iter_mut().enumerate() {
                p.x = x[2 * i];
                p.y = x[2 * i + 1];
            }
        }
        Kind::Point { p } => *p = pt(x[0], x[1]),
        _ => {}
    }
}

impl Vars {
    fn slot(&self, id: u64) -> Option<Slot> {
        self.slots.get(&id).copied()
    }

    fn point(&self, x: &[f64], r: &PRef) -> Option<Pt> {
        let s = self.slot(r.ent)?;
        let v = &x[s.off..s.off + s.len];
        let p = |i: usize| pt(v[i], v[i + 1]);
        Some(match (s.shape, r.at) {
            (Shape::Line, At::Start) => p(0),
            (Shape::Line, At::End) => p(2),
            (Shape::Line, _) => mid(p(0), p(2)),
            (Shape::Circle, _) => p(0),
            (Shape::Arc, At::Start) => p(0) + pt(v[3].cos(), v[3].sin()) * v[2],
            (Shape::Arc, At::End) => p(0) + pt(v[4].cos(), v[4].sin()) * v[2],
            (Shape::Arc, At::Mid) => {
                let a = v[3] + sweep_ccw(v[3], v[4]) / 2.0;
                p(0) + pt(a.cos(), a.sin()) * v[2]
            }
            (Shape::Arc, _) => p(0),
            (Shape::Pline { n, closed }, at) => match at {
                At::Start => p(0),
                At::End => p(2 * (n - 1)),
                At::Vertex => p(2 * r.i.min(n - 1)),
                _ => {
                    let i = r.i.min(if closed { n - 1 } else { n - 2 });
                    mid(p(2 * i), p(2 * ((i + 1) % n)))
                }
            },
            (Shape::Point, _) => p(0),
        })
    }

    fn line(&self, x: &[f64], r: &PRef) -> Option<(Pt, Pt)> {
        let s = self.slot(r.ent)?;
        let v = &x[s.off..s.off + s.len];
        let p = |i: usize| pt(v[i], v[i + 1]);
        match s.shape {
            Shape::Line => Some((p(0), p(2))),
            Shape::Pline { n, closed } => {
                let i = r.i.min(if closed { n - 1 } else { n - 2 });
                Some((p(2 * i), p(2 * ((i + 1) % n))))
            }
            _ => None,
        }
    }

    fn circle(&self, x: &[f64], r: &PRef) -> Option<(Pt, f64)> {
        let s = self.slot(r.ent)?;
        let v = &x[s.off..s.off + s.len];
        match s.shape {
            Shape::Circle | Shape::Arc => Some((pt(v[0], v[1]), v[2])),
            _ => None,
        }
    }

    /// The values a Fix constraint holds: the point it fixes, or (Whole) every value of the object.
    fn fix_values(&self, x: &[f64], r: &PRef) -> Option<Vec<f64>> {
        if r.at == At::Whole {
            let s = self.slot(r.ent)?;
            return Some(x[s.off..s.off + s.len].to_vec());
        }
        let p = self.point(x, r)?;
        Some(vec![p.x, p.y])
    }
}

fn wrap(a: f64) -> f64 {
    let mut a = a % (2.0 * PI);
    if a > PI {
        a -= 2.0 * PI;
    } else if a < -PI {
        a += 2.0 * PI;
    }
    a
}

/// The residuals of one constraint: all zero when it holds. `None` when its objects are gone or of the wrong kind.
fn residuals(vs: &Vars, x: &[f64], c: &Constraint, out: &mut Vec<f64>) -> Option<()> {
    let r = &c.refs;
    let dir = |l: (Pt, Pt)| l.1 - l.0;
    let norm = |v: Pt| len(v).max(1e-12);
    match c.kind {
        CKind::Horizontal | CKind::Vertical => {
            let (a, b) = if r.len() >= 2 { (vs.point(x, &r[0])?, vs.point(x, &r[1])?) } else { vs.line(x, &r[0])? };
            out.push(if c.kind == CKind::Horizontal { b.y - a.y } else { b.x - a.x });
        }
        CKind::Parallel | CKind::Perpendicular => {
            let (d1, d2) = (dir(vs.line(x, r.first()?)?), dir(vs.line(x, r.get(1)?)?));
            let k = norm(d1) * norm(d2);
            out.push(if c.kind == CKind::Parallel { cross(d1, d2) / k } else { dot(d1, d2) / k });
        }
        CKind::Coincident => {
            let a = vs.point(x, r.first()?)?;
            let other = r.get(1)?;
            // a point on a line or circle (the second reference is the object as a whole)
            if other.at == At::Whole {
                if let Some(l) = vs.line(x, other) {
                    out.push(cross(dir(l), a - l.0) / norm(dir(l)));
                    return Some(());
                }
                if let Some((cc, rad)) = vs.circle(x, other) {
                    out.push(dist(a, cc) - rad.abs());
                    return Some(());
                }
            }
            let b = vs.point(x, other)?;
            out.push(a.x - b.x);
            out.push(a.y - b.y);
        }
        CKind::Collinear => {
            let (l1, l2) = (vs.line(x, r.first()?)?, vs.line(x, r.get(1)?)?);
            let d = dir(l1);
            out.push(cross(d, l2.0 - l1.0) / norm(d));
            out.push(cross(d, l2.1 - l1.0) / norm(d));
        }
        CKind::Concentric => {
            let (a, b) = (vs.circle(x, r.first()?)?.0, vs.circle(x, r.get(1)?)?.0);
            out.push(a.x - b.x);
            out.push(a.y - b.y);
        }
        CKind::Equal => {
            let (a, b) = (r.first()?, r.get(1)?);
            match (vs.line(x, a), vs.line(x, b)) {
                (Some(l1), Some(l2)) => out.push(dist(l1.0, l1.1) - dist(l2.0, l2.1)),
                _ => out.push(vs.circle(x, a)?.1.abs() - vs.circle(x, b)?.1.abs()),
            }
        }
        CKind::Tangent => {
            let (a, b) = (r.first()?, r.get(1)?);
            match (vs.line(x, a), vs.line(x, b), vs.circle(x, a), vs.circle(x, b)) {
                (Some(l), _, _, Some((cc, rad))) | (_, Some(l), Some((cc, rad)), _) => out.push((cross(dir(l), cc - l.0) / norm(dir(l))).abs() - rad.abs()),
                (_, _, Some((c1, r1)), Some((c2, r2))) => {
                    let d = dist(c1, c2);
                    let (r1, r2) = (r1.abs(), r2.abs());
                    out.push(if d >= r1.max(r2) { d - (r1 + r2) } else { d - (r1 - r2).abs() });
                }
                _ => return None,
            }
        }
        CKind::Fix => {
            let now = vs.fix_values(x, r.first()?)?;
            if now.len() != c.hold.len() {
                return None;
            }
            out.extend(now.iter().zip(&c.hold).map(|(a, b)| a - b));
        }
        CKind::Symmetric => {
            let (a, b, l) = (vs.point(x, r.first()?)?, vs.point(x, r.get(1)?)?, vs.line(x, r.get(2)?)?);
            let d = dir(l);
            out.push(cross(d, mid(a, b) - l.0) / norm(d));
            out.push(dot(d, b - a) / norm(d));
        }
        CKind::Distance | CKind::HDistance | CKind::VDistance => {
            let (a, b) = if r.len() >= 2 { (vs.point(x, &r[0])?, vs.point(x, &r[1])?) } else { vs.line(x, &r[0])? };
            out.push(match c.kind {
                CKind::HDistance => (b.x - a.x).abs() - c.value,
                CKind::VDistance => (b.y - a.y).abs() - c.value,
                _ => dist(a, b) - c.value,
            });
        }
        CKind::Radius => out.push(vs.circle(x, r.first()?)?.1.abs() - c.value),
        CKind::Diameter => out.push(2.0 * vs.circle(x, r.first()?)?.1.abs() - c.value),
        CKind::Angle => {
            let (d1, d2) = (dir(vs.line(x, r.first()?)?), dir(vs.line(x, r.get(1)?)?));
            let a = wrap(d2.y.atan2(d2.x) - d1.y.atan2(d1.x));
            // the angle between the lines, whichever way they run
            let want = c.value.to_radians();
            let cands = [a - want, a + want, wrap(a + PI) - want, wrap(a + PI) + want];
            out.push(cands.iter().copied().map(wrap).min_by(|p, q| p.abs().total_cmp(&q.abs())).unwrap_or(0.0));
        }
    }
    Some(())
}

fn all_residuals(vs: &Vars, x: &[f64], cs: &[Constraint]) -> Vec<f64> {
    let mut out = vec![];
    for c in cs {
        residuals(vs, x, c, &mut out);
    }
    out
}

/// Solve `a y = b` (a is n×n, row-major) by Gaussian elimination with partial pivoting.
fn solve_linear(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for k in 0..n {
        let p = (k..n).max_by(|i, j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))?;
        if a[p * n + k].abs() < 1e-300 {
            return None;
        }
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
            b.swap(k, p);
        }
        for i in k + 1..n {
            let f = a[i * n + k] / a[k * n + k];
            if f != 0.0 {
                for j in k..n {
                    a[i * n + j] -= f * a[k * n + j];
                }
                b[i] -= f * b[k];
            }
        }
    }
    let mut y = vec![0.0; n];
    for k in (0..n).rev() {
        let s: f64 = (k + 1..n).map(|j| a[k * n + j] * y[j]).sum();
        y[k] = (b[k] - s) / a[k * n + k];
    }
    Some(y)
}

/// The Jacobian of the residuals (m rows × n columns), by central differences.
fn jacobian(vs: &Vars, x: &[f64], cs: &[Constraint], m: usize) -> Vec<f64> {
    let n = x.len();
    let mut j = vec![0.0; m * n];
    let mut xp = x.to_vec();
    for k in 0..n {
        let h = 1e-7 * (1.0 + x[k].abs());
        xp[k] = x[k] + h;
        let fp = all_residuals(vs, &xp, cs);
        xp[k] = x[k] - h;
        let fm = all_residuals(vs, &xp, cs);
        xp[k] = x[k];
        if fp.len() != m || fm.len() != m {
            continue;
        }
        for i in 0..m {
            j[i * n + k] = (fp[i] - fm[i]) / (2.0 * h);
        }
    }
    j
}

/// The rank of an m×n matrix (row echelon form with a relative tolerance).
fn rank(mut a: Vec<f64>, m: usize, n: usize) -> usize {
    let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs())).max(1e-12);
    let mut r = 0;
    for c in 0..n {
        if r == m {
            break;
        }
        let Some(p) = (r..m).max_by(|i, j| a[i * n + c].abs().total_cmp(&a[j * n + c].abs())) else { break };
        if a[p * n + c].abs() < 1e-9 * scale {
            continue;
        }
        for j in 0..n {
            a.swap(r * n + j, p * n + j);
        }
        for i in r + 1..m {
            let f = a[i * n + c] / a[r * n + c];
            for j in c..n {
                a[i * n + j] -= f * a[r * n + j];
            }
        }
        r += 1;
    }
    r
}

/// Gauss-Newton iterations from `x0` with minimum (weighted) norm steps: dx = W Jᵀ (J W Jᵀ + λI)⁻¹ (−f). A weight of
/// 0 holds a value still. Returns whether every residual came within `tol`, and the values reached.
fn gauss_newton(vs: &Vars, cs: &[Constraint], x0: &[f64], w: &[f64], tol: f64) -> (bool, Vec<f64>) {
    let n = x0.len();
    let mut x = x0.to_vec();
    for _ in 0..100 {
        let f = all_residuals(vs, &x, cs);
        let m = f.len();
        // polish well below the tolerance, so repeated solves do not creep
        if f.iter().all(|v| v.abs() <= tol * 1e-4) {
            return (true, x);
        }
        let j = jacobian(vs, &x, cs, m);
        let mut a = vec![0.0; m * m];
        for r in 0..m {
            for c in r..m {
                let s: f64 = (0..n).map(|k| j[r * n + k] * w[k] * j[c * n + k]).sum();
                a[r * m + c] = s;
                a[c * m + r] = s;
            }
        }
        let tr = (0..m).map(|i| a[i * m + i]).fold(0.0, f64::max).max(1e-12);
        for i in 0..m {
            a[i * m + i] += 1e-12 * tr;
        }
        let Some(y) = solve_linear(a, f.iter().map(|v| -v).collect(), m) else { break };
        let dx: Vec<f64> = (0..n).map(|k| w[k] * (0..m).map(|r| j[r * n + k] * y[r]).sum::<f64>()).collect();
        // halve the step until it does not make things worse
        let before: f64 = f.iter().map(|v| v * v).sum();
        let mut t = 1.0;
        let mut moved = false;
        for _ in 0..12 {
            let xt: Vec<f64> = x.iter().zip(&dx).map(|(a, b)| a + t * b).collect();
            let after: f64 = all_residuals(vs, &xt, cs).iter().map(|v| v * v).sum();
            if after < before {
                x = xt;
                moved = true;
                break;
            }
            t /= 2.0;
        }
        if !moved {
            break;
        }
    }
    let ok = all_residuals(vs, &x, cs).iter().all(|v| v.abs() <= tol);
    (ok, x)
}

fn load(d: &Drawing, cs: &[Constraint]) -> Vars {
    let mut slots = HashMap::new();
    let mut x = vec![];
    let ids: Vec<u64> = cs.iter().flat_map(|c| c.refs.iter().map(|r| r.ent)).collect();
    for id in ids {
        if slots.contains_key(&id) {
            continue;
        }
        let Some((shape, v)) = d.model.entities.iter().find(|e| e.id == id).and_then(shape_of) else { continue };
        slots.insert(id, Slot { shape, off: x.len(), len: v.len() });
        x.extend(v);
    }
    Vars { slots, x }
}

/// The outcome of [`solve`].
#[derive(Clone, Debug, PartialEq)]
pub struct Solved {
    /// Every constraint holds.
    pub ok: bool,
    /// How many objects the solver moved.
    pub moved: usize,
    /// The degrees of freedom left (0: fully constrained).
    pub dof: usize,
    /// Why it failed, when it did.
    pub message: String,
}

/// Bring the geometry in line with every constraint, moving the objects in `keep` (the ones the user just changed)
/// as little as possible. Constraints whose objects were erased are dropped. When the constraints cannot all hold,
/// the geometry is left as it was and the result says why.
pub fn solve(d: &mut Drawing, keep: &HashSet<u64>) -> Solved {
    let present: HashSet<u64> = d.model.entities.iter().filter(|e| constrainable(e)).map(|e| e.id).collect();
    d.model.constraints.list.retain(|c| c.refs.iter().all(|r| present.contains(&r.ent)));
    if let Err(e) = evaluate_params(&mut d.model.constraints) {
        return Solved { ok: false, moved: 0, dof: 0, message: format!("A parameter cannot be worked out ({e})") };
    }
    let cs = d.model.constraints.list.clone();
    if cs.is_empty() {
        return Solved { ok: true, moved: 0, dof: 0, message: String::new() };
    }
    let vs = load(d, &cs);
    let x0 = vs.x.clone();
    let n = x0.len();
    let size = x0.iter().fold(1.0f64, |s, v| s.max(v.abs()));
    let tol = 1e-9 * size;
    // what the user changed stays where it is; only when that cannot work does it move (a million times less than
    // the rest)
    let weights = |kept: f64| {
        let mut w = vec![1.0; n];
        for (id, s) in &vs.slots {
            if keep.contains(id) {
                w[s.off..s.off + s.len].iter_mut().for_each(|v| *v = kept);
            }
        }
        w
    };
    let (mut ok, mut x) = gauss_newton(&vs, &cs, &x0, &weights(0.0), tol);
    if !ok && !keep.is_empty() {
        (ok, x) = gauss_newton(&vs, &cs, &x0, &weights(1e-6), tol);
    }
    let m = all_residuals(&vs, &x, &cs).len();
    let dof = n - rank(jacobian(&vs, &x, &cs, m), m, n).min(n);
    if !ok {
        let worst = cs.iter().find(|c| {
            let mut r = vec![];
            residuals(&vs, &x, c, &mut r);
            r.iter().any(|v| v.abs() > tol * 1e3)
        });
        let what = worst.map(|c| if c.name.is_empty() { c.kind.label().to_string() } else { format!("{} ({})", c.kind.label(), c.name) }).unwrap_or_default();
        return Solved { ok: false, moved: 0, dof, message: format!("The constraints cannot all be satisfied{}.", if what.is_empty() { String::new() } else { format!("; check {what}") }) };
    }
    // a solution that shrinks a line or circle to nothing is a conflict (horizontal and vertical on one line...)
    let collapsed = vs.slots.values().any(|s| {
        let size_of = |v: &[f64]| match s.shape {
            Shape::Line => dist(pt(v[0], v[1]), pt(v[2], v[3])),
            Shape::Circle | Shape::Arc => v[2].abs(),
            _ => f64::INFINITY,
        };
        let (old, new) = (size_of(&x0[s.off..s.off + s.len]), size_of(&x[s.off..s.off + s.len]));
        old > 1e-6 * size && new <= 1e-6 * size
    });
    if collapsed {
        return Solved { ok: false, moved: 0, dof, message: "The constraints conflict: they would shrink an object to nothing.".into() };
    }
    let mut moved = 0;
    for e in d.model.entities.iter_mut() {
        if let Some(s) = vs.slots.get(&e.id) {
            let new = &x[s.off..s.off + s.len];
            if new.iter().zip(&x0[s.off..s.off + s.len]).any(|(a, b)| (a - b).abs() > 1e-12) {
                store(e, new);
                moved += 1;
            }
        }
    }
    Solved { ok: true, moved, dof, message: String::new() }
}

/// The degrees of freedom left in the constrained objects (0: fully constrained), and how many objects there are.
pub fn degrees_of_freedom(d: &Drawing) -> (usize, usize) {
    let cs = &d.model.constraints.list;
    let vs = load(d, cs);
    let n = vs.x.len();
    let m = all_residuals(&vs, &vs.x, cs).len();
    if m == 0 {
        return (n, vs.slots.len());
    }
    (n - rank(jacobian(&vs, &vs.x, cs, m), m, n).min(n), vs.slots.len())
}

// ---------------------------------------------------------------- choosing what a pick refers to

/// The part of `e` nearest `p` that a point constraint can use (an end, the middle, a centre or a vertex).
pub fn point_ref_near(e: &Entity, p: Pt) -> Option<PRef> {
    let id = e.id;
    let cands: Vec<(PRef, Pt)> = match &e.kind {
        Kind::Line { a, b } => vec![(PRef::new(id, At::Start, 0), *a), (PRef::new(id, At::End, 0), *b), (PRef::new(id, At::Mid, 0), mid(*a, *b))],
        Kind::Circle { c, .. } => vec![(PRef::new(id, At::Center, 0), *c)],
        Kind::Arc { c, r, a0, a1 } => {
            let m = a0 + sweep_ccw(*a0, *a1) / 2.0;
            vec![(PRef::new(id, At::Center, 0), *c), (PRef::new(id, At::Start, 0), polar(*c, *a0, *r)), (PRef::new(id, At::End, 0), polar(*c, *a1, *r)), (PRef::new(id, At::Mid, 0), polar(*c, m, *r))]
        }
        Kind::Pline { v, closed } => {
            let mut c: Vec<(PRef, Pt)> = v.iter().enumerate().map(|(i, q)| (PRef::new(id, At::Vertex, i), pt(q.x, q.y))).collect();
            let segs = if *closed { v.len() } else { v.len() - 1 };
            for i in 0..segs {
                let (a, b) = (v[i], v[(i + 1) % v.len()]);
                c.push((PRef::new(id, At::Mid, i), mid(pt(a.x, a.y), pt(b.x, b.y))));
            }
            c
        }
        Kind::Point { p } => vec![(PRef::new(id, At::Center, 0), *p)],
        _ => return None,
    };
    cands.into_iter().min_by(|a, b| dist(a.1, p).total_cmp(&dist(b.1, p))).map(|c| c.0)
}

/// The line `e` is (a line, or the polyline segment nearest `p`), as a reference.
pub fn line_ref_near(e: &Entity, p: Pt) -> Option<PRef> {
    match &e.kind {
        Kind::Line { .. } => Some(PRef::new(e.id, At::Whole, 0)),
        Kind::Pline { v, closed } if v.len() >= 2 => {
            let segs = if *closed { v.len() } else { v.len() - 1 };
            (0..segs)
                .min_by(|i, j| {
                    let d = |k: usize| dist_to_segs(&[Seg::line(pt(v[k].x, v[k].y), pt(v[(k + 1) % v.len()].x, v[(k + 1) % v.len()].y))], p).0;
                    d(*i).total_cmp(&d(*j))
                })
                .map(|i| PRef::new(e.id, At::Whole, i))
        }
        _ => None,
    }
}

/// A circle or arc as a reference.
pub fn circle_ref(e: &Entity) -> Option<PRef> {
    matches!(e.kind, Kind::Circle { .. } | Kind::Arc { .. }).then(|| PRef::new(e.id, At::Whole, 0))
}

/// The current value of what a dimensional constraint measures (degrees for angles).
pub fn measure(d: &Drawing, kind: CKind, refs: &[PRef]) -> Option<f64> {
    let cs = [Constraint { id: 0, kind, refs: refs.to_vec(), name: String::new(), expr: String::new(), value: 0.0, hold: vec![], off: 0.0 }];
    let vs = load(d, &cs);
    let mut r = vec![];
    residuals(&vs, &vs.x, &cs[0], &mut r)?;
    match kind {
        CKind::Angle => {
            let (d1, d2) = (vs.line(&vs.x, &refs[0])?, vs.line(&vs.x, &refs[1])?);
            let (u, v) = (d1.1 - d1.0, d2.1 - d2.0);
            let a = wrap(v.y.atan2(v.x) - u.y.atan2(u.x)).abs().to_degrees();
            Some(if a > 90.0 + 1e-9 { 180.0 - a } else { a }.max(0.0))
        }
        _ => r.first().copied(),
    }
}

/// The values a Fix constraint on `r` holds now.
pub fn fix_hold(d: &Drawing, r: &PRef) -> Option<Vec<f64>> {
    let cs = [Constraint { id: 0, kind: CKind::Fix, refs: vec![*r], name: String::new(), expr: String::new(), value: 0.0, hold: vec![], off: 0.0 }];
    let vs = load(d, &cs);
    vs.fix_values(&vs.x, r)
}

/// Make a constraint (its id and, for a dimensional one, its name and the current value as its expression).
pub fn make(d: &Drawing, kind: CKind, refs: Vec<PRef>) -> Option<Constraint> {
    let cs = &d.model.constraints;
    let mut c = Constraint { id: cs.next_id(), kind, refs, name: String::new(), expr: String::new(), value: 0.0, hold: vec![], off: 0.0 };
    if kind == CKind::Fix {
        c.hold = fix_hold(d, c.refs.first()?)?;
    }
    if kind.dimensional() {
        let v = measure(d, kind, &c.refs)?;
        c.name = cs.next_name(kind);
        c.value = v;
        c.expr = fmt_value(v);
    }
    Some(c)
}

/// A value as an expression: up to four decimals, without trailing zeros.
pub fn fmt_value(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

// ---------------------------------------------------------------- AUTOCONSTRAIN

/// Constraints that the selected objects already satisfy (within `tol` drawing units and one degree): coincident
/// ends, horizontal and vertical lines, parallel and perpendicular pairs, collinear lines, concentric circles and
/// tangent lines. Constraints that are already there, or follow from others, are left out.
pub fn infer(d: &Drawing, ids: &[u64], tol: f64) -> Vec<(CKind, Vec<PRef>)> {
    let ents: Vec<&Entity> = d.model.entities.iter().filter(|e| ids.contains(&e.id) && constrainable(e)).collect();
    type Key = (CKind, Vec<(u64, u8, usize)>);
    let have: HashSet<Key> = d.model.constraints.list.iter().map(|c| (c.kind, c.refs.iter().map(|r| (r.ent, r.at as u8, r.i)).collect())).collect();
    let mut out: Vec<(CKind, Vec<PRef>)> = vec![];
    let ang_tol = 1f64.to_radians();
    let push = |k: CKind, refs: Vec<PRef>, out: &mut Vec<(CKind, Vec<PRef>)>| {
        let key: Vec<(u64, u8, usize)> = refs.iter().map(|r| (r.ent, r.at as u8, r.i)).collect();
        let mut rev = key.clone();
        rev.reverse();
        if !have.contains(&(k, key.clone())) && !have.contains(&(k, rev)) {
            out.push((k, refs));
        }
    };
    // the lines (and polyline segments) with their direction
    let mut lines: Vec<(PRef, Pt, Pt)> = vec![];
    let mut ends: Vec<(PRef, Pt)> = vec![];
    let mut circles: Vec<(PRef, Pt, f64)> = vec![];
    for e in &ents {
        match &e.kind {
            Kind::Line { a, b } => {
                lines.push((PRef::new(e.id, At::Whole, 0), *a, *b));
                ends.push((PRef::new(e.id, At::Start, 0), *a));
                ends.push((PRef::new(e.id, At::End, 0), *b));
            }
            Kind::Pline { v, closed } => {
                let segs = if *closed { v.len() } else { v.len() - 1 };
                for i in 0..segs {
                    let (a, b) = (v[i], v[(i + 1) % v.len()]);
                    if a.b.abs() < 1e-9 {
                        lines.push((PRef::new(e.id, At::Whole, i), pt(a.x, a.y), pt(b.x, b.y)));
                    }
                }
            }
            Kind::Arc { c, r, a0, a1 } => {
                circles.push((PRef::new(e.id, At::Whole, 0), *c, *r));
                ends.push((PRef::new(e.id, At::Start, 0), polar(*c, *a0, *r)));
                ends.push((PRef::new(e.id, At::End, 0), polar(*c, *a1, *r)));
            }
            Kind::Circle { c, r } => circles.push((PRef::new(e.id, At::Whole, 0), *c, *r)),
            _ => {}
        }
    }
    // coincident ends of different objects (each end joins the first end it meets)
    let mut joined: HashSet<usize> = HashSet::new();
    for i in 0..ends.len() {
        for j in i + 1..ends.len() {
            if ends[i].0.ent != ends[j].0.ent && !joined.contains(&j) && dist(ends[i].1, ends[j].1) <= tol {
                push(CKind::Coincident, vec![ends[i].0, ends[j].0], &mut out);
                joined.insert(j);
                break;
            }
        }
    }
    let angle = |a: Pt, b: Pt| {
        let v = b - a;
        v.y.atan2(v.x)
    };
    let mut hv: HashMap<usize, CKind> = HashMap::new();
    for (i, (r, a, b)) in lines.iter().enumerate() {
        if dist(*a, *b) <= tol {
            continue;
        }
        let t = wrap(angle(*a, *b));
        if wrap(t).abs() <= ang_tol || (PI - t.abs()) <= ang_tol {
            push(CKind::Horizontal, vec![*r], &mut out);
            hv.insert(i, CKind::Horizontal);
        } else if (t.abs() - PI / 2.0).abs() <= ang_tol {
            push(CKind::Vertical, vec![*r], &mut out);
            hv.insert(i, CKind::Vertical);
        }
    }
    for i in 0..lines.len() {
        for j in i + 1..lines.len() {
            let (ri, ai, bi) = lines[i];
            let (rj, aj, bj) = lines[j];
            if dist(ai, bi) <= tol || dist(aj, bj) <= tol {
                continue;
            }
            let t = wrap(angle(ai, bi) - angle(aj, bj));
            let parallel = t.abs() <= ang_tol || (PI - t.abs()) <= ang_tol;
            let perpendicular = (t.abs() - PI / 2.0).abs() <= ang_tol;
            // horizontal and vertical lines need nothing more between them
            let both_hv = hv.contains_key(&i) && hv.contains_key(&j);
            if parallel {
                let u = unit(bi - ai);
                let off = cross(u, aj - ai).abs().max(cross(u, bj - ai).abs());
                if off <= tol {
                    push(CKind::Collinear, vec![ri, rj], &mut out);
                } else if !both_hv && ri.ent != rj.ent {
                    push(CKind::Parallel, vec![ri, rj], &mut out);
                }
            } else if perpendicular && !both_hv {
                // only lines that meet (adjacent sides), as AutoCAD infers them
                let meet = [ai, bi].iter().any(|p| dist(*p, aj) <= tol || dist(*p, bj) <= tol);
                if meet {
                    push(CKind::Perpendicular, vec![ri, rj], &mut out);
                }
            }
        }
    }
    for i in 0..circles.len() {
        for j in i + 1..circles.len() {
            if dist(circles[i].1, circles[j].1) <= tol {
                push(CKind::Concentric, vec![circles[i].0, circles[j].0], &mut out);
            }
        }
    }
    for (rl, a, b) in &lines {
        for (rc, c, r) in &circles {
            let u = unit(*b - *a);
            let along = dot(*c - *a, u);
            let touches_inside = along >= -tol && along <= dist(*a, *b) + tol;
            if touches_inside && (cross(u, *c - *a).abs() - r).abs() <= tol {
                push(CKind::Tangent, vec![*rl, *rc], &mut out);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- display

/// What the constraints look like on screen, as objects to draw over the drawing: a dimension for each dimensional
/// constraint (`d1=50`) and a small boxed symbol next to the geometry for each geometric one. `px` is the size of a
/// screen pixel in drawing units, so the symbols keep their size as the view zooms.
pub fn display(d: &Drawing, px: f64) -> Vec<Entity> {
    let cs = &d.model.constraints;
    display_of(d, &cs.list, px, cs.hide_bars, cs.hide_dims)
}

/// [`display`] for a given list of constraints (a command previews the one it is making this way).
pub fn display_of(d: &Drawing, list: &[Constraint], px: f64, hide_bars: bool, hide_dims: bool) -> Vec<Entity> {
    if list.is_empty() {
        return vec![];
    }
    let vs = load(d, list);
    let x = &vs.x;
    let mut out = vec![];
    let dim_col = crate::style::Color::Rgb(0x5c, 0xb8, 0xff);
    let bar_col = crate::style::Color::Rgb(0xb8, 0xc2, 0xcf);
    // geometric symbols: several on one place stack along it
    let mut used: HashMap<(i64, i64), usize> = HashMap::new();
    let h = 11.0 * px;
    for c in list {
        if c.kind.dimensional() {
            if hide_dims {
                continue;
            }
            let label = format!("{}={}", c.name, fmt_value(c.value));
            let mut dd = match c.kind {
                CKind::Radius | CKind::Diameter => {
                    let Some((cc, r)) = c.refs.first().and_then(|r| vs.circle(x, r)) else { continue };
                    let a = PI / 4.0;
                    let mut dd = DimData::new(if c.kind == CKind::Radius { DimType::Rad } else { DimType::Dia });
                    dd.c = Some(cc);
                    dd.pt = Some(polar(cc, a, r.abs()));
                    dd.dp = Some(polar(cc, a, r.abs() + c.off.max(18.0 * px)));
                    dd
                }
                CKind::Angle => {
                    let (Some(l1), Some(l2)) = (c.refs.first().and_then(|r| vs.line(x, r)), c.refs.get(1).and_then(|r| vs.line(x, r))) else { continue };
                    let Some(v) = line_meet(l1, l2) else { continue };
                    let far = |l: (Pt, Pt)| if dist(l.0, v) > dist(l.1, v) { l.0 } else { l.1 };
                    let (p1, p2) = (far(l1), far(l2));
                    let rr = (dist(p1, v).min(dist(p2, v)) * 0.6).max(30.0 * px);
                    let bis = unit(unit(p1 - v) + unit(p2 - v));
                    let mut dd = DimData::new(DimType::Ang);
                    dd.c = Some(v);
                    dd.p1 = Some(p1);
                    dd.p2 = Some(p2);
                    dd.dp = Some(v + if len(bis) > 0.5 { bis } else { perp(unit(p1 - v)) } * rr);
                    dd
                }
                _ => {
                    let pts = if c.refs.len() >= 2 { c.refs.first().and_then(|r| vs.point(x, r)).zip(c.refs.get(1).and_then(|r| vs.point(x, r))) } else { c.refs.first().and_then(|r| vs.line(x, r)) };
                    let Some((a, b)) = pts else { continue };
                    let off = if c.off.abs() > 1e-12 { c.off } else { 20.0 * px };
                    let mut dd = DimData::new(if c.kind == CKind::Distance { DimType::Ali } else { DimType::Lin });
                    dd.p1 = Some(a);
                    dd.p2 = Some(b);
                    match c.kind {
                        CKind::HDistance => {
                            dd.ang = Some(0.0);
                            dd.dp = Some(pt(mid(a, b).x, a.y.max(b.y) + off));
                        }
                        CKind::VDistance => {
                            dd.ang = Some(PI / 2.0);
                            dd.dp = Some(pt(a.x.max(b.x) + off, mid(a, b).y));
                        }
                        _ => {
                            let n = if dist(a, b) > 1e-12 { perp(unit(b - a)) } else { pt(0.0, 1.0) };
                            dd.dp = Some(mid(a, b) + n * off);
                        }
                    }
                    dd
                }
            };
            dd.txt = Some(label);
            // dimensional constraints keep their size on screen
            dd.sc = Some(px * 12.0 / d.settings.cur_dim().txt.max(1e-9) / d.settings.cur_dim().scale.max(1e-9));
            let mut e = Entity::dim(dd);
            e.color = Some(dim_col);
            out.push(e);
            continue;
        }
        if hide_bars {
            continue;
        }
        // where the symbol goes: beside the first object it constrains
        let Some(r) = c.refs.first() else { continue };
        let at = match (vs.line(x, r), vs.circle(x, r), vs.point(x, r)) {
            (Some(l), _, _) if r.at == At::Whole => {
                let m = mid(l.0, l.1);
                let n = if dist(l.0, l.1) > 1e-12 { perp(unit(l.1 - l.0)) } else { pt(0.0, 1.0) };
                m + n * (12.0 * px)
            }
            (_, Some((cc, rad)), _) if r.at == At::Whole => polar(cc, -PI / 4.0, rad.abs()) + pt(10.0 * px, -10.0 * px),
            (_, _, Some(p)) => p + pt(10.0 * px, 10.0 * px),
            _ => continue,
        };
        let key = ((at.x / (16.0 * px)).round() as i64, (at.y / (16.0 * px)).round() as i64);
        let k = used.entry(key).or_insert(0);
        let at = at + pt(*k as f64 * 15.0 * px, 0.0);
        *k += 1;
        let mut t = TextData::new(at, h * 0.8, c.kind.glyph());
        t.jh = crate::text::HJust::Center;
        t.jv = crate::text::VJust::Middle;
        let mut te = Entity::new(Kind::Text(t));
        te.color = Some(bar_col);
        out.push(te);
        let b = 7.0 * px;
        let mut bx = Entity::pline_pts(&[at + pt(-b, -b), at + pt(b, -b), at + pt(b, b), at + pt(-b, b)], true);
        bx.color = Some(bar_col);
        out.push(bx);
    }
    out
}

/// Where two lines (as infinite lines) cross.
fn line_meet(l1: (Pt, Pt), l2: (Pt, Pt)) -> Option<Pt> {
    let (d1, d2) = (l1.1 - l1.0, l2.1 - l2.0);
    let den = cross(d1, d2);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = cross(l2.0 - l1.0, d2) / den;
    Some(l1.0 + d1 * t)
}
