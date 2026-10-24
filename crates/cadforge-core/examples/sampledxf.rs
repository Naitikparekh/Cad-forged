//! Write the built-in sample drawing as DXF: cargo run -p cadforge-core --example sampledxf -- out.dxf
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    std::fs::write(&a[0], cadforge_core::io::write_dxf(&cadforge_core::sample::sample_drawing())).unwrap();
}
