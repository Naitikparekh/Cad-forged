//! Boolean operations on triangle meshes (union / subtract / intersect) using BSP trees
//! (port of `src/solid/boolean.js` over Evan Wallace's csg.js, MIT licence).
use super::{Mesh, V3};
use std::collections::HashMap;

pub const MAX_TRIANGLES: usize = 100_000;
const EPSILON: f64 = 1e-5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Subtract,
    Intersect,
}

impl BoolOp {
    pub fn parse(s: &str) -> Option<BoolOp> {
        match s {
            "union" => Some(BoolOp::Union),
            "subtract" => Some(BoolOp::Subtract),
            "intersect" => Some(BoolOp::Intersect),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            BoolOp::Union => "Union",
            BoolOp::Subtract => "Subtract",
            BoolOp::Intersect => "Intersect",
        }
    }
}

#[derive(Clone, Copy)]
struct Vertex {
    pos: V3,
    normal: V3,
}

impl Vertex {
    fn flip(&mut self) {
        self.normal = self.normal.scale(-1.0);
    }
    fn interpolate(&self, o: &Vertex, t: f64) -> Vertex {
        Vertex {
            pos: lerp(self.pos, o.pos, t),
            normal: lerp(self.normal, o.normal, t),
        }
    }
}

fn lerp(a: V3, b: V3, t: f64) -> V3 {
    a.add(b.sub(a).scale(t))
}

#[derive(Clone, Copy)]
struct Plane {
    normal: V3,
    w: f64,
}

#[derive(Clone)]
struct Polygon {
    vertices: Vec<Vertex>,
    plane: Plane,
}

impl Polygon {
    fn new(vertices: Vec<Vertex>) -> Polygon {
        let (a, b, c) = (vertices[0].pos, vertices[1].pos, vertices[2].pos);
        let n = b.sub(a).cross(c.sub(a));
        let l = n.len();
        let n = n.scale(1.0 / l);
        Polygon {
            plane: Plane {
                normal: n,
                w: n.dot(a),
            },
            vertices,
        }
    }
    fn flip(&mut self) {
        self.vertices.reverse();
        for v in &mut self.vertices {
            v.flip();
        }
        self.plane.normal = self.plane.normal.scale(-1.0);
        self.plane.w = -self.plane.w;
    }
}

#[derive(Clone, Copy)]
enum Where {
    CoplanarFront,
    CoplanarBack,
    Front,
    Back,
}

const COPLANAR: u8 = 0;
const FRONT: u8 = 1;
const BACK: u8 = 2;
const SPANNING: u8 = 3;

impl Plane {
    fn split(&self, polygon: Polygon, out: &mut impl FnMut(Where, Polygon)) {
        let mut polygon_type = 0u8;
        let mut types: Vec<u8> = Vec::with_capacity(polygon.vertices.len());
        for v in &polygon.vertices {
            let t = self.normal.dot(v.pos) - self.w;
            let ty = if t < -EPSILON {
                BACK
            } else if t > EPSILON {
                FRONT
            } else {
                COPLANAR
            };
            polygon_type |= ty;
            types.push(ty);
        }
        match polygon_type {
            COPLANAR => {
                if self.normal.dot(polygon.plane.normal) > 0.0 {
                    out(Where::CoplanarFront, polygon)
                } else {
                    out(Where::CoplanarBack, polygon)
                }
            }
            FRONT => out(Where::Front, polygon),
            BACK => out(Where::Back, polygon),
            _ => {
                let (mut f, mut b): (Vec<Vertex>, Vec<Vertex>) = (vec![], vec![]);
                let n = polygon.vertices.len();
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (ti, tj) = (types[i], types[j]);
                    let (vi, vj) = (polygon.vertices[i], polygon.vertices[j]);
                    if ti != BACK {
                        f.push(vi);
                    }
                    if ti != FRONT {
                        b.push(vi);
                    }
                    if (ti | tj) == SPANNING {
                        let t = (self.w - self.normal.dot(vi.pos))
                            / self.normal.dot(vj.pos.sub(vi.pos));
                        let v = vi.interpolate(&vj, t);
                        f.push(v);
                        b.push(v);
                    }
                }
                if f.len() >= 3 {
                    out(Where::Front, Polygon::new(f));
                }
                if b.len() >= 3 {
                    out(Where::Back, Polygon::new(b));
                }
            }
        }
    }
}

#[derive(Default)]
struct Node {
    plane: Option<Plane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polygons: Vec<Polygon>,
}

impl Node {
    fn from_polygons(polygons: Vec<Polygon>) -> Node {
        let mut n = Node::default();
        n.build(polygons);
        n
    }

    fn invert(&mut self) {
        for p in &mut self.polygons {
            p.flip();
        }
        if let Some(pl) = &mut self.plane {
            pl.normal = pl.normal.scale(-1.0);
            pl.w = -pl.w;
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    fn clip_polygons(&self, polygons: Vec<Polygon>) -> Vec<Polygon> {
        let Some(plane) = &self.plane else {
            return polygons;
        };
        let (mut front, mut back): (Vec<Polygon>, Vec<Polygon>) = (vec![], vec![]);
        for p in polygons {
            plane.split(p, &mut |w, poly| match w {
                Where::CoplanarFront | Where::Front => front.push(poly),
                Where::CoplanarBack | Where::Back => back.push(poly),
            });
        }
        if let Some(f) = &self.front {
            front = f.clip_polygons(front);
        }
        back = match &self.back {
            Some(b) => b.clip_polygons(back),
            None => vec![],
        };
        front.extend(back);
        front
    }

    fn clip_to(&mut self, bsp: &Node) {
        self.polygons = bsp.clip_polygons(std::mem::take(&mut self.polygons));
        if let Some(f) = &mut self.front {
            f.clip_to(bsp);
        }
        if let Some(b) = &mut self.back {
            b.clip_to(bsp);
        }
    }

    fn all_polygons(&self) -> Vec<Polygon> {
        let mut v = self.polygons.clone();
        if let Some(f) = &self.front {
            v.extend(f.all_polygons());
        }
        if let Some(b) = &self.back {
            v.extend(b.all_polygons());
        }
        v
    }

    fn build(&mut self, polygons: Vec<Polygon>) {
        if polygons.is_empty() {
            return;
        }
        if self.plane.is_none() {
            self.plane = Some(polygons[0].plane);
        }
        let plane = self.plane.expect("plane set");
        let (mut front, mut back): (Vec<Polygon>, Vec<Polygon>) = (vec![], vec![]);
        for p in polygons {
            plane.split(p, &mut |w, poly| match w {
                Where::CoplanarFront | Where::CoplanarBack => self.polygons.push(poly),
                Where::Front => front.push(poly),
                Where::Back => back.push(poly),
            });
        }
        if !front.is_empty() {
            self.front.get_or_insert_with(Default::default).build(front);
        }
        if !back.is_empty() {
            self.back.get_or_insert_with(Default::default).build(back);
        }
    }
}

fn to_csg(m: &Mesh) -> Vec<Polygon> {
    let mut polys = vec![];
    for f in &m.faces {
        let ps = [
            m.vertices[f[0] as usize],
            m.vertices[f[1] as usize],
            m.vertices[f[2] as usize],
        ];
        let n = ps[1].sub(ps[0]).cross(ps[2].sub(ps[0]));
        if n.len() < 1e-12 {
            continue; // skip degenerate triangles
        }
        let normal = n.scale(1.0 / n.len());
        polys.push(Polygon::new(
            ps.iter().map(|p| Vertex { pos: *p, normal }).collect(),
        ));
    }
    polys
}

fn from_csg(polys: &[Polygon]) -> Mesh {
    let mut vertices: Vec<V3> = vec![];
    let mut faces: Vec<[u32; 3]> = vec![];
    let mut lookup: HashMap<(i64, i64, i64), u32> = HashMap::new();
    let mut index = |p: V3| -> u32 {
        let key = (
            (p.x * 1e7).round() as i64,
            (p.y * 1e7).round() as i64,
            (p.z * 1e7).round() as i64,
        );
        *lookup.entry(key).or_insert_with(|| {
            let r = |x: f64| (x * 1e9).round() / 1e9;
            vertices.push(V3::new(r(p.x), r(p.y), r(p.z)));
            (vertices.len() - 1) as u32
        })
    };
    for poly in polys {
        let ids: Vec<u32> = poly.vertices.iter().map(|v| index(v.pos)).collect();
        for i in 1..ids.len().saturating_sub(1) {
            if ids[0] != ids[i] && ids[i] != ids[i + 1] && ids[0] != ids[i + 1] {
                faces.push([ids[0], ids[i], ids[i + 1]]);
            }
        }
    }
    Mesh { vertices, faces }
}

/// Boolean of two meshes. The result may be empty.
pub fn boolean_sync(a: &Mesh, b: &Mesh, op: BoolOp) -> Result<Mesh, String> {
    if a.faces.len() + b.faces.len() > MAX_TRIANGLES {
        return Err(format!(
            "The solids have more than {MAX_TRIANGLES} triangles."
        ));
    }
    let mut na = Node::from_polygons(to_csg(a));
    let mut nb = Node::from_polygons(to_csg(b));
    let polys = match op {
        BoolOp::Union => {
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            na.build(nb.all_polygons());
            na.all_polygons()
        }
        BoolOp::Subtract => {
            na.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            na.build(nb.all_polygons());
            na.invert();
            na.all_polygons()
        }
        BoolOp::Intersect => {
            na.invert();
            nb.clip_to(&na);
            nb.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            na.build(nb.all_polygons());
            na.invert();
            na.all_polygons()
        }
    };
    Ok(from_csg(&polys))
}
