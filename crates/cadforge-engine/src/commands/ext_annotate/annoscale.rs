//! Annotation scales: OBJECTSCALE (the scales an annotative object supports), ANNOUPDATE and ANNORESET (size
//! annotative objects for the current annotation scale) and SCALELISTEDIT (the drawing's scale list). CANNOSCALE sets
//! the current annotation scale; CHPROP Annotative and the Properties palette make objects annotative.
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::annotative::{default_scales, parse_scale, size_of, AnnoScale};

fn plural(n: usize, w: &str) -> String {
    format!("{n} {w}{}", if n == 1 { "" } else { "s" })
}

fn scale_names(c: &Ctx) -> Vec<String> {
    c.doc(|d| d.settings.scales.iter().map(|s| s.name.clone()).collect())
}

/// Ask for a scale of the list (`?` lists them); `None` on Enter.
async fn ask_scale(c: &Ctx, prompt: &str) -> CmdResult<Option<String>> {
    loop {
        let t = c.string(ps(prompt)).await?.text;
        let t = t.trim().to_string();
        if t.is_empty() {
            return Ok(None);
        }
        if t == "?" {
            c.log(&scale_names(c).join(", "));
            continue;
        }
        match c.doc(|d| d.anno_scale(&t).map(|s| s.name.clone())) {
            Some(n) => return Ok(Some(n)),
            None => c.log_error(&format!("\"{t}\" is not in the scale list. Use SCALELISTEDIT to add it.")),
        }
    }
}

/// OBJECTSCALE: add or delete annotation scales of the selected annotative objects, or list them.
async fn objectscale(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select annotative objects")).await?;
    let ids: Vec<u64> = c.entities(&sel.ents).into_iter().filter(|e| e.anno.is_some()).map(|e| e.id).collect();
    if ids.is_empty() {
        c.log_error("No annotative objects were selected (CHPROP Annotative makes objects annotative).");
        return Ok(());
    }
    if ids.len() < sel.ents.len() {
        c.log(&format!("Left out {} that {} not annotative.", plural(sel.ents.len() - ids.len(), "object"), if sel.ents.len() - ids.len() == 1 { "is" } else { "are" }));
    }
    let opt = c.keyword(ps("Enter an option [Add/Delete/?]").options(&["Add", "Delete", "?"]).default_str("Add")).await?.kw().unwrap_or("Add").to_string();
    match opt.as_str() {
        "?" => {
            let lines: Vec<String> = c.entities(&ids).into_iter().map(|e| format!("{} {}: {}", e.type_name(), e.id, e.anno.map(|a| a.scales.join(", ")).unwrap_or_default())).collect();
            c.log(&lines.join("\n"));
        }
        o => {
            let add = o == "Add";
            let Some(name) = ask_scale(&c, if add { "Enter named scale to add or [?]" } else { "Enter named scale to delete or [?]" }).await? else { return Ok(()) };
            let n = c.doc_mut(|d| {
                d.change(|d| {
                    let mut n = 0;
                    for id in &ids {
                        if let Some(a) = d.by_id_mut(*id).and_then(|e| e.anno.as_mut()) {
                            let has = a.scales.iter().any(|s| s.eq_ignore_ascii_case(&name));
                            if add && !has {
                                a.scales.push(name.clone());
                                n += 1;
                            } else if !add && has && a.scales.len() > 1 {
                                a.scales.retain(|s| !s.eq_ignore_ascii_case(&name));
                                n += 1;
                            }
                        }
                    }
                    n
                })
            });
            c.log(&format!("{} {} {name}.", plural(n, "object"), if add { "now supports" } else { "no longer supports" }));
        }
    }
    Ok(())
}

/// Size the selected annotative objects for the current annotation scale.
async fn update(c: Ctx, what: &str) -> CmdResult<()> {
    let sel = c.select(ps("Select objects")).await?;
    let factor = c.doc(|d| d.anno_factor());
    let ids: Vec<u64> = sel.ents.clone();
    let n = c.doc_mut(|d| {
        d.change(|d| {
            let mut n = 0;
            for id in &ids {
                if let Some(e) = d.by_id_mut(*id) {
                    if let (Some(a), Some(cur)) = (e.anno.clone(), size_of(e)) {
                        if cur > 0.0 && (a.paper * factor - cur).abs() > 1e-12 * cur.max(1.0) {
                            cadforge_core::annotative::scale_size(e, a.paper * factor / cur);
                        }
                        n += 1;
                    }
                }
            }
            n
        })
    });
    c.log(&format!("{} {what}.", plural(n, "annotative object")));
    Ok(())
}

async fn annoupdate(c: Ctx, _a: Args) -> CmdResult<()> {
    update(c, "updated").await
}

async fn annoreset(c: Ctx, _a: Args) -> CmdResult<()> {
    update(c, "reset to the current annotation scale").await
}

/// SCALELISTEDIT (command line): list, add, delete or reset the scales of the drawing.
async fn scalelistedit(c: Ctx, _a: Args) -> CmdResult<()> {
    loop {
        let opt = c.keyword(ps("Enter option [?/Add/Delete/Reset/Exit]").options(&["?", "Add", "Delete", "Reset", "Exit"]).default_str("Add")).await?.kw().unwrap_or("Exit").to_string();
        match opt.as_str() {
            "?" => {
                let lines: Vec<String> = c.doc(|d| d.settings.scales.iter().map(|s| format!("  {:<10} {} paper = {} drawing", s.name, s.paper, s.drawing)).collect());
                c.log(&format!("Scales:\n{}", lines.join("\n")));
            }
            "Add" => {
                let name = c.string(ps("Enter name of new scale")).await?.text.trim().to_string();
                if name.is_empty() {
                    continue;
                }
                if c.doc(|d| d.anno_scale(&name).is_some()) {
                    c.log_error(&format!("A scale named \"{name}\" already exists."));
                    continue;
                }
                let ratio = match parse_scale(&name) {
                    Some(r) => Some(r),
                    None => parse_scale(&c.string(ps("Enter scale ratio (paper units:drawing units)")).await?.text),
                };
                let Some((p, d)) = ratio else {
                    c.log_error("Enter the ratio as paper units:drawing units, such as 1:25.");
                    continue;
                };
                c.doc_mut(|doc| doc.change(|doc| doc.settings.scales.push(AnnoScale::new(&name, p, d))));
                c.log(&format!("Scale {name} added."));
            }
            "Delete" => {
                let Some(name) = ask_scale(&c, "Enter name of scale to delete or [?]").await? else { continue };
                if c.doc(|d| d.settings.cannoscale.eq_ignore_ascii_case(&name)) {
                    c.log_error("The current annotation scale cannot be deleted.");
                    continue;
                }
                c.doc_mut(|d| d.change(|d| d.settings.scales.retain(|s| !s.name.eq_ignore_ascii_case(&name))));
                c.log(&format!("Scale {name} deleted."));
            }
            "Reset" => {
                c.doc_mut(|d| {
                    d.change(|d| {
                        d.settings.scales = default_scales();
                        if d.anno_scale(&d.settings.cannoscale.clone()).is_none() {
                            d.settings.cannoscale = "1:1".into();
                        }
                    })
                });
                c.log("The scale list is back to the default scales.");
            }
            _ => return Ok(()),
        }
    }
}

pub fn register(r: &mut Registry) {
    r.define(def("Annotation", "OBJECTSCALE", run(objectscale)).title("Annotation Object Scale").icon("dim").desc("Adds or deletes supported annotation scales for annotative objects."));
    r.define(def("Annotation", "ANNOUPDATE", run(annoupdate)).title("Sync Scale Positions").icon("dim").desc("Updates annotative objects to the current annotation scale."));
    r.define(def("Annotation", "ANNORESET", run(annoreset)).title("Reset Scale Positions").icon("dim").desc("Resets annotative objects to their size at the current annotation scale."));
    r.define(def("Annotation", "SCALELISTEDIT", run(scalelistedit)).aliases(&["-SCALELISTEDIT"]).title("Scale List").icon("dim").desc("Controls the list of scales available for layout viewports, page layouts and annotation."));
}
