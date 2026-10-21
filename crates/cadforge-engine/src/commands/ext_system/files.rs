//! File, output and utility commands: AUDIT, RECOVER, CLOSEALL, SAVEALL, PREVIEW, PLOTSTYLE, PUBLISH, ETRANSMIT,
//! APPLOAD, REINIT, RECORD.
use super::{confirm, plural, ui_action};
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::Kind;
use cadforge_core::style::ltype_key;
use std::collections::{BTreeMap, HashSet};

// ---------------------------------------------------------------- AUDIT / RECOVER

/// Dangling references found in a drawing.
#[derive(Default)]
struct Findings {
    /// Layers used by objects but missing from the layer table, with the number of objects on each.
    layers: Vec<(String, usize)>,
    /// Block references to undefined blocks: (handle, block name).
    inserts: Vec<(u64, String)>,
    /// Layers with an unknown linetype: (layer, linetype).
    ltypes: Vec<(String, String)>,
    /// The current layer is not in the layer table.
    cur_layer: bool,
    audited: usize,
}

impl Findings {
    fn errors(&self) -> usize {
        self.layers.len() + self.inserts.len() + self.ltypes.len() + self.cur_layer as usize
    }
}

fn scan(d: &Drawing) -> Findings {
    let mut f = Findings::default();
    let mut missing: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut note_layer = |layer: &str| {
        if !layer.is_empty() && d.layer(layer).is_none() {
            missing.entry(layer.to_lowercase()).or_insert_with(|| (layer.to_string(), 0)).1 += 1;
        }
    };
    for e in d.all_entities() {
        f.audited += 1;
        note_layer(&e.layer);
        if let Kind::Insert(i) = &e.kind {
            if !d.blocks.contains_key(&i.name) {
                f.inserts.push((e.id, i.name.clone()));
            }
        }
    }
    for s in &d.solids {
        f.audited += 1;
        note_layer(&s.layer);
    }
    f.layers = missing.into_values().collect();
    f.ltypes = d.layers.iter().filter(|l| ltype_key(&l.ltype).is_none()).map(|l| (l.name.clone(), l.ltype.clone())).collect();
    f.cur_layer = d.layer(&d.cur.layer).is_none();
    f
}

/// Create missing layers, reset unknown linetypes and the current layer, and erase references to undefined blocks.
fn repair(d: &mut Drawing, f: &Findings) {
    d.change(|d| {
        for (name, _) in &f.layers {
            d.ensure_layer(name);
        }
        for l in d.layers.iter_mut().filter(|l| ltype_key(&l.ltype).is_none()) {
            l.ltype = "Continuous".into();
        }
        if f.cur_layer {
            d.cur.layer = "0".into();
        }
        let bad: HashSet<u64> = f.inserts.iter().map(|(id, _)| *id).collect();
        if !bad.is_empty() {
            d.model.entities.retain(|e| !bad.contains(&e.id));
            for l in d.layouts.iter_mut() {
                l.entities.retain(|e| !bad.contains(&e.id));
            }
            for b in d.blocks.values_mut() {
                b.entities.retain(|e| !bad.contains(&e.id));
            }
        }
    });
}

fn run_audit(c: &Ctx, fix: bool) {
    let f = c.doc(scan);
    c.log("Auditing Layers");
    for (l, n) in &f.layers {
        c.log(&format!("  {} on undefined layer \"{l}\"", plural(*n, "object")));
    }
    for (l, t) in &f.ltypes {
        c.log(&format!("  Layer \"{l}\" uses undefined linetype \"{t}\""));
    }
    if f.cur_layer {
        c.log("  The current layer is not defined");
    }
    c.log("Auditing Entities");
    for (id, name) in &f.inserts {
        c.log(&format!("  Block reference {id:X} refers to undefined block \"{name}\""));
    }
    let errors = f.errors();
    c.log(&format!("{} audited", plural(f.audited, "object")));
    if fix && errors > 0 {
        c.doc_mut(|d| repair(d, &f));
        c.ed(|e| e.prune());
    }
    c.log(&format!("Total errors found {errors} fixed {}", if fix { errors } else { 0 }));
}

async fn audit(c: Ctx, _a: Args) -> CmdResult<()> {
    let fix = confirm(&c, "Fix any errors detected?").await?;
    run_audit(&c, fix);
    Ok(())
}

async fn recover(c: Ctx, _a: Args) -> CmdResult<()> {
    c.log(&format!("Recovering \"{}\"", c.doc(|d| d.name.clone())));
    run_audit(&c, true);
    Ok(())
}

// ---------------------------------------------------------------- all drawings

async fn closeall(c: Ctx, _a: Args) -> CmdResult<()> {
    ui_action(&c, "CLOSEALL", "QUIT", Args::none());
    Ok(())
}

async fn saveall(c: Ctx, _a: Args) -> CmdResult<()> {
    let (active_dirty, others): (bool, Vec<String>) = c.app(|a| (a.doc().is_some_and(|d| d.dirty), a.editors.iter().enumerate().filter(|(i, e)| Some(*i) != a.active && e.doc.dirty).map(|(_, e)| e.doc.name.clone()).collect()));
    if !active_dirty && others.is_empty() {
        c.log("All drawings are saved.");
        return Ok(());
    }
    if active_dirty {
        ui_action(&c, "SAVEALL", "QSAVE", Args::none());
    }
    if !others.is_empty() {
        c.log_error(&format!("Not saved: {}. SAVEALL saves only the active drawing in this version; make each drawing active and use QSAVE.", others.join(", ")));
    }
    Ok(())
}

// ---------------------------------------------------------------- output

async fn preview(c: Ctx, _a: Args) -> CmdResult<()> {
    // the Plot dialog shows the sheet preview
    ui_action(&c, "PREVIEW", "PLOT", Args::none().with("preview", ArgVal::Bool(true)));
    Ok(())
}

async fn plotstyle(c: Ctx, _a: Args) -> CmdResult<()> {
    c.log_error("Plot styles are not supported in this version; objects plot with their own colour, linetype and lineweight.");
    Ok(())
}

async fn publish(c: Ctx, _a: Args) -> CmdResult<()> {
    c.log_error("Publishing several sheets in one job is not supported in this version.");
    if c.app(|a| a.ui.dialogs) {
        c.log("Opening the PDF export for the current layout instead.");
        ui_action(&c, "PUBLISH", "EXPORTPDF", Args::none());
    }
    Ok(())
}

async fn etransmit(c: Ctx, _a: Args) -> CmdResult<()> {
    let xrefs: Vec<String> = c.doc(|d| d.blocks.values().filter_map(|b| b.xref.as_ref().map(|x| format!("  {} -> {}{}", b.name, x.path, if x.overlay { " (overlay)" } else { "" }))).collect());
    c.log("A transmittal of this drawing would need these files:");
    c.log("  External references:");
    c.log(&if xrefs.is_empty() { "  none".to_string() } else { xrefs.join("\n") });
    c.log("  Fonts: none (text uses the built-in font)");
    c.log_error("Creating a transmittal package is not supported in this version; copy the files listed above by hand.");
    Ok(())
}

// ---------------------------------------------------------------- utilities

async fn appload(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.app(|a| a.ui.dialogs) {
        c.log("Choose an AutoLISP (.lsp) or script (.scr) file to load.");
        c.app(|a| a.emit(Event::UiAction { name: "APPLOAD".into(), args: Args::none() }));
    } else {
        c.log_error("APPLOAD needs a file chooser; type AutoLISP expressions at the command line instead, or run a script with SCRIPT.");
    }
    Ok(())
}

async fn reinit(c: Ctx, _a: Args) -> CmdResult<()> {
    let aliases = c.app(|a| a.registry.all_aliases().len() + a.registry.user_aliases.borrow().len());
    c.log("There are no digitizer or I/O ports to reinitialize.");
    c.log(&format!("Command aliases (the PGP file) are always current: {aliases} are in effect; edit them with ALIASEDIT."));
    Ok(())
}

async fn record(c: Ctx, _a: Args) -> CmdResult<()> {
    c.log_error("The Action Recorder is not supported in this version: command input cannot be captured. Use SCRIPT to run a script file.");
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("File", "AUDIT", run(audit)).title("Audit").desc("Evaluates the integrity of a drawing and corrects some errors.").once());
    r.define(def("File", "RECOVER", run(recover)).title("Recover").desc("Repairs the open drawing: undefined layers, linetypes and block references.").once());
    r.define(def("File", "CLOSEALL", run(closeall)).title("Close All").icon("close").desc("Closes all currently open drawings.").once());
    r.define(def("Utilities", "SAVEALL", run(saveall)).title("Save All").icon("save").desc("Saves all open drawings.").once());
    r.define(def("Output", "PREVIEW", run(preview)).aliases(&["PRE"]).title("Plot Preview").icon("plot").desc("Displays the drawing as it will be plotted.").once());
    r.define(def("Output", "PLOTSTYLE", run(plotstyle)).title("Plot Style").desc("Sets the plot style for new objects.").once());
    r.define(def("Output", "PUBLISH", run(publish)).title("Publish").icon("pdf").desc("Publishes drawings to PDF or plotters.").once());
    r.define(def("Output", "ETRANSMIT", run(etransmit)).title("eTransmit").desc("Lists the files needed to send a drawing to someone else.").once());
    r.define(def("Utilities", "APPLOAD", run(appload)).aliases(&["AP"]).title("Load Application").desc("Loads applications and script files.").once());
    r.define(def("Utilities", "REINIT", run(reinit)).title("Reinitialize").desc("Reinitializes the digitizer, the digitizer input/output port, and the program parameters file.").once());
    r.define(def("Utilities", "RECORD", run(record)).title("Action Recorder").desc("Starts the Action Recorder.").once());
}
