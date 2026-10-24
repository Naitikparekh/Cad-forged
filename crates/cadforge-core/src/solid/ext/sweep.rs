//! Solids swept from planar outlines: revolve, sweep along a path, loft between sections, plus torus and pyramid.
use super::xform::rotate_axis;
use super::{orient_outward, region_tris as cap_tris, rings_of, segments_for, Welder};
use crate::geom::*;
use crate::solid::mesh::{group_loops, m3, mesh_bounds, mesh_volume, Group, Loop};
use crate::solid::{Mesh, Solid, V3};

/// Closed solid from rings carried through stations (`st[station][ring][point]`). Neighbouring stations are joined by
/// quads; unless `wrap` is set the ends are capped with `start` / `end` (index triples over the first / last
/// station's concatenated ring points). The result has outward faces whichever way the rings run.
pub(super) fn tube(
    st: &[Vec<Vec<V3>>],
    wrap: bool,
    start: &[[usize; 3]],
    end: &[[usize; 3]],
) -> Mesh {
    let mut w = Welder::default();
    let s = st.len();
    for k in 0..if wrap { s } else { s - 1 } {
        let (a, b) = (&st[k], &st[(k + 1) % s]);
        for (ra, rb) in a.iter().zip(b) {
            let n = ra.len();
            for i in 0..n {
                let j = (i + 1) % n;
                w.tri(ra[i], ra[j], rb[j]);
                w.tri(ra[i], rb[j], rb[i]);
            }
        }
    }
    if !wrap {
        let flat = |k: usize| -> Vec<V3> { st[k].iter().flatten().copied().collect() };
        let (f0, f1) = (flat(0), flat(s - 1));
        for t in start {
            w.tri(f0[t[0]], f0[t[2]], f0[t[1]]);
        }
        for t in end {
            w.tri(f1[t[0]], f1[t[1]], f1[t[2]]);
        }
    }
    let mut m = w.finish();
    orient_outward(&mut m);
    m
}

/// Reject results that enclose (almost) no volume.
fn checked(m: Mesh, name: &str, why: &str) -> Result<Solid, String> {
    let b = mesh_bounds(&m.vertices);
    let size = (b.max.x - b.min.x)
        .max(b.max.y - b.min.y)
        .max(b.max.z - b.min.z);
    if m.faces.is_empty() || mesh_volume(&m) < 1e-9 * size.powi(3).max(1e-12) {
        return Err(why.into());
    }
    Ok(Solid::from_mesh(name, m))
}

// ---------------------------------------------------------------- revolve and torus

/// Revolve planar outlines (in the plane z = 0) by `sweep` radians about the axis through `o` along `axis`,
/// starting `start` radians from the outline's plane.
pub fn revolve(
    loops: &[Loop],
    o: V3,
    axis: V3,
    start: f64,
    sweep: f64,
    name: &str,
) -> Result<Solid, String> {
    if axis.len() < 1e-12 {
        return Err("The axis of revolution needs two different points.".into());
    }
    if sweep.abs() < 1e-9 {
        return Err("The angle of revolution must not be zero.".into());
    }
    let u = axis.unit();
    let groups = group_loops(loops);
    let rings = rings_of(&groups);
    let dist_axis = |p: V3| p.sub(o).cross(u).len();
    let snap = |p: V3| {
        if dist_axis(p) < 1e-9 {
            o.add(u.scale(p.sub(o).dot(u)))
        } else {
            p
        }
    };
    if u.z.abs() < 1e-9 && o.z.abs() < 1e-9 {
        let side = |p: &Pt| u.x * (p.y - o.y) - u.y * (p.x - o.x);
        let (lo, hi) = rings
            .iter()
            .flat_map(|r| r.iter())
            .map(side)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(v), b.max(v))
            });
        if lo < -1e-9 && hi > 1e-9 {
            return Err("The outline crosses the axis of revolution.".into());
        }
    }
    let full = sweep.abs() >= TAU - 1e-9;
    let rmax = rings
        .iter()
        .flat_map(|r| r.iter())
        .map(|p| dist_axis(V3::new(p.x, p.y, 0.0)))
        .fold(0.0, f64::max);
    let n = ((segments_for(rmax) as f64 * sweep.abs().min(TAU) / TAU).ceil() as usize).max(2);
    let steps = if full { n } else { n + 1 };
    let st: Vec<Vec<Vec<V3>>> = (0..steps)
        .map(|k| {
            let m = rotate_axis(
                o,
                u,
                start + sweep.signum() * sweep.abs().min(TAU) * k as f64 / n as f64,
            );
            rings
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|p| m3::apply(&m, snap(V3::new(p.x, p.y, 0.0))))
                        .collect()
                })
                .collect()
        })
        .collect();
    let caps = cap_tris(&groups)?;
    checked(
        tube(&st, full, &caps, &caps),
        name,
        "The outline does not enclose any volume when revolved about this axis.",
    )
}

/// Torus centred at `c` in the XY plane with ring radius `major` and tube radius `minor`.
pub fn torus(c: V3, major: f64, minor: f64, name: &str) -> Result<Solid, String> {
    if minor <= 0.0 || major <= 0.0 {
        return Err("The torus and tube radii must be positive.".into());
    }
    if minor >= major {
        return Err("The tube radius must be smaller than the torus radius.".into());
    }
    let (nu, nv) = (segments_for(major + minor), segments_for(minor));
    let mut vertices = Vec::with_capacity(nu * nv);
    for i in 0..nu {
        let u = TAU * i as f64 / nu as f64;
        for j in 0..nv {
            let v = TAU * j as f64 / nv as f64;
            let rr = major + minor * v.cos();
            vertices.push(V3::new(
                c.x + rr * u.cos(),
                c.y + rr * u.sin(),
                c.z + minor * v.sin(),
            ));
        }
    }
    let at = |i: usize, j: usize| ((i % nu) * nv + j % nv) as u32;
    let mut faces = vec![];
    for i in 0..nu {
        for j in 0..nv {
            faces.push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
            faces.push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
        }
    }
    let mut m = Mesh { vertices, faces };
    orient_outward(&mut m);
    Ok(Solid::from_mesh(name, m))
}

// ---------------------------------------------------------------- sweep

#[derive(Clone, Copy, Debug)]
pub struct SweepOpts {
    /// Turn the outline perpendicular to the path (otherwise it is only translated along it).
    pub align: bool,
    /// Point of the outline that follows the path.
    pub base: Pt,
    /// Scale factor reached at the end of the path.
    pub scale: f64,
    /// Total twist (radians) reached at the end of the path.
    pub twist: f64,
}

impl Default for SweepOpts {
    fn default() -> SweepOpts {
        SweepOpts {
            align: true,
            base: Pt::ZERO,
            scale: 1.0,
            twist: 0.0,
        }
    }
}

/// Frame (profile x, profile y, travel direction) at the start of a path: profile y points up where possible.
fn start_frame(t: V3) -> [V3; 3] {
    let z = V3::new(0.0, 0.0, 1.0);
    let mut up = z.sub(t.scale(z.dot(t)));
    if up.len() < 1e-6 {
        up = V3::new(0.0, 1.0, 0.0).sub(t.scale(t.y));
    }
    let ey = up.unit();
    [ey.cross(t).unit(), ey, t]
}

/// Smallest rotation taking direction `a` to `b`.
fn min_rotation(a: V3, b: V3) -> Result<crate::solid::mesh::M3, String> {
    let axis = a.cross(b);
    if axis.len() < 1e-12 {
        if a.dot(b) < 0.0 {
            return Err("The sweep path doubles back on itself.".into());
        }
        return Ok(m3::identity());
    }
    Ok(rotate_axis(V3::default(), axis, axis.len().atan2(a.dot(b))))
}

/// Sweep planar outlines along a path polyline. Interior path points get mitred sections so that the sweep keeps
/// its thickness around corners.
pub fn sweep(
    loops: &[Loop],
    path: &[V3],
    closed: bool,
    o: &SweepOpts,
    name: &str,
) -> Result<Solid, String> {
    let mut p: Vec<V3> = vec![];
    for q in path {
        if p.last().map_or(true, |l| q.sub(*l).len() > 1e-9) {
            p.push(*q);
        }
    }
    if closed && p.len() > 1 && p[0].sub(p[p.len() - 1]).len() < 1e-9 {
        p.pop();
    }
    let n = p.len();
    if n < if closed { 3 } else { 2 } {
        return Err("The sweep path is too short.".into());
    }
    let m = if closed { n } else { n - 1 };
    let t: Vec<V3> = (0..m).map(|j| p[(j + 1) % n].sub(p[j]).unit()).collect();
    let mut s = vec![0.0];
    for j in 0..m {
        s.push(s[j] + p[(j + 1) % n].sub(p[j]).len());
    }
    let total = s[m];
    let mut frames = vec![start_frame(t[0])];
    for j in 1..m {
        let r = min_rotation(t[j - 1], t[j])?;
        let f = frames[j - 1];
        frames.push([m3::apply(&r, f[0]), m3::apply(&r, f[1]), t[j]]);
    }
    let groups = group_loops(loops);
    let rings = rings_of(&groups);
    let st: Vec<Vec<Vec<V3>>> = (0..n)
        .map(|i| {
            let frac = s[i] / total;
            let (k, ang) = (1.0 + (o.scale - 1.0) * frac, o.twist * frac);
            let (tin, tout) = if closed {
                (t[(i + m - 1) % m], t[i % m])
            } else {
                (t[i.saturating_sub(1).min(m - 1)], t[i.min(m - 1)])
            };
            let bis = if closed || (i > 0 && i < n - 1) {
                tin.add(tout).unit()
            } else {
                tin
            };
            let fr = frames[if closed {
                (i + m - 1) % m
            } else {
                i.saturating_sub(1).min(m - 1)
            }];
            let place = |q: &Pt| -> V3 {
                let d = *q - o.base;
                let (c, sn) = (ang.cos(), ang.sin());
                let w = pt(k * (c * d.x - sn * d.y), k * (sn * d.x + c * d.y));
                if !o.align {
                    return V3::new(o.base.x + w.x, o.base.y + w.y, 0.0).add(p[i].sub(p[0]));
                }
                let off = fr[0].scale(w.x).add(fr[1].scale(w.y));
                let shift = -off.dot(bis) / tin.dot(bis).max(1e-9);
                p[i].add(off).add(tin.scale(shift))
            };
            rings
                .iter()
                .map(|r| r.iter().map(&place).collect())
                .collect()
        })
        .collect();
    let caps = cap_tris(&groups)?;
    checked(
        tube(&st, closed, &caps, &caps),
        name,
        "The outline lies along the sweep path, so the sweep has no volume.",
    )
}

// ---------------------------------------------------------------- loft

fn loop_len(l: &[Pt]) -> f64 {
    (0..l.len()).map(|i| dist(l[i], l[(i + 1) % l.len()])).sum()
}

/// Point at fraction `t` (0..1) of the loop's perimeter.
fn point_at(l: &[Pt], len: f64, t: f64) -> Pt {
    let mut want = t.rem_euclid(1.0) * len;
    for i in 0..l.len() {
        let (a, b) = (l[i], l[(i + 1) % l.len()]);
        let d = dist(a, b);
        if want <= d || i == l.len() - 1 {
            return lerp(a, b, if d < 1e-12 { 0.0 } else { (want / d).min(1.0) });
        }
        want -= d;
    }
    l[0]
}

/// Perimeter fractions of the loop's own vertices.
fn vertex_params(l: &[Pt]) -> Vec<f64> {
    let len = loop_len(l);
    let mut acc = 0.0;
    (0..l.len())
        .map(|i| {
            let t = acc / len;
            acc += dist(l[i], l[(i + 1) % l.len()]);
            t
        })
        .collect()
}

/// Loft solid through the sections (placed at heights `z`). Sections are matched by perimeter fraction, so corners
/// stay corners; with `smooth` and three or more sections the surface passes smoothly through them.
pub fn loft(sections: &[Loop], z: &[f64], smooth: bool, name: &str) -> Result<Solid, String> {
    if sections.len() < 2 || z.len() != sections.len() {
        return Err("A loft needs at least two cross-sections.".into());
    }
    let mut secs: Vec<Loop> = vec![];
    for l in sections {
        let mut l: Loop = l.iter().copied().fold(vec![], |mut v: Loop, p| {
            if v.last().map_or(true, |q| dist(*q, p) > 1e-9) {
                v.push(p);
            }
            v
        });
        if l.len() < 3 {
            return Err("Each loft cross-section must be a closed outline.".into());
        }
        if polygon_area(&l) < 0.0 {
            l.reverse();
        }
        secs.push(l);
    }
    for k in 1..secs.len() {
        let plen = loop_len(&secs[k - 1]);
        let prev: Vec<Pt> = (0..48)
            .map(|i| point_at(&secs[k - 1], plen, i as f64 / 48.0))
            .collect();
        let cur = secs[k].clone();
        let cost = |sh: usize| -> f64 {
            let rot: Loop = (0..cur.len()).map(|i| cur[(i + sh) % cur.len()]).collect();
            let len = loop_len(&rot);
            (0..48)
                .map(|i| dist(prev[i], point_at(&rot, len, i as f64 / 48.0)).powi(2))
                .sum()
        };
        let best = (0..cur.len())
            .map(|sh| (cost(sh), sh))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(0, |b| b.1);
        secs[k] = (0..cur.len())
            .map(|i| cur[(i + best) % cur.len()])
            .collect();
    }
    let mut params: Vec<f64> = secs.iter().flat_map(|l| vertex_params(l)).collect();
    params.sort_by(f64::total_cmp);
    params.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    if params.len() > 512 {
        params = (0..256).map(|i| i as f64 / 256.0).collect();
    }
    let sampled: Vec<Loop> = secs
        .iter()
        .map(|l| {
            params
                .iter()
                .map(|t| point_at(l, loop_len(l), *t))
                .collect()
        })
        .collect();
    let lift =
        |k: usize| -> Vec<V3> { sampled[k].iter().map(|p| V3::new(p.x, p.y, z[k])).collect() };
    let raw: Vec<Vec<V3>> = (0..sampled.len()).map(lift).collect();
    let stations: Vec<Vec<V3>> = if smooth && raw.len() >= 3 {
        catmull_rom(&raw, 8)
    } else {
        raw.clone()
    };
    let st: Vec<Vec<Vec<V3>>> = stations.into_iter().map(|r| vec![r]).collect();
    let group = |l: &Loop| Group {
        outer: l.clone(),
        holes: vec![],
    };
    let (first, last) = (
        cap_tris(&[group(&sampled[0])])?,
        cap_tris(&[group(&sampled[sampled.len() - 1])])?,
    );
    checked(
        tube(&st, false, &first, &last),
        name,
        "The cross-sections do not enclose any volume.",
    )
}

/// Catmull-Rom interpolation of corresponding points across the rows, `sub` stations between neighbouring rows.
fn catmull_rom(rows: &[Vec<V3>], sub: usize) -> Vec<Vec<V3>> {
    let n = rows.len();
    let at = |k: isize| &rows[k.clamp(0, n as isize - 1) as usize];
    let mut out = vec![];
    for k in 0..n - 1 {
        for s in 0..sub {
            let t = s as f64 / sub as f64;
            let (p0, p1, p2, p3) = (
                at(k as isize - 1),
                at(k as isize),
                at(k as isize + 1),
                at(k as isize + 2),
            );
            out.push(
                (0..p1.len())
                    .map(|i| {
                        let f = |a: f64, b: f64, c: f64, d: f64| {
                            0.5 * ((2.0 * b)
                                + (-a + c) * t
                                + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                                + (-a + 3.0 * b - 3.0 * c + d) * t * t * t)
                        };
                        V3::new(
                            f(p0[i].x, p1[i].x, p2[i].x, p3[i].x),
                            f(p0[i].y, p1[i].y, p2[i].y, p3[i].y),
                            f(p0[i].z, p1[i].z, p2[i].z, p3[i].z),
                        )
                    })
                    .collect(),
            );
        }
    }
    out.push(rows[n - 1].clone());
    out
}

// ---------------------------------------------------------------- pyramid

/// Pyramid or frustum with a regular `sides`-gon base of circumradius `r1` at `z0` and top circumradius `r2`
/// (0 for an apex) at `z0 + h`; the first base vertex lies at angle `rot`.
pub fn pyramid(
    cx: f64,
    cy: f64,
    z0: f64,
    r1: f64,
    r2: f64,
    h: f64,
    sides: usize,
    rot: f64,
    name: &str,
) -> Result<Solid, String> {
    if sides < 3 || r1 <= 0.0 || r2 < 0.0 || h == 0.0 {
        return Err(
            "A pyramid needs at least three sides, a positive base radius and a height.".into(),
        );
    }
    let ring = |r: f64, z: f64| -> Vec<V3> {
        (0..sides)
            .map(|i| {
                V3::new(
                    cx + r * (rot + TAU * i as f64 / sides as f64).cos(),
                    cy + r * (rot + TAU * i as f64 / sides as f64).sin(),
                    z,
                )
            })
            .collect()
    };
    let (bot, top) = (ring(r1, z0), ring(r2, z0 + h));
    let mut w = Welder::default();
    for i in 0..sides {
        let j = (i + 1) % sides;
        w.tri(bot[i], bot[j], top[j]);
        w.tri(bot[i], top[j], top[i]);
        w.tri(V3::new(cx, cy, z0 + h), top[i], top[j]);
        w.tri(V3::new(cx, cy, z0), bot[j], bot[i]);
    }
    let mut m = w.finish();
    orient_outward(&mut m);
    Ok(Solid::from_mesh(name, m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::mesh::is_watertight;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
        vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)]
    }

    fn circle(c: Pt, r: f64, n: usize) -> Loop {
        (0..n)
            .map(|i| {
                pt(
                    c.x + r * (TAU * i as f64 / n as f64).cos(),
                    c.y + r * (TAU * i as f64 / n as f64).sin(),
                )
            })
            .collect()
    }

    fn vol(s: &Solid) -> f64 {
        mesh_volume(&s.mesh())
    }

    #[test]
    fn revolving_a_rectangle_gives_a_hollow_cylinder() {
        let s = revolve(
            &[rect(2.0, 0.0, 5.0, 10.0)],
            V3::default(),
            V3::new(0.0, 1.0, 0.0),
            0.0,
            TAU,
            "Revolution",
        )
        .unwrap();
        let exact = PI * (25.0 - 4.0) * 10.0;
        assert!(
            (vol(&s) - exact).abs() / exact < 0.01,
            "{} vs {exact}",
            vol(&s)
        );
        assert!(is_watertight(&s.mesh()));
    }

    #[test]
    fn revolve_works_on_either_side_of_the_axis_and_for_partial_angles() {
        for x in [(3.0, 6.0), (-6.0, -3.0)] {
            let s = revolve(
                &[rect(x.0, 0.0, x.1, 4.0)],
                V3::default(),
                V3::new(0.0, 1.0, 0.0),
                0.0,
                PI,
                "R",
            )
            .unwrap();
            let exact = PI * (36.0 - 9.0) * 4.0 / 2.0;
            assert!(
                (vol(&s) - exact).abs() / exact < 0.01,
                "{} vs {exact}",
                vol(&s)
            );
            assert!(is_watertight(&s.mesh()));
        }
        let neg = revolve(
            &[rect(3.0, 0.0, 6.0, 4.0)],
            V3::default(),
            V3::new(0.0, 1.0, 0.0),
            0.0,
            -PI / 2.0,
            "R",
        )
        .unwrap();
        assert!(vol(&neg) > 0.0 && is_watertight(&neg.mesh()));
    }

    #[test]
    fn a_profile_touching_the_axis_gives_a_cone_and_crossing_it_fails() {
        let tri = vec![pt(0.0, 0.0), pt(3.0, 0.0), pt(0.0, 6.0)];
        let s = revolve(&[tri], V3::default(), V3::new(0.0, 1.0, 0.0), 0.0, TAU, "R").unwrap();
        let exact = PI * 9.0 * 6.0 / 3.0;
        assert!(
            (vol(&s) - exact).abs() / exact < 0.01,
            "{} vs {exact}",
            vol(&s)
        );
        assert!(is_watertight(&s.mesh()));
        assert!(revolve(
            &[rect(-1.0, 0.0, 2.0, 3.0)],
            V3::default(),
            V3::new(0.0, 1.0, 0.0),
            0.0,
            TAU,
            "R"
        )
        .is_err());
        assert!(revolve(
            &[rect(1.0, 0.0, 2.0, 3.0)],
            V3::default(),
            V3::new(0.0, 0.0, 1.0),
            0.0,
            TAU,
            "R"
        )
        .is_err());
    }

    #[test]
    fn revolving_about_a_slanted_axis_with_a_hole() {
        let ring = vec![
            circle(pt(10.0, 0.0), 3.0, 64),
            circle(pt(10.0, 0.0), 1.0, 64),
        ];
        let s = revolve(&ring, V3::default(), V3::new(0.0, 1.0, 0.0), 0.0, TAU, "R").unwrap();
        let exact = 2.0 * PI * 10.0 * PI * (9.0 - 1.0);
        assert!(
            (vol(&s) - exact).abs() / exact < 0.01,
            "{} vs {exact}",
            vol(&s)
        );
        assert!(is_watertight(&s.mesh()));
    }

    #[test]
    fn torus_volume_and_validation() {
        let s = torus(V3::default(), 10.0, 2.0, "Torus").unwrap();
        let exact = 2.0 * PI * PI * 10.0 * 4.0;
        assert!(
            (vol(&s) - exact).abs() / exact < 0.02,
            "{} vs {exact}",
            vol(&s)
        );
        assert!(is_watertight(&s.mesh()));
        assert!(torus(V3::default(), 2.0, 2.0, "T").is_err());
        assert!(torus(V3::default(), 2.0, 0.0, "T").is_err());
    }

    #[test]
    fn sweeping_a_square_along_a_straight_line() {
        let path = [V3::new(0.0, 0.0, 0.0), V3::new(20.0, 0.0, 0.0)];
        let s = sweep(
            &[rect(-1.0, -1.0, 1.0, 1.0)],
            &path,
            false,
            &SweepOpts::default(),
            "Sweep",
        )
        .unwrap();
        assert!((vol(&s) - 80.0).abs() < 1e-6, "{}", vol(&s));
        assert!(is_watertight(&s.mesh()));
    }

    #[test]
    fn sweep_corners_are_mitred_and_closed_paths_wrap() {
        let l_path = [
            V3::new(0.0, 0.0, 0.0),
            V3::new(10.0, 0.0, 0.0),
            V3::new(10.0, 10.0, 0.0),
        ];
        let s = sweep(
            &[rect(-1.0, -1.0, 1.0, 1.0)],
            &l_path,
            false,
            &SweepOpts::default(),
            "Sweep",
        )
        .unwrap();
        // two 10-long legs of 2x2 section; the mitre keeps the section area at the corner
        assert!((vol(&s) - 80.0).abs() < 1e-6, "{}", vol(&s));
        assert!(is_watertight(&s.mesh()));
        let ring = [
            V3::new(0.0, 0.0, 0.0),
            V3::new(10.0, 0.0, 0.0),
            V3::new(10.0, 10.0, 0.0),
            V3::new(0.0, 10.0, 0.0),
        ];
        let r = sweep(
            &[rect(-1.0, -1.0, 1.0, 1.0)],
            &ring,
            true,
            &SweepOpts::default(),
            "Sweep",
        )
        .unwrap();
        assert!((vol(&r) - 160.0).abs() < 1e-6, "{}", vol(&r));
        assert!(is_watertight(&r.mesh()));
    }

    #[test]
    fn sweep_scale_and_twist_and_unaligned_sweeps() {
        let path = [V3::new(0.0, 0.0, 0.0), V3::new(10.0, 0.0, 0.0)];
        let sq = rect(-1.0, -1.0, 1.0, 1.0);
        let o = SweepOpts {
            scale: 2.0,
            ..SweepOpts::default()
        };
        let s = sweep(&[sq.clone()], &path, false, &o, "S").unwrap();
        // linear taper: 2x2 -> 4x4 over 10 => frustum volume
        let exact = 10.0 / 3.0 * (4.0 + 16.0 + 8.0);
        assert!((vol(&s) - exact).abs() < 1e-6, "{} vs {exact}", vol(&s));
        let tw = sweep(
            &[sq.clone()],
            &path,
            false,
            &SweepOpts {
                twist: PI / 2.0,
                ..SweepOpts::default()
            },
            "S",
        )
        .unwrap();
        assert!(vol(&tw) > 0.0);
        let flat = sweep(
            &[sq],
            &path,
            false,
            &SweepOpts {
                align: false,
                ..SweepOpts::default()
            },
            "S",
        );
        assert!(flat.is_err());
    }

    #[test]
    fn loft_between_equal_squares_is_a_prism_and_a_frustum_otherwise() {
        let prism = loft(
            &[rect(0.0, 0.0, 10.0, 10.0), rect(0.0, 0.0, 10.0, 10.0)],
            &[0.0, 10.0],
            false,
            "Loft",
        )
        .unwrap();
        assert!((vol(&prism) - 1000.0).abs() < 1e-6);
        assert!(is_watertight(&prism.mesh()));
        let fr = loft(
            &[rect(0.0, 0.0, 10.0, 10.0), rect(3.0, 3.0, 7.0, 7.0)],
            &[0.0, 6.0],
            false,
            "Loft",
        )
        .unwrap();
        let exact = 6.0 / 3.0 * (100.0 + 16.0 + 40.0);
        assert!((vol(&fr) - exact).abs() < 1e-6, "{} vs {exact}", vol(&fr));
        assert!(is_watertight(&fr.mesh()));
    }

    #[test]
    fn loft_square_to_circle_and_three_smooth_sections() {
        let s = loft(
            &[rect(-5.0, -5.0, 5.0, 5.0), circle(pt(0.0, 0.0), 3.0, 48)],
            &[0.0, 8.0],
            false,
            "Loft",
        )
        .unwrap();
        assert!(is_watertight(&s.mesh()));
        let v = vol(&s);
        assert!(v > 8.0 * PI * 9.0 && v < 800.0, "{v}");
        let three = loft(
            &[
                circle(pt(0.0, 0.0), 2.0, 32),
                circle(pt(0.0, 0.0), 4.0, 32),
                circle(pt(0.0, 0.0), 2.0, 32),
            ],
            &[0.0, 5.0, 10.0],
            true,
            "Loft",
        )
        .unwrap();
        assert!(is_watertight(&three.mesh()));
        assert!(vol(&three) > PI * 4.0 * 10.0);
        assert!(loft(&[rect(0.0, 0.0, 1.0, 1.0)], &[0.0], false, "L").is_err());
    }

    #[test]
    fn pyramid_and_frustum_volumes() {
        let p = pyramid(0.0, 0.0, 0.0, 5.0, 0.0, 9.0, 4, 0.0, "Pyramid").unwrap();
        // square with circumradius 5 has area 2 r^2 = 50
        assert!((vol(&p) - 50.0 * 9.0 / 3.0).abs() < 1e-6, "{}", vol(&p));
        assert!(is_watertight(&p.mesh()));
        let f = pyramid(0.0, 0.0, 0.0, 5.0, 2.0, 9.0, 4, 0.0, "Pyramid").unwrap();
        let (a1, a2): (f64, f64) = (50.0, 8.0);
        assert!((vol(&f) - 9.0 / 3.0 * (a1 + a2 + (a1 * a2).sqrt())).abs() < 1e-6);
        assert!(is_watertight(&f.mesh()));
        assert!(pyramid(0.0, 0.0, 0.0, 5.0, 0.0, 9.0, 2, 0.0, "P").is_err());
    }
}
