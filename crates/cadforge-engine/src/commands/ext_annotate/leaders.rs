//! Multileader commands: MLEADERSTYLE and MLEADEREDIT.
use super::{group_id, plural, ps};
use crate::commands::helpers::pv;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::MLeaderStyle;

async fn mleaderstyle(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.doc(|d| d.settings.cur_mleader());
    let name = loop {
        let n = c.string(ps("Enter multileader style name or [?]").default_str(&cur.name)).await?.text;
        if n != "?" {
            break n;
        }
        let names = c.doc(|d| d.settings.mleader_styles.keys().cloned().collect::<Vec<_>>());
        c.log(&format!("Multileader styles: {}", if names.is_empty() { "Standard".into() } else { names.join(", ") }));
    };
    if name.is_empty() {
        return Ok(());
    }
    let old = c.doc(|d| d.settings.mleader_styles.get(&name).cloned()).unwrap_or(MLeaderStyle { name: name.clone(), ..cur });
    let Some(text_height) = c.distance(ps("Specify text height").default_num(old.text_height).min(0.0).nonzero()).await?.value() else { return Ok(()) };
    let Some(arrow) = c.distance(ps("Specify arrowhead size").default_num(old.arrow).min(0.0).nonzero()).await?.value() else { return Ok(()) };
    let Some(landing) = c.distance(ps("Specify landing distance").default_num(old.landing).min(0.0)).await?.value() else { return Ok(()) };
    c.doc_mut(|d| {
        d.change(|d| {
            d.settings.mleader_styles.insert(name.clone(), MLeaderStyle { name: name.clone(), text_height, arrow, landing });
            d.settings.cur_mleader_style = name.clone();
        })
    });
    c.log(&format!("Multileader style \"{name}\" is current."));
    Ok(())
}

/// The leader lines that make up one multileader: the objects sharing the group of leader `id`.
fn leader_group(c: &Ctx, id: u64) -> Vec<Entity> {
    c.doc(|d| match d.by_id(id).and_then(|e| e.grp.clone()) {
        Some(g) => d.entities().iter().filter(|e| e.grp.as_ref() == Some(&g) && matches!(e.kind, Kind::Leader { .. })).cloned().collect(),
        None => d.by_id(id).cloned().into_iter().collect(),
    })
}

/// MLEADEREDIT; AIMLEADEREDITADD and AIMLEADEREDITREMOVE run it with `option` Add or Remove (the ribbon's Add Leader
/// and Remove Leader).
async fn mleaderedit(c: Ctx, a: Args) -> CmdResult<()> {
    let only = a.str("option").map(str::to_ascii_lowercase);
    let Some(id) = c.entity(ps("Select a multileader").filter(|e| matches!(e.kind, Kind::Leader { .. }))).await?.id() else { return Ok(()) };
    let members = leader_group(&c, id);
    // the landing end of the leader that carries the text
    let holder = members.iter().find(|e| matches!(&e.kind, Kind::Leader { s, .. } if !s.is_empty())).or(members.first()).cloned();
    let Some(Entity { kind: Kind::Leader { pts, h, .. }, .. }) = holder.clone() else { return Ok(()) };
    let end = *pts.last().unwrap_or(&Pt::ZERO);
    let (remove, arrow) = if only.as_deref() == Some("remove") {
        (true, None)
    } else {
        let spec = ps("Specify leader arrowhead location").base(end).no_ortho().preview(move |_, p| vec![pv(Entity::leader(vec![p, end], "", h))]);
        let reply = c.point(if only.is_some() { spec } else { spec.options(&["Remove leader"]) }).await?;
        (reply.kw() == Some("Remove leader"), reply.pt())
    };
    if remove {
        if members.len() < 2 {
            c.log_error("A multileader needs at least one leader line.");
            return Ok(());
        }
        let ids: Vec<u64> = members.iter().map(|e| e.id).collect();
        let Some(gone) = c.entity(ps("Select leader to remove").filter(move |e| ids.contains(&e.id))).await?.id() else { return Ok(()) };
        // the text moves to a remaining leader when its own leader is removed
        let keep = members.iter().find(|e| e.id != gone).map(|e| e.id);
        let text = holder.filter(|hd| hd.id == gone).and_then(|hd| match hd.kind {
            Kind::Leader { s, h, .. } => Some((s, h)),
            _ => None,
        });
        c.doc_mut(|d| {
            d.change(|d| {
                if let (Some((txt, th)), Some(Entity { kind: Kind::Leader { s, h, .. }, .. })) = (text, keep.and_then(|k| d.by_id_mut(k))) {
                    (*s, *h) = (txt, th);
                }
                d.remove(gone);
            })
        });
        return Ok(());
    }
    let Some(arrow) = arrow else { return Ok(()) };
    // the new leader line joins the multileader's group
    let grp = match members.first().and_then(|e| e.grp.clone()) {
        Some(g) => g,
        None => {
            let g = group_id(&c);
            c.doc_mut(|d| d.modify(id, |e| e.grp = Some(g.clone())));
            g
        }
    };
    let mut line = Entity::leader(vec![arrow, end], "", h);
    line.grp = Some(grp);
    c.add(line);
    Ok(())
}

/// The multileaders (groups of leader lines) among the selected objects: one representative id each.
fn multileaders(c: &Ctx, ids: &[u64]) -> Vec<u64> {
    let mut seen: Vec<Option<String>> = vec![];
    let mut out = vec![];
    for id in ids {
        let Some(e) = c.entity_by_id(*id) else { continue };
        if !matches!(e.kind, Kind::Leader { .. }) {
            continue;
        }
        if e.grp.is_some() && seen.contains(&e.grp) {
            continue;
        }
        seen.push(e.grp.clone());
        out.push(*id);
    }
    out
}

/// MLEADERALIGN: line the selected multileaders' text up with one of them (same landing X, so the notes form a
/// column); their arrowheads stay where they point.
async fn mleaderalign(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select multileaders").filter(|e| matches!(e.kind, Kind::Leader { .. }))).await?;
    let picked = multileaders(&c, &sel.ents);
    if picked.len() < 2 {
        c.log_error("Select at least two multileaders.");
        return Ok(());
    }
    let ids = sel.ents.clone();
    let Some(target) = c.entity(ps("Select multileader to align to").filter(move |e| ids.contains(&e.id))).await?.id() else { return Ok(()) };
    let landing = |id: u64| leader_group(&c, id).iter().find_map(|e| if let Kind::Leader { pts, .. } = &e.kind { pts.last().copied() } else { None });
    let Some(x) = landing(target).map(|p| p.x) else { return Ok(()) };
    let tgrp = c.entity_by_id(target).and_then(|e| e.grp);
    let mut n = 0;
    for id in picked {
        let members = leader_group(&c, id);
        if members.iter().any(|m| m.id == target || (tgrp.is_some() && m.grp == tgrp)) {
            continue;
        }
        let Some(old) = landing(id) else { continue };
        let dx = x - old.x;
        if dx.abs() < 1e-12 {
            continue;
        }
        c.doc_mut(|d| {
            for m in &members {
                d.modify(m.id, |e| {
                    if let Kind::Leader { pts, .. } = &mut e.kind {
                        // the landing (last point) moves; a two-point leader gets its elbow moved with it
                        if let Some(last) = pts.last_mut() {
                            last.x += dx;
                        }
                        if pts.len() > 2 {
                            let k = pts.len() - 2;
                            pts[k].x += dx;
                        }
                    }
                });
            }
        });
        n += 1;
    }
    c.log(&format!("{} aligned.", plural(n, "multileader")));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Annotation", "AIMLEADEREDITADD", run(|c, a: Args| mleaderedit(c, a.with("option", ArgVal::Str("Add".into()))))).no_pickfirst().title("Add Leader").icon("mleader").desc("Adds a leader line to a multileader."));
    r.define(def("Annotation", "AIMLEADEREDITREMOVE", run(|c, a: Args| mleaderedit(c, a.with("option", ArgVal::Str("Remove".into()))))).no_pickfirst().title("Remove Leader").icon("mleader").desc("Removes a leader line from a multileader."));
    r.define(def("Annotation", "MLEADERALIGN", run(mleaderalign)).aliases(&["MLA"]).no_pickfirst().title("Align").icon("align").desc("Aligns the text of selected multileaders with one of them."));
    r.define(def("Annotation", "MLEADERSTYLE", run(mleaderstyle)).aliases(&["MLS"]).no_pickfirst().title("Multileader Style").desc("Creates and modifies multileader styles."));
    r.define(def("Annotation", "MLEADEREDIT", run(mleaderedit)).no_pickfirst().title("Edit Multileader").desc("Adds leader lines to, or removes them from, a multileader."));
}
