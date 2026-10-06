//! Pure 2D geometry (port of `src/core/geom.js`).
//!
//! A segment is either a line `{a, b}` or a circular arc `{c, r, a0, sw}` (start angle `a0`, signed sweep `sw`;
//! `sw > 0` is counter-clockwise). Polylines, arcs, circles and spline tessellations are all reduced to segments,
//! so intersection, trim, offset and snapping share one implementation.
use serde::{Deserialize, Serialize};

pub const TAU: f64 = std::f64::consts::PI * 2.0;
pub const PI: f64 = std::f64::consts::PI;
pub const EPS: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

pub const fn pt(x: f64, y: f64) -> Pt {
    Pt { x, y }
}

impl Pt {
    pub const ZERO: Pt = Pt { x: 0.0, y: 0.0 };
    pub fn new(x: f64, y: f64) -> Pt {
        Pt { x, y }
    }
}

impl std::ops::Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        pt(self.x + o.x, self.y + o.y)
    }
}
impl std::ops::Sub for Pt {
    type Output = Pt;
    fn sub(self, o: Pt) -> Pt {
        pt(self.x - o.x, self.y - o.y)
    }
}
impl std::ops::Mul<f64> for Pt {
    type Output = Pt;
    fn mul(self, s: f64) -> Pt {
        pt(self.x * s, self.y * s)
    }
}
impl std::ops::Neg for Pt {
    type Output = Pt;
    fn neg(self) -> Pt {
        pt(-self.x, -self.y)
    }
}

pub fn dist(a: Pt, b: Pt) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}
pub fn dot(a: Pt, b: Pt) -> f64 {
    a.x * b.x + a.y * b.y
}
pub fn cross(a: Pt, b: Pt) -> f64 {
    a.x * b.y - a.y * b.x
}
pub fn len(a: Pt) -> f64 {
    a.x.hypot(a.y)
}
pub fn unit(a: Pt) -> Pt {
    let l = len(a);
    if l < EPS {
        pt(1.0, 0.0)
    } else {
        pt(a.x / l, a.y / l)
    }
}
pub fn lerp(a: Pt, b: Pt, t: f64) -> Pt {
    pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}
pub fn mid(a: Pt, b: Pt) -> Pt {
    pt((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}
pub fn perp(a: Pt) -> Pt {
    pt(-a.y, a.x)
}
pub fn polar(c: Pt, ang: f64, d: f64) -> Pt {
    pt(c.x + d * ang.cos(), c.y + d * ang.sin())
}
pub fn angle_of(from: Pt, to: Pt) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}
/// Relative closeness of two numbers.
pub fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * 1f64.max(a.abs()).max(b.abs())
}
pub fn same_pt(a: Pt, b: Pt, tol: f64) -> bool {
    near(a.x, b.x, tol) && near(a.y, b.y, tol)
}
pub fn same(a: Pt, b: Pt) -> bool {
    same_pt(a, b, 1e-7)
}
pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}
pub fn deg(r: f64) -> f64 {
    r * 180.0 / PI
}
pub fn rad(d: f64) -> f64 {
    d * PI / 180.0
}

pub fn norm_angle(a: f64) -> f64 {
    let a = a % TAU;
    if a < 0.0 {
        a + TAU
    } else {
        a
    }
}
pub fn norm_angle_signed(a: f64) -> f64 {
    let a = norm_angle(a);
    if a > PI {
        a - TAU
    } else {
        a
    }
}
/// CCW sweep from a0 to a1 in [0, 2π).
pub fn sweep_ccw(a0: f64, a1: f64) -> f64 {
    norm_angle(a1 - a0)
}

// ---------------------------------------------------------------- bounds

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Bounds {
        Bounds { min_x, min_y, max_x, max_y }
    }
    pub fn of_points(pts: &[Pt]) -> Bounds {
        let mut b = Bounds::new(f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in pts {
            b.min_x = b.min_x.min(p.x);
            b.min_y = b.min_y.min(p.y);
            b.max_x = b.max_x.max(p.x);
            b.max_y = b.max_y.max(p.y);
        }
        b
    }
    pub fn union(self, o: Bounds) -> Bounds {
        Bounds::new(self.min_x.min(o.min_x), self.min_y.min(o.min_y), self.max_x.max(o.max_x), self.max_y.max(o.max_y))
    }
    pub fn union_opt(a: Option<Bounds>, b: Option<Bounds>) -> Option<Bounds> {
        match (a, b) {
            (None, b) => b,
            (a, None) => a,
            (Some(a), Some(b)) => Some(a.union(b)),
        }
    }
    pub fn overlaps(&self, o: &Bounds) -> bool {
        self.min_x <= o.max_x && self.max_x >= o.min_x && self.min_y <= o.max_y && self.max_y >= o.min_y
    }
    pub fn inside(&self, outer: &Bounds) -> bool {
        self.min_x >= outer.min_x && self.max_x <= outer.max_x && self.min_y >= outer.min_y && self.max_y <= outer.max_y
    }
    pub fn grow(&self, d: f64) -> Bounds {
        Bounds::new(self.min_x - d, self.min_y - d, self.max_x + d, self.max_y + d)
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.min_x && p.x <= self.max_x && p.y >= self.min_y && p.y <= self.max_y
    }
    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }
    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
    pub fn center(&self) -> Pt {
        pt((self.min_x + self.max_x) / 2.0, (self.min_y + self.max_y) / 2.0)
    }
    pub fn is_valid(&self) -> bool {
        self.min_x.is_finite() && self.max_x >= self.min_x && self.max_y >= self.min_y
    }
}

// ---------------------------------------------------------------- segments

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    /// Straight segment; `approx` marks a piece of a tessellated curve (not snappable).
    L { a: Pt, b: Pt, approx: bool },
    /// Circular arc; `full` marks a complete circle.
    A { c: Pt, r: f64, a0: f64, sw: f64, full: bool },
}

impl Seg {
    pub fn line(a: Pt, b: Pt) -> Seg {
        Seg::L { a, b, approx: false }
    }
    pub fn arc(c: Pt, r: f64, a0: f64, sw: f64) -> Seg {
        Seg::A { c, r, a0, sw, full: false }
    }
    pub fn is_line(&self) -> bool {
        matches!(self, Seg::L { .. })
    }
    pub fn is_full(&self) -> bool {
        matches!(self, Seg::A { full: true, .. })
    }
    pub fn is_approx(&self) -> bool {
        matches!(self, Seg::L { approx: true, .. })
    }
    pub fn start(&self) -> Pt {
        match *self {
            Seg::L { a, .. } => a,
            Seg::A { c, r, a0, .. } => polar(c, a0, r),
        }
    }
    pub fn end(&self) -> Pt {
        match *self {
            Seg::L { b, .. } => b,
            Seg::A { c, r, a0, sw, .. } => polar(c, a0 + sw, r),
        }
    }
    pub fn length(&self) -> f64 {
        match *self {
            Seg::L { a, b, .. } => dist(a, b),
            Seg::A { r, sw, .. } => r * sw.abs(),
        }
    }
    /// Point at parameter t (0 start, 1 end).
    pub fn at(&self, t: f64) -> Pt {
        match *self {
            Seg::L { a, b, .. } => lerp(a, b, t),
            Seg::A { c, r, a0, sw, .. } => polar(c, a0 + sw * t, r),
        }
    }
    pub fn reverse(&self) -> Seg {
        match *self {
            Seg::L { a, b, approx } => Seg::L { a: b, b: a, approx },
            Seg::A { c, r, a0, sw, full } => Seg::A { c, r, a0: a0 + sw, sw: -sw, full },
        }
    }
    /// Portion of the segment between params t0 and t1.
    pub fn slice(&self, t0: f64, t1: f64) -> Seg {
        match *self {
            Seg::L { a, b, .. } => Seg::line(lerp(a, b, t0), lerp(a, b, t1)),
            Seg::A { c, r, a0, sw, .. } => Seg::arc(c, r, a0 + sw * t0, sw * (t1 - t0)),
        }
    }
    pub fn bounds(&self) -> Bounds {
        match *self {
            Seg::L { a, b, .. } => Bounds::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)),
            Seg::A { c, r, a0, sw, .. } => {
                let mut pts = vec![self.start(), self.end()];
                for k in 0..4 {
                    let a = k as f64 * PI / 2.0;
                    if angle_in_arc(a, a0, sw, 1e-9) {
                        pts.push(polar(c, a, r));
                    }
                }
                Bounds::of_points(&pts)
            }
        }
    }
    /// Tangent direction (unit) at param t.
    pub fn tangent(&self, t: f64) -> Pt {
        match *self {
            Seg::L { a, b, .. } => unit(b - a),
            Seg::A { a0, sw, .. } => {
                let a = a0 + sw * t;
                let d = if sw >= 0.0 { 1.0 } else { -1.0 };
                pt(-a.sin() * d, a.cos() * d)
            }
        }
    }
    /// Bulge (tan(sweep/4)) of an arc, 0 for lines.
    pub fn bulge(&self) -> f64 {
        match *self {
            Seg::A { sw, .. } => (sw / 4.0).tan(),
            _ => 0.0,
        }
    }
    /// Parameter of the point on the (infinite) carrier closest to p; not clamped.
    pub fn project(&self, p: Pt) -> f64 {
        match *self {
            Seg::L { a, b, .. } => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let l2 = dx * dx + dy * dy;
                if l2 < 1e-24 {
                    0.0
                } else {
                    ((p.x - a.x) * dx + (p.y - a.y) * dy) / l2
                }
            }
            Seg::A { c, .. } => arc_param(angle_of(c, p), self),
        }
    }
    /// Nearest point on the segment: (point, t, distance), t clamped to [0,1].
    pub fn nearest(&self, p: Pt) -> (Pt, f64, f64) {
        match *self {
            Seg::L { a, b, .. } => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let l2 = dx * dx + dy * dy;
                let t = if l2 < 1e-24 { 0.0 } else { clamp(((p.x - a.x) * dx + (p.y - a.y) * dy) / l2, 0.0, 1.0) };
                let q = pt(a.x + dx * t, a.y + dy * t);
                (q, t, dist(p, q))
            }
            Seg::A { c, r, .. } => {
                let ang = angle_of(c, p);
                let t = arc_param(ang, self);
                if !(0.0..=1.0).contains(&t) {
                    let (e0, e1) = (self.start(), self.end());
                    return if dist(p, e0) <= dist(p, e1) { (e0, 0.0, dist(p, e0)) } else { (e1, 1.0, dist(p, e1)) };
                }
                if dist(p, c) < EPS {
                    return (self.start(), 0.0, r);
                }
                let q = polar(c, ang, r);
                (q, t, dist(p, q))
            }
        }
    }
    /// Offset to the left by d (negative = right). None if an arc collapses.
    pub fn offset(&self, d: f64) -> Option<Seg> {
        match *self {
            Seg::L { a, b, .. } => {
                let u = unit(b - a);
                let n = pt(-u.y * d, u.x * d);
                Some(Seg::line(a + n, b + n))
            }
            Seg::A { c, r, a0, sw, .. } => {
                let nr = if sw >= 0.0 { r - d } else { r + d };
                if nr < 1e-9 {
                    None
                } else {
                    Some(Seg::arc(c, nr, a0, sw))
                }
            }
        }
    }
    pub fn with_flags_from(self, o: &Seg) -> Seg {
        match (self, o) {
            (Seg::L { a, b, .. }, Seg::L { approx, .. }) => Seg::L { a, b, approx: *approx },
            (Seg::A { c, r, a0, sw, .. }, Seg::A { full, .. }) => Seg::A { c, r, a0, sw, full: *full },
            (s, _) => s,
        }
    }
}

// ---------------------------------------------------------------- arcs

/// Is angle `a` inside the directed arc starting at a0 with signed sweep sw?
pub fn angle_in_arc(a: f64, a0: f64, sw: f64, tol: f64) -> bool {
    if sw.abs() >= TAU - tol {
        return true;
    }
    let d = if sw >= 0.0 { norm_angle(a - a0) } else { norm_angle(a0 - a) };
    let s = sw.abs();
    d <= s + tol || d >= TAU - tol
}

/// Param t of angle `a` along the directed arc (0 at start, 1 at end). Outside the arc t<0 or t>1.
pub fn arc_param(a: f64, arc: &Seg) -> f64 {
    let (a0, sw) = match *arc {
        Seg::A { a0, sw, .. } => (a0, sw),
        _ => return 0.0,
    };
    let s = sw.abs();
    if s < EPS {
        return 0.0;
    }
    let d = if sw >= 0.0 { norm_angle(a - a0) } else { norm_angle(a0 - a) };
    if d <= s {
        return d / s;
    }
    if d - s < TAU - d {
        d / s
    } else {
        (d - TAU) / s
    }
}

/// Arc through three points (start, any point on it, end), or None if collinear.
pub fn arc_through(p1: Pt, p2: Pt, p3: Pt) -> Option<Seg> {
    let (ax, ay, bx, by, cx, cy) = (p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (ax * ax + ay * ay, bx * bx + by * by, cx * cx + cy * cy);
    let c = pt((a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d, (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d);
    let r = dist(c, p1);
    let (a0, am, a1) = (angle_of(c, p1), angle_of(c, p2), angle_of(c, p3));
    let ccw = sweep_ccw(a0, am) < sweep_ccw(a0, a1);
    let sw = if ccw { sweep_ccw(a0, a1) } else { -sweep_ccw(a1, a0) };
    Some(Seg::arc(c, r, a0, sw))
}

/// Circle (centre, radius) through three points.
pub fn circle_through(p1: Pt, p2: Pt, p3: Pt) -> Option<(Pt, f64)> {
    match arc_through(p1, p2, p3) {
        Some(Seg::A { c, r, .. }) => Some((c, r)),
        _ => None,
    }
}

/// Segment of a polyline edge p1->p2 with the given bulge (tan(sweep/4)).
pub fn bulge_seg(p1: Pt, p2: Pt, bulge: f64) -> Seg {
    if bulge.abs() < 1e-12 {
        return Seg::line(p1, p2);
    }
    let chord = dist(p1, p2);
    if chord < EPS {
        return Seg::line(p1, p2);
    }
    let sw = 4.0 * bulge.atan();
    let r = chord / (2.0 * (sw.abs() / 2.0).sin());
    let (m, u) = (mid(p1, p2), unit(p2 - p1));
    let h = chord / 2.0 / (sw / 2.0).tan();
    let c = pt(m.x - u.y * h, m.y + u.x * h);
    Seg::arc(c, r, angle_of(c, p1), sw)
}

// ---------------------------------------------------------------- intersection

const TOL_T: f64 = 1e-9;
fn in_range(t: f64) -> bool {
    (-TOL_T..=1.0 + TOL_T).contains(&t)
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub pt: Pt,
    pub t1: f64,
    pub t2: f64,
}

/// Intersections of two segments. `ext1`/`ext2` treat that segment as its infinite carrier (line extension / full circle).
pub fn seg_intersect(s1: &Seg, s2: &Seg, ext1: bool, ext2: bool) -> Vec<Hit> {
    let hits = match (s1, s2) {
        (Seg::L { .. }, Seg::L { .. }) => line_line(s1, s2),
        (Seg::L { .. }, Seg::A { .. }) => line_arc(s1, s2),
        (Seg::A { .. }, Seg::L { .. }) => line_arc(s2, s1).into_iter().map(|h| Hit { pt: h.pt, t1: h.t2, t2: h.t1 }).collect(),
        _ => arc_arc(s1, s2),
    };
    hits.into_iter().filter(|h| (ext1 || in_range(h.t1)) && (ext2 || in_range(h.t2))).collect()
}

fn line_line(s1: &Seg, s2: &Seg) -> Vec<Hit> {
    let (a1, b1, a2, b2) = match (s1, s2) {
        (Seg::L { a: a1, b: b1, .. }, Seg::L { a: a2, b: b2, .. }) => (*a1, *b1, *a2, *b2),
        _ => return vec![],
    };
    let (r, q) = (b1 - a1, b2 - a2);
    let den = cross(r, q);
    if den.abs() < 1e-14 * 1f64.max(len(r) * len(q)) {
        return vec![];
    }
    let w = a2 - a1;
    let (t1, t2) = (cross(w, q) / den, cross(w, r) / den);
    vec![Hit { pt: pt(a1.x + r.x * t1, a1.y + r.y * t1), t1, t2 }]
}

fn line_arc(l: &Seg, a: &Seg) -> Vec<Hit> {
    let (la, lb) = match l {
        Seg::L { a, b, .. } => (*a, *b),
        _ => return vec![],
    };
    let (ac, ar) = match a {
        Seg::A { c, r, .. } => (*c, *r),
        _ => return vec![],
    };
    let d = lb - la;
    let f = la - ac;
    let aa = dot(d, d);
    if aa < 1e-24 {
        return vec![];
    }
    let bb = 2.0 * dot(f, d);
    let cc = dot(f, f) - ar * ar;
    let mut disc = bb * bb - 4.0 * aa * cc;
    let scale = 1f64.max(bb * bb);
    if disc < -1e-12 * scale {
        return vec![];
    }
    if disc < 0.0 {
        disc = 0.0;
    }
    let sq = disc.sqrt();
    let ts: Vec<f64> = if sq < 1e-9 * scale.sqrt() { vec![-bb / (2.0 * aa)] } else { vec![(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)] };
    ts.into_iter()
        .map(|t1| {
            let p = pt(la.x + d.x * t1, la.y + d.y * t1);
            Hit { pt: p, t1, t2: arc_param(angle_of(ac, p), a) }
        })
        .collect()
}

fn arc_arc(a: &Seg, b: &Seg) -> Vec<Hit> {
    let (ac, ar) = match a {
        Seg::A { c, r, .. } => (*c, *r),
        _ => return vec![],
    };
    let (bc, br) = match b {
        Seg::A { c, r, .. } => (*c, *r),
        _ => return vec![],
    };
    let d = dist(ac, bc);
    if d < EPS || d > ar + br + 1e-9 || d < (ar - br).abs() - 1e-9 {
        return vec![];
    }
    let x = (d * d + ar * ar - br * br) / (2.0 * d);
    let h2 = (ar * ar - x * x).max(0.0);
    let h = h2.sqrt();
    let u = unit(bc - ac);
    let base = ac + u * x;
    let pts = if h < 1e-9 { vec![base] } else { vec![base + perp(u) * h, base + perp(u) * -h] };
    pts.into_iter().map(|p| Hit { pt: p, t1: arc_param(angle_of(ac, p), a), t2: arc_param(angle_of(bc, p), b) }).collect()
}

// ---------------------------------------------------------------- tangent / perpendicular helpers

/// Foot(s) of the perpendicular from p onto the carrier: (point, t).
pub fn perp_feet(s: &Seg, p: Pt) -> Vec<(Pt, f64)> {
    match *s {
        Seg::L { a, b, .. } => {
            let t = s.project(p);
            vec![(lerp(a, b, t), t)]
        }
        Seg::A { c, r, .. } => {
            if dist(p, c) < EPS {
                return vec![];
            }
            let a = angle_of(c, p);
            [polar(c, a, r), polar(c, a + PI, r)].into_iter().map(|q| (q, arc_param(angle_of(c, q), s))).collect()
        }
    }
}

/// Tangent points on a circle/arc carrier from an external point: (point, t).
pub fn tangent_points(s: &Seg, p: Pt) -> Vec<(Pt, f64)> {
    match *s {
        Seg::A { c, r, .. } => {
            let d = dist(p, c);
            if d < r - 1e-9 {
                return vec![];
            }
            let base = angle_of(c, p);
            let off = clamp(r / d, -1.0, 1.0).acos();
            [base + off, base - off].into_iter().map(|a| (polar(c, a, r), arc_param(a, s))).collect()
        }
        _ => vec![],
    }
}

/// Circle centres tangent to two segment carriers with radius r (for FILLET / CIRCLE TTR).
pub fn tangent_centres(s1: &Seg, s2: &Seg, r: f64) -> Vec<Pt> {
    let carriers = |s: &Seg| -> Vec<Option<Seg>> {
        [-1.0, 1.0]
            .iter()
            .map(|k| match *s {
                Seg::L { .. } => s.offset(k * r),
                Seg::A { c, r: rr, a0, sw, .. } => Seg::A { c, r: rr, a0, sw: sw.abs(), full: false }.offset(k * r),
            })
            .collect()
    };
    let mut out = vec![];
    for o1 in carriers(s1).into_iter().flatten() {
        for o2 in carriers(s2).into_iter().flatten() {
            for h in seg_intersect(&o1, &o2, true, true) {
                out.push(h.pt);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- polylines

/// A polyline vertex with the bulge of the edge that starts here.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Vertex {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub b: f64,
}

impl Vertex {
    pub fn new(x: f64, y: f64, b: f64) -> Vertex {
        Vertex { x, y, b }
    }
    pub fn pt(&self) -> Pt {
        pt(self.x, self.y)
    }
}

impl From<Pt> for Vertex {
    fn from(p: Pt) -> Vertex {
        Vertex { x: p.x, y: p.y, b: 0.0 }
    }
}

/// Segments of a bulge polyline.
pub fn pline_segs(v: &[Vertex], closed: bool) -> Vec<Seg> {
    let n = v.len();
    let last = if closed { n } else { n.saturating_sub(1) };
    (0..last).map(|i| bulge_seg(v[i].pt(), v[(i + 1) % n].pt(), v[i].b)).collect()
}

/// Rebuild bulge-vertices from consecutive segments.
pub fn segs_to_verts(segs: &[Seg], closed: bool) -> Vec<Vertex> {
    let mut v: Vec<Vertex> = segs.iter().map(|s| Vertex::new(s.start().x, s.start().y, s.bulge())).collect();
    if !closed {
        if let Some(last) = segs.last() {
            let e = last.end();
            v.push(Vertex::new(e.x, e.y, 0.0));
        }
    }
    v
}

pub fn polygon_area(pts: &[Pt]) -> f64 {
    let mut s = 0.0;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        s += a.x * b.y - b.x * a.y;
    }
    s / 2.0
}

/// Signed area of a closed bulge polyline including arc segments (positive = CCW).
pub fn pline_area(v: &[Vertex]) -> f64 {
    let n = v.len();
    let pts: Vec<Pt> = v.iter().map(|p| p.pt()).collect();
    let mut area = polygon_area(&pts);
    for i in 0..n {
        let b = v[i].b;
        if b == 0.0 {
            continue;
        }
        let (a, c) = (v[i].pt(), v[(i + 1) % n].pt());
        let ch = dist(a, c);
        let sw = 4.0 * b.atan();
        let r = ch / (2.0 * (sw.abs() / 2.0).sin());
        let seg = r * r / 2.0 * (sw.abs() - sw.abs().sin());
        area += b.signum() * seg;
    }
    area
}

pub fn pline_length(v: &[Vertex], closed: bool) -> f64 {
    pline_segs(v, closed).iter().map(|s| s.length()).sum()
}

pub fn point_in_polygon(p: Pt, pts: &[Pt]) -> bool {
    let mut inside = false;
    let mut j = pts.len().wrapping_sub(1);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Number of tessellation steps for an arc.
pub fn arc_steps(r: f64, sweep: f64, tol: f64) -> usize {
    let max_step = if r > tol { 2.0 * clamp(1.0 - tol / r, -1.0, 1.0).acos() } else { PI / 2.0 };
    clamp((sweep / max_step.max(1e-3)).ceil(), 2.0, 2048.0) as usize
}

/// Tessellate a segment into points. `tol` is the maximum chord error in drawing units.
pub fn tess_seg(s: &Seg, tol: f64) -> Vec<Pt> {
    match *s {
        Seg::L { a, b, .. } => vec![a, b],
        Seg::A { c, r, a0, sw, .. } => {
            let n = arc_steps(r, sw.abs(), tol);
            (0..=n).map(|i| polar(c, a0 + sw * i as f64 / n as f64, r)).collect()
        }
    }
}

/// Concatenate tessellations of consecutive segments into one point list.
pub fn tess_segs(segs: &[Seg], tol: f64) -> Vec<Pt> {
    let mut out: Vec<Pt> = vec![];
    for s in segs {
        let pts = tess_seg(s, tol);
        let skip = if out.is_empty() { 0 } else { 1 };
        out.extend(pts.into_iter().skip(skip));
    }
    out
}

// ---------------------------------------------------------------- affine matrices  [a b c d e f]  (canvas convention)

pub type Mat = [f64; 6];

pub mod mat {
    use super::*;
    pub const IDENTITY: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    pub fn mul(m: &Mat, n: &Mat) -> Mat {
        [
            m[0] * n[0] + m[2] * n[1],
            m[1] * n[0] + m[3] * n[1],
            m[0] * n[2] + m[2] * n[3],
            m[1] * n[2] + m[3] * n[3],
            m[0] * n[4] + m[2] * n[5] + m[4],
            m[1] * n[4] + m[3] * n[5] + m[5],
        ]
    }
    pub fn translate(dx: f64, dy: f64) -> Mat {
        [1.0, 0.0, 0.0, 1.0, dx, dy]
    }
    pub fn rotate(ang: f64, c: Pt) -> Mat {
        let (co, si) = (ang.cos(), ang.sin());
        [co, si, -si, co, c.x - co * c.x + si * c.y, c.y - si * c.x - co * c.y]
    }
    pub fn scale(sx: f64, sy: f64, c: Pt) -> Mat {
        [sx, 0.0, 0.0, sy, c.x - sx * c.x, c.y - sy * c.y]
    }
    /// Reflection across the line through p1, p2.
    pub fn mirror(p1: Pt, p2: Pt) -> Mat {
        let u = unit(p2 - p1);
        let (c2, s2) = (u.x * u.x - u.y * u.y, 2.0 * u.x * u.y);
        [c2, s2, s2, -c2, p1.x - c2 * p1.x - s2 * p1.y, p1.y - s2 * p1.x + c2 * p1.y]
    }
    pub fn apply(m: &Mat, p: Pt) -> Pt {
        pt(m[0] * p.x + m[2] * p.y + m[4], m[1] * p.x + m[3] * p.y + m[5])
    }
    pub fn apply_vec(m: &Mat, p: Pt) -> Pt {
        pt(m[0] * p.x + m[2] * p.y, m[1] * p.x + m[3] * p.y)
    }
    pub fn det(m: &Mat) -> f64 {
        m[0] * m[3] - m[1] * m[2]
    }
    pub fn invert(m: &Mat) -> Mat {
        let d = det(m);
        [m[3] / d, -m[1] / d, -m[2] / d, m[0] / d, (m[2] * m[5] - m[3] * m[4]) / d, (m[1] * m[4] - m[0] * m[5]) / d]
    }
    /// Similarity = uniform scale + rotation (+ optional reflection).
    pub fn is_similarity(m: &Mat) -> bool {
        (m[0].hypot(m[1]) - m[2].hypot(m[3])).abs() < 1e-9 * 1f64.max(m[0].hypot(m[1])) && (m[0] * m[2] + m[1] * m[3]).abs() < 1e-9 * 1f64.max(m[0] * m[0] + m[1] * m[1])
    }
    pub fn scale_factor(m: &Mat) -> f64 {
        det(m).abs().sqrt()
    }
    pub fn angle(m: &Mat) -> f64 {
        m[1].atan2(m[0])
    }
}

/// Apply an affine matrix to a segment (arcs keep their shape under similarity transforms only).
pub fn transform_seg(s: &Seg, m: &Mat) -> Seg {
    match *s {
        Seg::L { a, b, approx } => Seg::L { a: mat::apply(m, a), b: mat::apply(m, b), approx },
        Seg::A { c, r, a0, sw, full } => {
            let mirrored = mat::det(m) < 0.0;
            let k = mat::scale_factor(m);
            let nc = mat::apply(m, c);
            let v = mat::apply_vec(m, pt(a0.cos(), a0.sin()));
            Seg::A { c: nc, r: r * k, a0: v.y.atan2(v.x), sw: if mirrored { -sw } else { sw }, full }
        }
    }
}

// ---------------------------------------------------------------- misc

/// Closest approach of a point to a list of segments: (distance, nearest point).
pub fn dist_to_segs(segs: &[Seg], p: Pt) -> (f64, Option<Pt>) {
    let mut best = f64::INFINITY;
    let mut best_pt = None;
    for s in segs {
        let (q, _, d) = s.nearest(p);
        if d < best {
            best = d;
            best_pt = Some(q);
        }
    }
    (best, best_pt)
}

/// Line through `a` with direction `d` clipped to the rectangle; returns the two end points or None.
pub fn clip_line_to_rect(a: Pt, d: Pt, r: &Bounds, both_ways: bool) -> Option<(Pt, Pt)> {
    let mut t0 = if both_ways { f64::NEG_INFINITY } else { 0.0 };
    let mut t1 = f64::INFINITY;
    for (p, q, lo, hi) in [(a.x, d.x, r.min_x, r.max_x), (a.y, d.y, r.min_y, r.max_y)] {
        if q.abs() < 1e-14 {
            if p < lo || p > hi {
                return None;
            }
            continue;
        }
        let (mut u0, mut u1) = ((lo - p) / q, (hi - p) / q);
        if u0 > u1 {
            std::mem::swap(&mut u0, &mut u1);
        }
        t0 = t0.max(u0);
        t1 = t1.min(u1);
        if t0 > t1 {
            return None;
        }
    }
    Some((pt(a.x + d.x * t0, a.y + d.y * t0), pt(a.x + d.x * t1, a.y + d.y * t1)))
}

pub fn seg_in_rect(s: &Seg, r: &Bounds) -> bool {
    s.bounds().inside(r)
}

/// Does a segment touch (cross or lie inside) a rectangle? Used for crossing selection.
pub fn seg_touches_rect(s: &Seg, r: &Bounds) -> bool {
    if !s.bounds().overlaps(r) {
        return false;
    }
    if r.contains(s.start()) || r.contains(s.end()) {
        return true;
    }
    let edges = [
        Seg::line(pt(r.min_x, r.min_y), pt(r.max_x, r.min_y)),
        Seg::line(pt(r.max_x, r.min_y), pt(r.max_x, r.max_y)),
        Seg::line(pt(r.max_x, r.max_y), pt(r.min_x, r.max_y)),
        Seg::line(pt(r.min_x, r.max_y), pt(r.min_x, r.min_y)),
    ];
    edges.iter().any(|e| !seg_intersect(s, e, false, false).is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn angles_and_arcs() {
        assert_relative_eq!(norm_angle(-PI / 2.0), 1.5 * PI, epsilon = 1e-12);
        assert_relative_eq!(norm_angle_signed(1.5 * PI), -PI / 2.0, epsilon = 1e-12);
        assert_relative_eq!(sweep_ccw(0.1, 0.0), TAU - 0.1, epsilon = 1e-12);
        let arc = Seg::arc(pt(0.0, 0.0), 1.0, 0.0, PI);
        assert!(angle_in_arc(PI / 2.0, 0.0, PI, 1e-9) && !angle_in_arc(1.5 * PI, 0.0, PI, 1e-9));
        assert_relative_eq!(arc_param(PI / 2.0, &arc), 0.5, epsilon = 1e-12);
        let cw = Seg::arc(pt(0.0, 0.0), 1.0, PI, -PI);
        assert_relative_eq!(arc_param(PI / 2.0, &cw), 0.5, epsilon = 1e-12);
    }

    #[test]
    fn arc_through_three_points_and_bulge() {
        let a = arc_through(pt(1.0, 0.0), pt(0.0, 1.0), pt(-1.0, 0.0)).unwrap();
        match a {
            Seg::A { c, r, sw, .. } => {
                assert!(dist(c, Pt::ZERO) < 1e-9);
                assert_relative_eq!(r, 1.0, epsilon = 1e-9);
                assert_relative_eq!(sw, PI, epsilon = 1e-9);
            }
            _ => panic!(),
        }
        assert!(arc_through(pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)).is_none());
        let s = bulge_seg(pt(0.0, 0.0), pt(2.0, 0.0), 1.0);        // semicircle on the left of the chord
        match s {
            Seg::A { c, r, sw, .. } => {
                assert!(dist(c, pt(1.0, 0.0)) < 1e-9);
                assert_relative_eq!(r, 1.0, epsilon = 1e-9);
                assert_relative_eq!(sw, PI, epsilon = 1e-9);
            }
            _ => panic!(),
        }
        assert_relative_eq!(s.bulge(), 1.0, epsilon = 1e-9);
        let back = bulge_seg(pt(2.0, 0.0), pt(0.0, 0.0), -1.0);    // negative bulge: clockwise
        assert!(matches!(back, Seg::A { sw, .. } if sw < 0.0));
    }

    #[test]
    fn intersections() {
        let l1 = Seg::line(pt(0.0, 0.0), pt(10.0, 10.0));
        let l2 = Seg::line(pt(0.0, 10.0), pt(10.0, 0.0));
        let h = seg_intersect(&l1, &l2, false, false);
        assert_eq!(h.len(), 1);
        assert!(dist(h[0].pt, pt(5.0, 5.0)) < 1e-9);
        // parallel
        assert!(seg_intersect(&l1, &Seg::line(pt(0.0, 1.0), pt(10.0, 11.0)), true, true).is_empty());
        // extension only
        let short = Seg::line(pt(0.0, 0.0), pt(1.0, 0.0));
        let vert = Seg::line(pt(5.0, -1.0), pt(5.0, 1.0));
        assert!(seg_intersect(&short, &vert, false, false).is_empty());
        let ext = seg_intersect(&short, &vert, true, false);
        assert_eq!(ext.len(), 1);
        assert_relative_eq!(ext[0].t1, 5.0, epsilon = 1e-9);
        // line / circle
        let circle = Seg::A { c: pt(0.0, 0.0), r: 5.0, a0: 0.0, sw: TAU, full: true };
        let h = seg_intersect(&Seg::line(pt(-10.0, 0.0), pt(10.0, 0.0)), &circle, false, false);
        assert_eq!(h.len(), 2);
        // tangent line: single hit
        let t = seg_intersect(&Seg::line(pt(-10.0, 5.0), pt(10.0, 5.0)), &circle, false, false);
        assert_eq!(t.len(), 1);
        // arc / arc
        let c2 = Seg::A { c: pt(6.0, 0.0), r: 5.0, a0: 0.0, sw: TAU, full: true };
        let h = seg_intersect(&circle, &c2, false, false);
        assert_eq!(h.len(), 2);
        assert!(h.iter().all(|x| (x.pt.x - 3.0).abs() < 1e-9 && (x.pt.y.abs() - 4.0).abs() < 1e-9));
        // half arc excludes one of the hits
        let upper = Seg::arc(pt(0.0, 0.0), 5.0, 0.0, PI);
        assert_eq!(seg_intersect(&upper, &c2, false, false).len(), 1);
    }

    #[test]
    fn nearest_offset_and_tangents() {
        let l = Seg::line(pt(0.0, 0.0), pt(10.0, 0.0));
        let (p, t, d) = l.nearest(pt(4.0, 3.0));
        assert!(dist(p, pt(4.0, 0.0)) < 1e-12 && (t - 0.4).abs() < 1e-12 && (d - 3.0).abs() < 1e-12);
        assert_eq!(l.nearest(pt(-5.0, 1.0)).1, 0.0);
        match l.offset(2.0).unwrap() {
            Seg::L { a, b, .. } => assert!(dist(a, pt(0.0, 2.0)) < 1e-12 && dist(b, pt(10.0, 2.0)) < 1e-12),
            _ => panic!(),
        }
        let ccw = Seg::arc(Pt::ZERO, 5.0, 0.0, PI);
        assert!(matches!(ccw.offset(2.0), Some(Seg::A { r, .. }) if (r - 3.0).abs() < 1e-12));
        assert!(ccw.offset(6.0).is_none());
        let circle = Seg::A { c: pt(0.0, 0.0), r: 5.0, a0: 0.0, sw: TAU, full: true };
        let tp = tangent_points(&circle, pt(10.0, 0.0));
        assert_eq!(tp.len(), 2);
        for (q, _) in tp {
            assert!((dot(q - Pt::ZERO, q - pt(10.0, 0.0))).abs() < 1e-9);
        }
        // fillet centres between two perpendicular lines
        let x = Seg::line(pt(0.0, 0.0), pt(10.0, 0.0));
        let y = Seg::line(pt(0.0, 0.0), pt(0.0, 10.0));
        let cs = tangent_centres(&x, &y, 2.0);
        assert!(cs.iter().any(|c| dist(*c, pt(2.0, 2.0)) < 1e-9));
        assert_eq!(cs.len(), 4);
    }

    #[test]
    fn polylines_and_areas() {
        let sq = [Vertex::new(0.0, 0.0, 0.0), Vertex::new(4.0, 0.0, 0.0), Vertex::new(4.0, 3.0, 0.0), Vertex::new(0.0, 3.0, 0.0)];
        assert_relative_eq!(pline_area(&sq), 12.0, epsilon = 1e-12);
        assert_relative_eq!(pline_length(&sq, true), 14.0, epsilon = 1e-12);
        assert_eq!(pline_segs(&sq, false).len(), 3);
        // semicircle: two vertices with bulge 1
        let circle = [Vertex::new(1.0, 0.0, 1.0), Vertex::new(-1.0, 0.0, 1.0)];
        assert_relative_eq!(pline_area(&circle), PI, epsilon = 1e-9);
        assert_relative_eq!(pline_length(&circle, true), TAU, epsilon = 1e-9);
        let back = segs_to_verts(&pline_segs(&circle, true), true);
        assert_relative_eq!(back[0].b, 1.0, epsilon = 1e-9);
        assert!(point_in_polygon(pt(1.0, 1.0), &[pt(0.0, 0.0), pt(4.0, 0.0), pt(4.0, 3.0), pt(0.0, 3.0)]));
        assert!(!point_in_polygon(pt(5.0, 1.0), &[pt(0.0, 0.0), pt(4.0, 0.0), pt(4.0, 3.0), pt(0.0, 3.0)]));
    }

    #[test]
    fn tessellation_and_bounds() {
        let arc = Seg::arc(Pt::ZERO, 10.0, 0.0, PI);
        let pts = tess_seg(&arc, 0.01);
        assert!(pts.len() > 10);
        assert!(pts.iter().all(|p| (len(*p) - 10.0).abs() < 1e-9));
        let b = arc.bounds();
        assert_relative_eq!(b.max_y, 10.0, epsilon = 1e-9);
        assert_relative_eq!(b.min_x, -10.0, epsilon = 1e-9);
        assert!(b.min_y.abs() < 1e-9);
        let two = tess_segs(&[Seg::line(pt(0.0, 0.0), pt(1.0, 0.0)), Seg::line(pt(1.0, 0.0), pt(1.0, 1.0))], 0.01);
        assert_eq!(two.len(), 3);
    }

    #[test]
    fn matrices() {
        let r = mat::rotate(PI / 2.0, pt(1.0, 1.0));
        assert!(dist(mat::apply(&r, pt(2.0, 1.0)), pt(1.0, 2.0)) < 1e-12);
        let m = mat::mirror(pt(0.0, 0.0), pt(1.0, 1.0));
        assert!(dist(mat::apply(&m, pt(2.0, 0.0)), pt(0.0, 2.0)) < 1e-12);
        assert!(mat::det(&m) < 0.0);
        let s = mat::scale(2.0, 2.0, pt(1.0, 1.0));
        assert!(dist(mat::apply(&s, pt(2.0, 2.0)), pt(3.0, 3.0)) < 1e-12);
        let inv = mat::invert(&mat::mul(&r, &s));
        let p = pt(3.0, -2.0);
        assert!(dist(mat::apply(&inv, mat::apply(&mat::mul(&r, &s), p)), p) < 1e-12);
        assert!(mat::is_similarity(&r) && mat::is_similarity(&m) && !mat::is_similarity(&mat::scale(2.0, 1.0, Pt::ZERO)));
        // arcs under a mirror keep their shape but reverse their sweep
        let arc = Seg::arc(pt(0.0, 0.0), 2.0, 0.0, PI / 2.0);
        match transform_seg(&arc, &mat::mirror(pt(0.0, 0.0), pt(1.0, 0.0))) {
            Seg::A { c, r, a0, sw, .. } => {
                assert!(dist(c, Pt::ZERO) < 1e-12);
                assert_relative_eq!(r, 2.0, epsilon = 1e-12);
                assert_relative_eq!(a0, 0.0, epsilon = 1e-12);
                assert_relative_eq!(sw, -PI / 2.0, epsilon = 1e-12);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn clipping_and_rect_tests() {
        let r = Bounds::new(0.0, 0.0, 10.0, 10.0);
        let (p, q) = clip_line_to_rect(pt(5.0, 5.0), pt(1.0, 0.0), &r, true).unwrap();
        assert!(dist(p, pt(0.0, 5.0)) < 1e-9 && dist(q, pt(10.0, 5.0)) < 1e-9);
        let (p, _) = clip_line_to_rect(pt(5.0, 5.0), pt(1.0, 0.0), &r, false).unwrap();
        assert!(dist(p, pt(5.0, 5.0)) < 1e-9);
        assert!(clip_line_to_rect(pt(5.0, 20.0), pt(1.0, 0.0), &r, true).is_none());
        let crossing = Seg::line(pt(-5.0, 5.0), pt(15.0, 5.0));
        assert!(seg_touches_rect(&crossing, &r) && !seg_in_rect(&crossing, &r));
        assert!(seg_in_rect(&Seg::line(pt(1.0, 1.0), pt(2.0, 2.0)), &r));
        assert!(!seg_touches_rect(&Seg::line(pt(20.0, 20.0), pt(30.0, 30.0)), &r));
    }
}
