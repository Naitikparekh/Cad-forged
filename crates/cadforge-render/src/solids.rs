//! Triangle-mesh solids: painter's-algorithm shading with smoothed normals, feature edges and the AutoCAD visual styles
//! (port of `src/render/solids.js`).
use crate::ops::*;
use crate::scene::{Highlight, Renderer};
use cadforge_core::geom::{rad, PI};
use cadforge_core::solid::{Solid, V3};
use cadforge_core::style::{color_rgb, Color};
use std::collections::HashMap;
use std::rc::Rc;

fn crease() -> f64 {
    rad(35.0).cos()
}
fn edge_angle() -> f64 {
    rad(25.0).cos()
}

type N3 = [f64; 3];

fn dot3(a: N3, b: N3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn face_normal(v: &[V3], f: &[u32; 3]) -> (N3, f64) {
    let (a, b, c) = (v[f[0] as usize], v[f[1] as usize], v[f[2] as usize]);
    let n = b.sub(a).cross(c.sub(a));
    let l = n.len();
    if l < 1e-15 {
        ([0.0, 0.0, 1.0], 0.0)
    } else {
        ([n.x / l, n.y / l, n.z / l], l / 2.0)
    }
}

/// Per-solid derived data: smoothed per-face shading normals, adjacency and feature edges.
pub struct Derived {
    rev: u64,
    nv: usize,
    nf: usize,
    smooth: Vec<N3>,
    edges: Vec<(u32, u32)>,
    adj: Vec<[i64; 3]>,
    feat: Vec<[bool; 3]>,
}

#[derive(Default)]
pub struct SolidCache {
    map: HashMap<u64, Rc<Derived>>,
}

impl SolidCache {
    fn derived(&mut self, s: &Solid, rev: u64) -> Rc<Derived> {
        if let Some(d) = self.map.get(&s.id) {
            if d.rev == rev && d.nv == s.vertices.len() && d.nf == s.faces.len() {
                return d.clone();
            }
        }
        let d = Rc::new(compute(s, rev));
        self.map.insert(s.id, d.clone());
        d
    }
}

fn compute(s: &Solid, rev: u64) -> Derived {
    let (v, f) = (&s.vertices, &s.faces);
    let fnorm: Vec<(N3, f64)> = f.iter().map(|t| face_normal(v, t)).collect();
    let mut incident: Vec<Vec<usize>> = vec![vec![]; v.len()];
    for (i, t) in f.iter().enumerate() {
        for &x in t {
            incident[x as usize].push(i);
        }
    }
    let crease = crease();
    let smooth: Vec<N3> = f
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut acc = [0.0; 3];
            let n0 = fnorm[i].0;
            for &x in t {
                for &j in &incident[x as usize] {
                    let nj = &fnorm[j];
                    if dot3(n0, nj.0) >= crease {
                        for k in 0..3 {
                            acc[k] += nj.0[k] * nj.1;
                        }
                    }
                }
            }
            let l = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt();
            let l = if l == 0.0 { 1.0 } else { l };
            [acc[0] / l, acc[1] / l, acc[2] / l]
        })
        .collect();
    struct E {
        a: u32,
        b: u32,
        n1: N3,
        n2: Option<N3>,
        fi: usize,
        k: usize,
    }
    let mut map: HashMap<(u32, u32), E> = HashMap::new();
    let mut order: Vec<(u32, u32)> = vec![];
    let mut adj = vec![[-1i64; 3]; f.len()];
    let mut feat = vec![[true; 3]; f.len()];
    let ea = edge_angle();
    for (fi, t) in f.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let key = if a < b { (a, b) } else { (b, a) };
            if let Some(e) = map.get_mut(&key) {
                e.n2 = Some(fnorm[fi].0);
                adj[fi][k] = e.fi as i64;
                adj[e.fi][e.k] = fi as i64;
                let sharp = dot3(e.n1, fnorm[fi].0) < ea;
                feat[fi][k] = sharp;
                feat[e.fi][e.k] = sharp;
            } else {
                map.insert(key, E { a, b, n1: fnorm[fi].0, n2: None, fi, k });
                order.push(key);
            }
        }
    }
    let mut edges = vec![];
    for key in order {
        let e = &map[&key];
        match e.n2 {
            None => edges.push((e.a, e.b)),
            Some(n2) if dot3(e.n1, n2) < ea => edges.push((e.a, e.b)),
            _ => {}
        }
    }
    Derived { rev, nv: v.len(), nf: f.len(), smooth, edges, adj, feat }
}

fn light() -> N3 {
    let l: N3 = [-0.35, 0.45, 0.82];
    let n = dot3(l, l).sqrt();
    [l[0] / n, l[1] / n, l[2] / n]
}

fn base_color(rc: &Renderer, s: &Solid) -> Rgb {
    let doc = rc.doc;
    let layer = doc.layer(if s.layer.is_empty() { "0" } else { &s.layer }).unwrap_or(&doc.layers[0]);
    let col = s.color.unwrap_or(layer.color);
    match col {
        Color::Aci(7) | Color::ByBlock | Color::ByLayer => {
            if rc.theme.dark {
                [0xc3, 0xcc, 0xd6]
            } else {
                [0x86, 0x95, 0xa4]
            }
        }
        c => color_rgb(c, rc.theme.dark, rc.theme.contrast),
    }
}

/// Draw every visible solid in the given visual style (`2D Wireframe`, `Wireframe`, `Hidden`, `Shaded`,
/// `Shaded with Edges`).
pub fn draw_solids(rc: &mut Renderer, cache: &mut SolidCache, hl: &Highlight, style: &str) {
    let doc = rc.doc;
    if doc.solids.is_empty() {
        return;
    }
    let (view, theme) = (rc.view, rc.theme);
    let wire = style == "2D Wireframe" || style == "Wireframe";
    let fill = !wire;
    let edges_on = style != "Shaded";
    let (cy, sy, cp, sp) = (view.yaw.cos(), view.yaw.sin(), view.pitch.cos(), view.pitch.sin());
    let cam_n = |n: N3| -> N3 {
        let (x1, y1) = (n[0] * cy - n[1] * sy, n[0] * sy + n[1] * cy);
        [x1, y1 * sp + n[2] * cp, -y1 * cp + n[2] * sp]
    };
    let l = light();
    struct Poly3 {
        pts: [[f64; 2]; 3],
        depth: f64,
        col: Rgb,
        pe: Vec<([f64; 2], [f64; 2])>,
        sel: bool,
    }
    let mut polys: Vec<Poly3> = vec![];
    struct EdgeSet {
        p: Vec<[f64; 2]>,
        d: Rc<Derived>,
        sel: bool,
    }
    let mut edge_sets: Vec<EdgeSet> = vec![];
    for s in &doc.solids {
        if !doc.solid_visible(s) {
            continue;
        }
        let pr: Vec<_> = s.vertices.iter().map(|v| view.project(*v)).collect();
        let d = cache.derived(s, doc.rev);
        let sel = hl.selected.contains(&s.id);
        let base = base_color(rc, s);
        if fill {
            let front = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) < 0.0;
            let xy = |i: u32| [pr[i as usize].x, pr[i as usize].y];
            let face_front: Vec<bool> = s.faces.iter().map(|f| front(xy(f[0]), xy(f[1]), xy(f[2]))).collect();
            for (i, f) in s.faces.iter().enumerate() {
                if !face_front[i] {
                    continue; // back face
                }
                let n = cam_n(d.smooth[i]);
                let lum = if style == "Hidden" { 1.0 } else { 0.3 + 0.7 * dot3(n, l).max(0.0) };
                let mut pe = vec![];
                for k in 0..3 {
                    let o = d.adj[i][k];
                    if (edges_on || sel) && (d.feat[i][k] || o < 0 || !face_front[o as usize]) {
                        pe.push((xy(f[k]), xy(f[(k + 1) % 3])));
                    }
                }
                let col = if style == "Hidden" { theme.canvas } else { [0, 1, 2].map(|c| ((base[c] as f64 * lum * 1.08).round().min(255.0)) as u8) };
                let depth = (pr[f[0] as usize].depth + pr[f[1] as usize].depth + pr[f[2] as usize].depth) / 3.0;
                polys.push(Poly3 { pts: [xy(f[0]), xy(f[1]), xy(f[2])], depth, col, pe, sel });
            }
        }
        edge_sets.push(EdgeSet { p: pr.iter().map(|q| [q.x, q.y]).collect(), d, sel });
    }
    polys.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    let edge_col: Rgb = if theme.dark { [8, 14, 20] } else { [10, 20, 30] };
    let edge_alpha = if theme.dark { 0.9 } else { 0.85 };
    for p in &polys {
        let tri = crate::ops::Poly { pts: p.pts.to_vec(), closed: true };
        rc.ops.push(Op::Fill { paths: vec![tri.clone()], color: p.col, alpha: 1.0, evenodd: false });
        rc.ops.push(Op::Stroke { paths: vec![tri], color: p.col, alpha: 1.0, width: 0.7, dash: vec![], round: true });
        if !p.pe.is_empty() {
            let paths = p.pe.iter().map(|(a, b)| crate::ops::Poly { pts: vec![*a, *b], closed: false }).collect();
            rc.ops.push(Op::Stroke { paths, color: if p.sel { theme.select } else { edge_col }, alpha: if p.sel { 1.0 } else { edge_alpha }, width: if p.sel { 2.0 } else { 1.0 }, dash: vec![], round: true });
        }
    }
    if wire {
        let mut paths = vec![];
        for es in edge_sets.iter().filter(|e| !e.sel) {
            for &(a, b) in &es.d.edges {
                paths.push(crate::ops::Poly { pts: vec![es.p[a as usize], es.p[b as usize]], closed: false });
            }
        }
        if !paths.is_empty() {
            rc.ops.push(Op::Stroke { paths, color: if theme.dark { [0x7f, 0xd0, 0xff] } else { [0x0b, 0x5a, 0x96] }, alpha: 1.0, width: 1.15, dash: vec![], round: true });
        }
        for es in edge_sets.iter().filter(|e| e.sel) {
            let paths = es.d.edges.iter().map(|&(a, b)| crate::ops::Poly { pts: vec![es.p[a as usize], es.p[b as usize]], closed: false }).collect();
            rc.ops.push(Op::Stroke { paths, color: theme.select, alpha: 1.0, width: 2.2, dash: vec![], round: true });
        }
    }
    let _ = PI;
}
