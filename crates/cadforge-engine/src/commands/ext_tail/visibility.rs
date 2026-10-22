//! ISOLATEOBJECTS, HIDEOBJECTS and UNISOLATEOBJECTS: hide objects for the rest of the session without touching layers.
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use std::collections::BTreeSet;

fn plural(n: usize) -> String {
    format!("{n} object{}", if n == 1 { "" } else { "s" })
}

/// Hide everything in the current space except the selection.
async fn isolate(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects to isolate")).await?;
    if sel.ents.is_empty() && sel.solids.is_empty() {
        return Ok(());
    }
    let keep: BTreeSet<u64> = sel.ents.iter().chain(sel.solids.iter()).copied().collect();
    let others: Vec<u64> = c.doc(|d| {
        let mut v: Vec<u64> = d.entities().iter().filter(|e| d.is_visible(e) && !keep.contains(&e.id)).map(|e| e.id).collect();
        v.extend(d.solids.iter().filter(|s| d.solid_visible(s) && !keep.contains(&s.id)).map(|s| s.id));
        v
    });
    let n = others.len();
    c.doc_mut(|d| {
        d.hidden_objs.extend(others);
        d.touch();
    });
    c.ed(|e| e.clear_sel());
    c.log(&format!("{} hidden.", plural(n)));
    c.emit();
    Ok(())
}

/// Hide the selected objects.
async fn hideobjects(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects to hide")).await?;
    let ids: Vec<u64> = sel.ents.iter().chain(sel.solids.iter()).copied().collect();
    if ids.is_empty() {
        return Ok(());
    }
    let n = ids.len();
    c.doc_mut(|d| {
        d.hidden_objs.extend(ids);
        d.touch();
    });
    c.ed(|e| e.clear_sel());
    c.log(&format!("{} hidden.", plural(n)));
    c.emit();
    Ok(())
}

/// Show everything HIDEOBJECTS / ISOLATEOBJECTS hid.
async fn unisolate(c: Ctx, _a: Args) -> CmdResult<()> {
    let n = c.doc_mut(|d| {
        let n = d.hidden_objs.len();
        d.hidden_objs.clear();
        d.touch();
        n
    });
    if n == 0 {
        c.log("No objects are hidden.");
    } else {
        c.log(&format!("{} displayed.", plural(n)));
    }
    c.emit();
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("Select", "ISOLATEOBJECTS", run(isolate)).aliases(&["ISOLATE"]).title("Isolate Objects").icon("layiso").desc("Hides all objects except the selected ones until UNISOLATEOBJECTS."));
    r.define(def("Select", "HIDEOBJECTS", run(hideobjects)).aliases(&["OBJECTHIDE"]).title("Hide Objects").icon("layoff").desc("Hides the selected objects until UNISOLATEOBJECTS."));
    r.define(def("Select", "UNISOLATEOBJECTS", run(unisolate)).aliases(&["UNISOLATE", "ENDOBJECTISOLATION"]).title("End Object Isolation").icon("layuniso").desc("Displays the objects hidden by ISOLATEOBJECTS or HIDEOBJECTS.").once());
}
