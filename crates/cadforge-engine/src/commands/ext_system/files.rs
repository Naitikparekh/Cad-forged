//! File, output and utility commands: AUDIT, RECOVER, CLOSEALL, SAVEALL, PREVIEW, -DXFOUT, -SAVEAS, PLOTSTYLE, PUBLISH,
//! ETRANSMIT, APPLOAD, REINIT, RECORD.
use super::{confirm, plural, ps, ui_action};
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
            missing
                .entry(layer.to_lowercase())
                .or_insert_with(|| (layer.to_string(), 0))
                .1 += 1;
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
    f.ltypes = d
        .layers
        .iter()
        .filter(|l| ltype_key(&l.ltype).is_none())
        .map(|l| (l.name.clone(), l.ltype.clone()))
        .collect();
    f.cur_layer = d.layer(&d.cur.layer).is_none();
    f
}

/// Create missing layers, reset unknown linetypes and the current layer, and erase references to undefined blocks.
fn repair(d: &mut Drawing, f: &Findings) {
    d.change(|d| {
        for (name, _) in &f.layers {
            d.ensure_layer(name);
        }
        for l in d
            .layers
            .iter_mut()
            .filter(|l| ltype_key(&l.ltype).is_none())
        {
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
        c.log(&format!(
            "  {} on undefined layer \"{l}\"",
            plural(*n, "object")
        ));
    }
    for (l, t) in &f.ltypes {
        c.log(&format!("  Layer \"{l}\" uses undefined linetype \"{t}\""));
    }
    if f.cur_layer {
        c.log("  The current layer is not defined");
    }
    c.log("Auditing Entities");
    for (id, name) in &f.inserts {
        c.log(&format!(
            "  Block reference {id:X} refers to undefined block \"{name}\""
        ));
    }
    let errors = f.errors();
    c.log(&format!("{} audited", plural(f.audited, "object")));
    if fix && errors > 0 {
        c.doc_mut(|d| repair(d, &f));
        c.ed(|e| e.prune());
    }
    c.log(&format!(
        "Total errors found {errors} fixed {}",
        if fix { errors } else { 0 }
    ));
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
    let (active_dirty, others): (bool, Vec<String>) = c.app(|a| {
        (
            a.doc().is_some_and(|d| d.dirty),
            a.editors
                .iter()
                .enumerate()
                .filter(|(i, e)| Some(*i) != a.active && e.doc.dirty)
                .map(|(_, e)| e.doc.name.clone())
                .collect(),
        )
    });
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
    ui_action(
        &c,
        "PREVIEW",
        "PLOT",
        Args::none().with("preview", ArgVal::Bool(true)),
    );
    Ok(())
}

/// -DXFOUT: write the drawing as a DXF file, as text or (Binary) packed. The decimal places AutoCAD asks about first are
/// not offered: numbers are always written in full.
async fn dxfout(c: Ctx, a: Args) -> CmdResult<()> {
    let fmt = match a.str("format") {
        Some(f) => f.to_string(),
        None => c
            .keyword(
                ps("Enter file format [ASCII/Binary]")
                    .options(&["ASCII", "Binary"])
                    .default_str("ASCII"),
            )
            .await?
            .kw()
            .unwrap_or("ASCII")
            .to_string(),
    };
    let binary = fmt.eq_ignore_ascii_case("binary");
    let (name, data) = c.doc(|d| {
        (
            d.name.clone(),
            if binary {
                cadforge_core::io::write_dxf_binary(d)
            } else {
                cadforge_core::io::write_dxf(d).into_bytes()
            },
        )
    });
    let file = format!("{name}.dxf");
    c.log(&format!(
        "Wrote {file} ({} DXF, {} bytes).",
        if binary { "binary" } else { "ASCII" },
        data.len()
    ));
    if let Some(n) = c.doc(|d| SaveFormat::Dxf.not_kept_note(d)) {
        c.log_error(&n);
    }
    c.app(|app| {
        app.emit(Event::SaveFile {
            name: file,
            data,
            mime: "application/dxf".into(),
        })
    });
    Ok(())
}

/// A file format the drawing can be saved in.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SaveFormat {
    Dwg2004,
    Dwg2000,
    Dxf,
    CadForge,
}

impl SaveFormat {
    /// The format a file name asks for (DWG files in the drawing's DWG version).
    pub fn of(name: &str, d: &Drawing) -> SaveFormat {
        let l = name.to_lowercase();
        if l.ends_with(".dwg") {
            if d.dwg_version == Some(cadforge_core::io::dwg::Ver::R2000) {
                SaveFormat::Dwg2000
            } else {
                SaveFormat::Dwg2004
            }
        } else if l.ends_with(".dxf") {
            SaveFormat::Dxf
        } else {
            SaveFormat::CadForge
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            SaveFormat::Dwg2004 | SaveFormat::Dwg2000 => ".dwg",
            SaveFormat::Dxf => ".dxf",
            SaveFormat::CadForge => cadforge_core::io::FILE_EXT,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SaveFormat::Dwg2004 => "AutoCAD 2004 Drawing (*.dwg)",
            SaveFormat::Dwg2000 => "AutoCAD 2000 Drawing (*.dwg)",
            SaveFormat::Dxf => "AutoCAD DXF (*.dxf)",
            SaveFormat::CadForge => "CadForge Drawing (*.json)",
        }
    }

    pub const ALL: [SaveFormat; 4] = [
        SaveFormat::Dwg2004,
        SaveFormat::Dwg2000,
        SaveFormat::Dxf,
        SaveFormat::CadForge,
    ];

    /// What a file of this format cannot hold of the drawing (nothing for a CadForge project).
    pub fn not_kept(self, d: &Drawing) -> Vec<String> {
        match self {
            SaveFormat::CadForge => vec![],
            f => cadforge_core::io::not_kept_in_dxf(d, f == SaveFormat::Dwg2000),
        }
    }

    /// One line for the command line, or None when the file holds everything.
    pub fn not_kept_note(self, d: &Drawing) -> Option<String> {
        let n = self.not_kept(d);
        (!n.is_empty()).then(|| {
            format!(
                "Not kept in {}: {}. Save a CadForge project (*.json) to keep everything.",
                self.short(),
                n.join("; ")
            )
        })
    }

    fn short(self) -> &'static str {
        match self {
            SaveFormat::Dwg2004 | SaveFormat::Dwg2000 => "DWG",
            SaveFormat::Dxf => "DXF",
            SaveFormat::CadForge => "CadForge",
        }
    }

    /// The file's bytes and media type; the drawing remembers the DWG version for the next save.
    pub fn encode(self, d: &mut Drawing) -> Result<(Vec<u8>, &'static str), String> {
        use cadforge_core::io::dwg::{write_dwg_as, Ver};
        Ok(match self {
            SaveFormat::Dwg2004 | SaveFormat::Dwg2000 => {
                let ver = if self == SaveFormat::Dwg2000 {
                    Ver::R2000
                } else {
                    Ver::R2004
                };
                let bytes = write_dwg_as(d, ver)?;
                d.dwg_version = (ver == Ver::R2000).then_some(ver);
                (bytes, "image/vnd.dwg")
            }
            SaveFormat::Dxf => (
                cadforge_core::io::write_dxf(d).into_bytes(),
                "application/dxf",
            ),
            SaveFormat::CadForge => (
                cadforge_core::io::to_json(d).into_bytes(),
                "application/json",
            ),
        })
    }
}

/// The file name with the format's extension (an existing drawing extension is replaced).
pub fn with_ext(name: &str, fmt: SaveFormat) -> String {
    format!("{}{}", cadforge_core::io::strip_ext(name.trim()), fmt.ext())
}

/// -SAVEAS: save the drawing under a new name in a chosen format, from the command line (AutoCAD's command-line
/// form). DWG files are written for AutoCAD 2004 or 2000.
async fn saveas_cli(c: Ctx, a: Args) -> CmdResult<()> {
    let (name, cur) = c.doc(|d| {
        (
            d.file_name
                .clone()
                .unwrap_or_else(|| format!("{}.dwg", d.name)),
            SaveFormat::of(&d.file_name.clone().unwrap_or_else(|| "x.dwg".into()), d),
        )
    });
    let key = |f: SaveFormat| match f {
        SaveFormat::Dwg2004 => "2004",
        SaveFormat::Dwg2000 => "2000",
        SaveFormat::Dxf => "DXF",
        SaveFormat::CadForge => "CadForge",
    };
    c.log(&format!("Current file format: {}", cur.label()));
    let fmt_kw = match a.str("format") {
        Some(f) => f.to_string(),
        None => c
            .keyword(
                ps("Enter file format [2000/2004/DXF/CadForge]")
                    .options(&["2000", "2004", "DXF", "CadForge"])
                    .default_str(key(cur)),
            )
            .await?
            .kw()
            .unwrap_or(key(cur))
            .to_string(),
    };
    let fmt = match fmt_kw.to_lowercase().as_str() {
        "2000" => SaveFormat::Dwg2000,
        "dxf" => SaveFormat::Dxf,
        "cadforge" | "json" => SaveFormat::CadForge,
        _ => SaveFormat::Dwg2004,
    };
    let default = with_ext(&name, fmt);
    let typed = match a.str("name") {
        Some(n) => n.to_string(),
        None => {
            let r = c
                .string(ps(&format!("Save drawing as <{default}>")).default_str(&default))
                .await?;
            if r.text.trim().is_empty() {
                default.clone()
            } else {
                r.text
            }
        }
    };
    let file = with_ext(&typed, fmt);
    let note = c.doc(|d| fmt.not_kept_note(d));
    let out = c.doc_mut(|d| {
        let r = fmt.encode(d);
        if r.is_ok() {
            d.file_name = Some(file.clone());
            d.name = cadforge_core::io::strip_ext(&file);
            d.dirty = false;
        }
        r
    });
    match out {
        Ok((data, mime)) => {
            c.log(&format!(
                "Saved {file} ({}, {} bytes).",
                fmt.label(),
                data.len()
            ));
            if let Some(n) = note {
                c.log_error(&n);
            }
            c.app(|app| {
                app.emit(Event::SaveFile {
                    name: file,
                    data,
                    mime: mime.into(),
                });
                app.emit(Event::Change);
            });
        }
        Err(e) => c.log_error(&format!("Could not save {file}: {e}")),
    }
    Ok(())
}

/// PLOTSTYLE: the plot style table the drawing plots with (a `.ctb` colour dependent table decides the colour, lineweight
/// and screening of what it covers). Named `.stb` tables can be loaded and listed but objects cannot be assigned
/// to their styles yet, so they change nothing when plotting.
async fn plotstyle(c: Ctx, a: Args) -> CmdResult<()> {
    use cadforge_core::plotstyle::{describe, parse_base64, PlotStyleTable};
    let opt = match a.str("option") {
        Some(o) => o.to_string(),
        None => c
            .keyword(
                ps("Enter an option [?/Load/Monochrome/None]")
                    .options(&["?", "Load", "Monochrome", "None"])
                    .default_str("?"),
            )
            .await?
            .kw()
            .unwrap_or("?")
            .to_string(),
    };
    match opt.as_str() {
        "Load" => {
            let Some(data) = a.str("data") else {
                if c.app(|app| app.ui.dialogs) {
                    c.app(|app| app.emit(Event::UiAction { name: "PLOTSTYLELOAD".into(), args: Args::none() }));
                } else {
                    c.log_error("Loading a plot style table needs a file chooser; pass the file's bytes, base64 encoded, as the \"data\" argument instead.");
                }
                return Ok(());
            };
            let name = a.str("name").unwrap_or("plot styles").to_string();
            let name = name.rsplit(['/', '\\']).next().unwrap_or(&name).to_string();
            let name = name.rsplit_once('.').map_or(name.clone(), |(stem, _)| stem.to_string());
            match parse_base64(&name, data) {
                Ok(t) => {
                    let kind = if t.color_based { "colour dependent" } else { "named" };
                    let n = t.styles.len();
                    c.doc_mut(|d| d.change(|d| d.plot_styles = Some(std::rc::Rc::new(t))));
                    c.log(&format!("Plot style table \"{name}\" ({kind}, {}) will be used when plotting.", plural(n, "style")));
                    if !c.doc(|d| d.plot_styles.as_ref().is_some_and(|t| t.color_based)) {
                        c.log_error("A named table cannot be applied yet: objects have no plot style assignment. Colour dependent (.ctb) tables work.");
                    }
                }
                Err(e) => c.log_error(&e),
            }
        }
        "Monochrome" => {
            c.doc_mut(|d| d.change(|d| d.plot_styles = Some(std::rc::Rc::new(PlotStyleTable::monochrome()))));
            c.log("Everything will plot in black (table \"monochrome\").");
        }
        "None" => {
            let had = c.doc(|d| d.plot_styles.is_some());
            if had {
                c.doc_mut(|d| d.change(|d| d.plot_styles = None));
            }
            c.log(if had { "The plot style table was removed: objects plot with their own colour, linetype and lineweight." } else { "The drawing has no plot style table." });
        }
        _ => match c.doc(|d| d.plot_styles.clone()) {
            None => c.log("No plot style table: objects plot with their own colour, linetype and lineweight (Load one, or choose Monochrome)."),
            Some(t) => {
                c.log(&format!("Plot style table \"{}\": {}, {}. {}", t.name, if t.color_based { "colour dependent" } else { "named" }, plural(t.styles.len(), "style"), t.description));
                if t.color_based {
                    // the colours that change something
                    let mut shown = 0;
                    for (i, s) in t.styles.iter().enumerate() {
                        if (s.color.is_some() || s.lineweight.is_some() || s.screen != 100) && shown < 24 {
                            c.log(&format!("  colour {:>3}: {}", i + 1, describe(&t, (i + 1) as u8)));
                            shown += 1;
                        }
                    }
                    if shown == 0 {
                        c.log("  every colour plots as itself");
                    }
                }
            }
        },
    }
    Ok(())
}

/// PUBLISH: every sheet of the drawing (model space and each layout) as one multi-page PDF, handed to the host as a file.
async fn publish(c: Ctx, a: Args) -> CmdResult<()> {
    let layouts = c.doc(|d| d.layout_names());
    let which = match a.str("sheets") {
        Some(k) => k.to_string(),
        None if layouts.is_empty() => "Model".to_string(),
        None => c
            .keyword(
                ps("Sheets to publish [Model/Layouts/All]")
                    .options(&["Model", "Layouts", "All"])
                    .default_str("All"),
            )
            .await?
            .kw()
            .unwrap_or("All")
            .to_string(),
    };
    let mut sheets: Vec<Option<String>> = vec![];
    if which == "Model" || which == "All" {
        sheets.push(None);
    }
    if which == "Layouts" || which == "All" {
        sheets.extend(layouts.iter().cloned().map(Some));
    }
    let (name, doc_title) = c.doc(|d| (d.name.clone(), d.name.clone()));
    let result = c.doc(|d| {
        let mut o = cadforge_render::default_plot_options(d);
        o.title = doc_title.clone();
        cadforge_render::publish_pdf(d, &o, &sheets)
    });
    match result {
        Err(e) => c.log_error(&e),
        Ok((data, pages)) => {
            let stem: String = name
                .trim_end_matches(".json")
                .trim_end_matches(".dxf")
                .to_string();
            let file = format!("{}.pdf", if stem.is_empty() { "drawing" } else { &stem });
            c.log(&format!("Published {} to {file}.", plural(pages, "sheet")));
            c.app(|app| {
                app.emit(Event::SaveFile {
                    name: file,
                    data,
                    mime: "application/pdf".into(),
                })
            });
        }
    }
    Ok(())
}

/// ETRANSMIT: the drawing and the pictures it needs in one ZIP, with a report that lists what could not be included
/// (referenced drawings are only taken from open drawings, so they are named in the report).
async fn etransmit(c: Ctx, _a: Args) -> CmdResult<()> {
    crate::commands::ext_tail::archive::send_package(&c, "transmittal", "Transmittal").await
}

// ---------------------------------------------------------------- utilities

async fn appload(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.app(|a| a.ui.dialogs) {
        c.log("Choose an AutoLISP (.lsp) or script (.scr) file to load.");
        c.app(|a| {
            a.emit(Event::UiAction {
                name: "APPLOAD".into(),
                args: Args::none(),
            })
        });
    } else {
        c.log_error("APPLOAD needs a file chooser; type AutoLISP expressions at the command line instead, or run a script with SCRIPT.");
    }
    Ok(())
}

async fn reinit(c: Ctx, _a: Args) -> CmdResult<()> {
    let aliases =
        c.app(|a| a.registry.all_aliases().len() + a.registry.user_aliases.borrow().len());
    c.log("There are no digitizer or I/O ports to reinitialize.");
    c.log(&format!("Command aliases (the PGP file) are always current: {aliases} are in effect; edit them with ALIASEDIT."));
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(
        def("File", "AUDIT", run(audit))
            .title("Audit")
            .desc("Evaluates the integrity of a drawing and corrects some errors.")
            .once(),
    );
    r.define(
        def("File", "RECOVER", run(recover))
            .title("Recover")
            .desc("Repairs the open drawing: undefined layers, linetypes and block references.")
            .once(),
    );
    r.define(
        def("File", "CLOSEALL", run(closeall))
            .title("Close All")
            .icon("close")
            .desc("Closes all currently open drawings.")
            .once(),
    );
    r.define(
        def("Utilities", "SAVEALL", run(saveall))
            .title("Save All")
            .icon("save")
            .desc("Saves all open drawings.")
            .once(),
    );
    r.define(
        def("Output", "PREVIEW", run(preview))
            .aliases(&["PRE"])
            .title("Plot Preview")
            .icon("plot")
            .desc("Displays the drawing as it will be plotted.")
            .once(),
    );
    r.define(
        def("File", "-DXFOUT", run(dxfout))
            .title("Export DXF (options)")
            .icon("dxf")
            .desc("Writes the drawing as an ASCII or binary DXF file.")
            .once(),
    );
    r.define(def("File", "-SAVEAS", run(saveas_cli)).title("Save As (command line)").icon("saveas").desc("Saves the drawing under a new name as an AutoCAD 2004 or 2000 DWG, a DXF or a CadForge file.").once());
    r.define(def("Output", "PLOTSTYLE", run(plotstyle)).title("Plot Style Table").desc("Loads, lists or removes the plot style table (.ctb) that decides colours, lineweights and screening when plotting.").once());
    r.define(
        def("Output", "PUBLISH", run(publish))
            .title("Publish")
            .icon("pdf")
            .desc("Publishes drawings to PDF or plotters.")
            .once(),
    );
    r.define(
        def("Output", "ETRANSMIT", run(etransmit))
            .title("eTransmit")
            .desc("Lists the files needed to send a drawing to someone else.")
            .once(),
    );
    r.define(
        def("Utilities", "APPLOAD", run(appload))
            .aliases(&["AP"])
            .title("Load Application")
            .desc("Loads applications and script files.")
            .once(),
    );
    r.define(def("Utilities", "REINIT", run(reinit)).title("Reinitialize").desc("Reinitializes the digitizer, the digitizer input/output port, and the program parameters file.").once());
}
