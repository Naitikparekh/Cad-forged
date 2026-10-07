//! 3D solid commands: BOX, CYLINDER, SPHERE, CONE, WEDGE, EXTRUDE, PRESSPULL, UNION, SUBTRACT, INTERSECT, 3DMOVE,
//! MASSPROP, UPDATEEXTRUSION, VSCURRENT, OBJOUT/STLOUT (port of `src/commands/solids.js`).
use super::helpers::*;
use super::{def, run};
use crate::app::Event;
use crate::ctx::*;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::boundary::find_region;
use cadforge_core::solid::boolean::{boolean_sync, BoolOp};
use cadforge_core::solid::mesh::*;
use cadforge_core::solid::*;
use cadforge_core::style::fmt_num;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn d3(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("3D", name, f).no_pickfirst()
}

fn f(c: &Ctx, v: f64) -> String {
    fmt_num(v, c.doc(|d| d.settings.units.precision.max(0) as usize), false)
}

fn add_solid(c: &Ctx, s: Solid) -> u64 {
    c.doc_mut(|d| d.add_solid(s))
}

/// Prompt for solids (clicks and windows); returns their ids.
async fn select_solids(c: &Ctx, prompt: &str) -> CmdResult<Vec<u64>> {
    Ok(c.select(ps(prompt).solids().filter(|_| false).pickfirst(true)).await?.solids)
}

fn rect_preview(a: Pt, b: Pt) -> Vec<PreviewItem> {
    vec![pv(Entity::pline_pts(&[pt(a.x, a.y), pt(b.x, a.y), pt(b.x, b.y), pt(a.x, b.y)], true))]
}

struct Footprint {
    x0: f64,
    y0: f64,
    w: f64,
    d: f64,
    h: Option<f64>,
    b: Option<Pt>,
}

/// Two-corner footprint shared by BOX and WEDGE.
async fn box_footprint(c: &Ctx) -> CmdResult<Option<Footprint>> {
    let len_mem = c.app(|a| a.mem.solid.length);
    let mut r = c.point(ps("Specify first corner").options(&["Center"])).await?;
    let mut centre = false;
    if r.kw() == Some("Center") {
        centre = true;
        r = c.point(ps("Specify center")).await?;
    }
    let Some(a) = r.pt() else { return Ok(None) };
    loop {
        let mut spec = ps("Specify other corner").options(&["Cube", "Length"]);
        if centre {
            spec = spec.preview(move |_, p| rect_preview(a - (p - a), p));
        }
        let o = c.corner(a, spec).await?;
        match o.kw() {
            Some("Cube") => {
                let l = c.distance(ps("Specify length").base(a).default_num(len_mem).nonzero()).await?;
                let Some(lv) = l.value() else { return Ok(None) };
                c.app(|app| app.mem.solid.length = lv);
                return Ok(Some(if centre { Footprint { x0: a.x - lv / 2.0, y0: a.y - lv / 2.0, w: lv, d: lv, h: Some(lv), b: None } } else { Footprint { x0: a.x, y0: a.y, w: lv, d: lv, h: Some(lv), b: None } }));
            }
            Some("Length") => {
                let l = c.distance(ps("Specify length").base(a).default_num(len_mem).nonzero()).await?;
                let Some(lv) = l.value() else { return Ok(None) };
                let w = c.distance(ps("Specify width").base(a).default_num(len_mem).nonzero()).await?;
                let Some(wv) = w.value() else { return Ok(None) };
                c.app(|app| app.mem.solid.length = lv);
                return Ok(Some(if centre { Footprint { x0: a.x - lv / 2.0, y0: a.y - wv / 2.0, w: lv, d: wv, h: None, b: None } } else { Footprint { x0: a.x, y0: a.y, w: lv, d: wv, h: None, b: None } }));
            }
            _ => {}
        }
        let Some(b) = o.pt() else { return Ok(None) };
        if centre {
            let (dx, dy) = ((b.x - a.x).abs(), (b.y - a.y).abs());
            return Ok(Some(Footprint { x0: a.x - dx, y0: a.y - dy, w: 2.0 * dx, d: 2.0 * dy, h: None, b: None }));
        }
        return Ok(Some(Footprint { x0: a.x.min(b.x), y0: a.y.min(b.y), w: (b.x - a.x).abs(), d: (b.y - a.y).abs(), h: None, b: Some(b) }));
    }
}

async fn height_prompt(c: &Ctx, base: Pt, label: &str) -> CmdResult<Option<f64>> {
    let mem_h = c.app(|a| a.mem.solid.height);
    let h = c.distance(ps(label).options(&["2Point"]).base(base).default_num(mem_h).nonzero()).await?;
    if h.kw() == Some("2Point") {
        let q = c.distance(ps("Specify first point")).await?;
        return Ok(q.value());
    }
    Ok(h.value())
}

// ---------------------------------------------------------------- primitives

async fn box_(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(fp) = box_footprint(&c).await? else { return Ok(()) };
    if fp.w < 1e-9 || fp.d < 1e-9 {
        return Err("The box has zero width or depth.".into());
    }
    let h = match fp.h {
        Some(h) => h,
        None => {
            let Some(h) = height_prompt(&c, fp.b.unwrap_or(pt(fp.x0, fp.y0)), "Specify height").await? else { return Ok(()) };
            c.app(|a| a.mem.solid.height = h);
            h
        }
    };
    add_solid(&c, box_mesh(fp.x0, fp.y0, if h < 0.0 { h } else { 0.0 }, fp.w, fp.d, h.abs(), "Box"));
    c.log(&format!("Box {} x {} x {} created.", f(&c, fp.w), f(&c, fp.d), f(&c, h.abs())));
    Ok(())
}

async fn wedge(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(fp) = box_footprint(&c).await? else { return Ok(()) };
    if fp.w < 1e-9 || fp.d < 1e-9 {
        return Err("The wedge has zero width or depth.".into());
    }
    let h = match fp.h {
        Some(h) => h,
        None => {
            let Some(h) = height_prompt(&c, fp.b.unwrap_or(pt(fp.x0, fp.y0)), "Specify height").await? else { return Ok(()) };
            c.app(|a| a.mem.solid.height = h);
            h
        }
    };
    add_solid(&c, wedge_mesh(fp.x0, fp.y0, if h < 0.0 { h } else { 0.0 }, fp.w, fp.d, h.abs(), "Wedge"));
    Ok(())
}

async fn radius_prompt(c: &Ctx, centre: Pt, label: &str) -> CmdResult<Option<f64>> {
    let mem_r = c.app(|a| a.mem.solid.radius);
    let r = c
        .distance(ps(label).options(&["Diameter"]).base(centre).default_num(mem_r).nonzero().min(0.0).preview(move |_, p| vec![pv(Entity::circle(centre, dist(centre, p).max(1e-9)))]))
        .await?;
    if r.kw() == Some("Diameter") {
        let d = c.distance(ps("Specify diameter").base(centre).default_num(mem_r * 2.0).nonzero().min(0.0)).await?;
        return Ok(d.value().map(|v| v / 2.0));
    }
    Ok(r.value())
}

fn segments_for(r: f64) -> usize {
    (24.0 + r.max(0.0).sqrt() * 8.0).round().clamp(32.0, 96.0) as usize
}

async fn cylinder(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(ctr) = c.point(ps("Specify center point of base")).await?.pt() else { return Ok(()) };
    let Some(r) = radius_prompt(&c, ctr, "Specify base radius").await? else { return Ok(()) };
    c.app(|a| a.mem.solid.radius = r);
    let Some(h) = height_prompt(&c, ctr, "Specify height").await? else { return Ok(()) };
    c.app(|a| a.mem.solid.height = h);
    add_solid(&c, cylinder_mesh(ctr.x, ctr.y, if h < 0.0 { h } else { 0.0 }, r, h.abs(), segments_for(r), "Cylinder"));
    Ok(())
}

async fn cone(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(ctr) = c.point(ps("Specify center point of base")).await?.pt() else { return Ok(()) };
    let Some(r) = radius_prompt(&c, ctr, "Specify base radius").await? else { return Ok(()) };
    c.app(|a| a.mem.solid.radius = r);
    let mem_h = c.app(|a| a.mem.solid.height);
    let mut top = 0.0;
    let h: Option<f64>;
    loop {
        let k = c.distance(ps("Specify height").options(&["2Point", "Top radius"]).base(ctr).default_num(mem_h).nonzero()).await?;
        match k.kw() {
            Some("Top radius") => {
                let t = c.distance(ps("Specify top radius").base(ctr).default_num(0.0).min(0.0)).await?;
                if let Some(v) = t.value() {
                    top = v;
                }
                continue;
            }
            Some("2Point") => {
                h = c.distance(ps("Specify first point")).await?.value();
                break;
            }
            _ => {
                h = k.value();
                break;
            }
        }
    }
    let Some(h) = h else { return Ok(()) };
    c.app(|a| a.mem.solid.height = h);
    add_solid(&c, cone_mesh(ctr.x, ctr.y, if h < 0.0 { h } else { 0.0 }, r, top, h.abs(), segments_for(r.max(top)), if top > 0.0 { "Frustum" } else { "Cone" }));
    Ok(())
}

async fn sphere(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(ctr) = c.point(ps("Specify center point")).await?.pt() else { return Ok(()) };
    let Some(r) = radius_prompt(&c, ctr, "Specify radius").await? else { return Ok(()) };
    c.app(|a| a.mem.solid.radius = r);
    add_solid(&c, sphere_mesh(ctr.x, ctr.y, 0.0, r, 48, "Sphere"));
    Ok(())
}

// ---------------------------------------------------------------- extrusion

fn extrudable(e: &Entity) -> bool {
    matches!(e.kind, Kind::Pline { .. } | Kind::Circle { .. } | Kind::Ellipse(_) | Kind::Spline { .. } | Kind::Line { .. } | Kind::Arc { .. })
}

/// Solid for one extrusion feature (entities looked up by id so the source outline stays editable).
pub fn feature_mesh(doc: &Drawing, ft: &Feature) -> Result<Solid, String> {
    let ents: Vec<Entity> = ft.profile.iter().filter_map(|id| doc.by_id(*id).cloned()).collect();
    if ents.len() != ft.profile.len() {
        return Err("The source outline of this extrusion was deleted. Draw it again and use EXTRUDE.".into());
    }
    let refs: Vec<&Entity> = ents.iter().collect();
    let pr = profile_loops(&refs, doc, 0.02);
    if let Some(e) = pr.error {
        return Err(e);
    }
    extrude_loops(&pr.loops, ft.height, ft.start_z, "Extrusion", ft.taper)
}

fn apply_feature(cur: Option<Mesh>, mesh: &Solid, op: &str) -> Result<Mesh, String> {
    match (op, cur) {
        ("new", _) | (_, None) => Ok(mesh.mesh()),
        (o, Some(cur)) => boolean_sync(&cur, &mesh.mesh(), if o == "add" { BoolOp::Union } else { BoolOp::Subtract }),
    }
}

fn replace_solid(c: &Ctx, id: u64, mesh: Mesh, program: Option<Program>) {
    let empty = mesh.faces.is_empty();
    c.doc_mut(|d| {
        d.change(|d| {
            if empty {
                d.remove_solid(id);
            } else if let Some(s) = d.solid_by_id_mut(id) {
                s.vertices = mesh.vertices;
                s.faces = mesh.faces;
                if let Some(p) = program {
                    s.program = Some(p);
                }
            }
        })
    });
    if empty {
        c.log("No solid remains.");
    }
}

async fn extrude(c: Ctx, args: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects to extrude").filter(extrudable).pickfirst(true)).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let ents = c.entities(&sel.ents);
    let refs: Vec<&Entity> = ents.iter().collect();
    let pr = c.doc(|d| profile_loops(&refs, d, 0.02));
    if let Some(e) = pr.error {
        return Err(e.into());
    }
    let mut taper = 0.0;
    let mut start = 0.0;
    let mut op = args.str("op").unwrap_or("new").to_string();
    let mem_h = c.app(|a| a.mem.solid.height);
    let h;
    loop {
        let k = c.distance(ps("Specify height of extrusion").options(&["Taper angle", "Start elevation", "Operation"]).default_num(mem_h).nonzero()).await?;
        match k.kw() {
            Some("Taper angle") => {
                let t = c.angle(ps("Specify angle of taper for extrusion").default_num(0.0)).await?;
                if let Some(v) = t.value() {
                    taper = deg(v);
                }
                continue;
            }
            Some("Start elevation") => {
                let z = c.number(ps("Specify start elevation").default_num(start)).await?;
                if let Some(v) = z.value() {
                    start = v;
                }
                continue;
            }
            Some("Operation") => {
                let o = c.keyword(ps("Combine with an existing solid").options(&["New", "Add", "Subtract"]).default_str("New")).await?;
                op = o.kw().unwrap_or("New").to_lowercase();
                continue;
            }
            _ => {
                h = k.value();
                break;
            }
        }
    }
    let Some(h) = h else { return Ok(()) };
    if taper.abs() >= 89.0 {
        return Err("The taper angle must be between -89 and 89 degrees.".into());
    }
    c.app(|a| a.mem.solid.height = h);
    let feature = Feature { op: op.clone(), profile: sel.ents.clone(), height: h, start_z: start, taper };
    let mesh = extrude_loops(&pr.loops, h, start, "Extrusion", taper)?;
    if op == "new" {
        let mut s = mesh.clone();
        s.program = Some(Program { features: vec![feature], base: None });
        add_solid(&c, s);
        c.log(&format!("Extruded solid created (volume {}).", f(&c, mesh_volume(&mesh.mesh()))));
        return Ok(());
    }
    let t = c.select(ps("Select the solid to modify").solids().filter(|_| false).pickfirst(false)).await?;
    let Some(&tid) = t.solids.first() else { return Ok(()) };
    let Some(target) = c.doc(|d| d.solid_by_id(tid).cloned()) else { return Ok(()) };
    c.log(if op == "add" { "Computing union..." } else { "Computing difference..." });
    let res = boolean_sync(&target.mesh(), &mesh.mesh(), if op == "add" { BoolOp::Union } else { BoolOp::Subtract })?;
    let base = match &target.program {
        Some(p) => p.base.clone(),
        None => Some(target.mesh()),
    };
    let mut features = target.program.as_ref().map(|p| p.features.clone()).unwrap_or_default();
    features.push(feature);
    replace_solid(&c, tid, res, Some(Program { features, base }));
    Ok(())
}

async fn updateextrusion(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = select_solids(&c, "Select extruded solids to update").await?;
    let sols: Vec<Solid> = c.solids(&ids).into_iter().filter(|s| s.program.is_some()).collect();
    if sols.is_empty() {
        c.log_error("The selected solids have no extrusion history.");
        return Ok(());
    }
    for s in &sols {
        let prog = s.program.clone().expect("program");
        let mut cur: Option<Mesh> = prog.base.clone();
        for ft in &prog.features {
            let m = c.doc(|d| feature_mesh(d, ft))?;
            cur = Some(apply_feature(cur, &m, &ft.op)?);
        }
        replace_solid(&c, s.id, cur.unwrap_or_default(), Some(prog));
    }
    c.log(&format!("{} solid{} updated.", sols.len(), if sols.len() == 1 { "" } else { "s" }));
    c.clear_sel();
    Ok(())
}

fn tess_loop(l: &[Vertex]) -> Vec<Pt> {
    let mut pts: Vec<Pt> = vec![];
    for p in tess_segs(&pline_segs(l, true), 0.02) {
        if pts.last().map_or(true, |q| dist(p, *q) > 1e-9) {
            pts.push(pt(p.x, p.y));
        }
    }
    if pts.len() > 1 && dist(pts[0], pts[pts.len() - 1]) < 1e-9 {
        pts.pop();
    }
    pts
}

async fn presspull(c: Ctx, _a: Args) -> CmdResult<()> {
    let mem_h = c.app(|a| a.mem.solid.height);
    let Some(p) = c.point(ps("Click inside bounded areas to press or pull")).await?.pt() else { return Ok(()) };
    let reg = c.doc(|d| find_region(d, p, None));
    let reg = match reg {
        Ok(r) => r,
        Err(m) => {
            let hit = c.app(|a| {
                let sp = a.editor().map(|e| e.view.to_screen(p));
                a.solid_at(p, &Mods { screen: sp, ..Mods::default() }).is_some()
            });
            return Err(if hit { "Pressing faces of existing solids is not supported; use EXTRUDE with Add or Subtract.".into() } else { m.into() });
        }
    };
    let loops: Vec<Vec<Pt>> = reg.loops.iter().map(|l| tess_loop(l)).collect();
    let h = c.distance(ps("Specify extrusion height").default_num(mem_h).nonzero().base(p)).await?;
    let Some(hv) = h.value() else { return Ok(()) };
    c.app(|a| a.mem.solid.height = hv);
    add_solid(&c, extrude_loops(&loops, hv, 0.0, "Presspull", 0.0)?);
    Ok(())
}

// ---------------------------------------------------------------- booleans

fn finish_boolean(c: &Ctx, operands: &[u64], result: Mesh, name: &str) {
    let first = c.doc(|d| d.solid_by_id(operands[0]).cloned());
    let nonempty = !result.faces.is_empty();
    let mut vol = 0.0;
    c.doc_mut(|d| {
        d.change(|d| {
            for id in operands {
                d.remove_solid(*id);
            }
            if nonempty {
                let mut s = Solid::from_mesh(name, result.clone());
                if let Some(fs) = &first {
                    s.layer = fs.layer.clone();
                    s.color = fs.color;
                }
                vol = mesh_volume(&s.mesh());
                d.insert_solid_raw(s);
            }
        })
    });
    if nonempty {
        c.log(&format!("{name} complete (volume {}).", f(c, vol)));
    } else {
        c.log("The result is empty; no solid remains.");
    }
    c.clear_sel();
}

async fn combine(c: Ctx, op: BoolOp, label: &str) -> CmdResult<()> {
    let ids = select_solids(&c, &format!("Select solids to {label}")).await?;
    if ids.len() < 2 {
        c.log_error(&format!("{} needs at least two solids.", op.name()));
        return Ok(());
    }
    c.log("Computing...");
    let sols = c.solids(&ids);
    let mut acc = sols[0].mesh();
    for s in &sols[1..] {
        acc = boolean_sync(&acc, &s.mesh(), op)?;
    }
    finish_boolean(&c, &ids, acc, op.name());
    Ok(())
}

async fn subtract(c: Ctx, _a: Args) -> CmdResult<()> {
    let a = select_solids(&c, "Select solids to subtract from").await?;
    if a.is_empty() {
        return Ok(());
    }
    c.ed(|e| e.temp_sel = a.iter().copied().collect());
    let b: Vec<u64> = select_solids(&c, "Select solids to subtract").await?.into_iter().filter(|s| !a.contains(s)).collect();
    if b.is_empty() {
        c.log_error("Nothing to subtract.");
        return Ok(());
    }
    c.log("Computing...");
    let (sa, sb) = (c.solids(&a), c.solids(&b));
    let mut acc = sa[0].mesh();
    for s in &sa[1..] {
        acc = boolean_sync(&acc, &s.mesh(), BoolOp::Union)?;
    }
    for s in &sb {
        acc = boolean_sync(&acc, &s.mesh(), BoolOp::Subtract)?;
    }
    let all: Vec<u64> = a.iter().chain(b.iter()).copied().collect();
    finish_boolean(&c, &all, acc, "Subtract");
    Ok(())
}

// ---------------------------------------------------------------- solid editing and queries

async fn move3d(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = select_solids(&c, "Select solids to move").await?;
    if ids.is_empty() {
        return Ok(());
    }
    let Some(b) = c.point(ps("Specify base point")).await?.pt() else { return Ok(()) };
    let r = c.point(ps("Specify second point or <use first point as displacement>").base(b).allow_enter()).await?;
    let d = match r.pt() {
        Some(p) => p - b,
        None => b,
    };
    let z = c.number(ps("Specify Z displacement").default_num(0.0)).await?;
    let dz = z.value().unwrap_or(0.0);
    c.doc_mut(|doc| {
        doc.change(|doc| {
            for id in &ids {
                if let Some(s) = doc.solid_by_id_mut(*id) {
                    let t = transform_mesh(s, &m3::translate(d.x, d.y, dz));
                    s.vertices = t.vertices;
                }
            }
        })
    });
    c.clear_sel();
    Ok(())
}

async fn massprop(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = select_solids(&c, "Select objects").await?;
    for s in c.solids(&ids) {
        let m = s.mesh();
        let (b, cen) = (mesh_bounds(&s.vertices), mesh_centroid(&m));
        c.log(&format!("---------------- SOLID: {} ----------------", if s.name.is_empty() { "Solid" } else { &s.name }));
        c.log(&format!("Volume:           {}", f(&c, mesh_volume(&m))));
        c.log(&format!("Surface area:     {}", f(&c, mesh_area(&m))));
        c.log(&format!("Bounding box:     X: {} -- {}", f(&c, b.min.x), f(&c, b.max.x)));
        c.log(&format!("                  Y: {} -- {}", f(&c, b.min.y), f(&c, b.max.y)));
        c.log(&format!("                  Z: {} -- {}", f(&c, b.min.z), f(&c, b.max.z)));
        c.log(&format!("Centroid:         X: {}  Y: {}  Z: {}", f(&c, cen.x), f(&c, cen.y), f(&c, cen.z)));
        c.log(&format!("Triangles:        {}", s.faces.len()));
    }
    Ok(())
}

async fn vscurrent(c: Ctx, args: Args) -> CmdResult<()> {
    let mut name = args.str("style").map(str::to_string);
    if name.is_none() {
        let k = c.keyword(ps("Enter an option").options(&["2dwireframe", "Wireframe", "Hidden", "Shaded", "shaded with Edges"]).default_str("Shaded")).await?;
        name = k.kw().map(str::to_string);
    }
    let name = name.unwrap_or_else(|| "Shaded".into());
    let mapped = match name.to_lowercase().as_str() {
        "2dwireframe" => "2D Wireframe".to_string(),
        "wireframe" => "Wireframe".into(),
        "hidden" => "Hidden".into(),
        "shaded" => "Shaded".into(),
        "shaded with edges" => "Shaded with Edges".into(),
        _ => name,
    };
    c.ed(|e| e.visual_style = mapped);
    c.emit();
    Ok(())
}

async fn mesh_out(c: Ctx, fmt: &'static str) -> CmdResult<()> {
    if c.doc(|d| d.solids.is_empty()) {
        c.log_error("There are no solids to export.");
        return Ok(());
    }
    if !c.app(|a| a.ui.dialogs) {
        c.log_error(&format!("{} export is not available in this environment.", fmt.to_uppercase()));
        return Ok(());
    }
    c.app(|a| a.emit(Event::ExportMesh(fmt.to_string())));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(d3("BOX", run(box_)).title("Box").icon("box").desc("Creates a 3D solid box."));
    r.define(d3("WEDGE", run(wedge)).aliases(&["WE"]).title("Wedge").icon("wedge").desc("Creates a 3D solid wedge."));
    r.define(d3("CYLINDER", run(cylinder)).aliases(&["CYL"]).title("Cylinder").icon("cylinder").desc("Creates a 3D solid cylinder with a circular base."));
    r.define(d3("CONE", run(cone)).title("Cone").icon("cone").desc("Creates a 3D solid cone or frustum."));
    r.define(d3("SPHERE", run(sphere)).aliases(&["SPH"]).title("Sphere").icon("sphere").desc("Creates a 3D solid sphere."));
    r.define(d3("EXTRUDE", run(extrude)).aliases(&["EXT"]).title("Extrude").icon("extrude").desc("Extends the dimensions of a 2D closed outline into 3D space, creating a solid."));
    r.define(d3("UPDATEEXTRUSION", run(updateextrusion)).aliases(&["EXTUPDATE"]).title("Update Extrusion").icon("extrude").desc("Rebuilds solids from their source outlines after the outlines were edited."));
    r.define(d3("PRESSPULL", run(presspull)).aliases(&["PP"]).title("Presspull").icon("presspull").desc("Presses or pulls bounded areas into 3D."));
    r.define(d3("UNION", run(|c, _a| combine(c, BoolOp::Union, "combine"))).aliases(&["UNI"]).title("Union").icon("union").desc("Combines selected 3D solids into a single solid."));
    r.define(d3("INTERSECT", run(|c, _a| combine(c, BoolOp::Intersect, "intersect"))).aliases(&["IN"]).title("Intersect").icon("intersect").desc("Creates a 3D solid from the volume common to two or more solids."));
    r.define(d3("SUBTRACT", run(subtract)).aliases(&["SU"]).title("Subtract").icon("subtract").desc("Subtracts one set of 3D solids from another."));
    r.define(d3("3DMOVE", run(move3d)).aliases(&["3M"]).title("3D Move").icon("move").desc("Moves 3D solids a specified distance in a specified direction."));
    r.define(d3("MASSPROP", run(massprop)).title("Mass Properties").icon("list").desc("Calculates the mass properties of regions or 3D solids."));
    r.define(d3("VSCURRENT", run(vscurrent)).aliases(&["VS"]).title("Visual Style").icon("view").desc("Sets the visual style in the current viewport.").transparent());
    for (name, fmt, title) in [("OBJOUT", "obj", "Export OBJ"), ("STLOUT", "stl", "Export STL")] {
        r.define(d3(name, run(move |c, _a| mesh_out(c, fmt))).title(title).icon("export").desc(&format!("Writes the solids of the drawing as a {} mesh file.", fmt.to_uppercase())).once());
    }
}
