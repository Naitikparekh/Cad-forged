//! Layer tools: LAYERP, LAYMRG, LAYTRANS, LAYWALK, LAYERPALL, LAYVPI, LAYERCLOSE.
use super::{confirm, plural, ps, Shared};
use crate::app::Event;
use crate::commands::layer::match_layers;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::Kind;
use cadforge_core::style::Layer;

fn layer_name_of(c: &Ctx, id: u64) -> Option<String> {
    c.entity_by_id(id).map(|e| c.doc(|d| d.layer_of(&e).name.clone()))
}

fn valid_layer_name(n: &str) -> bool {
    !n.is_empty() && !n.chars().any(|c| "<>/\\\":;?*|,=`".contains(c))
}

/// Ask for layer names (wildcards allowed); `?` lists the layers. Empty when Enter is pressed.
async fn ask_layers(c: &Ctx, prompt: &str) -> CmdResult<Vec<String>> {
    loop {
        let t = c.string(ps(prompt)).await?.text;
        if t.is_empty() {
            return Ok(vec![]);
        }
        if t == "?" {
            let names: Vec<String> = c.doc(|d| d.layers.iter().map(|l| l.name.clone()).collect());
            c.log(&names.join("\n"));
            continue;
        }
        let found = c.doc(|d| match_layers(d, &t));
        if found.is_empty() {
            c.log_error(&format!("No layer matched \"{t}\"."));
            continue;
        }
        return Ok(found);
    }
}

// ---------------------------------------------------------------- LAYERP

/// Put back the saved properties of every layer that still exists (renamed, deleted and new layers are left alone).
/// Returns true when the current layer had to stay thawed.
fn restore_layers(d: &mut Drawing, saved: &[Layer]) -> bool {
    let mut kept_thawed = false;
    d.change(|d| {
        let cur = d.cur.layer.to_lowercase();
        for s in saved {
            let Some(l) = d.layer_mut(&s.name) else { continue };
            *l = Layer { name: l.name.clone(), ..s.clone() };
            if l.frozen && l.name.to_lowercase() == cur {
                l.frozen = false;
                kept_thawed = true;
            }
        }
    });
    kept_thawed
}

async fn layerp(st: Shared, c: Ctx, _a: Args) -> CmdResult<()> {
    let id = c.ed(|e| e.id);
    let saved = st.borrow_mut().docs.get_mut(&id).and_then(|d| d.history.pop());
    let Some(saved) = saved else {
        c.log_error("No previous layer settings to restore.");
        return Ok(());
    };
    let kept_thawed = c.doc_mut(|d| restore_layers(d, &saved));
    // the restored table is the new baseline, not a change to remember
    let (rev, layers) = c.doc(|d| (d.rev, d.layers.clone()));
    if let Some(t) = st.borrow_mut().docs.get_mut(&id) {
        t.rev = rev;
        t.layers = layers;
    }
    c.log("Previous layer settings restored.");
    if kept_thawed {
        c.log("The current layer cannot be frozen and was left thawed.");
    }
    Ok(())
}

// ---------------------------------------------------------------- LAYMRG / LAYTRANS

/// Move every object on the `sources` layers to `target` and delete the source layers. Returns the objects moved.
fn merge_layers(d: &mut Drawing, sources: &[String], target: &str) -> usize {
    let low: Vec<String> = sources.iter().map(|s| s.to_lowercase()).collect();
    let mut moved = 0;
    d.change(|d| {
        for e in d.all_entities_mut().filter(|e| low.contains(&e.layer.to_lowercase())) {
            e.layer = target.to_string();
            moved += 1;
        }
        for s in d.solids.iter_mut().filter(|s| low.contains(&s.layer.to_lowercase())) {
            s.layer = target.to_string();
            moved += 1;
        }
        if low.contains(&d.cur.layer.to_lowercase()) {
            d.cur.layer = target.to_string();
        }
        d.layers.retain(|l| !low.contains(&l.name.to_lowercase()));
    });
    moved
}

fn quoted(names: &[String]) -> String {
    names.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")
}

async fn laymrg(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut sources: Vec<String> = vec![];
    loop {
        let opts: &[&str] = if sources.is_empty() { &["Name"] } else { &["Name", "Undo"] };
        match c.entity(ps("Select object on layer to merge or").options(opts)).await? {
            EntR::Entity { id, .. } => {
                if let Some(n) = layer_name_of(&c, id) {
                    if n == "0" {
                        c.log_error("Layer \"0\" cannot be merged.");
                    } else if !sources.contains(&n) {
                        sources.push(n);
                    }
                }
            }
            EntR::Kw(k) if k == "Name" => {
                for n in ask_layers(&c, "Enter name of layer to merge or [?]").await? {
                    if n == "0" {
                        c.log_error("Layer \"0\" cannot be merged.");
                    } else if !sources.contains(&n) {
                        sources.push(n);
                    }
                }
            }
            EntR::Kw(_) => {
                sources.pop();
            }
            EntR::Enter => break,
        }
        if !sources.is_empty() {
            c.log(&format!("Selected layers: {}", sources.join(", ")));
        }
    }
    if sources.is_empty() {
        return Ok(());
    }
    let target = loop {
        match c.entity(ps("Select object on target layer or").options(&["Name"])).await? {
            EntR::Entity { id, .. } => {
                if let Some(n) = layer_name_of(&c, id) {
                    break n;
                }
            }
            EntR::Kw(_) => match ask_layers(&c, "Enter name of target layer or [?]").await?.as_slice() {
                [n] => break n.clone(),
                [] => {}
                _ => c.log_error("Enter the name of a single target layer."),
            },
            EntR::Enter => return Ok(()),
        }
    };
    sources.retain(|s| !s.eq_ignore_ascii_case(&target));
    if sources.is_empty() {
        c.log("Nothing to merge: the selected objects are already on that layer.");
        return Ok(());
    }
    let q = format!("Merge layer{} {} to layer \"{target}\"?", if sources.len() == 1 { "" } else { "s" }, quoted(&sources));
    if !confirm(&c, &q).await? {
        return Ok(());
    }
    let moved = c.doc_mut(|d| merge_layers(d, &sources, &target));
    c.log(&format!("{} merged to layer \"{target}\" ({} moved).", plural(sources.len(), "layer"), plural(moved, "object")));
    c.ed(|e| e.prune());
    Ok(())
}

async fn laytrans(c: Ctx, _a: Args) -> CmdResult<()> {
    let mut map: Vec<(String, String)> = vec![];
    loop {
        let k = c.keyword(ps("Enter an option").options(&["Map", "List", "Remove", "Translate"]).default_str("Translate")).await?;
        match k.kw() {
            Some("Map") => {
                let from = ask_layers(&c, "Enter name of layer(s) to translate (wildcards allowed)").await?;
                if from.is_empty() {
                    continue;
                }
                let to = c.string(ps("Enter name of the layer to translate to")).await?.text;
                if !valid_layer_name(&to) {
                    c.log_error("Invalid layer name.");
                    continue;
                }
                for f in from {
                    if f == "0" {
                        c.log_error("Layer \"0\" cannot be translated.");
                    } else if !f.eq_ignore_ascii_case(&to) {
                        map.retain(|(s, _)| *s != f);
                        map.push((f, to.clone()));
                    }
                }
            }
            Some("List") if map.is_empty() => c.log("No layer mappings defined."),
            Some("List") => c.log(&map.iter().map(|(a, b)| format!("  {a} -> {b}")).collect::<Vec<_>>().join("\n")),
            Some("Remove") => {
                let names = ask_layers(&c, "Enter name of layer(s) whose mapping to remove").await?;
                map.retain(|(s, _)| !names.contains(s));
            }
            _ => break,
        }
    }
    if map.is_empty() {
        c.log("No layer mappings defined.");
        return Ok(());
    }
    let (done, errors) = c.doc_mut(|d| {
        d.change(|d| {
            let (mut done, mut errors) = (0, vec![]);
            for (from, to) in &map {
                if d.layer(from).is_none() {
                    continue; // already consumed by an earlier mapping
                }
                match d.layer(to).map(|l| l.name.clone()) {
                    Some(t) => {
                        merge_layers(d, std::slice::from_ref(from), &t);
                    }
                    None => {
                        if let Err(e) = d.rename_layer(from, to) {
                            errors.push(e);
                            continue;
                        }
                    }
                }
                done += 1;
            }
            (done, errors)
        })
    });
    for e in errors {
        c.log_error(&e);
    }
    c.log(&format!("{} translated.", plural(done, "layer")));
    c.ed(|e| e.prune());
    Ok(())
}

// ---------------------------------------------------------------- LAYWALK

fn objects_on(d: &Drawing, layer: &str) -> usize {
    d.entities().iter().filter(|e| e.layer.eq_ignore_ascii_case(layer)).count() + d.solids.iter().filter(|s| s.layer.eq_ignore_ascii_case(layer)).count()
}

/// Switch off every layer except `shown`.
fn show_only(c: &Ctx, shown: &[String]) {
    c.doc_mut(|d| {
        d.change(|d| {
            for l in d.layers.iter_mut() {
                l.on = shown.iter().any(|s| s.eq_ignore_ascii_case(&l.name));
            }
        })
    });
    c.emit();
}

async fn walk(c: &Ctx, names: &[String]) -> CmdResult<()> {
    let mut i = 0;
    let mut shown = vec![names[0].clone()];
    loop {
        show_only(c, &shown);
        let n: usize = c.doc(|d| shown.iter().map(|s| objects_on(d, s)).sum());
        c.log(&format!("Layer {}: {} displayed.", quoted(&shown), plural(n, "object")));
        let k = c.keyword(ps("Enter an option").options(&["Next", "Previous", "Go", "eXit"]).default_str("Next")).await?;
        match k.kw() {
            Some("Next") => {
                i = (i + 1) % names.len();
                shown = vec![names[i].clone()];
            }
            Some("Previous") => {
                i = (i + names.len() - 1) % names.len();
                shown = vec![names[i].clone()];
            }
            Some("Go") => {
                let v = ask_layers(c, "Enter name of layer(s) to display or [?]").await?;
                if !v.is_empty() {
                    shown = v;
                }
            }
            _ => return Ok(()),
        }
    }
}

/// Display one layer at a time; the layers' on/off states come back when the command ends.
async fn laywalk(c: Ctx, _a: Args) -> CmdResult<()> {
    let names: Vec<String> = c.doc(|d| d.layers.iter().filter(|l| !l.frozen).map(|l| l.name.clone()).collect());
    if names.is_empty() {
        c.log_error("There are no thawed layers to walk through.");
        return Ok(());
    }
    let saved: Vec<(String, bool)> = c.doc(|d| d.layers.iter().map(|l| (l.name.clone(), l.on)).collect());
    let res = walk(&c, &names).await;
    c.doc_mut(|d| {
        d.change(|d| {
            for (n, on) in &saved {
                if let Some(l) = d.layer_mut(n) {
                    l.on = *on;
                }
            }
        })
    });
    c.emit();
    res
}

// ---------------------------------------------------------------- viewport overrides, layer manager

/// Layout viewports in the drawing. Viewports carry no per-viewport layer settings in this version.
fn viewport_count(c: &Ctx) -> usize {
    c.doc(|d| d.layouts.iter().flat_map(|l| l.entities.iter()).filter(|e| matches!(e.kind, Kind::Viewport(_))).count())
}

async fn layerpall(c: Ctx, _a: Args) -> CmdResult<()> {
    match viewport_count(&c) {
        0 => c.log_error("There are no layout viewports."),
        n => c.log_error(&format!("Viewport layer overrides are not supported in this version; nothing to reset in {}.", plural(n, "viewport"))),
    }
    Ok(())
}

async fn layvpi(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.doc(|d| d.is_model()) {
        c.log_error("LAYVPI is available only in a layout viewport.");
    } else {
        c.log_error("Viewport layer overrides are not supported in this version; use LAYISO to isolate layers in the whole drawing.");
    }
    Ok(())
}

async fn layerclose(c: Ctx, _a: Args) -> CmdResult<()> {
    c.app(|a| a.emit(Event::Palette { name: "layers".into(), show: Some(false) }));
    Ok(())
}

pub(super) fn register(r: &mut Registry, st: &Shared) {
    let s = st.clone();
    r.define(def("Layers", "LAYERP", run(move |c, a| layerp(s.clone(), c, a))).title("Layer Previous").icon("layers").desc("Undoes the last change or set of changes made to layer settings."));
    r.define(def("Layers", "LAYMRG", run(laymrg)).title("Layer Merge").icon("layers").desc("Merges selected layers into a target layer, removing the previous layers from the drawing."));
    r.define(def("Layers", "LAYTRANS", run(laytrans)).title("Layer Translator").icon("layers").desc("Changes layers to the layer names you specify and merges layers that map to the same name."));
    r.define(def("Layers", "LAYWALK", run(laywalk)).title("Layer Walk").icon("layers").desc("Displays objects on selected layers and hides objects on all other layers."));
    r.define(def("Layers", "LAYERPALL", run(layerpall)).title("Reset All Viewport Layer Overrides").icon("layers").desc("Resets the layer property overrides in all layout viewports."));
    r.define(def("Layers", "LAYVPI", run(layvpi)).title("Layer Viewport Isolate").icon("layiso").desc("Isolates the layer of a selected object in the current layout viewport."));
    r.define(def("Utilities", "LAYERCLOSE", run(layerclose)).title("Close Layer Manager").icon("layers").desc("Closes the Layer Properties Manager.").transparent().once());
}
