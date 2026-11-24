//! DIMOVERRIDE (dimension variables changed for chosen dimensions only) and DIMJOGLINE (a jog in the dimension line of
//! a linear or aligned dimension, to show the measured length is not drawn to scale).
use super::{plural, ps};
use crate::commands::helpers::pv;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::dim::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::DIM_VARS;

fn is_dim(e: &Entity) -> bool {
    matches!(e.kind, Kind::Dim(_))
}

fn is_linear(e: &Entity) -> bool {
    matches!(&e.kind, Kind::Dim(d) if matches!(d.kind, DimType::Lin | DimType::Ali))
}

// ---------------------------------------------------------------- DIMOVERRIDE

/// Set `(name, value)` in a dimension's overrides, replacing an earlier override of the same variable.
pub fn put_override(ovr: &mut Vec<(String, String)>, name: &str, value: &str) {
    ovr.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
    ovr.push((name.to_ascii_uppercase(), value.to_string()));
}

/// DIMOVERRIDE: change dimension variables for the selected dimensions only, without changing their dimension style;
/// Clear removes every override.
async fn dimoverride(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.doc(|d| d.settings.cur_dim().clone());
    let mut changes: Vec<(String, String)> = vec![];
    loop {
        let msg = if changes.is_empty() { "Enter dimension variable name to override or [Clear overrides/?]" } else { "Enter dimension variable name to override" };
        let t = c.string(ps(msg).allow_enter()).await?.text;
        let t = t.trim().to_string();
        if t.is_empty() {
            break;
        }
        if t == "?" {
            let list: Vec<String> = DIM_VARS.iter().map(|v| format!("  {v} = {}", cur.get_var(v).unwrap_or_default())).collect();
            c.log(&format!("Dimension variables (current style \"{}\"):\n{}", cur.name, list.join("\n")));
            continue;
        }
        if changes.is_empty() && ["C", "CL", "CLE", "CLEA", "CLEAR", "CLEAR OVERRIDES"].contains(&t.to_ascii_uppercase().as_str()) {
            let sel = c.select(ps("Select objects").filter(is_dim)).await?;
            let n = c.doc_mut(|d| {
                d.change(|d| {
                    let mut n = 0;
                    for id in &sel.ents {
                        if let Some(Entity { kind: Kind::Dim(dd), .. }) = d.by_id_mut(*id) {
                            if !dd.ovr.is_empty() {
                                dd.ovr.clear();
                                n += 1;
                            }
                        }
                    }
                    n
                })
            });
            c.log(&format!("Overrides cleared from {}.", plural(n, "dimension")));
            c.clear_sel();
            return Ok(());
        }
        let name = t.to_ascii_uppercase();
        let name = if name.starts_with("DIM") { name } else { format!("DIM{name}") };
        let Some(now) = cur.get_var(&name) else {
            c.log_error(&format!("{name} is not a dimension variable CadForge overrides. Type ? for the list."));
            continue;
        };
        let v = c.string(ps(&format!("Enter new value for dimension variable {name}")).default_str(&now)).await?.text;
        let v = if v.trim().is_empty() { now } else { v.trim().to_string() };
        let mut test = cur.clone();
        match test.set_var(&name, &v) {
            Ok(()) => put_override(&mut changes, &name, &v),
            Err(e) => c.log_error(&e),
        }
    }
    if changes.is_empty() {
        return Ok(());
    }
    let sel = c.select(ps("Select objects").filter(is_dim)).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let n = c.doc_mut(|d| {
        d.change(|d| {
            let mut n = 0;
            for id in &sel.ents {
                if let Some(Entity { kind: Kind::Dim(dd), .. }) = d.by_id_mut(*id) {
                    for (k, v) in &changes {
                        put_override(&mut dd.ovr, k, v);
                    }
                    n += 1;
                }
            }
            n
        })
    });
    c.log(&format!("{} overridden.", plural(n, "dimension")));
    c.clear_sel();
    Ok(())
}

// ---------------------------------------------------------------- DIMJOGLINE

/// DIMJOGLINE: add a jog to the dimension line of a linear or aligned dimension (Enter puts it between the first
/// extension line and the text), or Remove one.
async fn dimjogline(c: Ctx, _a: Args) -> CmdResult<()> {
    let r = c.entity(ps("Select dimension to add jog or").options(&["Remove"]).filter(is_linear)).await?;
    let id = match r {
        EntR::Entity { id, .. } => id,
        EntR::Kw(_) => {
            let has_jog = |e: &Entity| matches!(&e.kind, Kind::Dim(d) if d.jl.is_some());
            let Some(id) = c.entity(ps("Select jog to remove").filter(has_jog)).await?.id() else { return Ok(()) };
            c.doc_mut(|d| {
                d.modify(id, |e| {
                    if let Kind::Dim(dd) = &mut e.kind {
                        dd.jl = None;
                    }
                })
            });
            c.log("Jog removed.");
            return Ok(());
        }
        EntR::Enter => return Ok(()),
    };
    let Some(Kind::Dim(dim)) = c.entity_by_id(id).map(|e| e.kind) else { return Ok(()) };
    let geo = c.doc(|d| dim_geometry(&DimData { jl: None, ..dim.clone() }, &d.settings));
    let Some(line) = geo.dim_line.clone() else { return Ok(()) };
    let shape = dim.clone();
    let spec = ps("Specify jog location (or press ENTER)").allow_enter().no_ortho().preview(move |_, p| vec![pv(Entity::dim(DimData { jl: Some(p), ..shape.clone() }))]);
    let at = match c.point(spec).await?.pt() {
        Some(p) => p,
        // between the first extension line and the text, as AutoCAD places it
        None => mid(line.a, line.a + line.dir * dot(geo.text.p - line.a, line.dir)),
    };
    if jog_points(line.a, line.b, at, geo.text.h * 1.5).is_none() {
        c.log_error("The dimension line is too short for a jog.");
        return Ok(());
    }
    c.doc_mut(|d| {
        d.modify(id, |e| {
            if let Kind::Dim(dd) = &mut e.kind {
                dd.jl = Some(at);
            }
        })
    });
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Annotate", "DIMOVERRIDE", run(dimoverride)).aliases(&["DOV"]).title("Dimension Override").icon("dimstyle").desc("Overrides dimension variables for selected dimensions only, or clears their overrides."));
    r.define(def("Annotate", "DIMJOGLINE", run(dimjogline)).aliases(&["DJL"]).title("Jogged Linear").icon("dimjogged").desc("Adds or removes a jog line in a linear or aligned dimension."));
}
