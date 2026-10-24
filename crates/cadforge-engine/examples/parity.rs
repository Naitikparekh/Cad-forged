//! Command parity against the AutoCAD inventories in `tools/benchmark/acad-commands.tsv` (core) and
//! `tools/benchmark/acad-longtail.tsv` (long tail).
//! `cargo run -p cadforge-engine --example parity [-- --missing | --list | --json]`
use cadforge_engine::commands::build_registry;
use std::collections::BTreeMap;

fn main() {
    let reg = build_registry();
    if std::env::args().any(|a| a == "--list") {
        for n in reg.names() {
            println!("{n}");
        }
        return;
    }
    if std::env::args().any(|a| a == "--aliases") {
        for (a, c) in reg.all_aliases() {
            println!("{a}\t{c}");
        }
        return;
    }
    let stubs: std::collections::HashMap<String, f64> =
        include_str!("../../../tools/benchmark/stubs.tsv")
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                Some((f[0].to_uppercase(), f.get(1)?.parse().ok()?))
            })
            .collect();
    let core = include_str!("../../../tools/benchmark/acad-commands.tsv");
    let tail = include_str!("../../../tools/benchmark/acad-longtail.tsv");
    let show_missing = std::env::args().any(|a| a == "--missing");
    let json = std::env::args().any(|a| a == "--json");
    // per inventory: (total weight, weight implemented, commands, implemented)
    let mut sets: BTreeMap<&str, (f64, f64, usize, usize)> = BTreeMap::new();
    let mut cats: BTreeMap<String, (f64, f64, usize, usize)> = BTreeMap::new();
    let mut missing: Vec<(u32, String, String, &str)> = vec![];
    let mut seen: Vec<String> = vec![];
    for (set, tsv) in [("core", core), ("tail", tail)] {
        for line in tsv
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        {
            let f: Vec<&str> = line.split('\t').collect();
            let (name, cat, w) = (f[0], f[1], f[2].parse::<f64>().unwrap());
            if seen.iter().any(|s| s == name) {
                continue;
            }
            seen.push(name.to_string());
            let have = reg.find(name).is_some();
            // a command that only reports "not supported" earns part of its weight
            let credit = if have {
                stubs
                    .get(
                        &reg.find(name)
                            .map_or(String::new(), |d| d.name.to_uppercase()),
                    )
                    .copied()
                    .unwrap_or(1.0)
            } else {
                0.0
            };
            for e in [
                sets.entry(set).or_default(),
                cats.entry(cat.to_string()).or_default(),
            ] {
                e.0 += w;
                e.2 += 1;
                e.1 += w * credit;
                if have {
                    e.3 += 1;
                }
            }
            if !have {
                missing.push((w as u32, cat.to_string(), name.to_string(), set));
            }
        }
    }
    let pct = |e: &(f64, f64, usize, usize)| 100.0 * e.1 / e.0;
    let all = sets.values().fold((0.0, 0.0, 0, 0), |a, e| {
        (a.0 + e.0, a.1 + e.1, a.2 + e.2, a.3 + e.3)
    });
    if !json {
        for (c, e) in &cats {
            println!(
                "{c:<12} {:>3}/{:<3} commands   weighted {:>5.1}%",
                e.3,
                e.2,
                pct(e)
            );
        }
        for (n, e) in &sets {
            println!(
                "{:<12} {:>3}/{:<3} commands   weighted {:>5.1}%",
                n.to_uppercase(),
                e.3,
                e.2,
                pct(e)
            );
        }
        println!(
            "{:<12} {:>3}/{:<3} commands   weighted {:>5.1}%",
            "COMBINED",
            all.3,
            all.2,
            pct(&all)
        );
    }
    // aliases: the alias must resolve to the same command as acad.pgp says
    let pgp = include_str!("../../../tools/benchmark/acad-aliases.tsv");
    let (mut ok, mut total) = (0, 0);
    let mut bad: Vec<String> = vec![];
    for line in pgp
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        total += 1;
        match reg.find(f[0]) {
            Some(d) if d.name.trim_start_matches('-').eq_ignore_ascii_case(f[1]) => ok += 1,
            Some(d) => bad.push(format!("{} -> {} (expected {})", f[0], d.name, f[1])),
            None => bad.push(format!("{} -> (none) (expected {})", f[0], f[1])),
        }
    }
    if json {
        println!("{{\"core\": {:.2}, \"tail\": {:.2}, \"combined\": {:.2}, \"aliases\": {:.2}, \"core_n\": [{}, {}], \"tail_n\": [{}, {}]}}", pct(&sets["core"]), pct(&sets["tail"]), pct(&all), 100.0 * ok as f64 / total as f64, sets["core"].3, sets["core"].2, sets["tail"].3, sets["tail"].2);
        return;
    }
    println!(
        "{:<12} {ok:>3}/{total:<3} aliases    {:>5.1}%",
        "ALIASES",
        100.0 * ok as f64 / total as f64
    );
    if show_missing {
        for b in &bad {
            println!("  alias {b}");
        }
    }
    if show_missing {
        missing.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for (w, c, n, set) in missing {
            println!("  missing w{w} {c:<11} {n:<24} [{set}]");
        }
    }
}
