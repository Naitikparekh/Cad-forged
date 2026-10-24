//! Full comparison of a DWG file with its DXF twin (entity JSON, layers, blocks).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dwg = cadforge_core::io::read_dwg(&std::fs::read(&a[0]).unwrap(), "x").unwrap();
    let dxf = cadforge_core::io::read_dxf_bytes(&std::fs::read(&a[1]).unwrap(), "x").unwrap();
    let names = |d: &cadforge_core::drawing::Drawing| d.blocks.keys().cloned().collect::<Vec<_>>();
    println!("blocks dwg {:?}\nblocks dxf {:?}", names(&dwg), names(&dxf));
    let ents = |d: &cadforge_core::drawing::Drawing| {
        fn round(v: &mut serde_json::Value) {
            match v {
                serde_json::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        let r = if f.abs() < 1e-9 {
                            0.0
                        } else {
                            let k = 10f64.powi(9 - f.abs().log10().ceil() as i32);
                            (f * k).round() / k
                        };
                        *v = serde_json::json!(r);
                    }
                }
                serde_json::Value::Array(a) => a.iter_mut().for_each(round),
                serde_json::Value::Object(o) => o.values_mut().for_each(round),
                _ => {}
            }
        }
        let mut v: Vec<String> = d
            .model
            .entities
            .iter()
            .map(|e| {
                let mut e = e.clone();
                e.id = 0;
                e.grp = None;
                let mut j = serde_json::to_value(&e).unwrap();
                round(&mut j);
                j.to_string()
            })
            .collect();
        v.sort();
        v
    };
    let (x, y) = (ents(&dwg), ents(&dxf));
    let mut diff = 0;
    for (p, q) in x.iter().zip(y.iter()) {
        if p != q {
            diff += 1;
            if diff <= 6 {
                println!(
                    "DWG {}\nDXF {}\n",
                    &p[..p.len().min(400)],
                    &q[..q.len().min(400)]
                );
            }
        }
    }
    println!("{} of {} model entities differ", diff, x.len().max(y.len()));
    let lay = |d: &cadforge_core::drawing::Drawing| serde_json::to_string(&d.layers).unwrap();
    println!("layers equal: {}", lay(&dwg) == lay(&dxf));
    if lay(&dwg) != lay(&dxf) {
        println!(
            "DWG {}\nDXF {}",
            &lay(&dwg)[..1500.min(lay(&dwg).len())],
            &lay(&dxf)[..1500.min(lay(&dxf).len())]
        );
    }
}
