//! OFFSET: parallel copies of lines, arcs, circles, polylines (with extended corners), ellipses and splines.
use super::path::*;
use crate::drawing::Drawing;
use crate::entities::{
    ellipse_is_full, ellipse_point, ellipse_sweep, segs_of, spline_points, Entity, Kind,
};
use crate::geom::*;

/// Which side of a path (left = +1, right = -1) is the point on?
pub fn side_of(segs: &[Seg], p: Pt) -> f64 {
    let mut best = (f64::INFINITY, 1.0);
    for s in segs {
        let (q, t, d) = s.nearest(p);
        if d < best.0 {
            let (tan, v) = (s.tangent(t), p - q);
            let mut side = if cross(tan, v) >= 0.0 { 1.0 } else { -1.0 };
            if let Seg::A { c, r, sw, .. } = *s {
                if d > 0.0 {
                    // inside a CCW arc is its left
                    side = if (dist(p, c) < r) == (sw > 0.0) {
                        1.0
                    } else {
                        -1.0
                    };
                }
            }
            best = (d, side);
        }
    }
    best.1
}

/// Offset segments by distance `d` to the left (negative = right). `None` if the offset collapses completely.
pub fn offset_segs(segs: &[Seg], d: f64, closed: bool) -> Option<Vec<Seg>> {
    let mut offs: Vec<Option<Seg>> = segs.iter().map(|s| s.offset(d)).collect();
    // iterative removal of collapsed / reversed segments
    for _pass in 0..4 {
        let alive: Vec<(Seg, usize)> = offs
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.map(|s| (s, i)))
            .collect();
        if alive.is_empty() {
            return None;
        }
        let n = alive.len();
        let mut pts: Vec<Pt> = vec![];
        // joint k joins alive[k-1] and alive[k]
        for k in 0..n {
            let prev = alive[(k + n - 1) % n].0;
            let cur = alive[k].0;
            if !closed && k == 0 {
                pts.push(cur.start());
                continue;
            }
            let (end_a, start_b) = (prev.end(), cur.start());
            if dist(end_a, start_b) < 1e-9 {
                pts.push(end_a);
                continue;
            }
            // the original joint is where the un-offset segments meet; choose the carrier intersection closest to the offset ends
            let hits = seg_intersect(&prev, &cur, true, true);
            let m = mid(end_a, start_b);
            let mut best: Option<Pt> = None;
            for h in hits {
                if best.map_or(true, |b| dist(h.pt, m) < dist(b, m)) {
                    best = Some(h.pt);
                }
            }
            pts.push(best.unwrap_or(m));
        }
        let last_pt = if closed {
            None
        } else {
            Some(alive[n - 1].0.end())
        };
        // rebuild each segment between its joints; detect reversal
        let mut rebuilt = vec![];
        let mut bad = vec![];
        for k in 0..n {
            let (o, oi) = alive[k];
            let p0 = pts[k];
            let p1 = if closed {
                pts[(k + 1) % n]
            } else if k + 1 < n {
                pts[k + 1]
            } else {
                last_pt.expect("open path has a last point")
            };
            let ns = match o {
                Seg::L { a, b, .. } => {
                    if dot(p1 - p0, b - a) < -1e-12 {
                        bad.push(oi);
                    }
                    Seg::line(p0, p1)
                }
                Seg::A { c, r, sw: osw, .. } => {
                    let a0 = angle_of(c, p0);
                    let a1 = angle_of(c, p1);
                    let mut sw = if osw >= 0.0 {
                        sweep_ccw(a0, a1)
                    } else {
                        -sweep_ccw(a1, a0)
                    };
                    if sw.abs() > osw.abs() + 1e-6 && osw.abs() < TAU - 1e-6 {
                        bad.push(oi);
                    }
                    if dist(p0, p1) < 1e-12 && osw.abs() >= TAU - 1e-9 {
                        sw = osw;
                    }
                    Seg::arc(c, r, a0, sw)
                }
            };
            rebuilt.push(ns);
        }
        if bad.is_empty() {
            return Some(rebuilt);
        }
        for i in bad {
            offs[i] = None;
        }
    }
    None
}

pub struct OffsetResult {
    pub entity: Entity,
    /// True when a curve was approximated by segments (ellipse, spline).
    pub approximated: bool,
}

/// Offset any supported entity by `dist` towards `side_point`.
pub fn offset_entity(
    e: &Entity,
    dist_: f64,
    side_point: Pt,
    doc: &Drawing,
) -> Result<OffsetResult, String> {
    let ok = |entity: Entity| {
        Ok(OffsetResult {
            entity,
            approximated: false,
        })
    };
    match &e.kind {
        Kind::Line { .. } => {
            let segs = segs_of(e, doc);
            let d = dist_ * side_of(&segs, side_point);
            let o = segs[0].offset(d).ok_or("That object cannot be offset.")?;
            ok(inherit_props(e, Entity::line(o.start(), o.end())))
        }
        Kind::Circle { c, r } => {
            let outside = dist(side_point, *c) > *r;
            let nr = if outside { r + dist_ } else { r - dist_ };
            if nr <= 1e-9 {
                return Err("The offset distance is larger than the circle radius.".into());
            }
            ok(inherit_props(e, Entity::circle(*c, nr)))
        }
        Kind::Arc { c, r, a0, a1 } => {
            let outside = dist(side_point, *c) > *r;
            let nr = if outside { r + dist_ } else { r - dist_ };
            if nr <= 1e-9 {
                return Err("The offset distance is larger than the arc radius.".into());
            }
            ok(inherit_props(e, Entity::arc(*c, nr, *a0, *a1)))
        }
        Kind::Pline { .. } => {
            let path = path_of(e, doc).ok_or("That object cannot be offset.")?;
            let d = dist_ * side_of(&path.segs, side_point);
            let segs = offset_segs(&path.segs, d, path.closed)
                .ok_or("The offset distance is too large for this polyline.")?;
            let entity = entity_from_segs(&segs, e, path.closed)
                .ok_or("The offset distance is too large for this polyline.")?;
            ok(entity)
        }
        Kind::Ellipse(_) | Kind::Spline { .. } => {
            let (pts, closed) = match &e.kind {
                Kind::Ellipse(el) => {
                    let sw = ellipse_sweep(el);
                    (
                        (0..=128)
                            .map(|i| ellipse_point(el, el.t0 + sw * i as f64 / 128.0))
                            .collect::<Vec<_>>(),
                        ellipse_is_full(el),
                    )
                }
                Kind::Spline { fit, closed } => (spline_points(fit, *closed, 14), *closed),
                _ => unreachable!(),
            };
            let lines: Vec<Seg> = pts.windows(2).map(|w| Seg::line(w[0], w[1])).collect();
            let segs = offset_segs(&lines, dist_ * side_of(&lines, side_point), closed)
                .ok_or("The offset distance is too large.")?;
            let entity =
                entity_from_segs(&segs, e, closed).ok_or("The offset distance is too large.")?;
            Ok(OffsetResult {
                entity,
                approximated: true,
            })
        }
        Kind::Xline { p, d } => {
            let n = perp(*d);
            let s = if dot(side_point - *p, n) >= 0.0 {
                1.0
            } else {
                -1.0
            };
            ok(inherit_props(e, Entity::xline(*p + n * (dist_ * s), *d)))
        }
        _ => Err("That object cannot be offset.".into()),
    }
}
