//! Closed-region detection from an internal point (HATCH pick-points, BOUNDARY, REGION).
//!
//! All curve segments in the drawing are split at their mutual intersections to form a planar graph. Every
//! half-edge belongs to exactly one face cycle. The region containing the point is the smallest
//! counter-clockwise cycle that encloses it; clockwise cycles (outer boundaries of other connected pieces)
//! lying inside it are islands.
use super::path::edge_segs;
use crate::drawing::Drawing;
use crate::entities::{Entity, Kind};
use crate::geom::*;
use std::collections::HashMap;

const MAX_SEGS: usize = 6000;
const TOL: f64 = 1e-6;

/// Split a segment at the params `ts`, returning sub-segments (tiny ones dropped).
fn split_seg(s: &Seg, ts: &[f64]) -> Vec<Seg> {
    let mut pts: Vec<f64> = vec![0.0];
    pts.extend(ts.iter().copied().filter(|t| *t > 1e-9 && *t < 1.0 - 1e-9));
    pts.push(1.0);
    pts.sort_by(|a, b| a.total_cmp(b));
    let mut out = vec![];
    for w in pts.windows(2) {
        if w[1] - w[0] < 1e-9 {
            continue;
        }
        let piece = s.slice(w[0], w[1]);
        if piece.length() > TOL {
            out.push(piece);
        }
    }
    out
}

pub fn planar_edges(segs: &[Seg]) -> Vec<Seg> {
    let bounds: Vec<Bounds> = segs.iter().map(|s| s.bounds()).collect();
    let mut cuts: Vec<Vec<f64>> = vec![vec![]; segs.len()];
    for i in 0..segs.len() {
        let gi = bounds[i].grow(1e-7);
        for j in i + 1..segs.len() {
            if !gi.overlaps(&bounds[j]) {
                continue;
            }
            for h in seg_intersect(&segs[i], &segs[j], false, false) {
                cuts[i].push(h.t1);
                cuts[j].push(h.t2);
            }
        }
    }
    let mut out = vec![];
    for (i, s) in segs.iter().enumerate() {
        if s.is_full() {
            // a closed circle needs a node even without intersections
            let ts = if cuts[i].is_empty() {
                vec![0.0]
            } else {
                cuts[i].clone()
            };
            let mut sorted: Vec<f64> = ts.iter().map(|t| (t * 1e9).round() / 1e9).collect();
            sorted.sort_by(|a, b| a.total_cmp(b));
            sorted.dedup();
            if sorted.len() == 1 {
                out.push(s.slice(sorted[0], sorted[0] + 0.5));
                out.push(s.slice(sorted[0] + 0.5, sorted[0] + 1.0));
                continue;
            }
            for k in 0..sorted.len() {
                let t0 = sorted[k];
                let t1 = if k + 1 < sorted.len() {
                    sorted[k + 1]
                } else {
                    sorted[0] + 1.0
                };
                out.push(s.slice(t0, t1));
            }
            continue;
        }
        out.extend(split_seg(s, &cuts[i]));
    }
    out
}

struct Half {
    from: usize,
    to: usize,
    seg: Seg,
    twin: usize,
    angle: f64,
}

struct Graph {
    nodes: Vec<Pt>,
    key_map: HashMap<(i64, i64), usize>,
    half: Vec<Half>,
    out: Vec<Vec<usize>>,
    alive: Vec<bool>,
}

fn round_js(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

impl Graph {
    fn new(edges: &[Seg]) -> Graph {
        let mut g = Graph {
            nodes: vec![],
            key_map: HashMap::new(),
            half: vec![],
            out: vec![],
            alive: vec![],
        };
        for e in edges {
            let a = g.node(e.start());
            let b = g.node(e.end());
            if a == b {
                match e {
                    Seg::A { sw, .. } if sw.abs() < TAU - 1e-6 => continue,
                    Seg::L { .. } => continue,
                    _ => {}
                }
            }
            let i = g.half.len();
            g.half.push(Half {
                from: a,
                to: b,
                seg: *e,
                twin: i + 1,
                angle: 0.0,
            });
            g.half.push(Half {
                from: b,
                to: a,
                seg: e.reverse(),
                twin: i,
                angle: 0.0,
            });
        }
        g.out = vec![vec![]; g.nodes.len()];
        for (id, h) in g.half.iter().enumerate() {
            g.out[h.from].push(id);
        }
        for h in &mut g.half {
            h.angle = angle_of(pt(0.0, 0.0), h.seg.tangent(0.0));
        }
        g.alive = vec![true; g.half.len()];
        g
    }

    fn node(&mut self, p: Pt) -> usize {
        let k = (round_js(p.x / TOL), round_js(p.y / TOL));
        let mut id = self.key_map.get(&k).copied();
        if id.is_none() {
            for (dx, dy) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (-1, -1),
                (1, -1),
                (-1, 1),
            ] {
                if let Some(&near) = self.key_map.get(&(k.0 + dx, k.1 + dy)) {
                    if dist(self.nodes[near], p) < TOL * 2.0 {
                        id = Some(near);
                        break;
                    }
                }
            }
        }
        id.unwrap_or_else(|| {
            let id = self.nodes.len();
            self.nodes.push(p);
            self.key_map.insert(k, id);
            id
        })
    }

    /// Remove edges that cannot be part of a cycle (degree-1 chains).
    fn prune(&mut self) {
        let mut alive = vec![true; self.half.len()];
        let mut deg: Vec<i64> = self.out.iter().map(|o| o.len() as i64).collect();
        let mut queue: Vec<usize> = deg
            .iter()
            .enumerate()
            .filter(|(_, d)| **d == 1)
            .map(|(n, _)| n)
            .collect();
        while let Some(n) = queue.pop() {
            for &hid in &self.out[n] {
                if !alive[hid] {
                    continue;
                }
                let (from, to, twin) =
                    (self.half[hid].from, self.half[hid].to, self.half[hid].twin);
                alive[hid] = false;
                alive[twin] = false;
                deg[from] -= 1;
                deg[to] -= 1;
                if deg[to] == 1 {
                    queue.push(to);
                }
            }
        }
        for list in &mut self.out {
            list.retain(|id| alive[*id]);
        }
        self.alive = alive;
    }

    /// All face cycles as lists of half-edge ids.
    fn cycles(&self) -> Vec<Vec<usize>> {
        let mut seen = vec![false; self.half.len()];
        let mut res = vec![];
        for hid in 0..self.half.len() {
            if !self.alive[hid] || seen[hid] {
                continue;
            }
            let mut cyc = vec![];
            let mut cur = hid;
            let mut guard = 0;
            while !seen[cur] && guard < self.half.len() + 2 {
                guard += 1;
                seen[cur] = true;
                cyc.push(cur);
                let h = &self.half[cur];
                let twin_angle = self.half[h.twin].angle;
                // next edge: first outgoing at h.to clockwise from the twin direction
                let mut best: Option<usize> = None;
                let mut best_turn = f64::INFINITY;
                for &oid in &self.out[h.to] {
                    if oid == h.twin && self.out[h.to].len() > 1 {
                        continue;
                    }
                    let mut turn = norm_angle(twin_angle - self.half[oid].angle);
                    if turn == 0.0 {
                        turn = TAU;
                    }
                    if turn < best_turn {
                        best_turn = turn;
                        best = Some(oid);
                    }
                }
                cur = best.unwrap_or(h.twin);
            }
            if !cyc.is_empty() && cur == hid {
                res.push(cyc);
            }
        }
        res
    }

    fn cycle_segs(&self, cyc: &[usize]) -> Vec<Seg> {
        cyc.iter().map(|&id| self.half[id].seg).collect()
    }
}

pub struct Region {
    /// Outer loop first, then islands.
    pub loops: Vec<Vec<Vertex>>,
}

/// Find the region around `p` among `entities` (default: the visible entities of the active space).
pub fn find_region(doc: &Drawing, p: Pt, entities: Option<&[&Entity]>) -> Result<Region, String> {
    let list: Vec<&Entity> = match entities {
        Some(l) => l.to_vec(),
        None => doc
            .entities()
            .iter()
            .filter(|e| doc.is_visible(e))
            .collect(),
    };
    let segs: Vec<Seg> = list
        .iter()
        .flat_map(|e| {
            if matches!(e.kind, Kind::Hatch(_)) {
                vec![]
            } else {
                edge_segs(e, doc)
            }
        })
        .filter(|s| !matches!(s, Seg::L { a, .. } if a.x.abs() > 5e5))
        .collect();
    if segs.len() > MAX_SEGS {
        return Err("The drawing has too many objects for boundary detection.".into());
    }
    if segs.is_empty() {
        return Err("No closed boundary found.".into());
    }
    let mut graph = Graph::new(&planar_edges(&segs));
    graph.prune();
    struct Cyc {
        c: Vec<usize>,
        poly: Vec<Pt>,
        area: f64,
    }
    let cycles: Vec<Cyc> = graph
        .cycles()
        .into_iter()
        .map(|c| {
            let poly = tess_segs(&graph.cycle_segs(&c), 0.02);
            let area = polygon_area(&poly);
            Cyc { c, poly, area }
        })
        .collect();
    let mut enclosing: Vec<usize> = (0..cycles.len())
        .filter(|&i| cycles[i].area > 1e-9 && point_in_polygon(p, &cycles[i].poly))
        .collect();
    enclosing.sort_by(|&a, &b| cycles[a].area.total_cmp(&cycles[b].area));
    let Some(&outer) = enclosing.first() else {
        return Err("Unable to determine a valid closed boundary at that point.".into());
    };
    // component id for each node, to exclude the outer's own component from islands
    let mut comp = vec![-1i64; graph.nodes.len()];
    let mut cid = 0;
    for n in 0..graph.nodes.len() {
        if comp[n] >= 0 || graph.out[n].is_empty() {
            continue;
        }
        let mut stack = vec![n];
        comp[n] = cid;
        while let Some(m) = stack.pop() {
            for &hid in &graph.out[m] {
                let t = graph.half[hid].to;
                if comp[t] < 0 {
                    comp[t] = cid;
                    stack.push(t);
                }
            }
        }
        cid += 1;
    }
    let outer_comp = comp[graph.half[cycles[outer].c[0]].from];
    let verts = |c: &Cyc| segs_to_verts(&graph.cycle_segs(&c.c), true);
    let mut loops = vec![verts(&cycles[outer])];
    for (i, c) in cycles.iter().enumerate() {
        if c.area >= -1e-9 || i == outer {
            continue;
        }
        if comp[graph.half[c.c[0]].from] == outer_comp {
            continue;
        }
        if point_in_polygon(c.poly[0], &cycles[outer].poly) {
            loops.push(verts(c));
        }
    }
    Ok(Region { loops })
}
