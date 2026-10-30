//! CENTERMARK and CENTERLINE: center marks on circles and arcs, and the centerline between two lines, drawn in the
//! CENTER2 linetype (AutoCAD's CENTERLTYPE default) and grouped so each mark moves and erases as one object.
use super::{group_id, plural, ps};
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

const LTYPE: &str = "CENTER2";

fn styled(mut e: Entity, grp: &str) -> Entity {
    e.ltype = Some(LTYPE.into());
    e.grp = Some(grp.to_string());
    e
}

/// The two lines of a center mark: through the center, reaching a little past the circle (12% of the radius).
pub fn center_mark(c: Pt, r: f64) -> [(Pt, Pt); 2] {
    let reach = r * 1.12;
    [(pt(c.x - reach, c.y), pt(c.x + reach, c.y)), (pt(c.x, c.y - reach), pt(c.x, c.y + reach))]
}

/// The centerline of two lines: from the midpoint of their start points to the midpoint of their end points (the
/// second line taken in the first one's direction), extended 5% of its length at each end.
pub fn centerline(a: (Pt, Pt), b: (Pt, Pt)) -> Option<(Pt, Pt)> {
    let (da, db) = (a.1 - a.0, b.1 - b.0);
    let b = if da.x * db.x + da.y * db.y < 0.0 { (b.1, b.0) } else { b };
    let (s, e) = (pt((a.0.x + b.0.x) / 2.0, (a.0.y + b.0.y) / 2.0), pt((a.1.x + b.1.x) / 2.0, (a.1.y + b.1.y) / 2.0));
    let len = dist(s, e);
    if len < 1e-9 {
        return None;
    }
    let u = (e - s) * (1.0 / len);
    Some((s - u * (len * 0.05), e + u * (len * 0.05)))
}

async fn centermark(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select circles or arcs to add center marks").filter(|e| matches!(e.kind, Kind::Circle { .. } | Kind::Arc { .. }))).await?;
    let mut n = 0;
    for id in sel.ents {
        let Some((ctr, r)) = c.entity_by_id(id).and_then(|e| match e.kind {
            Kind::Circle { c, r } | Kind::Arc { c, r, .. } => Some((c, r)),
            _ => None,
        }) else {
            continue;
        };
        let g = group_id(&c);
        for (p, q) in center_mark(ctr, r) {
            c.add(styled(Entity::line(p, q), &g));
        }
        n += 1;
    }
    c.log(&format!("{} added.", plural(n, "center mark")));
    Ok(())
}

async fn centerline_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    let is_line = |e: &Entity| matches!(e.kind, Kind::Line { .. });
    let Some(first) = c.entity(ps("Select first line").filter(is_line)).await?.id() else { return Ok(()) };
    let Some(second) = c.entity(ps("Select second line").filter(move |e| is_line(e) && e.id != first)).await?.id() else { return Ok(()) };
    let seg = |id: u64| c.entity_by_id(id).and_then(|e| if let Kind::Line { a, b } = e.kind { Some((a, b)) } else { None });
    let (Some(a), Some(b)) = (seg(first), seg(second)) else { return Ok(()) };
    match centerline(a, b) {
        Some((p, q)) => {
            let g = group_id(&c);
            c.add(styled(Entity::line(p, q), &g));
            c.log("Centerline added.");
        }
        None => c.log_error("The lines have no length between them."),
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Annotation", "CENTERMARK", run(centermark)).aliases(&["CM"]).title("Center Mark").icon("dimcenter").desc("Adds center marks to selected circles and arcs."));
    r.define(def("Annotation", "CENTERLINE", run(centerline_cmd)).aliases(&["CL"]).title("Centerline").icon("centerline").desc("Draws the centerline between two selected lines."));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_and_centerlines() {
        let [(a, b), (p, q)] = center_mark(pt(10.0, 0.0), 5.0);
        assert!(dist(a, pt(4.4, 0.0)) < 1e-9 && dist(b, pt(15.6, 0.0)) < 1e-9 && dist(p, pt(10.0, -5.6)) < 1e-9 && dist(q, pt(10.0, 5.6)) < 1e-9);
        // two horizontal lines, the second drawn the other way
        let (s, e) = centerline((pt(0.0, 0.0), pt(10.0, 0.0)), (pt(10.0, 4.0), pt(0.0, 4.0))).unwrap();
        assert!(dist(s, pt(-0.5, 2.0)) < 1e-9 && dist(e, pt(10.5, 2.0)) < 1e-9, "{s:?} {e:?}");
        assert!(centerline((pt(0.0, 0.0), pt(0.0, 0.0)), (pt(0.0, 0.0), pt(0.0, 0.0))).is_none());
    }
}
