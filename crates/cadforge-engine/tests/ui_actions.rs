//! Commands that switch a mode, open a palette or dialog, or hand work to the host UI.
//! They are checked through the system variables they change and the `Event`s they emit.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::app::Event;
use cadforge_engine::runtime::{ArgVal, Args, DialogReply, DialogReq};
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn events(s: &Sim) -> Vec<Event> {
    s.h.borrow_mut().take_events()
}

fn with_dialogs() -> Sim {
    let s = Sim::new();
    s.h.borrow_mut().ui.dialogs = true;
    s
}

fn ui_actions(s: &Sim) -> Vec<String> {
    events(s).into_iter().filter_map(|e| if let Event::UiAction { name, .. } = e { Some(name) } else { None }).collect()
}

#[test]
fn mode_toggles_flip_their_system_variable_and_report_it() {
    let s = Sim::new();
    for (cmd, var, label) in [
        ("ORTHO", "ORTHOMODE", "Ortho"),
        ("GRID", "GRIDMODE", "Grid"),
        ("SNAP", "SNAPMODE", "Snap"),
        ("POLAR", "POLARMODE", "Polar"),
        ("OTRACK", "OTRACKMODE", "Object Snap Tracking"),
        ("LWDISPLAY", "LWDISPLAY", "Lineweight"),
        ("QUICKPROPERTIES", "QPMODE", "Quick Properties"),
    ] {
        let before = s.pref(var) != 0.0;
        s.cmd(cmd);
        assert_eq!(s.pref(var) != 0.0, !before, "{cmd} flips {var}: {}", s.log_text());
        assert!(s.history_has(&format!("<{label} {}>", if before { "off" } else { "on" })), "{cmd}: {}", s.log_text());
        s.cmd(cmd);
        assert_eq!(s.pref(var) != 0.0, before, "{cmd} flips back");
    }
}

#[test]
fn toggles_work_in_the_middle_of_another_command() {
    let s = Sim::new();
    let before = s.pref("ORTHOMODE");
    s.cmd("LINE").click(0., 0.);
    s.cmd("ORTHO");
    assert_ne!(s.pref("ORTHOMODE"), before);
    assert_eq!(s.running().as_deref(), Some("LINE"), "the interrupted command carries on");
    s.click(10., 0.).enter();
    assert_eq!(s.n(), 1);
}

#[test]
fn dynamic_input_and_object_snap_toggles() {
    let s = Sim::new();
    let dyn_on = s.pref("DYNMODE") > 0.0;
    s.cmd("DYN");
    assert_eq!(s.pref("DYNMODE") > 0.0, !dyn_on, "{}", s.log_text());
    s.cmd("DYN");
    assert_eq!(s.pref("DYNMODE") > 0.0, dyn_on);

    let on = s.h.borrow().sysvars.osnap_on();
    s.cmd("OSNAPTOGGLE");
    assert_eq!(s.h.borrow().sysvars.osnap_on(), !on, "{}", s.log_text());
    s.cmd("OSNAPTOGGLE");
    assert_eq!(s.h.borrow().sysvars.osnap_on(), on);
}

#[test]
fn osnap_reports_the_mode_headless_and_asks_for_the_osnap_tab_with_a_ui() {
    let s = Sim::new();
    s.cmd("OSNAP");
    assert!(s.history_has("OSMODE = "), "{}", s.log_text());
    let d = with_dialogs();
    d.cmd("OSNAP");
    assert!(matches!(d.h.borrow().pending_dialog(), Some(DialogReq::DSettings { tab }) if tab == "osnap"));
    d.h.answer_dialog(DialogReply::Done);
    assert!(!d.busy());
}

#[test]
fn ucsicon_switches_the_icon_off_and_on() {
    let s = Sim::new();
    s.cmd_args("UCSICON", Args::none().with("option", ArgVal::Str("OFF".into())));
    assert_eq!(s.pref("UCSICON"), 0.0);
    s.cmd("UCSICON").ty("ON");
    assert_eq!(s.pref("UCSICON"), 1.0, "{}", s.log_text());
    assert!(events(&s).contains(&Event::Regen), "the viewport is regenerated");
}

#[test]
fn commands_that_need_the_host_ui_emit_one_ui_action_each() {
    let s = with_dialogs();
    for name in [
        "ABOUT", "ALIASEDIT", "CLEANSCREENON", "CLOSE", "DIMSTYLE", "EXPORT", "EXPORTPDF", "HELP", "IMPORT", "LAYERSTATE", "MENUBAR", "PURGEDIALOG", "QSELECT", "QUICKCALC", "QUIT", "SAVEAS", "UNITS",
    ] {
        s.cmd(name);
        assert_eq!(ui_actions(&s), vec![name.to_string()], "{name}: {}", s.log_text());
        assert!(!s.busy(), "{name} hands over and finishes");
    }
}

#[test]
fn without_a_host_ui_those_commands_say_so_instead_of_failing_silently() {
    let s = Sim::new();
    for name in ["SAVEAS", "UNITS", "DIMSTYLE"] {
        s.cmd(name);
        assert!(s.history_has(&format!("{name} is not available in this environment.")), "{name}: {}", s.log_text());
    }
    assert!(ui_actions(&s).is_empty());
}

#[test]
fn palette_commands_show_hide_and_restore_palettes() {
    let s = Sim::new();
    s.cmd("COMMANDLINE").cmd("TOOLPALETTES").cmd("RIBBON").cmd("TOOLPALETTESCLOSE").cmd("SHOWPALETTES");
    let pal: Vec<(String, Option<bool>)> = events(&s).into_iter().filter_map(|e| if let Event::Palette { name, show } = e { Some((name, show)) } else { None }).collect();
    assert_eq!(
        pal,
        vec![
            ("command".to_string(), None),
            ("tools".to_string(), None),
            ("ribbon".to_string(), Some(true)),
            ("tools".to_string(), Some(false)),
            ("show-all".to_string(), None),
        ]
    );
}

#[test]
fn redraw_repaints_and_regenall_regenerates() {
    let s = Sim::new();
    events(&s);
    s.cmd("REDRAW");
    let ev = events(&s);
    assert!(ev.contains(&Event::Change) && !ev.contains(&Event::Regen), "{ev:?}");
    s.cmd("REGENALL");
    let ev = events(&s);
    assert!(ev.contains(&Event::Regen) && ev.contains(&Event::Change), "{ev:?}");
}

#[test]
fn script_runs_its_lines_as_if_typed() {
    let s = Sim::new();
    s.cmd_args("SCRIPT", Args::none().with("lines", ArgVal::Lines(vec!["LINE".into(), "0,0".into(), "10,0".into(), "".into()])));
    assert!(s.history_has("Running script (4 lines)"), "{}", s.log_text());
    assert_eq!(s.n(), 1, "{}", s.log_text());
    let Kind::Line { a, b } = s.ent(0).kind else { panic!("a line") };
    assert_eq!((a, b), (p(0., 0.), p(10., 0.)));
    // with no file and no UI there is nothing to run; with a UI the file chooser is asked for
    s.cmd("SCRIPT");
    assert!(s.history_has("No script selected."));
    let d = with_dialogs();
    d.cmd("SCRIPT");
    assert_eq!(ui_actions(&d), vec!["SCRIPT".to_string()]);
}

#[test]
fn mesh_export_needs_solids_and_a_host_to_receive_the_file() {
    let s = with_dialogs();
    s.cmd("OBJOUT");
    assert!(s.history_has("There are no solids to export."), "{}", s.log_text());
    s.play("BOX", &[cadforge_engine::sim::Step::Click(0., 0.), cadforge_engine::sim::Step::Click(10., 10.), cadforge_engine::sim::Step::Type("10")]);
    assert_eq!(s.solids().len(), 1, "{}", s.log_text());
    events(&s);
    s.cmd("OBJOUT").cmd("STLOUT");
    let fmts: Vec<String> = events(&s).into_iter().filter_map(|e| if let Event::ExportMesh(f) = e { Some(f) } else { None }).collect();
    assert_eq!(fmts, vec!["obj".to_string(), "stl".to_string()]);
    let headless = Sim::new();
    headless.play("BOX", &[cadforge_engine::sim::Step::Click(0., 0.), cadforge_engine::sim::Step::Click(10., 10.), cadforge_engine::sim::Step::Type("10")]);
    headless.cmd("STLOUT");
    assert!(headless.history_has("STL export is not available in this environment."), "{}", headless.log_text());
}

#[test]
fn hatchedit_changes_the_pattern_of_an_existing_hatch_through_its_dialog() {
    let s = with_dialogs();
    s.add(Entity::pline_pts(&[p(0., 0.), p(20., 0.), p(20., 10.), p(0., 10.)], true));
    s.cmd("H").click(5., 5.).enter();
    let id = s.ents().into_iter().find(|e| matches!(e.kind, Kind::Hatch(_))).expect("a hatch").id;
    s.cmd_args("HATCHEDIT", Args::none().with("entity", ArgVal::Id(id)));
    assert!(matches!(s.h.borrow().pending_dialog(), Some(DialogReq::HatchEdit)), "{}", s.log_text());
    {
        let a = s.h.borrow();
        let hs = a.hatch_session.as_ref().expect("session");
        assert_eq!(hs.editing, Some(id));
        let mut m = hs.settings.borrow_mut();
        m.pattern = "SOLID".into();
        m.scale = 2.5;
    }
    s.h.answer_dialog(DialogReply::Done);
    assert!(!s.busy());
    assert!(s.h.borrow().hatch_session.is_none(), "the session ends with the dialog");
    let Kind::Hatch(h) = s.doc(|d| d.by_id(id).unwrap().kind.clone()) else { panic!() };
    assert_eq!(h.pattern, "SOLID");
    assert!((h.ps - 2.5).abs() < 1e-9);
    s.undo();
    let Kind::Hatch(h) = s.doc(|d| d.by_id(id).unwrap().kind.clone()) else { panic!() };
    assert_eq!(h.pattern, "ANSI31", "the edit is one undo step");
}

#[test]
fn save_as_and_export_dwg_hand_the_file_type_to_the_host() {
    let s = with_dialogs();
    s.cmd("EXPORTDWG");
    assert_eq!(ui_actions(&s), vec!["EXPORTDWG"]);
    s.cmd("DWGOUT");
    assert_eq!(ui_actions(&s), vec!["EXPORTDWG"], "DWGOUT is its alias");
    s.cmd_args("SAVEAS", Args::none().with("type", ArgVal::Str("dwg2000".into())));
    let ev: Vec<(String, Option<String>)> = events(&s).into_iter().filter_map(|e| if let Event::UiAction { name, args } = e { Some((name, args.str("type").map(String::from))) } else { None }).collect();
    assert_eq!(ev, vec![("SAVEAS".to_string(), Some("dwg2000".to_string()))]);
    // without a dialog host, -SAVEAS does the same on the command line
    let plain = Sim::new();
    plain.cmd("SAVEAS");
    assert!(plain.log_text().contains("not available"), "{}", plain.log_text());
}
