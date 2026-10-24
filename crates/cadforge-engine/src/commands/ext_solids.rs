//! More 3D solid commands: REVOLVE, SLICE, TORUS, SWEEP, LOFT, PYRAMID, POLYSOLID, the 3D transforms (3DROTATE, ROTATE3D,
//! MIRROR3D, 3DSCALE, 3DALIGN, 3DARRAY), FILLETEDGE, CHAMFEREDGE, SOLIDEDIT (body options), SHELL, THICKEN, CONVTOSOLID,
//! INTERFERE, SOLIDHIST, SECTIONPLANE and FLATSHOT. The mesh kernel is in `cadforge_core::solid::ext`.
use super::helpers::*;
use super::{def, run};
use crate::ctx::*;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::solid::boolean::{boolean_sync, BoolOp};
use cadforge_core::solid::ext::edges::{self, Chain, Edge, Round};
use cadforge_core::solid::ext::flat::flatshot;
use cadforge_core::solid::ext::repair::{self, Topo};
use cadforge_core::solid::ext::slice::{section_loops, slice, Plane};
use cadforge_core::solid::ext::sweep::{self, SweepOpts};
use cadforge_core::solid::ext::xform;
use cadforge_core::solid::mesh::*;
use cadforge_core::solid::*;
use cadforge_core::style::fmt_num;
use cadforge_core::view::View;
use std::collections::HashMap;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn d3(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("3D", name, f).no_pickfirst()
}

fn dm(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("Modify", name, f).no_pickfirst()
}

fn f(c: &Ctx, v: f64) -> String {
    fmt_num(
        v,
        c.doc(|d| d.settings.units.precision.max(0) as usize),
        false,
    )
}

fn p3(p: Pt) -> V3 {
    V3::new(p.x, p.y, 0.0)
}

fn plural(n: usize, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

/// `n` of something whose plural is irregular.
fn count_of(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn add_solid(c: &Ctx, s: Solid) -> u64 {
    c.doc_mut(|d| d.add_solid(s))
}

/// A solid made from `src` (same layer and colour) with a new mesh.
fn derived(src: &Solid, m: Mesh) -> Solid {
    let mut s = Solid::from_mesh(&src.name, m);
    s.layer = src.layer.clone();
    s.color = src.color;
    s
}

/// Give solid `id` a new mesh; its extrusion history no longer describes it.
fn set_mesh(c: &Ctx, id: u64, m: Mesh) {
    c.doc_mut(|d| {
        d.change(|d| {
            if let Some(s) = d.solid_by_id_mut(id) {
                s.vertices = m.vertices;
                s.faces = m.faces;
                s.program = None;
            }
        })
    });
}

async fn select_solids(c: &Ctx, prompt: &str) -> CmdResult<Vec<u64>> {
    Ok(
        c.select(ps(prompt).solids().filter(|_| false).pickfirst(true))
            .await?
            .solids,
    )
}

/// Pick drawing objects and solids to transform; `None` when nothing was picked.
async fn pick_objects(c: &Ctx, prompt: &str) -> CmdResult<Option<Selection>> {
    let sel = c.select(ps(prompt).solids().pickfirst(true)).await?;
    Ok((!sel.is_empty()).then_some(sel))
}

fn view_of(c: &Ctx) -> View {
    c.view(|v| v.clone())
}

// ---------------------------------------------------------------- outlines and paths

/// Objects that can serve as (part of) a closed outline.
fn outline_ok(e: &Entity) -> bool {
    matches!(
        e.kind,
        Kind::Pline { .. }
            | Kind::Circle { .. }
            | Kind::Ellipse(_)
            | Kind::Spline { .. }
            | Kind::Line { .. }
            | Kind::Arc { .. }
            | Kind::Region(_)
    )
}

fn closed_outline(e: &Entity) -> bool {
    is_closed_curve(e) || matches!(e.kind, Kind::Region(_))
}

fn curve_ok(e: &Entity) -> bool {
    matches!(
        e.kind,
        Kind::Pline { .. }
            | Kind::Circle { .. }
            | Kind::Ellipse(_)
            | Kind::Spline { .. }
            | Kind::Line { .. }
            | Kind::Arc { .. }
    )
}

fn tess_ring(l: &[Vertex]) -> Loop {
    let mut pts: Loop = vec![];
    for p in tess_segs(&pline_segs(l, true), 0.02) {
        if pts.last().map_or(true, |q| dist(p, *q) > 1e-9) {
            pts.push(p);
        }
    }
    if pts.len() > 1 && dist(pts[0], pts[pts.len() - 1]) < 1e-9 {
        pts.pop();
    }
    pts
}

/// Closed planar loops of the objects (regions, closed curves, or chains of lines and arcs).
fn outline_loops(c: &Ctx, ids: &[u64]) -> CmdResult<Vec<Loop>> {
    let ents = c.entities(ids);
    let mut loops: Vec<Loop> = vec![];
    for e in &ents {
        if let Kind::Region(r) = &e.kind {
            loops.extend(r.loops.iter().map(|l| tess_ring(l)));
        }
    }
    let rest: Vec<&Entity> = ents
        .iter()
        .filter(|e| !matches!(e.kind, Kind::Region(_)))
        .collect();
    if !rest.is_empty() {
        let pr = c.doc(|d| profile_loops(&rest, d, 0.02));
        if let Some(e) = pr.error {
            return Err(e.into());
        }
        loops.extend(pr.loops);
    }
    if loops.is_empty() {
        return Err("Select closed polylines, circles, ellipses, regions or a closed chain of lines and arcs.".into());
    }
    Ok(loops)
}

/// Centroid of the largest loop (where a swept outline is attached to its path by default).
fn outline_centre(loops: &[Loop]) -> Pt {
    let Some(l) = loops
        .iter()
        .max_by(|a, b| polygon_area(a).abs().total_cmp(&polygon_area(b).abs()))
    else {
        return Pt::ZERO;
    };
    let a = polygon_area(l);
    if a.abs() < 1e-12 {
        return l[0];
    }
    let (mut x, mut y) = (0.0, 0.0);
    for i in 0..l.len() {
        let (p, q) = (l[i], l[(i + 1) % l.len()]);
        let w = p.x * q.y - q.x * p.y;
        x += (p.x + q.x) * w;
        y += (p.y + q.y) * w;
    }
    pt(x / (6.0 * a), y / (6.0 * a))
}

/// A curve as a polyline in the drawing plane, and whether it closes on itself.
fn curve_path(c: &Ctx, id: u64) -> CmdResult<(Vec<V3>, bool)> {
    let Some(e) = c.entity_by_id(id) else {
        return Err("That object no longer exists.".into());
    };
    let closed = is_closed_curve(&e);
    let segs = c.doc(|d| segs_for(&e, d));
    let mut pts = tess_segs(&segs, 0.02);
    if closed && pts.len() > 1 && dist(pts[0], pts[pts.len() - 1]) < 1e-9 {
        pts.pop();
    }
    Ok((pts.into_iter().map(p3).collect(), closed))
}

// ---------------------------------------------------------------- axes and planes

/// Straight segment of a line or polyline nearest the pick, as a point and direction.
async fn axis_object(c: &Ctx) -> CmdResult<Option<(V3, V3)>> {
    let r = c
        .entity(
            ps("Select an object")
                .filter(|e| matches!(e.kind, Kind::Line { .. } | Kind::Pline { .. })),
        )
        .await?;
    let EntR::Entity { id, pt: at, .. } = r else {
        return Ok(None);
    };
    let Some(e) = c.entity_by_id(id) else {
        return Ok(None);
    };
    let segs = c.doc(|d| segs_for(&e, d));
    let seg = segs
        .iter()
        .filter(|s| s.is_line())
        .min_by(|a, b| a.nearest(at).2.total_cmp(&b.nearest(at).2));
    match seg {
        Some(s) => Ok(Some((p3(s.start()), p3(s.end() - s.start())))),
        None => {
            c.log_error("Select a line or a straight polyline segment.");
            Ok(None)
        }
    }
}

/// Axis for REVOLVE and ROTATE3D: two points, an object, or one of the principal directions (`object_default`: Enter
/// selects an object). Returns a point on the axis and its direction.
async fn pick_axis(
    c: &Ctx,
    msg: &str,
    end_msg: &str,
    opts: &[&str],
    object_default: bool,
) -> CmdResult<Option<(V3, V3)>> {
    let mut spec = ps(msg).options(opts);
    if object_default {
        spec = spec.default_str("Object");
    }
    let r = c.point(spec).await?;
    let through = |c: &Ctx, name: &'static str, dir: V3| {
        let c = c.clone();
        async move {
            Ok::<_, CmdError>(
                c.point(ps(&format!("Specify a point on the {name} axis")).default_pt(Pt::ZERO))
                    .await?
                    .pt()
                    .map(|p| (p3(p), dir)),
            )
        }
    };
    match r.kw() {
        Some("Object") => axis_object(c).await,
        Some("X") => Ok(Some((V3::default(), V3::new(1.0, 0.0, 0.0)))),
        Some("Y") => Ok(Some((V3::default(), V3::new(0.0, 1.0, 0.0)))),
        Some("Z") => Ok(Some((V3::default(), V3::new(0.0, 0.0, 1.0)))),
        Some("Xaxis") => through(c, "X", V3::new(1.0, 0.0, 0.0)).await,
        Some("Yaxis") => through(c, "Y", V3::new(0.0, 1.0, 0.0)).await,
        Some("Zaxis") => through(c, "Z", V3::new(0.0, 0.0, 1.0)).await,
        Some("View") => {
            let dir = edges::toward_viewer(&view_of(c));
            Ok(
                c.point(ps("Specify a point on the view direction axis").default_pt(Pt::ZERO))
                    .await?
                    .pt()
                    .map(|p| (p3(p), dir)),
            )
        }
        Some(_) => axis_by_points(c, None, end_msg).await,
        None => match r.pt() {
            Some(a) => axis_by_points(c, Some(a), end_msg).await,
            None if object_default => axis_object(c).await,
            None => Ok(None),
        },
    }
}

async fn axis_by_points(c: &Ctx, first: Option<Pt>, end_msg: &str) -> CmdResult<Option<(V3, V3)>> {
    let a = match first {
        Some(a) => a,
        None => match c.point(ps("Specify first point on axis")).await?.pt() {
            Some(a) => a,
            None => return Ok(None),
        },
    };
    let Some(b) = c
        .point(
            ps(end_msg)
                .base(a)
                .preview(move |_, p| vec![pv(Entity::line(a, p))]),
        )
        .await?
        .pt()
    else {
        return Ok(None);
    };
    if same(a, b) {
        c.log_error("The axis needs two different points.");
        return Ok(None);
    }
    Ok(Some((p3(a), p3(b - a))))
}

/// Cutting / mirror plane chosen the AutoCAD way. Points picked on the drawing plane have z = 0, so the planes
/// parallel to XY ask for their elevation.
async fn pick_plane(c: &Ctx, msg: &str) -> CmdResult<Option<Plane>> {
    let r = c
        .point(
            ps(msg)
                .options(&["Object", "Zaxis", "View", "XY", "YZ", "ZX", "3points"])
                .default_str("3points"),
        )
        .await?;
    let plane = |p: V3, n: V3| Plane::new(p, n);
    let origin = |c: Ctx, what: &'static str| async move {
        Ok::<_, CmdError>(
            c.point(ps(&format!("Specify a point on the {what}")).default_pt(Pt::ZERO))
                .await?
                .pt(),
        )
    };
    let found = match r.kw() {
        Some("Object") => {
            let e = c
                .entity(
                    ps("Select a circle, ellipse, arc, 2D-spline, or 2D-polyline")
                        .filter(|e| curve_ok(e) && !matches!(e.kind, Kind::Line { .. })),
                )
                .await?;
            e.id()
                .and_then(|_| plane(V3::default(), V3::new(0.0, 0.0, 1.0)))
        }
        Some("Zaxis") => {
            let Some(a) = c
                .point(ps("Specify a point on the section plane"))
                .await?
                .pt()
            else {
                return Ok(None);
            };
            let Some(b) = c
                .point(
                    ps("Specify a point on the Z axis (normal) of the plane")
                        .base(a)
                        .preview(move |_, p| vec![pv(Entity::line(a, p))]),
                )
                .await?
                .pt()
            else {
                return Ok(None);
            };
            plane(p3(a), p3(b - a))
        }
        Some("View") => origin(c.clone(), "current view plane")
            .await?
            .and_then(|p| plane(p3(p), edges::toward_viewer(&view_of(c)))),
        Some("XY") => {
            if origin(c.clone(), "XY-plane").await?.is_none() {
                return Ok(None);
            }
            let z = c
                .number(ps("Specify Z elevation of the plane").default_num(0.0))
                .await?;
            z.value()
                .and_then(|z| plane(V3::new(0.0, 0.0, z), V3::new(0.0, 0.0, 1.0)))
        }
        Some("YZ") => origin(c.clone(), "YZ-plane")
            .await?
            .and_then(|p| plane(p3(p), V3::new(1.0, 0.0, 0.0))),
        Some("ZX") => origin(c.clone(), "ZX-plane")
            .await?
            .and_then(|p| plane(p3(p), V3::new(0.0, 1.0, 0.0))),
        _ => {
            let a = match r.pt() {
                Some(a) => a,
                None => match c.point(ps("Specify first point on plane")).await?.pt() {
                    Some(a) => a,
                    None => return Ok(None),
                },
            };
            let Some(b) = c
                .point(
                    ps("Specify second point on plane")
                        .base(a)
                        .preview(move |_, p| vec![pv(Entity::line(a, p))]),
                )
                .await?
                .pt()
            else {
                return Ok(None);
            };
            let Some(third) = c
                .point(ps("Specify third point on plane").base(b))
                .await?
                .pt()
            else {
                return Ok(None);
            };
            Plane::through(p3(a), p3(b), p3(third))
        }
    };
    if found.is_none() {
        c.log_error("The points do not define a plane.");
    }
    Ok(found)
}

// ---------------------------------------------------------------- REVOLVE, TORUS, SWEEP, LOFT

async fn revolve(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c
        .select(
            ps("Select objects to revolve")
                .filter(outline_ok)
                .pickfirst(true),
        )
        .await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let loops = outline_loops(&c, &sel.ents)?;
    let Some((o, dir)) = pick_axis(
        &c,
        "Specify axis start point or define axis by",
        "Specify axis endpoint",
        &["Object", "X", "Y", "Z"],
        true,
    )
    .await?
    else {
        return Ok(());
    };
    let (mut start, mut sign) = (0.0, 1.0);
    let sweep = loop {
        let r = c
            .angle(
                ps("Specify angle of revolution")
                    .options(&["STart angle", "Reverse"])
                    .default_num(TAU),
            )
            .await?;
        match r.kw() {
            Some("STart angle") => {
                if let Some(v) = c
                    .angle(ps("Specify start angle").default_num(0.0))
                    .await?
                    .value()
                {
                    start = v;
                }
            }
            Some("Reverse") => sign = -sign,
            _ => match r.value() {
                Some(v) => break v,
                None => return Ok(()),
            },
        }
    };
    if sweep.abs() > TAU + 1e-9 {
        return Err("The angle of revolution must be between -360 and 360 degrees.".into());
    }
    let s = sweep::revolve(&loops, o, dir, start, sign * sweep, "Revolution")?;
    c.log(&format!(
        "Revolved solid created (volume {}).",
        f(&c, mesh_volume(&s.mesh()))
    ));
    add_solid(&c, s);
    Ok(())
}

/// A radius typed or measured, or half a diameter when the `Diameter` option is chosen.
async fn radius_or_diameter(
    c: &Ctx,
    spec: PromptSpec,
    diameter_msg: &str,
    default: f64,
) -> CmdResult<Option<f64>> {
    let r = c.distance(spec).await?;
    if r.kw() == Some("Diameter") {
        return Ok(c
            .distance(
                ps(diameter_msg)
                    .default_num(default * 2.0)
                    .nonzero()
                    .min(0.0),
            )
            .await?
            .value()
            .map(|d| d / 2.0));
    }
    Ok(r.value())
}

async fn torus(c: Ctx, _a: Args) -> CmdResult<()> {
    let mem_r = c.app(|a| a.mem.solid.radius);
    let Some(ctr) = c.point(ps("Specify center point")).await?.pt() else {
        return Ok(());
    };
    let ring = move |_: &cadforge_core::drawing::Drawing, p: Pt| {
        vec![pv(Entity::circle(ctr, dist(ctr, p).max(1e-9)))]
    };
    let Some(r) = radius_or_diameter(
        &c,
        ps("Specify radius")
            .options(&["Diameter"])
            .base(ctr)
            .default_num(mem_r)
            .nonzero()
            .min(0.0)
            .preview(ring),
        "Specify diameter",
        mem_r,
    )
    .await?
    else {
        return Ok(());
    };
    let tube = c
        .distance(
            ps("Specify tube radius")
                .options(&["2Point", "Diameter"])
                .base(ctr)
                .default_num(r / 4.0)
                .nonzero()
                .min(0.0),
        )
        .await?;
    let t = match tube.kw() {
        Some("2Point") => c
            .distance(ps("Specify first point of tube diameter"))
            .await?
            .value()
            .map(|d| d / 2.0),
        Some("Diameter") => c
            .distance(
                ps("Specify tube diameter")
                    .default_num(r / 2.0)
                    .nonzero()
                    .min(0.0),
            )
            .await?
            .value()
            .map(|d| d / 2.0),
        _ => tube.value(),
    };
    let Some(t) = t else { return Ok(()) };
    add_solid(&c, sweep::torus(p3(ctr), r, t, "Torus")?);
    Ok(())
}

async fn sweep_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c
        .select(
            ps("Select objects to sweep")
                .filter(outline_ok)
                .pickfirst(true),
        )
        .await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let loops = outline_loops(&c, &sel.ents)?;
    let mut o = SweepOpts {
        base: outline_centre(&loops),
        ..SweepOpts::default()
    };
    let path = loop {
        let r = c
            .entity(
                ps("Select sweep path")
                    .options(&["Alignment", "Base point", "Scale", "Twist"])
                    .filter(curve_ok),
            )
            .await?;
        match r {
            EntR::Kw(k) => match k.as_str() {
                "Alignment" => {
                    let a = c
                        .keyword(
                            ps("Align sweep object perpendicular to path before sweep?")
                                .options(&["Yes", "No"])
                                .default_str(if o.align { "Yes" } else { "No" }),
                        )
                        .await?;
                    o.align = a.kw() != Some("No");
                }
                "Base point" => {
                    if let Some(p) = c.point(ps("Specify base point")).await?.pt() {
                        o.base = p;
                    }
                }
                "Scale" => {
                    if let Some(v) = c
                        .number(
                            ps("Enter scale factor")
                                .default_num(o.scale)
                                .nonzero()
                                .min(0.0),
                        )
                        .await?
                        .value()
                    {
                        o.scale = v;
                    }
                }
                _ => {
                    if let Some(v) = c
                        .angle(ps("Enter twist angle").default_num(o.twist))
                        .await?
                        .value()
                    {
                        o.twist = v;
                    }
                }
            },
            EntR::Entity { id, .. } => break id,
            EntR::Enter => return Ok(()),
        }
    };
    let (pts, closed) = curve_path(&c, path)?;
    let s = sweep::sweep(&loops, &pts, closed, &o, "Sweep")?;
    c.log(&format!(
        "Swept solid created (volume {}).",
        f(&c, mesh_volume(&s.mesh()))
    ));
    add_solid(&c, s);
    Ok(())
}

async fn loft(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c
        .select(
            ps("Select cross-sections in lofting order")
                .filter(closed_outline)
                .pickfirst(true),
        )
        .await?;
    if sel.ents.len() < 2 {
        if !sel.ents.is_empty() {
            c.log_error("A loft needs at least two cross-sections.");
        }
        return Ok(());
    }
    let mut sections: Vec<Loop> = vec![];
    for id in &sel.ents {
        let mut l = outline_loops(&c, &[*id])?;
        if l.len() != 1 {
            return Err("Each cross-section must be a single closed outline.".into());
        }
        sections.extend(l.pop());
    }
    let mut smooth = true;
    loop {
        let k = c
            .keyword(
                ps("Enter an option")
                    .options(&["Guides", "Path", "Cross-sections only", "Settings"])
                    .default_str("Cross-sections only"),
            )
            .await?;
        match k.kw() {
            Some("Guides") | Some("Path") => c.log_error("Loft guides and paths are not supported; the loft passes through the cross-sections only."),
            Some("Settings") => {
                let s = c.keyword(ps("Enter a surface-fit option").options(&["Ruled", "Smooth fit"]).default_str(if smooth { "Smooth fit" } else { "Ruled" })).await?;
                smooth = s.kw() != Some("Ruled");
            }
            _ => break,
        }
    }
    c.log("Drawing objects lie in one plane: the cross-sections are stacked at equal spacing.");
    let mem_h = c.app(|a| a.mem.solid.height);
    let Some(step) = c
        .distance(
            ps("Specify distance between cross-sections")
                .default_num(mem_h)
                .nonzero(),
        )
        .await?
        .value()
    else {
        return Ok(());
    };
    c.app(|a| a.mem.solid.height = step);
    let z: Vec<f64> = (0..sections.len()).map(|i| i as f64 * step).collect();
    let s = sweep::loft(&sections, &z, smooth, "Loft")?;
    c.log(&format!(
        "Lofted solid created (volume {}).",
        f(&c, mesh_volume(&s.mesh()))
    ));
    add_solid(&c, s);
    Ok(())
}

// ---------------------------------------------------------------- PYRAMID, POLYSOLID

async fn pyramid(c: Ctx, _a: Args) -> CmdResult<()> {
    let (mut sides, mut inscribed) = (4usize, false);
    let mem = c.app(|a| a.mem.solid.clone());
    // (centre, circumradius of the base, first vertex angle)
    let base: (Pt, f64, f64);
    loop {
        c.log(&format!(
            "{sides} sides {}",
            if inscribed {
                "Inscribed"
            } else {
                "Circumscribed"
            }
        ));
        let r = c
            .point(ps("Specify center point of base").options(&["Edge", "Sides"]))
            .await?;
        match r.kw() {
            Some("Sides") => {
                if let Some(n) = c
                    .number(
                        ps("Enter number of sides")
                            .integer()
                            .min(3.0)
                            .max(32.0)
                            .default_num(sides as f64),
                    )
                    .await?
                    .value()
                {
                    sides = n as usize;
                }
                continue;
            }
            Some("Edge") => {
                let Some(a) = c.point(ps("Specify first endpoint of edge")).await?.pt() else {
                    return Ok(());
                };
                let Some(b) = c
                    .point(
                        ps("Specify second endpoint of edge")
                            .base(a)
                            .preview(move |_, p| vec![pv(Entity::line(a, p))]),
                    )
                    .await?
                    .pt()
                else {
                    return Ok(());
                };
                let l = dist(a, b);
                if l < 1e-9 {
                    return Err("The edge has zero length.".into());
                }
                let step = PI / sides as f64;
                let centre = lerp(a, b, 0.5) + perp(unit(b - a)) * (l / (2.0 * step.tan()));
                base = (centre, l / (2.0 * step.sin()), angle_of(centre, a));
                break;
            }
            _ => {}
        }
        let Some(ctr) = r.pt() else { return Ok(()) };
        let preview = move |_: &cadforge_core::drawing::Drawing, p: Pt| {
            vec![pv(Entity::circle(ctr, dist(ctr, p).max(1e-9)))]
        };
        let rad = loop {
            let rad = c
                .distance(
                    ps("Specify base radius")
                        .options(&["Inscribed"])
                        .base(ctr)
                        .default_num(mem.radius)
                        .nonzero()
                        .min(0.0)
                        .preview(preview),
                )
                .await?;
            if rad.kw() == Some("Inscribed") {
                inscribed = true;
                continue;
            }
            break rad;
        };
        let Some(v) = rad.value() else { return Ok(()) };
        base = polygon_base(ctr, v, rad.pt(), sides, inscribed);
        break;
    }
    let (ctr, r1, rot) = base;
    let mut r2 = 0.0;
    let h = loop {
        let k = c
            .distance(
                ps("Specify height")
                    .options(&["2Point", "Top radius"])
                    .base(ctr)
                    .default_num(mem.height)
                    .nonzero(),
            )
            .await?;
        match k.kw() {
            Some("Top radius") => {
                if let Some(t) = c
                    .distance(ps("Specify top radius").base(ctr).default_num(0.0).min(0.0))
                    .await?
                    .value()
                {
                    // a top radius is measured the same way as the base radius (circumscribed: to the face centres)
                    r2 = if inscribed {
                        t
                    } else {
                        t / (PI / sides as f64).cos()
                    };
                }
            }
            Some("2Point") => match c.distance(ps("Specify first point")).await?.value() {
                Some(v) => break v,
                None => return Ok(()),
            },
            _ => match k.value() {
                Some(v) => break v,
                None => return Ok(()),
            },
        }
    };
    c.app(|a| a.mem.solid.height = h);
    let (z0, hh) = if h < 0.0 { (h, h.abs()) } else { (0.0, h) };
    let name = if r2 > 0.0 { "Frustum" } else { "Pyramid" };
    add_solid(
        &c,
        sweep::pyramid(ctr.x, ctr.y, z0, r1, r2, hh, sides, rot, name)?,
    );
    Ok(())
}

/// Centre, circumradius and first vertex angle of a regular base given the picked / typed radius.
fn polygon_base(
    ctr: Pt,
    r: f64,
    picked: Option<Pt>,
    sides: usize,
    inscribed: bool,
) -> (Pt, f64, f64) {
    let half = PI / sides as f64;
    let toward = picked.map(|p| angle_of(ctr, p));
    if inscribed {
        (ctr, r, toward.unwrap_or(0.0))
    } else {
        (ctr, r / half.cos(), toward.map_or(half, |a| a - half))
    }
}

async fn polysolid(c: Ctx, _a: Args) -> CmdResult<()> {
    let (mut h, mut w, mut just) = (4.0, 0.25, "Center".to_string());
    let (path, closed): (Vec<V3>, bool) = 'outer: loop {
        c.log(&format!(
            "Height = {}, Width = {}, Justification = {just}",
            f(&c, h),
            f(&c, w)
        ));
        let r = c
            .point(
                ps("Specify start point or")
                    .options(&["Object", "Height", "Width", "Justify"])
                    .default_str("Object"),
            )
            .await?;
        match r.kw() {
            Some("Height") => {
                if let Some(v) = c
                    .distance(ps("Specify height").default_num(h).nonzero().min(0.0))
                    .await?
                    .value()
                {
                    h = v;
                }
            }
            Some("Width") => {
                if let Some(v) = c
                    .distance(ps("Specify width").default_num(w).nonzero().min(0.0))
                    .await?
                    .value()
                {
                    w = v;
                }
            }
            Some("Justify") => {
                let k = c
                    .keyword(
                        ps("Enter justification")
                            .options(&["Left", "Center", "Right"])
                            .default_str(&just),
                    )
                    .await?;
                if let Some(k) = k.kw() {
                    just = k.to_string();
                }
            }
            Some(_) => {
                let r = c.entity(ps("Select an object").filter(curve_ok)).await?;
                let Some(id) = r.id() else { return Ok(()) };
                break 'outer curve_path(&c, id)?;
            }
            None => match r.pt() {
                Some(p) => break 'outer polysolid_path(&c, p).await?,
                None => {
                    let r = c.entity(ps("Select an object").filter(curve_ok)).await?;
                    let Some(id) = r.id() else { return Ok(()) };
                    break 'outer curve_path(&c, id)?;
                }
            },
        }
    };
    if path.len() < 2 {
        return Ok(());
    }
    let (x0, x1) = match just.as_str() {
        "Left" => (-w, 0.0),
        "Right" => (0.0, w),
        _ => (-w / 2.0, w / 2.0),
    };
    let section = vec![pt(x0, 0.0), pt(x1, 0.0), pt(x1, h), pt(x0, h)];
    let s = sweep::sweep(
        &[section],
        &path,
        closed,
        &SweepOpts {
            base: Pt::ZERO,
            ..SweepOpts::default()
        },
        "Polysolid",
    )?;
    c.log(&format!(
        "Polysolid created (volume {}).",
        f(&c, mesh_volume(&s.mesh()))
    ));
    add_solid(&c, s);
    Ok(())
}

/// Points of a polysolid path picked one by one (straight and tangent-arc segments).
async fn polysolid_path(c: &Ctx, start: Pt) -> CmdResult<(Vec<V3>, bool)> {
    let mut verts = vec![Vertex::new(start.x, start.y, 0.0)];
    let (mut arc, mut closed) = (false, false);
    loop {
        let n = verts.len();
        let mut opts: Vec<&str> = vec![];
        if n >= 2 {
            opts.push(if arc { "Line" } else { "Arc" });
        }
        if n >= 3 {
            opts.push("Close");
        }
        if n >= 2 {
            opts.push("Undo");
        }
        let v2 = verts.clone();
        let r = c
            .point(
                ps(if arc {
                    "Specify endpoint of arc"
                } else {
                    "Specify next point"
                })
                .options(&opts)
                .base(verts[n - 1].pt())
                .allow_enter()
                .preview(move |_, p| pline_preview(&v2, p)),
            )
            .await?;
        match r.kw() {
            Some("Arc") => arc = true,
            Some("Line") => arc = false,
            Some("Undo") => {
                verts.pop();
                if verts.len() < 2 {
                    arc = false;
                }
                if let Some(l) = verts.last_mut() {
                    l.b = 0.0;
                }
            }
            Some("Close") => {
                closed = true;
                break;
            }
            _ => {
                let Some(p) = r.pt() else { break };
                if arc {
                    let segs = pline_segs(&verts, false);
                    if let Some(last) = segs.last() {
                        let at = verts[n - 1].pt();
                        verts[n - 1].b = tangent_bulge(at, last.tangent(1.0), p);
                    }
                }
                verts.push(Vertex::new(p.x, p.y, 0.0));
            }
        }
    }
    let mut pts = tess_segs(&pline_segs(&verts, closed), 0.02);
    if closed && pts.len() > 1 && dist(pts[0], pts[pts.len() - 1]) < 1e-9 {
        pts.pop();
    }
    Ok((pts.into_iter().map(p3).collect(), closed))
}

// ---------------------------------------------------------------- SLICE, SECTIONPLANE

async fn slice_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = select_solids(&c, "Select objects to slice").await?;
    if ids.is_empty() {
        return Ok(());
    }
    let Some(pl) = pick_plane(
        &c,
        "Specify first point on slicing plane or define plane by",
    )
    .await?
    else {
        return Ok(());
    };
    let r = c
        .point(
            ps("Specify a point on desired side")
                .options(&["keep Both sides", "Positive", "Negative"])
                .default_str("Both"),
        )
        .await?;
    let (keep_front, keep_back) = match r.kw() {
        Some("Positive") => (true, false),
        Some("Negative") => (false, true),
        Some(_) => (true, true),
        None => match r.pt() {
            Some(p) => {
                let s = pl.dist(p3(p));
                if s.abs() < 1e-9 {
                    return Err(
                        "The point lies on the slicing plane; pick a point to one side of it."
                            .into(),
                    );
                }
                (s > 0.0, s < 0.0)
            }
            None => (true, true),
        },
    };
    let mut cut = 0;
    for s in c.solids(&ids) {
        let parts = slice(&s.mesh(), &pl)?;
        let (Some(front), Some(back)) = (parts.front, parts.back) else {
            c.log(&format!(
                "The slicing plane does not cut the {}.",
                if s.name.is_empty() {
                    "solid".to_string()
                } else {
                    s.name.to_lowercase()
                }
            ));
            continue;
        };
        c.doc_mut(|d| {
            d.change(|d| {
                d.remove_solid(s.id);
                for (keep, m) in [(keep_front, front), (keep_back, back)] {
                    if keep {
                        d.insert_solid_raw(derived(&s, m));
                    }
                }
            })
        });
        cut += 1;
    }
    if cut > 0 {
        c.log(&format!("{} sliced.", plural(cut, "solid")));
    }
    c.clear_sel();
    Ok(())
}

async fn sectionplane(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = select_solids(&c, "Select solids to section").await?;
    if ids.is_empty() {
        return Ok(());
    }
    let Some(pl) = pick_plane(
        &c,
        "Specify first point on section plane or define plane by",
    )
    .await?
    else {
        return Ok(());
    };
    let mut outlines: Vec<Vec<V3>> = vec![];
    for s in c.solids(&ids) {
        outlines.extend(section_loops(&s.mesh(), &pl));
    }
    if outlines.is_empty() {
        c.log_error("The section plane does not cut the selected solids.");
        return Ok(());
    }
    let Some(at) = c
        .point(ps("Specify insertion point for the section outline").default_pt(Pt::ZERO))
        .await?
        .pt()
    else {
        return Ok(());
    };
    let (u, v) = pl.basis();
    let o = pl.origin();
    let flat: Vec<Vec<Vertex>> = outlines
        .iter()
        .map(|l| {
            l.iter()
                .map(|p| Vertex::new(at.x + p.sub(o).dot(u), at.y + p.sub(o).dot(v), 0.0))
                .collect()
        })
        .collect();
    let layer = c.doc_mut(|d| d.ensure_layer("SECTION"));
    for l in &flat {
        c.add(Entity::pline(l.clone(), true).on_layer(&layer));
    }
    c.add(Entity::hatch(flat.clone(), "ANSI31").on_layer(&layer));
    c.log(&format!(
        "Section outline drawn flat on layer {layer} ({}). No live section object is created.",
        plural(flat.len(), "outline")
    ));
    Ok(())
}

/// SECTION: regions where a plane cuts the solids, drawn flat in the plane's own coordinates (for the XY plane that is
/// exactly where the cut is). Holes in the section stay holes.
async fn section_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = select_solids(&c, "Select solids to section").await?;
    if ids.is_empty() {
        return Ok(());
    }
    let Some(pl) = pick_plane(
        &c,
        "Specify first point on section plane or define plane by",
    )
    .await?
    else {
        return Ok(());
    };
    let mut outlines: Vec<Vec<V3>> = vec![];
    for s in c.solids(&ids) {
        outlines.extend(section_loops(&s.mesh(), &pl));
    }
    if outlines.is_empty() {
        c.log_error("The section plane does not cut the selected solids.");
        return Ok(());
    }
    let (u, v) = pl.basis();
    let loops: Vec<Vec<Vertex>> = outlines
        .iter()
        .map(|l| {
            l.iter()
                .map(|p| Vertex::new(p.dot(u), p.dot(v), 0.0))
                .collect()
        })
        .collect();
    let layer = c.doc_mut(|d| d.ensure_layer("SECTION"));
    let n = loops.len();
    c.add(Entity::new(Kind::Region(RegionData { loops })).on_layer(&layer));
    c.log(&format!(
        "Section region created on layer {layer} from {}.",
        plural(n, "outline")
    ));
    Ok(())
}

// ---------------------------------------------------------------- 3D transforms

/// Apply (or copy with) a 3D matrix. Drawing objects follow only when it keeps the drawing plane in place.
fn apply_m3(c: &Ctx, sel: &Selection, m: &M3, copy: bool) {
    let plan = xform::to_plan_mat(m);
    let (ents, solids) = (c.entities(&sel.ents), c.solids(&sel.solids));
    c.doc_mut(|d| {
        if copy {
            if let Some(mm) = &plan {
                add_copies(d, &ents, &[], mm);
            }
            for s in &solids {
                let mut copy = transform_mesh(s, m);
                copy.id = 0;
                d.add_solid(copy);
            }
        } else {
            d.change(|d| {
                for id in &sel.ents {
                    if let (Some(e), Some(mm)) = (d.by_id_mut(*id), &plan) {
                        transform_entity(e, mm);
                    }
                }
                for id in &sel.solids {
                    if let Some(s) = d.solid_by_id_mut(*id) {
                        let t = transform_mesh(s, m);
                        s.vertices = t.vertices;
                        s.faces = t.faces;
                    }
                }
            })
        }
    });
    if plan.is_none() && !sel.ents.is_empty() {
        c.log(&format!("{} lie in the drawing plane and cannot follow a move out of it; they were left unchanged.", plural(sel.ents.len(), "drawing object")));
    }
    c.clear_sel();
}

async fn rotate3d_gizmo(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(sel) = pick_objects(&c, "Select objects").await? else {
        return Ok(());
    };
    let Some(base) = c.point(ps("Specify base point")).await?.pt() else {
        return Ok(());
    };
    let axis = c
        .keyword(
            ps("Pick a rotation axis")
                .options(&["X", "Y", "Z"])
                .default_str("Z"),
        )
        .await?;
    let dir = match axis.kw() {
        Some("X") => V3::new(1.0, 0.0, 0.0),
        Some("Y") => V3::new(0.0, 1.0, 0.0),
        _ => V3::new(0.0, 0.0, 1.0),
    };
    let first = c
        .angle(ps("Specify angle start point or type an angle").base(base))
        .await?;
    let Some(v) = first.value() else {
        return Ok(());
    };
    let angle = if first.pt().is_some() {
        let Some(end) = c
            .angle(ps("Specify angle end point").base(base))
            .await?
            .value()
        else {
            return Ok(());
        };
        end - v
    } else {
        v
    };
    apply_m3(&c, &sel, &xform::rotate_axis(p3(base), dir, angle), false);
    Ok(())
}

async fn rotate3d(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(sel) = pick_objects(&c, "Select objects").await? else {
        return Ok(());
    };
    let Some((o, dir)) = pick_axis(
        &c,
        "Specify first point on axis or define axis by",
        "Specify second point on axis",
        &["Object", "View", "Xaxis", "Yaxis", "Zaxis", "2points"],
        false,
    )
    .await?
    else {
        return Ok(());
    };
    let r = c
        .angle(
            ps("Specify rotation angle")
                .options(&["Reference"])
                .default_num(0.0),
        )
        .await?;
    let angle = if r.kw() == Some("Reference") {
        let Some(a) = c
            .angle(ps("Specify the reference angle").default_num(0.0))
            .await?
            .value()
        else {
            return Ok(());
        };
        let Some(b) = c
            .angle(ps("Specify the new angle").default_num(0.0))
            .await?
            .value()
        else {
            return Ok(());
        };
        b - a
    } else {
        let Some(v) = r.value() else { return Ok(()) };
        v
    };
    apply_m3(&c, &sel, &xform::rotate_axis(o, dir, angle), false);
    Ok(())
}

async fn mirror3d(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(sel) = pick_objects(&c, "Select objects").await? else {
        return Ok(());
    };
    let Some(pl) = pick_plane(&c, "Specify first point of mirror plane (3 points) or").await?
    else {
        return Ok(());
    };
    let del = c
        .keyword(
            ps("Delete source objects?")
                .options(&["Yes", "No"])
                .default_str("No"),
        )
        .await?;
    apply_m3(
        &c,
        &sel,
        &xform::reflect(pl.n, pl.d),
        del.kw() != Some("Yes"),
    );
    Ok(())
}

async fn scale3d(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(sel) = pick_objects(&c, "Select objects").await? else {
        return Ok(());
    };
    let Some(base) = c.point(ps("Specify base point")).await?.pt() else {
        return Ok(());
    };
    let axes = c
        .keyword(
            ps("Pick a scale axis or plane")
                .options(&["Uniform", "X", "Y", "Z", "XY", "YZ", "ZX"])
                .default_str("Uniform"),
        )
        .await?;
    let which = axes.kw().unwrap_or("Uniform").to_string();
    let mut copy = false;
    let k = loop {
        let s = c
            .distance(
                ps("Specify scale factor")
                    .options(&["Copy", "Reference"])
                    .base(base)
                    .default_num(1.0)
                    .min(0.0)
                    .nonzero(),
            )
            .await?;
        match s.kw() {
            Some("Copy") => copy = true,
            Some("Reference") => {
                let Some(rl) = c
                    .distance(
                        ps("Specify reference length")
                            .default_num(1.0)
                            .nonzero()
                            .min(0.0),
                    )
                    .await?
                    .value()
                else {
                    return Ok(());
                };
                let Some(nl) = c
                    .distance(ps("Specify new length").base(base).nonzero().min(0.0))
                    .await?
                    .value()
                else {
                    return Ok(());
                };
                break nl / rl;
            }
            _ => match s.value() {
                Some(v) => break v,
                None => return Ok(()),
            },
        }
    };
    let on = |axis: &str| which == "Uniform" || which.contains(axis);
    let pick = |axis: &str| if on(axis) { k } else { 1.0 };
    let m = xform::scale_axes(p3(base), V3::new(pick("X"), pick("Y"), pick("Z")));
    // drawing objects scale only by a similarity in their plane
    let flat_ok = which == "Uniform" || which == "XY" || which == "Z";
    let sel = if flat_ok {
        sel
    } else {
        Selection {
            ents: vec![],
            solids: sel.solids,
        }
    };
    apply_m3(&c, &sel, &m, copy);
    Ok(())
}

async fn align3d(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(sel) = pick_objects(&c, "Select objects").await? else {
        return Ok(());
    };
    let mut copy = false;
    let first = loop {
        let r = c.point(ps("Specify base point").options(&["Copy"])).await?;
        if r.kw() == Some("Copy") {
            copy = true;
            continue;
        }
        match r.pt() {
            Some(p) => break p,
            None => return Ok(()),
        }
    };
    let mut src = vec![p3(first)];
    for msg in ["Specify second point or", "Specify third point or"] {
        match c
            .point(ps(msg).options(&["Continue"]).default_str("C").base(first))
            .await?
            .pt()
        {
            Some(p) => src.push(p3(p)),
            None => break,
        }
    }
    let Some(d1) = c.point(ps("Specify first destination point")).await?.pt() else {
        return Ok(());
    };
    let mut dst = vec![p3(d1)];
    for msg in [
        "Specify second destination point or",
        "Specify third destination point or",
    ] {
        if dst.len() >= src.len() {
            break;
        }
        match c
            .point(ps(msg).options(&["eXit"]).default_str("X").base(d1))
            .await?
            .pt()
        {
            Some(p) => dst.push(p3(p)),
            None => break,
        }
    }
    apply_m3(&c, &sel, &xform::align_points(&src, &dst), copy);
    Ok(())
}

async fn array3d(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(sel) = pick_objects(&c, "Select objects").await? else {
        return Ok(());
    };
    let kind = c
        .keyword(
            ps("Enter the type of array")
                .options(&["Rectangular", "Polar"])
                .default_str("Rectangular"),
        )
        .await?;
    let mut mats: Vec<M3> = vec![];
    if kind.kw() == Some("Polar") {
        let Some(n) = c
            .number(
                ps("Enter the number of items in the array")
                    .integer()
                    .min(2.0),
            )
            .await?
            .value()
        else {
            return Ok(());
        };
        let Some(fill) = c
            .angle(ps("Specify the angle to fill (+=ccw, -=cw)").default_num(TAU))
            .await?
            .value()
        else {
            return Ok(());
        };
        let rotate = c
            .keyword(
                ps("Rotate arrayed objects?")
                    .options(&["Yes", "No"])
                    .default_str("Yes"),
            )
            .await?;
        let Some(ctr) = c.point(ps("Specify center point of array")).await?.pt() else {
            return Ok(());
        };
        let second = c
            .point(
                ps("Specify second point on axis of rotation")
                    .base(ctr)
                    .allow_enter()
                    .default_str("Z axis"),
            )
            .await?;
        let dir = match second.pt() {
            Some(p) if !same(p, ctr) => p3(p - ctr),
            _ => V3::new(0.0, 0.0, 1.0),
        };
        let n = n as usize;
        let step = if (fill.abs() - TAU).abs() < 1e-9 {
            fill / n as f64
        } else {
            fill / (n - 1) as f64
        };
        let centre = selection_centre(&c, &sel);
        for i in 1..n {
            let m = xform::rotate_axis(p3(ctr), dir, step * i as f64);
            mats.push(if rotate.kw() == Some("No") {
                let moved = m3::apply(&m, centre);
                m3::translate(moved.x - centre.x, moved.y - centre.y, moved.z - centre.z)
            } else {
                m
            });
        }
    } else {
        let mut counts = [1usize; 3];
        for (i, label) in ["rows (---)", "columns (|||)", "levels (...)"]
            .iter()
            .enumerate()
        {
            let Some(v) = c
                .number(
                    ps(&format!("Enter the number of {label}"))
                        .integer()
                        .min(1.0)
                        .default_num(1.0),
                )
                .await?
                .value()
            else {
                return Ok(());
            };
            counts[i] = v as usize;
        }
        let mut gaps = [0.0f64; 3];
        for (i, label) in ["rows (---)", "columns (|||)", "levels (...)"]
            .iter()
            .enumerate()
        {
            if counts[i] > 1 {
                let Some(v) = c
                    .distance(ps(&format!("Specify the distance between {label}")).nonzero())
                    .await?
                    .value()
                else {
                    return Ok(());
                };
                gaps[i] = v;
            }
        }
        for lv in 0..counts[2] {
            for col in 0..counts[1] {
                for row in 0..counts[0] {
                    if (row, col, lv) != (0, 0, 0) {
                        mats.push(m3::translate(
                            col as f64 * gaps[1],
                            row as f64 * gaps[0],
                            lv as f64 * gaps[2],
                        ));
                    }
                }
            }
        }
    }
    let planar = mats
        .iter()
        .filter(|m| xform::to_plan_mat(m).is_none())
        .count();
    for m in &mats {
        apply_m3(&c, &sel, m, true);
    }
    if planar > 0 && !sel.ents.is_empty() {
        c.log(&format!(
            "{} not arrayed out of the drawing plane; only solids were copied there.",
            plural(sel.ents.len(), "drawing object")
        ));
    }
    c.log(&format!("{} created.", plural(mats.len(), "copy")));
    Ok(())
}

/// Centre of the bounding box of a mixed selection.
fn selection_centre(c: &Ctx, sel: &Selection) -> V3 {
    let (mut lo, mut hi) = (
        V3::new(f64::INFINITY, f64::INFINITY, 0.0),
        V3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, 0.0),
    );
    let mut grow = |p: V3| {
        lo = V3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = V3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    };
    for s in c.solids(&sel.solids) {
        s.vertices.iter().for_each(|p| grow(*p));
    }
    for e in c.entities(&sel.ents) {
        if let Some(b) = c.doc(|d| bounds_of(&e, d)) {
            grow(V3::new(b.min_x, b.min_y, 0.0));
            grow(V3::new(b.max_x, b.max_y, 0.0));
        }
    }
    V3::new(
        (lo.x + hi.x) / 2.0,
        (lo.y + hi.y) / 2.0,
        (lo.z + hi.z) / 2.0,
    )
}

// ---------------------------------------------------------------- FILLETEDGE, CHAMFEREDGE

type EdgeCache = HashMap<u64, (Topo, Vec<Edge>)>;

/// The sharp edge drawn under screen point `sp` on any visible solid: (solid id, edge index). Sharp edges are found
/// once per solid and kept in `cache`.
fn edge_at(c: &Ctx, cache: &mut EdgeCache, view: &View, sp: Pt) -> Option<(u64, usize)> {
    let ids: Vec<u64> = c.doc(|d| {
        d.solids
            .iter()
            .filter(|s| d.solid_visible(s))
            .map(|s| s.id)
            .collect()
    });
    for id in &ids {
        if !cache.contains_key(id) {
            if let Some(s) = c.solids(&[*id]).pop() {
                let topo = Topo::new(&s.mesh());
                let sharp = edges::feature_edges(&topo, 0.3);
                cache.insert(*id, (topo, sharp));
            }
        }
    }
    ids.iter()
        .filter_map(|id| {
            let (topo, sharp) = cache.get(id)?;
            edges::pick_edge(topo, sharp, view, sp, 12.0).map(|(e, depth)| (depth, *id, e))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|b| (b.1, b.2))
}

async fn ask_radius(c: &Ctx, radius: &mut f64) -> CmdResult<()> {
    if let Some(v) = c
        .distance(
            ps("Specify Fillet radius")
                .default_num(*radius)
                .nonzero()
                .min(0.0),
        )
        .await?
        .value()
    {
        *radius = v;
    }
    Ok(())
}

async fn ask_distances(c: &Ctx, d1: &mut f64, d2: &mut f64) -> CmdResult<()> {
    if let Some(v) = c
        .distance(ps("Specify Distance 1").default_num(*d1).nonzero().min(0.0))
        .await?
        .value()
    {
        *d1 = v;
    }
    if let Some(v) = c
        .distance(ps("Specify Distance 2").default_num(*d2).nonzero().min(0.0))
        .await?
        .value()
    {
        *d2 = v;
    }
    Ok(())
}

/// Pick sharp edges on solids and round them. Edges are picked with a point prompt: the sharp edge nearest the click
/// (drawn in front when several overlap) is chosen through the current view, together with the smoothly continuing
/// edges that form one curve with it (a circular rim is one edge).
async fn round_edges_cmd(c: Ctx, chamfer: bool) -> CmdResult<()> {
    let (mut radius, mut d1, mut d2) = (1.0, 1.0, 1.0);
    let mut next = "Edge";
    let mut cache = EdgeCache::new();
    // solid, chain, whether the second face is the one facing the viewer
    let mut picked: Vec<(u64, Chain, bool)> = vec![];
    c.log(&if chamfer {
        format!("Distance1 = {}, Distance2 = {}", f(&c, d1), f(&c, d2))
    } else {
        format!("Radius = {}", f(&c, radius))
    });
    'pick: loop {
        let opts: &[&str] = if chamfer {
            &["Loop", "Distance"]
        } else {
            &["Loop", "Radius"]
        };
        let r = c
            .point(
                ps(if chamfer {
                    "Select an edge on a solid or"
                } else {
                    "Select an edge or"
                })
                .options(opts)
                .allow_enter()
                .no_snap(),
            )
            .await?;
        match r {
            PointR::Kw(k) => match k.as_str() {
                "Loop" => {
                    next = "Loop";
                    c.log("The next edge picked selects the outline of the face in front of it.");
                }
                "Distance" => ask_distances(&c, &mut d1, &mut d2).await?,
                _ => ask_radius(&c, &mut radius).await?,
            },
            PointR::Pt(at) => {
                let view = view_of(&c);
                let Some((id, hit)) = edge_at(&c, &mut cache, &view, view.to_screen(at)) else {
                    c.log_error("No sharp edge was found at that point.");
                    continue;
                };
                let (topo, es) = &cache[&id];
                let seeds: Vec<usize> = if next == "Loop" {
                    edges::face_loop(topo, es, edges::front_face(topo, es, hit, &view))
                } else {
                    vec![hit]
                };
                next = "Edge";
                let toward = edges::toward_viewer(&view);
                let mut added = 0;
                for seed in seeds {
                    let chain = edges::chain_through(topo, es, seed, 0.45);
                    let same_chain = |o: &Chain| {
                        chain.pts.len() == o.pts.len()
                            && chain
                                .pts
                                .iter()
                                .all(|p| o.pts.iter().any(|q| q.sub(*p).len() < 1e-9))
                    };
                    if picked.iter().any(|(sid, o, _)| *sid == id && same_chain(o)) {
                        continue;
                    }
                    let swap = chain.n2[0].dot(toward) > chain.n1[0].dot(toward);
                    picked.push((id, chain, swap));
                    added += 1;
                }
                c.log(&format!(
                    "{} selected ({} total).",
                    plural(added, "edge"),
                    picked.len()
                ));
            }
            _ => {
                if picked.is_empty() {
                    return Ok(());
                }
                loop {
                    let k = c
                        .keyword(
                            ps(if chamfer {
                                "Press Enter to accept the chamfer or"
                            } else {
                                "Press Enter to accept the fillet or"
                            })
                            .options(if chamfer {
                                &["Distance"]
                            } else {
                                &["Radius"]
                            }),
                        )
                        .await?;
                    match k.kw() {
                        Some("Radius") => ask_radius(&c, &mut radius).await?,
                        Some("Distance") => ask_distances(&c, &mut d1, &mut d2).await?,
                        _ => break 'pick,
                    }
                }
            }
        }
    }
    let mut results: Vec<(u64, Mesh)> = vec![];
    let mut done: Vec<u64> = vec![];
    for (id, _, _) in &picked {
        if done.contains(id) {
            continue;
        }
        done.push(*id);
        let Some(s) = c.solids(&[*id]).pop() else {
            continue;
        };
        let jobs: Vec<(Chain, Round)> = picked
            .iter()
            .filter(|(sid, _, _)| sid == id)
            .map(|(_, ch, swap)| {
                (
                    ch.clone(),
                    if !chamfer {
                        Round::Fillet(radius)
                    } else if *swap {
                        Round::Chamfer(d2, d1)
                    } else {
                        Round::Chamfer(d1, d2)
                    },
                )
            })
            .collect();
        c.log("Computing...");
        results.push((*id, edges::round_edges(&s.mesh(), &jobs)?));
    }
    for (id, m) in results {
        set_mesh(&c, id, m);
    }
    c.log(&format!(
        "{} {}.",
        plural(picked.len(), "edge"),
        if chamfer { "chamfered" } else { "filleted" }
    ));
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- SOLIDEDIT, SHELL

type History = Vec<Vec<Solid>>;

fn remember(c: &Ctx, h: &mut History) {
    h.push(c.doc(|d| d.solids.clone()));
}

fn undo_edit(c: &Ctx, h: &mut History) {
    match h.pop() {
        Some(snap) => c.doc_mut(|d| d.change(|d| d.solids = snap)),
        None => c.log_error("Nothing to undo."),
    }
}

async fn pick_one_solid(c: &Ctx, prompt: &str) -> CmdResult<Option<Solid>> {
    Ok(match c.solid(ps(prompt).allow_enter()).await? {
        SolidR::Solid { id, .. } => c.solids(&[id]).pop(),
        _ => None,
    })
}

/// Choose faces to remove (planar faces under the pointer), then hollow the solid out.
async fn shell_solid(c: &Ctx, h: &mut History) -> CmdResult<()> {
    let Some(s) = pick_one_solid(c, "Select a 3D solid").await? else {
        return Ok(());
    };
    let m = s.mesh();
    let topo = Topo::new(&m);
    let mut faces: Vec<Vec<usize>> = vec![];
    loop {
        let r = c
            .point(
                ps("Remove faces or")
                    .options(&["Undo", "Add", "ALL"])
                    .allow_enter()
                    .no_snap(),
            )
            .await?;
        match r {
            PointR::Pt(at) => {
                let view = view_of(c);
                match repair::pick_face(&m, &view, view.to_screen(at)) {
                    Some(t) => {
                        faces.push(topo.planar_region(t));
                        c.log(&format!("{} removed.", plural(faces.len(), "face")));
                    }
                    None => c.log_error("No face of the solid was found at that point."),
                }
            }
            PointR::Kw(k) => match k.as_str() {
                "Undo" => {
                    faces.pop();
                }
                "ALL" => return Err("Removing every face leaves nothing to shell.".into()),
                _ => {}
            },
            _ => break,
        }
    }
    let Some(t) = c
        .distance(ps("Enter the shell offset distance").nonzero())
        .await?
        .value()
    else {
        return Ok(());
    };
    let open: Vec<usize> = faces.into_iter().flatten().collect();
    let hollow = repair::shell(&m, &open, t)?;
    remember(c, h);
    set_mesh(c, s.id, hollow);
    c.log("Shell complete.");
    Ok(())
}

async fn body_options(c: &Ctx, h: &mut History) -> CmdResult<()> {
    loop {
        let k = c
            .keyword(
                ps("Enter a body editing option")
                    .options(&[
                        "Imprint",
                        "seParate solids",
                        "Shell",
                        "cLean",
                        "Check",
                        "Undo",
                        "eXit",
                    ])
                    .default_str("eXit"),
            )
            .await?;
        match k.kw() {
            Some("Imprint") => c.log_error("Imprinting is not supported for mesh solids."),
            Some("seParate solids") => {
                let Some(s) = pick_one_solid(c, "Select a 3D solid").await? else {
                    continue;
                };
                let parts = repair::components(&s.mesh());
                if parts.len() < 2 {
                    c.log_error("The selected solid is a single body and cannot be separated.");
                    continue;
                }
                remember(c, h);
                c.doc_mut(|d| {
                    d.change(|d| {
                        d.remove_solid(s.id);
                        for p in &parts {
                            d.insert_solid_raw(derived(&s, p.clone()));
                        }
                    })
                });
                c.log(&format!(
                    "The solid was separated into {}.",
                    count_of(parts.len(), "body", "bodies")
                ));
            }
            Some("Shell") => shell_solid(c, h).await?,
            Some("cLean") => {
                let Some(s) = pick_one_solid(c, "Select a 3D solid").await? else {
                    continue;
                };
                let (m, rep) = repair::clean(&s.mesh());
                remember(c, h);
                set_mesh(c, s.id, m);
                c.log(&format!(
                    "Cleaned: {} merged, {} degenerate and {} duplicate faces removed.",
                    count_of(rep.merged_vertices, "vertex", "vertices"),
                    rep.degenerate_faces,
                    rep.duplicate_faces
                ));
            }
            Some("Check") => {
                let Some(s) = pick_one_solid(c, "Select a 3D solid").await? else {
                    continue;
                };
                let r = repair::check(&s.mesh());
                if r.valid() {
                    c.log("This object is a valid 3DSOLID.");
                } else {
                    c.log_error(&format!("This object is not a valid 3DSOLID ({} open edges, {} inconsistent edges, volume {}).", r.open_edges, r.bad_edges, f(c, r.volume)));
                }
            }
            Some("Undo") => undo_edit(c, h),
            _ => return Ok(()),
        }
    }
}

async fn solidedit(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut h: History = vec![];
    c.log("Solids editing automatic checking: SOLIDCHECK=1");
    loop {
        let k = c
            .keyword(
                ps("Enter a solids editing option")
                    .options(&["Face", "Edge", "Body", "Undo", "eXit"])
                    .default_str("eXit"),
            )
            .await?;
        match k.kw() {
            Some("Face") => loop {
                let o = c
                    .keyword(
                        ps("Enter a face editing option")
                            .options(&[
                                "Extrude", "Move", "Rotate", "Offset", "Taper", "Delete", "Copy",
                                "coLor", "mAterial", "Undo", "eXit",
                            ])
                            .default_str("eXit"),
                    )
                    .await?;
                match o.kw() {
                    Some("Undo") => undo_edit(&c, &mut h),
                    Some("eXit") | None => break,
                    Some(name) => c.log_error(&format!(
                        "Face option {name} is not supported for mesh solids."
                    )),
                }
            },
            Some("Edge") => loop {
                let o = c
                    .keyword(
                        ps("Enter an edge editing option")
                            .options(&["Copy", "coLor", "Undo", "eXit"])
                            .default_str("eXit"),
                    )
                    .await?;
                match o.kw() {
                    Some("Undo") => undo_edit(&c, &mut h),
                    Some("eXit") | None => break,
                    Some(name) => c.log_error(&format!(
                        "Edge option {name} is not supported for mesh solids."
                    )),
                }
            },
            Some("Body") => body_options(&c, &mut h).await?,
            Some("Undo") => undo_edit(&c, &mut h),
            _ => break,
        }
    }
    c.clear_sel();
    Ok(())
}

async fn shell_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    shell_solid(&c, &mut vec![]).await
}

// ---------------------------------------------------------------- THICKEN, CONVTOSOLID, INTERFERE, SOLIDHIST

/// Extrude each selected region or closed outline by a thickness.
async fn thicken_objects(c: &Ctx, prompt: &str, name: &str) -> CmdResult<()> {
    let sel = c
        .select(ps(prompt).filter(outline_ok).pickfirst(true))
        .await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let mem_h = c.app(|a| a.mem.solid.height);
    let Some(t) = c
        .distance(ps("Specify thickness").default_num(mem_h).nonzero())
        .await?
        .value()
    else {
        return Ok(());
    };
    c.app(|a| a.mem.solid.height = t);
    let mut made = 0;
    for id in &sel.ents {
        match outline_loops(c, &[*id]).and_then(|l| Ok(extrude_loops(&l, t, 0.0, name, 0.0)?)) {
            Ok(s) => {
                add_solid(c, s);
                made += 1;
            }
            Err(e) => c.log_error(&match e {
                CmdError::Msg(m) => m,
                CmdError::Cancel => "Cancelled.".into(),
            }),
        }
    }
    c.log(&format!("{} created.", plural(made, "solid")));
    c.clear_sel();
    Ok(())
}

async fn thicken(c: Ctx, _a: Args) -> CmdResult<()> {
    thicken_objects(&c, "Select surfaces to thicken", "Thicken").await
}

async fn convtosolid(c: Ctx, _a: Args) -> CmdResult<()> {
    c.log("Drawing objects carry no thickness property here: closed outlines and regions are converted with a thickness you specify.");
    thicken_objects(&c, "Select objects", "Solid").await
}

async fn interfere(c: Ctx, _a: Args) -> CmdResult<()> {
    let first = select_solids(&c, "Select first set of objects").await?;
    if first.is_empty() {
        return Ok(());
    }
    c.ed(|e| e.temp_sel = first.iter().copied().collect());
    let second = select_solids(
        &c,
        "Select second set of objects or [checK first set] <checK>",
    )
    .await?;
    let same_set = second.is_empty();
    let second = if same_set { first.clone() } else { second };
    let (mut pairs, mut hits): (Vec<(u64, u64)>, Vec<Mesh>) = (vec![], vec![]);
    let sols: HashMap<u64, Solid> = c
        .solids(
            &first
                .iter()
                .chain(second.iter())
                .copied()
                .collect::<Vec<_>>(),
        )
        .into_iter()
        .map(|s| (s.id, s))
        .collect();
    c.log("Computing...");
    for &a in &first {
        for &b in &second {
            if a == b || (same_set && a > b) {
                continue;
            }
            let (ma, mb) = (sols[&a].mesh(), sols[&b].mesh());
            let (ba, bb) = (mesh_bounds(&ma.vertices), mesh_bounds(&mb.vertices));
            if ba.min.x > bb.max.x
                || bb.min.x > ba.max.x
                || ba.min.y > bb.max.y
                || bb.min.y > ba.max.y
                || ba.min.z > bb.max.z
                || bb.min.z > ba.max.z
            {
                continue;
            }
            let common = boolean_sync(&ma, &mb, BoolOp::Intersect)?;
            if !common.faces.is_empty() && mesh_volume(&common) > 1e-9 {
                pairs.push((a, b));
                hits.push(common);
            }
        }
    }
    let count = |sides: &dyn Fn(&(u64, u64)) -> Vec<u64>| {
        let mut v: Vec<u64> = pairs.iter().flat_map(sides).collect();
        v.sort_unstable();
        v.dedup();
        v.len()
    };
    if same_set {
        c.log(&format!(
            "Interfering objects in first set: {}",
            count(&|p| vec![p.0, p.1])
        ));
    } else {
        c.log(&format!(
            "Interfering objects in first set: {}",
            count(&|p| vec![p.0])
        ));
        c.log(&format!(
            "Interfering objects in second set: {}",
            count(&|p| vec![p.1])
        ));
    }
    c.log(&format!("Number of interfering pairs: {}", pairs.len()));
    if pairs.is_empty() {
        return Ok(());
    }
    let make = c
        .keyword(
            ps("Create interference solids?")
                .options(&["Yes", "No"])
                .default_str("No"),
        )
        .await?;
    if make.kw() == Some("Yes") {
        for ((a, _), m) in pairs.iter().zip(hits) {
            let mut s = derived(&sols[a], m);
            s.name = "Interference".into();
            c.doc_mut(|d| d.insert_solid_raw(s));
        }
        c.log(&format!(
            "{} created.",
            plural(pairs.len(), "interference solid")
        ));
    }
    c.clear_sel();
    Ok(())
}

async fn solidhist(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = select_solids(&c, "Select solids").await?;
    if ids.is_empty() {
        return Ok(());
    }
    let has = c
        .solids(&ids)
        .iter()
        .filter(|s| s.program.is_some())
        .count();
    let k = c
        .keyword(
            ps("Enter history option")
                .options(&["Record", "None"])
                .default_str("None"),
        )
        .await?;
    if k.kw() == Some("Record") {
        c.log_error(if has > 0 { "The selected solids already record their extrusion history; history cannot be recorded for solids created another way." } else { "History is recorded only when a solid is created with EXTRUDE; it cannot be added to existing solids." });
        return Ok(());
    }
    c.doc_mut(|d| {
        d.change(|d| {
            for id in &ids {
                if let Some(s) = d.solid_by_id_mut(*id) {
                    s.program = None;
                }
            }
        })
    });
    c.log(&format!(
        "History removed from {} ({} had any).",
        plural(ids.len(), "solid"),
        has
    ));
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- FLATSHOT

async fn flatshot_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    let solids: Vec<Solid> = c.doc(|d| {
        d.solids
            .iter()
            .filter(|s| d.solid_visible(s))
            .cloned()
            .collect()
    });
    if solids.is_empty() {
        c.log_error("There are no 3D solids to flatten.");
        return Ok(());
    }
    let hidden = c
        .keyword(
            ps("Show obscured lines?")
                .options(&["Yes", "No"])
                .default_str("No"),
        )
        .await?;
    let Some(at) = c
        .point(ps("Specify insertion point for the flatshot block").default_pt(Pt::ZERO))
        .await?
        .pt()
    else {
        return Ok(());
    };
    let (yaw, pitch) = c.view(|v| (v.yaw, v.pitch));
    let meshes: Vec<Mesh> = solids.iter().map(|s| s.mesh()).collect();
    let lines = flatshot(&meshes.iter().collect::<Vec<_>>(), yaw, pitch, 0.35);
    let show_hidden = hidden.kw() == Some("Yes");
    let ents: Vec<Entity> = lines
        .iter()
        .filter(|l| l.visible || show_hidden)
        .map(|l| {
            if l.visible {
                Entity::line(l.a, l.b)
            } else {
                Entity::line(l.a, l.b).with_ltype("HIDDEN")
            }
        })
        .collect();
    if ents.is_empty() {
        c.log_error("Nothing to project.");
        return Ok(());
    }
    let name = c.doc(|d| {
        (1..)
            .map(|i| format!("FLATSHOT{i}"))
            .find(|n| !d.blocks.contains_key(n))
            .unwrap_or_default()
    });
    c.doc_mut(|d| d.define_block(&name, Pt::ZERO, ents.clone()));
    c.add(Entity::insert(&name, at));
    c.log(&format!(
        "Flat shot of {} created as block {name} ({}).",
        plural(solids.len(), "solid"),
        plural(ents.len(), "line")
    ));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(
        d3("REVOLVE", run(revolve))
            .aliases(&["REV"])
            .title("Revolve")
            .icon("extrude")
            .desc("Creates a 3D solid by sweeping a 2D outline about an axis."),
    );
    r.define(
        d3("TORUS", run(torus))
            .aliases(&["TOR"])
            .title("Torus")
            .icon("donut")
            .desc("Creates a doughnut-shaped 3D solid."),
    );
    r.define(
        d3("SWEEP", run(sweep_cmd))
            .title("Sweep")
            .icon("extrude")
            .desc("Creates a 3D solid by sweeping a 2D outline along an open or closed path."),
    );
    r.define(
        d3("LOFT", run(loft))
            .title("Loft")
            .icon("extrude")
            .desc("Creates a 3D solid through a set of cross-sections."),
    );
    r.define(
        d3("PYRAMID", run(pyramid))
            .aliases(&["PYR"])
            .title("Pyramid")
            .icon("cone")
            .desc("Creates a 3D solid pyramid or frustum with a regular base."),
    );
    r.define(
        d3("POLYSOLID", run(polysolid))
            .aliases(&["PSOLID"])
            .title("Polysolid")
            .icon("pline")
            .desc("Creates a 3D wall along a path, like a polyline with width and height."),
    );
    r.define(
        d3("SLICE", run(slice_cmd))
            .aliases(&["SL"])
            .title("Slice")
            .icon("subtract")
            .desc("Cuts solids with a plane and keeps one or both sides."),
    );
    r.define(
        d3("SECTION", run(section_cmd))
            .aliases(&["SEC"])
            .title("Section")
            .icon("view")
            .desc("Creates a region where a plane cuts 3D solids.")
            .once(),
    );
    r.define(
        d3("SECTIONPLANE", run(sectionplane))
            .aliases(&["SPLANE"])
            .title("Section Plane")
            .icon("view")
            .desc("Draws the outline where a plane cuts the solids, flat in the drawing."),
    );
    r.define(
        d3("3DROTATE", run(rotate3d_gizmo))
            .title("3D Rotate")
            .icon("rotate")
            .desc(
                "Rotates solids and drawing objects about the X, Y or Z axis through a base point.",
            ),
    );
    r.define(
        dm("ROTATE3D", run(rotate3d))
            .title("Rotate 3D")
            .icon("rotate")
            .desc("Moves objects about a 3D axis."),
    );
    r.define(
        dm("MIRROR3D", run(mirror3d))
            .aliases(&["3DMIRROR"])
            .title("3D Mirror")
            .icon("mirror")
            .desc("Creates a mirror image of objects about a plane."),
    );
    r.define(
        d3("3DSCALE", run(scale3d))
            .title("3D Scale")
            .icon("scale")
            .desc("Scales solids and drawing objects, uniformly or along chosen axes."),
    );
    r.define(
        d3("3DALIGN", run(align3d))
            .title("3D Align")
            .icon("move")
            .desc("Aligns objects with other objects using up to three point pairs."),
    );
    r.define(
        d3("3DARRAY", run(array3d))
            .aliases(&["3A"])
            .title("3D Array")
            .icon("array")
            .desc("Creates a three-dimensional rectangular or polar array."),
    );
    r.define(
        d3("FILLETEDGE", run(|c, _a| round_edges_cmd(c, false)))
            .title("Fillet Edge")
            .icon("fillet")
            .desc("Rounds the sharp edges of solids."),
    );
    r.define(
        d3("CHAMFEREDGE", run(|c, _a| round_edges_cmd(c, true)))
            .title("Chamfer Edge")
            .icon("chamfer")
            .desc("Bevels the sharp edges of solids."),
    );
    r.define(
        d3("SOLIDEDIT", run(solidedit))
            .title("Solid Editing")
            .icon("properties")
            .desc("Edits solids: body options shell, separate, clean and check."),
    );
    r.define(
        d3("SHELL", run(shell_cmd))
            .title("Shell")
            .icon("box")
            .desc("Hollows a solid out, leaving openings where faces are removed."),
    );
    r.define(d3("THICKEN", run(thicken)).title("Thicken").icon("presspull").desc("Turns planar surfaces (regions and closed outlines) into solids of a given thickness."));
    r.define(
        d3("CONVTOSOLID", run(convtosolid))
            .title("Convert to Solid")
            .icon("extrude")
            .desc("Converts closed outlines and regions into solids."),
    );
    r.define(
        d3("INTERFERE", run(interfere))
            .aliases(&["INF"])
            .title("Interference Checking")
            .icon("intersect")
            .desc("Finds solids that overlap and can create their common volumes."),
    );
    r.define(
        d3("SOLIDHIST", run(solidhist))
            .title("Solid History")
            .icon("list")
            .desc("Removes the recorded extrusion history from solids."),
    );
    r.define(
        d3("FLATSHOT", run(flatshot_cmd))
            .title("Flatshot")
            .icon("view")
            .desc("Creates a 2D drawing of the 3D solids as seen in the current view."),
    );
}
