//! Multileader commands: MLEADERSTYLE and MLEADEREDIT.
use super::{group_id, ps};
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
        let n = c
            .string(ps("Enter multileader style name or [?]").default_str(&cur.name))
            .await?
            .text;
        if n != "?" {
            break n;
        }
        let names = c.doc(|d| {
            d.settings
                .mleader_styles
                .keys()
                .cloned()
                .collect::<Vec<_>>()
        });
        c.log(&format!(
            "Multileader styles: {}",
            if names.is_empty() {
                "Standard".into()
            } else {
                names.join(", ")
            }
        ));
    };
    if name.is_empty() {
        return Ok(());
    }
    let old = c
        .doc(|d| d.settings.mleader_styles.get(&name).cloned())
        .unwrap_or(MLeaderStyle {
            name: name.clone(),
            ..cur
        });
    let Some(text_height) = c
        .distance(
            ps("Specify text height")
                .default_num(old.text_height)
                .min(0.0)
                .nonzero(),
        )
        .await?
        .value()
    else {
        return Ok(());
    };
    let Some(arrow) = c
        .distance(
            ps("Specify arrowhead size")
                .default_num(old.arrow)
                .min(0.0)
                .nonzero(),
        )
        .await?
        .value()
    else {
        return Ok(());
    };
    let Some(landing) = c
        .distance(
            ps("Specify landing distance")
                .default_num(old.landing)
                .min(0.0),
        )
        .await?
        .value()
    else {
        return Ok(());
    };
    c.doc_mut(|d| {
        d.change(|d| {
            d.settings.mleader_styles.insert(
                name.clone(),
                MLeaderStyle {
                    name: name.clone(),
                    text_height,
                    arrow,
                    landing,
                },
            );
            d.settings.cur_mleader_style = name.clone();
        })
    });
    c.log(&format!("Multileader style \"{name}\" is current."));
    Ok(())
}

/// The leader lines that make up one multileader: the objects sharing the group of leader `id`.
fn leader_group(c: &Ctx, id: u64) -> Vec<Entity> {
    c.doc(|d| match d.by_id(id).and_then(|e| e.grp.clone()) {
        Some(g) => d
            .entities()
            .iter()
            .filter(|e| e.grp.as_ref() == Some(&g) && matches!(e.kind, Kind::Leader { .. }))
            .cloned()
            .collect(),
        None => d.by_id(id).cloned().into_iter().collect(),
    })
}

async fn mleaderedit(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(id) = c
        .entity(ps("Select a multileader").filter(|e| matches!(e.kind, Kind::Leader { .. })))
        .await?
        .id()
    else {
        return Ok(());
    };
    let members = leader_group(&c, id);
    // the landing end of the leader that carries the text
    let holder = members
        .iter()
        .find(|e| matches!(&e.kind, Kind::Leader { s, .. } if !s.is_empty()))
        .or(members.first())
        .cloned();
    let Some(Entity {
        kind: Kind::Leader { pts, h, .. },
        ..
    }) = holder.clone()
    else {
        return Ok(());
    };
    let end = *pts.last().unwrap_or(&Pt::ZERO);
    let reply = c
        .point(
            ps("Specify leader arrowhead location or")
                .options(&["Remove leader"])
                .base(end)
                .no_ortho()
                .preview(move |_, p| vec![pv(Entity::leader(vec![p, end], "", h))]),
        )
        .await?;
    if reply.kw() == Some("Remove leader") {
        if members.len() < 2 {
            c.log_error("A multileader needs at least one leader line.");
            return Ok(());
        }
        let ids: Vec<u64> = members.iter().map(|e| e.id).collect();
        let Some(gone) = c
            .entity(ps("Select leader to remove").filter(move |e| ids.contains(&e.id)))
            .await?
            .id()
        else {
            return Ok(());
        };
        // the text moves to a remaining leader when its own leader is removed
        let keep = members.iter().find(|e| e.id != gone).map(|e| e.id);
        let text = holder
            .filter(|hd| hd.id == gone)
            .and_then(|hd| match hd.kind {
                Kind::Leader { s, h, .. } => Some((s, h)),
                _ => None,
            });
        c.doc_mut(|d| {
            d.change(|d| {
                if let (
                    Some((txt, th)),
                    Some(Entity {
                        kind: Kind::Leader { s, h, .. },
                        ..
                    }),
                ) = (text, keep.and_then(|k| d.by_id_mut(k)))
                {
                    (*s, *h) = (txt, th);
                }
                d.remove(gone);
            })
        });
        return Ok(());
    }
    let Some(arrow) = reply.pt() else {
        return Ok(());
    };
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

pub fn register(r: &mut Registry) {
    r.define(
        def("Annotation", "MLEADERSTYLE", run(mleaderstyle))
            .aliases(&["MLS"])
            .no_pickfirst()
            .title("Multileader Style")
            .desc("Creates and modifies multileader styles."),
    );
    r.define(
        def("Annotation", "MLEADEREDIT", run(mleaderedit))
            .no_pickfirst()
            .title("Edit Multileader")
            .desc("Adds leader lines to, or removes them from, a multileader."),
    );
}
