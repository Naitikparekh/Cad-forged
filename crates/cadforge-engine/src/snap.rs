//! Point acquisition: object snaps, polar/ortho tracking, object-snap tracking and grid snap
//! (port of `src/engine/snap.js`).
use crate::parse::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::{bounds_of, segs_of, snap_points_of, Kind, SnapType};
use cadforge_core::geom::*;
use std::rc::Rc;

pub const MAX_ACQUIRED: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    End,
    Mid,
    Cen,
    Nod,
    Qua,
    Int,
    Ins,
    Per,
    Tan,
    Nea,
    App,
}

impl SnapKind {
    pub fn code(self) -> &'static str {
        match self {
            SnapKind::End => "END",
            SnapKind::Mid => "MID",
            SnapKind::Cen => "CEN",
            SnapKind::Nod => "NOD",
            SnapKind::Qua => "QUA",
            SnapKind::Int => "INT",
            SnapKind::Ins => "INS",
            SnapKind::Per => "PER",
            SnapKind::Tan => "TAN",
            SnapKind::Nea => "NEA",
            SnapKind::App => "APP",
        }
    }
    pub fn bit(self) -> u32 {
        os_bit(self.code())
    }
    pub fn name(self) -> &'static str {
        os_full(self.code())
    }
    fn priority(self) -> i32 {
        match self {
            SnapKind::End => 0,
            SnapKind::Int => 1,
            SnapKind::Mid => 2,
            SnapKind::Cen | SnapKind::Nod => 3,
            SnapKind::Qua | SnapKind::Ins => 4,
            SnapKind::Per | SnapKind::Tan => 5,
            SnapKind::App => 6,
            SnapKind::Nea => 9,
        }
    }
}

impl From<SnapType> for SnapKind {
    fn from(t: SnapType) -> SnapKind {
        match t {
            SnapType::End => SnapKind::End,
            SnapType::Mid => SnapKind::Mid,
            SnapType::Cen => SnapKind::Cen,
            SnapType::Qua => SnapKind::Qua,
            SnapType::Nod => SnapKind::Nod,
            SnapType::Ins => SnapKind::Ins,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SnapHit {
    pub kind: SnapKind,
    pub pt: Pt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    Ortho,
    Polar,
    OTrack,
    Intersection,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackLine {
    pub a: Pt,
    pub b: Pt,
    pub through: Pt,
    pub kind: TrackKind,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub pt: Pt,
    pub lines: Vec<TrackLine>,
    pub marks: Vec<Pt>,
    pub label: String,
    pub kind: TrackKind,
    pub dist: Option<f64>,
    pub angle: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub pt: Pt,
    pub snap: Option<SnapHit>,
    pub track: Option<Track>,
}

#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub sx: f64,
    pub sy: f64,
    pub base: Pt,
    pub angle: f64,
}

#[derive(Clone, Debug)]
pub struct SnapOptions {
    /// Pixels per drawing unit.
    pub scale: f64,
    pub aperture_px: f64,
    /// Enabled object-snap bits (already resolved from OSMODE / overrides).
    pub osmask: u32,
    /// Previous point (for PER/TAN/polar/direct distance entry).
    pub base: Option<Pt>,
    pub ortho: bool,
    pub polar: bool,
    /// Radians.
    pub polar_inc: f64,
    pub polar_add: Vec<f64>,
    pub otrack: bool,
    pub grid: Option<Grid>,
    /// Rotation of the user coordinate system: ortho and polar directions follow its axes.
    pub ucs_angle: f64,
}

impl SnapOptions {
    pub fn new(scale: f64) -> SnapOptions {
        SnapOptions {
            scale,
            aperture_px: 12.0,
            osmask: 0,
            base: None,
            ortho: false,
            polar: false,
            polar_inc: PI / 2.0,
            polar_add: vec![],
            otrack: false,
            grid: None,
            ucs_angle: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Dwell {
    pt: Pt,
    since: f64,
    done: bool,
}

#[derive(Default)]
pub struct Snapper {
    pub acquired: Vec<Pt>,
    dwell: Option<Dwell>,
}

#[derive(Clone, Copy)]
struct Cand {
    kind: SnapKind,
    pt: Pt,
    d: f64,
}

#[derive(Clone, Copy)]
struct TrackVec {
    origin: Pt,
    dir: Pt,
    kind: TrackKind,
    angle: f64,
}

fn fmt(n: f64) -> String {
    let r = (n * 10000.0).round() / 10000.0;
    crate::sysvars::fmt_num(r)
}

fn degs(a: f64) -> String {
    format!(
        "{}°",
        crate::sysvars::fmt_num((deg(norm_angle(a)) * 100.0).round() / 100.0)
    )
}

fn mk_line(v: &TrackVec, to: Pt) -> TrackLine {
    TrackLine {
        a: v.origin,
        b: to + v.dir * 1e6,
        through: to,
        kind: v.kind,
    }
}

impl Snapper {
    pub fn new() -> Snapper {
        Snapper::default()
    }

    pub fn reset(&mut self) {
        self.acquired.clear();
        self.dwell = None;
    }

    /// Record that the cursor is resting on a snap point; returns true when the acquisition state changed.
    pub fn note_hover(&mut self, snap: Option<&SnapHit>, now: f64) -> bool {
        let Some(snap) = snap else {
            self.dwell = None;
            return false;
        };
        match self.dwell {
            Some(d) if same_pt(d.pt, snap.pt, 1e-7) => {
                if d.done || now - d.since < 300.0 {
                    return false;
                }
                self.dwell = Some(Dwell { done: true, ..d });
                if let Some(i) = self
                    .acquired
                    .iter()
                    .position(|a| same_pt(*a, snap.pt, 1e-7))
                {
                    self.acquired.remove(i);
                } else {
                    self.acquired.push(snap.pt);
                    if self.acquired.len() > MAX_ACQUIRED {
                        self.acquired.remove(0);
                    }
                }
                true
            }
            _ => {
                self.dwell = Some(Dwell {
                    pt: snap.pt,
                    since: now,
                    done: false,
                });
                false
            }
        }
    }

    /// Resolve a raw world point into the point to use.
    pub fn resolve(&self, doc: &Drawing, raw: Pt, o: &SnapOptions) -> Resolved {
        let ap = o.aperture_px / o.scale;
        let mut out = Resolved {
            pt: raw,
            snap: None,
            track: None,
        };

        // 1. object snaps
        if o.osmask != 0 {
            if let Some(hit) = self.object_snap(doc, raw, ap, o) {
                out.pt = hit.pt;
                out.snap = Some(hit);
                return out;
            }
        }
        // 2. ortho / polar / object-snap tracking
        if let Some(t) = self.tracking(raw, ap, o) {
            out.pt = t.pt;
            out.track = Some(t);
            return out;
        }
        // 3. grid snap
        if let Some(g) = &o.grid {
            out.pt = snap_to_grid(raw, g);
        }
        out
    }

    pub fn object_snap(&self, doc: &Drawing, raw: Pt, ap: f64, o: &SnapOptions) -> Option<SnapHit> {
        let mask = o.osmask;
        let win = Bounds::new(raw.x - ap, raw.y - ap, raw.x + ap, raw.y + ap);
        let mut near = vec![];
        let ents = doc.entities();
        for e in doc.candidates(&win).into_iter().map(|i| &ents[i]) {
            if !doc.is_visible(e) {
                continue;
            }
            if matches!(e.kind, Kind::Xline { .. } | Kind::Ray { .. }) {
                near.push(e);
                continue;
            }
            if bounds_of(e, doc).is_some_and(|b| b.overlaps(&win)) {
                near.push(e);
            }
        }
        let mut cands: Vec<Cand> = vec![];
        let mut add = |kind: SnapKind, pt: Pt, trigger: f64| {
            if trigger <= ap {
                cands.push(Cand {
                    kind,
                    pt,
                    d: trigger,
                });
            }
        };
        for e in &near {
            for s in snap_points_of(e, doc).iter() {
                let kind = SnapKind::from(s.kind);
                if mask & kind.bit() != 0 {
                    add(kind, s.pt, dist(raw, s.pt));
                }
            }
        }
        let segs_near: Vec<Rc<Vec<Seg>>> = near.iter().map(|e| segs_of(e, doc)).collect();
        if mask & (OS_INT | OS_APP) != 0 {
            for i in 0..segs_near.len() {
                for j in i..segs_near.len() {
                    for (ka, sa) in segs_near[i].iter().enumerate() {
                        for (kb, sb) in segs_near[j].iter().enumerate() {
                            if i == j && ka == kb {
                                continue;
                            }
                            for h in seg_intersect(sa, sb, false, false) {
                                add(SnapKind::Int, h.pt, dist(raw, h.pt));
                            }
                        }
                    }
                }
            }
        }
        if let Some(base) = o.base {
            if mask & (OS_PER | OS_TAN) != 0 {
                for segs in &segs_near {
                    for s in segs.iter() {
                        let (_, _, nd) = s.nearest(raw);
                        if nd > ap {
                            continue;
                        }
                        if mask & OS_PER != 0 {
                            for (fp, t) in perp_feet(s, base) {
                                if (-1e-9..=1.0 + 1e-9).contains(&t) {
                                    cands.push(Cand {
                                        kind: SnapKind::Per,
                                        pt: fp,
                                        d: nd,
                                    });
                                }
                            }
                        }
                        if mask & OS_TAN != 0 && !s.is_line() {
                            let mut tp: Vec<(Pt, f64)> = tangent_points(s, base)
                                .into_iter()
                                .filter(|q| (-1e-9..=1.0 + 1e-9).contains(&q.1))
                                .collect();
                            tp.sort_by(|p, q| dist(p.0, raw).total_cmp(&dist(q.0, raw)));
                            if let Some(first) = tp.first() {
                                cands.push(Cand {
                                    kind: SnapKind::Tan,
                                    pt: first.0,
                                    d: nd,
                                });
                            }
                        }
                    }
                }
            }
        }
        if mask & OS_NEA != 0 {
            for segs in &segs_near {
                for s in segs.iter() {
                    let (q, _, d) = s.nearest(raw);
                    if d <= ap {
                        cands.push(Cand {
                            kind: SnapKind::Nea,
                            pt: q,
                            d,
                        });
                    }
                }
            }
        }
        if cands.is_empty() {
            return None;
        }
        cands.sort_by(|a, b| {
            let diff = a.d - b.d;
            if diff != 0.0 {
                diff.total_cmp(&0.0)
            } else {
                a.kind.priority().cmp(&b.kind.priority())
            }
        });
        // NEA is a fallback: take it only if nothing else qualifies
        let best = cands
            .iter()
            .find(|c| c.kind != SnapKind::Nea)
            .unwrap_or(&cands[0]);
        Some(SnapHit {
            kind: best.kind,
            pt: best.pt,
        })
    }

    pub fn tracking(&self, raw: Pt, ap: f64, o: &SnapOptions) -> Option<Track> {
        let base = o.base;
        if let (Some(base), true) = (base, o.ortho) {
            let ua = o.ucs_angle;
            let (c, sn) = (ua.cos(), ua.sin());
            let w = raw - base;
            // offset in the user coordinate system
            let (dx, dy) = (w.x * c + w.y * sn, -w.x * sn + w.y * c);
            let horizontal = dx.abs() >= dy.abs();
            let (lx, ly) = if horizontal { (dx, 0.0) } else { (0.0, dy) };
            let p = pt(base.x + lx * c - ly * sn, base.y + lx * sn + ly * c);
            let d = dist(base, p);
            let a = ua
                + if horizontal {
                    if dx >= 0.0 {
                        0.0
                    } else {
                        PI
                    }
                } else if dy >= 0.0 {
                    PI / 2.0
                } else {
                    -PI / 2.0
                };
            return Some(Track {
                pt: p,
                lines: vec![],
                marks: vec![],
                label: format!("Ortho: {} < {}", fmt(d), degs(a - ua)),
                kind: TrackKind::Ortho,
                dist: Some(d),
                angle: Some(a),
            });
        }
        let angles: Vec<f64> = if o.polar {
            polar_angles(
                if o.polar_inc > 0.0 {
                    o.polar_inc
                } else {
                    PI / 2.0
                },
                &o.polar_add,
            )
        } else {
            vec![0.0, PI / 2.0, PI, 3.0 * PI / 2.0]
        };
        let angles: Vec<f64> = angles.into_iter().map(|a| a + o.ucs_angle).collect();
        let mut vectors: Vec<TrackVec> = vec![];
        if let (Some(base), true) = (base, o.polar) {
            for &a in &angles {
                vectors.push(TrackVec {
                    origin: base,
                    dir: pt(a.cos(), a.sin()),
                    kind: TrackKind::Polar,
                    angle: a,
                });
            }
        }
        if o.otrack {
            for acq in &self.acquired {
                for &a in &angles {
                    vectors.push(TrackVec {
                        origin: *acq,
                        dir: pt(a.cos(), a.sin()),
                        kind: TrackKind::OTrack,
                        angle: a,
                    });
                }
            }
        }
        if vectors.is_empty() {
            return None;
        }
        struct H {
            v: TrackVec,
            t: f64,
            d: f64,
        }
        let mut hits: Vec<H> = vec![];
        for v in &vectors {
            let w = raw - v.origin;
            let (t, d) = (dot(w, v.dir), cross(w, v.dir).abs());
            if t > ap * 0.25 && d <= ap {
                hits.push(H { v: *v, t, d });
            }
        }
        if hits.is_empty() {
            return None;
        }
        hits.sort_by(|a, b| a.d.total_cmp(&b.d));
        let first = &hits[0];
        // intersection of two tracking vectors from different origins
        for h2 in &hits[1..] {
            if same_pt(h2.v.origin, first.v.origin, 1e-9)
                || cross(first.v.dir, h2.v.dir).abs() < 1e-6
            {
                continue;
            }
            let a = Seg::line(first.v.origin, first.v.origin + first.v.dir * 1e7);
            let b = Seg::line(h2.v.origin, h2.v.origin + h2.v.dir * 1e7);
            if let Some(x) = seg_intersect(&a, &b, false, false).first() {
                if dist(x.pt, raw) <= ap * 1.5 {
                    return Some(Track {
                        pt: x.pt,
                        lines: vec![mk_line(&first.v, x.pt), mk_line(&h2.v, x.pt)],
                        marks: self.acquired.clone(),
                        label: "Intersection".into(),
                        kind: TrackKind::Intersection,
                        dist: None,
                        angle: None,
                    });
                }
            }
        }
        let p = first.v.origin + first.v.dir * first.t;
        let label = if first.v.kind == TrackKind::Polar {
            format!(
                "Polar: {} < {}",
                fmt(first.t),
                degs(first.v.angle - o.ucs_angle)
            )
        } else {
            format!(
                "Track: {} < {}",
                fmt(first.t),
                degs(first.v.angle - o.ucs_angle)
            )
        };
        Some(Track {
            pt: p,
            lines: vec![mk_line(&first.v, p)],
            marks: self.acquired.clone(),
            label,
            kind: first.v.kind,
            dist: Some(first.t),
            angle: Some(first.v.angle),
        })
    }
}

pub fn polar_angles(inc: f64, extra: &[f64]) -> Vec<f64> {
    let n = ((TAU / inc).round() as usize).max(1);
    let mut out: Vec<f64> = (0..n).map(|i| i as f64 * inc).collect();
    out.extend_from_slice(extra);
    out
}

pub fn snap_to_grid(p: Pt, g: &Grid) -> Pt {
    let (c, s) = ((-g.angle).cos(), (-g.angle).sin());
    let (dx, dy) = (p.x - g.base.x, p.y - g.base.y);
    let (u, v) = (dx * c - dy * s, dx * s + dy * c);
    let (su, sv) = ((u / g.sx).round() * g.sx, (v / g.sy).round() * g.sy);
    let (c2, s2) = (g.angle.cos(), g.angle.sin());
    pt(g.base.x + su * c2 - sv * s2, g.base.y + su * s2 + sv * c2)
}

/// One-shot object-snap override typed by the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsOverride {
    None,
    Mask(u32),
}

/// Resolve OSMODE + an optional one-shot override into the active bit mask.
pub fn active_mask(osmode: i64, ov: Option<OsOverride>) -> u32 {
    match ov {
        Some(OsOverride::None) => 0,
        Some(OsOverride::Mask(m)) => m,
        None => {
            let m = osmode as u32;
            if m & OS_OFF != 0 {
                0
            } else {
                m & !(OS_OFF | OS_QUI)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadforge_core::entities::Entity;

    fn doc() -> Drawing {
        let mut d = Drawing::new("t");
        d.add(Entity::line(pt(0.0, 0.0), pt(10.0, 0.0)));
        d.add(Entity::line(pt(5.0, -5.0), pt(5.0, 5.0)));
        d.add(Entity::circle(pt(20.0, 0.0), 4.0));
        d
    }

    fn opts(mask: u32) -> SnapOptions {
        let mut o = SnapOptions::new(10.0);
        o.osmask = mask;
        o
    }

    #[test]
    fn endpoint_midpoint_and_intersection() {
        let d = doc();
        let s = Snapper::new();
        let r = s.resolve(&d, pt(0.1, 0.05), &opts(OS_END | OS_MID | OS_INT));
        assert_eq!(r.snap.unwrap().kind, SnapKind::End);
        assert_eq!(r.pt, pt(0.0, 0.0));
        let r = s.resolve(&d, pt(5.05, 0.05), &opts(OS_END | OS_MID | OS_INT));
        assert_eq!(
            r.snap.unwrap().kind,
            SnapKind::Int,
            "intersection outranks the midpoint when equally close"
        );
        assert!(dist(r.pt, pt(5.0, 0.0)) < 1e-9);
        let r = s.resolve(&d, pt(20.1, 0.1), &opts(OS_CEN));
        assert_eq!(r.snap.unwrap().kind, SnapKind::Cen);
        let r = s.resolve(&d, pt(24.05, 0.05), &opts(OS_QUA));
        assert_eq!(r.pt, pt(24.0, 0.0));
        let r = s.resolve(&d, pt(100.0, 100.0), &opts(OS_END));
        assert!(r.snap.is_none());
    }

    #[test]
    fn nearest_is_a_fallback_and_perpendicular_uses_the_base() {
        let d = doc();
        let s = Snapper::new();
        let r = s.resolve(&d, pt(2.0, 0.05), &opts(OS_NEA));
        assert_eq!(r.snap.unwrap().kind, SnapKind::Nea);
        assert!((r.pt.y).abs() < 1e-9);
        let mut o = opts(OS_PER);
        o.base = Some(pt(3.0, 4.0));
        let r = s.resolve(&d, pt(3.0, 0.05), &o);
        assert_eq!(r.snap.unwrap().kind, SnapKind::Per);
        assert!(dist(r.pt, pt(3.0, 0.0)) < 1e-9);
    }

    #[test]
    fn ortho_polar_and_grid() {
        let d = Drawing::new("t");
        let s = Snapper::new();
        let mut o = SnapOptions::new(10.0);
        o.base = Some(pt(0.0, 0.0));
        o.ortho = true;
        let r = s.resolve(&d, pt(10.0, 1.0), &o);
        assert_eq!(r.pt, pt(10.0, 0.0));
        assert_eq!(r.track.unwrap().label, "Ortho: 10 < 0°");
        o.ortho = false;
        o.polar = true;
        o.polar_inc = PI / 4.0;
        let r = s.resolve(&d, pt(10.0, 10.1), &o);
        assert!(dist(r.pt, pt(10.05, 10.05)) < 0.1, "{:?}", r.pt);
        assert_eq!(r.track.unwrap().kind, TrackKind::Polar);
        o.polar = false;
        o.grid = Some(Grid {
            sx: 5.0,
            sy: 5.0,
            base: pt(0.0, 0.0),
            angle: 0.0,
        });
        let r = s.resolve(&d, pt(7.4, 12.6), &o);
        assert_eq!(r.pt, pt(5.0, 15.0));
    }

    #[test]
    fn acquisition_requires_a_dwell() {
        let mut s = Snapper::new();
        let h = SnapHit {
            kind: SnapKind::End,
            pt: pt(1.0, 1.0),
        };
        assert!(!s.note_hover(Some(&h), 0.0));
        assert!(!s.note_hover(Some(&h), 100.0));
        assert!(s.note_hover(Some(&h), 400.0));
        assert_eq!(s.acquired.len(), 1);
        assert!(
            !s.note_hover(Some(&h), 900.0),
            "only acquires once per dwell"
        );
        s.note_hover(None, 1000.0);
        s.note_hover(Some(&h), 1100.0);
        assert!(s.note_hover(Some(&h), 1500.0));
        assert!(s.acquired.is_empty(), "dwelling again releases the point");
    }

    #[test]
    fn masks() {
        assert_eq!(active_mask(4133, None), 4133 & !(OS_OFF | OS_QUI));
        assert_eq!(active_mask(4133 | 16384, None), 0);
        assert_eq!(active_mask(4133, Some(OsOverride::None)), 0);
        assert_eq!(active_mask(0, Some(OsOverride::Mask(OS_END))), OS_END);
    }
}
