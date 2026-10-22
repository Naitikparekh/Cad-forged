//! Block editor: BEDIT opens a block definition in its own drawing tab (`*BlockEdit:NAME`), BCLOSE writes the edited
//! entities back into the drawing the block came from and closes the tab.
use super::{block_key, db, ps};
use crate::commands::helpers::transformed_clones;
use crate::commands::run;
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::{Block, Drawing};
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::Layer;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// A block open in its own editor tab.
struct Session {
    /// Editor ids of the `*BlockEdit:NAME` tab and of the drawing the block belongs to.
    editor: u64,
    parent: u64,
    block: String,
}

type Sessions = Rc<RefCell<Vec<Session>>>;

/// The block's entities with its base point moved to the origin, in a drawing that shares the parent's layers,
/// settings and (other) block definitions so nested references keep working.
fn block_drawing(p: &Drawing, b: &Block) -> Drawing {
    let mut d = Drawing::new(&format!("*BlockEdit:{}", b.name));
    d.layers = p.layers.clone();
    d.blocks = p.blocks.clone();
    d.blocks.shift_remove(&b.name);
    d.settings = p.settings.clone();
    d.cur.layer = p.cur.layer.clone();
    d.next_id = p.next_id;
    d.splice(0, 0, transformed_clones(&b.entities, &mat::translate(-b.base.x, -b.base.y)));
    d.dirty = false;
    d
}

async fn bedit(c: Ctx, args: Args, sessions: Sessions) -> CmdResult<()> {
    let (here, label) = (c.ed(|e| e.id), c.name());
    let live: HashSet<u64> = c.app(|a| a.editors.iter().map(|e| e.id).collect());
    sessions.borrow_mut().retain(|s| live.contains(&s.editor));
    if sessions.borrow().iter().any(|s| s.editor == here) {
        c.log_error("BEDIT is not available in the Block Editor; close it with BCLOSE first.");
        return Ok(());
    }
    // a single selected block reference is edited directly
    let selected = c.ed(|e| match e.selected().as_slice() {
        [x] => match &x.kind {
            Kind::Insert(i) => Some(i.name.clone()),
            _ => None,
        },
        _ => None,
    });
    let name = match args.str("name").map(str::to_string).or(selected) {
        Some(n) => n,
        None => loop {
            let t = c.string(ps("Enter block to edit or [?]")).await?.text;
            match t.as_str() {
                "" => return Ok(()),
                "?" => c.log(&c.doc(|d| d.blocks.keys().cloned().collect::<Vec<_>>().join("\n"))),
                _ => break t,
            }
        },
    };
    let Some(key) = block_key(&c, &name) else {
        c.log_error(&format!("Block \"{name}\" not found."));
        return Ok(());
    };
    if c.doc(|d| d.blocks[&key].xref.is_some()) {
        c.log_error(&format!("\"{key}\" is an external reference and cannot be opened in the Block Editor."));
        return Ok(());
    }
    // the drawing being left closes its undo step now; the runtime would close the new tab's instead
    c.doc_mut(|d| d.end_command(&label));
    let open = sessions.borrow().iter().find(|s| s.parent == here && s.block == key).map(|s| s.editor);
    match open.and_then(|id| c.app(|a| a.editors.iter().position(|e| e.id == id))) {
        Some(idx) => c.app(|a| a.set_active(idx)),
        None => {
            let d = c.doc(|p| block_drawing(p, &p.blocks[&key]));
            let editor = c.app(|a| a.add_drawing(d));
            sessions.borrow_mut().push(Session { editor, parent: here, block: key });
            c.ed(|e| e.zoom_extents(0.1));
        }
    }
    Ok(())
}

/// Replace block `name` in `p` by the edited `entities` as one undo step. Layers and blocks created in the editor
/// are added; existing definitions other than the edited block are left alone.
fn write_back(p: &mut Drawing, name: &str, entities: &[Entity], blocks: &IndexMap<String, Block>, layers: &[Layer]) {
    let base = p.blocks.get(name).map_or(Pt::ZERO, |b| b.base);
    p.checkpoint("BCLOSE");
    p.change(|p| {
        for l in layers {
            if p.layer(&l.name).is_none() {
                p.layers.push(l.clone());
            }
        }
        for (n, b) in blocks {
            if !p.blocks.contains_key(n) {
                p.define_block(n, b.base, b.entities.iter().map(Entity::cloned_noid).collect());
            }
        }
        p.define_block(name, base, transformed_clones(entities, &mat::translate(base.x, base.y)));
    });
}

async fn bclose(c: Ctx, _a: Args, sessions: Sessions) -> CmdResult<()> {
    let here = c.ed(|e| e.id);
    let found = sessions.borrow().iter().find(|s| s.editor == here).map(|s| (s.parent, s.block.clone()));
    let Some((parent, block)) = found else {
        c.log_error("BCLOSE works in a Block Editor tab; open one with BEDIT.");
        return Ok(());
    };
    let mut save = c.doc(|d| d.dirty);
    if save {
        save = c.keyword(ps(&format!("Save changes to {block}?")).options(&["Yes", "No"]).default_str("Yes")).await?.kw() != Some("No");
    }
    if save {
        let (entities, blocks, layers) = c.doc(|d| (d.model.entities.clone(), d.blocks.clone(), d.layers.clone()));
        let saved = c.app(|a| a.editors.iter_mut().find(|e| e.id == parent).map(|e| write_back(&mut e.doc, &block, &entities, &blocks, &layers)));
        match saved {
            Some(()) => c.log(&format!("Block \"{block}\" redefined.")),
            None => c.log_error("The drawing this block came from is closed; the changes were discarded."),
        }
    }
    sessions.borrow_mut().retain(|s| s.editor != here);
    c.app(|a| {
        let at = |a: &crate::app::App, id: u64| a.editors.iter().position(|e| e.id == id);
        if let Some(i) = at(a, parent) {
            a.set_active(i);
        }
        if let Some(i) = at(a, here) {
            a.close_editor(i);
        }
    });
    Ok(())
}

/// The block this editor tab edits and the drawing it belongs to.
fn current_session(c: &Ctx, sessions: &Sessions) -> Option<(u64, String)> {
    let here = c.ed(|e| e.id);
    let found = sessions.borrow().iter().find(|s| s.editor == here).map(|s| (s.parent, s.block.clone()));
    found
}

/// BSAVE: write the edited block back without closing the editor.
async fn bsave(c: Ctx, _a: Args, sessions: Sessions) -> CmdResult<()> {
    let Some((parent, block)) = current_session(&c, &sessions) else {
        c.log_error("BSAVE works in a Block Editor tab; open one with BEDIT.");
        return Ok(());
    };
    let (entities, blocks, layers) = c.doc(|d| (d.model.entities.clone(), d.blocks.clone(), d.layers.clone()));
    let saved = c.app(|a| a.editors.iter_mut().find(|e| e.id == parent).map(|e| write_back(&mut e.doc, &block, &entities, &blocks, &layers)));
    match saved {
        Some(()) => {
            c.doc_mut(|d| d.dirty = false);
            c.log(&format!("Block \"{block}\" saved."));
        }
        None => c.log_error("The drawing this block came from is closed."),
    }
    Ok(())
}

/// BSAVEAS: save the edited content under a new block name (the block being edited is not changed).
async fn bsaveas(c: Ctx, _a: Args, sessions: Sessions) -> CmdResult<()> {
    let Some((parent, block)) = current_session(&c, &sessions) else {
        c.log_error("BSAVEAS works in a Block Editor tab; open one with BEDIT.");
        return Ok(());
    };
    let name = c.string(ps("Enter new block name")).await?.text.trim().to_string();
    if name.is_empty() || name.starts_with('*') || name.contains(['|', ',', '=']) {
        c.log_error("A block name cannot be empty, start with * or contain | , or =.");
        return Ok(());
    }
    let (entities, layers) = c.doc(|d| (d.model.entities.clone(), d.layers.clone()));
    let result = c.app(|a| {
        a.editors.iter_mut().find(|e| e.id == parent).map(|e| {
            if e.doc.blocks.keys().any(|n| n.eq_ignore_ascii_case(&name)) {
                return Err(());
            }
            let base = e.doc.blocks.get(&block).map_or(Pt::ZERO, |b| b.base);
            e.doc.checkpoint("BSAVEAS");
            e.doc.change(|p| {
                for l in &layers {
                    if p.layer(&l.name).is_none() {
                        p.layers.push(l.clone());
                    }
                }
                p.define_block(&name, base, transformed_clones(&entities, &mat::translate(base.x, base.y)));
            });
            Ok(())
        })
    });
    match result {
        Some(Ok(())) => c.log(&format!("Block \"{block}\" saved as \"{name}\".")),
        Some(Err(())) => c.log_error(&format!("A block named \"{name}\" already exists.")),
        None => c.log_error("The drawing this block came from is closed."),
    }
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    let sessions = Sessions::default();
    let (save, save_as) = (sessions.clone(), sessions.clone());
    r.define(db("BSAVE", run(move |c, a| bsave(c, a, save.clone()))).title("Save Block").icon("block").desc("Saves the current block definition in the Block Editor.").once());
    r.define(db("BSAVEAS", run(move |c, a| bsaveas(c, a, save_as.clone()))).title("Save Block As").icon("block").desc("Saves a copy of the current block definition under a new name.").once());
    let open = sessions.clone();
    r.define(db("BEDIT", run(move |c, a| bedit(c, a, open.clone()))).aliases(&["BE"]).title("Block Editor").icon("block").desc("Opens a block definition in the Block Editor."));
    r.define(db("BCLOSE", run(move |c, a| bclose(c, a, sessions.clone()))).title("Close Block Editor").icon("block").desc("Closes the Block Editor and saves the block definition."));
}
