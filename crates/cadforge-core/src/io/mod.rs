//! File formats: DXF read/write and CadForge project files.
pub mod binary_dxf;
pub mod dwg;
pub mod dxf_read;
pub mod dxf_write;
pub mod project;
pub mod zip;

pub use binary_dxf::is_binary_dxf;
pub use dwg::{is_dwg, read_dwg, write_dwg};
pub use dxf_read::{decode_dxf_bytes, dxf_text_from_bytes, read_dxf, read_dxf_bytes};
pub use dxf_write::{write_dxf, write_dxf_binary};
pub use project::{from_json, strip_ext, to_json, FILE_EXT};

/// What a DXF or DWG file (`r2000`: an AutoCAD 2000 DWG) cannot hold of a drawing, as short notes for the user:
/// everything else opens again as it was saved.
pub fn not_kept_in_dxf(d: &crate::drawing::Drawing, r2000: bool) -> Vec<String> {
    use crate::entities::Kind;
    let mut out = vec![];
    let mut count = |n: usize, one: &str, many: &str, what: &str| {
        if n > 0 {
            out.push(format!("{n} {} {what}", if n == 1 { one } else { many }));
        }
    };
    let ents: Vec<&crate::entities::Entity> = d.all_entities().collect();
    // 3D solids are saved as polyface meshes: their shape is kept, not the history that rebuilds them from an outline
    count(d.solids.iter().filter(|s| s.program.is_some()).count(), "3D solid keeps its shape but not its extrusion history", "3D solids keep their shape but not their extrusion history", "");
    if r2000 {
        let rgb = ents.iter().filter(|e| matches!(e.color, Some(crate::style::Color::Rgb(..)))).count() + d.layers.iter().filter(|l| matches!(l.color, crate::style::Color::Rgb(..))).count();
        count(rgb, "true colour is", "true colours are", "saved as the nearest of the 255 colours (AutoCAD 2000 has no true colours)");
        count(ents.iter().filter(|e| matches!(&e.kind, Kind::Hatch(h) if h.gradient.is_some())).count(), "gradient fill is", "gradient fills are", "saved as a solid fill (AutoCAD 2000 has no gradients)");
    }
    if !d.skipped.is_empty() {
        let list: Vec<String> = d.skipped.iter().map(|(k, n)| format!("{n} {k}")).collect();
        out.push(format!("objects of the opened file that CadForge could not read: {}", list.join(", ")));
    }
    out.iter_mut().for_each(|s| *s = s.trim_end().to_string());
    out
}
