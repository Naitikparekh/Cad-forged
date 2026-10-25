//! Draw commands: LINE, PLINE, CIRCLE, ARC, RECTANG, POLYGON, ELLIPSE, SPLINE, POINT, XLINE, RAY, DONUT
//! (port of `src/commands/draw.js`).
use super::helpers::*;
use super::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use crate::sysvars::fmt_num;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::fillet::{chamfer_pline, fillet_pline};

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

// ---------------------------------------------------------------- LINE

async fn line(c: Ctx, _a: Args) -> CmdResult<()> {
    let r = c.point(ps("Specify first point").allow_enter()).await?;
    let start = match r {
        PointR::Enter(_) => match c.app(|a| a.mem.last_line_end) {
            Some(p) => p,
            None => return Ok(()),
        },
        PointR::Pt(p) => p,
        _ => return Ok(()),
    };
    let mut made: Vec<(u64, Pt)> = vec![]; // (id, end point)
    let mut prev = start;
    loop {
        let options: &[&str] = if made.len() >= 2 { &["Close", "Undo"] } else if !made.is_empty() { &["Undo"] } else { &[] };
        let r = c.point(ps("Specify next point").base(prev).options(options).allow_enter().preview(move |_, p| vec![pv(Entity::line(prev, p))])).await?;
        match &r {
            PointR::Kw(k) if k == "Undo" => {
                if let Some((id, _)) = made.pop() {
                    c.doc_mut(|d| d.remove(id));
                    prev = made.last().map(|m| m.1).unwrap_or(start);
                }
                continue;
            }
            PointR::Kw(k) if k == "Close" => {
                if !same(prev, start) {
                    c.add(Entity::line(prev, start));
                }
                prev = start;
                break;
            }
            _ => {}
        }
        let Some(p) = r.pt() else { break };
        if same(prev, p) {
            continue;
        }
        let id = c.add(Entity::line(prev, p));
        made.push((id, p));
        prev = p;
    }
    c.app(|a| a.mem.last_line_end = Some(prev));
    Ok(())
}

// ---------------------------------------------------------------- PLINE

enum ArcMode {
    Second(Pt),
    Radius(f64),
}

/// Bulge for an arc segment from `p0` to `p1`; `None` when the radius is too small.
fn arc_bulge(mode: &ArcMode, p0: Pt, p1: Pt) -> Option<f64> {
    match mode {
        ArcMode::Second(s) => Some(arc_through(p0, *s, p1).map_or(0.0, |a| match a {
            Seg::A { sw, .. } => (sw / 4.0).tan(),
            _ => 0.0,
        })),
        ArcMode::Radius(r) => {
            let (chord, rr) = (dist(p0, p1), r.abs());
            if chord > 2.0 * rr {
                return None;
            }
            let half = 2.0 * (chord / (2.0 * rr)).asin();
            let sweep = if *r < 0.0 { TAU - half } else { half };
            Some((sweep / 4.0).tan())
        }
    }
}

struct PlineState {
    verts: Vec<Vertex>,
    dir: Pt,
    /// Start / end width of the next edge; after an edge the ending width carries on as the uniform width.
    w: (f64, f64),
}

impl PlineState {
    fn cur(&self) -> Pt {
        let v = self.verts.last().expect("a vertex");
        pt(v.x, v.y)
    }
    /// The vertices with the pending edge's widths on the last one (what the rubber band shows).
    fn snapshot(&self) -> Vec<Vertex> {
        let mut v = self.verts.clone();
        if let Some(l) = v.last_mut() {
            l.sw = self.w.0;
            l.ew = self.w.1;
        }
        v
    }
    fn push(&mut self, p: Pt, bulge: f64) {
        let n = self.verts.len();
        self.verts[n - 1].b = bulge;
        self.verts[n - 1].sw = self.w.0;
        self.verts[n - 1].ew = self.w.1;
        self.w = (self.w.1, self.w.1);
        self.verts.push(Vertex::new(p.x, p.y, 0.0));
        let prev = self.verts[n - 1];
        self.dir = if bulge != 0.0 { bulge_seg(prev.pt(), p, bulge).tangent(1.0) } else { unit(p - prev.pt()) };
    }
}

/// PLINE Width / Halfwidth: ask for the starting and ending width of the next edge.
async fn ask_pline_width(c: &Ctx, half: bool, cur: (f64, f64)) -> CmdResult<Option<(f64, f64)>> {
    let k = if half { 2.0 } else { 1.0 };
    let (kind, shown) = if half { ("half-width", cur.0 / 2.0) } else { ("width", cur.0) };
    let a = c.number(ps(&format!("Specify starting {kind} <{}>", fmt_num(shown))).default_num(shown).min(0.0)).await?;
    let Some(start) = a.value() else { return Ok(None) };
    let b = c.number(ps(&format!("Specify ending {kind} <{}>", fmt_num(start))).default_num(start).min(0.0)).await?;
    let Some(end) = b.value() else { return Ok(None) };
    Ok(Some((start * k, end * k)))
}

async fn pline(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(p0) = c.point(ps("Specify start point")).await?.pt() else { return Ok(()) };
    let w0 = c.pref("PLINEWID").max(0.0);
    let mut st = PlineState { verts: vec![Vertex::new(p0.x, p0.y, 0.0)], dir: pt(1.0, 0.0), w: (w0, w0) };
    let (mut arc_mode, mut closed) = (false, false);
    loop {
        if !arc_mode {
            let mut opts = vec!["Arc"];
            if st.verts.len() > 2 {
                opts.push("Close");
            }
            opts.extend(["Halfwidth", "Length", "Undo", "Width"]);
            let (cur, snapshot) = (st.cur(), st.snapshot());
            let r = c.point(ps("Specify next point").options(&opts).base(cur).allow_enter().preview(move |_, p| pline_preview(&with_last_bulge(&snapshot, 0.0), p))).await?;
            match r.kw() {
                Some("Arc") => {
                    arc_mode = true;
                    continue;
                }
                Some("Close") => {
                    let n = st.verts.len();
                    st.verts[n - 1].sw = st.w.0;
                    st.verts[n - 1].ew = st.w.1;
                    closed = true;
                    break;
                }
                Some("Undo") => {
                    if st.verts.len() > 1 {
                        st.verts.pop();
                        let n = st.verts.len();
                        st.verts[n - 1].b = 0.0;
                        st.w = (st.verts[n - 1].sw, st.verts[n - 1].ew);
                        if n > 1 {
                            st.dir = unit(st.cur() - st.verts[n - 2].pt());
                        }
                    }
                    continue;
                }
                Some(k @ ("Width" | "Halfwidth")) => {
                    if let Some(w) = ask_pline_width(&c, k == "Halfwidth", st.w).await? {
                        st.w = w;
                    }
                    continue;
                }
                Some("Length") => {
                    let len = c.distance(ps("Specify length of line").min(0.0).nonzero()).await?;
                    if let Some(l) = len.value().filter(|v| *v != 0.0) {
                        let a = if st.verts.len() > 1 { st.dir } else { pt(1.0, 0.0) };
                        let np = st.cur() + a * l;
                        st.push(np, 0.0);
                    }
                    continue;
                }
                _ => {}
            }
            let Some(p) = r.pt() else { break };
            st.push(p, 0.0);
        } else {
            let mut opts = vec!["Angle", "CEnter"];
            if st.verts.len() > 2 {
                opts.push("CLose");
            }
            opts.extend(["Direction", "Halfwidth", "Line", "Radius", "Second pt", "Undo", "Width"]);
            let (cur, dir, snapshot) = (st.cur(), st.dir, st.snapshot());
            let r = c
                .point(ps("Specify endpoint of arc").options(&opts).base(cur).allow_enter().preview(move |_, p| pline_preview(&with_last_bulge(&snapshot, tangent_bulge(cur, dir, p)), p)))
                .await?;
            match r.kw() {
                Some("Line") => {
                    arc_mode = false;
                    continue;
                }
                Some("CLose") => {
                    let b = tangent_bulge(st.cur(), st.dir, st.verts[0].pt());
                    let n = st.verts.len();
                    st.verts[n - 1].b = b;
                    st.verts[n - 1].sw = st.w.0;
                    st.verts[n - 1].ew = st.w.1;
                    closed = true;
                    break;
                }
                Some("Undo") => {
                    if st.verts.len() > 1 {
                        st.verts.pop();
                        let n = st.verts.len();
                        st.verts[n - 1].b = 0.0;
                        st.w = (st.verts[n - 1].sw, st.verts[n - 1].ew);
                    }
                    continue;
                }
                Some(k @ ("Width" | "Halfwidth")) => {
                    if let Some(w) = ask_pline_width(&c, k == "Halfwidth", st.w).await? {
                        st.w = w;
                    }
                    continue;
                }
                Some("Angle") => {
                    let a = c.angle(ps("Specify included angle")).await?;
                    let Some(av) = a.value() else { continue };
                    let b = (av / 4.0).tan();
                    let snapshot = st.snapshot();
                    let e = c.point(ps("Specify endpoint of arc").base(cur).preview(move |_, p| pline_preview(&with_last_bulge(&snapshot, b), p))).await?;
                    if let Some(p) = e.pt() {
                        st.push(p, b);
                    }
                    continue;
                }
                Some("CEnter") => {
                    let Some(cc) = c.point(ps("Specify center point of arc").base(cur)).await?.pt() else { continue };
                    let rad_ = dist(cc, cur);
                    let calc = move |p: Pt| -> (Pt, f64) {
                        let (a0, a1) = (angle_of(cc, cur), angle_of(cc, p));
                        (polar(cc, a1, rad_), (sweep_ccw(a0, a1) / 4.0).tan())
                    };
                    let snapshot = st.snapshot();
                    let e = c
                        .point(ps("Specify endpoint of arc").base(cc).preview(move |_, p| {
                            let (end, b) = calc(p);
                            pline_preview(&with_last_bulge(&snapshot, b), end)
                        }))
                        .await?;
                    if let Some(p) = e.pt() {
                        let (end, b) = calc(p);
                        st.push(end, b);
                    }
                    continue;
                }
                Some("Direction") => {
                    let Some(d) = c.point(ps("Specify the tangent direction for the start point of the arc").base(cur)).await?.pt() else { continue };
                    let dd = unit(d - cur);
                    let snapshot = st.snapshot();
                    let e = c.point(ps("Specify endpoint of arc").base(cur).preview(move |_, p| pline_preview(&with_last_bulge(&snapshot, tangent_bulge(cur, dd, p)), p))).await?;
                    if let Some(p) = e.pt() {
                        st.push(p, tangent_bulge(cur, dd, p));
                    }
                    continue;
                }
                Some("Radius") => {
                    let rr = c.distance(ps("Specify radius of arc").nonzero()).await?;
                    let Some(rv) = rr.value() else { continue };
                    let snapshot = st.snapshot();
                    let e = c
                        .point(ps("Specify endpoint of arc").base(cur).preview(move |_, p| pline_preview(&with_last_bulge(&snapshot, arc_bulge(&ArcMode::Radius(rv), cur, p).unwrap_or(0.0)), p)))
                        .await?;
                    if let Some(p) = e.pt() {
                        match arc_bulge(&ArcMode::Radius(rv), cur, p) {
                            None => c.log_error("The radius is too small for that endpoint."),
                            Some(b) => st.push(p, b),
                        }
                    }
                    continue;
                }
                Some("Second pt") => {
                    let Some(s) = c.point(ps("Specify second point on arc").base(cur)).await?.pt() else { continue };
                    let snapshot = st.snapshot();
                    let e = c
                        .point(ps("Specify end point of arc").base(cur).preview(move |_, p| pline_preview(&with_last_bulge(&snapshot, arc_bulge(&ArcMode::Second(s), cur, p).unwrap_or(0.0)), p)))
                        .await?;
                    if let Some(p) = e.pt() {
                        let b = arc_bulge(&ArcMode::Second(s), cur, p).unwrap_or(0.0);
                        st.push(p, b);
                    }
                    continue;
                }
                _ => {}
            }
            let Some(p) = r.pt() else { break };
            let b = tangent_bulge(st.cur(), st.dir, p);
            st.push(p, b);
        }
    }
    if st.verts.len() > 1 {
        c.add(Entity::pline(st.verts, closed));
    }
    Ok(())
}

fn with_last_bulge(verts: &[Vertex], b: f64) -> Vec<Vertex> {
    let mut v = verts.to_vec();
    if let Some(l) = v.last_mut() {
        l.b = b;
    }
    v
}

// ---------------------------------------------------------------- CIRCLE

/// Ask for a point on a line/circle/arc/polyline; returns the nearest segment and the pick point.
async fn pick_curve_seg(c: &Ctx, prompt: &str) -> CmdResult<Option<(Seg, Pt)>> {
    let r = c.entity(ps(prompt).filter(|e| matches!(e.kind, Kind::Line { .. } | Kind::Circle { .. } | Kind::Arc { .. } | Kind::Pline { .. }))).await?;
    let (EntR::Entity { id, pt: pick, .. }, true) = (&r, true) else { return Ok(None) };
    let segs = c.doc(|d| d.by_id(*id).map(|e| segs_of(e, d).to_vec()).unwrap_or_default());
    let mut best: Option<(Seg, f64)> = None;
    for s in segs {
        let (_, _, d) = s.nearest(*pick);
        if best.map_or(true, |b| d < b.1) {
            best = Some((s, d));
        }
    }
    Ok(best.map(|b| (b.0, *pick)))
}

async fn circle(c: Ctx, args: Args) -> CmdResult<()> {
    let mut mode: Option<String> = args.str("mode").map(str::to_string);
    let mut center = pt(0.0, 0.0);
    let mem_r = c.app(|a| a.mem.circle_radius);
    match mode.as_deref() {
        None => {
            let r = c.point(ps("Specify center point for circle").options(&["3P", "2P", "Ttr (tan tan radius)"])).await?;
            match r {
                PointR::Kw(k) => mode = Some(if k == "Ttr (tan tan radius)" { "TTR".into() } else { k }),
                PointR::Pt(p) => {
                    center = p;
                    mode = Some("CR".into());
                }
                _ => return Ok(()),
            }
        }
        Some("CR") | Some("CD") => {
            let Some(p) = c.point(ps("Specify center point for circle")).await?.pt() else { return Ok(()) };
            center = p;
        }
        _ => {}
    }
    let mode = mode.unwrap_or_default();
    match mode.as_str() {
        "3P" => {
            let Some(a) = c.point(ps("Specify first point on circle")).await?.pt() else { return Ok(()) };
            let Some(b) = c.point(ps("Specify second point on circle").base(a).preview(move |_, p| vec![pv(Entity::line(a, p))])).await?.pt() else { return Ok(()) };
            let Some(d) = c
                .point(ps("Specify third point on circle").base(b).preview(move |_, p| match circle_through(a, b, p) {
                    Some((cc, r)) => vec![pv(Entity::circle(cc, r))],
                    None => vec![],
                }))
                .await?
                .pt()
            else {
                return Ok(());
            };
            match circle_through(a, b, d) {
                None => c.log_error("The three points are collinear."),
                Some((cc, r)) => {
                    c.app(|a| a.mem.circle_radius = r);
                    c.add(Entity::circle(cc, r));
                }
            }
        }
        "2P" => {
            let Some(a) = c.point(ps("Specify first end point of circle's diameter")).await?.pt() else { return Ok(()) };
            let Some(b) = c
                .point(ps("Specify second end point of circle's diameter").base(a).preview(move |_, p| if dist(a, p) > 0.0 { vec![pv(Entity::circle(mid(a, p), dist(a, p) / 2.0))] } else { vec![] }))
                .await?
                .pt()
            else {
                return Ok(());
            };
            if same(a, b) {
                return Ok(());
            }
            let r = dist(a, b) / 2.0;
            c.app(|a| a.mem.circle_radius = r);
            c.add(Entity::circle(mid(a, b), r));
        }
        "TTR" => {
            let Some(p1) = pick_curve_seg(&c, "Specify point on object for first tangent of circle").await? else { return Ok(()) };
            let Some(p2) = pick_curve_seg(&c, "Specify point on object for second tangent of circle").await? else { return Ok(()) };
            let rr = c.distance(ps("Specify radius of circle").default_num(mem_r).min(0.0).nonzero()).await?;
            let Some(radius) = rr.value().filter(|v| *v != 0.0) else { return Ok(()) };
            let mut centres = tangent_centres(&p1.0, &p2.0, radius);
            if centres.is_empty() {
                c.log_error("Circle does not exist.");
                return Ok(());
            }
            let m = mid(p1.1, p2.1);
            centres.sort_by(|a, b| dist(*a, m).total_cmp(&dist(*b, m)));
            c.app(|a| a.mem.circle_radius = radius);
            c.add(Entity::circle(centres[0], radius));
        }
        _ => {
            // center + radius / diameter
            let is_dia = mode == "CD";
            let ctr = center;
            let asked = c
                .distance({
                    let s = ps(if is_dia { "Specify diameter of circle" } else { "Specify radius of circle" })
                        .base(ctr)
                        .default_num(mem_r * if is_dia { 2.0 } else { 1.0 })
                        .min(0.0)
                        .nonzero()
                        .preview(move |_, p| if dist(ctr, p) > 1e-12 { vec![pv(Entity::circle(ctr, if is_dia { dist(ctr, p) / 2.0 } else { dist(ctr, p) }))] } else { vec![] });
                    if is_dia {
                        s
                    } else {
                        s.options(&["Diameter"])
                    }
                })
                .await?;
            let radius = if asked.kw() == Some("Diameter") {
                let d = c
                    .distance(
                        ps("Specify diameter of circle")
                            .base(ctr)
                            .default_num(mem_r * 2.0)
                            .min(0.0)
                            .nonzero()
                            .preview(move |_, p| if dist(ctr, p) > 1e-12 { vec![pv(Entity::circle(ctr, dist(ctr, p) / 2.0))] } else { vec![] }),
                    )
                    .await?;
                let Some(v) = d.value() else { return Ok(()) };
                v / 2.0
            } else {
                let Some(v) = asked.value() else { return Ok(()) };
                if is_dia {
                    v / 2.0
                } else {
                    v
                }
            };
            c.app(|a| a.mem.circle_radius = radius);
            c.add(Entity::circle(ctr, radius));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- ARC

/// Arc given centre, start point and a ray toward the end (CCW).
fn arc_cse(center: Pt, start: Pt, end_ray: Pt) -> Entity {
    Entity::arc(center, dist(center, start), angle_of(center, start), angle_of(center, end_ray))
}

/// CCW arc by centre, start point and included angle (negative = CW).
fn arc_csa(center: Pt, start: Pt, ang: f64) -> Entity {
    let (a0, r) = (angle_of(center, start), dist(center, start));
    if ang >= 0.0 {
        Entity::arc(center, r, a0, a0 + ang)
    } else {
        Entity::arc(center, r, a0 + ang, a0)
    }
}

/// Start, end and included angle.
fn arc_sea(s: Pt, e: Pt, ang: f64) -> Option<Entity> {
    let chord = dist(s, e);
    if chord < 1e-12 || ang.abs() < 1e-9 || ang.abs() >= TAU {
        return None;
    }
    let r = chord / (2.0 * (ang.abs() / 2.0).sin());
    let (m, u) = (mid(s, e), unit(e - s));
    let h = (r * r - chord * chord / 4.0).max(0.0).sqrt() * if ang.abs() > PI { -1.0 } else { 1.0 };
    let side = if ang > 0.0 { 1.0 } else { -1.0 }; // CCW arcs from s to e have their centre on the left
    let center = m + perp(u) * (h * side);
    let a0 = angle_of(center, s);
    Some(if ang >= 0.0 { Entity::arc(center, r, a0, a0 + ang) } else { Entity::arc(center, r, a0 + ang, a0) })
}

/// Start, end, tangent direction at start.
fn arc_sed(s: Pt, e: Pt, dir: Pt) -> Option<Entity> {
    let b = tangent_bulge(s, dir, e);
    match bulge_seg(s, e, b) {
        a @ Seg::A { .. } => Some(Entity::arc_seg(&a)),
        _ => None,
    }
}

fn arc_ser(s: Pt, e: Pt, r_: f64) -> Option<Entity> {
    let (chord, r) = (dist(s, e), r_.abs());
    if chord > 2.0 * r || chord < 1e-12 {
        return None;
    }
    let half = 2.0 * (chord / (2.0 * r)).asin();
    let sweep = if r_ < 0.0 { TAU - half } else { half };
    arc_sea(s, e, sweep)
}

fn keep(c: &Ctx, e: Option<Entity>) {
    if let Some(e) = e {
        c.add(e);
    }
}

fn line_pv(a: Pt) -> impl Fn(&cadforge_core::drawing::Drawing, Pt) -> Vec<PreviewItem> {
    move |_, p| vec![pv(Entity::line(a, p))]
}

async fn arc(c: Ctx, args: Args) -> CmdResult<()> {
    let mode = args.str("mode").unwrap_or("3p").to_string();
    if mode == "continue" {
        let last = c.doc(|d| d.entities().iter().rev().find(|e| matches!(e.kind, Kind::Line { .. } | Kind::Arc { .. })).cloned());
        let Some(last) = last else {
            c.log_error("No line or arc to continue from.");
            return Ok(());
        };
        let (s, dir) = match &last.kind {
            Kind::Line { a, b } => (*b, unit(*b - *a)),
            Kind::Arc { c: cc, r, a1, .. } => (polar(*cc, *a1, *r), pt(-a1.sin(), a1.cos())),
            _ => return Ok(()),
        };
        let e = c.point(ps("Specify end point of arc").base(s).preview(move |_, p| arc_sed(s, p, dir).map(|x| vec![pv(x)]).unwrap_or_default())).await?;
        if let Some(p) = e.pt() {
            keep(&c, arc_sed(s, p, dir));
        }
        return Ok(());
    }
    // centre-first variants
    if ["cse", "csa", "csl"].contains(&mode.as_str()) {
        let Some(ctr) = c.point(ps("Specify center point of arc")).await?.pt() else { return Ok(()) };
        let Some(s) = c.point(ps("Specify start point of arc").base(ctr).preview(line_pv(ctr))).await?.pt() else { return Ok(()) };
        return finish_center(&c, ctr, s, &mode[1..]).await;
    }
    // start-first variants
    let s = c.point(ps("Specify start point of arc").options(if mode == "3p" { &["Center"] } else { &[] })).await?;
    if s.kw() == Some("Center") {
        let Some(ctr) = c.point(ps("Specify center point of arc")).await?.pt() else { return Ok(()) };
        let Some(st) = c.point(ps("Specify start point of arc").base(ctr).preview(line_pv(ctr))).await?.pt() else { return Ok(()) };
        return finish_center(&c, ctr, st, "se").await;
    }
    let Some(start) = s.pt() else { return Ok(()) };
    if mode == "3p" {
        let second = c.point(ps("Specify second point of arc").options(&["Center", "End"]).base(start).preview(line_pv(start))).await?;
        match second.kw() {
            Some("Center") => {
                let Some(ctr) = c.point(ps("Specify center point of arc").base(start)).await?.pt() else { return Ok(()) };
                return finish_center(&c, ctr, start, "se").await;
            }
            Some("End") => return start_end(&c, start, None, "se").await,
            _ => {}
        }
        let Some(sp) = second.pt() else { return Ok(()) };
        let Some(end) = c
            .point(ps("Specify end point of arc").base(sp).preview(move |_, p| match arc_through(start, sp, p) {
                Some(a) => vec![pv(Entity::arc_seg(&a))],
                None => vec![pv(Entity::line(start, p))],
            }))
            .await?
            .pt()
        else {
            return Ok(());
        };
        match arc_through(start, sp, end) {
            None => c.log_error("The three points are collinear."),
            Some(a) => {
                c.add(Entity::arc_seg(&a));
            }
        }
        return Ok(());
    }
    if mode == "sce" || mode == "sca" || mode == "scl" {
        let Some(ctr) = c.point(ps("Specify center point of arc").base(start)).await?.pt() else { return Ok(()) };
        return finish_center(&c, ctr, start, &mode[1..]).await;
    }
    // start, end ...
    let Some(end) = c.point(ps("Specify end point of arc").base(start).preview(line_pv(start))).await?.pt() else { return Ok(()) };
    start_end(&c, start, Some(end), &mode).await
}

async fn finish_center(c: &Ctx, ctr: Pt, start: Pt, kind0: &str) -> CmdResult<()> {
    let r = dist(ctr, start);
    let mut kind = kind0.to_string();
    loop {
        match kind.as_str() {
            "se" | "ce" => {
                // end point
                let e = c
                    .point(ps("Specify end point of arc").options(&["Angle", "chord Length"]).base(ctr).preview(move |_, p| if dist(ctr, p) > 1e-12 { vec![pv(arc_cse(ctr, start, p))] } else { vec![] }))
                    .await?;
                match e.kw() {
                    Some("Angle") => kind = "ca".into(),
                    Some("chord Length") => kind = "cl".into(),
                    _ => {
                        if let Some(p) = e.pt() {
                            c.add(arc_cse(ctr, start, p));
                        }
                        return Ok(());
                    }
                }
            }
            "ca" | "sa" => {
                let a = c.angle(ps("Specify included angle").base(ctr)).await?;
                if let Some(v) = a.value() {
                    c.add(arc_csa(ctr, start, v));
                }
                return Ok(());
            }
            "cl" | "sl" => {
                let l = c.distance(ps("Specify length of chord").min(0.0).nonzero()).await?;
                let Some(lv) = l.value() else { return Ok(()) };
                if lv > 2.0 * r {
                    c.log_error("The chord is longer than the diameter.");
                    return Ok(());
                }
                c.add(arc_csa(ctr, start, 2.0 * (lv / (2.0 * r)).asin()));
                return Ok(());
            }
            _ => return Ok(()),
        }
    }
}

async fn start_end(c: &Ctx, start: Pt, end: Option<Pt>, mode0: &str) -> CmdResult<()> {
    let end = match end {
        Some(e) => e,
        None => {
            let Some(e) = c.point(ps("Specify end point of arc").base(start).preview(line_pv(start))).await?.pt() else { return Ok(()) };
            e
        }
    };
    let mut mode = mode0.to_string();
    loop {
        match mode.as_str() {
            "sea" => {
                let a = c.angle(ps("Specify included angle")).await?;
                if let Some(v) = a.value() {
                    keep(c, arc_sea(start, end, v));
                }
                return Ok(());
            }
            "sed" => {
                let d = c.point(ps("Specify tangent direction for the start point of arc").base(start)).await?;
                if let Some(p) = d.pt() {
                    keep(c, arc_sed(start, end, unit(p - start)));
                }
                return Ok(());
            }
            "ser" => {
                let r = c.distance(ps("Specify radius of arc").nonzero()).await?;
                if let Some(v) = r.value() {
                    match arc_ser(start, end, v) {
                        Some(a) => {
                            c.add(a);
                        }
                        None => c.log_error("The radius is too small."),
                    }
                }
                return Ok(());
            }
            _ => {
                let r = c.point(ps("Specify center point of arc").options(&["Angle", "Direction", "Radius"]).base(start).preview(move |_, _| vec![pv(Entity::line(start, end))])).await?;
                match r.kw() {
                    Some("Angle") => mode = "sea".into(),
                    Some("Direction") => mode = "sed".into(),
                    Some("Radius") => mode = "ser".into(),
                    _ => {
                        if let Some(p) = r.pt() {
                            c.add(arc_cse(p, start, end));
                        }
                        return Ok(());
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- RECTANG

fn build_rect(doc: &cadforge_core::drawing::Drawing, a: Pt, b: Pt, rot: f64, fillet: f64, chamfer: [f64; 2]) -> Entity {
    let base = Entity::pline(rect_verts(a, b, rot), true);
    if fillet > 0.0 {
        if let Some(e) = fillet_pline(&base, fillet, doc).and_then(|r| r.entity) {
            return e;
        }
    }
    if chamfer[0] > 0.0 || chamfer[1] > 0.0 {
        if let Some(e) = chamfer_pline(&base, chamfer[0], chamfer[1], doc).and_then(|r| r.entity) {
            return e;
        }
    }
    base
}

async fn rectang(c: Ctx, _a: Args) -> CmdResult<()> {
    let (mut fillet, mut chamfer) = c.app(|a| (a.mem.rect_fillet, a.mem.rect_chamfer));
    let first;
    loop {
        let r = c.point(ps("Specify first corner point").options(&["Chamfer", "Fillet"])).await?;
        match r.kw() {
            Some("Chamfer") => {
                let a = c.distance(ps("Specify first chamfer distance for rectangles").default_num(chamfer[0]).min(0.0)).await?;
                let b = c.distance(ps("Specify second chamfer distance for rectangles").default_num(a.value().unwrap_or(chamfer[1])).min(0.0)).await?;
                if let (Some(av), Some(bv)) = (a.value(), b.value()) {
                    chamfer = [av, bv];
                    fillet = 0.0;
                    c.app(|app| {
                        app.mem.rect_chamfer = chamfer;
                        app.mem.rect_fillet = 0.0;
                    });
                }
                continue;
            }
            Some("Fillet") => {
                let f = c.distance(ps("Specify fillet radius for rectangles").default_num(fillet).min(0.0)).await?;
                if let Some(v) = f.value() {
                    fillet = v;
                    chamfer = [0.0, 0.0];
                    c.app(|app| {
                        app.mem.rect_fillet = v;
                        app.mem.rect_chamfer = [0.0, 0.0];
                    });
                }
                continue;
            }
            _ => {}
        }
        let Some(p) = r.pt() else { return Ok(()) };
        first = p;
        break;
    }
    let mut rot = 0.0;
    let sides = move |p: Pt, l: f64, w: f64| pt(first.x + if p.x >= first.x { l } else { -l }, first.y + if p.y >= first.y { w } else { -w });
    loop {
        let r = c
            .corner(
                first,
                ps("Specify other corner point").options(&["Area", "Dimensions", "Rotation"]).preview(move |d, p| {
                    if dist(first, p) > 1e-12 {
                        vec![pv(build_rect(d, first, p, rot, fillet, chamfer))]
                    } else {
                        vec![]
                    }
                }),
            )
            .await?;
        match r.kw() {
            Some("Rotation") => {
                let a = c.angle(ps("Specify rotation angle or [Pick points]").default_num(rot).base(first)).await?;
                if let Some(v) = a.value() {
                    rot = v;
                }
                continue;
            }
            Some("Dimensions") => {
                let l = c.distance(ps("Specify length for rectangles").default_num(10.0).nonzero()).await?;
                let w = c.distance(ps("Specify width for rectangles").default_num(10.0).nonzero()).await?;
                let (Some(lv), Some(wv)) = (l.value(), w.value()) else { return Ok(()) };
                let side = c
                    .point(ps("Specify other corner point or [Dimensions]").base(first).preview(move |d, p| vec![pv(build_rect(d, first, sides(p, lv, wv), rot, fillet, chamfer))]))
                    .await?;
                let Some(sp) = side.pt() else { return Ok(()) };
                let e = c.doc(|d| build_rect(d, first, sides(sp, lv, wv), rot, fillet, chamfer));
                c.add(e);
                return Ok(());
            }
            Some("Area") => {
                let area = c.number(ps("Enter area of rectangle").min(0.0).nonzero().default_num(100.0)).await?;
                let how = c.keyword(ps("Calculate rectangle dimensions based on [Length/Width]").options(&["Length", "Width"]).default_str("Length")).await?;
                let width_mode = how.kw() == Some("Width");
                let v = c.distance(ps(if width_mode { "Enter rectangle width" } else { "Enter rectangle length" }).default_num(10.0).nonzero()).await?;
                let (Some(av), Some(vv)) = (area.value(), v.value()) else { return Ok(()) };
                let other = av / vv;
                let (l, w) = if width_mode { (other, vv) } else { (vv, other) };
                let side = c.point(ps("Specify direction").base(first).preview(move |d, p| vec![pv(build_rect(d, first, sides(p, l, w), rot, fillet, chamfer))])).await?;
                let Some(sp) = side.pt() else { return Ok(()) };
                let e = c.doc(|d| build_rect(d, first, sides(sp, l, w), rot, fillet, chamfer));
                c.add(e);
                return Ok(());
            }
            _ => {}
        }
        let Some(p) = r.pt() else { return Ok(()) };
        if dist(first, p) < 1e-12 {
            return Ok(());
        }
        let e = c.doc(|d| build_rect(d, first, p, rot, fillet, chamfer));
        c.add(e);
        return Ok(());
    }
}

// ---------------------------------------------------------------- POLYGON

fn polygon_verts(center: Pt, n: usize, r: f64, ang: f64, circumscribed: bool) -> Vec<Vertex> {
    let rr = if circumscribed { r / (PI / n as f64).cos() } else { r };
    (0..n)
        .map(|i| {
            let p = polar(center, ang + i as f64 * TAU / n as f64, rr);
            Vertex::new(p.x, p.y, 0.0)
        })
        .collect()
}

fn polygon_edge(p: Pt, q: Pt, n: usize) -> Vec<Entity> {
    let l = dist(p, q);
    if l < 1e-12 {
        return vec![];
    }
    let nf = n as f64;
    let (r, m, h) = (l / (2.0 * (PI / nf).sin()), mid(p, q), l / (2.0 * (PI / nf).tan()));
    let center = m + perp(unit(q - p)) * h;
    let a0 = angle_of(center, p);
    vec![Entity::pline(
        (0..n)
            .map(|i| {
                let v = polar(center, a0 - i as f64 * TAU / nf, r);
                Vertex::new(v.x, v.y, 0.0)
            })
            .collect(),
        true,
    )]
}

async fn polygon(c: Ctx, _a: Args) -> CmdResult<()> {
    let (sides, mode) = c.app(|a| (a.mem.polygon_sides, a.mem.polygon_mode.clone()));
    let ns = c.number(ps("Enter number of sides").default_num(sides as f64).integer().min(3.0).max(1024.0).range_message("Requires an integer between 3 and 1024.")).await?;
    let Some(nv) = ns.value() else { return Ok(()) };
    let n = nv as usize;
    c.app(|a| a.mem.polygon_sides = n as i64);
    let r = c.point(ps("Specify center of polygon").options(&["Edge"])).await?;
    if r.kw() == Some("Edge") {
        let Some(a) = c.point(ps("Specify first endpoint of edge")).await?.pt() else { return Ok(()) };
        let Some(b) = c.point(ps("Specify second endpoint of edge").base(a).preview(move |_, p| pvs(polygon_edge(a, p, n)))).await?.pt() else { return Ok(()) };
        if let Some(e) = polygon_edge(a, b, n).into_iter().next() {
            c.add(e);
        }
        return Ok(());
    }
    let Some(center) = r.pt() else { return Ok(()) };
    let how = c
        .keyword(
            ps("Enter an option [Inscribed in circle/Circumscribed about circle]")
                .options(&["Inscribed in circle", "Circumscribed about circle"])
                .default_str(if mode == "Circumscribed" { "Circumscribed about circle" } else { "Inscribed in circle" }),
        )
        .await?;
    let circ = how.kw() == Some("Circumscribed about circle");
    c.app(|a| a.mem.polygon_mode = if circ { "Circumscribed".into() } else { "Inscribed".into() });
    let build = move |p: Pt| -> Vec<Entity> {
        let rr = dist(center, p);
        if rr < 1e-12 {
            return vec![];
        }
        let ang = angle_of(center, p) - if circ { PI / n as f64 } else { 0.0 };
        vec![Entity::pline(polygon_verts(center, n, rr, ang, circ), true)]
    };
    let Some(rp) = c.point(ps("Specify radius of circle").base(center).preview(move |_, p| pvs(build(p)))).await?.pt() else { return Ok(()) };
    if let Some(e) = build(rp).into_iter().next() {
        c.add(e);
    }
    Ok(())
}

// ---------------------------------------------------------------- ELLIPSE

async fn ellipse(c: Ctx, args: Args) -> CmdResult<()> {
    let mut mode: Option<String> = args.str("mode").map(str::to_string);
    let mut arc = args.flag("arc");
    let mut axis_end: Option<Pt> = None;
    if mode.is_none() {
        let r = c.point(ps("Specify axis endpoint of ellipse").options(&["Arc", "Center"])).await?;
        match r.kw() {
            Some("Arc") => {
                arc = true;
                let r2 = c.point(ps("Specify axis endpoint of elliptical arc").options(&["Center"])).await?;
                if r2.kw() == Some("Center") {
                    mode = Some("center".into());
                } else if let Some(p) = r2.pt() {
                    axis_end = Some(p);
                } else {
                    return Ok(());
                }
            }
            Some("Center") => mode = Some("center".into()),
            _ => match r.pt() {
                Some(p) => axis_end = Some(p),
                None => return Ok(()),
            },
        }
    }
    let (center, m): (Pt, Pt);
    if mode.as_deref() == Some("center") {
        let Some(cc) = c.point(ps("Specify center of ellipse")).await?.pt() else { return Ok(()) };
        let Some(ae) = c.point(ps("Specify endpoint of axis").base(cc).preview(line_pv(cc))).await?.pt() else { return Ok(()) };
        center = cc;
        m = ae - cc;
    } else {
        let ae = axis_end.expect("axis endpoint");
        let Some(other) = c.point(ps("Specify other endpoint of axis").base(ae).preview(line_pv(ae))).await?.pt() else { return Ok(()) };
        center = mid(ae, other);
        m = other - center;
    }
    if len(m) < 1e-12 {
        return Ok(());
    }
    let ell = move |p: Pt| -> Option<Entity> {
        let l = dist(center, p);
        if l < 1e-12 {
            None
        } else {
            Some(Entity::ellipse(center, m, (l / len(m)).min(1.0), 0.0, TAU))
        }
    };
    let d = c.point(ps("Specify distance to other axis").options(&["Rotation"]).base(center).preview(move |_, p| ell(p).map(|e| vec![pv(e)]).unwrap_or_default())).await?;
    let ratio = if d.kw() == Some("Rotation") {
        let a = c.angle(ps("Specify rotation around major axis").base(center)).await?;
        let Some(v) = a.value() else { return Ok(()) };
        v.cos()
    } else if let Some(p) = d.pt() {
        let n = unit(pt(-m.y, m.x));
        dot(p - center, n).abs() / len(m)
    } else {
        return Ok(());
    };
    let ratio = ratio.clamp(1e-4, 1.0);
    if !arc {
        c.add(Entity::ellipse(center, m, ratio, 0.0, TAU));
        return Ok(());
    }
    let param = move |p: Pt| -> f64 {
        let v = p - center;
        let x = dot(v, unit(m)) / len(m);
        let y = dot(v, unit(pt(-m.y, m.x))) / (len(m) * ratio);
        y.atan2(x)
    };
    let Some(sp) = c.point(ps("Specify start angle").options(&["Parameter"]).base(center).preview(line_pv(center))).await?.pt() else { return Ok(()) };
    let t0 = param(sp);
    let Some(ep) = c.point(ps("Specify end angle").base(center).preview(move |_, p| vec![pv(Entity::ellipse(center, m, ratio, t0, norm_angle(param(p))))])).await?.pt() else { return Ok(()) };
    c.add(Entity::ellipse(center, m, ratio, norm_angle(t0), norm_angle(param(ep))));
    Ok(())
}

// ---------------------------------------------------------------- SPLINE

/// A closed control-vertex spline: the first `deg` vertices repeated at the end with uniform knots (periodic).
fn closed_cv(deg: usize, cv: &[Pt]) -> Option<cadforge_core::nurbs::Nurbs> {
    let deg = deg.clamp(1, cv.len().saturating_sub(1).max(1));
    let mut all = cv.to_vec();
    all.extend_from_slice(&cv[..deg.min(cv.len())]);
    let knots: Vec<f64> = (0..all.len() + deg + 1).map(|i| i as f64).collect();
    cadforge_core::nurbs::Nurbs::new(deg, all, vec![], knots)
}

/// SPLINE with the control vertex method: the points are the control polygon of a curve of degree `deg`.
async fn spline_cv(c: &Ctx, first: Pt, deg: usize) -> CmdResult<()> {
    use cadforge_core::nurbs::Nurbs;
    let mut cv = vec![first];
    loop {
        let snapshot = cv.clone();
        let opts: &[&str] = if cv.len() > 2 { &["Undo", "Close"] } else { &["Undo"] };
        let r = c
            .point(ps("Enter next point").options(opts).base(*cv.last().expect("vertex")).allow_enter().preview(move |_, p| {
                let mut v = snapshot.clone();
                v.push(p);
                vec![pv(Entity::nurbs(Nurbs::clamped(deg, v.clone()))), pv(Entity::pline_pts(&v, false))]
            }))
            .await?;
        match r.kw() {
            Some("Undo") => {
                if cv.len() > 1 {
                    cv.pop();
                }
                continue;
            }
            Some("Close") => {
                if let Some(n) = closed_cv(deg, &cv) {
                    c.add(Entity::nurbs(n));
                }
                return Ok(());
            }
            _ => {}
        }
        let Some(p) = r.pt() else { break };
        cv.push(p);
    }
    if cv.len() > 1 {
        c.add(Entity::nurbs(Nurbs::clamped(deg, cv)));
    }
    Ok(())
}

async fn spline(c: Ctx, _a: Args) -> CmdResult<()> {
    // Method and Degree are remembered (SPLMETHOD, SPLDEGREE)
    let first = loop {
        let cv = c.pref("SPLMETHOD") == 1.0;
        let (label, opts): (&str, &[&str]) = if cv { ("Specify first point or [Method/Degree]", &["Method", "Degree"]) } else { ("Specify first point or [Method]", &["Method"]) };
        let r = c.point(ps(label).options(opts)).await?;
        match r.kw() {
            Some("Method") => {
                let cur = if cv { "CV" } else { "Fit" };
                let m = c.keyword(ps("Enter spline creation method [Fit/CV]").options(&["Fit", "CV"]).default_str(cur)).await?;
                let _ = c.setvar("SPLMETHOD", if m.kw() == Some("CV") { 1.0 } else { 0.0 });
            }
            Some("Degree") => {
                if let Some(d) = c.number(ps("Enter degree of spline").default_num(c.pref("SPLDEGREE").clamp(1.0, 10.0)).integer().min(1.0).max(10.0)).await?.value() {
                    let _ = c.setvar("SPLDEGREE", d);
                }
            }
            _ => match r.pt() {
                Some(p) => break p,
                None => return Ok(()),
            },
        }
    };
    if c.pref("SPLMETHOD") == 1.0 {
        return spline_cv(&c, first, c.pref("SPLDEGREE").clamp(1.0, 10.0) as usize).await;
    }
    let mut fit = vec![first];
    let mut closed = false;
    loop {
        let snapshot = fit.clone();
        let r = c
            .point(
                ps("Enter next point")
                    .options(if fit.len() > 2 { &["Undo", "Close"] } else { &["Undo"] })
                    .base(*fit.last().expect("fit point"))
                    .allow_enter()
                    .preview(move |_, p| {
                        let mut f = snapshot.clone();
                        f.push(p);
                        vec![pv(Entity::spline(f, false))]
                    }),
            )
            .await?;
        match r.kw() {
            Some("Undo") => {
                if fit.len() > 1 {
                    fit.pop();
                }
                continue;
            }
            Some("Close") => {
                closed = true;
                break;
            }
            _ => {}
        }
        let Some(p) = r.pt() else { break };
        fit.push(p);
    }
    if fit.len() > 1 {
        c.add(Entity::spline(fit, closed));
    }
    Ok(())
}

// ---------------------------------------------------------------- POINT / XLINE / RAY / DONUT

async fn point(c: Ctx, args: Args) -> CmdResult<()> {
    let pd = c.var("PDMODE").map(|v| v.text()).unwrap_or_default();
    let ps_ = c.var("PDSIZE").ok().and_then(|v| v.num()).unwrap_or(0.0);
    c.log(&format!("Current point modes: PDMODE={pd}  PDSIZE={}", num(ps_, 4)));
    let multiple = args.flag("multiple");
    loop {
        let r = c.point(if multiple { ps("Specify a point").allow_enter() } else { ps("Specify a point") }).await?;
        let Some(p) = r.pt() else { return Ok(()) };
        c.add(Entity::point(p));
        if !multiple {
            return Ok(());
        }
    }
}

async fn xline(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut mode_dir = false;
    let mut angle = 0.0;
    let mut p0: Option<Pt> = None;
    loop {
        if !mode_dir {
            let base = p0;
            let mut spec = ps(if p0.is_some() { "Specify through point" } else { "Specify a point" }).allow_enter().base_opt(base).preview(move |_, p| match base {
                Some(b) if !same(b, p) => vec![pv(Entity::xline(b, p - b))],
                _ => vec![],
            });
            if p0.is_none() {
                spec = spec.options(&["Hor", "Ver", "Ang", "Bisect", "Offset"]).no_ortho();
            }
            let r = c.point(spec).await?;
            match r.kw() {
                Some("Hor") => {
                    mode_dir = true;
                    angle = 0.0;
                    continue;
                }
                Some("Ver") => {
                    mode_dir = true;
                    angle = PI / 2.0;
                    continue;
                }
                Some("Ang") => {
                    let a = c.angle(ps("Enter angle of xline (0) or [Reference]").default_num(0.0)).await?;
                    let Some(v) = a.value() else { return Ok(()) };
                    angle = v;
                    mode_dir = true;
                    continue;
                }
                Some("Bisect") => {
                    let Some(v) = c.point(ps("Specify angle vertex point")).await?.pt() else { return Ok(()) };
                    let Some(s) = c.point(ps("Specify angle start point").base(v)).await?.pt() else { return Ok(()) };
                    let e = c
                        .point(ps("Specify angle end point").base(v).allow_enter().preview(move |_, p| vec![pv(Entity::xline(v, unit(s - v) + unit(p - v)))]))
                        .await?;
                    let Some(ep) = e.pt() else { return Ok(()) };
                    c.add(Entity::xline(v, unit(s - v) + unit(ep - v)));
                    return Ok(());
                }
                Some("Offset") => {
                    c.log("Use the OFFSET command on a construction line.");
                    return Ok(());
                }
                _ => {}
            }
            let Some(p) = r.pt() else { return Ok(()) };
            match p0 {
                None => p0 = Some(p),
                Some(b) => {
                    c.add(Entity::xline(b, p - b));
                }
            }
        } else {
            let d = pt(angle.cos(), angle.sin());
            let r = c.point(ps("Specify through point").allow_enter().preview(move |_, p| vec![pv(Entity::xline(p, d))])).await?;
            let Some(p) = r.pt() else { return Ok(()) };
            c.add(Entity::xline(p, d));
        }
    }
}

async fn ray(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(s) = c.point(ps("Specify start point")).await?.pt() else { return Ok(()) };
    loop {
        let r = c.point(ps("Specify through point").base(s).allow_enter().preview(move |_, p| if same(p, s) { vec![] } else { vec![pv(Entity::ray(s, p - s))] })).await?;
        let Some(p) = r.pt() else { return Ok(()) };
        if !same(p, s) {
            c.add(Entity::ray(s, p - s));
        }
    }
}

async fn donut(c: Ctx, _a: Args) -> CmdResult<()> {
    let (din, dout) = c.app(|a| (a.mem.donut_in, a.mem.donut_out));
    let inner = c.distance(ps("Specify inside diameter of donut").default_num(din).min(0.0)).await?;
    let Some(iv) = inner.value() else { return Ok(()) };
    let outer = c.distance(ps("Specify outside diameter of donut").default_num(dout).min(0.0).nonzero()).await?;
    let Some(ov) = outer.value() else { return Ok(()) };
    c.app(|a| {
        a.mem.donut_in = iv;
        a.mem.donut_out = ov;
    });
    if ov <= iv {
        c.log_error("The outside diameter must be larger than the inside diameter.");
        return Ok(());
    }
    let loops = move |ctr: Pt| -> Vec<Vec<Vertex>> {
        let ring = |r: f64| if r > 0.0 { Some(vec![Vertex::new(ctr.x + r, ctr.y, 1.0), Vertex::new(ctr.x - r, ctr.y, 1.0)]) } else { None };
        [ring(ov / 2.0), ring(iv / 2.0)].into_iter().flatten().collect()
    };
    loop {
        let r = c.point(ps("Specify center of donut or <exit>").allow_enter().preview(move |_, p| vec![pv(Entity::hatch(loops(p), "SOLID"))])).await?;
        let Some(p) = r.pt() else { return Ok(()) };
        c.add(Entity::hatch(loops(p), "SOLID"));
    }
}

pub fn register(r: &mut Registry) {
    r.define(def("Draw", "LINE", run(line)).aliases(&["L"]).title("Line").icon("line").desc("Creates straight line segments."));
    r.define(def("Draw", "PLINE", run(pline)).aliases(&["PL"]).title("Polyline").icon("pline").desc("Creates a 2D polyline: a single object made of connected line and arc segments."));
    r.define(def("Draw", "CIRCLE", run(circle)).aliases(&["C"]).title("Circle").icon("circle").desc("Creates a circle."));
    r.define(def("Draw", "ARC", run(arc)).aliases(&["A"]).title("Arc").icon("arc").desc("Creates an arc."));
    r.define(def("Draw", "RECTANG", run(rectang)).aliases(&["REC", "RECTANGLE"]).title("Rectangle").icon("rectangle").desc("Creates a rectangular polyline."));
    r.define(def("Draw", "POLYGON", run(polygon)).aliases(&["POL"]).title("Polygon").icon("polygon").desc("Creates an equilateral closed polyline."));
    r.define(def("Draw", "ELLIPSE", run(ellipse)).aliases(&["EL"]).title("Ellipse").icon("ellipse").desc("Creates an ellipse or an elliptical arc."));
    r.define(def("Draw", "SPLINE", run(spline)).aliases(&["SPL"]).title("Spline").icon("spline").desc("Creates a smooth curve through fit points."));
    r.define(def("Draw", "POINT", run(point)).aliases(&["PO"]).title("Point").icon("point").desc("Creates a point object."));
    r.define(def("Draw", "XLINE", run(xline)).aliases(&["XL"]).title("Construction Line").icon("xline").desc("Creates a line of infinite length."));
    r.define(def("Draw", "RAY", run(ray)).title("Ray").icon("ray").desc("Creates a semi-infinite line."));
    r.define(def("Draw", "DONUT", run(donut)).aliases(&["DO"]).title("Donut").icon("donut").desc("Creates a filled circle or a wide ring."));
}
