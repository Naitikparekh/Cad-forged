//! Command parity against the AutoCAD inventory in `tools/benchmark/acad-commands.tsv`.
//! `cargo run -p cadforge-engine --example parity [-- --missing]`
use cadforge_engine::commands::build_registry;
use std::collections::BTreeMap;

fn main() {
    let reg = build_registry();
    let tsv = include_str!("../../../tools/benchmark/acad-commands.tsv");
    let show_missing = std::env::args().any(|a| a == "--missing");
    let mut cats: BTreeMap<String, (f64, f64, usize, usize)> = BTreeMap::new();
    let mut missing: Vec<(u32, String, String)> = vec![];
    for line in tsv.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, cat, w) = (f[0], f[1], f[2].parse::<f64>().unwrap());
        let have = reg.find(name).is_some();
        let e = cats.entry(cat.to_string()).or_default();
        e.0 += w;
        e.2 += 1;
        if have {
            e.1 += w;
            e.3 += 1;
        } else {
            missing.push((w as u32, cat.to_string(), name.to_string()));
        }
    }
    let (mut tw, mut hw, mut tn, mut hn) = (0.0, 0.0, 0, 0);
    for (c, (t, h, n, k)) in &cats {
        println!("{c:<12} {k:>3}/{n:<3} commands   weighted {:>5.1}%", 100.0 * h / t);
        tw += t;
        hw += h;
        tn += n;
        hn += k;
    }
    println!("{:<12} {hn:>3}/{tn:<3} commands   weighted {:>5.1}%", "TOTAL", 100.0 * hw / tw);
    if show_missing {
        missing.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for (w, c, n) in missing {
            println!("  missing w{w} {c:<11} {n}");
        }
    }
}
