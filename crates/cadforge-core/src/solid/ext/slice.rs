//! Cutting closed triangle meshes with a plane: the two capped halves and the outline of the cut.
use super::{region_tris, rings_of, Welder};
use crate::geom::*;
use crate::solid::mesh::group_loops;
use crate::solid::{Mesh, V3};
use std::collections::HashMap;

/// Plane `n · p = d` with a unit normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub n: V3,
    pub d: f64,
}

impl Plane {
    /// Plane through `p` with normal `n`; `None` if `n` has no direction.
    pub fn new(p: V3, n: V3) -> Option<Plane> {
        (n.len() > 1e-12).then(|| {
            let n = n.unit();
            Plane { n, d: n.dot(p) }
        })
    }

    /// Plane through three points; `None` if they are collinear.
    pub fn through(a: V3, b: V3, c: V3) -> Option<Plane> {
        Plane::new(a, b.sub(a).cross(c.sub(a)))
    }

    /// Signed distance of `p` from the plane (positive on the side the normal points to).
    pub fn dist(&self, p: V3) -> f64 {
        self.n.dot(p) - self.d
    }

    /// In-plane axes `(u, v)` with `u × v = n`; `u` is horizontal unless the plane is.
    pub fn basis(&self) -> (V3, V3) {
        let u = if self.n.z.abs() > 0.999_999 { V3::new(1.0, 0.0, 0.0) } else { V3::new(0.0, 0.0, 1.0).cross(self.n).unit() };
        (u, self.n.cross(u))
    }

    /// The point of the plane closest to the origin.
    pub fn origin(&self) -> V3 {
        self.n.scale(self.d)
    }
}

/// The parts of a mesh on each side of a plane (`None` when nothing lies there).
#[derive(Clone, Debug, Default)]
pub struct Sliced {
    /// Side the plane normal points to.
    pub front: Option<Mesh>,
    pub back: Option<Mesh>,
}

const SIDE_TOL: f64 = 1e-7;

/// Join unordered segments into loops. Returns the closed loops and whether every segment ended up in one.
fn chain(segs: &[(V3, V3)], tol: f64) -> (Vec<Vec<V3>>, bool) {
    let cs = tol * 4.0;
    let cell = |p: V3| ((p.x / cs).floor() as i64, (p.y / cs).floor() as i64, (p.z / cs).floor() as i64);
    let end = |id: usize| if id % 2 == 0 { segs[id / 2].0 } else { segs[id / 2].1 };
    let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for id in 0..segs.len() * 2 {
        grid.entry(cell(end(id))).or_default().push(id);
    }
    let mut used = vec![false; segs.len()];
    let near = |p: V3, used: &[bool]| -> Option<usize> {
        let c = cell(p);
        let mut best: Option<(f64, usize)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &id in grid.get(&(c.0 + dx, c.1 + dy, c.2 + dz)).into_iter().flatten() {
                        let d = end(id).sub(p).len();
                        if !used[id / 2] && d <= tol && best.map_or(true, |b| d < b.0) {
                            best = Some((d, id));
                        }
                    }
                }
            }
        }
        best.map(|b| b.1)
    };
    let (mut loops, mut ok) = (vec![], true);
    for s0 in 0..segs.len() {
        if used[s0] {
            continue;
        }
        used[s0] = true;
        let (start, mut cur) = segs[s0];
        let mut pts = vec![start];
        let closed = loop {
            if cur.sub(start).len() <= tol {
                break true;
            }
            let Some(id) = near(cur, &used) else { break false };
            used[id / 2] = true;
            pts.push(cur);
            cur = end(id ^ 1);
        };
        if closed && pts.len() >= 3 {
            loops.push(pts);
        } else {
            ok = false;
        }
    }
    (loops, ok)
}

/// Add `seg` unless an equal one (in either direction) is already present.
fn push_cut(cuts: &mut Vec<(V3, V3)>, seen: &mut std::collections::HashSet<[i64; 6]>, a: V3, b: V3) {
    let q = |p: V3| [(p.x * 1e6).round() as i64, (p.y * 1e6).round() as i64, (p.z * 1e6).round() as i64];
    let (qa, qb) = (q(a), q(b));
    if qa == qb {
        return;
    }
    let key = if qa < qb { [qa[0], qa[1], qa[2], qb[0], qb[1], qb[2]] } else { [qb[0], qb[1], qb[2], qa[0], qa[1], qa[2]] };
    if seen.insert(key) {
        cuts.push((a, b));
    }
}

/// Cut segments where the mesh crosses the plane (edges lying in the plane included), and the signed vertex distances.
fn cut_segments(m: &Mesh, pl: &Plane) -> (Vec<(V3, V3)>, Vec<f64>) {
    let s: Vec<f64> = m.vertices.iter().map(|p| pl.dist(*p)).map(|d| if d.abs() < SIDE_TOL { 0.0 } else { d }).collect();
    let (mut cuts, mut seen) = (vec![], std::collections::HashSet::new());
    for f in &m.faces {
        let (sv, pv) = ([s[f[0] as usize], s[f[1] as usize], s[f[2] as usize]], [m.vertices[f[0] as usize], m.vertices[f[1] as usize], m.vertices[f[2] as usize]]);
        let mut on_plane: Vec<V3> = (0..3).filter(|&i| sv[i] == 0.0).map(|i| pv[i]).collect();
        for i in 0..3 {
            let j = (i + 1) % 3;
            if sv[i] * sv[j] < 0.0 {
                on_plane.push(crossing(f, &pv, &sv, i, j));
            }
        }
        if on_plane.len() == 2 {
            push_cut(&mut cuts, &mut seen, on_plane[0], on_plane[1]);
        }
    }
    (cuts, s)
}

/// Where the triangle edge `i`–`j` crosses the plane, computed the same way from both neighbouring triangles.
fn crossing(f: &[u32; 3], pv: &[V3; 3], sv: &[f64; 3], i: usize, j: usize) -> V3 {
    let (a, b) = if f[i] < f[j] { (i, j) } else { (j, i) };
    let t = sv[a] / (sv[a] - sv[b]);
    pv[a].add(pv[b].sub(pv[a]).scale(t))
}

/// Closed outlines where the plane cuts the mesh.
pub fn section_loops(m: &Mesh, pl: &Plane) -> Vec<Vec<V3>> {
    chain(&cut_segments(m, pl).0, 1e-6).0
}

/// Triangulated cap for the cut outlines; its triangles face along the plane normal.
fn cap(loops: &[Vec<V3>], pl: &Plane) -> Result<Vec<[V3; 3]>, String> {
    let (u, v) = pl.basis();
    let o = pl.origin();
    let flat: Vec<Vec<Pt>> = loops.iter().map(|l| l.iter().map(|p| pt(p.sub(o).dot(u), p.sub(o).dot(v))).collect()).collect();
    let mut back: HashMap<(u64, u64), V3> = HashMap::new();
    for (l3, l2) in loops.iter().zip(&flat) {
        for (p3, p2) in l3.iter().zip(l2) {
            back.insert((p2.x.to_bits(), p2.y.to_bits()), *p3);
        }
    }
    let groups = group_loops(&flat);
    let pts: Vec<V3> = rings_of(&groups).iter().flat_map(|r| r.iter().map(|p| back[&(p.x.to_bits(), p.y.to_bits())])).collect();
    Ok(region_tris(&groups)?.into_iter().map(|t| [pts[t[0]], pts[t[1]], pts[t[2]]]).collect())
}

/// Clip the triangle to the half-space (`keep_pos` selects the side). New points are computed from the lower vertex
/// index first so that neighbouring triangles agree on them bit for bit.
fn clip(f: &[u32; 3], pv: &[V3; 3], sv: &[f64; 3], keep_pos: bool) -> Vec<V3> {
    let inside = |v: f64| if keep_pos { v >= 0.0 } else { v <= 0.0 };
    let mut out = vec![];
    for i in 0..3 {
        let j = (i + 1) % 3;
        if inside(sv[i]) {
            out.push(pv[i]);
        }
        if sv[i] * sv[j] < 0.0 {
            out.push(crossing(f, pv, sv, i, j));
        }
    }
    out
}

/// Cut a closed mesh with the plane. Both halves are closed by a flat cap on the plane.
pub fn slice(m: &Mesh, pl: &Plane) -> Result<Sliced, String> {
    let (cuts, s) = cut_segments(m, pl);
    let (any_pos, any_neg) = (s.iter().any(|v| *v > 0.0), s.iter().any(|v| *v < 0.0));
    if !any_neg || !any_pos {
        let whole = Some(m.clone());
        return Ok(if any_pos { Sliced { front: whole, back: None } } else { Sliced { front: None, back: whole } });
    }
    let (mut front, mut back) = (Welder::default(), Welder::default());
    for f in &m.faces {
        let sv = [s[f[0] as usize], s[f[1] as usize], s[f[2] as usize]];
        let pv = [m.vertices[f[0] as usize], m.vertices[f[1] as usize], m.vertices[f[2] as usize]];
        let (pos, neg) = (sv.iter().any(|v| *v > 0.0), sv.iter().any(|v| *v < 0.0));
        match (pos, neg) {
            (true, false) => front.tri(pv[0], pv[1], pv[2]),
            (false, true) => back.tri(pv[0], pv[1], pv[2]),
            (true, true) => {
                front.poly(&clip(f, &pv, &sv, true));
                back.poly(&clip(f, &pv, &sv, false));
            }
            (false, false) => {
                // lies in the plane: it bounds the half the solid does not occupy
                let n = pv[1].sub(pv[0]).cross(pv[2].sub(pv[0]));
                if n.dot(pl.n) > 0.0 {
                    back.tri(pv[0], pv[1], pv[2]);
                } else {
                    front.tri(pv[0], pv[1], pv[2]);
                }
            }
        }
    }
    let (loops, closed) = chain(&cuts, 1e-6);
    if !closed || loops.is_empty() {
        return Err("The solid is not closed along the slicing plane.".into());
    }
    for t in cap(&loops, pl)? {
        back.tri(t[0], t[1], t[2]);
        front.tri(t[0], t[2], t[1]);
    }
    Ok(Sliced { front: Some(front.finish()), back: Some(back.finish()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::mesh::{box_mesh, cylinder_mesh, extrude_loops, is_watertight, mesh_volume, sphere_mesh};

    fn z_plane(z: f64) -> Plane {
        Plane::new(V3::new(0.0, 0.0, z), V3::new(0.0, 0.0, 1.0)).unwrap()
    }

    #[test]
    fn slicing_a_box_keeps_both_closed_halves() {
        let b = box_mesh(0.0, 0.0, 0.0, 10.0, 10.0, 10.0, "Box").mesh();
        let s = slice(&b, &z_plane(4.0)).unwrap();
        let (f, k) = (s.front.unwrap(), s.back.unwrap());
        assert!((mesh_volume(&f) - 600.0).abs() < 1e-6, "{}", mesh_volume(&f));
        assert!((mesh_volume(&k) - 400.0).abs() < 1e-6, "{}", mesh_volume(&k));
        assert!(is_watertight(&f) && is_watertight(&k));
    }

    #[test]
    fn an_oblique_plane_through_a_cube_halves_it() {
        let b = box_mesh(0.0, 0.0, 0.0, 10.0, 10.0, 10.0, "Box").mesh();
        let pl = Plane::new(V3::new(5.0, 5.0, 5.0), V3::new(1.0, 0.7, 0.3)).unwrap();
        let s = slice(&b, &pl).unwrap();
        let (f, k) = (mesh_volume(s.front.as_ref().unwrap()), mesh_volume(s.back.as_ref().unwrap()));
        assert!((f + k - 1000.0).abs() < 1e-6 && (f - 500.0).abs() < 1e-6, "{f} {k}");
        assert!(is_watertight(s.front.as_ref().unwrap()) && is_watertight(s.back.as_ref().unwrap()));
    }

    #[test]
    fn planes_through_vertices_and_missing_the_solid() {
        let b = box_mesh(0.0, 0.0, 0.0, 10.0, 10.0, 10.0, "Box").mesh();
        let diag = Plane::through(V3::new(0.0, 0.0, 0.0), V3::new(10.0, 10.0, 0.0), V3::new(0.0, 0.0, 10.0)).unwrap();
        let s = slice(&b, &diag).unwrap();
        assert!((mesh_volume(s.front.as_ref().unwrap()) - 500.0).abs() < 1e-6);
        assert!(is_watertight(s.front.as_ref().unwrap()));
        let miss = slice(&b, &z_plane(20.0)).unwrap();
        assert!(miss.front.is_none() && miss.back.is_some());
    }

    #[test]
    fn caps_keep_holes_and_curved_solids_are_halved() {
        let outer: Vec<Pt> = (0..48).map(|i| pt(10.0 * (TAU * i as f64 / 48.0).cos(), 10.0 * (TAU * i as f64 / 48.0).sin())).collect();
        let inner: Vec<Pt> = (0..48).map(|i| pt(6.0 * (TAU * i as f64 / 48.0).cos(), 6.0 * (TAU * i as f64 / 48.0).sin())).collect();
        let tube = extrude_loops(&[outer, inner], 10.0, 0.0, "Tube", 0.0).unwrap().mesh();
        let total = mesh_volume(&tube);
        assert!(is_watertight(&tube), "tube");
        let s = slice(&tube, &z_plane(3.0)).unwrap();
        let (f, k) = (s.front.unwrap(), s.back.unwrap());
        assert!((mesh_volume(&f) + mesh_volume(&k) - total).abs() < 1e-6);
        assert!((mesh_volume(&k) - total * 0.3).abs() < 1e-6);
        assert!(is_watertight(&f) && is_watertight(&k));
        let sph = sphere_mesh(0.0, 0.0, 0.0, 5.0, 32, "S").mesh();
        let h = slice(&sph, &z_plane(0.0)).unwrap();
        let (a, b) = (mesh_volume(h.front.as_ref().unwrap()), mesh_volume(h.back.as_ref().unwrap()));
        assert!((a - b).abs() / a < 0.02 && is_watertight(h.front.as_ref().unwrap()));
        let cyl = cylinder_mesh(0.0, 0.0, 0.0, 4.0, 10.0, 32, "C").mesh();
        let vert = Plane::new(V3::default(), V3::new(1.0, 0.0, 0.0)).unwrap();
        let v = slice(&cyl, &vert).unwrap();
        assert!((mesh_volume(v.front.as_ref().unwrap()) - mesh_volume(&cyl) / 2.0).abs() < 1e-6);
    }

    #[test]
    fn section_loops_follow_the_cut() {
        let b = box_mesh(0.0, 0.0, 0.0, 10.0, 6.0, 10.0, "Box").mesh();
        let l = section_loops(&b, &z_plane(5.0));
        assert_eq!(l.len(), 1);
        let pts: Vec<Pt> = l[0].iter().map(|p| pt(p.x, p.y)).collect();
        assert!((polygon_area(&pts).abs() - 60.0).abs() < 1e-6);
    }
}
