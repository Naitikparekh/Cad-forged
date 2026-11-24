//! Parametric constraints: GEOMCONSTRAINT and its GC* shortcuts, DIMCONSTRAINT and its DC* shortcuts, AUTOCONSTRAIN,
//! DELCONSTRAINT, PARAMETERS, CONSTRAINTBAR and DCDISPLAY. The solver is in `cadforge_core::constraints`; the drawing
//! solves the constraints again at the end of every command that moves constrained objects.
use crate::commands::helpers::pv;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::constraints::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use std::collections::{HashMap, HashSet};

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn is_line(e: &Entity) -> bool {
    matches!(e.kind, Kind::Line { .. } | Kind::Pline { .. })
}

fn is_circle(e: &Entity) -> bool {
    matches!(e.kind, Kind::Circle { .. } | Kind::Arc { .. })
}

fn in_model(c: &Ctx) -> bool {
    let ok = c.doc(|d| d.space == "model");
    if !ok {
        c.log_error("Constraints apply to model space objects.");
    }
    ok
}

/// Add constraint `con` and solve, keeping the objects in `keep` where they are. A constraint the geometry cannot
/// meet together with the others is not added.
fn apply(c: &Ctx, con: Constraint, keep: &[u64]) -> bool {
    let keep: HashSet<u64> = keep.iter().copied().collect();
    let label = con.kind.label();
    let r = c.doc_mut(|d| {
        d.change(|d| {
            d.model.constraints.list.push(con);
            let r = solve(d, &keep);
            if !r.ok {
                d.model.constraints.list.pop();
            }
            r
        })
    });
    if r.ok {
        c.log(&format!("{label} constraint applied. {}", dof_text(r.dof)));
    } else {
        c.log_error(&format!("{label} constraint not applied: {}", r.message));
    }
    r.ok
}

fn dof_text(dof: usize) -> String {
    if dof == 0 {
        "The constrained objects are fully constrained.".into()
    } else {
        format!("{dof} degree{} of freedom left.", if dof == 1 { "" } else { "s" })
    }
}

// ---------------------------------------------------------------- picking

/// Pick an object that `ok` accepts; returns it with the picked point.
async fn pick(c: &Ctx, msg: &str, ok: impl Fn(&Entity) -> bool + 'static) -> CmdResult<Option<(Entity, Pt)>> {
    match c.entity(ps(msg).filter(move |e| ok(e))).await? {
        EntR::Entity { id, pt: p, .. } => Ok(c.entity_by_id(id).map(|e| (e, p))),
        _ => Ok(None),
    }
}

async fn pick_point(c: &Ctx, msg: &str) -> CmdResult<Option<PRef>> {
    Ok(pick(c, msg, constrainable).await?.and_then(|(e, p)| point_ref_near(&e, p)))
}

async fn pick_line(c: &Ctx, msg: &str) -> CmdResult<Option<PRef>> {
    Ok(pick(c, msg, is_line).await?.and_then(|(e, p)| line_ref_near(&e, p)))
}

async fn pick_circle(c: &Ctx, msg: &str) -> CmdResult<Option<PRef>> {
    Ok(pick(c, msg, is_circle).await?.and_then(|(e, _)| circle_ref(&e)))
}

/// A line (or polyline segment) or a circle / arc.
async fn pick_curve(c: &Ctx, msg: &str) -> CmdResult<Option<PRef>> {
    Ok(pick(c, msg, |e| is_line(e) || is_circle(e)).await?.and_then(|(e, p)| if is_circle(&e) { circle_ref(&e) } else { line_ref_near(&e, p) }))
}

// ---------------------------------------------------------------- geometric constraints

async fn geometric(c: &Ctx, kind: CKind) -> CmdResult<()> {
    if !in_model(c) {
        return Ok(());
    }
    let made: Option<(Vec<PRef>, Vec<u64>)> = match kind {
        CKind::Horizontal | CKind::Vertical => {
            let r = c.entity(ps("Select an object or").options(&["2Points"]).default_str("2Points").allow_enter().filter(is_line)).await?;
            match r {
                EntR::Entity { id, pt: p, .. } => c.entity_by_id(id).and_then(|e| line_ref_near(&e, p)).map(|r| (vec![r], vec![])),
                _ => {
                    let Some(a) = pick_point(c, "Select first point").await? else { return Ok(()) };
                    let Some(b) = pick_point(c, "Select second point").await? else { return Ok(()) };
                    Some((vec![a, b], vec![a.ent]))
                }
            }
        }
        CKind::Parallel | CKind::Perpendicular | CKind::Collinear => {
            let Some(a) = pick_line(c, "Select first object").await? else { return Ok(()) };
            let Some(b) = pick_line(c, "Select second object").await? else { return Ok(()) };
            Some((vec![a, b], vec![a.ent]))
        }
        CKind::Coincident => {
            let Some(a) = pick_point(c, "Select first point").await? else { return Ok(()) };
            let r = c.entity(ps("Select second point or").options(&["Object"]).filter(constrainable)).await?;
            let b = match r {
                EntR::Entity { id, pt: p, .. } => c.entity_by_id(id).and_then(|e| point_ref_near(&e, p)),
                EntR::Kw(_) => pick_curve(c, "Select object").await?,
                EntR::Enter => None,
            };
            let Some(b) = b else { return Ok(()) };
            Some((vec![a, b], vec![a.ent]))
        }
        CKind::Concentric => {
            let Some(a) = pick_circle(c, "Select first object").await? else { return Ok(()) };
            let Some(b) = pick_circle(c, "Select second object").await? else { return Ok(()) };
            Some((vec![a, b], vec![a.ent]))
        }
        CKind::Equal => {
            let Some((e1, p1)) = pick(c, "Select first object", |e| is_line(e) || is_circle(e)).await? else { return Ok(()) };
            let a = if is_circle(&e1) { circle_ref(&e1) } else { line_ref_near(&e1, p1) };
            let want_circle = is_circle(&e1);
            let Some((e2, p2)) = pick(c, "Select second object", move |e| if want_circle { is_circle(e) } else { is_line(e) }).await? else { return Ok(()) };
            let b = if is_circle(&e2) { circle_ref(&e2) } else { line_ref_near(&e2, p2) };
            a.zip(b).map(|(a, b)| (vec![a, b], vec![a.ent]))
        }
        CKind::Tangent => {
            let Some(a) = pick_curve(c, "Select first object").await? else { return Ok(()) };
            let Some(b) = pick_curve(c, "Select second object").await? else { return Ok(()) };
            Some((vec![a, b], vec![a.ent]))
        }
        CKind::Fix => {
            let r = c.entity(ps("Select point or").options(&["Object"]).filter(constrainable)).await?;
            match r {
                EntR::Entity { id, pt: p, .. } => c.entity_by_id(id).and_then(|e| point_ref_near(&e, p)).map(|r| (vec![r], vec![r.ent])),
                EntR::Kw(_) => pick(c, "Select object", constrainable).await?.map(|(e, _)| (vec![PRef::new(e.id, At::Whole, 0)], vec![e.id])),
                EntR::Enter => None,
            }
        }
        CKind::Symmetric => {
            let Some(a) = pick_point(c, "Select first point").await? else { return Ok(()) };
            let Some(b) = pick_point(c, "Select second point").await? else { return Ok(()) };
            let Some(l) = pick_line(c, "Select symmetry line").await? else { return Ok(()) };
            Some((vec![a, b, l], vec![a.ent, l.ent]))
        }
        _ => None,
    };
    let Some((refs, keep)) = made else { return Ok(()) };
    if refs.len() >= 2 && refs[0] == refs[1] {
        c.log_error("Select two different objects or points.");
        return Ok(());
    }
    let Some(con) = c.doc(|d| make(d, kind, refs)) else {
        c.log_error("Those objects cannot take this constraint.");
        return Ok(());
    };
    apply(c, con, &keep);
    Ok(())
}

const GEO: [(&str, CKind); 11] = [
    ("Horizontal", CKind::Horizontal),
    ("Vertical", CKind::Vertical),
    ("Perpendicular", CKind::Perpendicular),
    ("PArallel", CKind::Parallel),
    ("Tangent", CKind::Tangent),
    ("Coincident", CKind::Coincident),
    ("CONcentric", CKind::Concentric),
    ("COLlinear", CKind::Collinear),
    ("Symmetric", CKind::Symmetric),
    ("Equal", CKind::Equal),
    ("Fix", CKind::Fix),
];

async fn geomconstraint(c: Ctx, a: Args) -> CmdResult<()> {
    let kw = match a.str("type") {
        Some(t) => t.to_string(),
        None => {
            let opts: Vec<&str> = GEO.iter().map(|g| g.0).collect();
            let last = c.app(|app| app.sysvars.prefs.get("GEOMLASTTYPE").map(|v| v.text())).unwrap_or_else(|| "Horizontal".into());
            c.keyword(ps("Enter constraint type").options(&opts).default_str(&last)).await?.kw().unwrap_or("Horizontal").to_string()
        }
    };
    let Some((name, kind)) = GEO.iter().find(|g| g.0.eq_ignore_ascii_case(&kw)) else { return Ok(()) };
    c.app(|app| {
        app.sysvars.prefs.insert("GEOMLASTTYPE".into(), crate::sysvars::Value::Str(name.to_string()));
    });
    geometric(&c, *kind).await
}

// ---------------------------------------------------------------- dimensional constraints

/// Which distance a linear constraint measures, from where its dimension line is placed (as DIMLINEAR decides).
fn linear_kind(a: Pt, b: Pt, at: Pt) -> CKind {
    let (lo, hi) = (pt(a.x.min(b.x), a.y.min(b.y)), pt(a.x.max(b.x), a.y.max(b.y)));
    let outside_x = at.x < lo.x || at.x > hi.x;
    let outside_y = at.y < lo.y || at.y > hi.y;
    let wider = (b.x - a.x).abs() >= (b.y - a.y).abs();
    // beside the points: vertical; above or below them: horizontal; otherwise the longer way
    if (outside_x && !outside_y) || (outside_x == outside_y && !wider) {
        CKind::VDistance
    } else {
        CKind::HDistance
    }
}

/// The offset of a dimensional constraint's dimension, from where the user placed it.
fn offset_for(kind: CKind, a: Pt, b: Pt, at: Pt) -> f64 {
    match kind {
        CKind::HDistance => at.y - a.y.max(b.y),
        CKind::VDistance => at.x - a.x.max(b.x),
        _ => {
            let n = if dist(a, b) > 1e-12 { perp(unit(b - a)) } else { pt(0.0, 1.0) };
            dot(at - mid(a, b), n)
        }
    }
}

/// The two points a distance constraint measures: picked points, or the ends of a line (Enter / Object).
async fn two_points(c: &Ctx) -> CmdResult<Option<(Vec<PRef>, Pt, Pt)>> {
    let r = c.entity(ps("Specify first constraint point or").options(&["Object"]).default_str("Object").allow_enter().filter(constrainable)).await?;
    let refs = match r {
        EntR::Entity { id, pt: p, .. } => {
            let Some(a) = c.entity_by_id(id).and_then(|e| point_ref_near(&e, p)) else { return Ok(None) };
            let Some(b) = pick_point(c, "Specify second constraint point").await? else { return Ok(None) };
            if a == b {
                c.log_error("Select two different points.");
                return Ok(None);
            }
            vec![a, b]
        }
        _ => {
            let Some(l) = pick_line(c, "Select object").await? else { return Ok(None) };
            vec![l]
        }
    };
    let ends = c.doc(|d| {
        let probe = make(d, CKind::Distance, refs.clone())?;
        // the ends, through the display of a probe constraint
        let shown = display_of(d, &[probe], 1.0, true, false);
        shown.into_iter().find_map(|e| if let Kind::Dim(dd) = e.kind { dd.p1.zip(dd.p2) } else { None })
    });
    let Some((a, b)) = ends else { return Ok(None) };
    Ok(Some((refs, a, b)))
}

/// Ask for the constraint's expression (`50`, `d1*2`, or `width=100` to name it) and apply it.
async fn finish_dimensional(c: &Ctx, mut con: Constraint, keep: &[u64]) -> CmdResult<()> {
    let prompt = format!("Dimension text = {}", con.expr);
    let t = c.string(ps(&prompt).default_str(&format!("{}={}", con.name, con.expr))).await?.text;
    let t = t.trim();
    let (name, expr) = match t.split_once('=') {
        Some((n, e)) => (n.trim().to_string(), e.trim().to_string()),
        None if t.is_empty() => (con.name.clone(), con.expr.clone()),
        None => (con.name.clone(), t.to_string()),
    };
    if name.is_empty() || !name.chars().next().is_some_and(|ch| ch.is_alphabetic()) || !name.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
        c.log_error(&format!("\"{name}\" is not a valid parameter name."));
        return Ok(());
    }
    if !name.eq_ignore_ascii_case(&con.name) && c.doc(|d| d.model.constraints.has_name(&name)) {
        c.log_error(&format!("A parameter is already named {name}."));
        return Ok(());
    }
    con.name = name;
    con.expr = if expr.is_empty() { con.expr } else { expr };
    apply(c, con, keep);
    Ok(())
}

async fn dimensional(c: &Ctx, kind: Option<CKind>) -> CmdResult<()> {
    if !in_model(c) {
        return Ok(());
    }
    let px = c.view(|v| 1.0 / v.scale.max(1e-12));
    match kind {
        Some(CKind::Radius) | Some(CKind::Diameter) => {
            let kind = kind.unwrap();
            let Some(r) = pick_circle(c, "Select arc or circle").await? else { return Ok(()) };
            let Some(con) = c.doc(|d| make(d, kind, vec![r])) else { return Ok(()) };
            let (cc, rad) = c.doc(|d| match d.by_id(r.ent).map(|e| &e.kind) {
                Some(Kind::Circle { c, r }) | Some(Kind::Arc { c, r, .. }) => (*c, *r),
                _ => (Pt::ZERO, 0.0),
            });
            let shape = con.clone();
            let at = c.point(ps("Specify dimension line location").preview(move |d, p| display_of(d, &[Constraint { off: dist(p, cc) - rad, ..shape.clone() }], px, true, false).into_iter().map(pv).collect())).await?;
            let Some(at) = at.pt() else { return Ok(()) };
            finish_dimensional(c, Constraint { off: dist(at, cc) - rad, ..con }, &[]).await
        }
        Some(CKind::Angle) => {
            let Some(a) = pick_line(c, "Select first line").await? else { return Ok(()) };
            let Some(b) = pick_line(c, "Select second line").await? else { return Ok(()) };
            let Some(con) = c.doc(|d| make(d, CKind::Angle, vec![a, b])) else {
                c.log_error("The lines are parallel.");
                return Ok(());
            };
            let shape = con.clone();
            let at = c.point(ps("Specify dimension line location").preview(move |d, _| display_of(d, std::slice::from_ref(&shape), px, true, false).into_iter().map(pv).collect())).await?;
            if at.pt().is_none() {
                return Ok(());
            }
            finish_dimensional(c, con, &[a.ent]).await
        }
        fixed => {
            let Some((refs, a, b)) = two_points(c).await? else { return Ok(()) };
            let probe_refs = refs.clone();
            let at = c
                .point(ps("Specify dimension line location").preview(move |d, p| {
                    let k = fixed.unwrap_or_else(|| linear_kind(a, b, p));
                    match make(d, k, probe_refs.clone()) {
                        Some(con) => display_of(d, &[Constraint { off: offset_for(k, a, b, p), ..con }], px, true, false).into_iter().map(pv).collect(),
                        None => vec![],
                    }
                }))
                .await?;
            let Some(at) = at.pt() else { return Ok(()) };
            let k = fixed.unwrap_or_else(|| linear_kind(a, b, at));
            let Some(con) = c.doc(|d| make(d, k, refs.clone())) else { return Ok(()) };
            let keep = if refs.len() == 2 && refs[0].ent != refs[1].ent { vec![refs[0].ent] } else { vec![] };
            finish_dimensional(c, Constraint { off: offset_for(k, a, b, at), ..con }, &keep).await
        }
    }
}

const DIMS: [(&str, Option<CKind>); 7] = [
    ("LInear", None),
    ("Horizontal", Some(CKind::HDistance)),
    ("Vertical", Some(CKind::VDistance)),
    ("Aligned", Some(CKind::Distance)),
    ("ANgular", Some(CKind::Angle)),
    ("Radial", Some(CKind::Radius)),
    ("Diameter", Some(CKind::Diameter)),
];

async fn dimconstraint(c: Ctx, _a: Args) -> CmdResult<()> {
    let opts: Vec<&str> = DIMS.iter().map(|g| g.0).collect();
    let kw = c.keyword(ps("Enter constraint type").options(&opts).default_str("Aligned")).await?.kw().unwrap_or("Aligned").to_string();
    let Some((_, kind)) = DIMS.iter().find(|g| g.0.eq_ignore_ascii_case(&kw)) else { return Ok(()) };
    dimensional(&c, *kind).await
}

// ---------------------------------------------------------------- AUTOCONSTRAIN / DELCONSTRAINT

async fn autoconstrain(c: Ctx, _a: Args) -> CmdResult<()> {
    if !in_model(&c) {
        return Ok(());
    }
    let sel = c.select(ps("Select objects").filter(constrainable)).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let found = c.doc(|d| infer(d, &sel.ents, 0.05));
    let n = c.doc_mut(|d| {
        d.change(|d| {
            let mut n = 0;
            for (k, refs) in found {
                let Some(con) = make(d, k, refs) else { continue };
                d.model.constraints.list.push(con);
                // one that cannot hold with the others is dropped
                if solve(d, &HashSet::new()).ok {
                    n += 1;
                } else {
                    d.model.constraints.list.pop();
                }
            }
            n
        })
    });
    let dof = c.doc(|d| degrees_of_freedom(d).0);
    c.log(&format!("{n} constraint{} applied to {} object{}. {}", if n == 1 { "" } else { "s" }, sel.ents.len(), if sel.ents.len() == 1 { "" } else { "s" }, dof_text(dof)));
    c.clear_sel();
    Ok(())
}

async fn delconstraint(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects").filter(constrainable)).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let ids: HashSet<u64> = sel.ents.iter().copied().collect();
    let n = c.doc_mut(|d| {
        d.change(|d| {
            let before = d.model.constraints.list.len();
            d.model.constraints.list.retain(|k| !k.refs.iter().any(|r| ids.contains(&r.ent)));
            before - d.model.constraints.list.len()
        })
    });
    c.log(&format!("{n} constraint{} removed.", if n == 1 { "" } else { "s" }));
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- PARAMETERS

fn list_params(c: &Ctx) {
    let (rows, dof) = c.doc(|d| {
        let cs = &d.model.constraints;
        let mut rows: Vec<String> = cs.list.iter().filter(|k| k.kind.dimensional()).map(|k| format!("  {} = {}{}  ({})", k.name, k.expr, if fmt_value(k.value) != k.expr { format!(" = {}", fmt_value(k.value)) } else { String::new() }, k.kind.label())).collect();
        rows.extend(cs.params.iter().map(|p| format!("  {} = {}{}  (user parameter)", p.name, p.expr, if fmt_value(p.value) != p.expr { format!(" = {}", fmt_value(p.value)) } else { String::new() })));
        let geo: HashMap<&str, usize> = cs.list.iter().filter(|k| !k.kind.dimensional()).fold(HashMap::new(), |mut m, k| {
            *m.entry(k.kind.label()).or_default() += 1;
            m
        });
        if !geo.is_empty() {
            let mut g: Vec<String> = geo.iter().map(|(k, n)| format!("{k} {n}")).collect();
            g.sort();
            rows.push(format!("  Geometric constraints: {}", g.join(", ")));
        }
        (rows, degrees_of_freedom(d))
    });
    if rows.is_empty() {
        c.log("The drawing has no constraints or parameters.");
    } else {
        c.log(&format!("Parameters:\n{}\n{}", rows.join("\n"), dof_text(dof.0)));
    }
}

/// Replace the word `old` by `new` in an expression (whole names only).
fn rename_in(expr: &str, old: &str, new: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut String| {
        if w.eq_ignore_ascii_case(old) {
            out.push_str(new);
        } else {
            out.push_str(w);
        }
        w.clear();
    };
    for ch in expr.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            flush(&mut word, &mut out);
            out.push(ch);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Change the drawing's parameters with `f`, then work them out and solve; undone when that fails.
fn change_params(c: &Ctx, f: impl FnOnce(&mut Constraints) -> Result<(), String>) -> bool {
    let r = c.doc_mut(|d| {
        let before = d.model.constraints.clone();
        d.change(|d| {
            f(&mut d.model.constraints)?;
            let r = solve(d, &HashSet::new());
            if !r.ok {
                d.model.constraints = before;
                return Err(r.message);
            }
            Ok(r.dof)
        })
    });
    match r {
        Ok(dof) => {
            c.log(&dof_text(dof));
            true
        }
        Err(e) => {
            c.log_error(&e);
            false
        }
    }
}

async fn parameters(c: Ctx, a: Args) -> CmdResult<()> {
    loop {
        let kw = match a.str("option") {
            Some(o) => o.to_string(),
            None => c.keyword(ps("Enter an option").options(&["Edit", "New", "Rename", "Delete", "?"]).default_str("?").allow_enter()).await?.kw().unwrap_or("").to_string(),
        };
        match kw.as_str() {
            "?" => list_params(&c),
            "Edit" => {
                let name = c.string(ps("Enter parameter name")).await?.text.trim().to_string();
                let cur = c.doc(|d| {
                    let cs = &d.model.constraints;
                    cs.list.iter().find(|k| k.name.eq_ignore_ascii_case(&name)).map(|k| k.expr.clone()).or_else(|| cs.params.iter().find(|p| p.name.eq_ignore_ascii_case(&name)).map(|p| p.expr.clone()))
                });
                let Some(cur) = cur else {
                    c.log_error(&format!("No parameter is named {name}."));
                    continue;
                };
                let e = c.string(ps(&format!("Enter expression for {name}")).default_str(&cur)).await?.text;
                let e = if e.trim().is_empty() { cur } else { e.trim().to_string() };
                change_params(&c, |cs| {
                    if let Some(k) = cs.list.iter_mut().find(|k| k.name.eq_ignore_ascii_case(&name)) {
                        k.expr = e;
                    } else if let Some(p) = cs.params.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&name)) {
                        p.expr = e;
                    }
                    Ok(())
                });
            }
            "New" => {
                let t = c.string(ps("Enter user parameter as name=expression")).await?.text;
                let Some((n, e)) = t.split_once('=') else {
                    c.log_error("Type the parameter as name=expression, for example width=100.");
                    continue;
                };
                let (n, e) = (n.trim().to_string(), e.trim().to_string());
                if n.is_empty() || !n.chars().next().is_some_and(|ch| ch.is_alphabetic()) || !n.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
                    c.log_error(&format!("\"{n}\" is not a valid parameter name."));
                    continue;
                }
                change_params(&c, |cs| {
                    if cs.has_name(&n) {
                        return Err(format!("A parameter is already named {n}."));
                    }
                    cs.params.push(UserParam { name: n, expr: e, value: 0.0 });
                    Ok(())
                });
            }
            "Rename" => {
                let old = c.string(ps("Enter parameter name")).await?.text.trim().to_string();
                let new = c.string(ps("Enter new name")).await?.text.trim().to_string();
                change_params(&c, |cs| {
                    if !cs.has_name(&old) {
                        return Err(format!("No parameter is named {old}."));
                    }
                    if new.is_empty() || !new.chars().all(|ch| ch.is_alphanumeric() || ch == '_') || cs.has_name(&new) {
                        return Err(format!("\"{new}\" cannot be used as a name."));
                    }
                    for k in cs.list.iter_mut() {
                        if k.name.eq_ignore_ascii_case(&old) {
                            k.name = new.clone();
                        }
                        k.expr = rename_in(&k.expr, &old, &new);
                    }
                    for p in cs.params.iter_mut() {
                        if p.name.eq_ignore_ascii_case(&old) {
                            p.name = new.clone();
                        }
                        p.expr = rename_in(&p.expr, &old, &new);
                    }
                    Ok(())
                });
            }
            "Delete" => {
                let name = c.string(ps("Enter parameter name")).await?.text.trim().to_string();
                change_params(&c, |cs| {
                    let value = cs.list.iter().find(|k| k.name.eq_ignore_ascii_case(&name)).map(|k| k.value).or_else(|| cs.params.iter().find(|p| p.name.eq_ignore_ascii_case(&name)).map(|p| p.value));
                    let Some(v) = value else { return Err(format!("No parameter is named {name}.")) };
                    cs.list.retain(|k| !k.name.eq_ignore_ascii_case(&name));
                    cs.params.retain(|p| !p.name.eq_ignore_ascii_case(&name));
                    // expressions that used it keep its value
                    let v = fmt_value(v);
                    for k in cs.list.iter_mut() {
                        k.expr = rename_in(&k.expr, &name, &v);
                    }
                    for p in cs.params.iter_mut() {
                        p.expr = rename_in(&p.expr, &name, &v);
                    }
                    Ok(())
                });
            }
            _ => return Ok(()),
        }
        if a.str("option").is_some() {
            return Ok(());
        }
    }
}

// ---------------------------------------------------------------- display switches

async fn constraintbar(c: Ctx, _a: Args) -> CmdResult<()> {
    let k = c.keyword(ps("Enter an option").options(&["Showall", "Hideall"]).default_str("Showall")).await?;
    let hide = k.kw() == Some("Hideall");
    c.doc_mut(|d| d.change(|d| d.model.constraints.hide_bars = hide));
    Ok(())
}

async fn dcdisplay(c: Ctx, _a: Args) -> CmdResult<()> {
    let k = c.keyword(ps("Enter an option").options(&["Show", "Hide"]).default_str("Show")).await?;
    let hide = k.kw() == Some("Hide");
    c.doc_mut(|d| d.change(|d| d.model.constraints.hide_dims = hide));
    Ok(())
}

pub fn register(r: &mut Registry) {
    let g = |name: &str| def("Parametric", name, run(geomconstraint));
    r.define(g("GEOMCONSTRAINT").aliases(&["GCON"]).title("Geometric Constraint").icon("dimstyle").desc("Applies a geometric constraint: horizontal, vertical, parallel, perpendicular, tangent, coincident, concentric, collinear, symmetric, equal or fix."));
    for (name, kind, title) in [
        ("GCHORIZONTAL", CKind::Horizontal, "Horizontal"),
        ("GCVERTICAL", CKind::Vertical, "Vertical"),
        ("GCPARALLEL", CKind::Parallel, "Parallel"),
        ("GCPERPENDICULAR", CKind::Perpendicular, "Perpendicular"),
        ("GCCOINCIDENT", CKind::Coincident, "Coincident"),
        ("GCCOLLINEAR", CKind::Collinear, "Collinear"),
        ("GCCONCENTRIC", CKind::Concentric, "Concentric"),
        ("GCEQUAL", CKind::Equal, "Equal"),
        ("GCTANGENT", CKind::Tangent, "Tangent"),
        ("GCFIX", CKind::Fix, "Fix"),
        ("GCSYMMETRIC", CKind::Symmetric, "Symmetric"),
    ] {
        r.define(def("Parametric", name, run(move |c, _a| async move { geometric(&c, kind).await })).title(title).icon("dimstyle").desc(&format!("Applies a {} constraint.", title.to_lowercase())));
    }
    r.define(def("Parametric", "DIMCONSTRAINT", run(dimconstraint)).aliases(&["DCON"]).title("Dimensional Constraint").icon("dimlinear").desc("Applies a dimensional constraint (a distance, angle, radius or diameter with a name and an expression)."));
    for (name, kind, title, icon) in [
        ("DCLINEAR", None, "Linear", "dimlinear"),
        ("DCHORIZONTAL", Some(CKind::HDistance), "Horizontal", "dimlinear"),
        ("DCVERTICAL", Some(CKind::VDistance), "Vertical", "dimlinear"),
        ("DCALIGNED", Some(CKind::Distance), "Aligned", "dimaligned"),
        ("DCANGULAR", Some(CKind::Angle), "Angular", "dimangular"),
        ("DCRADIUS", Some(CKind::Radius), "Radius", "dimradius"),
        ("DCDIAMETER", Some(CKind::Diameter), "Diameter", "dimdiameter"),
    ] {
        r.define(def("Parametric", name, run(move |c, _a| async move { dimensional(&c, kind).await })).title(title).icon(icon).desc(&format!("Applies a {} dimensional constraint.", title.to_lowercase())));
    }
    r.define(def("Parametric", "AUTOCONSTRAIN", run(autoconstrain)).title("Auto Constrain").icon("dimstyle").desc("Applies the geometric constraints the selected objects already meet (coincident, horizontal, vertical, parallel, perpendicular, collinear, concentric, tangent)."));
    r.define(def("Parametric", "DELCONSTRAINT", run(delconstraint)).title("Delete Constraints").icon("erase").desc("Removes every constraint from the selected objects."));
    r.define(def("Parametric", "PARAMETERS", run(parameters)).aliases(&["PAR", "-PARAMETERS"]).title("Parameters Manager").icon("list").desc("Lists, edits, renames and deletes dimensional constraint parameters and user parameters."));
    r.define(def("Parametric", "CONSTRAINTBAR", run(constraintbar)).aliases(&["CBAR"]).title("Constraint Bars").icon("dimstyle").desc("Shows or hides the geometric constraint symbols."));
    r.define(def("Parametric", "DCDISPLAY", run(dcdisplay)).title("Show Dynamic Constraints").icon("dimstyle").desc("Shows or hides the dimensional constraints."));
}
