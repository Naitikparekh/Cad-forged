//! REVERSE (the direction of lines, polylines and splines), TEXTTOFRONT and HATCHTOBACK (draw order by object type).
use super::{plural, ps};
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

// ---------------------------------------------------------------- REVERSE

/// The polyline through the same vertices in the opposite order: each edge keeps its arc (its bulge changes sign) and
/// its taper (the start and end widths swap).
pub fn reverse_vertices(v: &[Vertex], closed: bool) -> Vec<Vertex> {
    let n = v.len();
    (0..n)
        .map(|j| {
            let p = v[n - 1 - j];
            // the edge that now starts at vertex j was the edge ending there
            let k = if j == n - 1 { n - 1 } else { n - 2 - j };
            let e = v[k];
            let last_open = j == n - 1 && !closed;
            Vertex { x: p.x, y: p.y, b: if last_open { 0.0 } else { -e.b }, sw: if last_open { 0.0 } else { e.ew }, ew: if last_open { 0.0 } else { e.sw } }
        })
        .collect()
}

/// The same object running the other way; `None` for objects REVERSE does not change.
pub fn reversed(e: &Entity) -> Option<Kind> {
    Some(match &e.kind {
        Kind::Line { a, b } => Kind::Line { a: *b, b: *a },
        Kind::Pline { v, closed } if v.len() >= 2 => Kind::Pline { v: reverse_vertices(v, *closed), closed: *closed },
        Kind::Spline { fit, closed, nurbs } => {
            Kind::Spline { fit: fit.iter().rev().copied().collect(), closed: *closed, nurbs: nurbs.as_ref().map(|n| Box::new(n.reversed())) }
        }
        _ => return None,
    })
}

/// REVERSE: reverse the vertex order of lines, polylines and splines (a linetype with text or shapes then reads the
/// other way).
async fn reverse(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select lines, polylines or splines to reverse").filter(|e| reversed(e).is_some())).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let ids = sel.ents.clone();
    let n = c.doc_mut(|d| {
        d.change(|d| {
            let mut n = 0;
            for id in ids {
                if let Some(e) = d.by_id_mut(id) {
                    if let Some(k) = reversed(e) {
                        e.kind = k;
                        n += 1;
                    }
                }
            }
            n
        })
    });
    c.log(&format!("{} reversed.", plural(n, "object")));
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- TEXTTOFRONT / HATCHTOBACK

fn is_text(e: &Entity) -> bool {
    matches!(e.kind, Kind::Text(_) | Kind::Table(_))
}

fn is_dim(e: &Entity) -> bool {
    matches!(e.kind, Kind::Dim(_))
}

fn is_leader(e: &Entity) -> bool {
    matches!(e.kind, Kind::Leader { .. })
}

/// Bring the objects of the current space that `pick` accepts to the front (or send them to the back); returns how
/// many moved.
fn order_by(c: &Ctx, pick: impl Fn(&Entity) -> bool, front: bool) -> usize {
    let ids: Vec<u64> = c.doc(|d| d.entities().iter().filter(|e| pick(e)).map(|e| e.id).collect());
    if !ids.is_empty() {
        c.doc_mut(|d| d.reorder(&ids, front));
    }
    ids.len()
}

/// TEXTTOFRONT: bring text, dimensions and leaders in front of the other objects, so hatches and images do not hide
/// them.
async fn texttofront(c: Ctx, a: Args) -> CmdResult<()> {
    let which = match a.str("which") {
        Some(w) => w.to_string(),
        None => c.keyword(ps("Bring to front").options(&["Text", "Dimensions", "Leaders", "All"]).default_str("All")).await?.kw().unwrap_or("All").to_string(),
    };
    let n = match which.as_str() {
        "Text" => order_by(&c, is_text, true),
        "Dimensions" => order_by(&c, is_dim, true),
        "Leaders" => order_by(&c, is_leader, true),
        _ => order_by(&c, |e| is_text(e) || is_dim(e) || is_leader(e), true),
    };
    c.log(&format!("{} brought to front.", plural(n, "object")));
    Ok(())
}

/// HATCHTOBACK: send every hatch and fill behind the other objects.
async fn hatchtoback(c: Ctx, _a: Args) -> CmdResult<()> {
    let n = order_by(&c, |e| matches!(e.kind, Kind::Hatch(_)), false);
    c.log(&format!("{} sent to back.", plural(n, "hatch object")));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Modify", "REVERSE", run(reverse)).title("Reverse").icon("pline").desc("Reverses the direction of lines, polylines and splines."));
    r.define(def("Modify", "TEXTTOFRONT", run(texttofront)).title("Bring Text and Dimensions to Front").icon("draworder").desc("Brings text, dimensions and leaders in front of all other objects."));
    r.define(def("Modify", "HATCHTOBACK", run(hatchtoback)).title("Send Hatches to Back").icon("draworder").desc("Sends every hatch and fill behind all other objects."));
}
