//! Layer, color, linetype and lineweight commands (port of `src/commands/layer.js`).
use super::{def, run};
use crate::ctx::*;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use crate::sysvars::parse_color;
use cadforge_core::drawing::Drawing;
use cadforge_core::style::{describe_color, ltype_key, Color, Layer, LINEWEIGHTS};
use std::collections::BTreeSet;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn dl(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("Layers", name, f)
}

/// Case-insensitive glob match with `*` and `?`.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti, mut star, mut mark) = (0usize, 0usize, None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Match layer names against a comma separated list with `*` and `?` wildcards.
pub fn match_layers(doc: &Drawing, spec: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        for l in &doc.layers {
            if wildcard_match(part, &l.name) && !out.contains(&l.name) {
                out.push(l.name.clone());
            }
        }
    }
    out
}

fn layer_names_of(c: &Ctx, ids: &[u64]) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for e in c.entities(ids) {
        let name = c.doc(|d| d.layer_of(&e).name.clone());
        if !v.contains(&name) {
            v.push(name);
        }
    }
    v
}

// ---------------------------------------------------------------- LAYER / -LAYER

async fn layer(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.app(|a| a.ui.dialogs) {
        c.app(|a| a.emit(crate::app::Event::Palette { name: "layers".into(), show: Some(true) }));
        Ok(())
    } else {
        c.sub_forced("-LAYER", Args::none()).await;
        Ok(())
    }
}

async fn need(c: &Ctx, prompt: &str, default: Option<&str>) -> CmdResult<String> {
    let spec = match default {
        Some(d) => ps(prompt).default_str(d),
        None => ps(prompt),
    };
    Ok(c.string(spec).await?.text)
}

const LAYER_OPTS: [&str; 17] = ["?", "Make", "Set", "New", "Rename", "ON", "OFF", "Color", "Ltype", "LWeight", "Plot", "Freeze", "Thaw", "LOck", "Unlock", "stAte", "Description"];

async fn layer_option(c: &Ctx, kw: &str) -> CmdResult<()> {
    let cur = c.doc(|d| d.cur.layer.clone());
    match kw {
        "?" => {
            let lines: Vec<String> = c.doc(|d| {
                d.layers
                    .iter()
                    .map(|l| format!("{:<24} {} {} {}  {}  {}", l.name, if l.on { "On " } else { "Off" }, if l.frozen { "Frozen" } else { "Thawed" }, if l.locked { "Locked" } else { "Unlocked" }, describe_color(Some(l.color)), l.ltype))
                    .collect()
            });
            for l in lines {
                c.log(&l);
            }
        }
        "Make" => {
            let n = need(c, "Enter name for new layer (becomes the current layer)", Some(&cur)).await?;
            if !n.is_empty() {
                c.doc_mut(|d| -> Result<(), String> {
                    let name = d.ensure_layer(&n);
                    d.set_current_layer(&name)
                })?;
            }
        }
        "Set" => {
            let n = need(c, "Enter layer name to make current", Some(&cur)).await?;
            if !n.is_empty() {
                c.doc_mut(|d| d.set_current_layer(&n))?;
            }
        }
        "New" => {
            let n = need(c, "Enter name list for new layer(s)", None).await?;
            for name in n.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                c.doc_mut(|d| d.add_layer(name).map(|_| ()))?;
            }
        }
        "Rename" => {
            let o = need(c, "Enter old layer name", None).await?;
            let n = need(c, "Enter new layer name", None).await?;
            if !o.is_empty() && !n.is_empty() {
                c.doc_mut(|d| d.rename_layer(&o, &n))?;
            }
        }
        "Description" => {
            let n = need(c, "Enter layer name(s)", None).await?;
            let desc = c.string(ps("Enter description").raw()).await?.text;
            c.doc_mut(|d| {
                d.change(|d| {
                    for name in match_layers(d, &n) {
                        if let Some(l) = d.layer_mut(&name) {
                            l.desc = desc.clone();
                        }
                    }
                })
            });
        }
        "Color" => {
            let col = need(c, "New color [Truecolor/COlorbook]", None).await?;
            let n = need(c, "Enter name list of layer(s) for color", Some(&cur)).await?;
            let v = parse_color(&col)?.unwrap_or(Color::Aci(7));
            c.doc_mut(|d| {
                d.change(|d| {
                    for name in match_layers(d, &n) {
                        if let Some(l) = d.layer_mut(&name) {
                            l.color = v;
                        }
                    }
                })
            });
        }
        "Ltype" => {
            let t = need(c, "Enter loaded linetype name or [?]", Some("Continuous")).await?;
            let k = ltype_key(&t).ok_or_else(|| format!("Linetype \"{t}\" is not loaded."))?;
            let n = need(c, "Enter name list of layer(s) for linetype", Some(&cur)).await?;
            c.doc_mut(|d| {
                d.change(|d| {
                    for name in match_layers(d, &n) {
                        if let Some(l) = d.layer_mut(&name) {
                            l.ltype = k.to_string();
                        }
                    }
                })
            });
        }
        "LWeight" => {
            let w: f64 = need(c, "Enter lineweight in millimeters (0.00 to 2.11)", None).await?.trim().parse().unwrap_or(f64::NAN);
            let hw = (w * 100.0).round() as i32;
            if !LINEWEIGHTS.contains(&hw) {
                return Err("Invalid lineweight.".into());
            }
            let n = need(c, "Enter name list of layer(s) for lineweight", Some(&cur)).await?;
            c.doc_mut(|d| {
                d.change(|d| {
                    for name in match_layers(d, &n) {
                        if let Some(l) = d.layer_mut(&name) {
                            l.lw = hw;
                        }
                    }
                })
            });
        }
        "stAte" => {
            let k = c.keyword(ps("Enter an option [?/Save/Restore/Delete]").options(&["?", "Save", "Restore", "Delete"])).await?;
            if k.kw() == Some("?") {
                let names: Vec<String> = c.doc(|d| d.settings.layer_states.as_ref().map(|s| s.keys().cloned().collect()).unwrap_or_default());
                c.log(&if names.is_empty() { "No layer states.".to_string() } else { names.join("\n") });
                return Ok(());
            }
            let n = need(c, "Enter layer state name", None).await?;
            match k.kw() {
                Some("Save") => c.doc_mut(|d| {
                    d.change(|d| {
                        let layers = d.layers.clone();
                        d.settings.layer_states.get_or_insert_with(Default::default).insert(n.clone(), layers);
                    })
                }),
                Some("Restore") => {
                    let st = c.doc(|d| d.settings.layer_states.as_ref().and_then(|s| s.get(&n).cloned()));
                    let Some(st) = st else { return Err("Layer state not found.".into()) };
                    c.doc_mut(|d| {
                        d.change(|d| {
                            for s in st {
                                if let Some(l) = d.layer_mut(&s.name) {
                                    *l = s;
                                }
                            }
                        })
                    });
                }
                _ => c.doc_mut(|d| {
                    d.change(|d| {
                        if let Some(s) = d.settings.layer_states.as_mut() {
                            s.shift_remove(&n);
                        }
                    })
                }),
            }
        }
        verb => {
            let n = need(c, &format!("Enter name list of layer(s) to {verb}"), Some(&cur)).await?;
            let names = c.doc(|d| match_layers(d, &n));
            if names.is_empty() {
                return Err(format!("No layer matched \"{n}\".").into());
            }
            let cur_now = cur.clone();
            c.doc_mut(|d| -> Result<(), String> {
                d.will_change();
                let r = (|| {
                    for name in &names {
                        let Some(l) = d.layer_mut(name) else { continue };
                        match verb {
                            "ON" => l.on = true,
                            "OFF" => l.on = false,
                            "Freeze" => {
                                if l.name == cur_now {
                                    return Err("The current layer cannot be frozen.".to_string());
                                }
                                l.frozen = true;
                            }
                            "Thaw" => l.frozen = false,
                            "LOck" => l.locked = true,
                            "Unlock" => l.locked = false,
                            "Plot" => l.plot = !l.plot,
                            _ => {}
                        }
                    }
                    Ok(())
                })();
                d.touch();
                r
            })?;
        }
    }
    Ok(())
}

async fn layer_cmdline(c: Ctx, _a: Args) -> CmdResult<()> {
    loop {
        let r = c.keyword(ps("Enter an option [?/Make/Set/New/Rename/ON/OFF/Color/Ltype/LWeight/Plot/Freeze/Thaw/LOck/Unlock/stAte/Description]").options(&LAYER_OPTS).allow_enter()).await?;
        let Some(kw) = r.kw().map(str::to_string) else { return Ok(()) };
        match layer_option(&c, &kw).await {
            Err(CmdError::Msg(m)) => c.log_error(&m),
            Err(e) => return Err(e),
            Ok(()) => {}
        }
    }
}

// ---------------------------------------------------------------- one-shot layer tools

fn set_layers(c: &Ctx, names: &[String], f: impl Fn(&mut Layer)) {
    c.doc_mut(|d| {
        d.change(|d| {
            for n in names {
                if let Some(l) = d.layer_mut(n) {
                    f(l);
                }
            }
        })
    });
}

fn all_layer_names(c: &Ctx) -> Vec<String> {
    c.doc(|d| d.layers.iter().map(|l| l.name.clone()).collect())
}

async fn pick_layer(c: &Ctx, prompt: &str) -> CmdResult<Option<String>> {
    // the layer commands act on the layer of the picked object, so a locked layer must not hide it
    let Some(id) = c.pick_entity(ps(prompt).allow_locked()).await? else { return Ok(None) };
    Ok(c.entity_by_id(id).map(|e| c.doc(|d| d.layer_of(&e).name.clone())))
}

async fn laymcur(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(n) = pick_layer(&c, "Select object whose layer will become current").await? else { return Ok(()) };
    c.doc_mut(|d| d.set_current_layer(&n))?;
    c.log(&format!("{n} is now the current layer."));
    Ok(())
}

async fn layoff(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(n) = pick_layer(&c, "Select an object on the layer to be turned off").await? else { return Ok(()) };
    set_layers(&c, &[n], |l| l.on = false);
    c.ed(|e| e.prune());
    Ok(())
}

async fn layfrz(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(n) = pick_layer(&c, "Select an object on the layer to be frozen").await? else { return Ok(()) };
    if c.doc(|d| d.cur.layer == n) {
        c.log_error("The current layer cannot be frozen.");
        return Ok(());
    }
    set_layers(&c, &[n], |l| l.frozen = true);
    Ok(())
}

async fn laylck(c: Ctx, _a: Args) -> CmdResult<()> {
    if let Some(n) = pick_layer(&c, "Select an object on the layer to be locked").await? {
        set_layers(&c, &[n], |l| l.locked = true);
    }
    c.ed(|e| e.prune());
    Ok(())
}

async fn layulk(c: Ctx, _a: Args) -> CmdResult<()> {
    if let Some(n) = pick_layer(&c, "Select an object on the layer to be unlocked").await? {
        set_layers(&c, &[n], |l| l.locked = false);
    }
    Ok(())
}

async fn layiso(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects on the layer(s) to be isolated")).await?;
    let keep: BTreeSet<String> = layer_names_of(&c, &sel.ents).into_iter().map(|n| n.to_lowercase()).collect();
    if keep.is_empty() {
        return Ok(());
    }
    let fade = c.pref("LAYLOCKFADECTL") > 0.0;
    let cur = c.doc(|d| d.cur.layer.clone());
    c.doc_mut(|d| {
        d.layer_iso = Some(d.layers.iter().map(|l| (l.name.clone(), l.on, l.locked)).collect());
        d.change(|d| {
            for l in d.layers.iter_mut() {
                if !keep.contains(&l.name.to_lowercase()) && l.name != cur {
                    if !fade {
                        l.on = false;
                    } else {
                        l.locked = true;
                    }
                }
            }
        })
    });
    let names: Vec<String> = c.doc(|d| d.layers.iter().filter(|l| keep.contains(&l.name.to_lowercase())).map(|l| l.name.clone()).collect());
    c.log(&format!("Layer{} {} isolated.", if keep.len() == 1 { "" } else { "s" }, names.join(", ")));
    c.ed(|e| e.clear_sel());
    Ok(())
}

async fn layuniso(c: Ctx, _a: Args) -> CmdResult<()> {
    let iso = c.doc_mut(|d| d.layer_iso.take());
    let Some(iso) = iso else {
        c.log("No layers are isolated.");
        return Ok(());
    };
    c.doc_mut(|d| {
        d.change(|d| {
            for (name, on, locked) in iso {
                if let Some(l) = d.layer_mut(&name) {
                    l.on = on;
                    l.locked = locked;
                }
            }
        })
    });
    Ok(())
}

async fn laycur(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects to be changed to the current layer")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let cur = c.doc(|d| d.cur.layer.clone());
    let ids = sel.ents.clone();
    let cur2 = cur.clone();
    c.doc_mut(|d| {
        d.change(|d| {
            for id in &ids {
                if let Some(e) = d.by_id_mut(*id) {
                    e.layer = cur2.clone();
                }
            }
        })
    });
    c.log(&format!("{} object{} changed to layer {cur}.", sel.ents.len(), if sel.ents.len() == 1 { "" } else { "s" }));
    Ok(())
}

async fn laymch(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects to be changed")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let Some(dest) = c.pick_entity(ps("Select object on destination layer")).await? else { return Ok(()) };
    let Some(de) = c.entity_by_id(dest) else { return Ok(()) };
    let layer = de.layer.clone();
    c.doc_mut(|d| {
        d.change(|d| {
            for id in &sel.ents {
                if let Some(e) = d.by_id_mut(*id) {
                    e.layer = layer.clone();
                }
            }
        })
    });
    Ok(())
}

async fn laydel(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(id) = c.pick_entity(ps("Select object on the layer to be deleted")).await? else { return Ok(()) };
    let Some(e) = c.entity_by_id(id) else { return Ok(()) };
    let name = e.layer.clone();
    if name == "0" || c.doc(|d| d.cur.layer == name) {
        c.log_error(&format!("Layer {name} cannot be deleted."));
        return Ok(());
    }
    let yes = c.keyword(ps(&format!("Delete layer \"{name}\" and all of its objects? [Yes/No]")).options(&["Yes", "No"]).default_str("No")).await?;
    if yes.kw() != Some("Yes") {
        return Ok(());
    }
    let low = name.to_lowercase();
    c.doc_mut(|d| {
        d.change(|d| {
            let ids: Vec<u64> = d.entities().iter().filter(|x| x.layer.to_lowercase() == low).map(|x| x.id).collect();
            d.remove_all(&ids);
            d.layers.retain(|l| l.name.to_lowercase() != low);
        })
    });
    c.ed(|e| e.prune());
    Ok(())
}

// ---------------------------------------------------------------- COLOR / LINETYPE / LWEIGHT / LTSCALE

async fn color(c: Ctx, args: Args) -> CmdResult<()> {
    if !args.flag("cmdline") {
        let cur = c.doc(|d| d.cur.color);
        if let Some(r) = c.dialog(DialogReq::Color { value: cur, allow_by_layer: true }).await? {
            if let DialogReply::Color(v) = r {
                c.doc_mut(|d| d.cur.color = v);
            }
            return Ok(());
        }
    }
    let r = c.string(ps("Enter default object color [Truecolor/COlorbook]").default_str("BYLAYER")).await?;
    let v = parse_color(&r.text)?;
    c.doc_mut(|d| d.cur.color = v);
    Ok(())
}

async fn linetype(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.dialog(DialogReq::Linetype).await?.is_some() {
        return Ok(());
    }
    let t = c.string(ps("Enter linetype name").default_str("BYLAYER")).await?;
    if t.text.eq_ignore_ascii_case("bylayer") {
        c.doc_mut(|d| d.cur.ltype = None);
    } else {
        let k = ltype_key(&t.text).ok_or_else(|| format!("Linetype \"{}\" not found.", t.text))?;
        c.doc_mut(|d| d.cur.ltype = Some(k.to_string()));
    }
    Ok(())
}

async fn lweight(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.dialog(DialogReq::Lweight).await?.is_some() {
        return Ok(());
    }
    let v = c.number(ps("Enter lineweight in mm (0.00-2.11), or -1 for ByLayer").default_num(-1.0)).await?;
    let Some(x) = v.value() else { return Ok(()) };
    if x == -1.0 {
        c.doc_mut(|d| d.cur.lw = None);
    } else {
        let hw = (x * 100.0).round() as i32;
        if !LINEWEIGHTS.contains(&hw) {
            return Err("Invalid lineweight.".into());
        }
        c.doc_mut(|d| d.cur.lw = Some(hw));
    }
    Ok(())
}

async fn ltscale(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.doc(|d| d.settings.ltscale);
    let v = c.number(ps("Enter new linetype scale factor").default_num(cur).min(1e-9)).await?;
    if let Some(x) = v.value().filter(|x| *x != 0.0) {
        c.setvar("LTSCALE", x)?;
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(dl("LAYER", run(layer)).aliases(&["LA"]).title("Layer Properties").icon("layers").desc("Manages layers and layer properties."));
    r.define(dl("-LAYER", run(layer_cmdline)).title("Layer (command line)").icon("layers").desc("Manages layers from the command line."));
    r.define(dl("LAYMCUR", run(laymcur)).title("Make Current").icon("laymcur").desc("Sets the current layer to that of a selected object."));
    r.define(dl("LAYOFF", run(layoff)).title("Layer Off").icon("layoff").desc("Turns off the layer of selected objects."));
    r.define(dl("LAYON", run(|c, _a| async move { let n = all_layer_names(&c); set_layers(&c, &n, |l| l.on = true); Ok(()) })).title("Turn All Layers On").icon("layon").desc("Turns on all layers in the drawing."));
    r.define(dl("LAYFRZ", run(layfrz)).title("Layer Freeze").icon("layfrz").desc("Freezes the layer of selected objects."));
    r.define(dl("LAYTHW", run(|c, _a| async move { let n = all_layer_names(&c); set_layers(&c, &n, |l| l.frozen = false); Ok(()) })).title("Thaw All Layers").icon("laythw").desc("Thaws all layers in the drawing."));
    r.define(dl("LAYLCK", run(laylck)).title("Layer Lock").icon("laylck").desc("Locks the layer of a selected object."));
    r.define(dl("LAYULK", run(layulk)).title("Layer Unlock").icon("layulk").desc("Unlocks the layer of a selected object."));
    r.define(dl("LAYISO", run(layiso)).title("Layer Isolate").icon("layiso").desc("Hides or locks all layers except those of the selected objects."));
    r.define(dl("LAYUNISO", run(layuniso)).title("Layer Unisolate").icon("layuniso").desc("Restores layers hidden or locked by LAYISO."));
    r.define(dl("LAYCUR", run(laycur)).title("Change to Current Layer").icon("laycur").desc("Changes the layer of selected objects to the current layer."));
    r.define(dl("LAYMCH", run(laymch)).aliases(&["LAYMATCH"]).title("Match Layer").icon("laymatch").desc("Changes the layer of selected objects to match the layer of a destination object."));
    r.define(dl("LAYDEL", run(laydel)).title("Layer Delete").icon("laydel").desc("Deletes the layers of selected objects and the objects on them."));
    r.define(dl("COLOR", run(color)).aliases(&["COL"]).title("Color").icon("color").desc("Sets the color for new objects."));
    r.define(dl("LINETYPE", run(linetype)).aliases(&["LT", "LTYPE"]).title("Linetype").icon("linetype").desc("Loads, sets, and modifies linetypes."));
    r.define(dl("LWEIGHT", run(lweight)).aliases(&["LW", "LINEWEIGHT"]).title("Lineweight").icon("lweight").desc("Sets the current lineweight, lineweight display options, and lineweight units."));
    r.define(dl("LTSCALE", run(ltscale)).aliases(&["LTS"]).title("Linetype Scale").icon("ltscale").desc("Sets the global linetype scale factor."));
}
