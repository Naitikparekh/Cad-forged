//! Hidden-line projection of meshes onto a view plane (flat shots).
use super::repair::Topo;
use crate::geom::*;
use crate::solid::mesh::mesh_bounds;
use crate::solid::{Mesh, V3};

/// A projected edge piece, in view-plane units (x to the right, y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line2 {
    pub a: Pt,
    pub b: Pt,
    /// Seen by the viewer (otherwise hidden behind a face).
    pub visible: bool,
}

/// View axes for a camera at `yaw` / `pitch` (as in [`crate::view::View`]): right, up and the direction away from the viewer.
fn axes(yaw: f64, pitch: f64) -> (V3, V3, V3) {
    let (sy, cy, sp, cp) = (yaw.sin(), yaw.cos(), pitch.sin(), pitch.cos());
    (V3::new(cy, -sy, 0.0), V3::new(sy * sp, cy * sp, cp), V3::new(sy * cp, cy * cp, -sp))
}

/// A face seen in the view: 2D corners (counter-clockwise) with depths.
struct Tri {
    p: [Pt; 3],
    d: [f64; 3],
    lo: Pt,
    hi: Pt,
}

/// Parameter interval of the segment `s0 + t·dir` (t in 0..1) inside triangle `t`, with the linear function
/// (depth of the face minus depth of the segment) over it.
fn overlap(t: &Tri, s0: Pt, dir: Pt, sd: (f64, f64), tol: f64) -> Option<(f64, f64, f64, f64)> {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for i in 0..3 {
        let (p, q) = (t.p[i], t.p[(i + 1) % 3]);
        let e = q - p;
        let (g0, g1) = (cross(e, s0 - p) + tol * len(e), cross(e, dir));
        if g1.abs() < 1e-15 {
            if g0 < 0.0 {
                return None;
            }
        } else {
            let at = -g0 / g1;
            if g1 > 0.0 {
                lo = lo.max(at);
            } else {
                hi = hi.min(at);
            }
        }
        if hi - lo < 1e-9 {
            return None;
        }
    }
    // face depth at s0 and its change along the segment, from barycentric coordinates
    let area = cross(t.p[1] - t.p[0], t.p[2] - t.p[0]);
    let depth = |x: Pt| -> f64 {
        let (w1, w2) = (cross(x - t.p[0], t.p[2] - t.p[0]) / area, cross(t.p[1] - t.p[0], x - t.p[0]) / area);
        t.d[0] * (1.0 - w1 - w2) + t.d[1] * w1 + t.d[2] * w2
    };
    let (f0, f1) = (depth(s0) - sd.0, depth(s0 + dir) - sd.1);
    Some((lo, hi, f0, f1 - f0))
}

/// Edges of the meshes seen from the view direction as visible and hidden lines. Edges are the outlines of
/// faces turned away from each other by more than `min_angle`, silhouettes and open boundaries.
pub fn flatshot(meshes: &[&Mesh], yaw: f64, pitch: f64, min_angle: f64) -> Vec<Line2> {
    let (right, up, away) = axes(yaw, pitch);
    let to_view = away.scale(-1.0);
    let proj = |p: V3| (pt(p.dot(right), p.dot(up)), p.dot(away));
    let size = meshes.iter().filter(|m| !m.vertices.is_empty()).map(|m| mesh_bounds(&m.vertices)).fold(1e-9f64, |a, b| a.max(b.max.x - b.min.x).max(b.max.y - b.min.y).max(b.max.z - b.min.z));
    let eps = 1e-7 * size;
    let topos: Vec<Topo> = meshes.iter().map(|m| Topo::new(m)).collect();
    let mut tris: Vec<Tri> = vec![];
    for t in &topos {
        for (_, f) in t.live_faces() {
            let pr = [proj(t.pos[f[0] as usize]), proj(t.pos[f[1] as usize]), proj(t.pos[f[2] as usize])];
            let mut p = [pr[0].0, pr[1].0, pr[2].0];
            let mut d = [pr[0].1, pr[1].1, pr[2].1];
            let a = cross(p[1] - p[0], p[2] - p[0]);
            if a.abs() < 1e-12 * size * size {
                continue;
            }
            if a < 0.0 {
                p.swap(1, 2);
                d.swap(1, 2);
            }
            let (lo, hi) = (pt(p.iter().map(|q| q.x).fold(f64::INFINITY, f64::min), p.iter().map(|q| q.y).fold(f64::INFINITY, f64::min)), pt(p.iter().map(|q| q.x).fold(f64::NEG_INFINITY, f64::max), p.iter().map(|q| q.y).fold(f64::NEG_INFINITY, f64::max)));
            tris.push(Tri { p, d, lo, hi });
        }
    }
    let cos_min = min_angle.cos();
    let mut out = vec![];
    for t in &topos {
        let mut keys: Vec<(&(u32, u32), &Vec<usize>)> = t.edges.iter().collect();
        keys.sort_by_key(|(k, _)| **k);
        for (&(a, b), fs) in keys {
            let shown = match fs.len() {
                2 => {
                    let (n1, n2) = (t.normal[fs[0]], t.normal[fs[1]]);
                    n1.dot(n2) < cos_min || n1.dot(to_view) * n2.dot(to_view) < -1e-12
                }
                _ => true,
            };
            if !shown {
                continue;
            }
            let ((p0, d0), (p1, d1)) = (proj(t.pos[a as usize]), proj(t.pos[b as usize]));
            let dir = p1 - p0;
            if len(dir) < 1e-9 * size {
                continue;
            }
            let (lo, hi) = (pt(p0.x.min(p1.x), p0.y.min(p1.y)), pt(p0.x.max(p1.x), p0.y.max(p1.y)));
            let mut hidden: Vec<(f64, f64)> = vec![];
            for tr in tris.iter().filter(|tr| tr.lo.x <= hi.x + eps && tr.hi.x >= lo.x - eps && tr.lo.y <= hi.y + eps && tr.hi.y >= lo.y - eps) {
                let Some((t0, t1, f0, df)) = overlap(tr, p0, dir, (d0, d1), eps) else { continue };
                // hidden where the face is nearer than the edge: f0 + df·t < -eps
                let (mut s, mut e) = (t0, t1);
                if df.abs() < 1e-15 {
                    if f0 >= -eps {
                        continue;
                    }
                } else {
                    let at = (-eps - f0) / df;
                    if df > 0.0 {
                        e = e.min(at);
                    } else {
                        s = s.max(at);
                    }
                }
                if e - s > 1e-9 {
                    hidden.push((s, e));
                }
            }
            hidden.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut merged: Vec<(f64, f64)> = vec![];
            for h in hidden {
                match merged.last_mut() {
                    Some(l) if h.0 <= l.1 + 1e-9 => l.1 = l.1.max(h.1),
                    _ => merged.push(h),
                }
            }
            let mut cursor = 0.0;
            let mut push = |s: f64, e: f64, visible: bool| {
                if e - s > 1e-9 {
                    out.push(Line2 { a: p0 + dir * s, b: p0 + dir * e, visible });
                }
            };
            for (s, e) in merged {
                push(cursor, s, true);
                push(s, e, false);
                cursor = e;
            }
            push(cursor, 1.0, true);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::mesh::{box_mesh, cylinder_mesh};
    use std::f64::consts::FRAC_PI_2;

    fn total(lines: &[Line2], visible: bool) -> f64 {
        lines.iter().filter(|l| l.visible == visible).map(|l| dist(l.a, l.b)).sum()
    }

    #[test]
    fn a_box_in_plan_view_is_its_outline_with_the_base_edges_hidden() {
        let b = box_mesh(0.0, 0.0, 0.0, 10.0, 6.0, 4.0, "B").mesh();
        let l = flatshot(&[&b], 0.0, FRAC_PI_2, 0.3);
        assert!((total(&l, true) - 32.0).abs() < 1e-6, "{}", total(&l, true));
        assert!((total(&l, false) - 32.0).abs() < 1e-6, "{}", total(&l, false));
    }

    #[test]
    fn a_box_in_front_view_shows_a_rectangle() {
        let b = box_mesh(0.0, 0.0, 0.0, 10.0, 6.0, 4.0, "B").mesh();
        let l = flatshot(&[&b], 0.0, 0.0, 0.3);
        assert!((total(&l, true) - 28.0).abs() < 1e-6, "{}", total(&l, true));
        let (xs, ys): (Vec<f64>, Vec<f64>) = (l.iter().flat_map(|l| [l.a.x, l.b.x]).collect(), l.iter().flat_map(|l| [l.a.y, l.b.y]).collect());
        assert!((xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - xs.iter().cloned().fold(f64::INFINITY, f64::min) - 10.0).abs() < 1e-6);
        assert!((ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - ys.iter().cloned().fold(f64::INFINITY, f64::min) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn a_box_on_top_hides_part_of_an_edge_below() {
        let big = box_mesh(0.0, 0.0, 0.0, 10.0, 10.0, 2.0, "A").mesh();
        let small = box_mesh(8.0, 4.0, 2.0, 4.0, 2.0, 2.0, "B").mesh();
        let l = flatshot(&[&big, &small], 0.0, FRAC_PI_2, 0.3);
        let on_edge: f64 = l.iter().filter(|s| s.visible && (s.a.x - 10.0).abs() < 1e-9 && (s.b.x - 10.0).abs() < 1e-9).map(|s| dist(s.a, s.b)).sum();
        assert!((on_edge - 8.0).abs() < 1e-4, "{on_edge}");
    }

    #[test]
    fn a_cylinder_seen_from_the_side_has_silhouettes() {
        let c = cylinder_mesh(0.0, 0.0, 0.0, 5.0, 10.0, 32, "C").mesh();
        let l = flatshot(&[&c], 0.0, 0.0, 0.5);
        let vertical: f64 = l.iter().filter(|s| s.visible && (s.a.x - s.b.x).abs() < 1e-9).map(|s| dist(s.a, s.b)).sum();
        assert!(vertical >= 20.0 - 1e-6, "two silhouette lines of height 10, got {vertical}");
    }
}
