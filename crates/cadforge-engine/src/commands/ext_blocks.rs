//! Block, attribute and external-reference commands (parity batch: ext_blocks): ATTDEF, ATTEDIT, EATTEDIT, ATTDISP,
//! ATTSYNC, BATTMAN, BEDIT, BCLOSE, BASE, XREF, XATTACH and XBIND.
//!
//! INSERT and BLOCK are wrapped so that every block reference they create receives its attribute values; the same
//! prompting is available to other commands as [`fill_attributes`].
use super::{def, run};
use crate::ctx::*;
use crate::registry::{CommandDef, Registry, RunFn};
use crate::runtime::*;
use cadforge_core::entities::*;
use std::collections::HashSet;

mod attrs;
mod bedit;
mod xref;

pub use attrs::fill_attributes;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

/// A prompt showing `<default>` only when there is a default to show.
fn ps_def(m: &str, default: &str) -> PromptSpec {
    if default.is_empty() {
        ps(m)
    } else {
        ps(m).default_str(default)
    }
}

fn db(name: &str, f: RunFn) -> CommandDef {
    def("Block", name, f)
}

/// The name of the block called `text` (case-insensitive).
fn block_key(c: &Ctx, text: &str) -> Option<String> {
    c.doc(|d| d.blocks.keys().find(|n| n.eq_ignore_ascii_case(text)).cloned())
}

/// Re-register `name` so that the block references it creates are completed with attribute values.
fn with_attributes(r: &mut Registry, name: &str) {
    let Some(old) = r.get(name) else { return };
    let inner = old.run.clone();
    let mut wrapped = (*old).clone();
    wrapped.run = run(move |c, a| {
        let inner = inner.clone();
        async move {
            let seen: HashSet<u64> = c.doc(|d| d.entities().iter().map(|e| e.id).collect());
            let res = inner(c.clone(), a).await;
            let fresh: Vec<u64> = c.doc(|d| d.entities().iter().filter(|e| matches!(e.kind, Kind::Insert(_)) && !seen.contains(&e.id)).map(|e| e.id).collect());
            for id in &fresh {
                // cancelling an attribute prompt cancels the insertion; after a cancelled INSERT the defaults are used
                if let Err(e) = attrs::fill_attributes(&c, *id, res.is_ok()).await {
                    c.doc_mut(|d| d.remove_all(&fresh));
                    return Err(e);
                }
            }
            res
        }
    });
    r.define(wrapped);
}

async fn base(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.doc(|d| d.base);
    let Some(p) = c.point(ps("Enter base point").default_pt(cur)).await?.pt() else { return Ok(()) };
    c.doc_mut(|d| d.change(|d| d.base = p));
    Ok(())
}

pub fn register(r: &mut Registry) {
    with_attributes(r, "INSERT");
    with_attributes(r, "BLOCK");
    r.define(db("BASE", run(base)).title("Base").icon("block").desc("Sets the insertion base point for the current drawing."));
    attrs::register(r);
    bedit::register(r);
    xref::register(r);
}
