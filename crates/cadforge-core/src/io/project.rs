//! CadForge project files (JSON, format version 2). Legacy v1 projects are migrated on load.
use crate::drawing::Drawing;

pub const FILE_EXT: &str = ".cadforge.json";

/// File name without a recognised drawing extension.
pub fn strip_ext(name: &str) -> String {
    let lower = name.to_lowercase();
    for ext in [".cadforge.json", ".json", ".cadforge", ".dxf", ".dwg", ".dwt"] {
        if lower.ends_with(ext) {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    name.to_string()
}

pub fn to_json(doc: &Drawing) -> String {
    doc.to_json_string()
}

pub fn from_json(text: &str, name: &str) -> Result<Drawing, String> {
    Drawing::from_json_str(text, Some(name)).map_err(|e| if e.contains("expected") || e.contains("EOF") || e.contains("key must be") { "This file is not valid JSON.".to_string() } else { e })
}
