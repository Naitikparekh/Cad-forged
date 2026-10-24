//! Writes the command reference from the registry: `cargo run -q -p cadforge-engine --example commands_doc > docs/COMMANDS.md`
use cadforge_engine::commands::build_registry;
use cadforge_engine::sysvars::pref_defaults;
use std::collections::BTreeMap;

fn main() {
    let reg = build_registry();
    let mut aliases: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (a, n) in reg.all_aliases() {
        aliases.entry(n).or_default().push(a);
    }
    let mut groups: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for d in reg.defs() {
        if d.name.starts_with('_') || d.name.starts_with("OSNAP_") {
            continue;
        }
        let g = if d.group.is_empty() {
            "General".to_string()
        } else {
            d.group.clone()
        };
        groups.entry(g).or_default().push(d.clone());
    }
    let total: usize = groups.values().map(Vec::len).sum();
    println!("# CadForge command reference\n");
    println!("Generated from the command registry (`cargo run -q -p cadforge-engine --example commands_doc`): {total} commands. Type a command or alias at the command line, or use the ribbon.\n");
    for (g, mut list) in groups {
        list.sort_by(|a, b| a.name.cmp(&b.name));
        println!("## {g}\n\n| Command | Aliases | Description |\n|---|---|---|");
        for d in list {
            let mut al = aliases.get(&d.name).cloned().unwrap_or_default();
            al.sort();
            al.dedup();
            let al = al
                .iter()
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(" ");
            let text = if d.desc.is_empty() { &d.title } else { &d.desc };
            println!("| `{}` | {al} | {} |", d.name, text.replace('|', "\\|"));
        }
        println!();
    }
    println!("## System variables\n\nType a variable name at the command line to view or change it.\n\n| Variable | Default |\n|---|---|");
    let mut vars: Vec<_> = pref_defaults().into_iter().collect();
    vars.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in vars {
        let t = v.text();
        println!(
            "| `{k}` | `{}` |",
            if t.is_empty() { "\"\"".to_string() } else { t }
        );
    }
}
