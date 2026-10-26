//! CadForge project files (JSON, format version 2). Legacy v1 projects are migrated on load.
use crate::drawing::Drawing;

pub const FILE_EXT: &str = ".cadforge.json";

/// File name without its drawing extensions (all of them: `a.dwg.cadforge.json` is `a`).
pub fn strip_ext(name: &str) -> String {
    let mut out = name;
    'strip: loop {
        let lower = out.to_lowercase();
        for ext in [".cadforge.json", ".json", ".cadforge", ".dxf", ".dwg", ".dwt"] {
            if lower.ends_with(ext) && out.len() > ext.len() {
                out = &out[..out.len() - ext.len()];
                continue 'strip;
            }
        }
        return out.to_string();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_drawing_extension_is_stripped() {
        assert_eq!(super::strip_ext("Drawing1.dwg.cadforge.json"), "Drawing1");
        assert_eq!(super::strip_ext("plan.DXF"), "plan");
        assert_eq!(super::strip_ext("a.b.dwg"), "a.b");
        assert_eq!(super::strip_ext(".dwg"), ".dwg");
    }
}

pub fn to_json(doc: &Drawing) -> String {
    doc.to_json_string()
}

pub fn from_json(text: &str, name: &str) -> Result<Drawing, String> {
    Drawing::from_json_str(text, Some(name)).map_err(|e| if e.contains("expected") || e.contains("EOF") || e.contains("key must be") { "This file is not valid JSON.".to_string() } else { e })
}
