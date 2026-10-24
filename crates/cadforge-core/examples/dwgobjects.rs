//! List every object of a DWG file: cargo run -p cadforge-core --example dwgobjects -- file.dwg
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    for (h, t) in cadforge_core::io::dwg::list_objects(&std::fs::read(&a[0]).unwrap()).unwrap() {
        println!("{h:X} {t}");
    }
}
