//! TRIM, EXTEND, BREAK, JOIN and LENGTHEN on line / arc / circle / polyline paths.
use super::path::*;
use crate::drawing::Drawing;
use crate::entities::{Entity, Kind};
use crate::geom::*;

const EPS: f64 = 1e-9;

fn cut_params(path: &Path, cut_segs: &[Seg], extend_cutters: bool) -> Vec<f64> {
    let mut us = vec![];
    for (i, ts) in path.segs.iter().enumerate() {
        for cs in cut_segs {
            for h in seg_intersect(ts, cs, false, extend_cutters) {
                us.push(i as f64 + h.t1.clamp(0.0, 1.0));
            }
        }
    }
    us.sort_by(|a, b| a.total_cmp(b));
    let n = path.segs.len() as f64;
    let mut out: Vec<f64> = vec![];
    for u in us {
        let v = if path.closed && n - u < 1e-7 { 0.0 } else { u };
        if !out.iter().any(|o| (o - v).abs() < 1e-7) {
            out.push(v);
        }
    }
    out.sort_by(|a, b| a.total_cmp(b));
    out
}

/// Trim the part of `target` under `pick` back to the nearest cutting edges.
/// Returns the entities that should take the target's place (possibly none).
pub fn trim(target: &Entity, pick: Pt, cutters: &[&Entity], doc: &Drawing, extend_cutters: bool) -> Result<Vec<Entity>, String> {
    let path = path_of(target, doc).ok_or("That object cannot be trimmed.")?;
    let cut_segs: Vec<Seg> = cutters.iter().filter(|c| c.id != target.id || target.id == 0).flat_map(|c| edge_segs(c, doc)).collect();
    let cuts = cut_params(&path, &cut_segs, extend_cutters);
    let n = path.segs.len() as f64;
    let up = path_nearest(&path, pick).u;
    if cuts.is_empty() {
        return Err("Object does not intersect an edge.".into());
    }
    let mk = |segs: Vec<Seg>| entity_from_segs(&segs, target, false);

    if !path.closed {
        let prev = cuts.iter().rev().find(|&&u| u < up - EPS).copied();
        let next = cuts.iter().find(|&&u| u > up + EPS).copied();
        let inner = cuts.iter().any(|&u| u > 1e-7 && u < n - 1e-7);
        if !inner && !(prev.is_some() && next.is_some()) {
            return Err("Object does not intersect an edge.".into());
        }
        let a = prev.unwrap_or(0.0);
        let b = next.unwrap_or(n);
        let mut pieces = vec![];
        if a > 1e-7 {
            pieces.extend(mk(slice_linear(&path.segs, 0.0, a)));
        }
        if b < n - 1e-7 {
            pieces.extend(mk(slice_linear(&path.segs, b, n)));
        }
        return Ok(pieces);
    }
    if cuts.len() < 2 {
        return Err("A closed object needs at least two cutting points.".into());
    }
    let prev = cuts.iter().rev().find(|&&u| u < up - EPS).copied().unwrap_or(*cuts.last().expect("cuts"));
    let next = cuts.iter().find(|&&u| u > up + EPS).copied().unwrap_or(cuts[0]);
    let rest = path_slice(&path, next, prev); // from the next cut around to the previous cut
    Ok(mk(merge_segs(&rest)).into_iter().collect())
}

/// Extension of the last segment in its direction: candidate param t > 1 along the carrier.
fn extension_hits(seg: &Seg, bounds: &[Seg], extend_bounds: bool) -> Vec<(f64, Pt)> {
    let full = match seg {
        Seg::A { sw, .. } => TAU / sw.abs(),
        _ => f64::INFINITY,
    };
    let mut hits = vec![];
    for b in bounds {
        for h in seg_intersect(seg, b, true, extend_bounds) {
            let mut t = h.t1;
            if !seg.is_line() && t < 0.0 {
                t += full; // behind the start = ahead after wrapping
            }
            if t > 1.0 + 1e-9 && t < full - 1e-9 {
                hits.push((t, h.pt));
            }
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    hits
}

fn reversed(segs: &[Seg]) -> Vec<Seg> {
    segs.iter().rev().map(|s| s.reverse()).collect()
}

/// Extend the end of `target` nearest `pick` to the closest boundary.
pub fn extend(target: &Entity, pick: Pt, boundaries: &[&Entity], doc: &Drawing, extend_boundaries: bool) -> Result<Vec<Entity>, String> {
    let ok = match &target.kind {
        Kind::Line { .. } | Kind::Arc { .. } => true,
        Kind::Pline { closed, .. } => !closed,
        _ => false,
    };
    if !ok {
        return Err("That object cannot be extended.".into());
    }
    let path = path_of(target, doc).ok_or("That object cannot be extended.")?;
    let near_start = dist(pick, path.segs[0].start()) < dist(pick, path.segs[path.segs.len() - 1].end());
    let mut segs = path.segs.clone();
    if near_start {
        segs = reversed(&segs);
    }
    let last = *segs.last().expect("path has segments");
    let bounds: Vec<Seg> = boundaries.iter().filter(|b| b.id != target.id || target.id == 0).flat_map(|b| edge_segs(b, doc)).collect();
    let (t, p) = *extension_hits(&last, &bounds, extend_boundaries).first().ok_or("No boundary in the extension direction.")?;
    let ext = match last {
        Seg::L { a, .. } => Seg::line(a, p),
        Seg::A { c, r, a0, sw, full } => Seg::A { c, r, a0, sw: sw * t, full },
    };
    *segs.last_mut().expect("path has segments") = ext;
    if near_start {
        segs = reversed(&segs);
    }
    Ok(entity_from_segs(&segs, target, false).into_iter().collect())
}

/// BREAK between two points (pick order matters on circles: removal runs CCW from p1 to p2).
pub fn break_at(target: &Entity, p1: Pt, p2: Pt, doc: &Drawing) -> Result<Vec<Entity>, String> {
    let path = path_of(target, doc).ok_or("That object cannot be broken.")?;
    let n = path.segs.len() as f64;
    let (mut u1, mut u2) = (path_nearest(&path, p1).u, path_nearest(&path, p2).u);
    let mk = |segs: Vec<Seg>| entity_from_segs(&segs, target, false);
    if !path.closed {
        if u1 > u2 {
            std::mem::swap(&mut u1, &mut u2);
        }
        let mut pieces = vec![];
        if u1 > 1e-7 {
            pieces.extend(mk(slice_linear(&path.segs, 0.0, u1)));
        }
        if u2 < n - 1e-7 {
            pieces.extend(mk(slice_linear(&path.segs, u2, n)));
        }
        return Ok(pieces);
    }
    if (u1 - u2).abs() < 1e-9 {
        return Err("The break points coincide.".into());
    }
    let rest = path_slice(&path, u2, u1);
    Ok(mk(merge_segs(&rest)).into_iter().collect())
}

/// Split an object in two at a point (BREAK with @ / BREAKATPOINT).
pub fn split_at(target: &Entity, p: Pt, doc: &Drawing) -> Result<Vec<Entity>, String> {
    let path = match path_of(target, doc) {
        Some(p) if !p.closed => p,
        _ => return Err("That object cannot be split at a point.".into()),
    };
    let n = path.segs.len() as f64;
    let u = path_nearest(&path, p).u;
    if u < 1e-7 || u > n - 1e-7 {
        return Err("The point is at an end of the object.".into());
    }
    Ok([entity_from_segs(&slice_linear(&path.segs, 0.0, u), target, false), entity_from_segs(&slice_linear(&path.segs, u, n), target, false)].into_iter().flatten().collect())
}

pub struct JoinResult {
    pub results: Vec<Entity>,
    /// Indices (into the input) of every path entity that was considered.
    pub sources: Vec<usize>,
    /// Indices of the inputs that were merged into a result.
    pub used: Vec<usize>,
}

/// JOIN: chain connected lines, arcs and polylines. Returns one entity per chain.
pub fn join(entities: &[&Entity], doc: &Drawing, tol: f64) -> JoinResult {
    let items: Vec<(usize, Path)> = entities.iter().enumerate().filter_map(|(i, e)| path_of(e, doc).filter(|p| !p.closed && !p.segs.is_empty()).map(|p| (i, p))).collect();
    let mut used = vec![false; items.len()];
    let mut results = vec![];
    for si in 0..items.len() {
        if used[si] {
            continue;
        }
        used[si] = true;
        let mut segs = items[si].1.segs.clone();
        let mut grew = true;
        while grew {
            grew = false;
            for (k, it) in items.iter().enumerate() {
                if used[k] {
                    continue;
                }
                let head = segs[0].start();
                let tail = segs[segs.len() - 1].end();
                let s0 = it.1.segs[0].start();
                let s1 = it.1.segs[it.1.segs.len() - 1].end();
                if dist(tail, s0) <= tol {
                    segs.extend(it.1.segs.iter().copied());
                } else if dist(tail, s1) <= tol {
                    segs.extend(reversed(&it.1.segs));
                } else if dist(head, s1) <= tol {
                    let mut v = it.1.segs.clone();
                    v.extend(segs);
                    segs = v;
                } else if dist(head, s0) <= tol {
                    let mut v = reversed(&it.1.segs);
                    v.extend(segs);
                    segs = v;
                } else {
                    continue;
                }
                used[k] = true;
                grew = true;
            }
        }
        // merge collinear lines / co-circular arcs
        let mut merged: Vec<Seg> = vec![];
        for s in &segs {
            if let (Some(Seg::L { a: la, b: lb, .. }), Seg::L { a, b, .. }) = (merged.last().copied(), s) {
                let (d1, d2) = (lb - la, *b - *a);
                if cross(d1, d2).abs() < 1e-9 * len(d1) * len(d2) && dot(d1, d2) > 0.0 {
                    *merged.last_mut().expect("last exists") = Seg::line(la, *b);
                    continue;
                }
            }
            merged.push(*s);
        }
        let arc_merged = merge_segs(&merged);
        let template = entities[items[si].0];
        let closed = arc_merged.len() > 1 && dist(arc_merged[0].start(), arc_merged[arc_merged.len() - 1].end()) <= tol;
        if let [Seg::A { c, r, sw, .. }] = arc_merged[..] {
            if sw.abs() >= TAU - 1e-9 {
                results.push(inherit_props(template, Entity::circle(c, r)));
                continue;
            }
        }
        results.extend(entity_from_segs(&arc_merged, template, closed));
    }
    JoinResult { results, sources: items.iter().map(|i| i.0).collect(), used: items.iter().enumerate().filter(|(k, _)| used[*k]).map(|(_, i)| i.0).collect() }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthenMode {
    /// Add a length.
    Delta,
    /// New length as a percentage of the old one.
    Percent,
    /// New total length.
    Total,
}

/// LENGTHEN the end nearest `pick` of a line/arc/open polyline.
pub fn lengthen(target: &Entity, pick: Pt, mode: LengthenMode, value: f64, doc: &Drawing) -> Result<Vec<Entity>, String> {
    let ok = matches!(target.kind, Kind::Line { .. } | Kind::Arc { .. } | Kind::Pline { .. });
    let path = match path_of(target, doc) {
        Some(p) if ok && !p.closed => p,
        _ => return Err("That object cannot be lengthened.".into()),
    };
    let mut segs = path.segs.clone();
    let near_start = dist(pick, segs[0].start()) < dist(pick, segs[segs.len() - 1].end());
    if near_start {
        segs = reversed(&segs);
    }
    let total: f64 = segs.iter().map(|s| s.length()).sum();
    let delta = match mode {
        LengthenMode::Delta => value,
        LengthenMode::Percent => total * value / 100.0 - total,
        LengthenMode::Total => value - total,
    };
    let last = *segs.last().expect("segments");
    let new_l = last.length() + delta;
    if new_l <= EPS {
        return Err("The resulting length would be zero or negative.".into());
    }
    let ext = match last {
        Seg::L { a, b, .. } => Seg::line(a, a + unit(b - a) * new_l),
        Seg::A { c, r, a0, sw, full } => {
            let nsw = sw.signum() * new_l / r;
            if nsw.abs() > TAU {
                return Err("The arc would exceed a full circle.".into());
            }
            Seg::A { c, r, a0, sw: nsw, full }
        }
    };
    *segs.last_mut().expect("segments") = ext;
    if near_start {
        segs = reversed(&segs);
    }
    Ok(entity_from_segs(&segs, target, false).into_iter().collect())
}
