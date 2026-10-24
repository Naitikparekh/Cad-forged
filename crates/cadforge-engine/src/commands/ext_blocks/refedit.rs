//! In-place editing of block references: REFEDIT copies the objects of a block definition into the drawing at the
//! reference's place, the usual commands edit them there, and REFCLOSE writes them back into the definition. REFSET
//! moves objects in and out of that working set. XOPEN opens the drawing an external reference points to.
use super::{block_key, db, ps};
use crate::commands::helpers::transformed_clones;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// One reference being edited in place.
struct Session {
    editor: u64,
    block: String,
    /// The reference as it was, and its position in the draw order.
    insert: Entity,
    index: usize,
    /// Working set: the copies of the block's objects (plus anything added by REFSET).
    ids: Vec<u64>,
    /// Block coordinates → drawing coordinates.
    matrix: Mat,
}

type Sessions = Rc<RefCell<Vec<Session>>>;

/// Drop sessions of editors that were closed.
fn live(c: &Ctx, sessions: &Sessions) {
    let open: HashSet<u64> = c.app(|a| a.editors.iter().map(|e| e.id).collect());
    sessions.borrow_mut().retain(|s| open.contains(&s.editor));
}

async fn refedit(c: Ctx, _a: Args, sessions: Sessions) -> CmdResult<()> {
    live(&c, &sessions);
    let here = c.ed(|e| e.id);
    if sessions.borrow().iter().any(|s| s.editor == here) {
        c.log_error("A reference is already being edited. Use REFCLOSE to save or discard its changes first.");
        return Ok(());
    }
    let id = match c.ed(|e| match e.selected().as_slice() {
        [x] if matches!(x.kind, Kind::Insert(_)) => Some(x.id),
        _ => None,
    }) {
        Some(id) => id,
        None => match c
            .pick_entity(ps("Select reference").filter(|e| matches!(e.kind, Kind::Insert(_))))
            .await?
        {
            Some(id) => id,
            None => return Ok(()),
        },
    };
    let Some(insert) = c.entity_by_id(id) else {
        return Ok(());
    };
    let Kind::Insert(ins) = &insert.kind else {
        c.log_error("Select a block reference.");
        return Ok(());
    };
    let Some(key) = block_key(&c, &ins.name) else {
        c.log_error(&format!("Block \"{}\" is not defined.", ins.name));
        return Ok(());
    };
    let block = c.doc(|d| d.blocks[&key].clone());
    if block.xref.is_some() {
        c.log_error(&format!(
            "\"{key}\" is an external reference; open its drawing with XOPEN to edit it."
        ));
        return Ok(());
    }
    let matrix = insert_matrix(ins, block.base);
    let copies = transformed_clones(&block.entities, &matrix);
    let (ids, index) = c.doc_mut(|d| {
        let index = d.entities().iter().position(|e| e.id == id).unwrap_or(0);
        let mut ids = vec![];
        d.change(|d| {
            d.remove(id);
            ids = d.splice(index, 0, copies);
        });
        (ids, index)
    });
    let n = ids.len();
    c.ed(|e| {
        e.sel.clear();
        e.select(ids.iter().copied());
    });
    sessions.borrow_mut().push(Session {
        editor: here,
        block: key.clone(),
        insert,
        index,
        ids,
        matrix,
    });
    c.log(&format!("Editing reference \"{key}\": {n} object{} in the working set. Edit them, then use REFCLOSE.", if n == 1 { "" } else { "s" }));
    c.emit();
    Ok(())
}

fn session_index(c: &Ctx, sessions: &Sessions) -> Option<usize> {
    let here = c.ed(|e| e.id);
    sessions.borrow().iter().position(|s| s.editor == here)
}

async fn refclose(c: Ctx, _a: Args, sessions: Sessions) -> CmdResult<()> {
    live(&c, &sessions);
    let Some(si) = session_index(&c, &sessions) else {
        c.log_error("No reference is being edited. Start with REFEDIT.");
        return Ok(());
    };
    // undoing back past REFEDIT restores the reference itself: the session is obsolete
    let stale = c.doc(|d| d.by_id(sessions.borrow()[si].insert.id).is_some());
    if stale {
        sessions.borrow_mut().remove(si);
        c.log("The reference was restored by UNDO; the edit session was closed.");
        return Ok(());
    }
    let save = c
        .keyword(
            ps("Enter option [Save/Discard]")
                .options(&["Save", "Discard"])
                .default_str("Save"),
        )
        .await?
        .kw()
        != Some("Discard");
    let s = sessions.borrow_mut().remove(si);
    let present: Vec<u64> = c.doc(|d| {
        s.ids
            .iter()
            .copied()
            .filter(|i| d.by_id(*i).is_some())
            .collect()
    });
    let working: Vec<Entity> = c.entities(&present);
    let inverse = mat::invert(&s.matrix);
    c.doc_mut(|d| {
        d.change(|d| {
            d.remove_all(&present);
            if save {
                let base = d.blocks.get(&s.block).map_or(Pt::ZERO, |b| b.base);
                d.define_block(&s.block, base, transformed_clones(&working, &inverse));
            }
            let at = s.index.min(d.entities().len());
            let mut back = s.insert.clone();
            back.id = 0;
            d.splice(at, 0, vec![back]);
        })
    });
    c.ed(|e| e.sel.clear());
    c.log(&if save {
        format!(
            "Block \"{}\" redefined from {} object{}.",
            s.block,
            working.len(),
            if working.len() == 1 { "" } else { "s" }
        )
    } else {
        "Changes discarded.".to_string()
    });
    c.emit();
    Ok(())
}

async fn refset(c: Ctx, _a: Args, sessions: Sessions) -> CmdResult<()> {
    live(&c, &sessions);
    let Some(si) = session_index(&c, &sessions) else {
        c.log_error("No reference is being edited. Start with REFEDIT.");
        return Ok(());
    };
    let k = c
        .keyword(
            ps("Enter an option [Add/Remove]")
                .options(&["Add", "Remove"])
                .default_str("Add"),
        )
        .await?;
    let add = k.kw() != Some("Remove");
    let sel = c
        .select(ps(if add {
            "Select objects to add to the working set"
        } else {
            "Select objects to remove from the working set"
        }))
        .await?;
    let mut n = 0;
    {
        let mut all = sessions.borrow_mut();
        let ids = &mut all[si].ids;
        for id in sel.ents {
            let has = ids.contains(&id);
            if add && !has {
                ids.push(id);
                n += 1;
            } else if !add && has {
                ids.retain(|x| *x != id);
                n += 1;
            }
        }
    }
    c.log(&format!(
        "{n} object{} {}.",
        if n == 1 { "" } else { "s" },
        if add {
            "added to the working set"
        } else {
            "removed from the working set"
        }
    ));
    Ok(())
}

/// XOPEN: show the drawing an external reference was taken from (a copy of its contents when it is no longer open).
async fn xopen(c: Ctx, _a: Args) -> CmdResult<()> {
    let id = match c.ed(|e| match e.selected().as_slice() {
        [x] if matches!(x.kind, Kind::Insert(_)) => Some(x.id),
        _ => None,
    }) {
        Some(id) => id,
        None => match c
            .pick_entity(ps("Select xref").filter(|e| matches!(e.kind, Kind::Insert(_))))
            .await?
        {
            Some(id) => id,
            None => return Ok(()),
        },
    };
    let Some(Entity {
        kind: Kind::Insert(ins),
        ..
    }) = c.entity_by_id(id)
    else {
        return Ok(());
    };
    let Some(block) = c.doc(|d| d.blocks.get(&ins.name).cloned()) else {
        c.log_error("The block is not defined.");
        return Ok(());
    };
    let Some(info) = block.xref.clone() else {
        c.log_error(&format!("\"{}\" is an ordinary block, not an external reference. BEDIT opens block definitions.", block.name));
        return Ok(());
    };
    let want = info
        .path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&info.path)
        .to_lowercase();
    let open = c.app(|a| {
        a.editors.iter().position(|e| {
            let n = e
                .doc
                .file_name
                .clone()
                .unwrap_or_else(|| e.doc.name.clone())
                .to_lowercase();
            let stem = |s: &str| {
                s.rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(s)
                    .split('.')
                    .next()
                    .unwrap_or(s)
                    .to_string()
            };
            !e.doc.name.starts_with("*BlockEdit:") && stem(&n) == stem(&want)
        })
    });
    match open {
        Some(i) => {
            c.app(|a| a.set_active(i));
            c.log(&format!("Switched to the open drawing {}.", info.path));
        }
        None => {
            // the source is not open: show a copy of what the reference holds, without its `NAME|` prefixes
            let prefix = format!("{}|", block.name);
            let name = format!("{} (copy of xref)", block.name);
            let d = c.doc(|p| {
                let mut d = Drawing::new(&name);
                for l in &p.layers {
                    if let Some(short) = l.name.strip_prefix(&prefix) {
                        let mut nl = l.clone();
                        nl.name = short.to_string();
                        if d.layer(short).is_none() {
                            d.layers.push(nl);
                        }
                    }
                }
                for e in &block.entities {
                    let mut e = e.cloned_noid();
                    if let Some(short) = e.layer.strip_prefix(&prefix) {
                        e.layer = short.to_string();
                    }
                    d.add(e);
                }
                d.dirty = false;
                d
            });
            c.app(|a| {
                a.add_drawing(d);
            });
            c.ed(|e| e.zoom_extents(0.1));
            c.log(&format!(
                "{} is not open; showing a copy of the reference contents.",
                info.path
            ));
        }
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    let sessions = Sessions::default();
    let (a, b, d) = (sessions.clone(), sessions.clone(), sessions);
    r.define(
        db("REFEDIT", run(move |c, x| refedit(c, x, a.clone())))
            .title("Edit Reference")
            .icon("refedit")
            .desc("Selects a reference for editing in place."),
    );
    r.define(
        db("REFCLOSE", run(move |c, x| refclose(c, x, b.clone())))
            .title("Close Reference")
            .icon("refedit")
            .desc("Saves back or discards changes made during in-place editing of a reference.")
            .once(),
    );
    r.define(db("REFSET", run(move |c, x| refset(c, x, d.clone()))).title("Edit Working Set").icon("refedit").desc("Adds or removes objects from the working set during in-place editing of a reference."));
    r.define(
        db("XOPEN", run(xopen))
            .title("Open Xref")
            .icon("xref")
            .desc("Opens a selected drawing reference (xref) in a new window."),
    );
}
