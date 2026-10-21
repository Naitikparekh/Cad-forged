//! ARRAYPATH.
use super::{dm, point_at, ps};
use crate::commands::helpers::*;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

/// Segments of a path curve and whether it is closed.
fn path_segs(e: &Entity, doc: &Drawing) -> Option<(Vec<Seg>, bool)> {
    let closed = match &e.kind {
        Kind::Line { .. } | Kind::Arc { .. } => false,
        Kind::Circle { .. } => true,
        Kind::Pline { closed, .. } | Kind::Spline { closed, .. } => *closed,
        Kind::Ellipse(el) => ellipse_is_full(el),
        _ => return None,
    };
    Some((segs_of(e, doc).to_vec(), closed))
}

fn is_path(e: &Entity) -> bool {
    matches!(e.kind, Kind::Line { .. } | Kind::Arc { .. } | Kind::Circle { .. } | Kind::Pline { .. } | Kind::Spline { .. } | Kind::Ellipse(_))
}

/// Distributes copies of the selected objects along a path curve, either dividing the path into a number of items
/// or measuring a distance between them. With alignment each item turns with the path, the first one keeping
/// its drawn orientation. The objects themselves become the first item (their base point moves onto the path start).
async fn arraypath(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects").solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let (ents, solids) = (c.entities(&sel.ents), c.solids(&sel.solids));
    let Some(path_id) = c.pick_entity(ps("Select path curve").filter(is_path)).await? else { return Ok(()) };
    let Some((segs, closed)) = c.entity_by_id(path_id).and_then(|e| c.doc(|d| path_segs(&e, d))) else { return Ok(()) };
    let total: f64 = segs.iter().map(|s| s.length()).sum();
    if total < 1e-9 {
        c.log_error("The path curve has no length.");
        return Ok(());
    }
    let bounds = c.doc(|d| ents.iter().fold(None, |b, e| Bounds::union_opt(b, bounds_of(e, d))));
    let mut base = bounds.map_or(pt(0.0, 0.0), |b| b.center());
    let divide = loop {
        let r = c.keyword(ps("Enter path method").options(&["Divide", "Measure", "Base point"]).default_str("Measure")).await?;
        match r.kw() {
            Some("Base point") => {
                if let Some(p) = c.point(ps("Specify base point").default_pt(base)).await?.pt() {
                    base = p;
                }
            }
            Some("Divide") => break true,
            _ => break false,
        }
    };
    let at: Vec<f64> = if divide {
        let Some(n) = c.number(ps("Enter number of items along path").default_num(6.0).integer().min(2.0)).await?.value() else { return Ok(()) };
        let step = total / if closed { n } else { n - 1.0 };
        (0..n as usize).map(|i| step * i as f64).collect()
    } else {
        let width = bounds.map_or(1.0, |b| b.width()).max(1e-6);
        let Some(step) = c.distance(ps("Specify the distance between items along path").default_num(width).min(0.0).nonzero()).await?.value() else { return Ok(()) };
        let limit = if closed { total - 1e-9 } else { total + 1e-9 };
        (0u32..).map(|i| step * i as f64).take_while(|s| *s <= limit).collect()
    };
    if at.len() * (ents.len() + solids.len()) > 100_000 {
        c.log_error("The array would contain too many objects.");
        return Ok(());
    }
    let align = c.keyword(ps("Align items to match the orientation of the path").options(&["Yes", "No"]).default_str("Yes")).await?.kw() != Some("No");
    let turn0 = angle_of(Pt::ZERO, point_at(&segs, 0.0).1);
    let place = |s: f64| {
        let (p, t) = point_at(&segs, s);
        let rot = if align { angle_of(Pt::ZERO, t) - turn0 } else { 0.0 };
        mat::mul(&mat::rotate(rot, p), &mat::translate(p.x - base.x, p.y - base.y))
    };
    c.doc_mut(|d| {
        d.change(|d| {
            for s in &at[1..] {
                add_copies(d, &ents, &solids, &place(*s));
            }
            apply_transform(d, &sel.ents, &sel.solids, &place(at[0]));
        })
    });
    c.clear_sel();
    c.log(&format!("{} items in the path array.", at.len()));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(dm("ARRAYPATH", run(arraypath)).title("Path Array").icon("arraypath").desc("Evenly distributes copies of the selected object along a path or a portion of a path."));
}
