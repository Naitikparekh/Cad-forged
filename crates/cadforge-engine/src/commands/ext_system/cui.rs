//! User interface customization commands: CUI, QUICKCUI and CUSTOMIZE open the host's Customize User Interface
//! dialog; CUIEXPORT writes the user's customization to a file, CUIIMPORT merges a file into it, CUILOAD loads a file
//! as a partial customization group and CUIUNLOAD removes one (the model is in [`crate::cui`]).
//!
//! A file reaches CUIIMPORT and CUILOAD as the `text` (and `file`) arguments; with a UI and no text the host is asked
//! to choose one and starts the command again with them.
use super::{plural, ps};
use crate::app::Event;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::cui::{group_name, Customization};
use crate::registry::Registry;
use crate::runtime::*;

fn describe(c: &Customization) -> String {
    format!("{}, {}, {}", plural(c.qat.len(), "toolbar button"), plural(c.shortcuts.len(), "shortcut"), plural(c.panels.len(), "ribbon panel"))
}

/// The dialog, or (without a UI) a listing of what is customized.
fn open_dialog(c: &Ctx, tab: &str) {
    if c.app(|a| a.ui.dialogs) {
        c.app(|a| a.emit(Event::UiAction { name: "CUI".into(), args: Args::none().with("tab", ArgVal::Str(tab.into())) }));
        return;
    }
    let (main, partials) = c.app(|a| (a.cui.main.clone(), a.cui.partials.clone()));
    let mut lines = vec![format!("Main customization: {}", describe(&main))];
    for s in &main.shortcuts {
        lines.push(format!("  {} = {}", s.keys, s.cmd));
    }
    for p in &partials {
        lines.push(format!("Partial customization \"{}\": {}", p.name, describe(&p.cui)));
    }
    c.log(&lines.join("\n"));
}

async fn cui(c: Ctx, a: Args) -> CmdResult<()> {
    open_dialog(&c, a.str("tab").unwrap_or("qat"));
    Ok(())
}

async fn quickcui(c: Ctx, _a: Args) -> CmdResult<()> {
    open_dialog(&c, "qat");
    Ok(())
}

async fn customize(c: Ctx, _a: Args) -> CmdResult<()> {
    open_dialog(&c, "panels");
    Ok(())
}

async fn cuiexport(c: Ctx, a: Args) -> CmdResult<()> {
    let main = c.app(|a| a.cui.main.clone());
    let name = a.str("file").map_or_else(|| "custom.cui.json".to_string(), str::to_string);
    let msg = format!("Customization exported to {name}: {}.", describe(&main));
    c.app(|app| {
        app.emit(Event::SaveFile { name, data: main.to_json().into_bytes(), mime: "application/json".into() });
        app.log(&msg, "info");
    });
    Ok(())
}

/// The file's (name, text): the arguments, or ask the host (it starts `action` again with them).
fn file_or_ask(c: &Ctx, a: &Args, action: &str) -> Option<(String, String)> {
    if let Some(t) = a.str("text") {
        return Some((a.str("file").unwrap_or("custom.cui.json").to_string(), t.to_string()));
    }
    if c.app(|app| app.ui.dialogs) {
        c.app(|app| app.emit(Event::UiAction { name: action.into(), args: Args::none() }));
    } else {
        c.log_error(&format!("{action} needs a file chooser; pass the file's text as the \"text\" argument and its name as \"file\"."));
    }
    None
}

fn read(c: &Ctx, file: &str, text: &str) -> Option<Customization> {
    match Customization::from_json(text) {
        Ok(x) => Some(x),
        Err(e) => {
            c.log_error(&format!("{}: {e}.", group_name(file)));
            None
        }
    }
}

async fn cuiimport(c: Ctx, a: Args) -> CmdResult<()> {
    let Some((file, text)) = file_or_ask(&c, &a, "CUIIMPORT") else { return Ok(()) };
    let Some(other) = read(&c, &file, &text) else { return Ok(()) };
    let (b, s, p) = c.app(|app| {
        let n = app.cui.main.merge(&other);
        app.cui.touch();
        n
    });
    c.log(&format!("Imported from {file}: {} added, {} added or changed, {} added or changed.", plural(b, "toolbar button"), plural(s, "shortcut"), plural(p, "ribbon panel")));
    c.emit();
    Ok(())
}

async fn cuiload(c: Ctx, a: Args) -> CmdResult<()> {
    let Some((file, text)) = file_or_ask(&c, &a, "CUILOAD") else { return Ok(()) };
    let Some(cui) = read(&c, &file, &text) else { return Ok(()) };
    let name = group_name(&file);
    let line = describe(&cui);
    let replaced = c.app(|app| {
        let had = app.cui.partials.iter().any(|p| p.name.eq_ignore_ascii_case(&name));
        app.cui.load(&name, cui);
        had
    });
    c.log(&format!("Customization group \"{name}\" {}: {line}.", if replaced { "reloaded" } else { "loaded" }));
    c.emit();
    Ok(())
}

async fn cuiunload(c: Ctx, a: Args) -> CmdResult<()> {
    let names: Vec<String> = c.app(|app| app.cui.partials.iter().map(|p| p.name.clone()).collect());
    let Some(first) = names.first().cloned() else {
        c.log("No partial customization groups are loaded.");
        return Ok(());
    };
    let want = match a.str("name") {
        Some(n) => n.to_string(),
        None => c.string(ps(&format!("Enter name of customization group to unload ({})", names.join(", "))).default_str(&first)).await?.text,
    };
    let want = group_name(want.trim());
    if c.app(|app| app.cui.unload(&want)) {
        c.log(&format!("Customization group \"{want}\" unloaded."));
        c.emit();
    } else {
        c.log_error(&format!("Customization group \"{want}\" is not loaded."));
    }
    Ok(())
}

pub fn register(r: &mut Registry) {
    let g = "Customize";
    r.define(def(g, "CUI", run(cui)).title("Customize User Interface").icon("options").desc("Customizes the Quick Access Toolbar, keyboard shortcuts and ribbon panels of your own."));
    r.define(def(g, "QUICKCUI", run(quickcui)).title("Quick Customize").icon("options").desc("Opens the Customize User Interface dialog at the Quick Access Toolbar."));
    r.define(def(g, "CUSTOMIZE", run(customize)).title("Customize").icon("options").desc("Opens the Customize User Interface dialog at the ribbon panels of your own."));
    r.define(def(g, "CUIEXPORT", run(cuiexport)).title("Export Customizations").icon("export").desc("Writes your customizations (toolbar buttons, shortcuts, ribbon panels) to a .cui.json file."));
    r.define(def(g, "CUIIMPORT", run(cuiimport)).title("Import Customizations").icon("import").desc("Adds the customizations of a .cui.json file to yours."));
    r.define(def(g, "CUILOAD", run(cuiload)).title("Load Customization").icon("import").desc("Loads a .cui.json file as a partial customization group."));
    r.define(def(g, "CUIUNLOAD", run(cuiunload)).title("Unload Customization").icon("close").desc("Unloads a partial customization group."));
}
