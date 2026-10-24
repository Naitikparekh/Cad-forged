//! Loading definition files: `-LINETYPE` (`.lin`) and `LOADPAT` (`.pat`, hatch patterns).
//!
//! The text reaches the command as the `text` argument; without it the host is asked for a file, and starts the
//! command again with the file's text.
use super::ps;
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::defs::{parse_lin, parse_pat};
use cadforge_core::style::all_linetypes;

/// The file text: the `text` argument, or ask the host for a file (returns `None` then).
fn text_or_ask(c: &Ctx, a: &Args, action: &str, what: &str) -> Option<String> {
    if let Some(t) = a.str("text") {
        return Some(t.to_string());
    }
    if c.app(|app| app.ui.dialogs) {
        c.app(|app| app.emit(Event::UiAction { name: action.into(), args: Args::none() }));
    } else {
        c.log_error(&format!("{what} needs a file chooser; pass the file's text as the \"text\" argument instead."));
    }
    None
}

fn wanted(names: &str, name: &str) -> bool {
    let names = names.trim();
    names == "*" || names.split(',').any(|n| n.trim().eq_ignore_ascii_case(name))
}

async fn linetype_cmdline(c: Ctx, a: Args) -> CmdResult<()> {
    let opt = match a.str("option") {
        Some(o) => o.to_string(),
        None => c.keyword(ps("Enter an option [?/Create/Load/Set]").options(&["?", "Create", "Load", "Set"]).default_str("?")).await?.kw().unwrap_or("?").to_string(),
    };
    match opt.as_str() {
        "?" => {
            let list: Vec<String> = all_linetypes().iter().map(|l| format!("{:<16}{}", l.name, l.desc)).collect();
            c.log(&format!("Loaded linetypes:\n{}", list.join("\n")));
        }
        "Set" => {
            let t = c.string(ps("Enter linetype name").default_str("BYLAYER")).await?;
            if t.text.eq_ignore_ascii_case("bylayer") {
                c.doc_mut(|d| d.cur.ltype = None);
            } else {
                match cadforge_core::style::ltype_key(&t.text) {
                    Some(k) => c.doc_mut(|d| d.cur.ltype = Some(k.to_string())),
                    None => c.log_error(&format!("Linetype \"{}\" is not loaded.", t.text)),
                }
            }
        }
        "Load" => {
            let Some(text) = text_or_ask(&c, &a, "LINETYPELOAD", "Loading a linetype file") else { return Ok(()) };
            let names = match a.str("names") {
                Some(n) => n.to_string(),
                None => c.string(ps("Enter linetype(s) to load").default_str("*")).await?.text,
            };
            let parsed = parse_lin(&text);
            let mut loaded = vec![];
            let mut problems = parsed.skipped.clone();
            for def in parsed.defs.into_iter().filter(|d| wanted(&names, &d.name)) {
                let name = def.name.clone();
                match c.doc_mut(|d| d.change(|d| d.define_linetype(def))) {
                    Ok(()) => loaded.push(name),
                    Err(e) => problems.push(e),
                }
            }
            if loaded.is_empty() {
                c.log_error("No linetypes were loaded.");
            } else {
                c.log(&format!("Loaded {}: {}.", super::plural(loaded.len(), "linetype"), loaded.join(", ")));
            }
            for p in problems {
                c.log_error(&p);
            }
            c.emit();
        }
        _ => c.log_error("Creating a linetype is not supported; write a .lin file and load it."),
    }
    Ok(())
}

async fn loadpat(c: Ctx, a: Args) -> CmdResult<()> {
    let Some(text) = text_or_ask(&c, &a, "LOADPAT", "Loading a hatch pattern file") else { return Ok(()) };
    let parsed = parse_pat(&text);
    let mut loaded = vec![];
    let mut problems = parsed.skipped.clone();
    for def in parsed.defs {
        let name = def.name.clone();
        match c.doc_mut(|d| d.change(|d| d.define_pattern(def))) {
            Ok(()) => loaded.push(name),
            Err(e) => problems.push(e),
        }
    }
    if loaded.is_empty() {
        c.log_error("No hatch patterns were loaded.");
    } else {
        c.log(&format!("Loaded {}: {}. Use them by name in HATCH.", super::plural(loaded.len(), "hatch pattern"), loaded.join(", ")));
    }
    for p in problems {
        c.log_error(&p);
    }
    c.emit();
    Ok(())
}

pub(super) fn register(r: &mut Registry) {
    r.define(def("Properties", "-LINETYPE", run(linetype_cmdline)).title("Linetype (command line)").icon("linetype").desc("Lists, sets or loads linetypes (acad.lin format) from the command line."));
    r.define(def("Draw", "LOADPAT", run(loadpat)).title("Load Hatch Patterns").icon("hatch").desc("Loads hatch patterns from a .pat file (acad.pat format) so HATCH can use them by name.").once());
}
