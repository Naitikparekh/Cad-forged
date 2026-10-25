//! Associative dimensions: DIMREASSOCIATE ties a dimension's points to geometry, DIMDISASSOCIATE frees it and DIMREGEN
//! brings every associative dimension up to date (the links themselves are in `cadforge_core::dim_assoc`).
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::dim::*;
use cadforge_core::dim_assoc::keys;
use cadforge_core::entities::*;

fn is_dim(e: &Entity) -> bool {
    matches!(e.kind, Kind::Dim(_))
}

fn plural(n: usize, w: &str) -> String {
    format!("{n} {w}{}", if n == 1 { "" } else { "s" })
}

fn label(kind: DimType, key: &str) -> &'static str {
    match (kind, key) {
        (_, "p1") => "first extension line origin",
        (_, "p2") => "second extension line origin",
        (DimType::Ang, "c") => "angle vertex",
        (DimType::Ord, _) => "feature location",
        (_, "c") => "center of the circle or arc",
        _ => "point on the circle or arc",
    }
}

fn dims_of(c: &Ctx, ids: &[u64]) -> Vec<(u64, DimData)> {
    c.entities(ids).into_iter().filter_map(|e| if let Kind::Dim(d) = e.kind { Some((e.id, d)) } else { None }).collect()
}

/// DIMDISASSOCIATE: the selected dimensions no longer follow any geometry.
async fn dimdisassociate(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select dimensions to disassociate").filter(is_dim)).await?;
    let ids: Vec<u64> = dims_of(&c, &sel.ents).into_iter().filter(|(_, d)| !d.assoc.is_empty()).map(|(id, _)| id).collect();
    c.doc_mut(|d| {
        for id in &ids {
            d.modify(*id, |e| {
                if let Kind::Dim(dd) = &mut e.kind {
                    dd.assoc.clear();
                }
            });
        }
    });
    c.log(&format!("{} disassociated.", plural(ids.len(), "dimension")));
    Ok(())
}

/// DIMREASSOCIATE: for each selected dimension, each defining point can be picked again (with object snaps; Enter
/// keeps it where it is) and is then tied to the geometry under it.
async fn dimreassociate(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select dimensions to reassociate").filter(is_dim)).await?;
    for (id, mut d) in dims_of(&c, &sel.ents) {
        c.ed(|e| e.sel = [id].into_iter().collect());
        for key in keys(d.kind) {
            let Some(cur) = d.point_mut(key).and_then(|p| *p) else { continue };
            let r = c.point(ps(&format!("Specify {} or [Next] <next>", label(d.kind, key))).options(&["Next"]).allow_enter()).await?;
            if let (Some(p), Some(slot)) = (r.pt(), d.point_mut(key)) {
                if p != cur {
                    *slot = Some(p);
                }
            }
        }
        let refs = c.doc(|doc| doc.find_dim_refs(id, &d));
        let n = refs.len();
        d.assoc = refs;
        c.doc_mut(|doc| doc.modify(id, |e| e.kind = Kind::Dim(d)));
        c.log(&format!("Dimension tied to geometry at {}.", plural(n, "point")));
    }
    c.ed(|e| e.clear_sel());
    Ok(())
}

/// DIMREGEN: move every associative dimension to where its geometry is now.
async fn dimregen(c: Ctx, _a: Args) -> CmdResult<()> {
    let n = c.doc_mut(|d| d.update_dim_associations(u64::MAX));
    let total = c.doc(|d| d.all_entities().filter(|e| matches!(&e.kind, Kind::Dim(x) if !x.assoc.is_empty())).count());
    c.log(&format!("{} associative; {n} updated.", plural(total, "dimension")));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Annotation", "DIMREASSOCIATE", run(dimreassociate)).aliases(&["DRE"]).title("Reassociate Dimensions").icon("dim").desc("Associates selected dimensions with the geometry at their points, so they follow it."));
    r.define(def("Annotation", "DIMDISASSOCIATE", run(dimdisassociate)).aliases(&["DDA"]).title("Disassociate Dimensions").icon("dim").desc("Removes associativity from selected dimensions."));
    r.define(def("Annotation", "DIMREGEN", run(dimregen)).title("Update Dimensions").icon("dim").desc("Updates the locations of all associative dimensions."));
}
