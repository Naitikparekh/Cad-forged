//! REGION, GRADIENT, WIPEOUT and region-aware UNION / SUBTRACT / INTERSECT.
use super::{dd, pref_num, pref_text, ps, set_pref};
use crate::commands::helpers::*;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::{Registry, RunFn};
use crate::runtime::*;
use crate::sysvars::{parse_color, Value};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::path::{inherit_props, path_of};
use cadforge_core::ops::polybool::{boolean, Area, Loop, Op};
use cadforge_core::ops::boundary::find_region;
use cadforge_core::ops::trim::join;
use cadforge_core::solid::boolean::{boolean_sync, BoolOp};
use cadforge_core::solid::mesh::mesh_volume;
use cadforge_core::solid::Solid;
use cadforge_core::style::{aci_rgb, fmt_num, Color};
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------- closed loops from objects

/// A closed boundary found among selected objects, with the objects that formed it.
struct Closed {
    verts: Loop,
    src: Vec<u64>,
    proto: Entity,
}

fn circle_loop(c: Pt, r: f64) -> Loop {
    vec![Vertex::new(c.x + r, c.y, 1.0), Vertex::new(c.x - r, c.y, 1.0)]
}

/// Do two non-adjacent segments of a closed loop cross?
fn self_intersects(segs: &[Seg]) -> bool {
    let n = segs.len();
    (0..n).any(|i| (i + 2..n).any(|j| !(i == 0 && j == n - 1) && !seg_intersect(&segs[i], &segs[j], false, false).is_empty()))
}

/// Closed loops among `ents`: circles, closed polylines, ellipses and splines, plus lines, arcs and open polylines
/// that join end to end into a closed chain. Also returns how many open or self-intersecting curves were left out.
fn closed_loops(doc: &Drawing, ents: &[Entity]) -> (Vec<Closed>, usize) {
    let (mut out, mut open, mut skipped): (Vec<Closed>, Vec<&Entity>, usize) = (vec![], vec![], 0);
    for e in ents {
        let segs = segs_of(e, doc);
        let mut closed = |verts: Loop| out.push(Closed { verts, src: vec![e.id], proto: e.clone() });
        match &e.kind {
            Kind::Circle { c, r } => closed(circle_loop(*c, *r)),
            Kind::Pline { v, closed: true } if !self_intersects(&segs) => closed(v.clone()),
            Kind::Ellipse(el) if ellipse_is_full(el) => closed(segs_to_verts(&segs, true)),
            Kind::Spline { closed: true, .. } => closed(segs_to_verts(&segs, true)),
            Kind::Pline { closed: true, .. } => skipped += 1,
            Kind::Line { .. } | Kind::Arc { .. } | Kind::Pline { .. } => open.push(e),
            Kind::Spline { .. } | Kind::Ellipse(_) => skipped += 1,
            _ => {}
        }
    }
    let res = join(&open, doc, 1e-6);
    for r in res.results {
        let verts = match &r.kind {
            Kind::Pline { v, closed: true } => v.clone(),
            Kind::Circle { c, r } => circle_loop(*c, *r),
            _ => {
                skipped += 1;
                continue;
            }
        };
        let segs = pline_segs(&verts, true);
        if self_intersects(&segs) {
            skipped += 1;
            continue;
        }
        let src: Vec<u64> = open.iter().filter(|e| path_of(e, doc).and_then(|p| p.segs.first().map(|s| dist_to_segs(&segs, s.at(0.5)).0 < 1e-6)).unwrap_or(false)).map(|e| e.id).collect();
        out.push(Closed { verts, src, proto: r });
    }
    let found = out.len();
    out.retain(|l| pline_area(&l.verts).abs() > 1e-9);
    let dropped = found - out.len();
    (out, skipped + dropped)
}

fn region_entity(loops: Vec<Loop>, proto: &Entity) -> Entity {
    inherit_props(proto, Entity::new(Kind::Region(RegionData { loops })))
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

// ---------------------------------------------------------------- REGION

/// Converts closed loops into regions; the source objects are deleted.
async fn region(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let ents = c.entities(&sel.ents);
    let (loops, skipped) = c.doc(|d| closed_loops(d, &ents));
    if skipped > 0 {
        c.log_error(&format!("{} not part of a closed, non-self-intersecting loop.", plural(skipped, "object")));
    }
    if loops.is_empty() {
        c.log_error("No closed loops were found.");
        return Ok(());
    }
    let n = loops.len();
    let gone: Vec<u64> = loops.iter().flat_map(|l| l.src.iter().copied()).collect();
    c.doc_mut(|d| {
        d.change(|d| {
            d.remove_all(&gone);
            for l in loops {
                d.add(region_entity(vec![l.verts], &l.proto));
            }
        })
    });
    c.clear_sel();
    c.log(&format!("{} extracted.", plural(n, "loop")));
    c.log(&format!("{} created.", plural(n, "Region")));
    Ok(())
}

// ---------------------------------------------------------------- UNION / SUBTRACT / INTERSECT on regions

fn is_region(e: &Entity) -> bool {
    matches!(e.kind, Kind::Region(_))
}

fn loops_of(e: &Entity) -> Vec<Loop> {
    match &e.kind {
        Kind::Region(r) => r.loops.clone(),
        _ => vec![],
    }
}

/// Combine the regions one after another (`op` is Union or Intersect).
fn fold(list: &[Entity], op: Op) -> Vec<Area> {
    let mut areas: Vec<Area> = vec![loops_of(&list[0])];
    for r in &list[1..] {
        let acc: Vec<Loop> = areas.iter().flatten().cloned().collect();
        areas = boolean(&acc, &loops_of(r), op);
    }
    areas
}

async fn region_op(c: &Ctx, op: Op, a: Vec<Entity>) -> CmdResult<()> {
    let mut b: Vec<Entity> = vec![];
    if op == Op::Subtract {
        c.ed(|e| e.temp_sel = a.iter().map(|x| x.id).collect());
        let sel = c.select(ps("Select solids and regions to subtract").solids().filter(is_region)).await?;
        b = c.entities(&sel.ents).into_iter().filter(|e| !a.iter().any(|x| x.id == e.id)).collect();
        if b.is_empty() {
            c.log_error("Nothing to subtract.");
            return Ok(());
        }
    } else if a.len() < 2 {
        c.log_error("At least two regions must be selected.");
        return Ok(());
    }
    let areas = if op == Op::Subtract {
        let flat = |l: &[Entity]| fold(l, Op::Union).iter().flatten().cloned().collect::<Vec<Loop>>();
        boolean(&flat(&a), &flat(&b), Op::Subtract)
    } else {
        fold(&a, op)
    };
    let ids: Vec<u64> = a.iter().chain(&b).map(|e| e.id).collect();
    let n = areas.len();
    c.doc_mut(|d| {
        d.change(|d| {
            d.remove_all(&ids);
            for area in areas {
                d.add(region_entity(area, &a[0]));
            }
        })
    });
    c.clear_sel();
    if n == 0 {
        c.log("The result is empty; no region remains.");
    } else {
        c.log(&format!("{} created.", plural(n, "Region")));
    }
    Ok(())
}

/// Solid boolean used when the drawing also holds regions (the stock command only prompts for solids).
fn solid_op(c: &Ctx, op: Op, a: &[u64], b: &[u64]) -> CmdResult<()> {
    let (sa, sb) = (c.solids(a), c.solids(b));
    let bop = match op {
        Op::Union => BoolOp::Union,
        Op::Intersect => BoolOp::Intersect,
        Op::Subtract => BoolOp::Subtract,
    };
    c.log("Computing...");
    let mut acc = sa[0].mesh();
    for s in &sa[1..] {
        acc = boolean_sync(&acc, &s.mesh(), if op == Op::Subtract { BoolOp::Union } else { bop })?;
    }
    for s in &sb {
        acc = boolean_sync(&acc, &s.mesh(), BoolOp::Subtract)?;
    }
    let first = sa[0].clone();
    let nonempty = !acc.faces.is_empty();
    let mut vol = 0.0;
    c.doc_mut(|d| {
        d.change(|d| {
            for id in a.iter().chain(b) {
                d.remove_solid(*id);
            }
            if nonempty {
                let mut s = Solid::from_mesh(bop.name(), acc);
                (s.layer, s.color) = (first.layer.clone(), first.color);
                vol = mesh_volume(&s.mesh());
                d.insert_solid_raw(s);
            }
        })
    });
    if nonempty {
        let prec = c.doc(|d| d.settings.units.precision.max(0) as usize);
        c.log(&format!("{} complete (volume {}).", bop.name(), fmt_num(vol, prec, false)));
    } else {
        c.log("The result is empty; no solid remains.");
    }
    c.clear_sel();
    Ok(())
}

/// The stock solid command when the drawing has no regions; otherwise one that takes regions as well as solids.
async fn region_aware(c: Ctx, args: Args, op: Op, stock: RunFn) -> CmdResult<()> {
    if !c.doc(|d| d.entities().iter().any(is_region)) {
        return stock(c, args).await;
    }
    let verb = match op {
        Op::Union => "combine",
        Op::Intersect => "intersect",
        Op::Subtract => "subtract from",
    };
    let sel = c.select(ps(&format!("Select solids and regions to {verb}")).solids().filter(is_region).pickfirst(true)).await?;
    if sel.is_empty() {
        return Ok(());
    }
    if !sel.ents.is_empty() && !sel.solids.is_empty() {
        c.log_error("Regions and solids cannot be combined in one operation.");
        return Ok(());
    }
    if !sel.ents.is_empty() {
        return region_op(&c, op, c.entities(&sel.ents)).await;
    }
    let mut b: Vec<u64> = vec![];
    if op == Op::Subtract {
        c.ed(|e| e.temp_sel = sel.solids.iter().copied().collect());
        b = c.select(ps("Select solids and regions to subtract").solids().filter(|_| false)).await?.solids.into_iter().filter(|s| !sel.solids.contains(s)).collect();
        if b.is_empty() {
            c.log_error("Nothing to subtract.");
            return Ok(());
        }
    } else if sel.solids.len() < 2 {
        c.log_error(&format!("{} needs at least two solids.", if op == Op::Union { "Union" } else { "Intersect" }));
        return Ok(());
    }
    solid_op(&c, op, &sel.solids, &b)
}

// ---------------------------------------------------------------- GRADIENT

const PATTERNS: [&str; 9] = ["LINEAR", "CYLINDER", "INVCYLINDER", "SPHERICAL", "HEMISPHERICAL", "CURVED", "INVSPHERICAL", "INVHEMISPHERICAL", "INVCURVED"];
const PATTERN_LABELS: [&str; 9] = ["Linear", "CYlinder", "INVCylinder", "Spherical", "Hemispherical", "CUrved", "INVSpherical", "INVHemispherical", "INVCUrved"];

/// Gradient settings, kept in the same variables AutoCAD uses (GFNAME, GFANG, GFCLR1, GFCLR2, GFCLRSTATE, GFCLRLUM).
#[derive(Clone, Debug, PartialEq)]
struct Grad {
    /// 1-9, index into [`PATTERNS`] plus one.
    pattern: usize,
    /// Degrees.
    angle: f64,
    c1: [u8; 3],
    c2: [u8; 3],
    one_color: bool,
    tint: f64,
}

fn rgb_text(c: [u8; 3]) -> String {
    format!("{},{},{}", c[0], c[1], c[2])
}

fn rgb_pref(c: &Ctx, name: &str, default: [u8; 3]) -> [u8; 3] {
    let v: Vec<u8> = pref_text(c, name, &rgb_text(default)).split(',').filter_map(|p| p.trim().parse().ok()).collect();
    if v.len() == 3 {
        [v[0], v[1], v[2]]
    } else {
        default
    }
}

fn load_grad(c: &Ctx) -> Grad {
    Grad {
        pattern: (pref_num(c, "GFNAME", 1.0) as usize).clamp(1, 9),
        angle: pref_num(c, "GFANG", 0.0),
        c1: rgb_pref(c, "GFCLR1", [0, 0, 255]),
        c2: rgb_pref(c, "GFCLR2", [255, 255, 255]),
        one_color: pref_num(c, "GFCLRSTATE", 0.0) != 0.0,
        tint: pref_num(c, "GFCLRLUM", 0.5),
    }
}

fn save_grad(c: &Ctx, g: &Grad) {
    set_pref(c, "GFNAME", Value::Num(g.pattern as f64));
    set_pref(c, "GFANG", Value::Num(g.angle));
    set_pref(c, "GFCLR1", Value::Str(rgb_text(g.c1)));
    set_pref(c, "GFCLR2", Value::Str(rgb_text(g.c2)));
    set_pref(c, "GFCLRSTATE", Value::Num(g.one_color as u8 as f64));
    set_pref(c, "GFCLRLUM", Value::Num(g.tint));
}

fn mix(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    let m = |i: usize| (a[i] as f64 * (1.0 - t) + b[i] as f64 * t).round() as u8;
    [m(0), m(1), m(2)]
}

/// The fill description the renderer understands. The renderer has no inverted spherical patterns, so those are
/// the plain pattern with the two colours swapped.
fn gradient_data(g: &Grad) -> GradientData {
    let name = PATTERNS[g.pattern - 1];
    let swapped = matches!(name, "INVSPHERICAL" | "INVHEMISPHERICAL" | "INVCURVED");
    let (c1, c2) = if g.one_color { (g.c1, mix(g.c1, [255, 255, 255], g.tint)) } else { (g.c1, g.c2) };
    GradientData {
        c1: if swapped { c2 } else { c1 },
        c2: if swapped { c1 } else { c2 },
        angle: rad(g.angle),
        kind: if swapped { name.trim_start_matches("INV") } else { name }.to_string(),
        one_color: g.one_color && !swapped,
        tint: g.tint,
    }
}

fn gradient_hatch(loops: &[Loop], g: &Grad) -> Entity {
    let mut e = Entity::hatch(loops.to_vec(), "SOLID");
    if let Kind::Hatch(h) = &mut e.kind {
        h.gradient = Some(gradient_data(g));
    }
    e
}

/// Text to RGB for the gradient colours (index, name or `r,g,b`).
fn gradient_color(text: &str) -> Result<[u8; 3], String> {
    match parse_color(text)? {
        Some(Color::Aci(i)) => Ok(aci_rgb(i)),
        Some(Color::Rgb(r, g, b)) => Ok([r, g, b]),
        _ => Err("A gradient needs a specific color (not BYLAYER or BYBLOCK).".into()),
    }
}

/// The command-line replacement for the Gradient tab of the Hatch and Gradient dialog.
async fn gradient_settings(c: &Ctx, g: &RefCell<Grad>) -> CmdResult<()> {
    loop {
        let cur = g.borrow().clone();
        c.log(&format!("Current gradient: {}, angle {}, {}", PATTERN_LABELS[cur.pattern - 1].to_uppercase(), fmt_num(cur.angle, 2, true), if cur.one_color { format!("one color {} tint {}", rgb_text(cur.c1), fmt_num(cur.tint, 2, true)) } else { format!("colors {} to {}", rgb_text(cur.c1), rgb_text(cur.c2)) }));
        let r = c.keyword(ps("Enter a gradient option").options(&["Pattern", "Color1", "Color2", "One color", "Angle", "Tint", "Done"]).default_str("Done")).await?;
        match r.kw() {
            Some("Pattern") => {
                let k = c.keyword(ps("Enter gradient pattern").options(&PATTERN_LABELS).default_str(PATTERN_LABELS[cur.pattern - 1])).await?;
                if let Some(i) = PATTERN_LABELS.iter().position(|l| Some(*l) == k.kw()) {
                    g.borrow_mut().pattern = i + 1;
                }
            }
            Some(kw @ ("Color1" | "Color2")) => {
                let (n, now) = if kw == "Color1" { (1, cur.c1) } else { (2, cur.c2) };
                let t = c.string(ps(&format!("Enter color {n} (index, name or r,g,b)")).default_str(&rgb_text(now))).await?;
                match gradient_color(&t.text) {
                    Ok(rgb) if n == 1 => g.borrow_mut().c1 = rgb,
                    Ok(rgb) => g.borrow_mut().c2 = rgb,
                    Err(m) => c.log_error(&m),
                }
            }
            Some("One color") => {
                let k = c.keyword(ps("Enter color mode").options(&["One", "Two"]).default_str(if cur.one_color { "One" } else { "Two" })).await?;
                g.borrow_mut().one_color = k.kw() == Some("One");
            }
            Some("Angle") => {
                if let Some(a) = c.angle(ps("Specify gradient angle").default_num(rad(cur.angle))).await?.value() {
                    g.borrow_mut().angle = deg(a);
                }
            }
            Some("Tint") => {
                if let Some(v) = c.number(ps("Specify tint of the one-color gradient (0 = color, 1 = white)").default_num(cur.tint)).await?.value() {
                    if (0.0..=1.0).contains(&v) {
                        g.borrow_mut().tint = v;
                    } else {
                        c.log_error("The tint must be between 0 and 1.");
                    }
                }
            }
            _ => return Ok(()),
        }
    }
}

/// Fills an enclosed area with a gradient (HATCH with a gradient fill, picked like HATCH).
async fn gradient(c: Ctx, _a: Args) -> CmdResult<()> {
    let g = Rc::new(RefCell::new(load_grad(&c)));
    let loops: Rc<RefCell<Vec<Loop>>> = Rc::new(RefCell::new(vec![]));
    let (l2, g2) = (loops.clone(), g.clone());
    let preview = move |_: &Drawing, _: Pt| -> Vec<PreviewItem> {
        let l = l2.borrow();
        if l.is_empty() {
            vec![]
        } else {
            vec![pv(gradient_hatch(&l, &g2.borrow()))]
        }
    };
    c.set_preview(preview.clone());
    loop {
        let r = c.point(ps("Pick internal point or").options(&["Select objects", "seTtings", "Undo"]).allow_enter().no_ortho().preview(preview.clone())).await?;
        match r.kw() {
            Some("seTtings") => {
                gradient_settings(&c, &g).await?;
                continue;
            }
            Some("Undo") => {
                loops.borrow_mut().clear();
                continue;
            }
            Some("Select objects") => {
                let sel = c.select(ps("Select objects")).await?;
                let ents = c.entities(&sel.ents);
                let (found, skipped) = c.doc(|d| closed_loops(d, &ents));
                if found.is_empty() {
                    c.log_error("The selected objects do not form a closed boundary.");
                } else if skipped > 0 {
                    c.log_error(&format!("{} not part of a closed, non-self-intersecting loop.", plural(skipped, "object")));
                }
                loops.borrow_mut().extend(found.into_iter().map(|l| l.verts));
                continue;
            }
            _ => {}
        }
        let Some(p) = r.pt() else { break };
        match c.doc(|d| find_region(d, p, None)) {
            Err(m) => c.log_error(&m),
            Ok(reg) => {
                c.log(&format!("{} extracted", plural(reg.loops.len(), "loop")));
                loops.borrow_mut().extend(reg.loops);
            }
        }
    }
    let l = loops.borrow().clone();
    if !l.is_empty() {
        let gr = g.borrow().clone();
        save_grad(&c, &gr);
        c.add(gradient_hatch(&l, &gr));
        c.log("Gradient created.");
    }
    Ok(())
}

// ---------------------------------------------------------------- WIPEOUT

/// Masks the objects below it with a polygon filled in the background colour.
async fn wipeout(c: Ctx, _a: Args) -> CmdResult<()> {
    loop {
        let r = c.point(ps("Specify first point").options(&["Frames", "Polyline"]).default_str("Polyline").allow_enter()).await?;
        match r {
            PointR::Kw(k) if k == "Frames" => {
                let m = c.keyword(ps("Enter mode").options(&["ON", "OFF"]).default_str("ON")).await?;
                let on = m.kw() != Some("OFF");
                set_pref(&c, "WIPEOUTFRAME", Value::Num(on as u8 as f64));
                if !on {
                    c.log_error("Hiding wipeout frames is not supported: frames stay visible on screen (they are never plotted).");
                }
            }
            PointR::Pt(first) => return wipeout_points(&c, first).await,
            _ => return wipeout_polyline(&c).await,
        }
    }
}

async fn wipeout_points(c: &Ctx, first: Pt) -> CmdResult<()> {
    let mut pts = vec![first];
    loop {
        let snapshot = pts.clone();
        let opts: &[&str] = if pts.len() > 2 { &["Close", "Undo"] } else { &["Undo"] };
        let last = *pts.last().expect("a point");
        let r = c
            .point(ps("Specify next point").options(opts).base(last).allow_enter().preview(move |_, p| {
                let mut v = snapshot.clone();
                v.push(p);
                vec![pv(Entity::pline_pts(&v, true))]
            }))
            .await?;
        match r.kw() {
            Some("Undo") => {
                if pts.len() > 1 {
                    pts.pop();
                }
            }
            Some("Close") => break,
            _ => match r.pt() {
                Some(p) => pts.push(p),
                None => break,
            },
        }
    }
    if pts.len() < 3 {
        c.log_error("A wipeout needs at least three points.");
        return Ok(());
    }
    c.add(Entity::new(Kind::Wipeout(WipeoutData { pts })));
    Ok(())
}

async fn wipeout_polyline(c: &Ctx) -> CmdResult<()> {
    let Some(id) = c.pick_entity(ps("Select a closed polyline").filter(|e| matches!(e.kind, Kind::Pline { closed: true, .. }))).await? else { return Ok(()) };
    let Some(e) = c.entity_by_id(id) else { return Ok(()) };
    let mut pts = c.doc(|d| {
        let b = bounds_of(&e, d).map_or(1.0, |b| b.width().hypot(b.height()));
        tess_segs(&segs_of(&e, d), (b * 1e-3).max(1e-6))
    });
    if pts.len() > 1 && same(pts[0], pts[pts.len() - 1]) {
        pts.pop();
    }
    if pts.len() < 3 {
        c.log_error("The polyline is too small for a wipeout.");
        return Ok(());
    }
    let erase = c.keyword(ps("Erase polyline?").options(&["Yes", "No"]).default_str("No")).await?.kw() == Some("Yes");
    c.doc_mut(|d| {
        d.change(|d| {
            if erase {
                d.remove(id);
            }
            d.add(inherit_props(&e, Entity::new(Kind::Wipeout(WipeoutData { pts }))));
        })
    });
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(dd("REGION", run(region)).aliases(&["REG"]).title("Region").icon("region").desc("Converts an object that encloses an area into a region object. The source objects are deleted."));
    r.define(dd("GRADIENT", run(gradient)).title("Gradient").icon("hatch").desc("Fills an enclosed area or selected objects with a gradient fill. Settings are chosen at the command line."));
    r.define(dd("WIPEOUT", run(wipeout)).title("Wipeout").icon("region").desc("Creates a polygonal area that masks existing objects with the background color."));
    // UNION / SUBTRACT / INTERSECT keep their solid behaviour and additionally take regions once the drawing has any
    for (name, op, text) in [
        ("UNION", Op::Union, "Combines selected 3D solids or 2D regions into a single object."),
        ("SUBTRACT", Op::Subtract, "Subtracts one set of 3D solids or 2D regions from another."),
        ("INTERSECT", Op::Intersect, "Creates a 3D solid or 2D region from the volume or area common to two or more objects."),
    ] {
        let Some(stock) = r.get(name) else { continue };
        let mut d = (*stock).clone();
        let prev = stock.run.clone();
        d.run = run(move |c, a| region_aware(c, a, op, prev.clone()));
        d.desc = text.into();
        r.define(d);
    }
}
