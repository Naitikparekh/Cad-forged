//! FILLET and CHAMFER between lines/arcs, and the whole-polyline variants.
use super::path::*;
use crate::drawing::Drawing;
use crate::entities::Entity;
use crate::geom::*;

/// Portion of `seg` that contains the point at param `tp`, bounded at param `tb` (tb may lie outside [0, 1]).
fn keep_side(seg: &Seg, tp: f64, tb: f64) -> Seg {
    if tp < tb {
        seg.slice(0.0, tb)
    } else {
        seg.slice(tb, 1.0)
    }
}

fn param_of(seg: &Seg, p: Pt) -> f64 {
    seg.project(p)
}

/// Foot of the perpendicular from a fillet centre onto a carrier (the tangent point).
fn tangent_foot(seg: &Seg, c: Pt) -> Pt {
    match *seg {
        Seg::L { .. } => perp_feet(seg, c).first().map(|f| f.0).unwrap_or(c),
        Seg::A { c: sc, r, .. } => sc + unit(c - sc) * r,
    }
}

/// Result of a corner operation: replacement segments for both objects (`None` = leave untouched, e.g. full circles)
/// plus the arc/line that fills the corner.
#[derive(Clone, Copy, Debug)]
pub struct Corner {
    pub a: Option<Seg>,
    pub b: Option<Seg>,
    pub insert: Option<Seg>,
}

/// Fillet two segments. Pick points choose which ends survive and which corner is used.
pub fn fillet_segs(s_a: &Seg, pick_a: Pt, s_b: &Seg, pick_b: Pt, r: f64) -> Result<Corner, String> {
    let (keep_a, keep_b) = (!s_a.is_full(), !s_b.is_full());
    if r < 1e-12 {
        let hits = seg_intersect(s_a, s_b, true, true);
        if hits.is_empty() {
            return Err("The objects are parallel or do not meet.".into());
        }
        let m = mid(pick_a, pick_b);
        let h = hits.iter().copied().fold(hits[0], |best, c| if dist(c.pt, m) < dist(best.pt, m) { c } else { best });
        return Ok(Corner {
            a: keep_a.then(|| keep_side(s_a, param_of(s_a, pick_a), h.t1)),
            b: keep_b.then(|| keep_side(s_b, param_of(s_b, pick_b), h.t2)),
            insert: None,
        });
    }
    let centres = tangent_centres(s_a, s_b, r);
    if centres.is_empty() {
        return Err("A fillet of that radius cannot be created between these objects.".into());
    }
    // The arc lies in the sector spanned by the picked portions: both tangent points must be on the picked side of the corner.
    let corners = seg_intersect(s_a, s_b, true, true);
    let (pa, pb) = (param_of(s_a, pick_a), param_of(s_b, pick_b));
    let mut best: Option<(Pt, Pt, Pt, f64)> = None;
    for c in centres {
        let (ta, tb) = (tangent_foot(s_a, c), tangent_foot(s_b, c));
        let (qa, qb) = (param_of(s_a, ta), param_of(s_b, tb));
        let on_picked_side = corners.iter().any(|h| sign(pa - h.t1) == sign(qa - h.t1) && sign(pb - h.t2) == sign(qb - h.t2));
        let score = (if on_picked_side { 0.0 } else { 1e9 }) + dist(ta, pick_a) + dist(tb, pick_b);
        if best.map_or(true, |b| score < b.3) {
            best = Some((c, ta, tb, score));
        }
    }
    let (c, ta, tb, _) = best.expect("at least one centre");
    let a0 = angle_of(c, ta);
    let sw = norm_angle_signed(angle_of(c, tb) - a0);
    Ok(Corner {
        a: keep_a.then(|| keep_side(s_a, param_of(s_a, pick_a), param_of(s_a, ta))),
        b: keep_b.then(|| keep_side(s_b, param_of(s_b, pick_b), param_of(s_b, tb))),
        insert: Some(Seg::arc(c, r, a0, sw)),
    })
}

/// JS `Math.sign`: 0 for 0, ±1 otherwise.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn as_line(s: &Seg) -> Option<(Pt, Pt)> {
    match *s {
        Seg::L { a, b, .. } => Some((a, b)),
        _ => None,
    }
}

/// Chamfer two line segments by distances d1 (on A) and d2 (on B).
pub fn chamfer_segs(s_a: &Seg, pick_a: Pt, s_b: &Seg, pick_b: Pt, d1: f64, d2: f64) -> Result<Corner, String> {
    let (Some((aa, ab)), Some((ba, bb))) = (as_line(s_a), as_line(s_b)) else { return Err("Chamfer works on straight segments.".into()) };
    let hits = seg_intersect(s_a, s_b, true, true);
    let Some(h0) = hits.first().copied() else { return Err("The lines are parallel.".into()) };
    let p = h0.pt;
    if d1 < 1e-12 && d2 < 1e-12 {
        return Ok(Corner { a: Some(keep_side(s_a, param_of(s_a, pick_a), h0.t1)), b: Some(keep_side(s_b, param_of(s_b, pick_b), h0.t2)), insert: None });
    }
    let toward = |a: Pt, b: Pt, pick: Pt| -> Pt {
        let v = pick - p;
        unit(if v.x == 0.0 && v.y == 0.0 {
            b - a
        } else if dot(v, b - a) >= 0.0 {
            b - a
        } else {
            a - b
        })
    };
    let (ua, ub) = (toward(aa, ab, pick_a), toward(ba, bb, pick_b));
    let (ca, cb) = (p + ua * d1, p + ub * d2);
    let (la, lb) = (Seg::line(aa, ab), Seg::line(ba, bb));
    Ok(Corner { a: Some(keep_side(&la, param_of(&la, pick_a), param_of(&la, ca))), b: Some(keep_side(&lb, param_of(&lb, pick_b), param_of(&lb, cb))), insert: Some(Seg::line(ca, cb)) })
}

/// Chamfer by length on A and an angle (radians) measured from A.
pub fn chamfer_angle(s_a: &Seg, pick_a: Pt, s_b: &Seg, pick_b: Pt, d1: f64, angle: f64) -> Result<Corner, String> {
    let (Some((aa, ab)), Some((ba, bb))) = (as_line(s_a), as_line(s_b)) else { return Err("Chamfer works on straight segments.".into()) };
    let hits = seg_intersect(s_a, s_b, true, true);
    let Some(h0) = hits.first().copied() else { return Err("The lines are parallel.".into()) };
    let p = h0.pt;
    let ua = unit(if dot(pick_a - p, ab - aa) >= 0.0 { ab - aa } else { aa - ab });
    let ca = p + ua * d1;
    let ub = unit(if dot(pick_b - p, bb - ba) >= 0.0 { bb - ba } else { ba - bb });
    // direction from ca at `angle` from A's direction back toward the corner, towards B's side
    let base = angle_of(ca, p);
    let side = if cross(ua, ub) >= 0.0 { 1.0 } else { -1.0 };
    let dir = pt((base - side * angle).cos(), (base - side * angle).sin());
    let probe = Seg::line(ca, ca + dir * 1e6);
    let h = seg_intersect(&probe, s_b, false, true).first().copied().ok_or("The chamfer angle does not meet the second line.")?;
    let (la, lb) = (Seg::line(aa, ab), Seg::line(ba, bb));
    Ok(Corner { a: Some(keep_side(&la, param_of(&la, pick_a), param_of(&la, ca))), b: Some(keep_side(&lb, param_of(&lb, pick_b), param_of(&lb, h.pt))), insert: Some(Seg::line(ca, h.pt)) })
}

pub struct PlineResult {
    pub entity: Option<Entity>,
    pub applied: usize,
}

/// Walk every vertex of a polyline applying `corner(segA, pickA, segB, pickB)`.
fn corner_pass(e: &Entity, doc: &Drawing, corner: impl Fn(&Seg, Pt, &Seg, Pt) -> Result<Corner, String>) -> Option<PlineResult> {
    let path = path_of(e, doc)?;
    let mut segs = path.segs.clone();
    let n = segs.len();
    let mut inserts: Vec<Option<Seg>> = vec![None; n];
    let mut applied = 0;
    let joints = if path.closed { n } else { n.saturating_sub(1) };
    for i in 0..joints {
        let j = (i + 1) % n;
        let (a, b) = (segs[i], segs[j]);
        let (pa, pb) = (a.at(0.05), b.at(0.95)); // pick the far portions: those are the parts that survive
        let Ok(c) = corner(&a, pa, &b, pb) else { continue };
        let (Some(ra), Some(rb)) = (c.a, c.b) else { continue };
        // the radius must fit: the trimmed segments may not vanish, and may not grow (an extension means it did not fit)
        if ra.length() < 1e-9 || rb.length() < 1e-9 {
            continue;
        }
        if ra.length() > a.length() + 1e-9 || rb.length() > b.length() + 1e-9 {
            continue;
        }
        segs[i] = ra;
        segs[j] = rb;
        inserts[i] = c.insert;
        applied += 1;
    }
    let mut out = vec![];
    for (i, s) in segs.iter().enumerate() {
        out.push(*s);
        if let Some(x) = inserts[i] {
            out.push(x);
        }
    }
    Some(PlineResult { entity: entity_from_segs(&out, e, path.closed), applied })
}

pub fn fillet_pline(e: &Entity, r: f64, doc: &Drawing) -> Option<PlineResult> {
    corner_pass(e, doc, |a, pa, b, pb| fillet_segs(a, pa, b, pb, r))
}

pub fn chamfer_pline(e: &Entity, d1: f64, d2: f64, doc: &Drawing) -> Option<PlineResult> {
    corner_pass(e, doc, |a, pa, b, pb| chamfer_segs(a, pa, b, pb, d1, d2))
}
