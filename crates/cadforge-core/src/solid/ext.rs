//! Mesh-kernel extensions behind the extra 3D commands: swept solids (revolve, sweep, loft, torus, pyramid), plane
//! slicing, rounding of edges, shelling, view projection (flat shots) and mesh repair.
pub mod edges;
pub mod flat;
pub mod repair;
pub mod slice;
pub mod sweep;
pub mod xform;

use super::mesh::{mesh_volume, Group, Loop};
use super::{Mesh, V3};
use crate::geom::{cross, dot, Pt};
use std::collections::HashMap;

/// Builds a mesh while merging vertices that coincide (to 1e-7), so independently generated pieces share indices.
#[derive(Default)]
pub struct Welder {
    map: HashMap<(i64, i64, i64), u32>,
    pub vertices: Vec<V3>,
    pub faces: Vec<[u32; 3]>,
}

impl Welder {
    pub fn add(&mut self, p: V3) -> u32 {
        let key = (
            (p.x * 1e7).round() as i64,
            (p.y * 1e7).round() as i64,
            (p.z * 1e7).round() as i64,
        );
        let verts = &mut self.vertices;
        *self.map.entry(key).or_insert_with(|| {
            verts.push(p);
            (verts.len() - 1) as u32
        })
    }

    /// Add a triangle by index; triangles that collapse after welding are dropped.
    pub fn tri_idx(&mut self, a: u32, b: u32, c: u32) {
        if a != b && b != c && a != c {
            self.faces.push([a, b, c]);
        }
    }

    pub fn tri(&mut self, a: V3, b: V3, c: V3) {
        let (a, b, c) = (self.add(a), self.add(b), self.add(c));
        self.tri_idx(a, b, c);
    }

    /// Fan-triangulate a convex or star-shaped polygon.
    pub fn poly(&mut self, pts: &[V3]) {
        for i in 1..pts.len().saturating_sub(1) {
            self.tri(pts[0], pts[i], pts[i + 1]);
        }
    }

    pub fn finish(self) -> Mesh {
        Mesh {
            vertices: self.vertices,
            faces: self.faces,
        }
    }
}

/// Reverse every face when the mesh encloses negative volume (inside-out), so the result has outward faces.
pub fn orient_outward(m: &mut Mesh) {
    if mesh_volume(m) < 0.0 {
        for f in &mut m.faces {
            f.swap(1, 2);
        }
    }
}

/// Unit normal of a triangle (zero vector for degenerate triangles).
pub fn tri_normal(m: &Mesh, f: &[u32; 3]) -> V3 {
    let (a, b, c) = (
        m.vertices[f[0] as usize],
        m.vertices[f[1] as usize],
        m.vertices[f[2] as usize],
    );
    let n = b.sub(a).cross(c.sub(a));
    let l = n.len();
    if l < 1e-300 {
        V3::default()
    } else {
        n.scale(1.0 / l)
    }
}

/// Number of segments around a full circle of radius `r`.
pub fn segments_for(r: f64) -> usize {
    (24.0 + r.max(0.0).sqrt() * 8.0).round().clamp(32.0, 96.0) as usize
}

/// Every ring of the groups in order: each outer ring followed by its holes.
pub fn rings_of(groups: &[Group]) -> Vec<&Loop> {
    groups
        .iter()
        .flat_map(|g| std::iter::once(&g.outer).chain(g.holes.iter()))
        .collect()
}

/// Split triangle edges at any other point lying on them, so regions that share an outline also share its vertices.
fn split_at_points(tris: Vec<[usize; 3]>, p: &[Pt]) -> Vec<[usize; 3]> {
    let mut out = vec![];
    let mut work = tris;
    while let Some(t) = work.pop() {
        let hit = (0..3).find_map(|e| {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            let d = p[b] - p[a];
            let l2 = dot(d, d);
            (0..p.len())
                .filter(|&k| k != a && k != b)
                .find(|&k| {
                    let (w, u) = (p[k] - p[a], dot(p[k] - p[a], d) / l2);
                    u > 1e-9 && u < 1.0 - 1e-9 && cross(d, w).abs() < 1e-7 * l2.sqrt()
                })
                .map(|k| (e, k))
        });
        match hit {
            Some((e, k)) => {
                let (a, b, c) = (t[e], t[(e + 1) % 3], t[(e + 2) % 3]);
                work.push([a, k, c]);
                work.push([k, b, c]);
            }
            None => out.push(t),
        }
    }
    out
}

/// Counter-clockwise triangles that fill the regions of `groups` (outer rings and holes), as index triples into the
/// concatenation of every ring's points. Points that lie on a triangle's edge become its vertices too.
pub fn region_tris(groups: &[Group]) -> Result<Vec<[usize; 3]>, String> {
    let (mut out, mut base) = (vec![], 0);
    for g in groups {
        let (mut flat, mut holes, mut pts) = (vec![], vec![], vec![]);
        for (ri, ring) in std::iter::once(&g.outer).chain(g.holes.iter()).enumerate() {
            if ri > 0 {
                holes.push(flat.len() / 2);
            }
            for p in ring {
                flat.extend([p.x, p.y]);
                pts.push(*p);
            }
        }
        let tri = earcutr::earcut(&flat, &holes, 2)
            .map_err(|e| format!("Triangulation failed: {e:?}"))?;
        let ccw: Vec<[usize; 3]> = tri
            .chunks_exact(3)
            .map(|t| {
                if cross(pts[t[1]] - pts[t[0]], pts[t[2]] - pts[t[0]]) < 0.0 {
                    [t[0], t[2], t[1]]
                } else {
                    [t[0], t[1], t[2]]
                }
            })
            .collect();
        out.extend(
            split_at_points(ccw, &pts)
                .into_iter()
                .map(|t| [base + t[0], base + t[1], base + t[2]]),
        );
        base += pts.len();
    }
    Ok(out)
}
