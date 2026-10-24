//! Triangle-mesh solids: primitives, extrusion of planar profiles (with holes), measurements and export
//! (port of `src/solid/mesh.js`). A solid has outward (CCW) faces.
use super::{Mesh, Solid, V3};
use crate::drawing::Drawing;
use crate::entities::{ellipse_is_full, ellipse_point, segs_of, spline_points, Entity, Kind};
use crate::geom::*;
use std::collections::HashMap;

pub type Loop = Vec<Pt>;

// ---------------------------------------------------------------- profiles -> planar loops

/// Tessellated outline of one entity, or `None` when it is not a usable closed curve.
fn loop_of_entity(e: &Entity, tol: f64) -> Option<Loop> {
    match &e.kind {
        Kind::Circle { c, r } => {
            let n = (TAU / (2.0 * (1.0 - tol / r.max(tol * 2.0)).max(0.0).acos()))
                .ceil()
                .clamp(24.0, 256.0) as usize;
            Some(
                (0..n)
                    .map(|i| {
                        pt(
                            c.x + r * (TAU * i as f64 / n as f64).cos(),
                            c.y + r * (TAU * i as f64 / n as f64).sin(),
                        )
                    })
                    .collect(),
            )
        }
        Kind::Ellipse(el) => {
            if !ellipse_is_full(el) {
                return None;
            }
            Some(
                (0..96)
                    .map(|i| ellipse_point(el, TAU * i as f64 / 96.0))
                    .collect(),
            )
        }
        Kind::Pline { v, closed: true } => Some(dedupe(&tess_segs(&pline_segs(v, true), tol))),
        Kind::Spline { fit, closed: true } => {
            let mut p = spline_points(fit, true, 16);
            p.pop();
            Some(dedupe(&p))
        }
        _ => None,
    }
}

fn dedupe(pts: &[Pt]) -> Loop {
    let mut out: Loop = vec![];
    for p in pts {
        if out.last().map_or(true, |l| dist(*p, *l) > 1e-9) {
            out.push(*p);
        }
    }
    if out.len() > 1 && dist(out[0], out[out.len() - 1]) < 1e-9 {
        out.pop();
    }
    out
}

/// Join open entities (lines, arcs, polylines) end to end into closed loops. `Err` carries the loops found so far.
fn chain_open(entities: &[&Entity], doc: &Drawing, tol: f64) -> Result<Vec<Loop>, Vec<Loop>> {
    let paths: Vec<Vec<Seg>> = entities
        .iter()
        .map(|e| segs_of(e, doc).to_vec())
        .filter(|p| !p.is_empty())
        .collect();
    let mut loops = vec![];
    let eps = 1e-6;
    let mut used = vec![false; paths.len()];
    for i in 0..paths.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut chain = paths[i].clone();
        let mut grew = true;
        while grew && dist(chain[0].start(), chain[chain.len() - 1].end()) > eps {
            grew = false;
            for j in 0..paths.len() {
                if used[j] {
                    continue;
                }
                let p = &paths[j];
                let (tail, head) = (chain[chain.len() - 1].end(), chain[0].start());
                let rev = || p.iter().rev().map(|s| s.reverse()).collect::<Vec<_>>();
                if dist(p[0].start(), tail) <= eps {
                    chain.extend(p.iter().copied());
                } else if dist(p[p.len() - 1].end(), tail) <= eps {
                    chain.extend(rev());
                } else if dist(p[p.len() - 1].end(), head) <= eps {
                    let mut v = p.clone();
                    v.extend(chain);
                    chain = v;
                } else if dist(p[0].start(), head) <= eps {
                    let mut v = rev();
                    v.extend(chain);
                    chain = v;
                } else {
                    continue;
                }
                used[j] = true;
                grew = true;
                break;
            }
        }
        if dist(chain[0].start(), chain[chain.len() - 1].end()) <= eps {
            loops.push(dedupe(&tess_segs(&chain, tol)));
        } else {
            return Err(loops);
        }
    }
    Ok(loops)
}

pub struct ProfileResult {
    pub loops: Vec<Loop>,
    pub error: Option<String>,
}

/// Closed planar loops from selected entities. Closed polylines, circles, ellipses and closed splines are used
/// directly; lines, arcs and open polylines are joined into closed loops.
pub fn profile_loops(entities: &[&Entity], doc: &Drawing, tol: f64) -> ProfileResult {
    let mut loops: Vec<Loop> = vec![];
    let mut open: Vec<&Entity> = vec![];
    for e in entities {
        match loop_of_entity(e, tol) {
            Some(l) if l.len() >= 3 => loops.push(l),
            _ if matches!(
                e.kind,
                Kind::Line { .. } | Kind::Arc { .. } | Kind::Pline { .. }
            ) =>
            {
                open.push(e)
            }
            _ => {}
        }
    }
    if !open.is_empty() {
        match chain_open(&open, doc, tol) {
            Err(found) => {
                loops.extend(found);
                return ProfileResult {
                    loops,
                    error: Some("The selected lines and arcs do not form a closed outline.".into()),
                };
            }
            Ok(l) => loops.extend(l.into_iter().filter(|l| l.len() >= 3)),
        }
    }
    if loops.is_empty() {
        return ProfileResult {
            loops,
            error: Some(
                "Select closed polylines, circles, ellipses or a closed chain of lines and arcs."
                    .into(),
            ),
        };
    }
    ProfileResult { loops, error: None }
}

pub struct Group {
    pub outer: Loop,
    pub holes: Vec<Loop>,
}

/// Orient loops by even-odd nesting: outer rings CCW, holes CW, each hole attached to its outer ring.
pub fn group_loops(loops: &[Loop]) -> Vec<Group> {
    struct It {
        pts: Loop,
        area: f64,
        abs: f64,
        parents: Vec<usize>,
        depth: usize,
    }
    let mut items: Vec<It> = loops
        .iter()
        .map(|l| {
            let area = polygon_area(l);
            It {
                pts: l.clone(),
                area,
                abs: area.abs(),
                parents: vec![],
                depth: 0,
            }
        })
        .collect();
    items.sort_by(|a, b| b.abs.total_cmp(&a.abs));
    for i in 0..items.len() {
        let mut parents = vec![];
        for o in 0..items.len() {
            if o != i
                && items[o].abs > items[i].abs
                && point_in_polygon(items[i].pts[0], &items[o].pts)
            {
                parents.push(o);
            }
        }
        items[i].depth = parents.len();
        items[i].parents = parents;
    }
    let mut groups: Vec<Group> = vec![];
    let mut group_of: HashMap<usize, usize> = HashMap::new();
    for (i, it) in items.iter().enumerate() {
        if it.depth % 2 == 0 {
            let outer = if it.area >= 0.0 {
                it.pts.clone()
            } else {
                it.pts.iter().rev().copied().collect()
            };
            group_of.insert(i, groups.len());
            groups.push(Group {
                outer,
                holes: vec![],
            });
        }
    }
    for it in items.iter() {
        if it.depth % 2 == 1 {
            let mut cands: Vec<usize> = it
                .parents
                .iter()
                .copied()
                .filter(|&p| items[p].depth + 1 == it.depth)
                .collect();
            cands.sort_by(|a, b| items[*a].abs.total_cmp(&items[*b].abs));
            if let Some(g) = cands.first().and_then(|o| group_of.get(o)) {
                groups[*g].holes.push(if it.area <= 0.0 {
                    it.pts.clone()
                } else {
                    it.pts.iter().rev().copied().collect()
                });
            }
        }
    }
    groups
}

// ---------------------------------------------------------------- extrusion

fn offset_ring(ring: &[Pt], d: f64) -> Loop {
    // inward offset by moving each vertex along its bisector (for tapered extrusions of convex-ish outlines)
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (p, c, q) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
            let (e1, e2) = (unit(c - p), unit(q - c));
            let (n1, n2) = (pt(e1.y, -e1.x), pt(e2.y, -e2.x));
            let bis = unit(n1 + n2);
            let cos_half = dot(bis, n1).max(0.2);
            pt(c.x + bis.x * d / cos_half, c.y + bis.y * d / cos_half)
        })
        .collect()
}

/// Extrude planar loops (even-odd holes) from z = `start_z` by `height` (negative extrudes downwards).
pub fn extrude_loops(
    loops: &[Loop],
    height: f64,
    start_z: f64,
    name: &str,
    taper: f64,
) -> Result<Solid, String> {
    if !height.is_finite() || height == 0.0 {
        return Err("The extrusion height must not be zero.".into());
    }
    let groups = group_loops(loops);
    if groups.is_empty() {
        return Err("There is no closed outline to extrude.".into());
    }
    let (z0, z1) = if height > 0.0 {
        (start_z, start_z + height)
    } else {
        (start_z + height, start_z)
    };
    let h = height.abs();
    let k = rad(taper).tan();
    let mut vertices: Vec<V3> = vec![];
    let mut faces: Vec<[u32; 3]> = vec![];
    for grp in &groups {
        let mut rings: Vec<&Loop> = vec![&grp.outer];
        rings.extend(grp.holes.iter());
        let mut flat: Vec<f64> = vec![];
        let mut hole_starts: Vec<usize> = vec![];
        let mut bottoms: Vec<Vec<u32>> = vec![];
        let mut tops: Vec<Vec<u32>> = vec![];
        for (ri, ring) in rings.iter().enumerate() {
            if ri > 0 {
                hole_starts.push(flat.len() / 2);
            }
            let top = if taper != 0.0 {
                offset_ring(ring, (if ri == 0 { -1.0 } else { 1.0 }) * k * h)
            } else {
                (*ring).clone()
            }; // positive taper narrows the solid
            let (mut b, mut t) = (vec![], vec![]);
            for p in ring.iter() {
                b.push(vertices.len() as u32);
                vertices.push(V3::new(p.x, p.y, z0));
            }
            for p in &top {
                t.push(vertices.len() as u32);
                vertices.push(V3::new(p.x, p.y, z1));
            }
            bottoms.push(b);
            tops.push(t);
            for p in ring.iter() {
                flat.push(p.x);
                flat.push(p.y);
            }
        }
        // caps
        let tri = earcutr::earcut(&flat, &hole_starts, 2)
            .map_err(|e| format!("Triangulation failed: {e:?}"))?;
        let idx_bottom: Vec<u32> = bottoms.iter().flatten().copied().collect();
        let idx_top: Vec<u32> = tops.iter().flatten().copied().collect();
        for t in tri.chunks_exact(3) {
            let (a, mut b, mut c) = (t[0], t[1], t[2]);
            let (pa, pb, pc) = (
                pt(flat[a * 2], flat[a * 2 + 1]),
                pt(flat[b * 2], flat[b * 2 + 1]),
                pt(flat[c * 2], flat[c * 2 + 1]),
            );
            if (pb.x - pa.x) * (pc.y - pa.y) - (pb.y - pa.y) * (pc.x - pa.x) < 0.0 {
                std::mem::swap(&mut b, &mut c); // make CCW
            }
            faces.push([idx_top[a], idx_top[b], idx_top[c]]);
            faces.push([idx_bottom[a], idx_bottom[c], idx_bottom[b]]);
        }
        // side walls: outer ring CCW and hole rings CW, so the right-hand side of each edge is outside the material
        for (ri, ring) in rings.iter().enumerate() {
            let n = ring.len();
            for i in 0..n {
                let j = (i + 1) % n;
                let (a0, b0, a1, b1) = (bottoms[ri][i], bottoms[ri][j], tops[ri][i], tops[ri][j]);
                faces.push([a0, b0, b1]);
                faces.push([a0, b1, a1]);
            }
        }
    }
    Ok(Solid::from_mesh(name, Mesh { vertices, faces }))
}

// ---------------------------------------------------------------- primitives

fn orient(mut s: Solid) -> Solid {
    if mesh_volume(&s.mesh()) < 0.0 {
        for f in &mut s.faces {
            f.swap(1, 2);
        }
    }
    s
}

pub fn box_mesh(x0: f64, y0: f64, z0: f64, w: f64, d: f64, h: f64, name: &str) -> Solid {
    let (x1, y1, z1) = (x0 + w, y0 + d, z0 + h);
    let v = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y1, z0],
        [x0, y1, z0],
        [x0, y0, z1],
        [x1, y0, z1],
        [x1, y1, z1],
        [x0, y1, z1],
    ];
    let f: [[u32; 3]; 12] = [
        [0, 3, 2],
        [0, 2, 1],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    orient(Solid::from_mesh(
        name,
        Mesh {
            vertices: v.iter().map(|p| V3::new(p[0], p[1], p[2])).collect(),
            faces: f.to_vec(),
        },
    ))
}

/// Wedge: the sloped face rises from the front (y = y0) edge at z = z0; the high end is at x = x0 + w.
pub fn wedge_mesh(x0: f64, y0: f64, z0: f64, w: f64, d: f64, h: f64, name: &str) -> Solid {
    let (x1, y1, z1) = (x0 + w, y0 + d, z0 + h);
    let v = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y1, z0],
        [x0, y1, z0],
        [x1, y0, z1],
        [x1, y1, z1],
    ];
    let f: [[u32; 3]; 8] = [
        [0, 3, 2],
        [0, 2, 1],
        [1, 2, 5],
        [1, 5, 4],
        [0, 1, 4],
        [3, 5, 2],
        [0, 4, 5],
        [0, 5, 3],
    ];
    orient(Solid::from_mesh(
        name,
        Mesh {
            vertices: v.iter().map(|p| V3::new(p[0], p[1], p[2])).collect(),
            faces: f.to_vec(),
        },
    ))
}

pub fn cylinder_mesh(cx: f64, cy: f64, z0: f64, r: f64, h: f64, seg: usize, name: &str) -> Solid {
    cone_mesh(cx, cy, z0, r, r, h, seg, name)
}

/// Cone or frustum: radius `r1` at z0 and `r2` at z0 + h (`r2` = 0 gives an apex).
pub fn cone_mesh(
    cx: f64,
    cy: f64,
    z0: f64,
    r1: f64,
    r2: f64,
    h: f64,
    seg: usize,
    name: &str,
) -> Solid {
    let mut vertices: Vec<V3> = vec![];
    let mut faces: Vec<[u32; 3]> = vec![];
    let ring = |r: f64, z: f64, vertices: &mut Vec<V3>| -> Vec<u32> {
        let s = vertices.len() as u32;
        if r < 1e-12 {
            vertices.push(V3::new(cx, cy, z));
            return vec![s];
        }
        for i in 0..seg {
            vertices.push(V3::new(
                cx + r * (TAU * i as f64 / seg as f64).cos(),
                cy + r * (TAU * i as f64 / seg as f64).sin(),
                z,
            ));
        }
        (0..seg as u32).map(|i| s + i).collect()
    };
    let bot = ring(r1, z0, &mut vertices);
    let top = ring(r2, z0 + h, &mut vertices);
    let cap =
        |idx: &[u32], up: bool, centre: f64, vertices: &mut Vec<V3>, faces: &mut Vec<[u32; 3]>| {
            if idx.len() == 1 {
                return;
            }
            let c = vertices.len() as u32;
            vertices.push(V3::new(cx, cy, centre));
            for i in 0..seg {
                let (a, b) = (idx[i], idx[(i + 1) % seg]);
                faces.push(if up { [c, a, b] } else { [c, b, a] });
            }
        };
    for i in 0..seg {
        let j = (i + 1) % seg;
        if bot.len() == 1 {
            faces.push([bot[0], top[i], top[j]]);
        } else if top.len() == 1 {
            faces.push([bot[i], bot[j], top[0]]);
        } else {
            faces.push([bot[i], bot[j], top[j]]);
            faces.push([bot[i], top[j], top[i]]);
        }
    }
    cap(&top, true, z0 + h, &mut vertices, &mut faces);
    cap(&bot, false, z0, &mut vertices, &mut faces);
    Solid::from_mesh(name, Mesh { vertices, faces })
}

pub fn sphere_mesh(cx: f64, cy: f64, cz: f64, r: f64, seg: usize, name: &str) -> Solid {
    let rings = (seg >> 1).max(6);
    let mut vertices = vec![V3::new(cx, cy, cz - r)];
    let mut faces: Vec<[u32; 3]> = vec![];
    for k in 1..rings {
        let phi = -PI / 2.0 + PI * k as f64 / rings as f64;
        for i in 0..seg {
            let th = TAU * i as f64 / seg as f64;
            vertices.push(V3::new(
                cx + r * phi.cos() * th.cos(),
                cy + r * phi.cos() * th.sin(),
                cz + r * phi.sin(),
            ));
        }
    }
    vertices.push(V3::new(cx, cy, cz + r));
    let at = |k: usize, i: usize| (1 + (k - 1) * seg + (i % seg)) as u32;
    let (south, north) = (0u32, (vertices.len() - 1) as u32);
    for i in 0..seg {
        faces.push([south, at(1, i + 1), at(1, i)]);
    }
    for k in 1..rings - 1 {
        for i in 0..seg {
            faces.push([at(k, i), at(k, i + 1), at(k + 1, i + 1)]);
            faces.push([at(k, i), at(k + 1, i + 1), at(k + 1, i)]);
        }
    }
    for i in 0..seg {
        faces.push([north, at(rings - 1, i), at(rings - 1, i + 1)]);
    }
    Solid::from_mesh(name, Mesh { vertices, faces })
}

// ---------------------------------------------------------------- measurements

fn tri_vol(a: V3, b: V3, c: V3) -> f64 {
    (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x))
        / 6.0
}

pub fn mesh_volume(m: &Mesh) -> f64 {
    m.faces
        .iter()
        .map(|f| {
            tri_vol(
                m.vertices[f[0] as usize],
                m.vertices[f[1] as usize],
                m.vertices[f[2] as usize],
            )
        })
        .sum()
}

pub fn mesh_area(m: &Mesh) -> f64 {
    m.faces
        .iter()
        .map(|f| {
            let (a, b, c) = (
                m.vertices[f[0] as usize],
                m.vertices[f[1] as usize],
                m.vertices[f[2] as usize],
            );
            b.sub(a).cross(c.sub(a)).len() / 2.0
        })
        .sum()
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds3 {
    pub min: V3,
    pub max: V3,
}

pub fn mesh_bounds(vertices: &[V3]) -> Bounds3 {
    let mut b = Bounds3 {
        min: V3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
        max: V3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
    };
    for p in vertices {
        b.min = V3::new(b.min.x.min(p.x), b.min.y.min(p.y), b.min.z.min(p.z));
        b.max = V3::new(b.max.x.max(p.x), b.max.y.max(p.y), b.max.z.max(p.z));
    }
    b
}

pub fn mesh_centroid(m: &Mesh) -> V3 {
    let (mut vol, mut c) = (0.0, V3::new(0.0, 0.0, 0.0));
    for f in &m.faces {
        let (a, b, d) = (
            m.vertices[f[0] as usize],
            m.vertices[f[1] as usize],
            m.vertices[f[2] as usize],
        );
        let v = tri_vol(a, b, d);
        vol += v;
        c = c.add(a.add(b).add(d).scale(v / 4.0));
    }
    if vol.abs() < 1e-12 {
        V3::new(0.0, 0.0, 0.0)
    } else {
        c.scale(1.0 / vol)
    }
}

/// Every undirected edge is used exactly twice, once in each direction (closed, consistently oriented surface).
pub fn is_watertight(m: &Mesh) -> bool {
    let mut dir: HashMap<(u32, u32), u32> = HashMap::new();
    for f in &m.faces {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            *dir.entry((a, b)).or_insert(0) += 1;
        }
    }
    dir.iter()
        .all(|(&(a, b), &n)| n == 1 && dir.get(&(b, a)) == Some(&1))
}

// ---------------------------------------------------------------- transforms

/// Affine 3D matrix as 12 numbers `[m00 m01 m02 tx, m10 m11 m12 ty, m20 m21 m22 tz]`.
pub type M3 = [f64; 12];

pub mod m3 {
    use super::*;
    pub fn identity() -> M3 {
        [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
    }
    pub fn translate(x: f64, y: f64, z: f64) -> M3 {
        [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z]
    }
    pub fn rotate_z(a: f64, c: Pt) -> M3 {
        let (co, si) = (a.cos(), a.sin());
        [
            co,
            -si,
            0.0,
            c.x - co * c.x + si * c.y,
            si,
            co,
            0.0,
            c.y - si * c.x - co * c.y,
            0.0,
            0.0,
            1.0,
            0.0,
        ]
    }
    pub fn scale(s: f64, c: V3) -> M3 {
        [
            s,
            0.0,
            0.0,
            c.x * (1.0 - s),
            0.0,
            s,
            0.0,
            c.y * (1.0 - s),
            0.0,
            0.0,
            s,
            c.z * (1.0 - s),
        ]
    }
    pub fn mirror_xy(a: Pt, b: Pt) -> M3 {
        let d = unit(b - a);
        let (c2, s2) = (d.x * d.x - d.y * d.y, 2.0 * d.x * d.y);
        [
            c2,
            s2,
            0.0,
            a.x - c2 * a.x - s2 * a.y,
            s2,
            -c2,
            0.0,
            a.y - s2 * a.x + c2 * a.y,
            0.0,
            0.0,
            1.0,
            0.0,
        ]
    }
    pub fn apply(m: &M3, p: V3) -> V3 {
        V3::new(
            m[0] * p.x + m[1] * p.y + m[2] * p.z + m[3],
            m[4] * p.x + m[5] * p.y + m[6] * p.z + m[7],
            m[8] * p.x + m[9] * p.y + m[10] * p.z + m[11],
        )
    }
    pub fn det(m: &M3) -> f64 {
        m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8])
            + m[2] * (m[4] * m[9] - m[5] * m[8])
    }
}

/// Transformed copy of a solid (faces are re-wound when the transform mirrors).
pub fn transform_mesh(s: &Solid, m: &M3) -> Solid {
    let mut out = s.clone();
    out.vertices = s.vertices.iter().map(|p| m3::apply(m, *p)).collect();
    if m3::det(m) < 0.0 {
        for f in &mut out.faces {
            f.swap(1, 2);
        }
    }
    out
}

/// 3D matrix for a plan-view 2D affine map `[a b c d e f]`; z is scaled by the map's uniform scale factor.
pub fn solid_matrix(m: &Mat) -> M3 {
    let [a, b, c, d, e, f] = *m;
    let k = (a * d - b * c).abs().sqrt();
    [a, c, 0.0, e, b, d, 0.0, f, 0.0, 0.0, k, 0.0]
}

pub fn transform_solid(s: &Solid, m2d: &Mat) -> Solid {
    transform_mesh(s, &solid_matrix(m2d))
}

// ---------------------------------------------------------------- export

pub fn to_obj(solids: &[&Solid]) -> String {
    let mut out = vec!["# CadForge solids (drawing units)".to_string()];
    let mut off = 1;
    for (n, s) in solids.iter().enumerate() {
        let name = if s.name.is_empty() {
            format!("solid{}", n + 1)
        } else {
            s.name.clone()
        };
        out.push(format!(
            "o {}",
            name.chars()
                .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                })
                .collect::<String>()
        ));
        for p in &s.vertices {
            out.push(format!("v {} {} {}", p.x, p.y, p.z));
        }
        for f in &s.faces {
            out.push(format!("f {} {} {}", f[0] + off, f[1] + off, f[2] + off));
        }
        off += s.vertices.len() as u32;
    }
    out.join("\n") + "\n"
}

pub fn to_stl(solids: &[&Solid]) -> String {
    let mut out = vec!["solid CadForge".to_string()];
    for s in solids {
        for f in &s.faces {
            let (a, b, c) = (
                s.vertices[f[0] as usize],
                s.vertices[f[1] as usize],
                s.vertices[f[2] as usize],
            );
            let n = b.sub(a).cross(c.sub(a));
            let l = if n.len() == 0.0 { 1.0 } else { n.len() };
            let n = n.scale(1.0 / l);
            out.push(format!(" facet normal {} {} {}", n.x, n.y, n.z));
            out.push("  outer loop".into());
            for p in [a, b, c] {
                out.push(format!("   vertex {} {} {}", p.x, p.y, p.z));
            }
            out.push("  endloop".into());
            out.push(" endfacet".into());
        }
    }
    out.push("endsolid CadForge".into());
    out.join("\n") + "\n"
}
