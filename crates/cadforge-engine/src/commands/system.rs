//! System commands: UNDO/REDO, SETVAR, drafting toggles, selection helpers, draw order and UI/file delegates
//! (port of `src/commands/system.js`).
use super::layer::wildcard_match;
use super::{def, run};
use crate::app::Event;
use crate::ctx::*;
use crate::parse::{os_bit, os_full, parse_number, OS_BITS, OS_OFF};
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use crate::select::selectable;
use crate::sysvars::VarType;
use std::collections::BTreeSet;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn ds(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("System", name, f)
}

// ---------------------------------------------------------------- undo / redo

async fn u(c: Ctx, _a: Args) -> CmdResult<()> {
    match c.doc_mut(|d| d.undo()) {
        None => c.log("Everything has been undone"),
        Some(label) => {
            c.ed(|e| e.prune());
            c.log(if label.is_empty() { "Undo" } else { &label });
            c.emit();
        }
    }
    Ok(())
}

async fn undo(c: Ctx, _a: Args) -> CmdResult<()> {
    let n = c.number(ps("Enter the number of operations to undo or [Auto/Control/BEgin/End/Mark/Back]").default_num(1.0).integer().min(1.0).options(&["Auto", "Control", "BEgin", "End", "Mark", "Back"])).await?;
    if let Some(k) = n.kw() {
        c.log(&format!("UNDO {k}: not supported; each command is one undo step."));
        return Ok(());
    }
    let want = n.value().unwrap_or(1.0) as usize;
    let mut done = 0;
    for _ in 0..want {
        if c.doc_mut(|d| d.undo()).is_none() {
            break;
        }
        done += 1;
    }
    c.ed(|e| e.prune());
    c.log(&if done < want { "Everything has been undone".to_string() } else { format!("{done} {} undone", if done == 1 { "command" } else { "commands" }) });
    c.emit();
    Ok(())
}

async fn redo(c: Ctx, _a: Args) -> CmdResult<()> {
    match c.doc_mut(|d| d.redo()) {
        None => c.log_error("Nothing to redo."),
        Some(l) => {
            c.ed(|e| e.prune());
            c.log(if l.is_empty() { "Redo" } else { &l });
        }
    }
    c.emit();
    Ok(())
}

// ---------------------------------------------------------------- SETVAR

async fn setvar(c: Ctx, args: Args) -> CmdResult<()> {
    let mut name = args.str("name").map(str::to_string);
    while name.is_none() {
        let r = c.string(ps("Enter variable name or [?]")).await?;
        let t = r.text.trim().to_string();
        if t.is_empty() {
            return Ok(());
        }
        if t == "?" {
            let w = c.string(ps("Enter variable(s) to list").default_str("*")).await?.text;
            let w = if w.is_empty() { "*".to_string() } else { w };
            let lines: Vec<String> = c.app(|a| {
                a.sysvars
                    .names()
                    .into_iter()
                    .filter(|n| wildcard_match(&w, n))
                    .map(|n| format!("{:<20}{}{}", n, a.getvar(&n).map(|v| v.text()).unwrap_or_default(), if a.sysvars.is_read_only(&n) { "  (read only)" } else { "" }))
                    .collect()
            });
            for l in lines {
                c.log(&l);
            }
            c.app(|a| a.emit(Event::ShowHistory));
            continue;
        }
        name = Some(t);
    }
    let name = name.unwrap_or_default().to_uppercase();
    if !c.app(|a| a.sysvars.has(&name)) {
        c.log_error(&format!("Unknown variable name {name}."));
        return Ok(());
    }
    let cur = c.var(&name)?.text();
    if c.app(|a| a.sysvars.is_read_only(&name)) {
        c.log(&format!("{name} = {cur} (read only)"));
        return Ok(());
    }
    let r = c.string(ps(&format!("Enter new value for {name}")).default_str(&cur)).await?;
    let text = r.text.trim().to_string();
    if text.is_empty() || text == cur {
        return Ok(());
    }
    let ty = c.app(|a| a.sysvars.var_type(&name));
    if ty == VarType::Str {
        c.setvar(&name, text)?;
    } else {
        let Some(v) = parse_number(&text) else {
            c.log_error("Requires numeric value.");
            return Ok(());
        };
        c.setvar(&name, v)?;
    }
    c.emit();
    Ok(())
}

// ---------------------------------------------------------------- toggles

async fn toggle(c: Ctx, args: Args, var: &'static str, label: &'static str) -> CmdResult<()> {
    let cur = c.pref(var) != 0.0;
    let mut next = args.opt_bool("value").unwrap_or(!cur);
    if args.opt_bool("value").is_none() && args.flag("ask") {
        let r = c.keyword(ps("Enter mode [ON/OFF]").options(&["ON", "OFF"]).default_str(if cur { "ON" } else { "OFF" })).await?;
        next = r.kw() == Some("ON");
    }
    c.setvar(var, if next { 1.0 } else { 0.0 })?;
    c.log(&format!("<{label} {}>", if next { "on" } else { "off" }));
    c.emit();
    Ok(())
}

async fn dyn_(c: Ctx, _a: Args) -> CmdResult<()> {
    let cur = c.pref("DYNMODE");
    let next = if cur > 0.0 { -cur.abs() } else if cur.abs() != 0.0 { cur.abs() } else { 3.0 };
    c.setvar("DYNMODE", next)?;
    c.log(&format!("<Dynamic input {}>", if c.pref("DYNMODE") > 0.0 { "on" } else { "off" }));
    c.emit();
    Ok(())
}

async fn osnaptoggle(c: Ctx, _a: Args) -> CmdResult<()> {
    let m = c.pref("OSMODE") as i64 as u32;
    c.setvar("OSMODE", (m ^ OS_OFF) as f64)?;
    c.log(&format!("<Object Snap {}>", if c.app(|a| a.sysvars.osnap_on()) { "on" } else { "off" }));
    c.emit();
    Ok(())
}

async fn osnap(c: Ctx, _a: Args) -> CmdResult<()> {
    if c.dialog(DialogReq::DSettings { tab: "osnap".into() }).await?.is_none() {
        c.log(&format!("OSMODE = {}", c.pref("OSMODE") as i64));
    }
    Ok(())
}

// ---------------------------------------------------------------- selection helpers

async fn selectall(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids: Vec<u64> = c.doc(|d| d.entities().iter().filter(|e| selectable(d, e)).map(|e| e.id).collect());
    let n = ids.len();
    c.ed(|e| e.sel = ids.into_iter().collect());
    c.log(&format!("{n} found"));
    c.emit();
    Ok(())
}

async fn selectsimilar(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let ents = c.entities(&sel.ents);
    let types: BTreeSet<&str> = ents.iter().map(|e| e.type_name()).collect();
    let layers: BTreeSet<&str> = ents.iter().map(|e| e.layer.as_str()).collect();
    let ids: Vec<u64> = c.doc(|d| d.entities().iter().filter(|e| selectable(d, e) && types.contains(e.type_name()) && layers.contains(e.layer.as_str())).map(|e| e.id).collect());
    let n = ids.len();
    c.ed(|e| e.sel = ids.into_iter().collect());
    c.log(&format!("{n} found"));
    Ok(())
}

async fn draworder(c: Ctx, args: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects")).await?;
    if sel.ents.is_empty() {
        return Ok(());
    }
    let mut where_ = args.str("where").map(str::to_string);
    if where_.is_none() {
        let k = c.keyword(ps("Enter object ordering option [Above objects/Under objects/Front/Back]").options(&["Above objects", "Under objects", "Front", "Back"]).default_str("Back")).await?;
        where_ = k.kw().map(str::to_string);
    }
    let front = where_.as_deref() == Some("Front");
    c.doc_mut(|d| d.reorder(&sel.ents, front));
    Ok(())
}

async fn script(c: Ctx, args: Args) -> CmdResult<()> {
    let Some(lines) = args.lines("lines").map(|l| l.to_vec()) else {
        if c.app(|a| a.ui.dialogs) {
            c.app(|a| a.emit(Event::UiAction { name: "SCRIPT".into(), args: Args::none() }));
        } else {
            c.log("No script selected.");
        }
        return Ok(());
    };
    c.log(&format!("Running script ({} lines)", lines.len()));
    // scripts feed the same input channel as the command line once this command has finished
    c.app(|a| a.queue_script(lines));
    Ok(())
}

// ---------------------------------------------------------------- delegates to the UI / file layer

async fn ui_action(c: Ctx, args: Args, name: &'static str) -> CmdResult<()> {
    if !c.app(|a| a.ui.dialogs) {
        c.log_error(&format!("{name} is not available in this environment."));
        return Ok(());
    }
    c.app(|a| a.emit(Event::UiAction { name: name.to_string(), args }));
    Ok(())
}

async fn palette(c: Ctx, args: Args, pal: &'static str) -> CmdResult<()> {
    let show = args.opt_bool("show");
    c.app(|a| a.emit(Event::Palette { name: pal.to_string(), show }));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(ds("U", run(u)).title("Undo").icon("undo").desc("Reverses the most recent action.").no_pickfirst());
    r.define(ds("UNDO", run(undo)).title("Undo").icon("undo").desc("Reverses the effect of commands.").no_pickfirst());
    r.define(ds("REDO", run(redo)).title("Redo").icon("redo").desc("Reverses the effects of the previous UNDO or U command.").no_pickfirst());
    r.define(ds("SETVAR", run(setvar)).aliases(&["SET"]).title("Set Variable").desc("Lists or changes the values of system variables.").no_pickfirst());

    let toggles: [(&'static str, &'static str, &'static str, &'static str, &'static str); 7] = [
        ("ORTHO", "ORTHOMODE", "Ortho", "Constrains cursor movement to the horizontal or vertical direction.", "ortho"),
        ("GRID", "GRIDMODE", "Grid", "Displays a grid pattern in the current viewport.", "grid"),
        ("SNAP", "SNAPMODE", "Snap", "Restricts cursor movement to specified intervals.", "snap"),
        ("POLAR", "POLARMODE", "Polar", "Restricts cursor movement to specified angle increments.", "polar"),
        ("OTRACK", "OTRACKMODE", "Object Snap Tracking", "Tracks from acquired object snap points.", "otrack"),
        ("LWDISPLAY", "LWDISPLAY", "Lineweight", "Displays or hides lineweights.", "lweight"),
        ("QUICKPROPERTIES", "QPMODE", "Quick Properties", "Displays the Quick Properties panel.", "qprops"),
    ];
    for (name, var, label, desc, icon) in toggles {
        r.define(ds(name, run(move |c, a| toggle(c, a, var, label))).title(label).icon(icon).desc(desc).transparent());
    }
    r.define(ds("DYN", run(dyn_)).title("Dynamic Input").icon("dyn").desc("Turns dynamic input on or off.").transparent());
    r.define(ds("OSNAPTOGGLE", run(osnaptoggle)).title("Object Snap").icon("osnap").desc("Turns running object snaps on or off.").transparent().once());
    r.define(ds("OSNAP", run(osnap)).aliases(&["OS"]).title("Object Snap Settings").icon("osnap").desc("Sets running object snap modes.").transparent());
    for (code, bit) in OS_BITS {
        if code == "QUI" || code == "APP" {
            continue;
        }
        let _ = os_bit(code);
        r.define(
            ds(&format!("OSNAP_{code}"), run(move |c, _a| async move {
                let m = c.pref("OSMODE") as i64 as u32;
                c.setvar("OSMODE", (m ^ bit) as f64)?;
                c.emit();
                Ok(())
            }))
            .title(os_full(code))
            .icon("osnap")
            .desc(&format!("Toggles the {} running object snap.", os_full(code)))
            .transparent()
            .once()
            .silent(),
        );
    }

    r.define(ds("SELECTALL", run(selectall)).title("Select All").desc("Selects all objects except those on locked or frozen layers.").once());
    r.define(ds("SELECTSIMILAR", run(selectsimilar)).aliases(&["SELSIM"]).title("Select Similar").desc("Adds similar objects to the selection set based on selected objects."));
    r.define(ds("DRAWORDER", run(draworder)).aliases(&["DR"]).title("Draw Order").icon("draworder").desc("Changes the draw order of images and other objects."));
    r.define(ds("TOFRONT", run(|c, _a| async move { c.sub_forced("DRAWORDER", Args::none().with("where", ArgVal::Str("Front".into()))).await; Ok(()) })).title("Bring to Front").icon("draworder").desc("Brings objects to the front."));
    r.define(ds("TOBACK", run(|c, _a| async move { c.sub_forced("DRAWORDER", Args::none().with("where", ArgVal::Str("Back".into()))).await; Ok(()) })).title("Send to Back").icon("draworder").desc("Sends objects to the back."));
    r.define(ds("SCRIPT", run(script)).aliases(&["SCR"]).title("Run Script").icon("script").desc("Executes a sequence of commands from a script file."));

    // palettes
    let pals: [(&'static str, &'static str, &[&str], &str, &str, &str); 3] = [
        ("PROPERTIES", "properties", &["PR", "MO", "CH", "PROPS", "DDMODIFY"], "Properties", "properties", "Controls properties of existing objects."),
        ("TOOLPALETTES", "tools", &["TP"], "Tool Palettes", "toolpal", "Opens the Tool Palettes window."),
        ("COMMANDLINE", "command", &[], "Command Line", "", "Displays the command line."),
    ];
    for (name, pal, aliases, title, icon, desc) in pals {
        r.define(ds(name, run(move |c, a| palette(c, a, pal))).aliases(aliases).title(title).icon(icon).desc(desc).transparent().once());
    }
    r.define(ds("CLEANSCREENON", run(|c, a| ui_action(c, a, "CLEANSCREENON"))).title("Clean Screen").icon("cleanscreen").desc("Clears the screen of toolbars and dockable windows.").transparent().once());

    let ui: [(&'static str, &[&str], &str, &str, &str); 24] = [
        ("DSETTINGS", &["DS", "SE", "RM"], "Drafting Settings", "dsettings", "Sets grid and snap, polar and object snap tracking, object snap modes, dynamic input, and quick properties."),
        ("OPTIONS", &["OP", "PREFERENCES"], "Options", "options", "Customizes the program settings."),
        ("UNITS", &["UN", "DDUNITS"], "Units", "units", "Controls coordinate and angle display formats and precision."),
        ("DIMSTYLE", &["D", "DST", "DDIM"], "Dimension Style", "dimstyle", "Creates and modifies dimension styles."),
        ("PLOT", &["PRINT"], "Plot", "plot", "Plots a drawing to a plotter, printer, or file."),
        ("NEW", &["QNEW"], "New", "new", "Creates a new drawing file."),
        ("OPEN", &[], "Open", "open", "Opens an existing drawing file."),
        ("QSAVE", &["SAVE"], "Save", "save", "Saves the current drawing."),
        ("SAVEAS", &[], "Save As", "saveas", "Saves a copy of the current drawing under a new file name."),
        ("DXFOUT", &[], "Export DXF", "dxf", "Writes the drawing as an ASCII DXF file."),
        ("EXPORT", &["EXP"], "Export", "export", "Saves the objects in a drawing to a different file format."),
        ("EXPORTPDF", &[], "Export to PDF", "pdf", "Exports the layout to a PDF file."),
        ("IMPORT", &["IMP", "DXFIN"], "Import", "import", "Imports files of various formats into the current drawing."),
        ("CLOSE", &[], "Close", "close", "Closes the current drawing."),
        ("QUIT", &["EXIT"], "Exit", "exit", "Exits the program."),
        ("QSELECT", &[], "Quick Select", "qselect", "Creates a selection set based on filtering criteria."),
        ("ALIASEDIT", &["AE"], "Edit Aliases", "aliasedit", "Defines and modifies command aliases."),
        ("STYLE", &["ST", "DDSTYLE"], "Text Style", "textstyle", "Creates, modifies, or specifies text styles."),
        ("LAYERSTATE", &["LAS"], "Layer States Manager", "layerstate", "Saves, restores, and manages named layer states."),
        ("ABOUT", &[], "About CadForge", "about", "Displays information about the application."),
        ("HELP", &["?"], "Help", "help", "Displays command and keyboard help."),
        ("MENUBAR", &[], "Menu Bar", "menubar", "Controls the display of the menu bar."),
        ("QUICKCALC", &["QC", "CAL"], "QuickCalc", "calc", "Opens the QuickCalc calculator."),
        ("PURGEDIALOG", &[], "Purge", "purge", "Purge unused named objects."),
    ];
    for (name, aliases, title, icon, desc) in ui {
        let mut d = ds(name, run(move |c, a| ui_action(c, a, name))).aliases(aliases).title(title).icon(icon).desc(desc);
        if ["QUIT", "CLOSE", "NEW", "OPEN", "QSAVE", "SAVEAS"].contains(&name) {
            d = d.once();
        }
        r.define(d);
    }
}
