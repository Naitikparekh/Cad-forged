//! Clipboard variants: COPYBASE, PASTEBLOCK, PASTEORIG, COPYHIST.
use super::{plural, ps, ui_action};
use crate::app::{Clipboard, Event};
use crate::commands::helpers::{add_copies, pvs, transformed_clones};
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::Entity;
use cadforge_core::geom::mat;

async fn copybase(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(base) = c.point(ps("Specify base point")).await?.pt() else {
        return Ok(());
    };
    let sel = c.select(ps("Select objects")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let ents: Vec<Entity> = c
        .entities(&sel.ents)
        .iter()
        .map(Entity::cloned_noid)
        .collect();
    let n = ents.len();
    c.app(|a| {
        a.clipboard = Some(Clipboard {
            entities: ents,
            base,
        });
        a.emit(Event::Clipboard);
    });
    c.log(&format!("{} copied to the clipboard", plural(n, "object")));
    c.ed(|e| e.temp_sel.clear());
    Ok(())
}

/// The clipboard contents; logs an error and returns `None` when it is empty.
fn clipboard(c: &Ctx) -> Option<Clipboard> {
    let clip = c
        .app(|a| a.clipboard.clone())
        .filter(|k| !k.entities.is_empty());
    if clip.is_none() {
        c.log_error("The clipboard is empty.");
    }
    clip
}

async fn pasteblock(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(Clipboard { entities, base }) = clipboard(&c) else {
        return Ok(());
    };
    let e2 = entities.clone();
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
    let (name, id) = c.doc_mut(|d| {
        let mut n = d.blocks.len();
        let name = loop {
            n += 1;
            let name = format!("A$C{n:08X}");
            if !d.blocks.contains_key(&name) {
                break name;
            }
        };
        let inner = entities
            .iter()
            .map(|e| {
                let mut k = e.cloned_noid();
                k.grp = None;
                k
            })
            .collect();
        d.define_block(&name, base, inner);
        let id = d.add(Entity::insert(&name, p));
        (name, id)
    });
    c.ed(|e| {
        e.clear_sel();
        e.select([id]);
    });
    c.log(&format!(
        "Block \"{name}\" defined with {}.",
        plural(entities.len(), "object")
    ));
    Ok(())
}

async fn pasteorig(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(clip) = clipboard(&c) else {
        return Ok(());
    };
    let (made, _) = c.doc_mut(|d| add_copies(d, &clip.entities, &[], &mat::IDENTITY));
    c.log(&format!(
        "{} pasted at the original coordinates.",
        plural(made.len(), "object")
    ));
    c.ed(|e| {
        e.clear_sel();
        e.select(made);
    });
    Ok(())
}

async fn copyhist(c: Ctx, _a: Args) -> CmdResult<()> {
    let lines: Vec<String> = c.app(|a| a.history.iter().map(|l| l.text.clone()).collect());
    ui_action(
        &c,
        "COPYHIST",
        "COPYHIST",
        Args::none().with("lines", ArgVal::Lines(lines)),
    );
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(
        def("Utilities", "COPYBASE", run(copybase))
            .title("Copy with Base Point")
            .icon("copyclip")
            .desc("Copies objects to the clipboard along with a specified base point.")
            .once(),
    );
    r.define(
        def("Utilities", "PASTEBLOCK", run(pasteblock))
            .title("Paste as Block")
            .icon("paste")
            .desc("Pastes copied objects as a block.")
            .once(),
    );
    r.define(
        def("Utilities", "PASTEORIG", run(pasteorig))
            .title("Paste to Original Coordinates")
            .icon("paste")
            .desc("Pastes copied objects at the coordinates they were copied from.")
            .once(),
    );
    r.define(
        def("Utilities", "COPYHIST", run(copyhist))
            .title("Copy History")
            .desc("Copies the text in the command line history to the clipboard.")
            .once(),
    );
}
