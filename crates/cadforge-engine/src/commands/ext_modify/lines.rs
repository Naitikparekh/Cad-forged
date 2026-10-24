//! MLINE and SKETCH.
use super::{dd, pref_num, pref_text, ps, set_pref};
use crate::commands::helpers::*;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use crate::sysvars::Value;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::offset::offset_segs;
use cadforge_core::style::fmt_num;

// ---------------------------------------------------------------- MLINE

const JUSTIFICATIONS: [&str; 3] = ["Top", "Zero", "Bottom"];

/// The two parallel elements of the STANDARD multiline style, one polyline each. The justification line is the
/// path through the picked points; the other element lies `scale` away to its right (left when `scale` is negative).
fn mline_elements(pts: &[Pt], closed: bool, just: usize, scale: f64) -> Vec<Entity> {
    let path = pline_segs(
        &pts.iter().map(|p| Vertex::from(*p)).collect::<Vec<_>>(),
        closed,
    );
    let shift = [0.5, 0.0, -0.5][just];
    [0.5, -0.5]
        .iter()
        .filter_map(|e| offset_segs(&path, (e - shift) * scale, closed))
        .map(|segs| Entity::pline(segs_to_verts(&segs, closed), closed))
        .collect()
}

/// Draws multiline objects as a group of two parallel polylines (the STANDARD style: no caps, no MLSTYLE/MLEDIT).
async fn mline(c: Ctx, _a: Args) -> CmdResult<()> {
    let first = loop {
        let just = (pref_num(&c, "CMLJUST", 0.0) as usize).min(2);
        let scale = pref_num(&c, "CMLSCALE", 20.0);
        c.log(&format!(
            "Current settings: Justification = {}, Scale = {}, Style = {}",
            JUSTIFICATIONS[just],
            fmt_num(scale, 2, false),
            pref_text(&c, "CMLSTYLE", "STANDARD")
        ));
        let r = c
            .point(
                ps("Specify start point or")
                    .options(&["Justification", "Scale", "STyle"])
                    .no_ortho(),
            )
            .await?;
        match r.kw() {
            Some("Justification") => {
                let k = c
                    .keyword(
                        ps("Enter justification type")
                            .options(&JUSTIFICATIONS)
                            .default_str(JUSTIFICATIONS[just]),
                    )
                    .await?;
                if let Some(i) = JUSTIFICATIONS.iter().position(|j| Some(*j) == k.kw()) {
                    set_pref(&c, "CMLJUST", Value::Num(i as f64));
                }
            }
            Some("Scale") => {
                if let Some(v) = c
                    .number(ps("Enter mline scale").default_num(scale).nonzero())
                    .await?
                    .value()
                {
                    set_pref(&c, "CMLSCALE", Value::Num(v));
                }
            }
            Some("STyle") => {
                let n = c
                    .string(ps("Enter mline style name or [?]").default_str("STANDARD"))
                    .await?
                    .text;
                if n == "?" {
                    c.log("Available multiline styles: STANDARD");
                } else if !n.eq_ignore_ascii_case("STANDARD") {
                    c.log_error(&format!(
                        "Multiline style \"{n}\" is not defined; only STANDARD is available."
                    ));
                }
            }
            _ => match r.pt() {
                Some(p) => break p,
                None => return Ok(()),
            },
        }
    };
    let (just, scale) = (
        (pref_num(&c, "CMLJUST", 0.0) as usize).min(2),
        pref_num(&c, "CMLSCALE", 20.0),
    );
    let mut pts = vec![first];
    let mut closed = false;
    loop {
        let snapshot = pts.clone();
        let opts: &[&str] = if pts.len() > 2 {
            &["Close", "Undo"]
        } else {
            &["Undo"]
        };
        let last = pts[pts.len() - 1];
        let r = c
            .point(
                ps("Specify next point")
                    .options(opts)
                    .base(last)
                    .allow_enter()
                    .preview(move |_, p| {
                        let mut v = snapshot.clone();
                        v.push(p);
                        pvs(mline_elements(&v, false, just, scale))
                    }),
            )
            .await?;
        match r.kw() {
            Some("Undo") => {
                if pts.len() > 1 {
                    pts.pop();
                }
            }
            Some("Close") => {
                closed = true;
                break;
            }
            _ => match r.pt() {
                Some(p) if !same(p, last) => pts.push(p),
                Some(_) => {}
                None => break,
            },
        }
    }
    if pts.len() < 2 {
        return Ok(());
    }
    let serial = c.app(|a| {
        a.mem.group_serial += 1;
        a.mem.group_serial
    });
    let gid = format!("*A{serial}_{}", c.doc(|d| d.next_id));
    let parts = mline_elements(&pts, closed, just, scale);
    c.doc_mut(|d| {
        d.change(|d| {
            for mut e in parts {
                e.grp = Some(gid.clone());
                d.add(e);
            }
        })
    });
    c.log(&format!("Multiline created as group *A{serial}."));
    Ok(())
}

// ---------------------------------------------------------------- SKETCH

const SKETCH_TYPES: [&str; 3] = ["Line", "Polyline", "Spline"];

/// Records freehand sketches as consecutive picked points: a point is only recorded when it is at least the
/// increment away from the previous one. Enter finishes and creates lines, a polyline or a spline.
async fn sketch(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut pts: Vec<Pt> = vec![];
    loop {
        let inc = pref_num(&c, "SKETCHINC", 1.0);
        let kind = (pref_num(&c, "SKETCHTYPE", 1.0) as usize).min(2);
        let snapshot = pts.clone();
        let (msg, opts): (&str, &[&str]) = if pts.is_empty() {
            ("Specify sketch or", &["Type", "Increment"])
        } else {
            ("Specify next point or", &["Type", "Increment", "Undo"])
        };
        let mut spec = ps(msg).options(opts).allow_enter().no_ortho();
        if let Some(last) = pts.last() {
            spec = spec.base(*last).preview(move |_, p| {
                let mut v = snapshot.clone();
                v.push(p);
                vec![pv(Entity::pline_pts(&v, false))]
            });
        }
        let r = c.point(spec).await?;
        match r.kw() {
            Some("Type") => {
                let k = c
                    .keyword(
                        ps("Enter sketch type")
                            .options(&SKETCH_TYPES)
                            .default_str(SKETCH_TYPES[kind]),
                    )
                    .await?;
                if let Some(i) = SKETCH_TYPES.iter().position(|t| Some(*t) == k.kw()) {
                    set_pref(&c, "SKETCHTYPE", Value::Num(i as f64));
                }
            }
            Some("Increment") => {
                if let Some(v) = c
                    .distance(
                        ps("Specify sketch increment")
                            .default_num(inc)
                            .min(0.0)
                            .nonzero(),
                    )
                    .await?
                    .value()
                {
                    set_pref(&c, "SKETCHINC", Value::Num(v));
                }
            }
            Some("Undo") => {
                pts.pop();
            }
            _ => match r.pt() {
                Some(p) if pts.last().is_none_or(|l| dist(*l, p) >= inc - 1e-9) => pts.push(p),
                Some(_) => c.log(&format!(
                    "Point not recorded: it is closer than the increment ({}).",
                    fmt_num(inc, 4, true)
                )),
                None => break,
            },
        }
    }
    if pts.len() < 2 {
        c.log("Nothing sketched.");
        return Ok(());
    }
    let n = pts.len() - 1;
    match (pref_num(&c, "SKETCHTYPE", 1.0) as usize).min(2) {
        0 => {
            c.doc_mut(|d| {
                d.change(|d| {
                    for w in pts.windows(2) {
                        d.add(Entity::line(w[0], w[1]));
                    }
                })
            });
            c.log(&format!(
                "{n} line{} recorded.",
                if n == 1 { "" } else { "s" }
            ));
        }
        1 => {
            c.add(Entity::pline_pts(&pts, false));
            c.log(&format!(
                "Polyline with {n} edge{} recorded.",
                if n == 1 { "" } else { "s" }
            ));
        }
        _ => {
            c.add(Entity::spline(pts.clone(), false));
            c.log(&format!("Spline with {} fit points recorded.", pts.len()));
        }
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(dd("MLINE", run(mline)).aliases(&["ML"]).title("Multiline").icon("line").desc("Creates multiple parallel lines as a group of two polylines (STANDARD style; justification and scale are supported, styles and MLEDIT are not)."));
    r.define(dd("SKETCH", run(sketch)).title("Sketch").icon("pline").desc("Creates a series of freehand line segments, a polyline or a spline from consecutive picked points spaced at least the increment apart."));
}
