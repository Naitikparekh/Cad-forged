//! Picking solids through the camera: nearest triangle under a screen point, and projected-box window selection
//! (port of `src/solid/pick.js`).
use crate::view::{Projected, View};
use cadforge_core::geom::{Bounds, Pt};
use cadforge_core::solid::Solid;

fn project(solid: &Solid, view: &View) -> Vec<Projected> {
    solid.vertices.iter().map(|p| view.project(*p)).collect()
}

fn bary(p: Pt, a: &Projected, b: &Projected, c: &Projected) -> Option<[f64; 3]> {
    let d = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if d.abs() < 1e-12 {
        return None;
    }
    let u = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / d;
    let v = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / d;
    Some([u, v, 1.0 - u - v])
}

/// Distance from p to segment ab and the interpolated depth at the nearest point.
fn seg_hit(p: Pt, a: &Projected, b: &Projected) -> (f64, f64) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l = dx * dx + dy * dy;
    let t = if l < 1e-12 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l).clamp(0.0, 1.0) };
    ((p.x - a.x - dx * t).hypot(p.y - a.y - dy * t), a.depth + (b.depth - a.depth) * t)
}

/// Nearest solid under screen point `sp`. With `interior` false (wireframe) only edges within `aperture` pixels
/// count. Returns (solid id, depth).
pub fn pick_solid(solids: &[Solid], view: &View, sp: Pt, aperture: f64, interior: bool, is_visible: &dyn Fn(&Solid) -> bool) -> Option<(u64, f64)> {
    let mut best: Option<(u64, f64)> = None;
    for s in solids {
        if !is_visible(s) {
            continue;
        }
        let pr = project(s, view);
        for f in &s.faces {
            let (a, b, c) = (&pr[f[0] as usize], &pr[f[1] as usize], &pr[f[2] as usize]);
            let mut depth: Option<f64> = None;
            let w = if interior { bary(sp, a, b, c) } else { None };
            match w {
                Some(w) if w[0] >= -1e-9 && w[1] >= -1e-9 && w[2] >= -1e-9 => depth = Some(w[0] * a.depth + w[1] * b.depth + w[2] * c.depth),
                _ => {
                    for (p, q) in [(a, b), (b, c), (c, a)] {
                        let (d, dep) = seg_hit(sp, p, q);
                        if d <= aperture && depth.map_or(true, |cur| dep < cur) {
                            depth = Some(dep - 1e-3);
                        }
                    }
                }
            }
            if let Some(d) = depth {
                if best.map_or(true, |b| d < b.1) {
                    best = Some((s.id, d));
                }
            }
        }
    }
    best
}

/// Screen-space bounding box of a solid.
pub fn solid_screen_bounds(solid: &Solid, view: &View) -> Bounds {
    let p: Vec<Pt> = project(solid, view).iter().map(|q| Pt { x: q.x, y: q.y }).collect();
    Bounds::of_points(&p)
}

/// Solids inside (window) or touching (crossing) a screen rectangle.
pub fn solids_in_screen_rect(solids: &[Solid], view: &View, rect: &Bounds, crossing: bool, is_visible: &dyn Fn(&Solid) -> bool) -> Vec<u64> {
    solids
        .iter()
        .filter(|s| {
            if !is_visible(s) || s.vertices.is_empty() {
                return false;
            }
            let b = solid_screen_bounds(s, view);
            if crossing {
                b.overlaps(rect)
            } else {
                b.inside(rect)
            }
        })
        .map(|s| s.id)
        .collect()
}
