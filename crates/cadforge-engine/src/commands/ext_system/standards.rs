//! CAD standards: STANDARDS associates standards files with the drawing and CHECKSTANDARDS reports and fixes what
//! does not follow them (the checks are in `cadforge_core::standards`).
//!
//! A standards file reaches STANDARDS as the `json` argument (a `Standard` the host made from a `.dws`, `.dwg` or
//! `.dxf` file it read) or as the name of another open drawing. With a UI and no arguments both commands hand over to
//! the host's Configure Standards and Check Standards dialogs.
use super::{plural, ps};
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::standards::{check, fix, fix_all, Problem, Standard};

fn ui(c: &Ctx, a: &Args, name: &str) -> bool {
    if a.0.is_empty() && c.app(|app| app.ui.dialogs) {
        c.app(|app| app.emit(Event::UiAction { name: name.into(), args: Args::none() }));
        return true;
    }
    false
}

fn stem(file: &str) -> String {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    base.rsplit_once('.').map_or(base, |(s, _)| s).to_string()
}

fn summary(s: &Standard) -> String {
    format!(
        "{}: {}, {}, {}, {}, {}",
        s.file,
        plural(s.layers.len(), "layer"),
        plural(s.ltypes.len(), "linetype"),
        plural(s.dim_styles.len(), "dimension style"),
        plural(s.mleader_styles.len(), "multileader style"),
        plural(s.table_styles.len(), "table style")
    )
}

fn add(c: &Ctx, s: Standard) {
    let line = summary(&s);
    c.doc_mut(|d| {
        d.change(|d| {
            d.standards.files.retain(|f| !stem(&f.file).eq_ignore_ascii_case(&stem(&s.file)));
            d.standards.files.push(s);
        })
    });
    c.log(&format!("Standards file associated with the drawing. {line}"));
    c.emit();
}

/// Other open drawings (not block editors): their file or drawing names and their standards.
fn open_drawings(c: &Ctx) -> Vec<(String, Standard)> {
    c.app(|a| {
        let others = a.editors.iter().enumerate().filter(|(i, e)| Some(*i) != a.active && !e.doc.name.starts_with("*BlockEdit:"));
        others
            .map(|(_, e)| {
                let file = e.doc.file_name.clone().unwrap_or_else(|| e.doc.name.clone());
                let s = Standard::from_drawing(&file, &e.doc);
                (file, s)
            })
            .collect()
    })
}

/// STANDARDS: Add (a file given by the host, or an open drawing), Remove, List, or Check.
async fn standards(c: Ctx, a: Args) -> CmdResult<()> {
    if ui(&c, &a, "STANDARDS") {
        return Ok(());
    }
    let opt = match a.str("option") {
        Some(o) => o.to_string(),
        None => c.keyword(ps("Enter an option [Add/Remove/List/Check]").options(&["Add", "Remove", "List", "Check"]).default_str("List")).await?.kw().unwrap_or("List").to_string(),
    };
    match opt.to_ascii_uppercase().as_str() {
        "ADD" | "A" => {
            if let Some(j) = a.str("json") {
                match serde_json::from_str::<Standard>(j) {
                    Ok(s) => add(&c, s),
                    Err(e) => c.log_error(&format!("Not a standards file: {e}")),
                }
                return Ok(());
            }
            let open = open_drawings(&c);
            let Some(first) = open.first().map(|(f, _)| stem(f)) else {
                c.log_error("No other drawing is open. Open the standards file (.dws, .dwg or .dxf) first, or add it in the Configure Standards dialog.");
                return Ok(());
            };
            let want = match a.str("drawing") {
                Some(n) => n.to_string(),
                None => c.string(ps("Enter name of open drawing to use as a standards file").default_str(&first)).await?.text,
            };
            match open.into_iter().find(|(f, _)| stem(f).eq_ignore_ascii_case(&stem(&want))) {
                Some((_, s)) => add(&c, s),
                None => c.log_error(&format!("Drawing \"{want}\" is not open.")),
            }
        }
        "REMOVE" | "R" => {
            let files: Vec<String> = c.doc(|d| d.standards.files.iter().map(|f| f.file.clone()).collect());
            let Some(first) = files.first().map(|f| stem(f)) else {
                c.log("No standards files are associated with this drawing.");
                return Ok(());
            };
            let want = match a.str("file") {
                Some(n) => n.to_string(),
                None => c.string(ps("Enter name of standards file to remove").default_str(&first)).await?.text,
            };
            let removed = c.doc_mut(|d| {
                let before = d.standards.files.len();
                if d.standards.files.iter().any(|f| stem(&f.file).eq_ignore_ascii_case(&stem(&want))) {
                    d.change(|d| d.standards.files.retain(|f| !stem(&f.file).eq_ignore_ascii_case(&stem(&want))));
                }
                before - d.standards.files.len()
            });
            if removed == 0 {
                c.log_error(&format!("\"{want}\" is not a standards file of this drawing."));
            } else {
                c.log(&format!("Standards file \"{want}\" removed."));
            }
            c.emit();
        }
        "CHECK" | "C" => {
            c.sub_forced("CHECKSTANDARDS", Args::none().with("option", ArgVal::Str("Each".into()))).await;
        }
        _ => {
            let lines: Vec<String> = c.doc(|d| d.standards.files.iter().map(summary).collect());
            if lines.is_empty() {
                c.log("No standards files are associated with this drawing.");
            } else {
                c.log(&format!("Standards files:\n  {}", lines.join("\n  ")));
            }
        }
    }
    Ok(())
}

fn describe(i: usize, n: usize, p: &Problem) -> String {
    let fix = match &p.preferred {
        Some(f) if p.file.is_some() => format!(" Preferred fix: the standard properties of \"{f}\"."),
        Some(f) => format!(" Preferred fix: replace with \"{f}\"."),
        None => String::new(),
    };
    format!("Problem {i} of {n}: {}.{fix}", p.message())
}

/// CHECKSTANDARDS: list the problems, then fix them: All (the preferred fixes), Each (one at a time, choosing the
/// replacement), or None.
async fn checkstandards(c: Ctx, a: Args) -> CmdResult<()> {
    if c.doc(|d| d.standards.files.is_empty()) {
        c.log_error("No standards files are associated with this drawing. Use STANDARDS to add one.");
        return Ok(());
    }
    if ui(&c, &a, "CHECKSTANDARDS") {
        return Ok(());
    }
    // one problem fixed by the host's dialog: `key` with `with` (or the preferred fix), or ignored
    if let Some(key) = a.str("key") {
        let Some(p) = c.doc(check).into_iter().find(|p| p.key() == key) else {
            c.log_error(&format!("No standards problem \"{key}\"."));
            return Ok(());
        };
        if a.flag("ignore") {
            c.doc_mut(|d| d.change(|d| d.standards.ignored.push(p.key())));
            c.log(&format!("{} \"{}\" is ignored from now on.", p.table.label(), p.name));
        } else {
            let with = a.str("with").map(str::to_string);
            match c.doc_mut(|d| d.change(|d| fix(d, &p, with.as_deref()))) {
                Ok(m) => c.log(&m),
                Err(e) => c.log_error(&e),
            }
        }
        c.emit();
        return Ok(());
    }
    let probs = c.doc(check);
    if probs.is_empty() {
        c.log("Checking is complete. No standards problems were found.");
        return Ok(());
    }
    let n = probs.len();
    let list: Vec<String> = probs.iter().enumerate().map(|(i, p)| describe(i + 1, n, p)).collect();
    c.log(&format!("{} found:\n{}", plural(n, "problem"), list.join("\n")));
    let opt = match a.str("option") {
        Some(o) => o.to_string(),
        None => c.keyword(ps("Fix problems [All/Each/None]").options(&["All", "Each", "None"]).default_str("Each")).await?.kw().unwrap_or("Each").to_string(),
    };
    match opt.to_ascii_uppercase().as_str() {
        "ALL" | "A" => {
            let (done, left) = c.doc_mut(|d| d.change(fix_all));
            for m in &done {
                c.log(m);
            }
            c.log(&format!("{} fixed.", plural(done.len(), "problem")));
            if !left.is_empty() {
                c.log(&format!("{} left without a preferred fix: run CHECKSTANDARDS and choose a replacement for each.", plural(left.len(), "problem")));
            }
        }
        "EACH" | "E" => {
            let (mut fixed, mut i) = (0, 0);
            let mut skipped: Vec<String> = vec![];
            loop {
                let probs: Vec<Problem> = c.doc(check).into_iter().filter(|p| !skipped.contains(&p.key())).collect();
                let Some(p) = probs.first().cloned() else { break };
                i += 1;
                c.log(&describe(i, i + probs.len() - 1, &p));
                let def = p.preferred.clone().unwrap_or_else(|| "Skip".into());
                let names = if p.candidates.len() <= 8 { p.candidates.join("/") } else { "?".into() };
                let t = c.string(ps(&format!("Replace with standard [{names}] or [Skip/Ignore/eXit]")).default_str(&def)).await?.text;
                match t.trim().to_ascii_uppercase().as_str() {
                    "S" | "SKIP" => skipped.push(p.key()),
                    "X" | "EXIT" => break,
                    "I" | "IGNORE" => {
                        c.doc_mut(|d| d.change(|d| d.standards.ignored.push(p.key())));
                        skipped.push(p.key());
                    }
                    "?" => {
                        c.log(&format!("Standard {}s: {}", p.table.label().to_lowercase(), p.candidates.join(", ")));
                        i -= 1;
                    }
                    _ => {
                        let with = (!t.trim().is_empty()).then(|| t.trim().to_string());
                        match c.doc_mut(|d| d.change(|d| fix(d, &p, with.as_deref()))) {
                            Ok(m) => {
                                c.log(&m);
                                fixed += 1;
                            }
                            Err(e) => {
                                c.log_error(&e);
                                skipped.push(p.key());
                            }
                        }
                    }
                }
            }
            c.log(&format!("Checking is complete. {} fixed.", plural(fixed, "problem")));
        }
        _ => {}
    }
    c.emit();
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Utilities", "STANDARDS", run(standards)).title("Configure Standards").icon("standards").desc("Associates standards files (.dws, .dwg or .dxf) with the current drawing."));
    r.define(def("Utilities", "CHECKSTANDARDS", run(checkstandards)).title("Check Standards").icon("standards").desc("Checks layers, linetypes and dimension, multileader and table styles against the standards files and fixes them."));
}
