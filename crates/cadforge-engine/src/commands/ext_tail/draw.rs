//! TRACE and HATCHGENERATEBOUNDARY.
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

/// A solid-filled quadrilateral (what AutoCAD stores as a TRACE / SOLID).
fn solid_quad(ring: [Pt; 4]) -> Entity {
    let loops = vec![ring.iter().map(|p| Vertex::new(p.x, p.y, 0.0)).collect()];
    Entity::new(Kind::Hatch(HatchData { loops, pattern: "SOLID".into(), ps: 1.0, pa: 0.0, gradient: None }))
}

/// Left / right offsets of the point `i` of a centre line, mitred where two segments meet.
fn corners(pts: &[Pt], i: usize, half: f64) -> (Pt, Pt) {
    let dir = |a: Pt, b: Pt| {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let l = dx.hypot(dy);
        if l < 1e-12 {
            (1.0, 0.0)
        } else {
            (dx / l, dy / l)
        }
    };
    let n_of = |d: (f64, f64)| (-d.1, d.0);
    let n = match (i.checked_sub(1), pts.get(i + 1)) {
        (Some(p), Some(next)) => {
            let (n1, n2) = (n_of(dir(pts[p], pts[i])), n_of(dir(pts[i], *next)));
            let k = 1.0 + n1.0 * n2.0 + n1.1 * n2.1;
            if k < 1e-6 {
                n2
            } else {
                ((n1.0 + n2.0) / k, (n1.1 + n2.1) / k)
            }
        }
        (Some(p), None) => n_of(dir(pts[p], pts[i])),
        (None, Some(next)) => n_of(dir(pts[i], *next)),
        (None, None) => (0.0, 1.0),
    };
    (pt(pts[i].x + n.0 * half, pts[i].y + n.1 * half), pt(pts[i].x - n.0 * half, pts[i].y - n.1 * half))
}

async fn trace(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.pref("TRACEWID");
    let Some(w) = c.distance(ps("Specify trace width").default_num(cur).min(0.0)).await?.value() else { return Ok(()) };
    c.setvar("TRACEWID", w)?;
    let Some(first) = c.point(ps("Specify start point")).await?.pt() else { return Ok(()) };
    let mut pts = vec![first];
    loop {
        let base = *pts.last().expect("a start point");
        let w2 = w;
        let pts2 = pts.clone();
        c.set_preview(move |_, p| {
            let mut all = pts2.clone();
            all.push(p);
            let i = all.len() - 2;
            let (l0, r0) = corners(&all, i, w2 / 2.0);
            let (l1, r1) = corners(&all, i + 1, w2 / 2.0);
            vec![PreviewItem::Entity(solid_quad([l0, l1, r1, r0]))]
        });
        let r = c.point(ps("Specify next point").base(base).allow_enter()).await?;
        c.clear_preview();
        let Some(p) = r.pt() else { break };
        if dist(p, base) < 1e-9 {
            continue;
        }
        pts.push(p);
        // the segment before the new point is final now that its far corner is known
        if pts.len() >= 3 {
            let i = pts.len() - 3;
            let (l0, r0) = corners(&pts, i, w / 2.0);
            let (l1, r1) = corners(&pts, i + 1, w / 2.0);
            c.add(solid_quad([l0, l1, r1, r0]));
        }
    }
    if pts.len() >= 2 {
        let i = pts.len() - 2;
        let (l0, r0) = corners(&pts, i, w / 2.0);
        let (l1, r1) = corners(&pts, i + 1, w / 2.0);
        c.add(solid_quad([l0, l1, r1, r0]));
    }
    Ok(())
}

async fn hatchgenerateboundary(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(id) = c.pick_entity(ps("Select hatch object").filter(|e| matches!(e.kind, Kind::Hatch(_)))).await? else { return Ok(()) };
    let Some(src) = c.entity_by_id(id) else { return Ok(()) };
    let Kind::Hatch(h) = &src.kind else { return Ok(()) };
    let mut n = 0;
    for l in &h.loops {
        if l.len() < 2 {
            continue;
        }
        let mut e = Entity::new(Kind::Pline { v: l.clone(), closed: true });
        e.layer = src.layer.clone();
        e.color = src.color;
        e.ltype = src.ltype.clone();
        e.lw = src.lw;
        c.add(e);
        n += 1;
    }
    c.log(&format!("{n} polyline{} created from the hatch boundary.", if n == 1 { "" } else { "s" }));
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("Draw", "TRACE", run(trace)).title("Trace").icon("solid").desc("Creates solid lines of a given width."));
    r.define(def("Draw", "HATCHGENERATEBOUNDARY", run(hatchgenerateboundary)).title("Generate Hatch Boundary").icon("boundary").desc("Creates a polyline around each loop of a hatch."));
}
