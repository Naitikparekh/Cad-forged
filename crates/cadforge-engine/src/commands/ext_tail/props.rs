//! SETBYLAYER, TXT2MTXT and LISTSELECT.
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;

/// SETBYLAYER: send colour, linetype, lineweight and transparency back to the layer.
async fn setbylayer(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects or [Change settings]").options(&["Change settings"])).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let mut which = ["Color", "Ltype", "LWeight", "TRansparency"].map(|n| (n, true));
    let opt = c.keyword(ps("Enter object properties to change [Color/Ltype/LWeight/TRansparency/All]").options(&["Color", "Ltype", "LWeight", "TRansparency", "All"]).default_str("All")).await?;
    if let Some(k) = opt.kw().filter(|k| *k != "All") {
        for w in which.iter_mut() {
            w.1 = w.0 == k;
        }
    }
    let by_block = c.keyword(ps("Change ByBlock to ByLayer? [Yes/No]").options(&["Yes", "No"]).default_str("Yes")).await?;
    let keep_block = by_block.kw() == Some("No");
    let ids = sel.ents.clone();
    let mut changed = 0;
    c.doc_mut(|d| {
        d.change(|d| {
            for id in ids {
                let Some(e) = d.by_id_mut(id) else { continue };
                let mut hit = false;
                for (name, on) in which {
                    if !on {
                        continue;
                    }
                    match name {
                        "Color" if e.color.is_some() && !(keep_block && e.color == Some(cadforge_core::style::Color::ByBlock)) => {
                            e.color = None;
                            hit = true;
                        }
                        "Ltype" if e.ltype.is_some() && !(keep_block && e.ltype.as_deref() == Some("BYBLOCK")) => {
                            e.ltype = None;
                            hit = true;
                        }
                        "LWeight" if e.lw.is_some() && !(keep_block && e.lw == Some(cadforge_core::style::LW_BYBLOCK)) => {
                            e.lw = None;
                            hit = true;
                        }
                        "TRansparency" if e.transp.is_some() => {
                            e.transp = None;
                            hit = true;
                        }
                        _ => {}
                    }
                }
                changed += hit as usize;
            }
        })
    });
    c.log(&format!("{changed} object{} changed to ByLayer.", if changed == 1 { "" } else { "s" }));
    c.emit();
    Ok(())
}

/// TXT2MTXT: single-line text becomes multiline text, keeping position, height and style.
async fn txt2mtxt(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select annotation objects").filter(|e| matches!(&e.kind, Kind::Text(t) if !t.multi && t.att.is_none()))).await?;
    let ids: Vec<u64> = sel.ents.iter().copied().filter(|i| matches!(c.entity_by_id(*i).map(|e| e.kind), Some(Kind::Text(t)) if !t.multi && t.att.is_none())).collect();
    if ids.is_empty() {
        c.log_error("No single-line text objects selected.");
        return Ok(());
    }
    let n = ids.len();
    c.doc_mut(|d| {
        d.change(|d| {
            for id in ids {
                if let Some(Entity { kind: Kind::Text(t), .. }) = d.by_id_mut(id) {
                    t.multi = true;
                }
            }
        })
    });
    c.log(&format!("{n} text object{} converted to multiline text.", if n == 1 { "" } else { "s" }));
    c.emit();
    Ok(())
}

/// LISTSELECT: LIST for the objects that are already selected.
async fn listselect(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.ed(|e| e.sel.is_empty()) {
        c.log_error("No objects are selected.");
        return Ok(());
    }
    c.sub_forced("LIST", Args::none()).await;
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("Properties", "SETBYLAYER", run(setbylayer)).title("SetByLayer").icon("bylayer").desc("Changes the properties of selected objects to ByLayer."));
    r.define(def("Annotate", "TXT2MTXT", run(txt2mtxt)).title("Convert Text to Multiline Text").icon("mtext").desc("Converts single-line text objects to multiline text."));
    r.define(def("Inquiry", "LISTSELECT", run(listselect)).title("List Selected").icon("list").desc("Lists the properties of the objects that are currently selected.").no_pickfirst());
}
