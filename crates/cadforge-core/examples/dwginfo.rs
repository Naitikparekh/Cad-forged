//! Compare DWG files with their DXF twins: cargo run -p cadforge-core --example dwginfo -- file.dwg [twin.dxf]
use cadforge_core::drawing::Drawing;
use std::collections::BTreeMap;

fn summary(d: &Drawing) -> (BTreeMap<String, usize>, usize, usize, Vec<String>) {
    let mut c: BTreeMap<String, usize> = BTreeMap::new();
    for e in &d.model.entities {
        *c.entry(e.type_name().to_string()).or_default() += 1;
    }
    (c, d.layers.len(), d.blocks.len(), d.layout_names())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&args[0]).unwrap();
    let t = std::time::Instant::now();
    match cadforge_core::io::dwg::dwg_to_dxf(&bytes) {
        Ok((_, rep)) => println!(
            "{}: version {} objects {} failed {:?} unsupported {:?} ({:?})",
            args[0],
            rep.version,
            rep.objects,
            rep.failed,
            rep.unsupported,
            t.elapsed()
        ),
        Err(e) => {
            println!("{}: ERROR {e}", args[0]);
            return;
        }
    }
    let d = cadforge_core::io::read_dwg(&bytes, "x").unwrap();
    let s = summary(&d);
    println!(
        "  dwg: {:?} layers {} blocks {} layouts {:?} skipped {:?}",
        s.0, s.1, s.2, s.3, d.skipped
    );
    if let Some(twin) = args.get(1) {
        let t = cadforge_core::io::read_dxf_bytes(&std::fs::read(twin).unwrap(), "x").unwrap();
        let s2 = summary(&t);
        println!(
            "  dxf: {:?} layers {} blocks {} layouts {:?} skipped {:?}",
            s2.0, s2.1, s2.2, s2.3, t.skipped
        );
    }
}
