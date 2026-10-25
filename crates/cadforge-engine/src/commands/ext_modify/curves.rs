//! BLEND and SPLINEDIT.
use super::{dm, ps};
use crate::commands::helpers::*;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

// ---------------------------------------------------------------- BLEND

/// The end of an open curve: its position, the unit direction in which the curve would continue past the end, and
/// the signed curvature along that direction (positive turns left).
struct End {
    p: Pt,
    t: Pt,
    k: f64,
}

fn curvature(s: &Seg) -> f64 {
    match s {
        Seg::A { r, sw, .. } => sw.signum() / r,
        Seg::L { .. } => 0.0,
    }
}

/// The end of `segs` nearest to `pick`.
fn end_near(segs: &[Seg], pick: Pt) -> End {
    let (first, last) = (segs[0], segs[segs.len() - 1]);
    if dist(first.start(), pick) < dist(last.end(), pick) {
        End { p: first.start(), t: -first.tangent(0.0), k: -curvature(&first) }
    } else {
        End { p: last.end(), t: last.tangent(1.0), k: curvature(&last) }
    }
}

/// Quintic Hermite curve from `a` out along its continuation and into `b` against its continuation. G1 blends use
/// zero end acceleration; smooth (G2) blends match the curvature of both curves.
fn blend_points(a: &End, b: &End, smooth: bool, n: usize) -> Vec<Pt> {
    let m = dist(a.p, b.p);
    let (v0, v1) = (a.t * m, -b.t * m);
    let (a0, a1) = if smooth { (perp(a.t) * (a.k * m * m), perp(-b.t) * (-b.k * m * m)) } else { (Pt::ZERO, Pt::ZERO) };
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let (t2, t3) = (t * t, t * t * t);
            let (t4, t5) = (t3 * t, t3 * t2);
            let h = [1.0 - 10.0 * t3 + 15.0 * t4 - 6.0 * t5, t - 6.0 * t3 + 8.0 * t4 - 3.0 * t5, 0.5 * t2 - 1.5 * t3 + 1.5 * t4 - 0.5 * t5, 0.5 * t3 - t4 + 0.5 * t5, -4.0 * t3 + 7.0 * t4 - 3.0 * t5, 10.0 * t3 - 15.0 * t4 + 6.0 * t5];
            a.p * h[0] + v0 * h[1] + a0 * h[2] + a1 * h[3] + v1 * h[4] + b.p * h[5]
        })
        .collect()
}

fn blendable(e: &Entity) -> bool {
    match &e.kind {
        Kind::Line { .. } | Kind::Arc { .. } => true,
        Kind::Pline { closed, .. } | Kind::Spline { closed, .. } => !closed,
        Kind::Ellipse(el) => !ellipse_is_full(el),
        _ => false,
    }
}

/// Creates a spline between the ends of two open curves, tangent (or curvature) continuous with both. The blend is a
/// fit-point spline sampled from the Hermite curve; the source curves are not changed.
async fn blend(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut smooth = false;
    let first = loop {
        let r = c.entity(ps("Select first object or").options(&["CONtinuity"]).filter(blendable)).await?;
        match r {
            EntR::Kw(_) => {
                let k = c.keyword(ps("Enter continuity").options(&["Tangent", "Smooth"]).default_str(if smooth { "Smooth" } else { "Tangent" })).await?;
                smooth = k.kw() == Some("Smooth");
            }
            EntR::Entity { id, pt, .. } => break (id, pt),
            EntR::Enter => return Ok(()),
        }
    };
    c.ed(|e| e.temp_sel = [first.0].into_iter().collect());
    let EntR::Entity { id: id2, pt: pick2, .. } = c.entity(ps("Select second point on object").filter(blendable)).await? else { return Ok(()) };
    if id2 == first.0 {
        c.log_error("Select two different objects.");
        return Ok(());
    }
    let (Some(e1), Some(e2)) = (c.entity_by_id(first.0), c.entity_by_id(id2)) else { return Ok(()) };
    let (a, b) = c.doc(|d| (end_near(&segs_of(&e1, d), first.1), end_near(&segs_of(&e2, d), pick2)));
    if dist(a.p, b.p) < 1e-9 {
        c.log_error("The selected ends already meet.");
        return Ok(());
    }
    c.add(Entity::spline(blend_points(&a, &b, smooth, 16), false));
    Ok(())
}

// ---------------------------------------------------------------- SPLINEDIT

/// Fit points and closed flag of a spline.
fn spline_of(c: &Ctx, id: u64) -> Option<(Vec<Pt>, bool)> {
    match c.entity_by_id(id)?.kind {
        Kind::Spline { fit, closed, .. } => Some((fit, closed)),
        _ => None,
    }
}

/// Store fit points: the curve is refitted through them (a control-vertex definition it had is dropped).
fn store_spline(c: &Ctx, id: u64, fit: Vec<Pt>, closed: bool) {
    c.doc_mut(|d| d.modify(id, |e| e.kind = Kind::Spline { fit, closed, nurbs: None }));
}

/// The control-vertex definition of a spline made of control vertices (no fit points).
fn nurbs_of(c: &Ctx, id: u64) -> Option<cadforge_core::nurbs::Nurbs> {
    match c.entity_by_id(id)?.kind {
        Kind::Spline { fit, nurbs: Some(n), .. } if fit.is_empty() => Some(*n),
        _ => None,
    }
}

fn store_nurbs(c: &Ctx, id: u64, n: cadforge_core::nurbs::Nurbs) {
    c.doc_mut(|d| d.modify(id, |e| e.kind = Entity::nurbs(n.clone()).kind));
}

/// Edit vertex: move control vertices or change their weights.
async fn edit_vertex(c: &Ctx, id: u64, hist: &mut Vec<cadforge_core::nurbs::Nurbs>) -> CmdResult<()> {
    let mut at = 0usize;
    loop {
        let Some(n) = nurbs_of(c, id) else { return Ok(()) };
        let r = c.keyword(ps("Enter a vertex editing option").options(&["Move", "Weight", "eXit"]).default_str("eXit")).await?;
        match r.kw() {
            Some("Move") => loop {
                let Some(n) = nurbs_of(c, id) else { return Ok(()) };
                let count = n.cv.len();
                at = at.min(count - 1);
                let snapshot = n.clone();
                let spec = ps("Specify new location or").options(&["Next", "Previous", "Select point", "eXit"]).default_str("N").allow_enter().no_ortho().base(n.cv[at]).preview(move |_, p| {
                    let mut m = snapshot.clone();
                    m.cv[at] = p;
                    vec![pv(Entity::nurbs(m))]
                });
                match c.point(spec).await? {
                    PointR::Kw(k) if k == "Next" => at = (at + 1) % count,
                    PointR::Kw(k) if k == "Previous" => at = (at + count - 1) % count,
                    PointR::Kw(k) if k == "Select point" => {
                        if let Some(p) = c.point(ps("Select point")).await?.pt() {
                            at = nearest_fit(&n.cv, p);
                        }
                    }
                    PointR::Kw(_) => break,
                    PointR::Enter(_) => at = (at + 1) % count,
                    other => {
                        if let Some(p) = other.pt() {
                            hist.push(n.clone());
                            let mut m = n.clone();
                            m.cv[at] = p;
                            store_nurbs(c, id, m);
                            at = (at + 1).min(count - 1);
                        }
                    }
                }
            },
            Some("Weight") => {
                let Some(p) = c.point(ps("Select control vertex")).await?.pt() else { continue };
                let i = nearest_fit(&n.cv, p);
                let Some(w) = c.number(ps("Enter new weight").default_num(n.weight(i)).min(0.0).nonzero()).await?.value() else { continue };
                hist.push(n.clone());
                let mut m = n.clone();
                if m.w.is_empty() {
                    m.w = vec![1.0; m.cv.len()];
                }
                m.w[i] = w;
                if m.w.iter().all(|x| (*x - 1.0).abs() < 1e-12) {
                    m.w.clear();
                }
                store_nurbs(c, id, m);
            }
            _ => return Ok(()),
        }
    }
}

/// Index of the fit point nearest `p`.
fn nearest_fit(fit: &[Pt], p: Pt) -> usize {
    (0..fit.len()).min_by(|&i, &j| dist(fit[i], p).total_cmp(&dist(fit[j], p))).unwrap_or(0)
}

type History = Vec<(Vec<Pt>, bool)>;

/// Fit data: move, add and delete fit points.
async fn fit_data(c: &Ctx, id: u64, hist: &mut History) -> CmdResult<()> {
    let mut at = 0usize;
    loop {
        let Some((fit, closed)) = spline_of(c, id) else { return Ok(()) };
        let r = c.keyword(ps("Enter a fit data option").options(&["Add", "Delete", "Move", "eXit"]).default_str("eXit")).await?;
        match r.kw() {
            Some("Move") => loop {
                let Some((fit, closed)) = spline_of(c, id) else { return Ok(()) };
                let (n, snapshot) = (fit.len(), fit.clone());
                at = at.min(n - 1);
                let spec = ps("Specify new location or").options(&["Next", "Previous", "Select point", "eXit"]).default_str("N").allow_enter().no_ortho().base(fit[at]).preview(move |_, p| {
                    let mut f = snapshot.clone();
                    f[at] = p;
                    vec![pv(Entity::spline(f, closed))]
                });
                match c.point(spec).await? {
                    PointR::Kw(k) if k == "Next" => at = (at + 1) % n,
                    PointR::Kw(k) if k == "Previous" => at = (at + n - 1) % n,
                    PointR::Kw(k) if k == "Select point" => {
                        if let Some(p) = c.point(ps("Select point")).await?.pt() {
                            at = nearest_fit(&fit, p);
                        }
                    }
                    PointR::Kw(_) => break,
                    PointR::Enter(_) => at = (at + 1) % n,
                    other => {
                        if let Some(p) = other.pt() {
                            hist.push((fit.clone(), closed));
                            let mut f = fit.clone();
                            f[at] = p;
                            store_spline(c, id, f, closed);
                            at = if closed { (at + 1) % n } else { (at + 1).min(n - 1) };
                        }
                    }
                }
            },
            Some("Delete") => {
                let need = if closed { 3 } else { 2 };
                if fit.len() <= need {
                    c.log_error(&format!("A spline needs at least {need} fit points."));
                    continue;
                }
                if let Some(p) = c.point(ps("Select fit point to delete")).await?.pt() {
                    hist.push((fit.clone(), closed));
                    let mut f = fit.clone();
                    f.remove(nearest_fit(&fit, p));
                    store_spline(c, id, f, closed);
                }
            }
            Some("Add") => {
                let Some(p) = c.point(ps("Select fit point to add after")).await?.pt() else { continue };
                let i = nearest_fit(&fit, p);
                let Some(q) = c.point(ps("Specify new fit point").base(fit[i])).await?.pt() else { continue };
                hist.push((fit.clone(), closed));
                let mut f = fit.clone();
                f.insert(i + 1, q);
                store_spline(c, id, f, closed);
            }
            _ => return Ok(()),
        }
    }
}

/// Edits a spline: a fit-point spline's fit data (move, add, delete), close / open, reverse and undo; a control-vertex
/// spline's vertices (move, weight) and direction.
async fn splineedit(c: Ctx, _a: Args) -> CmdResult<()> {
    let pre: Vec<u64> = c.ed(|e| e.selected().into_iter().filter(|x| matches!(x.kind, Kind::Spline { .. })).map(|x| x.id).collect());
    let id = match pre[..] {
        [one] => one,
        _ => match c.pick_entity(ps("Select spline").filter(|e| matches!(e.kind, Kind::Spline { .. }))).await? {
            Some(id) => id,
            None => return Ok(()),
        },
    };
    let mut hist: History = vec![];
    // a spline made of control vertices: edit them
    if nurbs_of(&c, id).is_some() {
        let mut cv_hist: Vec<cadforge_core::nurbs::Nurbs> = vec![];
        loop {
            let Some(n) = nurbs_of(&c, id) else { return Ok(()) };
            let r = c.keyword(ps("Enter an option").options(&["Edit vertex", "rEverse", "Undo", "eXit"]).default_str("eXit")).await?;
            match r.kw() {
                Some("Edit vertex") => edit_vertex(&c, id, &mut cv_hist).await?,
                Some("rEverse") => {
                    cv_hist.push(n.clone());
                    store_nurbs(&c, id, n.reversed());
                }
                Some("Undo") => match cv_hist.pop() {
                    Some(m) => store_nurbs(&c, id, m),
                    None => c.log("Nothing to undo."),
                },
                _ => return Ok(()),
            }
        }
    }
    loop {
        let Some((fit, closed)) = spline_of(&c, id) else { return Ok(()) };
        let opts = ["Fit data", if closed { "Open" } else { "Close" }, "rEverse", "Undo", "eXit"];
        let r = c.keyword(ps("Enter an option").options(&opts).default_str("eXit")).await?;
        match r.kw() {
            Some("Fit data") => fit_data(&c, id, &mut hist).await?,
            Some("Close") => {
                if fit.len() < 3 {
                    c.log_error("A closed spline needs at least three fit points.");
                } else {
                    hist.push((fit.clone(), closed));
                    store_spline(&c, id, fit, true);
                }
            }
            Some("Open") => {
                hist.push((fit.clone(), closed));
                store_spline(&c, id, fit, false);
            }
            Some("rEverse") => {
                hist.push((fit.clone(), closed));
                store_spline(&c, id, fit.into_iter().rev().collect(), closed);
            }
            Some("Undo") => match hist.pop() {
                Some((f, cl)) => store_spline(&c, id, f, cl),
                None => c.log("Nothing to undo."),
            },
            _ => return Ok(()),
        }
    }
}

pub fn register(r: &mut Registry) {
    r.define(dm("BLEND", run(blend)).title("Blend Curve").icon("spline").desc("Creates a spline in the gap between two selected lines or open curves (tangent or smooth continuity)."));
    r.define(dm("SPLINEDIT", run(splineedit)).aliases(&["SPE"]).title("Edit Spline").icon("pedit").desc("Edits a spline: fit points (move, add, delete, close, open) or control vertices (move, weight), and its direction."));
}
