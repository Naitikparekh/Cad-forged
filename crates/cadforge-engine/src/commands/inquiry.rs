//! Inquiry commands: DIST, AREA, LIST, ID, MEASUREGEOM (port of `src/commands/inquiry.js`).
use super::helpers::*;
use super::{def, run};
use crate::app::Event;
use crate::ctx::*;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::solid::mesh::mesh_volume;
use cadforge_core::style::{describe_color, fmt_num, lw_display};

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn di(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("Inquiry", name, f).no_pickfirst()
}

fn prec(c: &Ctx) -> usize {
    c.doc(|d| d.settings.units.precision.max(0) as usize)
}

fn f(c: &Ctx, v: f64) -> String {
    fmt_num(v, prec(c), false)
}

fn ang_text(c: &Ctx, a: f64) -> String {
    let p = c.doc(|d| d.settings.units.ang_precision);
    fmt_num(deg(norm_angle(a)), if p == 0 { 4 } else { p.max(0) as usize }, false)
}

async fn dist_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(a) = c.point(ps("Specify first point")).await?.pt() else { return Ok(()) };
    let Some(b) = c.point(ps("Specify second point or [Multiple points]").options(&["Multiple points"]).base(a).preview(move |_, p| vec![pv(Entity::line(a, p))])).await?.pt() else { return Ok(()) };
    let d = b - a;
    c.log(&format!("Distance = {},  Angle in XY Plane = {},  Angle from XY Plane = 0", f(&c, len(d)), ang_text(&c, d.y.atan2(d.x))));
    c.log(&format!("  Delta X = {},  Delta Y = {},  Delta Z = {}", f(&c, d.x), f(&c, d.y), f(&c, 0.0)));
    c.app(|a| a.mem.last_distance = len(d));
    Ok(())
}

/// Area and perimeter of a closed entity.
pub fn entity_area(e: &Entity, doc: &Drawing) -> Option<(f64, f64)> {
    match &e.kind {
        Kind::Circle { r, .. } => Some((PI * r * r, 2.0 * PI * r)),
        Kind::Ellipse(el) => {
            if ellipse_sweep(el) < TAU - 1e-9 {
                return None;
            }
            let a = len(el.m);
            let b = a * el.ratio;
            let h = ((a - b) / (a + b)).powi(2);
            Some((PI * a * b, PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()))))
        }
        Kind::Pline { v, closed } => Some((pline_area(v).abs(), pline_length(v, *closed))),
        Kind::Spline { .. } => {
            let segs = segs_of(e, doc);
            let pts: Vec<Pt> = segs.iter().map(|s| s.start()).collect();
            Some((polygon_area(&pts).abs(), segs.iter().map(|s| s.length()).sum()))
        }
        Kind::Hatch(h) => {
            let (mut a, mut p) = (0.0, 0.0);
            for (i, l) in h.loops.iter().enumerate() {
                a += (if i == 0 { 1.0 } else { -1.0 }) * pline_area(l).abs();
                p += pline_length(l, true);
            }
            Some((a, p))
        }
        _ => None,
    }
}

async fn area(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut total = 0.0;
    let r0 = c.point(ps("Specify first corner point or").options(&["Object", "Add area", "Subtract area", "eXit"])).await?;
    if let Some(k) = r0.kw().filter(|k| ["Object", "Add area", "Subtract area"].contains(k)) {
        let sign = if k == "Subtract area" { -1.0 } else { 1.0 };
        loop {
            let doc_ok = move |e: &Entity| matches!(e.kind, Kind::Circle { .. } | Kind::Ellipse(_) | Kind::Pline { .. } | Kind::Spline { .. } | Kind::Hatch(_));
            let o = c.entity(ps("Select objects").allow_enter().filter(doc_ok)).await?;
            let Some(id) = o.id() else { break };
            let Some(e) = c.entity_by_id(id) else { break };
            let Some((a, p)) = c.doc(|d| entity_area(&e, d)) else { continue };
            total += sign * a;
            c.log(&format!("Area = {},  {} = {}", f(&c, a), if matches!(e.kind, Kind::Circle { .. }) { "Circumference" } else { "Perimeter" }, f(&c, p)));
            if k != "Object" {
                c.log(&format!("Total area = {}", f(&c, total)));
            }
        }
        return Ok(());
    }
    let Some(p0) = r0.pt() else { return Ok(()) };
    let mut pts = vec![p0];
    loop {
        let snapshot = pts.clone();
        let r = c
            .point(ps("Specify next corner point or press ENTER for total").base(*pts.last().expect("a point")).allow_enter().preview(move |_, p| {
                let mut v = snapshot.clone();
                v.push(p);
                vec![pv(Entity::pline_pts(&v, true))]
            }))
            .await?;
        match r.pt() {
            Some(p) => pts.push(p),
            None => break,
        }
    }
    if pts.len() < 3 {
        c.log_error("At least three points are required.");
        return Ok(());
    }
    let a = polygon_area(&pts).abs();
    let per = pline_length(&pts.iter().map(|p| Vertex::new(p.x, p.y, 0.0)).collect::<Vec<_>>(), true);
    c.log(&format!("Area = {},  Perimeter = {}", f(&c, a), f(&c, per)));
    Ok(())
}

fn describe(c: &Ctx, e: &Entity) -> String {
    let (is_model, ) = c.doc(|d| (d.is_model(), ));
    let mut out: Vec<String> = vec![];
    out.push(format!("{:<14} Layer: \"{}\"", e.label().to_uppercase(), e.layer));
    out.push(format!("                Space: {}", if is_model { "Model space" } else { "Paper space" }));
    out.push(format!("                Handle = {:X}", e.id));
    if e.color.is_some() {
        out.push(format!("                Color: {}", describe_color(e.color)));
    }
    if let Some(l) = &e.ltype {
        out.push(format!("                Linetype: \"{l}\""));
    }
    if e.lw.is_some() {
        out.push(format!("                Lineweight: {}", lw_display(e.lw)));
    }
    let pp = |p: Pt| format!("X={:>10}  Y={:>10}  Z={:>10}", f(c, p.x), f(c, p.y), f(c, 0.0));
    match &e.kind {
        Kind::Line { a, b } => {
            out.push(format!("      from point, {}", pp(*a)));
            out.push(format!("        to point, {}", pp(*b)));
            out.push(format!("  Length =  {},  Angle in XY Plane =  {}", f(c, dist(*a, *b)), ang_text(c, angle_of(*a, *b))));
            out.push(format!("       Delta X =  {},  Delta Y =  {},  Delta Z =  {}", f(c, b.x - a.x), f(c, b.y - a.y), f(c, 0.0)));
        }
        Kind::Circle { c: cc, r } => {
            out.push(format!("       center point, {}", pp(*cc)));
            out.push(format!("  radius  {}", f(c, *r)));
            out.push(format!("  circumference  {}", f(c, 2.0 * PI * r)));
            out.push(format!("  area  {}", f(c, PI * r * r)));
        }
        Kind::Arc { c: cc, r, a0, a1 } => {
            let sw = { let s = sweep_ccw(*a0, *a1); if s == 0.0 { TAU } else { s } };
            out.push(format!("       center point, {}", pp(*cc)));
            out.push(format!("  radius  {}", f(c, *r)));
            out.push(format!("  start angle  {}", ang_text(c, *a0)));
            out.push(format!("  end angle  {}", ang_text(c, *a1)));
            out.push(format!("  length  {}", f(c, r * sw)));
        }
        Kind::Pline { v, closed } => {
            out.push(format!("  {}", if *closed { "Closed" } else { "Open" }));
            out.push(format!("  Area  {}", f(c, pline_area(v).abs())));
            out.push(format!("  Perimeter  {}", f(c, pline_length(v, *closed))));
            for q in v {
                out.push(format!("  at point  {}{}", pp(q.pt()), if q.b != 0.0 { format!("  bulge {}", num(q.b, 4)) } else { String::new() }));
            }
        }
        Kind::Ellipse(el) => {
            out.push(format!("       center point, {}", pp(el.c)));
            out.push(format!("  major axis  {}", f(c, len(el.m) * 2.0)));
            out.push(format!("  minor axis  {}", f(c, len(el.m) * 2.0 * el.ratio)));
        }
        Kind::Text(t) => {
            out.push("  Style = \"Standard\"".into());
            out.push("  Annotative: No".into());
            out.push(format!("  Height  {}", f(c, t.h)));
            out.push(format!("  Text  {}", t.s));
            out.push(format!("  insertion point, {}", pp(t.p)));
        }
        Kind::Insert(i) => {
            out.push(format!("  \"{}\"", i.name));
            out.push(format!("  at point, {}", pp(i.p)));
            out.push(format!("  X scale factor  {}", f(c, i.sx)));
            out.push(format!("  Y scale factor  {}", f(c, i.sy)));
            out.push(format!("  rotation angle  {}", ang_text(c, i.rot)));
        }
        Kind::Point { p } => out.push(format!("  at point, {}", pp(*p))),
        Kind::Hatch(h) => {
            let a = c.doc(|d| entity_area(e, d)).map_or(0.0, |x| x.0);
            out.push(format!("  Pattern name  {}", h.pattern));
            out.push(format!("  Scale  {}", f(c, h.ps)));
            out.push(format!("  Angle  {}", ang_text(c, h.pa)));
            out.push(format!("  Area  {}", f(c, a)));
        }
        Kind::Dim(d) => out.push(format!("  Dimension type {}", format!("{:?}", d.kind).to_lowercase())),
        _ => {}
    }
    out.join("\n")
}

async fn list(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    for e in c.entities(&sel.ents) {
        let t = describe(&c, &e);
        c.log(&t);
    }
    c.app(|a| a.emit(Event::ShowHistory));
    c.clear_sel();
    Ok(())
}

async fn id(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(p) = c.point(ps("Specify point")).await?.pt() else { return Ok(()) };
    c.log(&format!("X = {}  Y = {}  Z = {}", f(&c, p.x), f(&c, p.y), f(&c, 0.0)));
    c.app(|a| a.rt.last_point = Some(p));
    Ok(())
}

async fn measuregeom(c: Ctx, _a: Args) -> CmdResult<()> {
    let k = c.keyword(ps("Enter an option [Distance/Radius/Angle/ARea/Volume/Quick]").options(&["Distance", "Radius", "Angle", "ARea", "Volume", "Quick"]).default_str("Distance")).await?;
    match k.kw() {
        Some("Radius") => {
            let Some(oid) = c.pick_entity(ps("Select arc or circle").filter(|e| matches!(e.kind, Kind::Circle { .. } | Kind::Arc { .. }))).await? else { return Ok(()) };
            if let Some(Entity { kind: Kind::Circle { r, .. } | Kind::Arc { r, .. }, .. }) = c.entity_by_id(oid) {
                c.log(&format!("Radius = {}", f(&c, r)));
                c.log(&format!("Diameter = {}", f(&c, r * 2.0)));
            }
        }
        Some("ARea") => c.sub_forced("AREA", Args::none()).await,
        Some("Angle") => {
            let a = c.entity(ps("Select arc, circle, line, or <specify vertex>").filter(|e| matches!(e.kind, Kind::Line { .. } | Kind::Arc { .. }))).await?;
            let Some(ae) = a.id().and_then(|i| c.entity_by_id(i)) else { return Ok(()) };
            if let Kind::Arc { a0, a1, .. } = ae.kind {
                c.log(&format!("Angle = {}", ang_text(&c, sweep_ccw(a0, a1))));
                return Ok(());
            }
            let Some(bid) = c.pick_entity(ps("Select second line").filter(|e| matches!(e.kind, Kind::Line { .. }))).await? else { return Ok(()) };
            if let (Kind::Line { a: a1, b: b1 }, Some(Entity { kind: Kind::Line { a: a2, b: b2 }, .. })) = (ae.kind, c.entity_by_id(bid)) {
                let (d1, d2) = (b1 - a1, b2 - a2);
                c.log(&format!("Angle = {}", ang_text(&c, dot(unit(d1), unit(d2)).abs().min(1.0).acos())));
            }
        }
        Some("Volume") => {
            let v: Option<f64> = c.doc(|d| if d.solids.is_empty() { None } else { Some(d.solids.iter().map(|s| mesh_volume(&s.mesh()).abs()).sum()) });
            match v {
                None => c.log_error("No solids in the drawing."),
                Some(v) => c.log(&format!("Volume = {}", f(&c, v))),
            }
        }
        _ => c.sub_forced("DIST", Args::none()).await,
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(di("DIST", run(dist_cmd)).aliases(&["DI"]).title("Distance").icon("dist").desc("Measures the distance and angle between two points."));
    r.define(di("AREA", run(area)).aliases(&["AA"]).title("Area").icon("area").desc("Calculates the area and perimeter of objects or of defined areas."));
    r.define(di("LIST", run(list)).aliases(&["LI", "LS"]).title("List").icon("list").desc("Displays property data for selected objects."));
    // LIST honours pick-first selection
    if let Some(d) = r.get("LIST") {
        let mut d2 = (*d).clone();
        d2.pickfirst = true;
        r.define(d2);
    }
    r.define(di("ID", run(id)).title("ID Point").icon("idpoint").desc("Displays the UCS coordinate values of a specified location."));
    r.define(di("MEASUREGEOM", run(measuregeom)).aliases(&["MEA"]).title("Measure").icon("dist").desc("Measures distances, radii, angles, areas, and volumes of selected objects."));
}
