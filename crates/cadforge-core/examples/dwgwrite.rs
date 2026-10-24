//! Write a drawing as DWG: cargo run -p cadforge-core --example dwgwrite -- [in.dxf|in.dwg|in.json] out.dwg
//! (with no input, the built-in sample drawing; DXF_OUT=file.dxf also writes the DXF the DWG was encoded from).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let d = if a.len() > 1 {
        let b = std::fs::read(&a[0]).unwrap();
        if cadforge_core::io::is_dwg(&b) {
            cadforge_core::io::read_dwg(&b, "in").unwrap()
        } else if a[0].ends_with(".json") {
            cadforge_core::io::from_json(&String::from_utf8(b).unwrap(), "in").unwrap()
        } else {
            cadforge_core::io::read_dxf_bytes(&b, "in").unwrap()
        }
    } else {
        cadforge_core::sample::sample_drawing()
    };
    let out = a.last().unwrap();
    std::fs::write(out, cadforge_core::io::write_dwg(&d).unwrap()).unwrap();
    // the DXF the DWG was encoded from, for checks against other readers
    if let Ok(p) = std::env::var("DXF_OUT") {
        std::fs::write(p, cadforge_core::io::write_dxf(&d)).unwrap();
    }
}
