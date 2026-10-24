//! XCLIP: crop the display of block references and external references to a boundary.
//!
//! The boundary is a convex polygon in world coordinates (a rectangle, or a polygon drawn or picked that turns out
//! convex). It cuts the display and the plot; snapping and selection still see the whole reference.
use super::{db, ps};
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

/// True when every turn of the closed polygon has the same direction (collinear points are tolerated).
fn is_convex(p: &[Pt]) -> bool {
    let n = p.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0.0f64;
    for i in 0..n {
        let (a, b, c) = (p[i], p[(i + 1) % n], p[(i + 2) % n]);
        let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        if cross.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    sign != 0.0
}

fn edit(c: &Ctx, ids: &[u64], f: impl Fn(&mut InsertData)) {
    c.doc_mut(|d| {
        d.change(|d| {
            for id in ids {
                if let Some(Entity { kind: Kind::Insert(i), .. }) = d.by_id_mut(*id) {
                    f(i);
                }
            }
        })
    });
    c.emit();
}

async fn xclip(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects").filter(|e| matches!(e.kind, Kind::Insert(_)))).await?;
    let ids: Vec<u64> = sel.ents.iter().copied().filter(|i| matches!(c.entity_by_id(*i).map(|e| e.kind), Some(Kind::Insert(_)))).collect();
    if ids.is_empty() {
        c.log_error("Select block references or external references.");
        return Ok(());
    }
    let r = c.keyword(ps("Enter clipping option [ON/OFF/Delete/generate Polyline/New boundary]").options(&["ON", "OFF", "Delete", "generate Polyline", "New boundary"]).default_str("New boundary")).await?;
    match r.kw().unwrap_or("New boundary") {
        "ON" => edit(&c, &ids, |i| i.clip_off = false),
        "OFF" => edit(&c, &ids, |i| i.clip_off = true),
        "Delete" => edit(&c, &ids, |i| {
            i.clip.clear();
            i.clip_off = false;
        }),
        "generate Polyline" => {
            let Some(Entity { kind: Kind::Insert(i), layer, .. }) = c.entity_by_id(ids[0]) else { return Ok(()) };
            if i.clip.len() < 3 {
                c.log_error("The reference has no clipping boundary.");
                return Ok(());
            }
            let mut e = Entity::pline_pts(&i.clip, true);
            e.layer = layer;
            c.add(e);
        }
        _ => {
            let k = c.keyword(ps("Specify clipping boundary [Select polyline/Polygonal/Rectangular]").options(&["Select polyline", "Polygonal", "Rectangular"]).default_str("Rectangular")).await?;
            let poly: Vec<Pt> = match k.kw().unwrap_or("Rectangular") {
                "Select polyline" => {
                    let Some(id) = c.pick_entity(ps("Select a closed polyline").filter(|e| matches!(&e.kind, Kind::Pline { closed: true, .. }))).await? else { return Ok(()) };
                    match c.entity_by_id(id).map(|e| e.kind) {
                        Some(Kind::Pline { v, .. }) => {
                            if v.iter().any(|q| q.b != 0.0) {
                                c.log_error("The polyline has arc segments; a clipping boundary needs straight sides.");
                                return Ok(());
                            }
                            v.iter().map(|q| pt(q.x, q.y)).collect()
                        }
                        _ => return Ok(()),
                    }
                }
                "Polygonal" => {
                    let Some(first) = c.point(ps("Specify first point")).await?.pt() else { return Ok(()) };
                    let mut pts = vec![first];
                    loop {
                        let r = c.point(ps("Specify next point or [Undo]").options(&["Undo"]).base(*pts.last().expect("a point")).allow_enter()).await?;
                        match (r.kw(), r.pt()) {
                            (Some("Undo"), _) => {
                                if pts.len() > 1 {
                                    pts.pop();
                                }
                            }
                            (_, Some(p)) => pts.push(p),
                            _ => break,
                        }
                    }
                    pts
                }
                _ => {
                    let Some(a) = c.point(ps("Specify first corner point")).await?.pt() else { return Ok(()) };
                    let Some(b) = c.corner(a, ps("Specify opposite corner point")).await?.pt() else { return Ok(()) };
                    vec![pt(a.x.min(b.x), a.y.min(b.y)), pt(a.x.max(b.x), a.y.min(b.y)), pt(a.x.max(b.x), a.y.max(b.y)), pt(a.x.min(b.x), a.y.max(b.y))]
                }
            };
            if !is_convex(&poly) {
                c.log_error("The clipping boundary must be convex in this version (a rectangle or a polygon without dents).");
                return Ok(());
            }
            edit(&c, &ids, move |i| {
                i.clip = poly.clone();
                i.clip_off = false;
            });
        }
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(db("XCLIP", run(xclip)).aliases(&["XC"]).title("Clip Reference").icon("imageclip").desc("Crops the display of a selected external reference or block reference to a specified boundary."));
}
