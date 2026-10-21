//! Paper-space commands: LAYOUT, MVIEW, MSPACE, PSPACE, PAGESETUP, VPCLIP, VPORTS and LAYOUTWIZARD.
use super::{def, run};
use crate::app::Event;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::{Paper, MODEL};
use cadforge_core::entities::*;
use cadforge_core::geom::*;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

const PAPERS: [&str; 9] = ["A0", "A1", "A2", "A3", "A4", "Letter", "Legal", "Tabloid", "ANSI C"];

fn refresh(c: &Ctx) {
    c.app(|a| a.emit(Event::Regen));
    c.emit();
}

fn next_name(c: &Ctx) -> String {
    c.doc(|d| {
        let mut n = d.layouts.len() + 1;
        while d.layout(&format!("Layout{n}")).is_some() {
            n += 1;
        }
        format!("Layout{n}")
    })
}

fn current_layout(c: &Ctx) -> Option<String> {
    c.ed(|e| {
        let t = e.current_tab();
        (!t.eq_ignore_ascii_case(MODEL)).then_some(t)
    })
}

/// The sheet that commands draw on: leaves an active viewport; fails (with a message) in model space.
fn paper_space(c: &Ctx, cmd: &str) -> Option<String> {
    let Some(l) = current_layout(c) else {
        c.log_error(&format!("** Command not allowed unless a layout tab is current ** ({cmd})"));
        return None;
    };
    c.ed(|e| e.leave_mspace());
    Some(l)
}

async fn layout(c: Ctx, args: Args) -> CmdResult<()> {
    let cur = c.ed(|e| e.current_tab());
    let kw = match args.str("option") {
        Some(o) => o.to_string(),
        None => {
            let r = c.keyword(ps("Enter layout option [Copy/Delete/New/Template/Rename/SAveas/Set/?]").options(&["Copy", "Delete", "New", "Template", "Rename", "SAveas", "Set", "?"]).default_str("Set").allow_enter()).await?;
            r.kw().unwrap_or("Set").to_string()
        }
    };
    let cur_name = if cur.eq_ignore_ascii_case(MODEL) { "Model".to_string() } else { cur.clone() };
    match kw.as_str() {
        "?" => {
            let names = c.doc(|d| d.layout_names());
            c.log(&format!("Model\n{}", names.join("\n")));
        }
        "New" => {
            let dflt = next_name(&c);
            let n = c.string(ps("Enter new layout name").default_str(&dflt)).await?;
            let name = if n.text.trim().is_empty() { dflt } else { n.text };
            match c.doc_mut(|d| d.add_layout(&name)) {
                Ok(()) => c.log(&format!("Layout \"{name}\" created.")),
                Err(m) => c.log_error(&m),
            }
        }
        "Copy" => {
            let from = c.string(ps("Enter layout to copy").default_str(&cur_name)).await?;
            let from = if from.text.trim().is_empty() { cur_name.clone() } else { from.text };
            let dflt = format!("{from} (2)");
            let to = c.string(ps("Enter layout name for the copy").default_str(&dflt)).await?;
            let to = if to.text.trim().is_empty() { dflt } else { to.text };
            if let Err(m) = c.doc_mut(|d| d.copy_layout(&from, &to)) {
                c.log_error(&m);
            }
        }
        "Delete" => {
            let n = c.string(ps("Enter name of layout to delete").default_str(&cur_name)).await?;
            let name = if n.text.trim().is_empty() { cur_name.clone() } else { n.text };
            if name.eq_ignore_ascii_case("Model") {
                c.log_error("Model space cannot be deleted.");
            } else {
                let was_current = cur.eq_ignore_ascii_case(&name);
                if was_current {
                    c.ed(|e| e.leave_mspace());
                }
                match c.doc_mut(|d| d.delete_layout(&name)) {
                    Ok(()) => c.log(&format!("Layout \"{name}\" deleted.")),
                    Err(m) => c.log_error(&m),
                }
            }
        }
        "Rename" => {
            let from = c.string(ps("Enter layout to rename").default_str(&cur_name)).await?;
            let from = if from.text.trim().is_empty() { cur_name.clone() } else { from.text };
            let to = c.string(ps("Enter new layout name")).await?.text;
            if let Err(m) = c.doc_mut(|d| d.rename_layout(&from, &to)) {
                c.log_error(&m);
            }
        }
        "Set" => {
            let n = c.string(ps("Enter layout to make current").default_str(&cur_name)).await?;
            let name = if n.text.trim().is_empty() { cur_name.clone() } else { n.text };
            if let Err(m) = c.ed(|e| e.switch_space(&name)) {
                c.log_error(&m);
            }
        }
        "Template" | "SAveas" => c.log_error("Layout templates (DWT/DWG) are not supported; use the Copy option instead."),
        _ => {}
    }
    refresh(&c);
    Ok(())
}

fn tess_closed(e: &Entity, doc: &cadforge_core::drawing::Drawing) -> Option<Vec<Pt>> {
    let closed = match &e.kind {
        Kind::Pline { closed, .. } | Kind::Spline { closed, .. } => *closed,
        Kind::Circle { .. } => true,
        Kind::Ellipse(el) => ellipse_is_full(el),
        _ => false,
    };
    if !closed {
        return None;
    }
    let pts = tess_segs(&segs_of(e, doc), 0.05);
    (pts.len() >= 3).then_some(pts)
}

fn viewport_from_polygon(poly: Vec<Pt>) -> Entity {
    let b = Bounds::of_points(&poly);
    let mut v = ViewportData::new(b.center(), b.max_x - b.min_x, b.max_y - b.min_y);
    v.clip = Some(poly);
    Entity::new(Kind::Viewport(v))
}

fn rect_viewport(a: Pt, b: Pt) -> Entity {
    let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
    Entity::new(Kind::Viewport(ViewportData::new(pt((x0 + x1) / 2.0, (y0 + y1) / 2.0), x1 - x0, y1 - y0)))
}

fn is_viewport(e: &Entity) -> bool {
    matches!(e.kind, Kind::Viewport(_))
}

fn set_viewports(c: &Ctx, ids: &[u64], f: impl Fn(&mut ViewportData)) {
    c.doc_mut(|d| {
        d.change(|d| {
            for id in ids {
                if let Some(Entity { kind: Kind::Viewport(v), .. }) = d.by_id_mut(*id) {
                    f(v);
                }
            }
        })
    });
}

/// Split the rectangle `a`-`b` into `n` viewports (2, 3 or 4) following the AutoCAD arrangements.
fn arrangement(n: usize, kind: &str, a: Pt, b: Pt) -> Vec<(Pt, Pt)> {
    let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
    let (xm, ym) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let r = |ax: f64, ay: f64, bx: f64, by: f64| (pt(ax, ay), pt(bx, by));
    match (n, kind) {
        (2, "Horizontal") => vec![r(x0, y0, x1, ym), r(x0, ym, x1, y1)],
        (2, _) => vec![r(x0, y0, xm, y1), r(xm, y0, x1, y1)],
        (3, "Horizontal") => vec![r(x0, y0, x1, y0 + (y1 - y0) / 3.0), r(x0, y0 + (y1 - y0) / 3.0, x1, y0 + 2.0 * (y1 - y0) / 3.0), r(x0, y0 + 2.0 * (y1 - y0) / 3.0, x1, y1)],
        (3, "Vertical") => vec![r(x0, y0, x0 + (x1 - x0) / 3.0, y1), r(x0 + (x1 - x0) / 3.0, y0, x0 + 2.0 * (x1 - x0) / 3.0, y1), r(x0 + 2.0 * (x1 - x0) / 3.0, y0, x1, y1)],
        (3, "Above") => vec![r(x0, ym, x1, y1), r(x0, y0, xm, ym), r(xm, y0, x1, ym)],
        (3, "Below") => vec![r(x0, y0, x1, ym), r(x0, ym, xm, y1), r(xm, ym, x1, y1)],
        (3, "Left") => vec![r(x0, y0, xm, y1), r(xm, y0, x1, ym), r(xm, ym, x1, y1)],
        (3, _) => vec![r(xm, y0, x1, y1), r(x0, y0, xm, ym), r(x0, ym, xm, y1)],
        _ => vec![r(x0, y0, xm, ym), r(xm, y0, x1, ym), r(x0, ym, xm, y1), r(xm, ym, x1, y1)],
    }
}

async fn make_array(c: &Ctx, n: usize) -> CmdResult<()> {
    let kind = if n == 4 {
        String::new()
    } else if n == 2 {
        let r = c.keyword(ps("Enter viewport arrangement [Horizontal/Vertical]").options(&["Horizontal", "Vertical"]).default_str("Vertical").allow_enter()).await?;
        r.kw().unwrap_or("Vertical").to_string()
    } else {
        let r = c.keyword(ps("Enter viewport arrangement [Horizontal/Vertical/Above/Below/Left/Right]").options(&["Horizontal", "Vertical", "Above", "Below", "Left", "Right"]).default_str("Right").allow_enter()).await?;
        r.kw().unwrap_or("Right").to_string()
    };
    let r = c.point(ps("Specify first corner or [Fit]").options(&["Fit"]).no_ortho()).await?;
    let (a, b) = if r.kw() == Some("Fit") {
        let p = c.doc(|d| d.paper_of(&d.space).cloned()).unwrap_or_else(|| Paper::named("A4", true));
        (pt(10.0, 10.0), pt(p.w - 10.0, p.h - 10.0))
    } else {
        let Some(a) = r.pt() else { return Ok(()) };
        let Some(b) = c.corner(a, ps("Specify opposite corner")).await?.pt() else { return Ok(()) };
        (a, b)
    };
    for (p, q) in arrangement(n, &kind, a, b) {
        c.add(rect_viewport(p, q));
    }
    Ok(())
}

async fn mview(c: Ctx, _args: Args) -> CmdResult<()> {
    if paper_space(&c, "MVIEW").is_none() {
        return Ok(());
    }
    let opts = ["ON", "OFF", "Fit", "Lock", "Object", "Polygonal", "2", "3", "4"];
    loop {
        let r = c.point(ps("Specify corner of viewport or [ON/OFF/Fit/Lock/Object/Polygonal/Restore/LAyer/2/3/4] <Fit>").options(&opts).allow_enter().no_ortho()).await?;
        match r.kw() {
            Some("ON") | Some("OFF") => {
                let on = r.kw() == Some("ON");
                let sel = c.select(ps("Select objects").filter(is_viewport)).await?;
                set_viewports(&c, &sel.ents, |v| v.off = !on);
            }
            Some("Lock") => {
                let st = c.keyword(ps("Enter viewport lock option [ON/OFF]").options(&["ON", "OFF"])).await?;
                let lock = st.kw() == Some("ON");
                let sel = c.select(ps("Select objects").filter(is_viewport)).await?;
                set_viewports(&c, &sel.ents, |v| v.locked = lock);
            }
            Some("Fit") => {
                let p = c.doc(|d| d.paper_of(&d.space).cloned()).unwrap_or_else(|| Paper::named("A4", true));
                c.add(Drawing::default_viewport(&p));
            }
            Some("Object") => {
                let r = c.entity(ps("Select object to clip viewport")).await?;
                let Some(id) = r.id() else { return Ok(()) };
                let Some(poly) = c.doc(|d| d.by_id(id).and_then(|e| tess_closed(e, d))) else {
                    c.log_error("Select a closed polyline, circle, ellipse or spline.");
                    continue;
                };
                c.add(viewport_from_polygon(poly));
            }
            Some("Polygonal") => {
                let Some(first) = c.point(ps("Specify start point")).await?.pt() else { return Ok(()) };
                let mut pts = vec![first];
                loop {
                    let r = c.point(ps("Specify next point or [Close/Undo]").options(&["Close", "Undo"]).base(*pts.last().expect("point")).allow_enter()).await?;
                    match r.kw() {
                        Some("Undo") => {
                            if pts.len() > 1 {
                                pts.pop();
                            }
                        }
                        Some("Close") => break,
                        _ => match r.pt() {
                            Some(p) if matches!(r, PointR::Pt(_)) => pts.push(p),
                            _ => break,
                        },
                    }
                }
                if pts.len() < 3 {
                    c.log_error("A polygonal viewport needs at least three points.");
                    continue;
                }
                c.add(viewport_from_polygon(pts));
            }
            Some("2") => make_array(&c, 2).await?,
            Some("3") => make_array(&c, 3).await?,
            Some("4") => make_array(&c, 4).await?,
            Some(other) => {
                c.log_error(&format!("The {other} option is not available (viewport layer overrides need per-viewport freeze, which is not supported)."));
                continue;
            }
            None => match r {
                PointR::Pt(a) => {
                    let Some(b) = c.corner(a, ps("Specify opposite corner")).await?.pt() else { return Ok(()) };
                    if (a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9 {
                        c.log_error("The viewport has no area.");
                        continue;
                    }
                    c.add(rect_viewport(a, b));
                }
                _ => {
                    let p = c.doc(|d| d.paper_of(&d.space).cloned()).unwrap_or_else(|| Paper::named("A4", true));
                    c.add(Drawing::default_viewport(&p));
                }
            },
        }
        break;
    }
    refresh(&c);
    Ok(())
}

use cadforge_core::drawing::Drawing;

async fn mspace(c: Ctx, _args: Args) -> CmdResult<()> {
    if c.ed(|e| e.mspace.is_some()) {
        c.log("Model space is already active in a viewport.");
        return Ok(());
    }
    let Some(layout) = current_layout(&c) else {
        c.log_error("** Command not allowed unless a layout tab is current ** (MSPACE)");
        return Ok(());
    };
    let ids = c.ed(|e| e.viewport_ids(&layout));
    let active: Vec<u64> = c.doc(|d| ids.iter().copied().filter(|id| matches!(d.by_id(*id).map(|e| &e.kind), Some(Kind::Viewport(v)) if !v.off)).collect());
    let vp = match active.len() {
        0 => {
            c.log_error("There is no viewport in this layout. Use MVIEW to create one.");
            return Ok(());
        }
        1 => active[0],
        _ => {
            let r = c.point(ps("Select viewport to make active").no_ortho()).await?;
            let Some(p) = r.pt() else { return Ok(()) };
            match c.ed(|e| e.viewport_at(p)) {
                Some(id) => id,
                None => {
                    c.log_error("No viewport at that point.");
                    return Ok(());
                }
            }
        }
    };
    if let Err(m) = c.ed(|e| e.enter_mspace(vp)) {
        c.log_error(&m);
    }
    refresh(&c);
    Ok(())
}

async fn pspace(c: Ctx, _args: Args) -> CmdResult<()> {
    if c.ed(|e| e.mspace.is_some()) {
        c.ed(|e| e.leave_mspace());
        refresh(&c);
    } else if current_layout(&c).is_none() {
        c.log_error("** Command not allowed unless a layout tab is current ** (PSPACE)");
    }
    Ok(())
}

async fn vpclip(c: Ctx, _args: Args) -> CmdResult<()> {
    if paper_space(&c, "VPCLIP").is_none() {
        return Ok(());
    }
    let r = c.entity(ps("Select viewport to clip").filter(is_viewport)).await?;
    let Some(id) = r.id() else { return Ok(()) };
    let k = c.keyword(ps("Select clipping object or [Delete/Polygonal]").options(&["Delete", "Polygonal", "Select"]).default_str("Select").allow_enter()).await?;
    let poly: Option<Vec<Pt>> = match k.kw() {
        Some("Delete") => None,
        Some("Polygonal") => {
            let Some(first) = c.point(ps("Specify start point")).await?.pt() else { return Ok(()) };
            let mut pts = vec![first];
            loop {
                let r = c.point(ps("Specify next point or [Close]").options(&["Close"]).base(*pts.last().expect("point")).allow_enter()).await?;
                match r {
                    PointR::Pt(p) => pts.push(p),
                    _ => break,
                }
            }
            if pts.len() < 3 {
                c.log_error("A clip polygon needs at least three points.");
                return Ok(());
            }
            Some(pts)
        }
        _ => {
            let r = c.entity(ps("Select clipping object")).await?;
            let Some(oid) = r.id() else { return Ok(()) };
            match c.doc(|d| d.by_id(oid).and_then(|e| tess_closed(e, d))) {
                Some(p) => Some(p),
                None => {
                    c.log_error("Select a closed polyline, circle, ellipse or spline.");
                    return Ok(());
                }
            }
        }
    };
    set_viewports(&c, &[id], |v| match &poly {
        Some(p) => {
            let b = Bounds::of_points(p);
            v.c = b.center();
            v.w = b.max_x - b.min_x;
            v.h = b.max_y - b.min_y;
            v.clip = Some(p.clone());
        }
        None => {
            if let Some(cl) = v.clip.take() {
                let b = Bounds::of_points(&cl);
                v.c = b.center();
                v.w = b.max_x - b.min_x;
                v.h = b.max_y - b.min_y;
            }
        }
    });
    refresh(&c);
    Ok(())
}

async fn vports(c: Ctx, _args: Args) -> CmdResult<()> {
    if current_layout(&c).is_none() {
        c.log_error("Tiled model-space viewports are not supported. Use a layout (VPORTS then creates floating viewports).");
        return Ok(());
    }
    let r = c.keyword(ps("Enter an option [SIngle/2/3/4]").options(&["SIngle", "2", "3", "4"]).default_str("3").allow_enter()).await?;
    paper_space(&c, "VPORTS");
    match r.kw().unwrap_or("3") {
        "SIngle" => {
            let p = c.doc(|d| d.paper_of(&d.space).cloned()).unwrap_or_else(|| Paper::named("A4", true));
            c.add(Drawing::default_viewport(&p));
        }
        "2" => make_array(&c, 2).await?,
        "4" => make_array(&c, 4).await?,
        _ => make_array(&c, 3).await?,
    }
    refresh(&c);
    Ok(())
}

fn parse_paper(name: &str) -> Option<&'static str> {
    PAPERS.iter().find(|p| p.eq_ignore_ascii_case(name.trim())).copied()
}

async fn pagesetup(c: Ctx, args: Args) -> CmdResult<()> {
    let Some(layout) = current_layout(&c) else {
        c.log_error("Page setup applies to a layout. Switch to a layout tab first.");
        return Ok(());
    };
    if c.app(|a| a.ui.dialogs) {
        c.app(|a| a.emit(Event::UiAction { name: "PAGESETUP".into(), args: Args::none() }));
        return Ok(());
    }
    let paper = match args.str("paper").and_then(parse_paper) {
        Some(p) => p.to_string(),
        None => {
            let cur = c.doc(|d| d.paper_of(&layout).map(|p| p.name.clone()).unwrap_or_default());
            let r = c.keyword(ps("Enter paper size [A0/A1/A2/A3/A4/Letter/Legal/Tabloid/ANSI C]").options(&PAPERS).default_str(&cur).allow_enter()).await?;
            r.kw().and_then(parse_paper).unwrap_or("A4").to_string()
        }
    };
    let landscape = match args.str("orientation") {
        Some(o) => !o.eq_ignore_ascii_case("portrait"),
        None => {
            let r = c.keyword(ps("Enter drawing orientation [Landscape/Portrait]").options(&["Landscape", "Portrait"]).default_str("Landscape").allow_enter()).await?;
            r.kw() != Some("Portrait")
        }
    };
    c.doc_mut(|d| {
        d.change(|d| {
            if let Some(l) = d.layouts.iter_mut().find(|l| l.name.eq_ignore_ascii_case(&layout)) {
                l.paper = Paper::named(&paper, landscape);
            }
        })
    });
    c.ed(|e| e.fit_sheet());
    refresh(&c);
    Ok(())
}

async fn layoutwizard(c: Ctx, _args: Args) -> CmdResult<()> {
    let dflt = next_name(&c);
    let n = c.string(ps("Enter a name for the new layout").default_str(&dflt)).await?;
    let name = if n.text.trim().is_empty() { dflt } else { n.text };
    let p = c.keyword(ps("Enter paper size [A0/A1/A2/A3/A4/Letter/Legal/Tabloid/ANSI C]").options(&PAPERS).default_str("A4").allow_enter()).await?;
    let paper = p.kw().and_then(parse_paper).unwrap_or("A4");
    let o = c.keyword(ps("Enter drawing orientation [Landscape/Portrait]").options(&["Landscape", "Portrait"]).default_str("Landscape").allow_enter()).await?;
    let landscape = o.kw() != Some("Portrait");
    if let Err(m) = c.doc_mut(|d| d.add_layout(&name).map(|_| d.layouts.last_mut().map(|l| l.paper = Paper::named(paper, landscape)))) {
        c.log_error(&m);
        return Ok(());
    }
    // the viewport created with the layout covers the old sheet; refit it to the chosen paper
    c.doc_mut(|d| {
        if let Some(l) = d.layouts.last_mut() {
            let (w, h) = (l.paper.w, l.paper.h);
            for e in l.entities.iter_mut() {
                if let Kind::Viewport(v) = &mut e.kind {
                    v.c = pt(w / 2.0, h / 2.0);
                    v.w = w - 20.0;
                    v.h = h - 20.0;
                }
            }
        }
    });
    if let Err(m) = c.ed(|e| e.switch_space(&name)) {
        c.log_error(&m);
    }
    refresh(&c);
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Layout", "LAYOUT", run(layout)).title("Layout").desc("Creates and modifies layout tabs.").icon("layout"));
    r.define(def("Layout", "MVIEW", run(mview)).aliases(&["MV"]).title("Viewport").desc("Creates and controls layout viewports.").icon("viewport"));
    r.define(def("Layout", "MSPACE", run(mspace)).aliases(&["MS"]).title("Model").desc("Switches from paper space to model space in a layout viewport.").icon("model"));
    r.define(def("Layout", "PSPACE", run(pspace)).aliases(&["PS"]).title("Paper").desc("Switches from model space in a layout viewport to paper space.").icon("paper"));
    r.define(def("Layout", "VPCLIP", run(vpclip)).title("Clip Viewport").desc("Clips layout viewport objects and reshapes the viewport border.").icon("viewport"));
    r.define(def("Layout", "VPORTS", run(vports)).title("Viewports").desc("Creates multiple viewports in a layout.").icon("viewport"));
    r.define(def("Layout", "PAGESETUP", run(pagesetup)).title("Page Setup").desc("Controls the page layout, plotting device, paper size and other settings for each new layout.").icon("plot"));
    r.define(def("Layout", "LAYOUTWIZARD", run(layoutwizard)).title("Layout Wizard").desc("Creates a new layout tab and specifies page and plot settings.").icon("layout"));
}
