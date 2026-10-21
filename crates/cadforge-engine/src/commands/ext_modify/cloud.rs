//! REVCLOUD: revision clouds drawn as polylines of arcs.
use super::{dd, point_at, pref_num, ps, set_pref};
use crate::commands::helpers::*;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use crate::sysvars::Value;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::path::inherit_props;
use cadforge_core::style::fmt_num;

/// Included angle of every cloud arc.
const SWEEP: f64 = TAU / 3.0;
/// Upper bound on the arcs of one path piece (a tiny arc length must not exhaust memory).
const MAX_ARCS: usize = 100_000;
const OPTIONS: [&str; 7] = ["Arc length", "Object", "Rectangular", "Polygonal", "Freehand", "Style", "Modify"];

/// The arc length the cloud is built with: AutoCAD varies arcs between the minimum and maximum, this uses the mean.
fn arc_length(c: &Ctx) -> (f64, f64) {
    let min = pref_num(c, "REVCLOUDMINARCLENGTH", 0.5).max(1e-6);
    (min, pref_num(c, "REVCLOUDMAXARCLENGTH", min).max(min))
}

/// Points evenly spaced by arc length along a path made of tessellated pieces.
fn resample(path: &[Seg], n: usize, closed: bool) -> Vec<Pt> {
    let total: f64 = path.iter().map(|s| s.length()).sum();
    (0..if closed { n } else { n + 1 }).map(|i| point_at(path, total * i as f64 / n as f64).0).collect()
}

/// A cloud along `path`: every piece is divided into arcs about `arc_len` long. Closed paths bulge outwards.
fn cloud(path: &[Seg], closed: bool, arc_len: f64, reverse: bool) -> Vec<Vertex> {
    let bulge = |ccw: bool| (if ccw != reverse { 1.0 } else { -1.0 }) * (SWEEP / 4.0).tan();
    let ccw = !closed || polygon_area(&tess_segs(path, arc_len * 0.1)) >= 0.0;
    if path.iter().any(|s| s.is_approx()) {
        // a curve stored as a fine polyline (ellipse, spline) is divided as a whole
        let total: f64 = path.iter().map(|s| s.length()).sum();
        let n = ((total / arc_len).round() as usize).clamp(if closed { 3 } else { 1 }, MAX_ARCS);
        let pts = resample(path, n, closed);
        let last = pts.len() - 1;
        return pts.iter().enumerate().map(|(i, p)| Vertex::new(p.x, p.y, if closed || i < last { bulge(ccw) } else { 0.0 })).collect();
    }
    let mut v: Vec<Vertex> = vec![];
    for s in path {
        let n = ((s.length() / arc_len).round() as usize).clamp(1, MAX_ARCS);
        v.extend((0..n).map(|k| {
            let p = s.at(k as f64 / n as f64);
            Vertex::new(p.x, p.y, bulge(ccw))
        }));
    }
    if !closed {
        let e = path[path.len() - 1].end();
        v.push(Vertex::new(e.x, e.y, 0.0));
    }
    v
}

fn make_cloud(c: &Ctx, path: &[Seg], closed: bool, reverse: bool, proto: Option<&Entity>) {
    let (min, max) = arc_length(c);
    let e = Entity::pline(cloud(path, closed, (min + max) / 2.0, reverse), closed);
    c.add(match proto {
        Some(p) => inherit_props(p, e),
        None => e,
    });
    c.log("Revision cloud finished.");
}

fn path_of_points(pts: &[Pt], closed: bool) -> Vec<Seg> {
    pline_segs(&pts.iter().map(|p| Vertex::from(*p)).collect::<Vec<_>>(), closed)
}

/// Collect points until Enter; returns them in order.
async fn trace(c: &Ctx, first: Pt) -> CmdResult<Vec<Pt>> {
    let mut pts = vec![first];
    loop {
        let snapshot = pts.clone();
        let last = pts[pts.len() - 1];
        let r = c
            .point(ps("Specify next point").options(&["Undo"]).base(last).allow_enter().preview(move |_, p| {
                let mut v = snapshot.clone();
                v.push(p);
                vec![pv(Entity::pline_pts(&v, false))]
            }))
            .await?;
        match r.kw() {
            Some("Undo") => {
                if pts.len() > 1 {
                    pts.pop();
                }
            }
            _ => match r.pt() {
                Some(p) => pts.push(p),
                None => return Ok(pts),
            },
        }
    }
}

async fn start_point(c: &Ctx, prompt: &str, given: Option<Pt>) -> CmdResult<Option<Pt>> {
    match given {
        Some(p) => Ok(Some(p)),
        None => Ok(c.point(ps(prompt)).await?.pt()),
    }
}

async fn polygonal(c: &Ctx, given: Option<Pt>) -> CmdResult<()> {
    let Some(first) = start_point(c, "Specify start point", given).await? else { return Ok(()) };
    let pts = trace(c, first).await?;
    if pts.len() < 3 {
        c.log_error("A polygonal revision cloud needs at least three points.");
        return Ok(());
    }
    make_cloud(c, &path_of_points(&pts, true), true, false, None);
    Ok(())
}

/// Freehand: the cloud follows the picked points and closes when the last one is back at the start.
async fn freehand(c: &Ctx, given: Option<Pt>) -> CmdResult<()> {
    let Some(first) = start_point(c, "Specify start point", given).await? else { return Ok(()) };
    let mut pts = trace(c, first).await?;
    if pts.len() < 2 {
        c.log_error("Pick at least two points for a freehand revision cloud.");
        return Ok(());
    }
    let (min, max) = arc_length(c);
    let closed = pts.len() > 2 && dist(pts[0], pts[pts.len() - 1]) <= (min + max) / 2.0 * 2.0;
    if closed {
        pts.pop();
    }
    make_cloud(c, &path_of_points(&pts, closed), closed, false, None);
    Ok(())
}

async fn rectangular(c: &Ctx) -> CmdResult<()> {
    let Some(a) = c.point(ps("Specify first corner point")).await?.pt() else { return Ok(()) };
    let Some(b) = c.corner(a, ps("Specify opposite corner point")).await?.pt() else { return Ok(()) };
    if (a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9 {
        c.log_error("The corners must span an area.");
        return Ok(());
    }
    make_cloud(c, &pline_segs(&rect_verts(a, b, 0.0), true), true, false, None);
    Ok(())
}

async fn from_object(c: &Ctx) -> CmdResult<()> {
    let sel = c.entity(ps("Select object").filter(|e| matches!(e.kind, Kind::Circle { .. } | Kind::Arc { .. } | Kind::Pline { .. } | Kind::Ellipse(_) | Kind::Spline { .. }))).await?;
    let Some(id) = sel.id() else { return Ok(()) };
    let Some(e) = c.entity_by_id(id) else { return Ok(()) };
    let reverse = c.keyword(ps("Reverse direction?").options(&["Yes", "No"]).default_str("No")).await?.kw() == Some("Yes");
    let path = c.doc(|d| segs_of(&e, d).to_vec());
    if path.is_empty() {
        c.log_error("The selected object has no length.");
        return Ok(());
    }
    let closed = match &e.kind {
        Kind::Circle { .. } => true,
        Kind::Pline { closed, .. } | Kind::Spline { closed, .. } => *closed,
        Kind::Ellipse(el) => ellipse_is_full(el),
        _ => false,
    };
    let (min, max) = arc_length(c);
    let verts = cloud(&path, closed, (min + max) / 2.0, reverse);
    let proto = e.clone();
    c.doc_mut(|d| {
        d.change(|d| {
            d.remove(id);
            d.add(inherit_props(&proto, Entity::pline(verts, closed)));
        })
    });
    c.log("Revision cloud finished.");
    Ok(())
}

/// Modify only reverses the arc direction; adding or removing parts of a cloud is not supported.
async fn modify(c: &Ctx) -> CmdResult<()> {
    let Some(id) = c.pick_entity(ps("Select revision cloud to modify").filter(|e| matches!(&e.kind, Kind::Pline { v, .. } if v.iter().any(|x| x.b != 0.0)))).await? else { return Ok(()) };
    let reverse = c.keyword(ps("Reverse direction?").options(&["Yes", "No"]).default_str("No")).await?.kw() == Some("Yes");
    if reverse {
        c.doc_mut(|d| {
            d.modify(id, |e| {
                if let Kind::Pline { v, .. } = &mut e.kind {
                    v.iter_mut().for_each(|x| x.b = -x.b);
                }
            })
        });
    }
    c.log("Revision cloud finished.");
    Ok(())
}

async fn set_arc_length(c: &Ctx) -> CmdResult<()> {
    let (cur_min, cur_max) = arc_length(c);
    let Some(min) = c.distance(ps("Specify minimum length of arc").default_num(cur_min).min(0.0).nonzero()).await?.value() else { return Ok(()) };
    let Some(mut max) = c.distance(ps("Specify maximum length of arc").default_num(cur_max.max(min)).min(0.0).nonzero()).await?.value() else { return Ok(()) };
    if max < min {
        max = min;
    } else if max > 3.0 * min {
        max = 3.0 * min;
        c.log_error(&format!("The maximum arc length cannot exceed three times the minimum; using {}.", fmt_num(max, 4, true)));
    }
    set_pref(c, "REVCLOUDMINARCLENGTH", Value::Num(min));
    set_pref(c, "REVCLOUDMAXARCLENGTH", Value::Num(max));
    Ok(())
}

async fn revcloud(c: Ctx, _a: Args) -> CmdResult<()> {
    loop {
        let (min, max) = arc_length(&c);
        c.log(&format!("Minimum arc length: {}  Maximum arc length: {}  Style: Normal", fmt_num(min, 4, true), fmt_num(max, 4, true)));
        let r = c.point(ps("Specify start point or").options(&OPTIONS).default_str("Object").allow_enter().no_ortho()).await?;
        match r {
            PointR::Kw(k) => match k.as_str() {
                "Arc length" => set_arc_length(&c).await?,
                "Rectangular" => return rectangular(&c).await,
                "Polygonal" => return polygonal(&c, None).await,
                "Freehand" => return freehand(&c, None).await,
                "Style" => {
                    let s = c.keyword(ps("Select arc style").options(&["Normal", "Calligraphy"]).default_str("Normal")).await?;
                    if s.kw() == Some("Calligraphy") {
                        c.log_error("Calligraphy needs polyline widths, which drawings here do not store; the Normal style is used.");
                    }
                }
                "Modify" => return modify(&c).await,
                _ => return from_object(&c).await,
            },
            PointR::Pt(p) => return freehand(&c, Some(p)).await,
            _ => return from_object(&c).await,
        }
    }
}

pub fn register(r: &mut Registry) {
    r.define(dd("REVCLOUD", run(revcloud)).title("Revision Cloud").icon("spline").desc("Creates a polyline of sequential arcs to form a cloud-shaped object. Modify only reverses the arc direction; the Calligraphy style is not available."));
}
