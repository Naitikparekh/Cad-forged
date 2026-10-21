//! Property and placement commands: CHPROP, CHANGE, ALIGN, OVERKILL, FLATTEN.
use super::{dm, ps};
use crate::commands::helpers::*;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use crate::sysvars::parse_color;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::{ltype_key, Color, LINEWEIGHTS, LW_BYBLOCK, LW_DEFAULT_VALUE};

// ---------------------------------------------------------------- CHPROP / CHANGE Properties

/// One property edit applied to every selected object.
enum Prop {
    Color(Option<Color>),
    Layer(String),
    LType(Option<String>),
    LtScale(f64),
    LWeight(Option<i32>),
    Transp(Option<f64>),
}

fn apply_prop(e: &mut Entity, p: &Prop) {
    match p {
        Prop::Color(v) => e.color = *v,
        Prop::Layer(n) => e.layer = n.clone(),
        Prop::LType(v) => e.ltype = v.clone(),
        Prop::LtScale(v) => e.lts = Some(*v),
        Prop::LWeight(v) => e.lw = *v,
        Prop::Transp(v) => e.transp = *v,
    }
}

fn parse_ltype(t: &str) -> Result<Option<String>, String> {
    match t.trim().to_uppercase().as_str() {
        "BYLAYER" => Ok(None),
        "BYBLOCK" => Ok(Some("BYBLOCK".into())),
        _ => ltype_key(t.trim()).map(|k| Some(k.to_string())).ok_or_else(|| format!("Linetype \"{}\" not found.", t.trim())),
    }
}

fn parse_lweight(t: &str) -> Result<Option<i32>, String> {
    match t.trim().to_uppercase().as_str() {
        "BYLAYER" => Ok(None),
        "BYBLOCK" => Ok(Some(LW_BYBLOCK)),
        "DEFAULT" => Ok(Some(LW_DEFAULT_VALUE)),
        s => {
            let hw = (s.parse::<f64>().map_err(|_| "Invalid lineweight.".to_string())? * 100.0).round() as i32;
            LINEWEIGHTS.contains(&hw).then_some(Some(hw)).ok_or_else(|| "Invalid lineweight.".to_string())
        }
    }
}

fn parse_transp(t: &str) -> Result<Option<f64>, String> {
    match t.trim().to_uppercase().as_str() {
        "BYLAYER" => Ok(None),
        s => s.parse::<f64>().ok().filter(|v| (0.0..=90.0).contains(v)).map(Some).ok_or_else(|| "Transparency must be between 0 and 90.".to_string()),
    }
}

/// Ask for properties until Enter and apply them to the selection.
async fn change_properties(c: &Ctx, sel: &Selection) -> CmdResult<()> {
    loop {
        let opts = ["Color", "LAyer", "LType", "ltScale", "LWeight", "Thickness", "TRansparency"];
        let r = c.keyword(ps("Enter property to change").options(&opts)).await?;
        let Some(kw) = r.kw() else { return Ok(()) };
        let prop: Result<Prop, String> = match kw {
            "Color" => c.string(ps("Enter new color [Truecolor/COlorbook]").default_str("BYLAYER")).await.map(|t| parse_color(&t.text).map(Prop::Color))?,
            "LAyer" => {
                let cur = c.doc(|d| d.cur.layer.clone());
                let t = c.string(ps("Enter new layer name").default_str(&cur)).await?.text;
                c.doc(|d| d.layer(t.trim()).map(|l| Prop::Layer(l.name.clone()))).ok_or(format!("Layer \"{}\" not found.", t.trim()))
            }
            "LType" => c.string(ps("Enter new linetype name").default_str("BYLAYER")).await.map(|t| parse_ltype(&t.text).map(Prop::LType))?,
            "ltScale" => match c.number(ps("Specify new object linetype scale").default_num(1.0).min(0.0).nonzero()).await?.value() {
                Some(v) => Ok(Prop::LtScale(v)),
                None => continue,
            },
            "LWeight" => c.string(ps("Enter new lineweight").default_str("BYLAYER")).await.map(|t| parse_lweight(&t.text).map(Prop::LWeight))?,
            "TRansparency" => c.string(ps("Enter new transparency value").default_str("BYLAYER")).await.map(|t| parse_transp(&t.text).map(Prop::Transp))?,
            _ => {
                c.number(ps("Specify new thickness").default_num(0.0)).await?;
                c.log_error("Thickness is not stored on 2D objects; nothing was changed.");
                continue;
            }
        };
        match prop {
            Err(m) => c.log_error(&m),
            Ok(p) => c.doc_mut(|d| {
                d.change(|d| {
                    for id in &sel.ents {
                        if let Some(e) = d.by_id_mut(*id) {
                            apply_prop(e, &p);
                        }
                    }
                    // solids carry only a layer and a colour
                    for id in &sel.solids {
                        if let Some(s) = d.solid_by_id_mut(*id) {
                            match &p {
                                Prop::Layer(n) => s.layer = n.clone(),
                                Prop::Color(v) => s.color = *v,
                                _ => {}
                            }
                        }
                    }
                })
            }),
        }
    }
}

async fn chprop(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects").solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    change_properties(&c, &sel).await?;
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- CHANGE

/// Change a text object: the change point is its new insertion point.
async fn change_text(c: &Ctx, id: u64, at: Option<Pt>) -> CmdResult<()> {
    let Some(Entity { kind: Kind::Text(t), .. }) = c.entity_by_id(id) else { return Ok(()) };
    c.ed(|e| e.temp_sel = [id].into_iter().collect());
    let p = match at {
        Some(p) => p,
        None => c.point(ps("Enter new text insertion point <no change>").allow_enter().default_pt(t.p)).await?.pt().unwrap_or(t.p),
    };
    let Some(h) = c.distance(ps("Specify new height").default_num(t.h).nonzero().min(0.0)).await?.value() else { return Ok(()) };
    let Some(rot) = c.angle(ps("Specify new rotation angle").default_num(t.rot)).await?.value() else { return Ok(()) };
    let s = c.string(ps("Enter new text").default_str(&t.s).raw()).await?.text;
    c.doc_mut(|d| {
        d.change(|d| {
            if let Some(Entity { kind: Kind::Text(t), .. }) = d.by_id_mut(id) {
                (t.p, t.h, t.rot, t.s) = (p, h, rot, s);
            }
        })
    });
    Ok(())
}

async fn change_insert(c: &Ctx, id: u64, at: Option<Pt>) -> CmdResult<()> {
    let Some(Entity { kind: Kind::Insert(i), .. }) = c.entity_by_id(id) else { return Ok(()) };
    c.ed(|e| e.temp_sel = [id].into_iter().collect());
    let p = match at {
        Some(p) => p,
        None => c.point(ps("Enter new block insertion point <no change>").allow_enter().default_pt(i.p)).await?.pt().unwrap_or(i.p),
    };
    let Some(rot) = c.angle(ps("Enter new block rotation angle").default_num(i.rot)).await?.value() else { return Ok(()) };
    c.doc_mut(|d| {
        d.change(|d| {
            if let Some(Entity { kind: Kind::Insert(i), .. }) = d.by_id_mut(id) {
                // attribute values are positioned in world coordinates, so they follow the block
                let m = mat::mul(&mat::translate(p.x - i.p.x, p.y - i.p.y), &mat::rotate(rot - i.rot, i.p));
                for a in &mut i.attribs {
                    a.t.p = mat::apply(&m, a.t.p);
                    a.t.rot += rot - i.rot;
                }
                (i.p, i.rot) = (p, rot);
            }
        })
    });
    Ok(())
}

async fn change(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let r = c.point(ps("Specify change point or").options(&["Properties"])).await?;
    if r.kw() == Some("Properties") {
        change_properties(&c, &sel).await?;
        c.clear_sel();
        return Ok(());
    }
    let Some(at) = r.pt() else { return Ok(()) };
    let (mut first_circle, mut first_text, mut first_block) = (true, true, true);
    for e in c.entities(&sel.ents) {
        let id = e.id;
        match &e.kind {
            Kind::Line { a, b } => {
                // the end nearest the change point moves to it
                let (na, nb) = if dist(*a, at) <= dist(*b, at) { (at, *b) } else { (*a, at) };
                c.doc_mut(|d| d.modify(id, |x| x.kind = Kind::Line { a: na, b: nb }));
            }
            Kind::Circle { c: ctr, .. } => {
                let r = if std::mem::take(&mut first_circle) {
                    dist(*ctr, at)
                } else {
                    c.ed(|e| e.temp_sel = [id].into_iter().collect());
                    match c.distance(ps("Specify new circle radius <no change>").allow_enter().base(*ctr)).await?.value() {
                        Some(v) => v,
                        None => continue,
                    }
                };
                if r > 0.0 {
                    let ctr = *ctr;
                    c.doc_mut(|d| d.modify(id, |x| x.kind = Kind::Circle { c: ctr, r }));
                }
            }
            Kind::Text(_) => change_text(&c, id, std::mem::take(&mut first_text).then_some(at)).await?,
            Kind::Insert(_) => change_insert(&c, id, std::mem::take(&mut first_block).then_some(at)).await?,
            _ => c.log_error(&format!("CHANGE does not move the change point of a {}; use Properties.", e.label().to_lowercase())),
        }
    }
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- ALIGN

/// Move so `s1` lands on `d1`; with a second pair also rotate `s1s2` onto `d1d2` (and scale to fit when asked).
fn align_matrix(s1: Pt, d1: Pt, pair: Option<(Pt, Pt)>, scale: bool) -> Mat {
    let shift = mat::translate(d1.x - s1.x, d1.y - s1.y);
    let Some((s2, d2)) = pair else { return shift };
    if same(s1, s2) || same(d1, d2) {
        return shift;
    }
    let turn = mat::rotate(angle_of(d1, d2) - angle_of(s1, s2), s1);
    let k = if scale { dist(d1, d2) / dist(s1, s2) } else { 1.0 };
    mat::mul(&shift, &mat::mul(&turn, &mat::scale(k, k, s1)))
}

async fn align(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects").solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let ents = c.entities(&sel.ents);
    let Some(s1) = c.point(ps("Specify first source point")).await?.pt() else { return Ok(()) };
    let e1 = ents.clone();
    let Some(d1) = c.point(ps("Specify first destination point").base(s1).preview(move |_, p| pvs(transformed_clones(&e1, &align_matrix(s1, p, None, false))))).await?.pt() else { return Ok(()) };
    let Some(s2) = c.point(ps("Specify second source point").base(s1).allow_enter()).await?.pt() else {
        c.doc_mut(|d| apply_transform(d, &sel.ents, &sel.solids, &align_matrix(s1, d1, None, false)));
        c.clear_sel();
        return Ok(());
    };
    let e2 = ents.clone();
    let Some(d2) = c.point(ps("Specify second destination point").base(d1).preview(move |_, p| pvs(transformed_clones(&e2, &align_matrix(s1, d1, Some((s2, p)), false))))).await?.pt() else { return Ok(()) };
    if c.point(ps("Specify third source point or <continue>").allow_enter()).await?.pt().is_some() {
        c.log_error("Three-point (3D) alignment is not supported for 2D drawings; aligning with two point pairs.");
    }
    let scale = c.keyword(ps("Scale objects based on alignment points?").options(&["Yes", "No"]).default_str("No")).await?.kw() == Some("Yes");
    if scale && (same(s1, s2) || same(d1, d2)) {
        c.log_error("The alignment points must be distinct to scale.");
        return Ok(());
    }
    c.doc_mut(|d| apply_transform(d, &sel.ents, &sel.solids, &align_matrix(s1, d1, Some((s2, d2)), scale)));
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- OVERKILL

struct Kill {
    tol: f64,
    /// Properties that need not match: layer, color, linetype, lineweight.
    ignore: [bool; 4],
    overlap: bool,
    end_to_end: bool,
}

fn same_props(a: &Entity, b: &Entity, k: &Kill) -> bool {
    let col = |e: &Entity| e.color.filter(|c| *c != Color::ByLayer);
    let lt = |e: &Entity| e.ltype.as_deref().map(str::to_uppercase).filter(|l| l != "BYLAYER");
    let lw = |e: &Entity| e.lw.filter(|w| *w != -1);
    (k.ignore[0] || a.layer.eq_ignore_ascii_case(&b.layer))
        && (k.ignore[1] || col(a) == col(b))
        && (k.ignore[2] || lt(a) == lt(b))
        && (k.ignore[3] || lw(a) == lw(b))
        && a.lts.unwrap_or(1.0) == b.lts.unwrap_or(1.0)
        && a.transp.unwrap_or(0.0) == b.transp.unwrap_or(0.0)
}

fn seg_same(a: &Seg, b: &Seg, tol: f64) -> bool {
    match (a, b) {
        (Seg::L { .. }, Seg::L { .. }) => dist(a.start(), b.start()) <= tol && dist(a.end(), b.end()) <= tol,
        (Seg::A { c: c1, r: r1, sw: s1, .. }, Seg::A { c: c2, r: r2, sw: s2, .. }) => {
            dist(*c1, *c2) <= tol && (r1 - r2).abs() <= tol && dist(a.start(), b.start()) <= tol && dist(a.end(), b.end()) <= tol && s1.signum() == s2.signum()
        }
        _ => false,
    }
}

/// Do two entities have the same geometry? Lines and arcs match in either direction; closed polylines from any start.
fn same_geometry(a: &Entity, b: &Entity, doc: &Drawing, tol: f64) -> bool {
    if a.type_name() != b.type_name() || !matches!(a.kind, Kind::Line { .. } | Kind::Arc { .. } | Kind::Circle { .. } | Kind::Pline { .. }) {
        return false;
    }
    let (sa, sb) = (segs_of(a, doc), segs_of(b, doc));
    if sa.len() != sb.len() || sa.is_empty() {
        return false;
    }
    let n = sa.len();
    let rev: Vec<Seg> = sb.iter().rev().map(|s| s.reverse()).collect();
    let closed = matches!(a.kind, Kind::Pline { closed: true, .. } | Kind::Circle { .. });
    let shifts = if closed { n } else { 1 };
    let fits = |other: &[Seg]| (0..shifts).any(|k| (0..n).all(|i| seg_same(&sa[i], &other[(i + k) % n], tol)));
    match a.kind {
        Kind::Circle { c, r } => matches!(b.kind, Kind::Circle { c: c2, r: r2 } if dist(c, c2) <= tol && (r - r2).abs() <= tol),
        _ => fits(&sb) || fits(&rev),
    }
}

/// Union of two collinear lines when they overlap (or touch end to end), else `None`.
fn merge_lines(p: (Pt, Pt), q: (Pt, Pt), k: &Kill) -> Option<(Pt, Pt)> {
    let len_p = dist(p.0, p.1);
    if len_p <= k.tol {
        return None;
    }
    let u = unit(p.1 - p.0);
    if cross(u, q.0 - p.0).abs() > k.tol || cross(u, q.1 - p.0).abs() > k.tol {
        return None;
    }
    let (t0, t1) = (dot(u, q.0 - p.0), dot(u, q.1 - p.0));
    let (lo, hi) = (t0.min(t1), t0.max(t1));
    let overlap = len_p.min(hi) - 0f64.max(lo);
    let ok = if overlap > k.tol { k.overlap } else { overlap >= -k.tol && k.end_to_end };
    ok.then(|| (p.0 + u * lo.min(0.0), p.0 + u * hi.max(len_p)))
}

/// Union of two arcs of the same circle (start angle, ccw sweep) when they overlap or touch.
fn merge_arcs(a: (Pt, f64, f64, f64), b: (Pt, f64, f64, f64), k: &Kill) -> Option<(f64, f64)> {
    let (ca, ra, a0, sa) = a;
    let (cb, rb, b0, sb) = b;
    if dist(ca, cb) > k.tol || (ra - rb).abs() > k.tol {
        return None;
    }
    let atol = k.tol / ra.max(1e-12);
    let (d, e) = (sweep_ccw(a0, b0), sweep_ccw(b0, a0));
    let overlap = if d <= sa + atol {
        sb.min(sa - d)
    } else if e <= sb + atol {
        sa.min(sb - e)
    } else {
        return None;
    };
    let ok = if overlap > atol { k.overlap } else { k.end_to_end };
    if !ok {
        return None;
    }
    if d <= sa + atol {
        Some((a0, sa.max(d + sb)))
    } else {
        Some((b0, sb.max(e + sa)))
    }
}

async fn overkill(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let mut k = Kill { tol: 1e-6, ignore: [false; 4], overlap: true, end_to_end: true };
    loop {
        let opts = ["Tolerance", "Ignore properties", "Overlap", "End to end", "Proceed"];
        let r = c.keyword(ps("Enter an option").options(&opts).default_str("Proceed")).await?;
        match r.kw() {
            Some("Tolerance") => {
                if let Some(v) = c.number(ps("Specify tolerance").default_num(k.tol).min(0.0)).await?.value() {
                    k.tol = v.max(1e-12);
                }
            }
            Some("Ignore properties") => loop {
                let names = ["Layer", "Color", "LType", "LWeight"];
                let r = c.keyword(ps("Enter property to toggle ignoring").options(&["Layer", "Color", "LType", "LWeight", "Done"]).default_str("Done")).await?;
                match names.iter().position(|n| Some(*n) == r.kw()) {
                    Some(i) => {
                        k.ignore[i] = !k.ignore[i];
                        c.log(&format!("{}: {}", names[i], if k.ignore[i] { "ignored" } else { "must match" }));
                    }
                    None => break,
                }
            },
            Some("Overlap") => {
                k.overlap = !k.overlap;
                c.log(&format!("Combine collinear objects that partially overlap: {}", if k.overlap { "Yes" } else { "No" }));
            }
            Some("End to end") => {
                k.end_to_end = !k.end_to_end;
                c.log(&format!("Combine collinear objects when aligned end to end: {}", if k.end_to_end { "Yes" } else { "No" }));
            }
            _ => break,
        }
    }
    let (mut dups, mut merged) = (0, 0);
    let mut gone: Vec<u64> = vec![];
    let mut items: Vec<Entity> = c.entities(&sel.ents);
    // duplicates first: equal geometry and matching properties, the earlier object stays
    c.doc(|d| {
        for j in 1..items.len() {
            if (0..j).any(|i| !gone.contains(&items[i].id) && same_props(&items[i], &items[j], &k) && same_geometry(&items[i], &items[j], d, k.tol)) {
                gone.push(items[j].id);
                dups += 1;
            }
        }
    });
    items.retain(|e| !gone.contains(&e.id));
    // then collinear lines and co-circular arcs that overlap or touch
    let before = items.clone();
    let mut progress = true;
    while progress {
        progress = false;
        'scan: for i in 0..items.len() {
            for j in i + 1..items.len() {
                if !same_props(&items[i], &items[j], &k) {
                    continue;
                }
                let union = match (&items[i].kind, &items[j].kind) {
                    (Kind::Line { a, b }, Kind::Line { a: a2, b: b2 }) => merge_lines((*a, *b), (*a2, *b2), &k).map(|(a, b)| Kind::Line { a, b }),
                    (Kind::Arc { c: c1, r: r1, a0, a1 }, Kind::Arc { c: c2, r: r2, a0: b0, a1: b1 }) => {
                        let sw = |s: f64, e: f64| if sweep_ccw(s, e) < 1e-12 { TAU } else { sweep_ccw(s, e) };
                        merge_arcs((*c1, *r1, *a0, sw(*a0, *a1)), (*c2, *r2, *b0, sw(*b0, *b1)), &k).map(|(s, w)| if w >= TAU - 1e-9 { Kind::Circle { c: *c1, r: *r1 } } else { Kind::Arc { c: *c1, r: *r1, a0: norm_angle(s), a1: norm_angle(s + w) } })
                    }
                    _ => None,
                };
                if let Some(kind) = union {
                    items[i].kind = kind;
                    gone.push(items[j].id);
                    items.remove(j);
                    merged += 1;
                    progress = true;
                    break 'scan;
                }
            }
        }
    }
    let edits: Vec<(u64, Kind)> = items.iter().filter(|e| before.iter().any(|b| b.id == e.id && b.kind != e.kind)).map(|e| (e.id, e.kind.clone())).collect();
    c.doc_mut(|d| {
        d.change(|d| {
            d.remove_all(&gone);
            for (id, kind) in edits {
                d.modify(id, |e| e.kind = kind);
            }
        })
    });
    c.ed(|e| e.prune());
    if dups + merged == 0 {
        c.log("No duplicate or overlapping objects found.");
    } else {
        c.log(&format!("{dups} duplicate object(s) deleted; {merged} overlapping object(s) combined or deleted."));
    }
    Ok(())
}

// ---------------------------------------------------------------- FLATTEN

/// Drawing data is two-dimensional, so there is nothing to flatten; solids are the only 3D data and are left alone.
async fn flatten(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects").solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    if !sel.ents.is_empty() {
        c.log(&format!("{} object(s) already lie in the XY plane (drawing geometry is 2D); nothing to flatten.", sel.ents.len()));
    }
    if !sel.solids.is_empty() {
        c.log_error(&format!("{} 3D solid(s) cannot be flattened and were left unchanged.", sel.solids.len()));
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(dm("CHPROP", run(chprop)).title("Change Properties").icon("properties").desc("Changes the properties of objects: color, layer, linetype, linetype scale, lineweight, thickness and transparency."));
    r.define(dm("CHANGE", run(change)).title("Change").icon("properties").desc("Changes the properties of existing objects: moves line ends, resizes circles, edits text and block insertions, or edits properties."));
    r.define(dm("ALIGN", run(align)).aliases(&["AL"]).title("Align").icon("move").desc("Aligns objects with other objects in 2D by moving, rotating (and optionally scaling) them between pairs of points."));
    r.define(dm("OVERKILL", run(overkill)).title("Overkill").icon("erase").desc("Deletes duplicate objects and combines overlapping or end-to-end collinear lines and arcs of the same circle. Polylines and circles are only checked for duplicates."));
    r.define(dm("FLATTEN", run(flatten)).title("Flatten").desc("Sets the Z coordinate of objects to zero. Drawing geometry is already 2D, so nothing changes."));
}
