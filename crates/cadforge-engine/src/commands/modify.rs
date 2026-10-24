//! Modify commands (port of `src/commands/modify.js`).
use super::helpers::*;
use super::{def, run};
use crate::app::{Clipboard, Event};
use crate::ctx::*;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::fillet::*;
use cadforge_core::ops::offset::offset_entity;
use cadforge_core::ops::path::*;
use cadforge_core::ops::trim::*;
use cadforge_core::solid::Solid;
use std::rc::Rc;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

const SELECT: &str = "Select objects";

fn dm(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("Modify", name, f)
}

fn is_curve_type(e: &Entity, kinds: &[&str]) -> bool {
    kinds.contains(&e.type_name())
}

fn bounds_union(ents: &[Entity], doc: &Drawing) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;
    for e in ents {
        b = Bounds::union_opt(b, bounds_of(e, doc));
    }
    b
}

/// Entities and solids of a selection, cloned for previews and copies.
fn sel_data(c: &Ctx, sel: &Selection) -> (Vec<Entity>, Vec<Solid>) {
    (c.entities(&sel.ents), c.solids(&sel.solids))
}

// ---------------------------------------------------------------- ERASE / OOPS

async fn erase(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps(SELECT).solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let ents: Vec<Entity> = c
        .entities(&sel.ents)
        .iter()
        .map(|e| e.cloned_noid())
        .collect();
    c.app(|a| a.mem.erased = Some(ents));
    c.doc_mut(|d| {
        d.remove_all(&sel.ents);
        for s in &sel.solids {
            d.remove_solid(*s);
        }
    });
    c.clear_sel();
    Ok(())
}

async fn oops(c: Ctx, _a: Args) -> CmdResult<()> {
    let erased = c.app(|a| a.mem.erased.take());
    match erased {
        Some(list) if !list.is_empty() => {
            for e in list {
                c.add(e.cloned_noid());
            }
        }
        _ => c.log_error("Nothing to restore."),
    }
    Ok(())
}

// ---------------------------------------------------------------- MOVE / COPY

async fn move_(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps(SELECT).solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let (ents, _) = sel_data(&c, &sel);
    let r = c
        .point(ps("Specify base point").options(&["Displacement"]))
        .await?;
    let delta;
    if r.kw() == Some("Displacement") {
        let d = c
            .point(ps("Specify displacement").default_pt(pt(0.0, 0.0)))
            .await?;
        let Some(p) = d.pt() else { return Ok(()) };
        delta = p;
    } else {
        let Some(base) = r.pt() else { return Ok(()) };
        let r2 = c
            .point(
                ps("Specify second point or <use first point as displacement>")
                    .base(base)
                    .allow_enter()
                    .preview(move |_, p| {
                        pvs(transformed_clones(
                            &ents,
                            &mat::translate(p.x - base.x, p.y - base.y),
                        ))
                    }),
            )
            .await?;
        delta = match r2.pt() {
            Some(p) => p - base,
            None => base,
        };
    }
    c.doc_mut(|d| apply_transform(d, &sel.ents, &sel.solids, &mat::translate(delta.x, delta.y)));
    c.clear_sel();
    Ok(())
}

async fn copy(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps(SELECT).solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let (ents, solids) = sel_data(&c, &sel);
    let r = c
        .point(ps("Specify base point").options(&["Displacement", "mOde"]))
        .await?;
    if r.kw() == Some("Displacement") {
        let d = c
            .point(ps("Specify displacement").default_pt(pt(0.0, 0.0)))
            .await?;
        if let Some(p) = d.pt() {
            c.doc_mut(|dd| add_copies(dd, &ents, &solids, &mat::translate(p.x, p.y)));
        }
        c.clear_sel();
        return Ok(());
    }
    let Some(base) = r.pt() else { return Ok(()) };
    let mut made: Vec<(Vec<u64>, Vec<u64>)> = vec![];
    loop {
        let opts: &[&str] = if made.is_empty() {
            &[]
        } else {
            &["Array", "Exit", "Undo"]
        };
        let ents2 = ents.clone();
        let n = c
            .point(
                ps(if made.is_empty() {
                    "Specify second point or [Array]"
                } else {
                    "Specify second point"
                })
                .options(opts)
                .base(base)
                .allow_enter()
                .preview(move |_, p| {
                    pvs(transformed_clones(
                        &ents2,
                        &mat::translate(p.x - base.x, p.y - base.y),
                    ))
                }),
            )
            .await?;
        match n.kw() {
            Some("Undo") => {
                if let Some((e, s)) = made.pop() {
                    c.doc_mut(|d| {
                        d.remove_all(&e);
                        for sd in s {
                            d.remove_solid(sd);
                        }
                    });
                }
                continue;
            }
            Some("Exit") => break,
            _ => {}
        }
        let Some(p) = n.pt() else { break };
        let m = mat::translate(p.x - base.x, p.y - base.y);
        made.push(c.doc_mut(|d| add_copies(d, &ents, &solids, &m)));
    }
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- ROTATE / SCALE / MIRROR

fn do_transform(c: &Ctx, sel: &Selection, ents: &[Entity], solids: &[Solid], m: &Mat, copy: bool) {
    if copy {
        c.doc_mut(|d| add_copies(d, ents, solids, m));
    } else {
        c.doc_mut(|d| apply_transform(d, &sel.ents, &sel.solids, m));
    }
    c.clear_sel();
}

async fn rotate(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps(SELECT).solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let (ents, solids) = sel_data(&c, &sel);
    let Some(base) = c.point(ps("Specify base point")).await?.pt() else {
        return Ok(());
    };
    let (mut copy, mut ref_angle) = (false, 0.0);
    loop {
        let e2 = ents.clone();
        let a = c
            .angle(
                ps("Specify rotation angle")
                    .options(&["Copy", "Reference"])
                    .base(base)
                    .default_num(0.0)
                    .preview(move |_, p| {
                        pvs(transformed_clones(
                            &e2,
                            &mat::rotate(angle_of(base, p) - ref_angle, base),
                        ))
                    }),
            )
            .await?;
        match a.kw() {
            Some("Copy") => {
                copy = true;
                continue;
            }
            Some("Reference") => {
                let ra = c
                    .angle(
                        ps("Specify the reference angle")
                            .base(base)
                            .default_num(0.0),
                    )
                    .await?;
                let Some(rv) = ra.value() else { return Ok(()) };
                ref_angle = rv;
                let e3 = ents.clone();
                let na = c
                    .angle(
                        ps("Specify the new angle")
                            .base(base)
                            .default_num(0.0)
                            .preview(move |_, p| {
                                pvs(transformed_clones(
                                    &e3,
                                    &mat::rotate(angle_of(base, p) - rv, base),
                                ))
                            }),
                    )
                    .await?;
                let Some(nv) = na.value() else { return Ok(()) };
                do_transform(
                    &c,
                    &sel,
                    &ents,
                    &solids,
                    &mat::rotate(nv - ref_angle, base),
                    copy,
                );
                break;
            }
            _ => {}
        }
        let Some(v) = a.value() else { return Ok(()) };
        let ang = if a.pt().is_some() { v - ref_angle } else { v };
        do_transform(&c, &sel, &ents, &solids, &mat::rotate(ang, base), copy);
        break;
    }
    Ok(())
}

async fn scale(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps(SELECT).solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let (ents, solids) = sel_data(&c, &sel);
    let Some(base) = c.point(ps("Specify base point")).await?.pt() else {
        return Ok(());
    };
    let mut copy = false;
    loop {
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
        if s.kw() == Some("Copy") {
            copy = true;
            continue;
        }
        let factor;
        if s.kw() == Some("Reference") {
            let rl = c
                .distance(
                    ps("Specify reference length")
                        .default_num(1.0)
                        .nonzero()
                        .min(0.0),
                )
                .await?;
            let Some(rv) = rl.value() else { return Ok(()) };
            let nl = c
                .distance(ps("Specify new length").base(base).nonzero().min(0.0))
                .await?;
            let Some(nv) = nl.value() else { return Ok(()) };
            factor = nv / rv;
        } else {
            let Some(v) = s.value() else { return Ok(()) };
            factor = v;
        }
        if !(factor > 0.0) {
            c.log_error("The scale factor must be positive.");
            return Ok(());
        }
        do_transform(
            &c,
            &sel,
            &ents,
            &solids,
            &mat::scale(factor, factor, base),
            copy,
        );
        return Ok(());
    }
}

async fn mirror(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps(SELECT).solids()).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let (ents, solids) = sel_data(&c, &sel);
    let Some(a) = c
        .point(ps("Specify first point of mirror line"))
        .await?
        .pt()
    else {
        return Ok(());
    };
    let e2 = ents.clone();
    let b = c
        .point(
            ps("Specify second point of mirror line")
                .base(a)
                .preview(move |_, p| {
                    if same(a, p) {
                        return vec![];
                    }
                    let mut v = vec![pv(Entity::line(a, p))];
                    v.extend(pvs(transformed_clones(&e2, &mat::mirror(a, p))));
                    v
                }),
        )
        .await?;
    let Some(bp) = b.pt() else { return Ok(()) };
    if same(a, bp) {
        return Ok(());
    }
    let del = c
        .keyword(
            ps("Erase source objects? [Yes/No]")
                .options(&["Yes", "No"])
                .default_str("No"),
        )
        .await?;
    let m = mat::mirror(a, bp);
    do_transform(&c, &sel, &ents, &solids, &m, del.kw() != Some("Yes"));
    Ok(())
}

// ---------------------------------------------------------------- OFFSET

fn nearest_distance(e: &Entity, p: Pt, doc: &Drawing) -> f64 {
    segs_of(e, doc)
        .iter()
        .map(|s| s.nearest(p).2)
        .fold(f64::INFINITY, f64::min)
}

async fn offset(c: Ctx, _a: Args) -> CmdResult<()> {
    let gap = c.doc(|d| d.settings.offset_gap);
    let mut dist_: Option<f64> = if gap > 0.0 { Some(gap) } else { None };
    let mut erase = false;
    let mut through = dist_.is_none();
    loop {
        let spec = ps("Specify offset distance")
            .options(&["Through", "Erase", "Layer"])
            .min(0.0);
        let spec = if through {
            spec.default_str("Through")
        } else {
            spec.default_num(dist_.unwrap_or(0.0))
        };
        let first = c.distance(spec).await?;
        match first.kw() {
            Some("Erase") => {
                let y = c
                    .keyword(
                        ps("Erase source object after offsetting? [Yes/No]")
                            .options(&["Yes", "No"])
                            .default_str(if erase { "Yes" } else { "No" }),
                    )
                    .await?;
                erase = y.kw() == Some("Yes");
                continue;
            }
            Some("Layer") => {
                c.log("Offset objects are created on the layer of the source object.");
                continue;
            }
            Some("Through") => {
                through = true;
                break;
            }
            _ => {}
        }
        if matches!(first, MeasR::Enter(None)) && through {
            break;
        }
        let Some(v) = first.value() else {
            return Ok(());
        };
        if v <= 0.0 {
            c.log_error("Requires a positive, nonzero distance.");
            continue;
        }
        dist_ = Some(v);
        through = false;
        c.doc_mut(|d| d.settings.offset_gap = v);
        break;
    }
    let mut done: Vec<u64> = vec![];
    loop {
        let o = c
            .entity(
                ps("Select object to offset")
                    .options(&["Exit", "Undo"])
                    .allow_enter()
                    .filter(|e| {
                        is_curve_type(
                            e,
                            &[
                                "line", "circle", "arc", "pline", "ellipse", "spline", "xline",
                            ],
                        )
                    }),
            )
            .await?;
        match o.kw() {
            Some("Undo") => {
                if let Some(u) = done.pop() {
                    c.doc_mut(|d| d.remove(u));
                }
                continue;
            }
            Some("Exit") => return Ok(()),
            _ => {}
        }
        let Some(src_id) = o.id() else { return Ok(()) };
        let Some(src) = c.entity_by_id(src_id) else {
            continue;
        };
        c.ed(|e| e.temp_sel = [src_id].into_iter().collect());
        let src2 = src.clone();
        let dd = dist_.unwrap_or(0.0);
        let side = c
            .point(
                ps(if through {
                    "Specify through point"
                } else {
                    "Specify point on side to offset"
                })
                .options(&["Exit", "Multiple", "Undo"])
                .allow_enter()
                .preview(move |d, p| {
                    let dd2 = if through {
                        nearest_distance(&src2, p, d).max(1e-9)
                    } else {
                        dd
                    };
                    match offset_entity(&src2, dd2, p, d) {
                        Ok(r) => vec![pv(r.entity)],
                        Err(_) => vec![],
                    }
                }),
            )
            .await;
        c.ed(|e| e.temp_sel.clear());
        let side = side?;
        match side.kw() {
            Some("Undo") => {
                if let Some(u) = done.pop() {
                    c.doc_mut(|d| d.remove(u));
                }
                continue;
            }
            Some("Exit") => return Ok(()),
            _ => {}
        }
        let Some(sp) = side.pt() else {
            if matches!(side, PointR::Enter(_)) {
                return Ok(());
            }
            continue;
        };
        let res = c.doc(|d| {
            let dd = if through {
                nearest_distance(&src, sp, d)
            } else {
                dd
            };
            offset_entity(&src, dd, sp, d)
        });
        match res {
            Err(e) => c.log_error(&e),
            Ok(r) => {
                let made = c.add(r.entity);
                done.push(made);
                if erase {
                    c.doc_mut(|d| d.remove(src_id));
                }
            }
        }
    }
}

// ---------------------------------------------------------------- ARRAY

fn centroid_of(ents: &[Entity], doc: &Drawing) -> Pt {
    match bounds_union(ents, doc) {
        Some(b) => b.center(),
        None => pt(0.0, 0.0),
    }
}

async fn array_command(c: Ctx, kind: Option<&'static str>) -> CmdResult<()> {
    let sel = c.select(ps(SELECT)).await?;
    if sel.is_empty() {
        return Ok(());
    }
    let (ents, solids) = sel_data(&c, &sel);
    let mem = c.app(|a| a.mem.clone());
    let mut ty = kind.map(str::to_string);
    if ty.is_none() {
        let t = c
            .keyword(
                ps("Enter array type [Rectangular/Path/POlar]")
                    .options(&["Rectangular", "POlar"])
                    .default_str("Rectangular"),
            )
            .await?;
        ty = Some(if t.kw() == Some("POlar") {
            "polar".into()
        } else {
            "rect".into()
        });
    }
    if ty.as_deref() == Some("rect") {
        let r = c
            .number(
                ps("Enter the number of rows (---)")
                    .default_num(mem.array_rect.rows as f64)
                    .integer()
                    .min(1.0),
            )
            .await?;
        let cl = c
            .number(
                ps("Enter the number of columns (|||)")
                    .default_num(mem.array_rect.cols as f64)
                    .integer()
                    .min(1.0),
            )
            .await?;
        let (Some(rv), Some(cv)) = (r.value(), cl.value()) else {
            return Ok(());
        };
        let (mut dr, mut dc) = (0.0, 0.0);
        if rv > 1.0 {
            let d = c
                .distance(
                    ps("Specify the distance between rows (---)").default_num(mem.array_rect.dr),
                )
                .await?;
            let Some(v) = d.value() else { return Ok(()) };
            dr = v;
        }
        if cv > 1.0 {
            let d = c
                .distance(
                    ps("Specify the distance between columns (|||)").default_num(mem.array_rect.dc),
                )
                .await?;
            let Some(v) = d.value() else { return Ok(()) };
            dc = v;
        }
        c.app(|a| {
            a.mem.array_rect = crate::memory::ArrayRect {
                rows: rv as i64,
                cols: cv as i64,
                dr: if dr != 0.0 { dr } else { mem.array_rect.dr },
                dc: if dc != 0.0 { dc } else { mem.array_rect.dc },
            };
        });
        if rv * cv > 100000.0 {
            c.log_error("The array would contain too many objects.");
            return Ok(());
        }
        for i in 0..rv as i64 {
            for j in 0..cv as i64 {
                if i != 0 || j != 0 {
                    c.doc_mut(|d| {
                        add_copies(
                            d,
                            &ents,
                            &solids,
                            &mat::translate(j as f64 * dc, i as f64 * dr),
                        )
                    });
                }
            }
        }
    } else {
        let Some(cp) = c.point(ps("Specify center point of array")).await?.pt() else {
            return Ok(());
        };
        let n = c
            .number(
                ps("Enter number of items in the array")
                    .default_num(mem.array_polar.n as f64)
                    .integer()
                    .min(2.0),
            )
            .await?;
        let Some(nv) = n.value() else { return Ok(()) };
        let fill = c
            .angle(
                ps("Specify the angle to fill (+=ccw, -=cw)")
                    .default_num(rad(mem.array_polar.fill)),
            )
            .await?;
        let Some(fv) = fill.value() else {
            return Ok(());
        };
        let rot = c
            .keyword(
                ps("Rotate arrayed objects as they are copied? [Yes/No]")
                    .options(&["Yes", "No"])
                    .default_str(if mem.array_polar.rotate { "Yes" } else { "No" }),
            )
            .await?;
        let rotate = rot.kw() != Some("No");
        let full = (fv.abs() - TAU).abs() < 1e-6;
        let step = fv / if full { nv } else { nv - 1.0 };
        c.app(|a| {
            a.mem.array_polar = crate::memory::ArrayPolar {
                n: nv as i64,
                fill: deg(fv),
                rotate,
            }
        });
        let centre = c.doc(|d| centroid_of(&ents, d));
        for i in 1..nv as i64 {
            let a = step * i as f64;
            if rotate {
                c.doc_mut(|d| add_copies(d, &ents, &solids, &mat::rotate(a, cp)));
            } else {
                let delta = mat::apply(&mat::rotate(a, cp), centre) - centre;
                c.doc_mut(|d| add_copies(d, &ents, &solids, &mat::translate(delta.x, delta.y)));
            }
        }
    }
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- STRETCH

async fn stretch(c: Ctx, _a: Args) -> CmdResult<()> {
    c.app(|a| a.rt.last_window = None);
    let sel = c
        .select(
            ps("Select objects to stretch by crossing-window or crossing-polygon").pickfirst(false),
        )
        .await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let win = c.app(|a| a.rt.last_window.map(|w| w.0));
    let ids = sel.ents.clone();
    let rect_for = move |e: &Entity, d: &Drawing| -> Bounds {
        let b = bounds_of(e, d).unwrap_or(Bounds::new(0.0, 0.0, 0.0, 0.0));
        match win {
            Some(w) if w.overlaps(&b) => w,
            _ => b.grow(1.0),
        }
    };
    let b = c
        .point(ps("Specify base point or [Displacement]").options(&["Displacement"]))
        .await?;
    let delta;
    if b.kw() == Some("Displacement") {
        let d = c
            .point(ps("Specify displacement").default_pt(pt(0.0, 0.0)))
            .await?;
        let Some(p) = d.pt() else { return Ok(()) };
        delta = p;
    } else {
        let Some(base) = b.pt() else { return Ok(()) };
        let ids2 = ids.clone();
        let preview = move |d: &Drawing, p: Pt| -> Vec<PreviewItem> {
            ids2.iter()
                .filter_map(|id| d.by_id(*id))
                .map(|e| {
                    let mut cl = e.cloned_noid();
                    stretch_entity(&mut cl, &rect_for(e, d), p - base, d);
                    pv(cl)
                })
                .collect()
        };
        let s = c
            .point(
                ps("Specify second point or <use first point as displacement>")
                    .base(base)
                    .allow_enter()
                    .preview(preview),
            )
            .await?;
        delta = match s.pt() {
            Some(p) => p - base,
            None => base,
        };
    }
    let news: Vec<(u64, Entity)> = c.doc(|d| {
        ids.iter()
            .filter_map(|id| d.by_id(*id))
            .map(|e| {
                let mut n = e.clone();
                stretch_entity(&mut n, &rect_for(e, d), delta, d);
                (e.id, n)
            })
            .collect()
    });
    c.doc_mut(|d| {
        d.change(|d| {
            for (id, n) in news {
                if let Some(slot) = d.by_id_mut(id) {
                    *slot = n;
                }
            }
        })
    });
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- TRIM / EXTEND

struct Hist {
    idx: usize,
    old: Entity,
    news: Vec<u64>,
}

fn trimmable(e: &Entity) -> bool {
    is_curve_type(e, &["line", "arc", "pline", "circle"])
}

/// Apply a trim/extend to `target_id`; returns whether it changed the drawing.
fn trim_apply(
    c: &Ctx,
    cutters: &[Entity],
    history: &mut Vec<Hist>,
    target_id: u64,
    pick: Pt,
    shift: bool,
    is_trim: bool,
) -> bool {
    let edge_extend = c.app(|a| a.mem.edge_extend);
    let do_trim = is_trim != shift;
    let Some(target) = c.entity_by_id(target_id) else {
        return false;
    };
    let refs: Vec<&Entity> = cutters.iter().collect();
    let res = c.doc(|d| {
        if do_trim {
            trim(&target, pick, &refs, d, edge_extend)
        } else {
            extend(&target, pick, &refs, d, edge_extend)
        }
    });
    match res {
        Err(m) => {
            c.log_error(&m);
            false
        }
        Ok(news) => {
            let Some(idx) = c.doc(|d| d.index_of(target_id)) else {
                return false;
            };
            let ids = c.doc_mut(|d| d.change(|d| d.splice(idx, 1, news)));
            history.push(Hist {
                idx,
                old: target,
                news: ids,
            });
            true
        }
    }
}

async fn fence_or_crossing(
    c: &Ctx,
    mode: &str,
    cutters: &[Entity],
    history: &mut Vec<Hist>,
    is_trim: bool,
) -> CmdResult<()> {
    if mode == "fence" {
        let mut pts: Vec<Pt> = vec![];
        loop {
            let snapshot = pts.clone();
            let p = c
                .point(
                    ps(if pts.is_empty() {
                        "Specify first fence point"
                    } else {
                        "Specify next fence point"
                    })
                    .allow_enter()
                    .base_opt(pts.last().copied())
                    .preview(move |_, q| {
                        if snapshot.is_empty() {
                            vec![]
                        } else {
                            let mut v = snapshot.clone();
                            v.push(q);
                            vec![pv(Entity::pline_pts(&v, false))]
                        }
                    }),
                )
                .await?;
            match p.pt() {
                Some(q) => pts.push(q),
                None => break,
            }
        }
        if pts.len() < 2 {
            return Ok(());
        }
        let edges: Vec<Seg> = pts.windows(2).map(|w| Seg::line(w[0], w[1])).collect();
        let ents = c.doc(|d| d.entities().clone());
        for e in ents {
            let visible = c.doc(|d| d.is_visible(&e));
            if !trimmable(&e) || !visible {
                continue;
            }
            let segs = c.doc(|d| segs_of(&e, d).to_vec());
            'segs: for s in segs {
                for q in &edges {
                    if let Some(hit) = seg_intersect(&s, q, false, false).first() {
                        trim_apply(c, cutters, history, e.id, hit.pt, false, is_trim);
                        break 'segs;
                    }
                }
            }
        }
    } else {
        let Some(a) = c.point(ps("Specify first corner")).await?.pt() else {
            return Ok(());
        };
        let Some(b) = c.corner(a, ps("Specify opposite corner")).await?.pt() else {
            return Ok(());
        };
        let rect = crate::select::rect_of(a, b);
        let centre = rect.center();
        let ids = c.doc(|d| crate::select::select_rect(d, &rect, true, None));
        for id in ids {
            let Some(e) = c.entity_by_id(id) else {
                continue;
            };
            if !trimmable(&e) {
                continue;
            }
            let Some(n) = c.doc(|d| path_of(&e, d).map(|p| path_nearest(&p, centre))) else {
                continue;
            };
            trim_apply(c, cutters, history, id, n.pt, false, is_trim);
        }
    }
    Ok(())
}

async fn trim_extend(c: Ctx, is_trim: bool) -> CmdResult<()> {
    let edge = c.app(|a| a.mem.edge_extend);
    c.log(&format!(
        "Current settings: Projection=UCS, Edge={}",
        if edge { "Extend" } else { "None" }
    ));
    let edges = c
        .select(
            ps(if is_trim {
                "Select cutting edges... Select objects or <select all>"
            } else {
                "Select boundary edges... Select objects or <select all>"
            })
            .pickfirst(false),
        )
        .await?;
    let cutters: Vec<Entity> = if edges.ents.is_empty() {
        c.doc(|d| {
            d.entities()
                .iter()
                .filter(|e| d.is_visible(e))
                .cloned()
                .collect()
        })
    } else {
        c.entities(&edges.ents)
    };
    let mut history: Vec<Hist> = vec![];
    loop {
        let mut opts = vec!["Fence", "Crossing", "Project", "Edge"];
        if is_trim {
            opts.push("eRase");
        }
        opts.push("Undo");
        let r = c
            .entity(
                ps(if is_trim {
                    "Select object to trim or shift-select to extend or"
                } else {
                    "Select object to extend or shift-select to trim or"
                })
                .options(&opts)
                .allow_enter()
                .filter(trimmable),
            )
            .await?;
        match r.kw() {
            Some("Undo") => {
                let Some(h) = history.pop() else { continue };
                c.doc_mut(|d| {
                    d.change(|d| {
                        for id in &h.news {
                            if let Some(i) = d.index_of(*id) {
                                d.splice(i, 1, vec![]);
                            }
                        }
                        let n = d.entities().len();
                        d.splice(h.idx.min(n), 0, vec![h.old.clone()]);
                    })
                });
                continue;
            }
            Some("Edge") => {
                let cur = c.app(|a| a.mem.edge_extend);
                let k = c
                    .keyword(
                        ps("Enter an implied edge extension mode [Extend/No extend]")
                            .options(&["Extend", "No extend"])
                            .default_str(if cur { "Extend" } else { "No extend" }),
                    )
                    .await?;
                c.app(|a| a.mem.edge_extend = k.kw() == Some("Extend"));
                continue;
            }
            Some("Project") => {
                c.log("Projection=None; only UCS plane trimming is supported.");
                continue;
            }
            Some("eRase") => {
                let sel = c
                    .select(ps("Select objects to erase").pickfirst(false))
                    .await?;
                c.doc_mut(|d| d.remove_all(&sel.ents));
                continue;
            }
            Some("Fence") => {
                fence_or_crossing(&c, "fence", &cutters, &mut history, is_trim).await?;
                continue;
            }
            Some("Crossing") => {
                fence_or_crossing(&c, "crossing", &cutters, &mut history, is_trim).await?;
                continue;
            }
            _ => {}
        }
        let EntR::Entity { id, pt: p, shift } = r else {
            break;
        };
        trim_apply(&c, &cutters, &mut history, id, p, shift, is_trim);
    }
    Ok(())
}

// ---------------------------------------------------------------- BREAK / JOIN / LENGTHEN

fn replace_with(c: &Ctx, id: u64, news: Vec<Entity>) {
    if let Some(idx) = c.doc(|d| d.index_of(id)) {
        c.doc_mut(|d| d.change(|d| d.splice(idx, 1, news)));
    }
}

async fn break_(c: Ctx, _a: Args) -> CmdResult<()> {
    let o = c.entity(ps("Select object").filter(is_path_entity)).await?;
    let EntR::Entity { id, pt: p0, .. } = o else {
        return Ok(());
    };
    let Some(target) = c.entity_by_id(id) else {
        return Ok(());
    };
    let mut p1 = p0;
    loop {
        let r = c
            .point(ps("Specify second break point or [First point]").options(&["First point"]))
            .await?;
        if r.kw() == Some("First point") {
            if let Some(f) = c.point(ps("Specify first break point")).await?.pt() {
                p1 = f;
            }
            continue;
        }
        let Some(p2) = r.pt() else { return Ok(()) };
        let res = c.doc(|d| {
            if dist(p2, p1) < 1e-9 {
                split_at(&target, p1, d)
            } else {
                break_at(&target, p1, p2, d)
            }
        });
        match res {
            Err(m) => c.log_error(&m),
            Ok(news) => replace_with(&c, id, news),
        }
        return Ok(());
    }
}

async fn breakatpoint(c: Ctx, _a: Args) -> CmdResult<()> {
    let o = c
        .entity(ps("Select object").filter(|e| {
            is_path_entity(e)
                && !matches!(
                    e.kind,
                    Kind::Circle { .. } | Kind::Pline { closed: true, .. }
                )
        }))
        .await?;
    let EntR::Entity { id, .. } = o else {
        return Ok(());
    };
    let Some(target) = c.entity_by_id(id) else {
        return Ok(());
    };
    let Some(p) = c.point(ps("Specify break point")).await?.pt() else {
        return Ok(());
    };
    match c.doc(|d| split_at(&target, p, d)) {
        Err(m) => c.log_error(&m),
        Ok(news) => replace_with(&c, id, news),
    }
    Ok(())
}

async fn join_(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c
        .select(
            ps("Select source object or multiple objects to join at once").filter(is_path_entity),
        )
        .await?;
    if sel.ents.len() < 2 {
        if !sel.ents.is_empty() {
            c.log_error("1 object cannot be joined by itself.");
        }
        return Ok(());
    }
    let ents = c.entities(&sel.ents);
    let refs: Vec<&Entity> = ents.iter().collect();
    let res = c.doc(|d| join(&refs, d, 1e-6));
    let joined = res.sources.len() as i64 - res.results.len() as i64;
    if res.results.is_empty() {
        return Ok(());
    }
    let used: Vec<u64> = res.used.iter().map(|i| ents[*i].id).collect();
    let n = res.results.len();
    c.doc_mut(|d| {
        d.change(|d| {
            d.remove_all(&used);
            for e in res.results {
                d.insert_raw(e);
            }
        })
    });
    c.log(&format!(
        "{joined} object{} joined to {n} object{}",
        if joined == 1 { "" } else { "s" },
        if n == 1 { "" } else { "s" }
    ));
    c.clear_sel();
    Ok(())
}

async fn lengthen_(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut mode: Option<LengthenMode> = None;
    let mut value = 0.0;
    loop {
        let r = c
            .entity(
                ps("Select an object or [DElta/Percent/Total/DYnamic]")
                    .options(&["DElta", "Percent", "Total", "DYnamic"])
                    .allow_enter()
                    .filter(|e| is_curve_type(e, &["line", "arc", "pline"])),
            )
            .await?;
        if let Some(k) = r.kw() {
            let m = k.to_lowercase();
            if m == "dynamic" {
                c.log("Dynamic lengthening: use grips on the object.");
                continue;
            }
            let (prompt, def_) = match m.as_str() {
                "delta" => ("Enter delta length", 0.0),
                "percent" => ("Enter percentage length", 100.0),
                _ => ("Specify total length", 0.0),
            };
            let v = c.distance(ps(prompt).default_num(def_)).await?;
            let Some(vv) = v.value() else { return Ok(()) };
            mode = Some(match m.as_str() {
                "delta" => LengthenMode::Delta,
                "percent" => LengthenMode::Percent,
                _ => LengthenMode::Total,
            });
            value = vv;
            continue;
        }
        let EntR::Entity { id, pt: p, .. } = r else {
            return Ok(());
        };
        let Some(e) = c.entity_by_id(id) else {
            continue;
        };
        let Some(m) = mode else {
            let l: f64 = c.doc(|d| segs_of(&e, d).iter().map(|q| q.length()).sum());
            c.log(&format!("Current length: {}", num(l, 4)));
            continue;
        };
        match c.doc(|d| lengthen(&e, p, m, value, d)) {
            Err(msg) => c.log_error(&msg),
            Ok(news) => {
                if let Some(n) = news.into_iter().next() {
                    c.doc_mut(|d| d.replace(id, n));
                }
            }
        }
    }
}

// ---------------------------------------------------------------- FILLET / CHAMFER

struct SegPick {
    path: Path,
    seg: Seg,
    index: usize,
}

fn seg_at_pick(e: &Entity, pick: Pt, doc: &Drawing) -> Option<SegPick> {
    let path = path_of(e, doc)?;
    let n = path_nearest(&path, pick);
    let index = ((n.u.floor()) as usize).min(path.segs.len() - 1);
    Some(SegPick {
        seg: path.segs[index],
        path,
        index,
    })
}

/// Replace a line/arc entity's geometry with a segment (in place).
fn replace_seg(d: &mut Drawing, id: u64, seg: &Seg) {
    if let Some(e) = d.by_id_mut(id) {
        match (&mut e.kind, seg) {
            (Kind::Line { a, b }, Seg::L { a: sa, b: sb, .. }) => {
                *a = *sa;
                *b = *sb;
            }
            (Kind::Arc { .. }, s) => {
                if let Kind::Arc {
                    c: nc,
                    r: nr,
                    a0,
                    a1,
                } = Entity::arc_seg(s).kind
                {
                    e.kind = Kind::Arc {
                        c: nc,
                        r: nr,
                        a0,
                        a1,
                    };
                }
            }
            _ => {}
        }
    }
}

async fn fillet(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut trim_mode = c.app(|a| a.mem.fillet_trim);
    let mut multiple = false;
    loop {
        let radius_now = c.doc(|d| d.settings.fillet_radius);
        c.log(&format!(
            "Current settings: Mode = {}, Radius = {}",
            if trim_mode { "TRIM" } else { "NOTRIM" },
            num(radius_now, 4)
        ));
        let a = c
            .entity(
                ps("Select first object or")
                    .options(&["Undo", "Polyline", "Radius", "Trim", "Multiple"])
                    .allow_enter()
                    .filter(is_path_entity),
            )
            .await?;
        match a.kw() {
            Some("Radius") => {
                let r = c
                    .distance(ps("Specify fillet radius").default_num(radius_now).min(0.0))
                    .await?;
                if let Some(v) = r.value() {
                    c.doc_mut(|d| d.settings.fillet_radius = v);
                }
                continue;
            }
            Some("Trim") => {
                let t = c
                    .keyword(
                        ps("Enter Trim mode option [Trim/No trim]")
                            .options(&["Trim", "No trim"])
                            .default_str(if trim_mode { "Trim" } else { "No trim" }),
                    )
                    .await?;
                trim_mode = t.kw() != Some("No trim");
                c.app(|a| a.mem.fillet_trim = trim_mode);
                continue;
            }
            Some("Multiple") => {
                multiple = true;
                continue;
            }
            Some("Undo") => {
                c.log("Nothing to undo.");
                continue;
            }
            Some("Polyline") => {
                let p = c
                    .entity(
                        ps("Select 2D polyline").filter(|e| matches!(e.kind, Kind::Pline { .. })),
                    )
                    .await?;
                let Some(pid) = p.id() else { continue };
                let Some(pe) = c.entity_by_id(pid) else {
                    continue;
                };
                let res = c.doc(|d| fillet_pline(&pe, radius_now, d));
                match res {
                    Some(r) if r.applied > 0 && r.entity.is_some() => {
                        c.doc_mut(|d| d.replace(pid, r.entity.expect("entity")));
                        c.log(&format!("{} were filleted", r.applied));
                        if !multiple {
                            return Ok(());
                        }
                    }
                    _ => c.log_error("No corners could be filleted with that radius."),
                }
                continue;
            }
            _ => {}
        }
        let EntR::Entity {
            id: aid, pt: apt, ..
        } = a
        else {
            return Ok(());
        };
        let b = c
            .entity(
                ps("Select second object or shift-select to apply corner or [Radius]")
                    .options(&["Radius"])
                    .filter(is_path_entity),
            )
            .await?;
        if b.kw() == Some("Radius") {
            let r = c
                .distance(ps("Specify fillet radius").default_num(radius_now).min(0.0))
                .await?;
            if let Some(v) = r.value() {
                c.doc_mut(|d| d.settings.fillet_radius = v);
            }
            continue;
        }
        let EntR::Entity {
            id: bid,
            pt: bpt,
            shift,
            ..
        } = b
        else {
            return Ok(());
        };
        let radius = if shift { 0.0 } else { radius_now };
        let (Some(ae), Some(be)) = (c.entity_by_id(aid), c.entity_by_id(bid)) else {
            continue;
        };
        if aid == bid {
            // two segments of the same polyline
            let (pa, pb) = c.doc(|d| (seg_at_pick(&ae, apt, d), seg_at_pick(&be, bpt, d)));
            let (Some(pa), Some(pb)) = (pa, pb) else {
                continue;
            };
            let n = pa.path.segs.len();
            let closed = matches!(ae.kind, Kind::Pline { closed: true, .. });
            let adjacent = (pa.index as i64 - pb.index as i64).abs() == 1
                || (closed && (pa.index as i64 - pb.index as i64).abs() == n as i64 - 1);
            if !matches!(ae.kind, Kind::Pline { .. }) || !adjacent {
                c.log_error("Select two different objects or adjacent segments.");
                continue;
            }
            let mut segs = pa.path.segs.clone();
            let i = pa.index.min(pb.index);
            let j = (i + 1) % n;
            match fillet_segs(
                &segs[i],
                segs[i].at(0.05),
                &segs[j],
                segs[j].at(0.95),
                radius,
            ) {
                Err(m) => {
                    c.log_error(&m);
                    continue;
                }
                Ok(r) => {
                    if let Some(x) = r.a {
                        segs[i] = x;
                    }
                    if let Some(x) = r.b {
                        segs[j] = x;
                    }
                    let mut out = vec![];
                    for (k, s) in segs.iter().enumerate() {
                        out.push(*s);
                        if k == i {
                            if let Some(arc) = r.insert {
                                out.push(arc);
                            }
                        }
                    }
                    if let Some(ne) = entity_from_segs(&out, &ae, closed) {
                        c.doc_mut(|d| d.replace(aid, ne));
                    }
                }
            }
            if !multiple {
                return Ok(());
            }
            continue;
        }
        let (pa, pb) = c.doc(|d| (seg_at_pick(&ae, apt, d), seg_at_pick(&be, bpt, d)));
        let ok = matches!((&pa, &pb), (Some(_), Some(_)))
            && !matches!(ae.kind, Kind::Pline { .. })
            && !matches!(be.kind, Kind::Pline { .. });
        if !ok {
            c.log_error("Fillet between separate objects requires lines, arcs or circles.");
            continue;
        }
        let (pa, pb) = (pa.expect("a"), pb.expect("b"));
        match fillet_segs(&pa.seg, apt, &pb.seg, bpt, radius) {
            Err(m) => {
                c.log_error(&m);
                continue;
            }
            Ok(r) => {
                c.doc_mut(|d| {
                    d.change(|d| {
                        if trim_mode {
                            if let Some(s) = &r.a {
                                replace_seg(d, aid, s);
                            }
                            if let Some(s) = &r.b {
                                replace_seg(d, bid, s);
                            }
                        }
                        if let Some(arc) = &r.insert {
                            d.add(inherit_props(&ae, Entity::arc_seg(arc)));
                        }
                    })
                });
            }
        }
        if !multiple {
            return Ok(());
        }
    }
}

async fn chamfer(c: Ctx, _a: Args) -> CmdResult<()> {
    let (mut method_angle, mut angle_len, mut angle, mut trim_mode) = (false, 0.0, rad(45.0), true);
    loop {
        let (d1_, d2_) = c.doc(|d| (d.settings.chamfer_d1, d.settings.chamfer_d2));
        c.log(&format!(
            "(TRIM mode) Current chamfer Dist1 = {}, Dist2 = {}",
            num(d1_, 4),
            num(d2_, 4)
        ));
        let a = c
            .entity(
                ps("Select first line or")
                    .options(&[
                        "Undo", "Polyline", "Distance", "Angle", "Trim", "mEthod", "Multiple",
                    ])
                    .allow_enter()
                    .filter(|e| matches!(e.kind, Kind::Line { .. } | Kind::Pline { .. })),
            )
            .await?;
        match a.kw() {
            Some("Distance") => {
                let d1 = c
                    .distance(
                        ps("Specify first chamfer distance")
                            .default_num(d1_)
                            .min(0.0),
                    )
                    .await?;
                let Some(v1) = d1.value() else { continue };
                let d2 = c
                    .distance(
                        ps("Specify second chamfer distance")
                            .default_num(v1)
                            .min(0.0),
                    )
                    .await?;
                let Some(v2) = d2.value() else { continue };
                c.doc_mut(|d| {
                    d.settings.chamfer_d1 = v1;
                    d.settings.chamfer_d2 = v2;
                });
                method_angle = false;
                continue;
            }
            Some("Angle") => {
                let l = c
                    .distance(
                        ps("Specify chamfer length on the first line")
                            .default_num(if angle_len != 0.0 { angle_len } else { 1.0 })
                            .min(0.0),
                    )
                    .await?;
                let an = c
                    .angle(ps("Specify chamfer angle from the first line").default_num(angle))
                    .await?;
                if let (Some(lv), Some(av)) = (l.value(), an.value()) {
                    angle_len = lv;
                    angle = av;
                    method_angle = true;
                }
                continue;
            }
            Some("Trim") => {
                let t = c
                    .keyword(
                        ps("Enter Trim mode option [Trim/No trim]")
                            .options(&["Trim", "No trim"])
                            .default_str("Trim"),
                    )
                    .await?;
                trim_mode = t.kw() != Some("No trim");
                continue;
            }
            Some("mEthod") => {
                let m = c
                    .keyword(
                        ps("Enter trim method [Distance/Angle]")
                            .options(&["Distance", "Angle"])
                            .default_str(if method_angle { "Angle" } else { "Distance" }),
                    )
                    .await?;
                method_angle = m.kw() == Some("Angle");
                continue;
            }
            Some("Polyline") => {
                let p = c
                    .entity(
                        ps("Select 2D polyline").filter(|e| matches!(e.kind, Kind::Pline { .. })),
                    )
                    .await?;
                let Some(pid) = p.id() else { continue };
                let Some(pe) = c.entity_by_id(pid) else {
                    continue;
                };
                match c.doc(|d| chamfer_pline(&pe, d1_, d2_, d)) {
                    Some(r) if r.applied > 0 && r.entity.is_some() => {
                        c.doc_mut(|d| d.replace(pid, r.entity.expect("entity")));
                        c.log(&format!("{} lines were chamfered", r.applied));
                        return Ok(());
                    }
                    _ => c.log_error("No corners could be chamfered with those distances."),
                }
                continue;
            }
            Some("Undo") | Some("Multiple") => continue,
            _ => {}
        }
        let EntR::Entity {
            id: aid, pt: apt, ..
        } = a
        else {
            return Ok(());
        };
        let Some(ae) = c.entity_by_id(aid) else {
            continue;
        };
        if !matches!(ae.kind, Kind::Line { .. }) {
            c.log_error("Select a straight line.");
            continue;
        }
        let b = c
            .entity(
                ps("Select second line or shift-select to apply corner")
                    .filter(|e| matches!(e.kind, Kind::Line { .. })),
            )
            .await?;
        let EntR::Entity {
            id: bid,
            pt: bpt,
            shift,
            ..
        } = b
        else {
            return Ok(());
        };
        let Some(be) = c.entity_by_id(bid) else {
            continue;
        };
        let (sa, sb) = c.doc(|d| (segs_of(&ae, d)[0], segs_of(&be, d)[0]));
        let r = if shift {
            chamfer_segs(&sa, apt, &sb, bpt, 0.0, 0.0)
        } else if method_angle {
            chamfer_angle(&sa, apt, &sb, bpt, angle_len, angle)
        } else {
            chamfer_segs(&sa, apt, &sb, bpt, d1_, d2_)
        };
        match r {
            Err(m) => {
                c.log_error(&m);
                continue;
            }
            Ok(r) => {
                c.doc_mut(|d| {
                    d.change(|d| {
                        if trim_mode {
                            if let Some(s) = &r.a {
                                replace_seg(d, aid, s);
                            }
                            if let Some(s) = &r.b {
                                replace_seg(d, bid, s);
                            }
                        }
                        if let Some(Seg::L { a, b, .. }) = r.insert {
                            d.add(inherit_props(&ae, Entity::line(a, b)));
                        }
                    })
                });
            }
        }
        return Ok(());
    }
}

// ---------------------------------------------------------------- EXPLODE / PEDIT

async fn explode(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c
        .select(ps(SELECT).filter(|e| {
            is_curve_type(e, &["pline", "insert", "dim", "leader"])
                || matches!(&e.kind, Kind::Text(t) if t.multi)
        }))
        .await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let mut count = 0;
    for id in &sel.ents {
        let Some(e) = c.entity_by_id(*id) else {
            continue;
        };
        let parts = c.doc(|d| explode_entity(&e, d));
        let Some(parts) = parts else { continue };
        let made: Vec<Entity> = parts
            .into_iter()
            .map(|mut p| {
                if p.layer.is_empty() {
                    p.layer = e.layer.clone();
                }
                p.id = 0;
                p
            })
            .collect();
        count += made.len();
        replace_with(&c, *id, made);
    }
    c.log(&format!("{count} objects created"));
    c.clear_sel();
    Ok(())
}

fn set_closed(d: &mut Drawing, id: u64, v: bool) {
    if let Some(Entity {
        kind: Kind::Pline { closed, .. },
        ..
    }) = d.by_id_mut(id)
    {
        *closed = v;
    }
}

async fn pedit(c: Ctx, args: Args) -> CmdResult<()> {
    let mut eid = args.id("entity");
    if eid.is_none() {
        let pre = c.ed(|e| {
            e.selected()
                .into_iter()
                .map(|e| (e.id, e.type_name()))
                .collect::<Vec<_>>()
        });
        if pre.len() == 1 && ["pline", "line", "arc"].contains(&pre[0].1) {
            eid = Some(pre[0].0);
        }
    }
    if eid.is_none() {
        let r = c
            .entity(
                ps("Select polyline or [Multiple]")
                    .options(&["Multiple"])
                    .filter(|x| is_curve_type(x, &["pline", "line", "arc"])),
            )
            .await?;
        eid = r.id();
    }
    let Some(mut id) = eid else { return Ok(()) };
    let Some(e) = c.entity_by_id(id) else {
        return Ok(());
    };
    if !matches!(e.kind, Kind::Pline { .. }) {
        let y = c
            .keyword(
                ps("Object selected is not a polyline\nDo you want to turn it into one? <Y>")
                    .options(&["Yes", "No"])
                    .default_str("Yes"),
            )
            .await?;
        if y.kw() == Some("No") {
            return Ok(());
        }
        let segs = c.doc(|d| segs_of(&e, d).to_vec());
        let np = match entity_from_segs(&segs, &e, false) {
            Some(pl) if matches!(pl.kind, Kind::Pline { .. }) => pl,
            _ => Entity::pline(segs_to_verts(&segs, false), false),
        };
        let np = inherit_props(&e, np);
        c.doc_mut(|d| d.replace(id, np));
    }
    loop {
        let Some(cur) = c.entity_by_id(id) else {
            return Ok(());
        };
        let closed = matches!(cur.kind, Kind::Pline { closed: true, .. });
        let opts = [
            if closed { "Open" } else { "Close" },
            "Join",
            "Width",
            "Reverse",
            "Undo",
        ];
        let r = c
            .keyword(ps("Enter an option").options(&opts).allow_enter())
            .await?;
        match r.kw() {
            None => return Ok(()),
            Some("Close") | Some("Open") => {
                let v = r.kw() == Some("Close");
                c.doc_mut(|d| d.change(|d| set_closed(d, id, v)));
            }
            Some("Width") => {
                let w = c
                    .distance(
                        ps("Specify new width for all segments")
                            .default_num(0.0)
                            .min(0.0),
                    )
                    .await?;
                let Some(w) = w.value() else { continue };
                c.doc_mut(|d| {
                    d.change(|d| {
                        if let Some(Entity {
                            kind: Kind::Pline { v, .. },
                            ..
                        }) = d.by_id_mut(id)
                        {
                            for q in v.iter_mut() {
                                q.sw = w;
                                q.ew = w;
                            }
                        }
                    })
                });
            }
            Some("Reverse") => {
                let segs = c.doc(|d| {
                    segs_of(&cur, d)
                        .iter()
                        .rev()
                        .map(|s| s.reverse())
                        .collect::<Vec<_>>()
                });
                let mut verts = match entity_from_segs(&segs, &cur, closed) {
                    Some(Entity {
                        kind: Kind::Pline { v, .. },
                        ..
                    }) => v,
                    _ => segs_to_verts(&segs, closed),
                };
                // the edges come back in reverse order and each one tapers the other way round
                if let Kind::Pline { v: old, .. } = &cur.kind {
                    let widths: Vec<(f64, f64)> = old
                        .iter()
                        .take(if closed { old.len() } else { old.len() - 1 })
                        .rev()
                        .map(|q| (q.ew, q.sw))
                        .collect();
                    for (q, w) in verts.iter_mut().zip(widths) {
                        q.sw = w.0;
                        q.ew = w.1;
                    }
                }
                c.doc_mut(|d| {
                    d.change(|d| {
                        if let Some(Entity {
                            kind: Kind::Pline { v, .. },
                            ..
                        }) = d.by_id_mut(id)
                        {
                            *v = verts;
                        }
                    })
                });
            }
            Some("Join") => {
                let keep = id;
                let more = c
                    .select(
                        ps("Select objects to join")
                            .filter(move |x| is_path_entity(x) && x.id != keep)
                            .pickfirst(false),
                    )
                    .await?;
                let mut all = vec![cur.clone()];
                all.extend(c.entities(&more.ents));
                let refs: Vec<&Entity> = all.iter().collect();
                let res = c.doc(|d| join(&refs, d, 1e-6));
                let used: Vec<u64> = res.used.iter().map(|i| all[*i].id).collect();
                let first = res.results.first().cloned();
                let mut new_id = None;
                c.doc_mut(|d| {
                    d.change(|d| {
                        d.remove_all(&used);
                        for (k, n) in res.results.into_iter().enumerate() {
                            let nid = d.insert_raw(n);
                            if k == 0 {
                                new_id = Some(nid);
                            }
                        }
                    })
                });
                if first.is_some() {
                    id = new_id.unwrap_or(id);
                }
            }
            Some("Undo") => {
                c.doc_mut(|d| d.undo());
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- MATCHPROP

async fn matchprop(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(sid) = c.pick_entity(ps("Select source object")).await? else {
        return Ok(());
    };
    let Some(src) = c.entity_by_id(sid) else {
        return Ok(());
    };
    c.log("Current active settings = Color Layer Ltype Ltscale Lineweight Transparency Thickness Dim Text Hatch Polyline");
    loop {
        let t = c
            .entity(
                ps("Select destination object(s) or [Settings]")
                    .options(&["Settings"])
                    .allow_enter(),
            )
            .await?;
        if t.kw().is_some() {
            c.log("Settings: all basic properties are matched.");
            continue;
        }
        let Some(tid) = t.id() else { return Ok(()) };
        let src2 = src.clone();
        c.doc_mut(|d| {
            d.change(|d| {
                if let Some(dst) = d.by_id_mut(tid) {
                    if !src2.layer.is_empty() {
                        dst.layer = src2.layer.clone();
                    }
                    dst.color = src2.color;
                    dst.ltype = src2.ltype.clone();
                    dst.lw = src2.lw;
                    dst.lts = src2.lts;
                    dst.transp = src2.transp;
                    match (&src2.kind, &mut dst.kind) {
                        (Kind::Text(s), Kind::Text(t)) => {
                            t.h = s.h;
                            t.jh = s.jh;
                        }
                        (Kind::Dim(s), Kind::Dim(t)) => t.ds = s.ds.clone(),
                        (Kind::Hatch(s), Kind::Hatch(t)) => {
                            t.pattern = s.pattern.clone();
                            t.ps = s.ps;
                            t.pa = s.pa;
                        }
                        _ => {}
                    }
                }
            })
        });
    }
}

// ---------------------------------------------------------------- DIVIDE / MEASURE

async fn mark_along(c: Ctx, is_divide: bool) -> CmdResult<()> {
    let Some(oid) = c
        .pick_entity(
            ps(if is_divide {
                "Select object to divide"
            } else {
                "Select object to measure"
            })
            .filter(|e| is_path_entity(e) || is_curve_type(e, &["ellipse", "spline"])),
        )
        .await?
    else {
        return Ok(());
    };
    let Some(o) = c.entity_by_id(oid) else {
        return Ok(());
    };
    let segs: Vec<Seg> = c.doc(|d| {
        path_of(&o, d)
            .map(|p| p.segs)
            .unwrap_or_else(|| segs_of(&o, d).to_vec())
    });
    if segs.is_empty() {
        return Ok(());
    }
    let lengths: Vec<f64> = segs.iter().map(|s| s.length()).collect();
    let total: f64 = lengths.iter().sum();
    let point_at = |d: f64| -> Pt {
        let mut acc = 0.0;
        for (i, s) in segs.iter().enumerate() {
            if d <= acc + lengths[i] + 1e-9 {
                return s.at(if lengths[i] != 0.0 {
                    (d - acc) / lengths[i]
                } else {
                    0.0
                });
            }
            acc += lengths[i];
        }
        segs[segs.len() - 1].end()
    };
    let mut pts = vec![];
    if is_divide {
        let n = c
            .number(
                ps("Enter the number of segments")
                    .integer()
                    .min(2.0)
                    .max(32767.0),
            )
            .await?;
        let Some(nv) = n.value() else { return Ok(()) };
        for i in 1..nv as i64 {
            pts.push(point_at(total * i as f64 / nv));
        }
    } else {
        let l = c
            .distance(ps("Specify length of segment").min(0.0).nonzero())
            .await?;
        let Some(lv) = l.value() else { return Ok(()) };
        let mut d = lv;
        while d < total - 1e-9 {
            pts.push(point_at(d));
            d += lv;
        }
    }
    let n = pts.len();
    c.doc_mut(|d| {
        d.change(|d| {
            for p in pts {
                d.add(Entity::point(p));
            }
        })
    });
    c.log(&format!("{n} points created"));
    Ok(())
}

// ---------------------------------------------------------------- clipboard

async fn copyclip(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps(SELECT)).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let ents = c.entities(&sel.ents);
    let b = c
        .doc(|d| bounds_union(&ents, d))
        .unwrap_or(Bounds::new(0.0, 0.0, 0.0, 0.0));
    let n = ents.len();
    c.app(|a| {
        a.clipboard = Some(Clipboard {
            entities: ents.iter().map(|e| e.cloned_noid()).collect(),
            base: pt(b.min_x, b.min_y),
        });
        a.emit(Event::Clipboard);
    });
    c.log(&format!(
        "{n} object{} copied to the clipboard",
        if n == 1 { "" } else { "s" }
    ));
    c.ed(|e| e.temp_sel.clear());
    Ok(())
}

async fn cutclip(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps(SELECT)).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let ents = c.entities(&sel.ents);
    let b = c
        .doc(|d| bounds_union(&ents, d))
        .unwrap_or(Bounds::new(0.0, 0.0, 0.0, 0.0));
    c.app(|a| {
        a.clipboard = Some(Clipboard {
            entities: ents.iter().map(|e| e.cloned_noid()).collect(),
            base: pt(b.min_x, b.min_y),
        })
    });
    c.doc_mut(|d| d.remove_all(&sel.ents));
    c.clear_sel();
    Ok(())
}

async fn pasteclip(c: Ctx, _a: Args) -> CmdResult<()> {
    let clip = c.app(|a| a.clipboard.clone());
    let Some(clip) = clip.filter(|k| !k.entities.is_empty()) else {
        c.log_error("The clipboard is empty.");
        return Ok(());
    };
    let (ents, base) = (clip.entities.clone(), clip.base);
    let e2 = ents.clone();
    let Some(p) = c
        .point(ps("Specify insertion point").preview(move |_, p| {
            pvs(transformed_clones(
                &e2,
                &mat::translate(p.x - base.x, p.y - base.y),
            ))
        }))
        .await?
        .pt()
    else {
        return Ok(());
    };
    let (made, _) =
        c.doc_mut(|d| add_copies(d, &ents, &[], &mat::translate(p.x - base.x, p.y - base.y)));
    c.ed(|e| {
        e.clear_sel();
        e.select(made);
    });
    Ok(())
}

// ---------------------------------------------------------------- grip editing (internal)

const GRIP_MODES: [&str; 5] = ["STRETCH", "MOVE", "ROTATE", "SCALE", "MIRROR"];

fn next_mode(m: &str) -> &'static str {
    let i = GRIP_MODES.iter().position(|x| *x == m).unwrap_or(0);
    GRIP_MODES[(i + 1) % GRIP_MODES.len()]
}

fn shared_grip(e: &Entity, doc: &Drawing, hot: Pt) -> Option<Grip> {
    let tol = 1e-7 * hot.x.abs().max(1.0);
    grips_of(e, doc).into_iter().find(|q| dist(q.pt, hot) < tol)
}

fn grip_matrix(mode: &str, base: Pt, hot: Pt, pt_: Option<Pt>, value: f64) -> Option<Mat> {
    match mode {
        "MOVE" => pt_.map(|p| mat::translate(p.x - base.x, p.y - base.y)),
        "ROTATE" => Some(mat::rotate(pt_.map_or(value, |p| angle_of(base, p)), base)),
        "SCALE" => {
            let f = match pt_ {
                Some(p) => dist(base, p) / dist(base, hot).max(1e-12),
                None => value,
            };
            let f = f.max(1e-9);
            Some(mat::scale(f, f, base))
        }
        "MIRROR" => pt_.map(|p| mat::mirror(base, p)),
        _ => None,
    }
}

async fn gripedit(c: Ctx, args: Args) -> CmdResult<()> {
    let Some(grip) = args.grip("grip").cloned() else {
        return Ok(());
    };
    let sel_ids = c.ed(|e| e.selected_ids());
    let sels = c.entities(&sel_ids);
    let hot = grip.pt;
    let mut mode = if grip.kind == GripKind::Move {
        "MOVE"
    } else {
        "STRETCH"
    };
    let (mut base, mut copy) = (hot, false);
    loop {
        let opts = ["Base point", "Copy", "Undo", "eXit"];
        let label = format!(
            "** {mode} {}**\n{}",
            if copy { "(multiple) " } else { "" },
            match mode {
                "STRETCH" => "Specify stretch point",
                "MOVE" => "Specify move point",
                "ROTATE" => "Specify rotation angle",
                "SCALE" => "Specify scale factor",
                _ => "Specify second point",
            }
        );
        // preview for the current mode
        let (m_now, base_now, sels2) = (mode, base, sels.clone());
        let preview = move |d: &Drawing, p: Pt| -> Vec<PreviewItem> {
            if m_now == "STRETCH" {
                return sels2
                    .iter()
                    .map(|e| {
                        pv(shared_grip(e, d, hot)
                            .map_or_else(|| e.clone(), |g| grip_edit(e, &g.id, p, d)))
                    })
                    .collect();
            }
            match grip_matrix(m_now, base_now, hot, Some(p), dist(base_now, p)) {
                Some(m) => pvs(transformed_clones(&sels2, &m)),
                None => vec![],
            }
        };
        let (enter, kw, rpt, rval): (bool, Option<String>, Option<Pt>, f64);
        if mode == "ROTATE" {
            let r = c
                .angle(
                    ps(&label)
                        .options(&opts)
                        .base(base)
                        .default_num(0.0)
                        .allow_enter()
                        .preview(preview),
                )
                .await?;
            enter = matches!(r, MeasR::Enter(_));
            kw = r.kw().map(str::to_string);
            rpt = r.pt();
            rval = if let MeasR::Value { value, .. } = r {
                value
            } else {
                0.0
            };
        } else if mode == "SCALE" {
            let r = c
                .distance(
                    ps(&label)
                        .options(&opts)
                        .base(base)
                        .default_num(1.0)
                        .allow_enter()
                        .preview(preview),
                )
                .await?;
            enter = matches!(r, MeasR::Enter(_));
            kw = r.kw().map(str::to_string);
            rpt = r.pt();
            rval = if let MeasR::Value { value, .. } = r {
                value
            } else {
                1.0
            };
        } else {
            let b = if mode == "MOVE" || mode == "MIRROR" {
                base
            } else {
                hot
            };
            let r = c
                .point(
                    ps(&label)
                        .options(&opts)
                        .base(b)
                        .allow_enter()
                        .preview(preview),
                )
                .await?;
            enter = matches!(r, PointR::Enter(_));
            kw = r.kw().map(str::to_string);
            rpt = if let PointR::Pt(p) = r { Some(p) } else { None };
            rval = 0.0;
        }
        match kw.as_deref() {
            Some("eXit") => return Ok(()),
            Some("Copy") => {
                copy = true;
                continue;
            }
            Some("Base point") => {
                if let Some(b) = c.point(ps("Specify base point")).await?.pt() {
                    base = b;
                }
                continue;
            }
            Some("Undo") => continue,
            _ => {}
        }
        if enter {
            mode = next_mode(mode);
            continue;
        }
        // apply
        if mode == "STRETCH" {
            let Some(p) = rpt else { continue };
            let news: Vec<(u64, Entity)> = c.doc(|d| {
                sels.iter()
                    .filter_map(|e| {
                        let cur = d.by_id(e.id).unwrap_or(e);
                        shared_grip(cur, d, hot).map(|g| (e.id, grip_edit(cur, &g.id, p, d)))
                    })
                    .collect()
            });
            c.doc_mut(|d| {
                d.change(|d| {
                    for (id, ne) in news {
                        if copy {
                            d.add(ne.cloned_noid());
                        } else if let Some(slot) = d.by_id_mut(id) {
                            let keep = slot.id;
                            *slot = ne;
                            slot.id = keep;
                        }
                    }
                })
            });
        } else if let Some(m) = grip_matrix(mode, base, hot, rpt, rval) {
            if copy {
                c.doc_mut(|d| add_copies(d, &sels, &[], &m));
            } else {
                c.doc_mut(|d| apply_transform(d, &sel_ids, &[], &m));
            }
        }
        if !copy {
            return Ok(());
        }
    }
}

pub fn register(r: &mut Registry) {
    r.define(
        dm("ERASE", run(erase))
            .aliases(&["E", "DEL"])
            .title("Erase")
            .icon("erase")
            .desc("Removes objects from the drawing."),
    );
    r.define(
        dm("OOPS", run(oops))
            .title("Oops")
            .desc("Restores objects deleted by the last ERASE command."),
    );
    r.define(
        dm("MOVE", run(move_))
            .aliases(&["M"])
            .title("Move")
            .icon("move")
            .desc("Moves objects a specified distance in a specified direction."),
    );
    r.define(
        dm("COPY", run(copy))
            .aliases(&["CO", "CP"])
            .title("Copy")
            .icon("copy")
            .desc("Copies objects a specified distance in a specified direction."),
    );
    r.define(
        dm("ROTATE", run(rotate))
            .aliases(&["RO"])
            .title("Rotate")
            .icon("rotate")
            .desc("Rotates objects around a base point."),
    );
    r.define(
        dm("SCALE", run(scale))
            .aliases(&["SC"])
            .title("Scale")
            .icon("scale")
            .desc("Enlarges or reduces selected objects, keeping the proportions the same."),
    );
    r.define(
        dm("MIRROR", run(mirror))
            .aliases(&["MI"])
            .title("Mirror")
            .icon("mirror")
            .desc("Creates a mirrored copy of selected objects."),
    );
    r.define(
        dm("OFFSET", run(offset))
            .aliases(&["O"])
            .title("Offset")
            .icon("offset")
            .desc("Creates concentric circles, parallel lines, and parallel curves.")
            .no_pickfirst(),
    );
    let arr = |kind: Option<&'static str>| run(move |c, _a| array_command(c, kind));
    r.define(
        dm("ARRAY", arr(None))
            .aliases(&["AR"])
            .title("Array")
            .icon("arrayrect")
            .desc("Creates copies of objects arranged in a pattern."),
    );
    r.define(dm("ARRAYRECT", arr(Some("rect"))).title("Rectangular Array").icon("arrayrect").desc("Distributes copies of the selected object into any combination of rows and columns."));
    r.define(dm("ARRAYPOLAR", arr(Some("polar"))).title("Polar Array").icon("arraypolar").desc("Evenly distributes copies of the selected object in a circular pattern around a center point."));
    r.define(
        dm("STRETCH", run(stretch))
            .aliases(&["S"])
            .title("Stretch")
            .icon("stretch")
            .desc("Stretches objects crossed by a selection window or polygon."),
    );
    r.define(
        dm("TRIM", run(|c, _a| trim_extend(c, true)))
            .aliases(&["TR"])
            .title("Trim")
            .icon("trim")
            .desc("Trims objects to meet the edges of other objects.")
            .no_pickfirst(),
    );
    r.define(
        dm("EXTEND", run(|c, _a| trim_extend(c, false)))
            .aliases(&["EX"])
            .title("Extend")
            .icon("extend")
            .desc("Extends objects to meet the edges of other objects.")
            .no_pickfirst(),
    );
    r.define(
        dm("BREAK", run(break_))
            .aliases(&["BR"])
            .title("Break")
            .icon("break")
            .desc("Breaks the selected object between two points.")
            .no_pickfirst(),
    );
    r.define(
        dm("BREAKATPOINT", run(breakatpoint))
            .title("Break at Point")
            .icon("breakpoint")
            .desc("Breaks the selected object at a single point.")
            .no_pickfirst(),
    );
    r.define(
        dm("JOIN", run(join_))
            .aliases(&["J"])
            .title("Join")
            .icon("join")
            .desc("Joins the endpoints of linear and curved objects to create a single object."),
    );
    r.define(
        dm("LENGTHEN", run(lengthen_))
            .aliases(&["LEN"])
            .title("Lengthen")
            .icon("lengthen")
            .desc("Changes the length of objects and the included angle of arcs.")
            .no_pickfirst(),
    );
    r.define(
        dm("FILLET", run(fillet))
            .aliases(&["F"])
            .title("Fillet")
            .icon("fillet")
            .desc("Rounds and fillets the edges of objects.")
            .no_pickfirst(),
    );
    r.define(
        dm("CHAMFER", run(chamfer))
            .aliases(&["CHA"])
            .title("Chamfer")
            .icon("chamfer")
            .desc("Bevels the edges of objects.")
            .no_pickfirst(),
    );
    r.define(
        dm("EXPLODE", run(explode))
            .aliases(&["X"])
            .title("Explode")
            .icon("explode")
            .desc("Breaks a compound object into its component objects."),
    );
    r.define(
        dm("PEDIT", run(pedit))
            .aliases(&["PE"])
            .title("Edit Polyline")
            .icon("pedit")
            .desc("Edits polylines and 3D polygon meshes."),
    );
    r.define(
        dm("MATCHPROP", run(matchprop))
            .aliases(&["MA"])
            .title("Match Properties")
            .icon("matchprop")
            .desc("Applies the properties of a selected object to other objects.")
            .no_pickfirst(),
    );
    r.define(dm("DIVIDE", run(|c, _a| mark_along(c, true))).aliases(&["DIV"]).title("Divide").icon("divide").desc("Creates evenly spaced point objects or blocks along the length or perimeter of an object.").no_pickfirst());
    r.define(dm("MEASURE", run(|c, _a| mark_along(c, false))).aliases(&["ME"]).title("Measure").icon("measure").desc("Creates point objects or blocks at measured intervals along the length or perimeter of an object.").no_pickfirst());
    r.define(
        dm("COPYCLIP", run(copyclip))
            .title("Copy")
            .icon("copyclip")
            .desc("Copies selected objects to the clipboard.")
            .once(),
    );
    r.define(
        dm("CUTCLIP", run(cutclip))
            .title("Cut")
            .icon("cutclip")
            .desc("Copies selected objects to the clipboard and removes them from the drawing.")
            .once(),
    );
    r.define(
        dm("PASTECLIP", run(pasteclip))
            .title("Paste")
            .icon("paste")
            .desc("Inserts data from the clipboard into the drawing.")
            .once(),
    );
    r.define(def("Internal", "_GRIPEDIT", run(gripedit)).once().silent());
    let _ = Rc::new(());
}
