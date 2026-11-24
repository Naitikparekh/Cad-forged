//! CadForge without its window: open, report on, convert and save drawings, and run commands on them as a script
//! would type them. The `cadforge-cli` program and its MCP server (see [`mcp`]) are built on [`Session`].
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::{bounds_of, Entity, Kind};
use cadforge_core::io;
use cadforge_engine::sim::Sim;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub mod mcp;

/// One drawing open in a headless CadForge, driven through the same command line as the application.
pub struct Session {
    sim: Sim,
}

impl Default for Session {
    fn default() -> Session {
        Session::new()
    }
}

/// The file formats a drawing can be written in, chosen by the file's extension.
pub fn format_of(path: &str) -> Result<&'static str, String> {
    let l = path.to_ascii_lowercase();
    for (ext, f) in [(".dwg", "dwg"), (".dxf", "dxf"), (".json", "json"), (".cadforge", "json"), (".svg", "svg"), (".pdf", "pdf")] {
        if l.ends_with(ext) {
            return Ok(f);
        }
    }
    Err(format!("{path}: CadForge writes .dwg, .dxf, .json, .svg and .pdf files."))
}

/// Read a drawing: DWG (by its signature), DXF (text or binary) or a CadForge project (JSON).
pub fn read_drawing(path: &str, bytes: &[u8]) -> Result<Drawing, String> {
    let name = Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
    let base = io::strip_ext(&name);
    let lower = name.to_ascii_lowercase();
    let read = || {
        if io::is_dwg(bytes) || lower.ends_with(".dwg") {
            io::read_dwg(bytes, &base)
        } else if lower.ends_with(".dxf") || io::is_binary_dxf(bytes) {
            io::read_dxf_bytes(bytes, &base)
        } else {
            io::from_json(&String::from_utf8_lossy(bytes), &base)
        }
    };
    // a reader fault on an unusual file is an error for this file, not a crash
    let mut d = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read)).unwrap_or_else(|_| Err("the file has content CadForge cannot read yet".into()))?;
    d.file_name = Some(name);
    Ok(d)
}

/// The bytes of a drawing in the format the file name asks for (`.svg` and `.pdf` plot model space to fit an A3
/// sheet).
pub fn encode_drawing(d: &mut Drawing, path: &str) -> Result<Vec<u8>, String> {
    Ok(match format_of(path)? {
        "dwg" => io::write_dwg(d)?,
        "dxf" => io::write_dxf(d).into_bytes(),
        "json" => io::to_json(d).into_bytes(),
        f => {
            use cadforge_render::plot::*;
            let mut o = default_plot_options(d);
            o.paper = "A3".into();
            o.format = if f == "svg" { PlotFormat::Svg } else { PlotFormat::Pdf };
            o.title = d.name.clone();
            export_vector(d, &o, None)?.data
        }
    })
}

/// What a drawing holds: its objects by type, layers, blocks with their reference counts, text styles and extents.
pub fn drawing_info(d: &Drawing) -> Value {
    let mut types: BTreeMap<&str, usize> = BTreeMap::new();
    let mut refs: BTreeMap<String, usize> = BTreeMap::new();
    let mut ext: Option<(f64, f64, f64, f64)> = None;
    for e in &d.model.entities {
        *types.entry(e.type_name()).or_default() += 1;
        if let Kind::Insert(i) = &e.kind {
            *refs.entry(i.name.clone()).or_default() += 1;
        }
        if let Some(b) = bounds_of(e, d) {
            ext = Some(match ext {
                None => (b.min_x, b.min_y, b.max_x, b.max_y),
                Some((a, b2, c, e2)) => (a.min(b.min_x), b2.min(b.min_y), c.max(b.max_x), e2.max(b.max_y)),
            });
        }
    }
    let blocks: Vec<Value> = d.blocks.iter().filter(|(n, _)| !n.starts_with('*')).map(|(n, b)| json!({ "name": n, "objects": b.entities.len(), "references": refs.get(n).copied().unwrap_or(0), "xref": b.xref.is_some() })).collect();
    let layers: Vec<Value> = d.layers.iter().map(|l| json!({ "name": l.name, "on": l.on, "frozen": l.frozen, "locked": l.locked })).collect();
    json!({
        "name": d.name,
        "objects": d.model.entities.len(),
        "solids": d.solids.len(),
        "types": types,
        "layers": layers,
        "current_layer": d.cur.layer,
        "blocks": blocks,
        "layouts": d.layouts.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
        "text_styles": d.settings.all_text_styles().into_iter().map(|s| s.name).collect::<Vec<_>>(),
        "dim_styles": d.settings.dim_styles.keys().cloned().collect::<Vec<_>>(),
        "extents": ext.map(|(a, b, c, e)| json!({ "min": [a, b], "max": [c, e] })),
        "units": { "insunits": d.settings.units.ins_units, "precision": d.settings.units.precision },
    })
}

impl Session {
    /// A session with a new, empty (metric) drawing.
    pub fn new() -> Session {
        Session { sim: Sim::new() }
    }

    pub fn new_drawing(&mut self, imperial: bool) {
        self.sim.h.borrow_mut().new_drawing(None, imperial);
    }

    /// Open a drawing file; it replaces the session's drawing.
    pub fn open(&mut self, path: &str) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        self.open_bytes(path, &bytes)
    }

    pub fn open_bytes(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let d = read_drawing(path, bytes)?;
        let mut a = self.sim.h.borrow_mut();
        a.add_drawing(d);
        // one drawing at a time: drop the others so commands and reports see this one
        while a.editors.len() > 1 {
            a.editors.remove(0);
        }
        a.active = Some(0);
        Ok(())
    }

    /// Write the drawing; the format follows the extension (.dwg, .dxf, .json, .svg or .pdf).
    pub fn save(&mut self, path: &str) -> Result<usize, String> {
        let bytes = self.with_doc_mut(|d| encode_drawing(d, path))?;
        std::fs::write(path, &bytes).map_err(|e| format!("{path}: {e}"))?;
        Ok(bytes.len())
    }

    pub fn doc<R>(&self, f: impl FnOnce(&Drawing) -> R) -> R {
        self.sim.doc(f)
    }

    fn with_doc_mut<R>(&self, f: impl FnOnce(&mut Drawing) -> R) -> R {
        self.sim.doc_mut(f)
    }

    pub fn info(&self) -> Value {
        self.doc(drawing_info)
    }

    /// Run command-line input, one line at a time, exactly as a script (SCR file) types it: a command name starts
    /// it, the following lines answer its prompts (`10,20` for a point, a number, an option keyword) and an empty
    /// line is Enter. Returns what the command line showed and the prompt still waiting, if any.
    pub fn run(&mut self, lines: &[String]) -> Value {
        let before = self.sim.h.borrow().history.len();
        let n0 = self.sim.n();
        for l in lines {
            let l = l.trim_end_matches(['\r', '\n']);
            if l.trim_start().starts_with(';') {
                continue;
            }
            if l.trim().is_empty() {
                self.sim.enter();
            } else {
                self.sim.ty(l);
            }
        }
        let a = self.sim.h.borrow();
        let log: Vec<String> = a.history[before.min(a.history.len())..].iter().map(|l| l.text.clone()).collect();
        let errors: Vec<String> = a.history[before.min(a.history.len())..].iter().filter(|l| l.kind == cadforge_engine::app::LogKind::Error).map(|l| l.text.clone()).collect();
        let waiting = a.busy().then(|| a.prompt_line());
        drop(a);
        json!({ "log": log.join("\n"), "errors": errors, "waiting_for": waiting, "objects": self.sim.n(), "objects_added": self.sim.n() as i64 - n0 as i64 })
    }

    /// Cancel a command that is waiting for input.
    pub fn cancel(&mut self) {
        self.sim.esc();
        self.sim.esc();
    }

    /// The objects of model space, optionally only those of a type (`line`, `circle`, `insert`...) or on a layer, as
    /// CadForge's JSON (coordinates in drawing units, angles in radians).
    pub fn entities(&self, ty: Option<&str>, layer: Option<&str>, limit: usize) -> Value {
        self.doc(|d| {
            let pick = |e: &&Entity| ty.is_none_or(|t| e.type_name().eq_ignore_ascii_case(t)) && layer.is_none_or(|l| e.layer.eq_ignore_ascii_case(l) || (e.layer.is_empty() && l == "0"));
            let all: Vec<&Entity> = d.model.entities.iter().filter(pick).collect();
            let shown: Vec<Value> = all.iter().take(limit).map(|e| serde_json::to_value(e).unwrap_or(Value::Null)).collect();
            json!({ "count": all.len(), "shown": shown.len(), "objects": shown })
        })
    }

    /// Add objects given as CadForge JSON (the form [`Session::entities`] returns); returns their ids.
    pub fn add_entities(&mut self, objects: &[Value]) -> Result<Vec<u64>, String> {
        let mut parsed = vec![];
        for (i, o) in objects.iter().enumerate() {
            let mut e: Entity = serde_json::from_value(o.clone()).map_err(|err| format!("object {}: {err}", i + 1))?;
            e.id = 0;
            parsed.push(e);
        }
        Ok(parsed.into_iter().map(|e| self.sim.add(e)).collect())
    }

    /// The commands CadForge knows (name, group, description), optionally those whose name or description contains
    /// `filter`.
    pub fn commands(&self, filter: Option<&str>) -> Vec<Value> {
        let reg = self.sim.registry();
        let f = filter.map(|s| s.to_ascii_lowercase());
        let mut v: Vec<Value> = reg
            .defs()
            .filter(|d| f.as_ref().is_none_or(|f| d.name.to_ascii_lowercase().contains(f) || d.desc.to_ascii_lowercase().contains(f) || d.title.to_ascii_lowercase().contains(f)))
            .map(|d| json!({ "name": d.name, "aliases": d.aliases, "group": d.group, "title": d.title, "description": d.desc }))
            .collect();
        v.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        v
    }
}
