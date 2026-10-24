//! Mesh connectivity and repair: welded topology, clean-up, separation into bodies, validity checks, shelling and
//! picking of faces under a camera.
use super::{tri_normal, Welder};
use crate::geom::*;
use crate::solid::mesh::mesh_volume;
use crate::solid::{Mesh, V3};
use crate::view::View;
use std::collections::{HashMap, HashSet};

/// Mesh connectivity over welded vertices (positions closer than 1e-7 are one vertex).
pub struct Topo {
    /// Welded vertex positions.
    pub pos: Vec<V3>,
    /// Per original face: welded vertex ids.
    pub tris: Vec<[u32; 3]>,
    /// Per original face: unit normal (zero for degenerate faces).
    pub normal: Vec<V3>,
    /// Undirected edge `(low, high)` to the live faces using it.
    pub edges: HashMap<(u32, u32), Vec<usize>>,
}

impl Topo {
    pub fn new(m: &Mesh) -> Topo {
        let mut w = Welder::default();
        let ids: Vec<u32> = m.vertices.iter().map(|p| w.add(*p)).collect();
        let tris: Vec<[u32; 3]> = m
            .faces
            .iter()
            .map(|f| [ids[f[0] as usize], ids[f[1] as usize], ids[f[2] as usize]])
            .collect();
        let pos = w.vertices;
        let welded = Mesh {
            vertices: pos.clone(),
            faces: tris.clone(),
        };
        let normal: Vec<V3> = tris
            .iter()
            .map(|f| {
                if f[0] == f[1] || f[1] == f[2] || f[0] == f[2] {
                    V3::default()
                } else {
                    tri_normal(&welded, f)
                }
            })
            .collect();
        let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (i, f) in tris.iter().enumerate() {
            if normal[i].len() < 0.5 {
                continue;
            }
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                edges.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        Topo {
            pos,
            tris,
            normal,
            edges,
        }
    }

    pub fn live(&self, f: usize) -> bool {
        self.normal[f].len() > 0.5
    }

    /// Non-degenerate faces with their indices.
    pub fn live_faces(&self) -> impl Iterator<Item = (usize, &[u32; 3])> + '_ {
        self.tris.iter().enumerate().filter(|(i, _)| self.live(*i))
    }

    /// Faces that share an edge with face `f`.
    pub fn neighbours(&self, f: usize) -> Vec<usize> {
        let t = self.tris[f];
        let mut out = vec![];
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if let Some(fs) = self.edges.get(&(a.min(b), a.max(b))) {
                out.extend(fs.iter().copied().filter(|g| *g != f));
            }
        }
        out
    }

    /// The planar face containing triangle `f`: connected triangles with (nearly) the same normal.
    pub fn planar_region(&self, f: usize) -> Vec<usize> {
        let mut seen: HashSet<usize> = HashSet::from([f]);
        let mut stack = vec![f];
        while let Some(g) = stack.pop() {
            for h in self.neighbours(g) {
                if !seen.contains(&h) && self.normal[h].dot(self.normal[f]) > 1.0 - 1e-6 {
                    seen.insert(h);
                    stack.push(h);
                }
            }
        }
        let mut v: Vec<usize> = seen.into_iter().collect();
        v.sort_unstable();
        v
    }
}

/// What [`clean`] removed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CleanReport {
    pub merged_vertices: usize,
    pub degenerate_faces: usize,
    pub duplicate_faces: usize,
}

/// Weld coincident vertices and drop degenerate and duplicate faces and unused vertices.
pub fn clean(m: &Mesh) -> (Mesh, CleanReport) {
    let t = Topo::new(m);
    let mut rep = CleanReport::default();
    let mut seen: HashSet<[u32; 3]> = HashSet::new();
    let mut faces = vec![];
    for (i, f) in t.tris.iter().enumerate() {
        if !t.live(i) {
            rep.degenerate_faces += 1;
            continue;
        }
        let key = {
            let k = (0..3).min_by_key(|k| f[*k]).unwrap_or(0);
            [f[k], f[(k + 1) % 3], f[(k + 2) % 3]]
        };
        if !seen.insert(key) {
            rep.duplicate_faces += 1;
            continue;
        }
        faces.push(*f);
    }
    let mut out = Mesh {
        vertices: t.pos,
        faces,
    };
    compact(&mut out);
    rep.merged_vertices = m.vertices.len().saturating_sub(out.vertices.len());
    (out, rep)
}

/// Drop vertices no face uses.
pub fn compact(m: &mut Mesh) {
    let mut map = vec![u32::MAX; m.vertices.len()];
    let mut vertices = vec![];
    for f in &mut m.faces {
        for v in f.iter_mut() {
            if map[*v as usize] == u32::MAX {
                map[*v as usize] = vertices.len() as u32;
                vertices.push(m.vertices[*v as usize]);
            }
            *v = map[*v as usize];
        }
    }
    m.vertices = vertices;
}

/// Split a mesh into its connected bodies (faces joined through shared welded vertices).
pub fn components(m: &Mesh) -> Vec<Mesh> {
    let t = Topo::new(m);
    let mut parent: Vec<usize> = (0..t.pos.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for (_, f) in t.live_faces() {
        let (a, b, c) = (
            find(&mut parent, f[0] as usize),
            find(&mut parent, f[1] as usize),
            find(&mut parent, f[2] as usize),
        );
        parent[b] = a;
        parent[c] = a;
    }
    let mut groups: Vec<(usize, Vec<[u32; 3]>)> = vec![];
    for (_, f) in t.live_faces() {
        let root = find(&mut parent, f[0] as usize);
        match groups.iter_mut().find(|g| g.0 == root) {
            Some(g) => g.1.push(*f),
            None => groups.push((root, vec![*f])),
        }
    }
    groups
        .into_iter()
        .map(|(_, faces)| {
            let mut mesh = Mesh {
                vertices: t.pos.clone(),
                faces,
            };
            compact(&mut mesh);
            mesh
        })
        .collect()
}

/// Result of [`check`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Check {
    /// Edges used by one face only (holes in the surface).
    pub open_edges: usize,
    /// Edges shared by more than two faces, or used twice in the same direction.
    pub bad_edges: usize,
    pub volume: f64,
}

impl Check {
    pub fn valid(&self) -> bool {
        self.open_edges == 0 && self.bad_edges == 0 && self.volume > 0.0
    }
}

/// Is the surface closed, consistently oriented and enclosing positive volume?
pub fn check(m: &Mesh) -> Check {
    let t = Topo::new(m);
    let mut dir: HashMap<(u32, u32), u32> = HashMap::new();
    for (_, f) in t.live_faces() {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            *dir.entry((a, b)).or_insert(0) += 1;
        }
    }
    let (mut open, mut bad) = (0, 0);
    for (&(a, b), &n) in &dir {
        match dir.get(&(b, a)) {
            None => open += 1,
            Some(&r) if n != 1 || r != 1 => bad += 1,
            _ => {}
        }
    }
    Check {
        open_edges: open,
        bad_edges: bad,
        volume: mesh_volume(&Mesh {
            vertices: t.pos.clone(),
            faces: t.live_faces().map(|(_, f)| *f).collect(),
        }),
    }
}

// ---------------------------------------------------------------- shelling

/// Offset vector `v` for a vertex such that `n · v = 1` for each distinct face normal `n` around it (least squares).
fn offset_vector(normals: &[V3]) -> V3 {
    let mut ns: Vec<V3> = vec![];
    for n in normals {
        if !ns.iter().any(|m| m.dot(*n) > 0.999_999) {
            ns.push(*n);
        }
    }
    let mut a = [[0.0; 4]; 3];
    for n in &ns {
        let c = [n.x, n.y, n.z];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += c[i] * c[j];
            }
            a[i][3] += c[i];
        }
    }
    for (i, row) in a.iter_mut().enumerate() {
        row[i] += 1e-9;
    }
    for col in 0..3 {
        let piv = (col..3)
            .max_by(|x, y| a[*x][col].abs().total_cmp(&a[*y][col].abs()))
            .unwrap_or(col);
        a.swap(col, piv);
        let d = a[col][col];
        for j in col..4 {
            a[col][j] /= d;
        }
        for r in 0..3 {
            if r != col {
                let f = a[r][col];
                for j in col..4 {
                    a[r][j] -= f * a[col][j];
                }
            }
        }
    }
    let v = V3::new(a[0][3], a[1][3], a[2][3]);
    // very sharp corners would throw the offset far out; keep it within a few thicknesses
    if v.len() > 4.0 {
        v.scale(4.0 / v.len())
    } else {
        v
    }
}

/// Hollow a solid out. The surface is offset inwards by `thickness` (outwards when negative) with mitred corners and
/// the triangles listed in `open` (original face indices) are removed from both surfaces, leaving openings.
pub fn shell(m: &Mesh, open: &[usize], thickness: f64) -> Result<Mesh, String> {
    if thickness.abs() < 1e-9 {
        return Err("The shell thickness must not be zero.".into());
    }
    let t = Topo::new(m);
    let n = t.pos.len();
    let removed: HashSet<usize> = open.iter().copied().collect();
    // the planes of removed faces do not push the walls back: those walls run on up to the opening
    let mut around: Vec<Vec<V3>> = vec![vec![]; n];
    for (i, f) in t.live_faces().filter(|(i, _)| !removed.contains(i)) {
        for v in f {
            around[*v as usize].push(t.normal[i]);
        }
    }
    let moved: Vec<V3> = (0..n)
        .map(|i| {
            if around[i].is_empty() {
                t.pos[i]
            } else {
                t.pos[i].sub(offset_vector(&around[i]).scale(thickness))
            }
        })
        .collect();
    let (hi, lo) = if thickness > 0.0 {
        (t.pos.clone(), moved)
    } else {
        (moved, t.pos.clone())
    };
    let kept: Vec<usize> = (0..t.tris.len())
        .filter(|f| t.live(*f) && !removed.contains(f))
        .collect();
    let off = n as u32;
    let mut faces: Vec<[u32; 3]> = kept.iter().map(|&f| t.tris[f]).collect();
    faces.extend(kept.iter().map(|&f| {
        let [a, b, c] = t.tris[f];
        [off + a, off + c, off + b]
    }));
    for &f in open.iter().filter(|f| t.live(**f)) {
        let tri = t.tris[f];
        for (u, v) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let shared = &t.edges[&(u.min(v), u.max(v))];
            if shared.iter().any(|g| *g != f && !removed.contains(g)) {
                faces.push([u, v, off + v]);
                faces.push([u, off + v, off + u]);
            }
        }
    }
    let mut vertices = hi.clone();
    vertices.extend(lo.iter().copied());
    let mut out = Mesh { vertices, faces };
    let full = |p: &[V3]| {
        mesh_volume(&Mesh {
            vertices: p.to_vec(),
            faces: t.live_faces().map(|(_, f)| *f).collect(),
        })
    };
    let (outer_vol, inner_vol) = (full(&hi), full(&lo));
    if inner_vol <= 1e-12 || inner_vol >= outer_vol {
        return Err("The shell thickness is too large for this solid.".into());
    }
    compact(&mut out);
    Ok(out)
}

// ---------------------------------------------------------------- picking faces through a camera

/// Index of the face drawn under screen point `sp` (the one nearest the viewer).
pub fn pick_face(m: &Mesh, view: &View, sp: Pt) -> Option<usize> {
    let pr: Vec<_> = m.vertices.iter().map(|p| view.project(*p)).collect();
    let mut best: Option<(f64, usize)> = None;
    for (i, f) in m.faces.iter().enumerate() {
        let (a, b, c) = (&pr[f[0] as usize], &pr[f[1] as usize], &pr[f[2] as usize]);
        let d = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
        if d.abs() < 1e-12 {
            continue;
        }
        let u = ((b.y - c.y) * (sp.x - c.x) + (c.x - b.x) * (sp.y - c.y)) / d;
        let v = ((c.y - a.y) * (sp.x - c.x) + (a.x - c.x) * (sp.y - c.y)) / d;
        let w = 1.0 - u - v;
        if u >= -1e-9 && v >= -1e-9 && w >= -1e-9 {
            let depth = u * a.depth + v * b.depth + w * c.depth;
            if best.map_or(true, |b| depth < b.0) {
                best = Some((depth, i));
            }
        }
    }
    best.map(|b| b.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::mesh::{box_mesh, cylinder_mesh, is_watertight};

    fn cube() -> Mesh {
        box_mesh(0.0, 0.0, 0.0, 10.0, 10.0, 10.0, "Box").mesh()
    }

    #[test]
    fn planar_regions_join_the_triangles_of_a_face() {
        let m = cube();
        let t = Topo::new(&m);
        let top = (0..m.faces.len()).find(|f| t.normal[*f].z > 0.99).unwrap();
        assert_eq!(t.planar_region(top).len(), 2);
    }

    #[test]
    fn clean_removes_duplicates_and_degenerates() {
        let mut m = cube();
        let extra = m.faces[0];
        m.faces.push(extra);
        let a = m.vertices.len() as u32;
        m.vertices.push(V3::new(5.0, 5.0, 5.0));
        m.faces.push([a, a, 0]);
        let (c, rep) = clean(&m);
        assert_eq!((rep.duplicate_faces, rep.degenerate_faces), (1, 1));
        assert_eq!(c.faces.len(), 12);
        assert_eq!(c.vertices.len(), 8);
        assert!(is_watertight(&c));
    }

    #[test]
    fn separate_bodies_and_check() {
        let a = box_mesh(0.0, 0.0, 0.0, 1.0, 1.0, 1.0, "A").mesh();
        let b = box_mesh(5.0, 0.0, 0.0, 2.0, 2.0, 2.0, "B").mesh();
        let off = a.vertices.len() as u32;
        let both = Mesh {
            vertices: a.vertices.iter().chain(&b.vertices).copied().collect(),
            faces: a
                .faces
                .iter()
                .copied()
                .chain(b.faces.iter().map(|f| [f[0] + off, f[1] + off, f[2] + off]))
                .collect(),
        };
        let parts = components(&both);
        assert_eq!(parts.len(), 2);
        let mut vols: Vec<f64> = parts.iter().map(mesh_volume).collect();
        vols.sort_by(f64::total_cmp);
        assert!((vols[0] - 1.0).abs() < 1e-9 && (vols[1] - 8.0).abs() < 1e-9);
        assert!(check(&both).valid());
        let mut open = cube();
        open.faces.pop();
        let c = check(&open);
        assert!(!c.valid() && c.open_edges > 0);
    }

    #[test]
    fn shell_of_a_box_with_an_open_top_face() {
        let m = cube();
        let t = Topo::new(&m);
        let top = (0..m.faces.len()).find(|f| t.normal[*f].z > 0.99).unwrap();
        let s = shell(&m, &t.planar_region(top), 1.0).unwrap();
        // outer 1000 minus the cavity 8 x 8 x 9 (open to the top)
        assert!(
            (mesh_volume(&s) - (1000.0 - 8.0 * 8.0 * 9.0)).abs() < 1e-6,
            "{}",
            mesh_volume(&s)
        );
        assert!(is_watertight(&s));
        let sealed = shell(&m, &[], 2.0).unwrap();
        assert!((mesh_volume(&sealed) - (1000.0 - 6.0 * 6.0 * 6.0)).abs() < 1e-6);
        assert!(shell(&m, &[], 6.0).is_err());
        assert!(shell(&m, &[], 0.0).is_err());
    }

    #[test]
    fn shell_follows_curved_walls_and_grows_outwards() {
        let c = cylinder_mesh(0.0, 0.0, 0.0, 10.0, 20.0, 64, "C").mesh();
        let s = shell(&c, &[], 2.0).unwrap();
        let hollow = mesh_volume(&c) - PI * 8.0 * 8.0 * 16.0;
        assert!(
            (mesh_volume(&s) - hollow).abs() / hollow < 0.02,
            "{} vs {hollow}",
            mesh_volume(&s)
        );
        let out = shell(
            &box_mesh(0.0, 0.0, 0.0, 10.0, 10.0, 10.0, "B").mesh(),
            &[],
            -1.0,
        )
        .unwrap();
        assert!(
            (mesh_volume(&out) - (12.0f64.powi(3) - 1000.0)).abs() < 1e-6,
            "{}",
            mesh_volume(&out)
        );
    }
}
