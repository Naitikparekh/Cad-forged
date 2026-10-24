//! ARCHIVE and ETRANSMIT: pack the drawing and the files it needs into one ZIP for the host to save.
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::drawing::Drawing;
use cadforge_core::io::{write_dxf, zip::zip};

/// The text report that travels with the package.
pub fn report(d: &Drawing, files: &[String]) -> String {
    let mut r = format!("CadForge transmittal for {}\n\nFiles in this package:\n", d.name);
    for f in files {
        r.push_str(&format!("  {f}\n"));
    }
    r.push_str("\nExternal references:\n");
    let xrefs: Vec<String> = d.blocks.values().filter_map(|b| b.xref.as_ref().map(|x| format!("  {} -> {}{}", b.name, x.path, if x.overlay { " (overlay)" } else { "" }))).collect();
    r.push_str(&if xrefs.is_empty() { "  none\n".to_string() } else { xrefs.join("\n") + "\n  (referenced drawings are not copied: add them to the package by hand)\n" });
    r.push_str("\nPictures:\n");
    let pics: Vec<String> = d.images.values().map(|i| format!("  {}{}", i.path, if i.data.is_some() { "" } else { " (the file is not available, so it is not in the package)" })).collect();
    r.push_str(&if pics.is_empty() { "  none\n".to_string() } else { pics.join("\n") + "\n" });
    r.push_str("\nFonts: none needed (text uses the built-in font)\n");
    if !d.custom_ltypes.is_empty() || !d.custom_patterns.is_empty() {
        r.push_str(&format!("\nCustom definitions stored inside the drawing: {} linetype(s), {} hatch pattern(s)\n", d.custom_ltypes.len(), d.custom_patterns.len()));
    }
    r
}

/// The files of a package: the drawing as project JSON and DXF, its pictures and the report. Returns (path, bytes) pairs.
pub fn package_files(d: &Drawing) -> Vec<(String, Vec<u8>)> {
    let stem = if d.name.is_empty() { "drawing".to_string() } else { d.name.clone() };
    let mut files: Vec<(String, Vec<u8>)> = vec![(format!("{stem}.cadforge.json"), d.to_json_string().into_bytes()), (format!("{stem}.dxf"), write_dxf(d).into_bytes())];
    for (key, img) in &d.images {
        if let Some(data) = &img.data {
            let base = img.path.rsplit(['/', '\\']).next().filter(|n| !n.is_empty()).unwrap_or(key);
            files.push((format!("pictures/{base}"), data.0.as_ref().clone()));
        }
    }
    let names: Vec<String> = files.iter().map(|f| f.0.clone()).chain(["transmittal.txt".to_string()]).collect();
    files.push(("transmittal.txt".into(), report(d, &names).into_bytes()));
    files
}

/// Build the package and hand it to the host as `<drawing>-<suffix>.zip`.
pub async fn send_package(c: &Ctx, suffix: &str, what: &str) -> CmdResult<()> {
    let (files, name) = c.doc(|d| (package_files(d), if d.name.is_empty() { "drawing".to_string() } else { d.name.clone() }));
    let n = files.len();
    let data = zip(&files);
    let file = format!("{name}-{suffix}.zip");
    c.log(&format!("{what}: {n} file{} in {file} ({} KB).", if n == 1 { "" } else { "s" }, data.len().div_ceil(1024)));
    c.app(|a| a.emit(Event::SaveFile { name: file, data, mime: "application/zip".into() }));
    Ok(())
}

async fn archive(c: Ctx, _a: Args) -> CmdResult<()> {
    send_package(&c, "archive", "Archive").await
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("File", "ARCHIVE", run(archive)).title("Archive").icon("save").desc("Packs the drawing, its pictures and a report into one ZIP file.").once());
}
