//! What changes when a drawing is saved as DWG (or DXF, with RTDIFF_DXF=1) and read back, by object kind:
//! cargo run -p cadforge-core --example rtdiff -- [in.dwg|in.dxf|in.json]   (no input: the built-in sample)
use cadforge_core::drawing::Drawing;
use std::collections::BTreeMap;
fn main() {
    let d = match std::env::args().nth(1) {
        Some(p) => {
            let b = std::fs::read(&p).unwrap();
            if cadforge_core::io::is_dwg(&b) {
                cadforge_core::io::read_dwg(&b, "in").unwrap()
            } else if p.ends_with(".json") {
                cadforge_core::io::from_json(&String::from_utf8(b).unwrap(), "in").unwrap()
            } else {
                cadforge_core::io::read_dxf_bytes(&b, "in").unwrap()
            }
        }
        None => cadforge_core::sample::sample_drawing(),
    };
    let back = if std::env::var("RTDIFF_DXF").is_ok() {
        cadforge_core::io::read_dxf(&cadforge_core::io::write_dxf(&d), "in").unwrap()
    } else {
        cadforge_core::io::read_dwg(&cadforge_core::io::write_dwg(&d).unwrap(), "in").unwrap()
    };
    let count = |d: &Drawing| {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for e in d.model.entities.iter().map(|e| format!("model {}", e.type_name())).chain(d.layouts.iter().flat_map(|l| l.entities.iter().map(move |e| format!("layout {} {}", l.name, e.type_name())))) {
            *m.entry(e).or_default() += 1;
        }
        m.insert("solids".to_string(), d.solids.len());
        m.insert("blocks".to_string(), d.blocks.len());
        m.insert("groups".to_string(), d.model.entities.iter().filter_map(|e| e.grp.clone()).collect::<std::collections::BTreeSet<_>>().len());
        m
    };
    let (a, b) = (count(&d), count(&back));
    for k in a.keys().chain(b.keys()).collect::<std::collections::BTreeSet<_>>() {
        let (x, y) = (a.get(k).copied().unwrap_or(0), b.get(k).copied().unwrap_or(0));
        if x != y {
            println!("{k}: {x} -> {y}");
        }
    }
}
