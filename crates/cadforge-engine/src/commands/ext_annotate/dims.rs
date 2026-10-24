//! Dimension commands: DIMARC, DIMJOGGED, DIMORDINATE, QDIM, DIMBREAK, DIMCENTER, DIMSPACE, DIMEDIT, DIMTEDIT and
//! DIMINSPECT.
use super::{group_id, ps};
use crate::commands::helpers::{pv, pvs};
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::dim::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::text::measure_text;

fn dim_entity(kind: DimType, ds: &str, f: impl FnOnce(&mut DimData)) -> Entity {
    let mut d = DimData::new(kind);
    d.ds = Some(ds.to_string());
    f(&mut d);
    Entity::dim(d)
}

fn cur_ds(c: &Ctx) -> String {
    c.doc(|d| d.settings.cur_dim_style.clone())
}

fn is_dim(e: &Entity) -> bool {
    matches!(e.kind, Kind::Dim(_))
}

fn is_arc_or_circle(e: &Entity) -> bool {
    matches!(e.kind, Kind::Arc { .. } | Kind::Circle { .. })
}

/// Rotate the dimension text to the absolute `angle` (`None` restores the automatic angle). Stored relative to the
/// automatic angle so the text turns with the dimension.
fn set_text_angle(c: &Ctx, d: &mut DimData, angle: Option<f64>) {
    d.tr = None;
    let auto = c.doc(|doc| dim_geometry(d, &doc.settings).text.rot);
    d.tr = angle.map(|a| norm_angle_signed(a - auto));
}

/// Apply `f` to the dimensions among `ids` (one change notification). Returns how many were edited.
fn edit_dims(c: &Ctx, ids: &[u64], f: impl Fn(&mut DimData)) -> usize {
    let edited: Vec<(u64, DimData)> = ids
        .iter()
        .filter_map(|id| match c.entity_by_id(*id)?.kind {
            Kind::Dim(mut d) => {
                f(&mut d);
                Some((*id, d))
            }
            _ => None,
        })
        .collect();
    c.doc_mut(|doc| {
        doc.change(|doc| {
            for (id, d) in &edited {
                if let Some(e) = doc.by_id_mut(*id) {
                    e.kind = Kind::Dim(d.clone());
                }
            }
        })
    });
    edited.len()
}

/// The Mtext / Text / Angle options shared by the commands that create dimensions.
#[derive(Clone, Default)]
struct TextOpts {
    txt: Option<String>,
    angle: Option<f64>,
}

impl TextOpts {
    /// Handle an option keyword; `false` when it is not one of Mtext / Text / Angle.
    async fn handle(&mut self, c: &Ctx, kw: &str) -> CmdResult<bool> {
        match kw {
            "Mtext" | "Text" => {
                let t = c.string(ps("Enter dimension text <>").raw()).await?.text;
                self.txt = (!t.is_empty() && t != "<>").then_some(t);
            }
            "Angle" => {
                if let Some(a) = c
                    .angle(ps("Specify angle of dimension text"))
                    .await?
                    .value()
                {
                    self.angle = Some(a);
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn apply(&self, c: &Ctx, d: &mut DimData) {
        d.txt = self.txt.clone();
        set_text_angle(c, d, self.angle);
    }

    fn finish(&self, c: &Ctx, mut e: Entity) -> Entity {
        if let Kind::Dim(d) = &mut e.kind {
            self.apply(c, d);
        }
        e
    }
}

// ---------------------------------------------------------------- DIMARC

/// Centre, radius, start angle and sweep of the arc under the pick: an ARC object or the nearest arc segment of a
/// polyline.
fn arc_at(c: &Ctx, e: &Entity, at: Pt) -> Option<(Pt, f64, f64, f64)> {
    let segs = c.doc(|d| segs_of(e, d).to_vec());
    let best = segs
        .iter()
        .min_by(|a, b| a.nearest(at).2.total_cmp(&b.nearest(at).2))?;
    match *best {
        Seg::A {
            c: ctr, r, a0, sw, ..
        } if sw >= 0.0 => Some((ctr, r, a0, sw)),
        Seg::A {
            c: ctr, r, a0, sw, ..
        } => Some((ctr, r, a0 + sw, -sw)),
        Seg::L { .. } => None,
    }
}

async fn dimarc(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c
        .entity(
            ps("Select arc or polyline arc segment")
                .filter(|e| matches!(e.kind, Kind::Arc { .. } | Kind::Pline { .. })),
        )
        .await?;
    let EntR::Entity { id, pt: at, .. } = sel else {
        return Ok(());
    };
    let Some((ctr, r, a0, sw)) = c.entity_by_id(id).and_then(|e| arc_at(&c, &e, at)) else {
        c.log_error("Select an arc or a polyline arc segment.");
        return Ok(());
    };
    let (mut p1, mut p2) = (polar(ctr, a0, r), polar(ctr, a0 + sw, r));
    let (ds, mut opts) = (cur_ds(&c), TextOpts::default());
    loop {
        let (ds2, txt) = (ds.clone(), opts.txt.clone());
        let make = move |dp: Pt| {
            dim_entity(DimType::Arc, &ds2, |d| {
                (d.c, d.p1, d.p2, d.dp, d.txt) =
                    (Some(ctr), Some(p1), Some(p2), Some(dp), txt.clone());
            })
        };
        let m2 = make.clone();
        let reply = c
            .point(
                ps("Specify arc length dimension location, or")
                    .options(&["Mtext", "Text", "Angle", "Partial", "Leader"])
                    .base(ctr)
                    .no_ortho()
                    .preview(move |_, p| vec![pv(m2(p))]),
            )
            .await?;
        match reply.kw() {
            Some("Partial") => {
                // parameter of a picked point along the arc, clamped to its ends
                let t = |q: Pt| match sweep_ccw(a0, angle_of(ctr, q)) {
                    s if s <= sw => s,
                    s if s - sw < TAU - s => sw,
                    _ => 0.0,
                };
                let Some(f) = c
                    .point(ps("Specify first point for arc length dimension").base(ctr))
                    .await?
                    .pt()
                else {
                    continue;
                };
                let Some(g) = c
                    .point(ps("Specify second point for arc length dimension").base(ctr))
                    .await?
                    .pt()
                else {
                    continue;
                };
                let (t0, t1) = (t(f).min(t(g)), t(f).max(t(g)));
                if t1 - t0 < 1e-9 {
                    c.log_error("The two points are the same.");
                    continue;
                }
                (p1, p2) = (polar(ctr, a0 + t0, r), polar(ctr, a0 + t1, r));
            }
            Some("Leader") => c.log_error("Leaders are not supported for arc length dimensions."),
            Some(kw) => {
                opts.handle(&c, kw).await?;
            }
            None => {
                let Some(dp) = reply.pt() else { return Ok(()) };
                c.add(opts.finish(&c, make(dp)));
                return Ok(());
            }
        }
    }
}

// ---------------------------------------------------------------- DIMJOGGED

/// First intersection of the ray from `o` along unit `dir` with the circle (`None` when it misses).
fn ray_circle(o: Pt, dir: Pt, ctr: Pt, r: f64) -> Option<Pt> {
    let f = o - ctr;
    let (b, disc) = (dot(f, dir), dot(f, dir).powi(2) - (dot(f, f) - r * r));
    if disc < 0.0 {
        return None;
    }
    let (t1, t2) = (-b - disc.sqrt(), -b + disc.sqrt());
    [t1, t2]
        .into_iter()
        .find(|t| *t > 1e-9)
        .map(|t| o + dir * t)
}

async fn dimjogged(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c
        .entity(ps("Select arc or circle").filter(is_arc_or_circle))
        .await?;
    let EntR::Entity { id, pt: at, .. } = sel else {
        return Ok(());
    };
    let Some(Kind::Circle { c: ctr, r } | Kind::Arc { c: ctr, r, .. }) =
        c.entity_by_id(id).map(|e| e.kind)
    else {
        return Ok(());
    };
    let Some(o) = c
        .point(ps("Specify center location override").base(ctr))
        .await?
        .pt()
    else {
        return Ok(());
    };
    let (ds, mut opts) = (cur_ds(&c), TextOpts::default());
    // the dimension line runs from the centre override through the location to the arc
    let on_arc = move |dp: Pt| {
        ray_circle(o, unit(dp - o), ctr, r).unwrap_or_else(|| {
            ctr + unit(if dist(ctr, dp) > 1e-9 {
                dp - ctr
            } else {
                at - ctr
            }) * r
        })
    };
    let make = move |ds: &str, txt: Option<String>, dp: Pt, jog: Option<Pt>| {
        let p = on_arc(dp);
        dim_entity(DimType::Jog, ds, |d| {
            (d.c, d.p1, d.pt, d.dp, d.p2, d.txt) = (
                Some(ctr),
                Some(o),
                Some(p),
                Some(dp),
                Some(jog.unwrap_or_else(|| mid(o, p))),
                txt,
            );
        })
    };
    let dp = loop {
        let (ds2, txt) = (ds.clone(), opts.txt.clone());
        let reply = c
            .point(
                ps("Specify dimension line location, or")
                    .options(&["Mtext", "Text", "Angle"])
                    .base(o)
                    .no_ortho()
                    .preview(move |_, p| vec![pv(make(&ds2, txt.clone(), p, None))]),
            )
            .await?;
        match reply.kw() {
            Some(kw) => {
                opts.handle(&c, kw).await?;
            }
            None => match reply.pt() {
                Some(p) => break p,
                None => return Ok(()),
            },
        }
    };
    let (ds2, txt) = (ds.clone(), opts.txt.clone());
    let reply = c
        .point(
            ps("Specify jog location")
                .base(o)
                .no_ortho()
                .preview(move |_, p| vec![pv(make(&ds2, txt.clone(), dp, Some(p)))]),
        )
        .await?;
    let Some(jog) = reply.pt() else { return Ok(()) };
    c.add(opts.finish(&c, make(&ds, opts.txt.clone(), dp, Some(jog))));
    Ok(())
}

// ---------------------------------------------------------------- DIMORDINATE

async fn dimordinate(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(f) = c.point(ps("Specify feature location")).await?.pt() else {
        return Ok(());
    };
    let (ds, mut opts) = (cur_ds(&c), TextOpts::default());
    let mut forced: Option<bool> = None;
    loop {
        let (ds2, txt) = (ds.clone(), opts.txt.clone());
        // coordinates are measured from the world origin (there is no user coordinate system); like AutoCAD, the X
        // ordinate is measured when the leader is longer vertically than horizontally
        let make = move |end: Pt| {
            let xd = forced.unwrap_or((end.y - f.y).abs() > (end.x - f.x).abs());
            dim_entity(DimType::Ord, &ds2, |d| {
                (d.c, d.pt, d.dp, d.xd, d.txt) =
                    (Some(pt(0.0, 0.0)), Some(f), Some(end), xd, txt.clone());
            })
        };
        let m2 = make.clone();
        let reply = c
            .point(
                ps("Specify leader endpoint or")
                    .options(&["Xdatum", "Ydatum", "Mtext", "Text", "Angle"])
                    .base(f)
                    .no_ortho()
                    .preview(move |_, p| vec![pv(m2(p))]),
            )
            .await?;
        match reply.kw() {
            Some("Xdatum") => forced = Some(true),
            Some("Ydatum") => forced = Some(false),
            Some(kw) => {
                opts.handle(&c, kw).await?;
            }
            None => {
                let Some(end) = reply.pt() else { return Ok(()) };
                c.add(opts.finish(&c, make(end)));
                return Ok(());
            }
        }
    }
}

// ---------------------------------------------------------------- QDIM

#[derive(Clone, Copy, PartialEq)]
enum Qdim {
    Continuous,
    Staggered,
    Baseline,
    Ordinate,
    Radius,
    Diameter,
}

impl Qdim {
    fn label(self) -> &'static str {
        [
            "Continuous",
            "Staggered",
            "Baseline",
            "Ordinate",
            "Radius",
            "Diameter",
        ][self as usize]
    }
}

/// Dimensionable features of the selected geometry: key points and circles / arcs.
#[derive(Clone, Default)]
struct Features {
    pts: Vec<Pt>,
    circles: Vec<(Pt, f64)>,
}

fn features(ents: &[Entity]) -> Features {
    let mut f = Features::default();
    for e in ents {
        match &e.kind {
            Kind::Line { a, b } => f.pts.extend([*a, *b]),
            Kind::Pline { v, .. } => f.pts.extend(v.iter().map(Vertex::pt)),
            Kind::Arc { c, r, a0, a1 } => {
                f.pts.extend([polar(*c, *a0, *r), polar(*c, *a1, *r)]);
                f.circles.push((*c, *r));
            }
            Kind::Circle { c, r } => {
                f.pts.push(*c);
                f.circles.push((*c, *r));
            }
            Kind::Point { p } => f.pts.push(*p),
            Kind::Insert(i) => f.pts.push(i.p),
            Kind::Ellipse(el) => f.pts.push(el.c),
            _ => {}
        }
    }
    f
}

/// The dimensions QDIM creates for `kind` with the dimension line through `dp`.
fn qdim_build(
    kind: Qdim,
    f: &Features,
    dp: Pt,
    datum: Option<Pt>,
    ds: &str,
    space: f64,
) -> Vec<Entity> {
    if matches!(kind, Qdim::Radius | Qdim::Diameter) {
        return f
            .circles
            .iter()
            .map(|&(c, r)| {
                let dir = unit(dp - c);
                dim_entity(
                    if kind == Qdim::Radius {
                        DimType::Rad
                    } else {
                        DimType::Dia
                    },
                    ds,
                    |d| {
                        (d.c, d.pt, d.dp) =
                            (Some(c), Some(c + dir * r), Some(c + dir * (r + space)));
                    },
                )
            })
            .collect();
    }
    if f.pts.is_empty() {
        return vec![];
    }
    // dimension lines above or below the geometry measure along X, beside it along Y
    let bb = Bounds::of_points(&f.pts);
    let outside = |lo: f64, hi: f64, v: f64| (lo - v).max(v - hi).max(0.0);
    let horiz = outside(bb.min_y, bb.max_y, dp.y) >= outside(bb.min_x, bb.max_x, dp.x);
    let key = |p: &Pt| if horiz { p.x } else { p.y };
    let mut pts = f.pts.clone();
    pts.sort_by(|a, b| key(a).total_cmp(&key(b)));
    pts.dedup_by(|a, b| (key(a) - key(b)).abs() < 1e-6);
    let side = if (if horiz {
        dp.y - bb.center().y
    } else {
        dp.x - bb.center().x
    }) >= 0.0
    {
        1.0
    } else {
        -1.0
    };
    let at = |m: Pt, k: f64| {
        if horiz {
            pt(m.x, dp.y + side * k * space)
        } else {
            pt(dp.x + side * k * space, m.y)
        }
    };
    let lin = |a: Pt, b: Pt, k: f64| {
        dim_entity(DimType::Lin, ds, |d| {
            (d.p1, d.p2, d.ang, d.dp) = (
                Some(a),
                Some(b),
                Some(if horiz { 0.0 } else { PI / 2.0 }),
                Some(at(mid(a, b), k)),
            );
        })
    };
    let origin = datum.unwrap_or(pts[0]);
    match kind {
        Qdim::Continuous => pts.windows(2).map(|w| lin(w[0], w[1], 0.0)).collect(),
        Qdim::Staggered => pts
            .windows(2)
            .enumerate()
            .map(|(i, w)| lin(w[0], w[1], (i % 2) as f64))
            .collect(),
        Qdim::Baseline => pts
            .iter()
            .filter(|p| (key(p) - key(&origin)).abs() > 1e-6)
            .enumerate()
            .map(|(i, p)| lin(origin, *p, i as f64))
            .collect(),
        _ => pts
            .iter()
            .map(|p| {
                dim_entity(DimType::Ord, ds, |d| {
                    (d.c, d.pt, d.dp, d.xd) = (
                        Some(origin),
                        Some(*p),
                        Some(if horiz { pt(p.x, dp.y) } else { pt(dp.x, p.y) }),
                        horiz,
                    );
                })
            })
            .collect(),
    }
}

/// QDIM's Edit option: remove or add the points that get dimensioned.
async fn qdim_edit(c: &Ctx, f: &mut Features) -> CmdResult<()> {
    let tol = c.app(|a| a.aperture_world(Some(10.0)));
    loop {
        match c
            .point(
                ps("Indicate dimension point to remove, or")
                    .options(&["Add", "eXit"])
                    .default_str("eXit"),
            )
            .await?
        {
            PointR::Kw(k) if k == "Add" => {
                if let Some(p) = c.point(ps("Indicate dimension point to add")).await?.pt() {
                    f.pts.push(p);
                }
            }
            PointR::Pt(p) => match f.pts.iter().position(|q| dist(*q, p) <= tol) {
                Some(i) => {
                    f.pts.remove(i);
                }
                None => c.log_error("No dimension point there."),
            },
            _ => return Ok(()),
        }
    }
}

async fn qdim(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select geometry to dimension")).await?;
    let mut feat = features(&c.entities(&sel.ents));
    if feat.pts.is_empty() && feat.circles.is_empty() {
        c.log_error("No geometry that can be dimensioned was selected.");
        return Ok(());
    }
    let ds = cur_ds(&c);
    let space = c.doc(|d| 3.75 * d.settings.cur_dim().scale);
    let (mut kind, mut datum) = (Qdim::Continuous, None);
    loop {
        let (f2, ds2) = (feat.clone(), ds.clone());
        let opts = [
            "Continuous",
            "Staggered",
            "Baseline",
            "Ordinate",
            "Radius",
            "Diameter",
            "datumPoint",
            "Edit",
        ];
        let reply = c
            .point(
                ps("Specify dimension line position, or")
                    .options(&opts)
                    .default_str(kind.label())
                    .no_ortho()
                    .preview(move |_, p| pvs(qdim_build(kind, &f2, p, datum, &ds2, space))),
            )
            .await?;
        let dp = match reply {
            PointR::Pt(p) => p,
            PointR::Enter(_) => c.app(|a| a.rt.cursor.pt),
            PointR::Kw(k) => {
                match k.as_str() {
                    "datumPoint" => {
                        datum = c.point(ps("Select new datum point")).await?.pt().or(datum)
                    }
                    "Edit" => qdim_edit(&c, &mut feat).await?,
                    other => {
                        kind = [
                            Qdim::Continuous,
                            Qdim::Staggered,
                            Qdim::Baseline,
                            Qdim::Ordinate,
                            Qdim::Radius,
                            Qdim::Diameter,
                        ]
                        .into_iter()
                        .find(|q| q.label() == other)
                        .unwrap_or(kind)
                    }
                }
                continue;
            }
            PointR::Num { .. } => continue,
        };
        let dims = qdim_build(kind, &feat, dp, datum, &ds, space);
        if dims.is_empty() {
            c.log_error("There is nothing to dimension.");
        }
        for e in dims {
            c.add(e);
        }
        return Ok(());
    }
}

// ---------------------------------------------------------------- DIMBREAK

/// Dimension breaks need straight dimension geometry (no angular / arc length dimensions).
fn breakable(doc: &Drawing, id: u64) -> bool {
    matches!(doc.by_id(id), Some(Entity { kind: Kind::Dim(d), .. }) if dim_geometry(d, &doc.settings).arc.is_none())
}

/// Breaks `(segment, from, to)` where the lines of dimension `id` cross other objects (all, or only `only`).
fn auto_breaks(doc: &Drawing, id: u64, only: Option<u64>) -> Vec<(usize, f64, f64)> {
    let Some(Entity {
        kind: Kind::Dim(d), ..
    }) = doc.by_id(id)
    else {
        return vec![];
    };
    let mut plain = d.clone();
    plain.brk.clear();
    let geo = dim_geometry(&plain, &doc.settings);
    let gap = 1.5 * geo.style.asz * geo.style.scale; // DIMBRKGAP
    let others: Vec<Seg> = doc
        .entities()
        .iter()
        .filter(|e| {
            e.id != id && (only.is_none() || only == Some(e.id)) && (e.is_curve() || is_dim(e))
        })
        .flat_map(|e| segs_of(e, doc).to_vec())
        .collect();
    let mut out = vec![];
    for (i, s) in geo.segs.iter().enumerate().filter(|(_, s)| s.is_line()) {
        let l = s.length();
        for h in others
            .iter()
            .flat_map(|o| seg_intersect(s, o, false, false))
        {
            let t = h.t1 * l;
            if t > gap / 2.0 && t < l - gap / 2.0 {
                out.push((i, t - gap / 2.0, t + gap / 2.0));
            }
        }
    }
    out
}

/// The break between two picked points along the dimension line nearest to the first.
fn manual_break(doc: &Drawing, id: u64, p1: Pt, p2: Pt) -> Option<(usize, f64, f64)> {
    let Some(Entity {
        kind: Kind::Dim(d), ..
    }) = doc.by_id(id)
    else {
        return None;
    };
    let mut plain = d.clone();
    plain.brk.clear();
    let geo = dim_geometry(&plain, &doc.settings);
    let (i, s) = geo
        .segs
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_line())
        .min_by(|a, b| a.1.nearest(p1).2.total_cmp(&b.1.nearest(p1).2))?;
    let (t1, t2) = (
        s.project(p1).clamp(0.0, 1.0) * s.length(),
        s.project(p2).clamp(0.0, 1.0) * s.length(),
    );
    (t1 != t2).then_some((i, t1.min(t2), t1.max(t2)))
}

#[derive(Clone, Copy, PartialEq)]
enum BreakMode {
    Auto,
    Manual,
    Remove,
}

async fn dimbreak(c: Ctx, _a: Args) -> CmdResult<()> {
    let first = c
        .entity(
            ps("Select dimension to add/remove break or")
                .options(&["Multiple"])
                .filter(is_dim),
        )
        .await?;
    let (ids, mode, only) = match first {
        EntR::Entity { id, .. } => {
            let (mode, only) = match c
                .entity(
                    ps("Select object to break dimension or")
                        .options(&["Auto", "Manual", "Remove"])
                        .allow_enter()
                        .default_str("Auto"),
                )
                .await?
            {
                EntR::Kw(k) if k == "Manual" => (BreakMode::Manual, None),
                EntR::Kw(k) if k == "Remove" => (BreakMode::Remove, None),
                EntR::Entity { id: other, .. } => (BreakMode::Auto, Some(other)),
                _ => (BreakMode::Auto, None),
            };
            (vec![id], mode, only)
        }
        EntR::Kw(_) => {
            let ids = c.select(ps("Select dimensions").filter(is_dim)).await?.ents;
            let mode = if c
                .keyword(
                    ps("Enter an option")
                        .options(&["Break", "Remove"])
                        .default_str("Break"),
                )
                .await?
                .kw()
                == Some("Remove")
            {
                BreakMode::Remove
            } else {
                BreakMode::Auto
            };
            (ids, mode, None)
        }
        EntR::Enter => return Ok(()),
    };
    let manual = if mode == BreakMode::Manual {
        let Some(p1) = c.point(ps("Specify first break point")).await?.pt() else {
            return Ok(());
        };
        let Some(p2) = c
            .point(ps("Specify second break point").base(p1))
            .await?
            .pt()
        else {
            return Ok(());
        };
        Some((p1, p2))
    } else {
        None
    };
    let mut added = 0;
    for id in ids {
        if mode == BreakMode::Remove {
            edit_dims(&c, &[id], |d| d.brk.clear());
        } else if !c.doc(|d| breakable(d, id)) {
            c.log_error("Breaks are not supported for angular and arc length dimensions.");
        } else {
            let new = c.doc(|d| match manual {
                Some((p1, p2)) => manual_break(d, id, p1, p2).into_iter().collect(),
                None => auto_breaks(d, id, only),
            });
            added += new.len();
            edit_dims(&c, &[id], |d| {
                d.brk.extend(
                    new.iter()
                        .filter(|n| !d.brk.iter().any(|o| o.0 == n.0 && (o.1 - n.1).abs() < 1e-6))
                        .copied()
                        .collect::<Vec<_>>(),
                );
            });
        }
    }
    if mode != BreakMode::Remove {
        c.log(&format!(
            "{added} dimension break{} added.",
            if added == 1 { "" } else { "s" }
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------- DIMCENTER

async fn dimcenter(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c
        .entity(ps("Select arc or circle").filter(is_arc_or_circle))
        .await?;
    let Some(Kind::Circle { c: ctr, r } | Kind::Arc { c: ctr, r, .. }) =
        sel.id().and_then(|i| c.entity_by_id(i)).map(|e| e.kind)
    else {
        return Ok(());
    };
    // DIMCEN: positive = centre mark, negative = centre lines
    let size = c.doc(|d| d.settings.cur_dim().cen * d.settings.cur_dim().scale);
    if size == 0.0 {
        c.log_error("The current dimension style draws no center marks (DIMCEN is 0).");
        return Ok(());
    }
    let a = size.abs();
    let mut lines = vec![];
    for u in [pt(1.0, 0.0), pt(0.0, 1.0)] {
        if size > 0.0 {
            lines.push((ctr - u * a, ctr + u * a));
        } else {
            lines.push((ctr - u * (a / 2.0), ctr + u * (a / 2.0)));
            for s in [-1.0, 1.0] {
                lines.push((ctr + u * (s * a), ctr + u * (s * (r + a))));
            }
        }
    }
    let grp = group_id(&c);
    for (p, q) in lines {
        let mut e = Entity::line(p, q);
        e.grp = Some(grp.clone());
        c.add(e);
    }
    Ok(())
}

// ---------------------------------------------------------------- DIMSPACE

/// Direction of the dimension line of a linear or aligned dimension.
fn dim_dir(d: &DimData) -> Option<Pt> {
    match d.kind {
        DimType::Lin => d.ang.map(|a| pt(a.cos(), a.sin())).or(Some(pt(1.0, 0.0))),
        DimType::Ali => Some(unit(d.p2? - d.p1?)),
        _ => None,
    }
}

async fn dimspace(c: Ctx, _a: Args) -> CmdResult<()> {
    let linear = |e: &Entity| matches!(&e.kind, Kind::Dim(d) if dim_dir(d).is_some());
    let Some(base_id) = c
        .entity(ps("Select base dimension").filter(linear))
        .await?
        .id()
    else {
        return Ok(());
    };
    let sel = c
        .select(ps("Select dimensions to space").filter(linear))
        .await?;
    let ids: Vec<u64> = sel.ents.into_iter().filter(|i| *i != base_id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let Some(Kind::Dim(base)) = c.entity_by_id(base_id).map(|e| e.kind) else {
        return Ok(());
    };
    let auto = c.doc(|d| {
        let st = dim_style_of(&base, &d.settings);
        2.0 * st.txt * st.scale // Auto: twice the dimension text height
    });
    let space = match c
        .number(
            ps("Enter value or")
                .options(&["Auto"])
                .default_str("Auto")
                .min(0.0),
        )
        .await?
    {
        NumR::Value(v) => v,
        _ => auto,
    };
    let (dir, base_dp) = (
        dim_dir(&base).unwrap_or(pt(1.0, 0.0)),
        base.dp.unwrap_or(Pt::ZERO),
    );
    let n = perp(dir);
    let offset = |d: &DimData| dot(d.dp.unwrap_or(Pt::ZERO) - base_dp, n);
    // parallel dimensions keep their side of the base dimension and their order, and move to multiples of the spacing
    let mut dims: Vec<(u64, f64)> = vec![];
    for id in &ids {
        if let Some(Kind::Dim(d)) = c.entity_by_id(*id).map(|e| e.kind) {
            if dim_dir(&d).is_some_and(|v| cross(v, dir).abs() < 1e-6) {
                dims.push((*id, offset(&d)));
            }
        }
    }
    dims.sort_by(|a, b| a.1.total_cmp(&b.1));
    let split = dims.partition_point(|d| d.1 < -1e-9);
    let mut shifts: Vec<(u64, f64)> = vec![];
    for (k, (id, off)) in dims[split..].iter().enumerate() {
        shifts.push((*id, (k + 1) as f64 * space - off));
    }
    for (k, (id, off)) in dims[..split].iter().rev().enumerate() {
        shifts.push((*id, -((k + 1) as f64) * space - off));
    }
    for (id, delta) in &shifts {
        edit_dims(&c, &[*id], |d| {
            d.dp = d.dp.map(|p| p + n * *delta);
            d.tp = d.tp.map(|p| p + n * *delta);
        });
    }
    if shifts.len() < ids.len() {
        c.log(&format!(
            "{} dimension(s) skipped: not parallel to the base dimension.",
            ids.len() - shifts.len()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------- DIMEDIT / DIMTEDIT

async fn dimedit(c: Ctx, _a: Args) -> CmdResult<()> {
    let kind = c
        .keyword(
            ps("Enter type of dimension editing")
                .options(&["Home", "New", "Rotate", "Oblique"])
                .default_str("Home"),
        )
        .await?;
    let Some(kind) = kind.kw().map(str::to_string) else {
        return Ok(());
    };
    let mut text = None;
    let mut angle = None;
    match kind.as_str() {
        "New" => {
            text = Some(
                c.string(ps("Enter new dimension text <>").raw())
                    .await?
                    .text,
            )
        }
        "Rotate" => {
            let Some(a) = c
                .angle(ps("Specify angle for dimension text"))
                .await?
                .value()
            else {
                return Ok(());
            };
            angle = Some(a);
        }
        _ => {}
    }
    let ids = c.select(ps("Select objects").filter(is_dim)).await?.ents;
    if ids.is_empty() {
        return Ok(());
    }
    match kind.as_str() {
        "Home" => edit_dims(&c, &ids, |d| (d.tp, d.tr) = (None, None)),
        "New" => edit_dims(&c, &ids, |d| {
            d.txt = text.clone().filter(|t| !t.is_empty() && t != "<>")
        }),
        "Rotate" => edit_dims(&c, &ids, |d| set_text_angle(&c, d, angle)),
        _ => {
            // Oblique: extension lines at the given angle (Enter restores perpendicular ones)
            let theta = c
                .angle(ps("Enter obliquing angle (press ENTER for none)").allow_enter())
                .await?
                .value();
            edit_dims(&c, &ids, |d| {
                let Some(dir) = dim_dir(d) else { return };
                d.obl = theta.map(|t| {
                    // a line direction: fold into (-90°, 90°]
                    let dev = norm_angle_signed(t - angle_of(Pt::ZERO, dir) - PI / 2.0);
                    if dev > PI / 2.0 {
                        dev - PI
                    } else if dev <= -PI / 2.0 {
                        dev + PI
                    } else {
                        dev
                    }
                });
            })
        }
    };
    Ok(())
}

async fn dimtedit(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(id) = c.entity(ps("Select dimension").filter(is_dim)).await?.id() else {
        return Ok(());
    };
    let Some(Kind::Dim(dim)) = c.entity_by_id(id).map(|e| e.kind) else {
        return Ok(());
    };
    let moved = dim.clone();
    let reply = c
        .point(
            ps("Specify new location for dimension text or")
                .options(&["Left", "Right", "Center", "Home", "Angle"])
                .no_ortho()
                .preview(move |_, p| {
                    vec![pv(Entity::dim(DimData {
                        tp: Some(p),
                        ..moved.clone()
                    }))]
                }),
        )
        .await?;
    match reply.kw() {
        Some(side @ ("Left" | "Right")) => {
            // text beside the first / last arrow, on the side of the dimension line it normally sits
            let geo = c.doc(|d| {
                dim_geometry(
                    &DimData {
                        tp: None,
                        tr: None,
                        ..dim
                    },
                    &d.settings,
                )
            });
            let Some(line) = geo.dim_line.as_ref() else {
                c.log_error("Left and Right apply to linear dimensions only.");
                return Ok(());
            };
            let k = geo.style.scale;
            let shift = dist(line.a, line.b) / 2.0
                - geo.style.asz * k
                - geo.style.gap * k
                - measure_text(&geo.text.s, geo.text.h) / 2.0;
            let sgn = if side == "Left" { -1.0 } else { 1.0 };
            edit_dims(&c, &[id], |d| {
                d.tp = Some(geo.text.p + line.dir * (sgn * shift.max(0.0)))
            });
        }
        Some("Center") => {
            edit_dims(&c, &[id], |d| d.tp = None);
        }
        Some("Home") => {
            edit_dims(&c, &[id], |d| (d.tp, d.tr) = (None, None));
        }
        Some("Angle") => {
            if let Some(a) = c
                .angle(ps("Specify angle for dimension text"))
                .await?
                .value()
            {
                edit_dims(&c, &[id], |d| set_text_angle(&c, d, Some(a)));
            }
        }
        _ => {
            if let Some(p) = reply.pt() {
                edit_dims(&c, &[id], |d| d.tp = Some(p));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- DIMINSPECT

async fn diminspect(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = c
        .select(ps("Select dimensions to inspect").filter(is_dim))
        .await?
        .ents;
    if ids.is_empty() {
        return Ok(());
    }
    let remove = c
        .keyword(
            ps("Enter an option")
                .options(&["Add", "Remove"])
                .default_str("Add"),
        )
        .await?
        .kw()
        == Some("Remove");
    if remove {
        edit_dims(&c, &ids, |d| d.insp = None);
        return Ok(());
    }
    let shape = match c
        .keyword(
            ps("Enter inspection shape")
                .options(&["Round", "Angular", "None"])
                .default_str("Round"),
        )
        .await?
        .kw()
    {
        Some("Angular") => InspShape::Angular,
        Some("None") => InspShape::None,
        _ => InspShape::Round,
    };
    let label = c
        .string(ps("Enter label text (Enter for none)").raw())
        .await?
        .text;
    let rate = c
        .number(
            ps("Enter inspection rate in percent (0 for none)")
                .default_num(100.0)
                .integer()
                .min(0.0)
                .max(100.0),
        )
        .await?
        .value()
        .unwrap_or(100.0) as u32;
    edit_dims(&c, &ids, |d| {
        d.insp = Some(DimInspect {
            label: label.clone(),
            rate: (rate > 0).then_some(rate),
            shape,
        })
    });
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(
        def("Annotation", "DIMARC", run(dimarc))
            .aliases(&["DAR"])
            .no_pickfirst()
            .title("Arc Length")
            .desc("Creates an arc length dimension."),
    );
    r.define(
        def("Annotation", "DIMJOGGED", run(dimjogged))
            .aliases(&["DJO"])
            .no_pickfirst()
            .title("Jogged")
            .desc("Creates jogged dimensions for circles and arcs."),
    );
    r.define(
        def("Annotation", "DIMORDINATE", run(dimordinate))
            .aliases(&["DOR"])
            .no_pickfirst()
            .title("Ordinate")
            .desc("Creates ordinate dimensions."),
    );
    r.define(
        def("Annotation", "QDIM", run(qdim))
            .title("Quick Dimension")
            .desc("Quickly creates a series of dimensions from selected objects."),
    );
    r.define(
        def("Annotation", "DIMBREAK", run(dimbreak))
            .no_pickfirst()
            .title("Break")
            .desc(
                "Breaks or restores dimension and extension lines where they cross other objects.",
            ),
    );
    r.define(
        def("Annotation", "DIMCENTER", run(dimcenter))
            .aliases(&["DCE"])
            .no_pickfirst()
            .title("Center Mark")
            .desc("Creates the center mark or the centerlines of circles and arcs."),
    );
    r.define(
        def("Annotation", "DIMSPACE", run(dimspace))
            .no_pickfirst()
            .title("Adjust Space")
            .desc("Adjusts the spacing between parallel linear dimensions."),
    );
    r.define(
        def("Annotation", "DIMEDIT", run(dimedit))
            .aliases(&["DED"])
            .title("Edit Dimension")
            .desc("Edits dimension text and extension lines."),
    );
    r.define(
        def("Annotation", "DIMTEDIT", run(dimtedit))
            .aliases(&["DIMTED"])
            .no_pickfirst()
            .title("Dimension Text Edit")
            .desc("Moves and rotates dimension text."),
    );
    r.define(
        def("Annotation", "DIMINSPECT", run(diminspect))
            .title("Inspection")
            .desc("Adds or removes inspection information for a selected dimension."),
    );
}
