//! Rounding of mesh edges: sharp-edge detection, chains of edges, and fillets / chamfers cut (or filled) with
//! swept boolean tools.
use super::repair::Topo;
use super::sweep::tube;
use super::region_tris;
use crate::geom::*;
use crate::solid::boolean::{boolean_sync, BoolOp};
use crate::solid::mesh::{mesh_volume, Group};
use crate::solid::{Mesh, V3};
use crate::view::View;
use std::collections::{HashMap, HashSet};

/// A sharp edge between two faces. `a → b` runs the way face 1 (normal `n1`) traverses it.
#[derive(Clone, Debug)]
pub struct Edge {
    pub a: u32,
    pub b: u32,
    pub n1: V3,
    pub n2: V3,
    /// The two faces (triangle indices) the edge joins; face 1 comes first.
    pub f1: usize,
    pub f2: usize,
    /// The solid is convex along the edge (material wedge narrower than a half-space).
    pub convex: bool,
}

impl Edge {
    fn key(&self) -> (u32, u32) {
        (self.a.min(self.b), self.a.max(self.b))
    }
}

/// Edges where the two faces meet at more than `min_angle` radians (sorted, so results are reproducible).
pub fn feature_edges(t: &Topo, min_angle: f64) -> Vec<Edge> {
    let cos_min = min_angle.cos();
    let mut keys: Vec<(&(u32, u32), &Vec<usize>)> = t.edges.iter().filter(|(_, f)| f.len() == 2).collect();
    keys.sort_by_key(|(k, _)| **k);
    let mut out = vec![];
    for (&(lo, hi), fs) in keys {
        let (n1, n2) = (t.normal[fs[0]], t.normal[fs[1]]);
        if n1.dot(n2) > cos_min {
            continue;
        }
        let tri = t.tris[fs[0]];
        let forward = [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])].contains(&(lo, hi));
        let (a, b) = if forward { (lo, hi) } else { (hi, lo) };
        let convex = n1.cross(n2).dot(t.pos[b as usize].sub(t.pos[a as usize])) > 0.0;
        out.push(Edge { a, b, n1, n2, f1: fs[0], f2: fs[1], convex });
    }
    out
}

/// A run of connected edges that are rounded together.
#[derive(Clone, Debug)]
pub struct Chain {
    pub pts: Vec<V3>,
    pub closed: bool,
    /// Per segment: outward normals of the face on the left / right of the direction of travel.
    pub n1: Vec<V3>,
    pub n2: Vec<V3>,
    /// Per segment: the faces (triangle indices) on those two sides.
    pub f1: Vec<usize>,
    pub f2: Vec<usize>,
    pub convex: bool,
}

/// An edge traversed `from → to`, with the normals of the faces on its two sides in that direction.
#[derive(Clone, Copy)]
struct Step {
    from: u32,
    to: u32,
    n1: V3,
    n2: V3,
    f1: usize,
    f2: usize,
    id: usize,
}

fn step(edges: &[Edge], id: usize, forward: bool) -> Step {
    let e = &edges[id];
    if forward {
        Step { from: e.a, to: e.b, n1: e.n1, n2: e.n2, f1: e.f1, f2: e.f2, id }
    } else {
        Step { from: e.b, to: e.a, n1: e.n2, n2: e.n1, f1: e.f2, f2: e.f1, id }
    }
}

/// Follow `from` through vertices where exactly one other edge of the same convexity continues smoothly
/// (turning less than `max_turn` radians, face normals changing less than that too).
fn walk(t: &Topo, edges: &[Edge], adj: &HashMap<u32, Vec<usize>>, first: Step, max_turn: f64, stop_at: usize) -> Vec<Step> {
    let mut out = vec![];
    let mut cur = first;
    let cos_turn = max_turn.cos();
    loop {
        let others: Vec<usize> = adj[&cur.to].iter().copied().filter(|e| *e != cur.id).collect();
        if others.len() != 1 || others[0] == stop_at || edges[others[0]].convex != edges[cur.id].convex {
            break;
        }
        let next = step(edges, others[0], edges[others[0]].a == cur.to);
        let (d0, d1) = (t.pos[cur.to as usize].sub(t.pos[cur.from as usize]).unit(), t.pos[next.to as usize].sub(t.pos[next.from as usize]).unit());
        if d0.dot(d1) < cos_turn || cur.n1.dot(next.n1) < cos_turn || cur.n2.dot(next.n2) < cos_turn {
            break;
        }
        out.push(next);
        cur = next;
    }
    out
}

/// The chain of smoothly continuing edges through edge `start`.
pub fn chain_through(t: &Topo, edges: &[Edge], start: usize, max_turn: f64) -> Chain {
    let mut adj: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        adj.entry(e.a).or_default().push(i);
        adj.entry(e.b).or_default().push(i);
    }
    let first = step(edges, start, true);
    let ahead = walk(t, edges, &adj, first, max_turn, start);
    let last = ahead.last().copied().unwrap_or(first);
    let closed = last.to == first.from;
    let mut steps: Vec<Step> = vec![first];
    steps.extend(ahead);
    if !closed {
        let back = walk(t, edges, &adj, step(edges, start, false), max_turn, start);
        let mut pre: Vec<Step> = back.iter().rev().map(|s| step(edges, s.id, edges[s.id].b == s.from)).collect();
        pre.append(&mut steps);
        steps = pre;
    }
    let mut pts: Vec<V3> = steps.iter().map(|s| t.pos[s.from as usize]).collect();
    if !closed {
        pts.push(t.pos[steps[steps.len() - 1].to as usize]);
    }
    Chain { pts, closed, n1: steps.iter().map(|s| s.n1).collect(), n2: steps.iter().map(|s| s.n2).collect(), f1: steps.iter().map(|s| s.f1).collect(), f2: steps.iter().map(|s| s.f2).collect(), convex: edges[start].convex }
}

/// Feature edges bounding the planar face that contains triangle `face`.
pub fn face_loop(t: &Topo, edges: &[Edge], face: usize) -> Vec<usize> {
    let region: HashSet<usize> = t.planar_region(face).into_iter().collect();
    (0..edges.len()).filter(|i| t.edges[&edges[*i].key()].iter().filter(|f| region.contains(f)).count() == 1).collect()
}

/// Direction from the model towards the viewer.
pub fn toward_viewer(view: &View) -> V3 {
    let (sy, cy, sp, cp) = (view.yaw.sin(), view.yaw.cos(), view.pitch.sin(), view.pitch.cos());
    V3::new(-sy * cp, -cy * cp, sp)
}

/// The face on either side of edge `id` that looks more towards the viewer.
pub fn front_face(t: &Topo, edges: &[Edge], id: usize, view: &View) -> usize {
    let toward = toward_viewer(view);
    let fs = &t.edges[&edges[id].key()];
    *fs.iter().max_by(|a, b| t.normal[**a].dot(toward).total_cmp(&t.normal[**b].dot(toward))).unwrap_or(&fs[0])
}

/// The edge drawn closest to screen point `sp` (within `tol` pixels) and its depth; among overlapping edges the one
/// nearest to the viewer.
pub fn pick_edge(t: &Topo, edges: &[Edge], view: &View, sp: Pt, tol: f64) -> Option<(usize, f64)> {
    let mut hits: Vec<(f64, f64, usize)> = vec![];
    for (i, e) in edges.iter().enumerate() {
        let (p, q) = (view.project(t.pos[e.a as usize]), view.project(t.pos[e.b as usize]));
        let (dx, dy) = (q.x - p.x, q.y - p.y);
        let l2 = dx * dx + dy * dy;
        let u = if l2 < 1e-12 { 0.0 } else { (((sp.x - p.x) * dx + (sp.y - p.y) * dy) / l2).clamp(0.0, 1.0) };
        let d = (sp.x - p.x - dx * u).hypot(sp.y - p.y - dy * u);
        if d <= tol {
            hits.push((d, p.depth + (q.depth - p.depth) * u, i));
        }
    }
    let best = hits.iter().map(|h| h.0).fold(f64::INFINITY, f64::min);
    hits.into_iter().filter(|h| h.0 <= best + 1.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|h| (h.2, h.1))
}

// ---------------------------------------------------------------- rounding

/// How an edge is rounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Round {
    Fillet(f64),
    /// Distances along face 1 and face 2.
    Chamfer(f64, f64),
}

impl Round {
    fn reach(&self) -> f64 {
        match *self {
            Round::Fillet(r) => r,
            Round::Chamfer(a, b) => a.max(b),
        }
    }
}

const ARC_STEPS: usize = 12;

fn perp(v: V3, t: V3) -> V3 {
    v.sub(t.scale(v.dot(t))).unit()
}

/// Outline, in the plane across the edge at `e`, of the material to cut away (convex edge) or add (concave edge),
/// padded `pad` beyond the faces so the tool never lies exactly in a face.
fn section(e: V3, t: V3, n1: V3, n2: V3, convex: bool, kind: Round, pad: f64) -> Result<Vec<V3>, String> {
    let (n1, n2) = (perp(n1, t), perp(n2, t));
    let (u1, u2) = (n1.cross(t), t.cross(n2));
    let alpha = u1.dot(u2).clamp(-1.0, 1.0).acos();
    if !(0.02..std::f64::consts::PI - 0.02).contains(&alpha) {
        return Err("The faces along this edge are too close to flat or too sharp to round.".into());
    }
    let mut poly = match kind {
        Round::Fillet(r) => {
            let (t1, t2) = (e.add(u1.scale(r / (alpha / 2.0).tan())), e.add(u2.scale(r / (alpha / 2.0).tan())));
            let c = e.add(u1.add(u2).unit().scale(r / (alpha / 2.0).sin()));
            let (a1, a2) = (t1.sub(c), t2.sub(c));
            let phi = std::f64::consts::PI - alpha;
            let s = if a1.cross(a2).dot(t) < 0.0 { -1.0 } else { 1.0 };
            (0..=ARC_STEPS)
                .map(|k| {
                    let a = s * phi * k as f64 / ARC_STEPS as f64;
                    c.add(a1.scale(a.cos())).add(t.cross(a1).scale(a.sin()))
                })
                .collect::<Vec<_>>()
        }
        Round::Chamfer(d1, d2) => vec![e.add(u1.scale(d1)), e.add(u2.scale(d2))],
    };
    let out = if convex { 1.0 } else { -1.0 };
    let (first, last) = (poly[0], poly[poly.len() - 1]);
    poly.push(last.add(n2.scale(out * pad)));
    poly.push(e.add(n1.add(n2).scale(out * pad)));
    poly.push(first.add(n1.scale(out * pad)));
    Ok(poly)
}

/// Round one chain with a boolean tool swept along it.
fn round_chain(m: &Mesh, c: &Chain, kind: Round) -> Result<Mesh, String> {
    let n = c.pts.len();
    let segs = if c.closed { n } else { n - 1 };
    let tan: Vec<V3> = (0..segs).map(|j| c.pts[(j + 1) % n].sub(c.pts[j]).unit()).collect();
    let pad = (0.03 * kind.reach()).max(1e-4);
    let at = |i: usize| -> (V3, V3, V3) {
        let (a, b) = if c.closed { ((i + segs - 1) % segs, i % segs) } else { (i.saturating_sub(1).min(segs - 1), i.min(segs - 1)) };
        let mix = |x: &[V3]| if a == b { x[a] } else { x[a].add(x[b]).unit() };
        (mix(&tan), mix(&c.n1), mix(&c.n2))
    };
    let mut rings: Vec<Vec<V3>> = vec![];
    for i in 0..n {
        let (t, n1, n2) = at(i);
        rings.push(section(c.pts[i], t, n1, n2, c.convex, kind, pad)?);
    }
    if !c.closed && c.convex {
        // run the tool a little past the ends so it leaves clean cuts through the end faces
        let (t0, t1) = (tan[0], tan[segs - 1]);
        rings[0].iter_mut().for_each(|p| *p = p.sub(t0.scale(pad)));
        rings[n - 1].iter_mut().for_each(|p| *p = p.add(t1.scale(pad)));
    }
    let caps = |ring: &[V3], t: V3| -> Result<Vec<[usize; 3]>, String> {
        let e1 = perp(ring[0].sub(ring[ring.len() / 2]), t);
        let e2 = t.cross(e1);
        let pts: Vec<Pt> = ring.iter().map(|p| pt(p.dot(e1), p.dot(e2))).collect();
        let flipped = polygon_area(&pts) < 0.0;
        let pts: Vec<Pt> = if flipped { pts.iter().map(|p| pt(p.x, -p.y)).collect() } else { pts };
        region_tris(&[Group { outer: pts, holes: vec![] }])
    };
    let st: Vec<Vec<Vec<V3>>> = rings.iter().map(|r| vec![r.clone()]).collect();
    let (start, end) = if c.closed { (vec![], vec![]) } else { (caps(&rings[0], tan[0])?, caps(&rings[n - 1], tan[segs - 1])?) };
    let tool = tube(&st, c.closed, &start, &end);
    let before = mesh_volume(m);
    let out = boolean_sync(m, &tool, if c.convex { BoolOp::Subtract } else { BoolOp::Union })?;
    let after = mesh_volume(&out);
    let changed = if c.convex { before - after } else { after - before };
    if out.faces.is_empty() || changed <= 1e-9 * before.abs() {
        return Err("The edge could not be rounded; the distance is probably too large for the faces beside it.".into());
    }
    Ok(out)
}

/// Every segment's rounding must fit on the (planar) faces beside it.
fn check_reach(t: &Topo, c: &Chain, kind: Round) -> Result<(), String> {
    let mut regions: HashMap<usize, Vec<usize>> = HashMap::new();
    for j in 0..c.f1.len() {
        let (e, tj) = (c.pts[j], c.pts[(j + 1) % c.pts.len()].sub(c.pts[j]).unit());
        let (u1, u2) = (perp(c.n1[j], tj).cross(tj), tj.cross(perp(c.n2[j], tj)));
        let half = u1.dot(u2).clamp(-1.0, 1.0).acos() / 2.0;
        let (d1, d2) = match kind {
            Round::Fillet(r) => (r / half.tan(), r / half.tan()),
            Round::Chamfer(a, b) => (a, b),
        };
        for (f, u, d) in [(c.f1[j], u1, d1), (c.f2[j], u2, d2)] {
            let region = regions.entry(f).or_insert_with(|| t.planar_region(f));
            let reach = region.iter().flat_map(|g| t.tris[*g]).map(|v| t.pos[v as usize].sub(e).dot(u)).fold(0.0, f64::max);
            if reach < d - 1e-6 {
                return Err("The distance is too large for the faces beside the edge.".into());
            }
        }
    }
    Ok(())
}

/// Fillet or chamfer chains (which must come from this mesh's own [`feature_edges`]), each with its own rounding.
pub fn round_edges(m: &Mesh, jobs: &[(Chain, Round)]) -> Result<Mesh, String> {
    let topo = Topo::new(m);
    for (c, kind) in jobs {
        check_reach(&topo, c, *kind)?;
    }
    let mut cur = m.clone();
    for (c, kind) in jobs {
        cur = round_chain(&cur, c, *kind)?;
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::mesh::{box_mesh, cylinder_mesh, extrude_loops, is_watertight};

    fn cube(s: f64) -> Mesh {
        box_mesh(0.0, 0.0, 0.0, s, s, s, "Box").mesh()
    }

    fn edges_of(m: &Mesh) -> (Topo, Vec<Edge>) {
        let t = Topo::new(m);
        let e = feature_edges(&t, 0.3);
        (t, e)
    }

    #[test]
    fn a_cube_has_twelve_sharp_edges_all_convex() {
        let (_, e) = edges_of(&cube(10.0));
        assert_eq!(e.len(), 12);
        assert!(e.iter().all(|x| x.convex));
    }

    #[test]
    fn chains_follow_smooth_runs_only() {
        let (t, e) = edges_of(&cube(10.0));
        assert_eq!(chain_through(&t, &e, 0, 0.45).pts.len(), 2);
        let cyl = cylinder_mesh(0.0, 0.0, 0.0, 5.0, 8.0, 32, "C").mesh();
        let (t, e) = edges_of(&cyl);
        let c = chain_through(&t, &e, 0, 0.45);
        assert!(c.closed, "the rim of a cylinder is one closed chain");
        assert_eq!(c.pts.len(), 32);
    }

    #[test]
    fn filleting_a_box_edge_removes_the_corner_sliver() {
        let m = cube(20.0);
        let (t, e) = edges_of(&m);
        let c = chain_through(&t, &e, 0, 0.45);
        let out = round_edges(&m, &[(c, Round::Fillet(3.0))]).unwrap();
        let removed = mesh_volume(&m) - mesh_volume(&out);
        let exact = (1.0 - PI / 4.0) * 9.0 * 20.0;
        assert!((removed - exact).abs() / exact < 0.02, "{removed} vs {exact}");
        assert!(is_watertight(&out) || mesh_volume(&out) > 0.0);
    }

    #[test]
    fn chamfering_and_asymmetric_chamfers() {
        let m = cube(20.0);
        let (t, e) = edges_of(&m);
        let c = chain_through(&t, &e, 0, 0.45);
        let out = round_edges(&m, &[(c.clone(), Round::Chamfer(2.0, 2.0))]).unwrap();
        assert!((mesh_volume(&m) - mesh_volume(&out) - 0.5 * 4.0 * 20.0).abs() < 1e-4, "{}", mesh_volume(&m) - mesh_volume(&out));
        let asym = round_edges(&m, &[(c, Round::Chamfer(2.0, 5.0))]).unwrap();
        assert!((mesh_volume(&m) - mesh_volume(&asym) - 0.5 * 10.0 * 20.0).abs() < 1e-4);
    }

    #[test]
    fn rounding_the_rim_of_a_cylinder_and_all_edges_of_a_cube() {
        let cyl = cylinder_mesh(0.0, 0.0, 0.0, 10.0, 20.0, 64, "C").mesh();
        let (t, e) = edges_of(&cyl);
        let top = e.iter().position(|x| t.pos[x.a as usize].z > 19.0).unwrap();
        let c = chain_through(&t, &e, top, 0.45);
        assert!(c.closed);
        let out = round_edges(&cyl, &[(c, Round::Fillet(2.0))]).unwrap();
        // the removed ring has cross-section (1 - pi/4) r^2 and its centroid lies 0.2234 r inside the corner
        let removed = mesh_volume(&cyl) - mesh_volume(&out);
        let approx = (1.0 - PI / 4.0) * 4.0 * 2.0 * PI * (10.0 - 0.2234 * 2.0);
        assert!((removed - approx).abs() / approx < 0.03, "{removed} vs {approx}");
        let m = cube(20.0);
        let (t, e) = edges_of(&m);
        let top: Vec<(Chain, Round)> = (0..e.len()).filter(|i| t.pos[e[*i].a as usize].z > 19.0 && t.pos[e[*i].b as usize].z > 19.0).map(|i| (chain_through(&t, &e, i, 0.45), Round::Fillet(2.0))).collect();
        assert_eq!(top.len(), 4);
        let all = round_edges(&m, &top).unwrap();
        let single = (1.0 - PI / 4.0) * 4.0 * 20.0;
        assert!(mesh_volume(&m) - mesh_volume(&all) > 3.5 * single, "{}", mesh_volume(&m) - mesh_volume(&all));
    }

    #[test]
    fn concave_edges_are_filled() {
        let l = vec![pt(0.0, 0.0), pt(30.0, 0.0), pt(30.0, 10.0), pt(10.0, 10.0), pt(10.0, 30.0), pt(0.0, 30.0)];
        let m = extrude_loops(&[l], 12.0, 0.0, "L", 0.0).unwrap().mesh();
        let (t, e) = edges_of(&m);
        let inner = e.iter().position(|x| !x.convex && (t.pos[x.a as usize].x - 10.0).abs() < 1e-9 && (t.pos[x.a as usize].y - 10.0).abs() < 1e-9).expect("the inner corner edge");
        let c = chain_through(&t, &e, inner, 0.45);
        assert!(!c.convex);
        let out = round_edges(&m, &[(c, Round::Fillet(3.0))]).unwrap();
        let added = mesh_volume(&out) - mesh_volume(&m);
        let exact = (1.0 - PI / 4.0) * 9.0 * 12.0;
        assert!((added - exact).abs() / exact < 0.05, "{added} vs {exact}");
    }

    #[test]
    fn oversized_rounds_fail_cleanly() {
        let m = box_mesh(0.0, 0.0, 0.0, 20.0, 20.0, 2.0, "Plate").mesh();
        let (t, e) = edges_of(&m);
        let c = chain_through(&t, &e, 0, 0.45);
        assert!(round_edges(&m, &[(c, Round::Fillet(15.0))]).is_err());
    }

    #[test]
    fn loops_and_picking() {
        let m = cube(10.0);
        let (t, e) = edges_of(&m);
        let top = (0..m.faces.len()).find(|f| t.normal[*f].z > 0.99).unwrap();
        assert_eq!(face_loop(&t, &e, top).len(), 4);
        let mut v = View::new();
        v.scale = 10.0;
        let sp = v.to_screen(pt(0.0, 5.0));
        let (id, _) = pick_edge(&t, &e, &v, sp, 6.0).expect("edge at x = 0");
        assert!((t.pos[e[id].a as usize].x).abs() < 1e-9 && (t.pos[e[id].b as usize].x).abs() < 1e-9);
        assert!(t.pos[e[id].a as usize].z > 9.0, "the edge nearest the viewer wins");
        assert!(pick_edge(&t, &e, &v, v.to_screen(pt(5.0, 5.0)), 6.0).is_none());
    }
}
