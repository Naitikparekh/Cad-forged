//! Convert a DWG file to DXF text on stdout: cargo run -p cadforge-core --example dwg2dxf -- file.dwg
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (text, rep) = cadforge_core::io::dwg::dwg_to_dxf(&std::fs::read(&a[0]).unwrap()).unwrap();
    eprintln!("{rep:?}");
    print!("{text}");
}
