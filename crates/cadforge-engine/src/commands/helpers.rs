//! Shared helpers for command implementations (port of `src/commands/helpers.js`).
use crate::runtime::PreviewItem;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::{segs_of, transform_entity, Entity, Kind};
use cadforge_core::geom::*;
use cadforge_core::solid::mesh::transform_solid;
use cadforge_core::solid::Solid;
use std::collections::HashMap;

pub fn pv(e: Entity) -> PreviewItem {
    PreviewItem::Entity(e)
}

pub fn pvs(v: Vec<Entity>) -> Vec<PreviewItem> {
    v.into_iter().map(PreviewItem::Entity).collect()
}

/// Clones of `entities` with `matrix` applied (new objects, no ids) – for previews and COPY.
pub fn transformed_clones(entities: &[Entity], m: &Mat) -> Vec<Entity> {
    entities
        .iter()
        .map(|e| {
            let mut c = e.cloned_noid();
            transform_entity(&mut c, m);
            c
        })
        .collect()
}

/// Apply a matrix to existing entities and solids in place (inside a change notification).
pub fn apply_transform(doc: &mut Drawing, ids: &[u64], solid_ids: &[u64], m: &Mat) {
    doc.change(|d| {
        for id in ids {
            if let Some(e) = d.by_id_mut(*id) {
                transform_entity(e, m);
            }
        }
        for sid in solid_ids {
            if let Some(s) = d.solid_by_id_mut(*sid) {
                let t = transform_solid(s, m);
                s.vertices = t.vertices;
                s.faces = t.faces;
            }
        }
    });
}

/// Add transformed copies to the drawing, remapping group ids so copies form their own groups.
/// Returns the new entity ids and solid ids.
pub fn add_copies(
    doc: &mut Drawing,
    entities: &[Entity],
    solids: &[Solid],
    m: &Mat,
) -> (Vec<u64>, Vec<u64>) {
    let mut group_map: HashMap<String, String> = HashMap::new();
    let mut out = vec![];
    for e in entities {
        let mut c = e.cloned_noid();
        transform_entity(&mut c, m);
        if let Some(g) = c.grp.clone() {
            let n = group_map.len();
            let ng = group_map
                .entry(g)
                .or_insert_with(|| format!("G{}_{}", doc.next_id, n))
                .clone();
            c.grp = Some(ng);
        }
        out.push(doc.add(c));
    }
    let mut sout = vec![];
    for s in solids {
        let mut copy = transform_solid(s, m);
        copy.id = 0;
        sout.push(doc.add_solid(copy));
    }
    (out, sout)
}

pub fn pt_str(p: Pt, prec: usize) -> String {
    format!("{},{}", num(p.x, prec), num(p.y, prec))
}

/// Number with at most `prec` decimals (no trailing zeros), like JS `+x.toFixed(prec)`.
pub fn num(v: f64, prec: usize) -> String {
    let s = format!("{:.*}", prec, v);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

pub fn deg_text(r: f64, prec: usize) -> String {
    num(deg(r), prec)
}

/// Bulge of an arc from p0 to p1 given the tangent direction at p0.
pub fn tangent_bulge(p0: Pt, dir: Pt, p1: Pt) -> f64 {
    let chord = angle_of(p0, p1);
    let theta = norm_angle_signed(chord - dir.y.atan2(dir.x));
    (theta / 2.0).tan()
}

/// Closed rectangle outline (CCW) from two corners, optionally rotated about the first corner.
pub fn rect_verts(a: Pt, b: Pt, rot: f64) -> Vec<Vertex> {
    let u = pt(rot.cos(), rot.sin());
    let v = pt(-u.y, u.x);
    let d = b - a;
    let (w, h) = (dot(d, u), dot(d, v));
    [a, a + u * w, b, a + v * h]
        .iter()
        .map(|p| Vertex::new(p.x, p.y, 0.0))
        .collect()
}

/// Rubber-band preview of a polyline under construction.
pub fn pline_preview(verts: &[Vertex], tail: Pt) -> Vec<PreviewItem> {
    if verts.is_empty() {
        return vec![];
    }
    let mut v = verts.to_vec();
    v.push(Vertex::new(tail.x, tail.y, 0.0));
    vec![pv(Entity::pline(v, false))]
}

pub fn is_closed_curve(e: &Entity) -> bool {
    match &e.kind {
        Kind::Circle { .. } | Kind::Ellipse(_) => true,
        Kind::Pline { closed, .. } | Kind::Spline { closed, .. } => *closed,
        _ => false,
    }
}

/// Segments of a drawing object, for tangency etc.
pub fn segs_for(e: &Entity, doc: &Drawing) -> Vec<Seg> {
    segs_of(e, doc).to_vec()
}
