//! User interface customization: CUIEXPORT, CUIIMPORT, CUILOAD, CUIUNLOAD, and CUI / QUICKCUI / CUSTOMIZE handing over
//! to the host's dialog.
use cadforge_engine::app::Event;
use cadforge_engine::cui::Customization;
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::Sim;

fn file(name: &str, text: &str) -> Args {
    Args::none().with("file", ArgVal::Str(name.into())).with("text", ArgVal::Str(text.into()))
}

const TOOLS: &str = r#"{"cadforge_cui": 1,
  "qat": [{"cmd": "circle", "args": {"mode": "TTR"}}],
  "shortcuts": [{"keys": "ctrl+shift+c", "cmd": "circle"}],
  "panels": [{"title": "Site", "cmds": ["LINE", "RECTANG"]}]}"#;

#[test]
fn export_import_round_trip() {
    let s = Sim::new();
    {
        let mut a = s.h.borrow_mut();
        a.cui.add_qat("LINE", None);
        a.cui.set_shortcut("Alt+L", "line").unwrap();
    }
    s.h.borrow_mut().take_events();
    s.cmd("CUIEXPORT");
    let (name, data) = s.h.borrow_mut().take_events().into_iter().find_map(|e| if let Event::SaveFile { name, data, mime } = e { (mime == "application/json").then_some((name, data)) } else { None }).expect("a file");
    assert_eq!(name, "custom.cui.json");
    assert!(s.history_has("Customization exported to custom.cui.json: 1 toolbar button, 1 shortcut, 0 ribbon panels."), "{}", s.log_text());
    let text = String::from_utf8(data).unwrap();
    // a fresh session takes it in, and the file's own buttons are merged with what is there
    let t = Sim::new();
    t.cmd_args("CUIIMPORT", file("custom.cui.json", &text));
    assert_eq!(t.h.borrow().cui.main, Customization::from_json(&text).unwrap());
    t.cmd_args("CUIIMPORT", file("tools.cui.json", TOOLS));
    assert!(t.history_has("Imported from tools.cui.json: 1 toolbar button added, 1 shortcut added or changed, 1 ribbon panel added or changed."), "{}", t.log_text());
    let main = t.h.borrow().cui.main.clone();
    assert_eq!(main.qat.iter().map(|q| q.cmd.as_str()).collect::<Vec<_>>(), ["LINE", "CIRCLE"]);
    assert_eq!(main.qat[1].args.as_ref().and_then(|a| a.get("mode")).and_then(|m| m.as_str()), Some("TTR"), "the button keeps its arguments");
    assert_eq!(t.h.borrow().cui.shortcut("ctrl+shift+c").as_deref(), Some("CIRCLE"));
    // not a customization file
    t.cmd_args("CUIIMPORT", file("notes.json", "{\"a\": 1}"));
    assert!(t.history_has("notes: not a customization file (no \"cadforge_cui\" key)."), "{}", t.log_text());
}

#[test]
fn partial_customizations_load_and_unload() {
    let s = Sim::new();
    s.cmd_args("CUILOAD", file("C:/cui/tools.cui.json", TOOLS));
    assert!(s.history_has("Customization group \"tools\" loaded: 1 toolbar button, 1 shortcut, 1 ribbon panel."), "{}", s.log_text());
    {
        let a = s.h.borrow();
        assert!(a.cui.main.is_empty(), "a partial customization is not the user's own");
        let all = a.cui.effective();
        assert_eq!((all.qat.len(), all.panels[0].title.as_str()), (1, "Site"));
        assert_eq!(a.cui.shortcut("Ctrl+Shift+C").as_deref(), Some("CIRCLE"));
    }
    s.cmd_args("CUILOAD", file("tools.cui.json", TOOLS));
    assert!(s.history_has("Customization group \"tools\" reloaded"), "{}", s.log_text());
    assert_eq!(s.h.borrow().cui.partials.len(), 1);
    s.cmd("CUIUNLOAD").ty("nope");
    assert!(s.history_has("Customization group \"nope\" is not loaded."), "{}", s.log_text());
    s.cmd("CUIUNLOAD").enter();
    assert!(s.history_has("Customization group \"tools\" unloaded."), "{}", s.log_text());
    assert!(s.h.borrow().cui.effective().is_empty());
    s.cmd("CUIUNLOAD");
    assert!(s.history_has("No partial customization groups are loaded."));
}

#[test]
fn the_dialog_commands_hand_over_to_the_host() {
    let s = Sim::new();
    s.h.borrow_mut().ui.dialogs = true;
    s.h.borrow_mut().take_events();
    let asks = |s: &Sim| -> Vec<(String, Option<String>)> { s.h.borrow_mut().take_events().into_iter().filter_map(|e| if let Event::UiAction { name, args } = e { Some((name, args.str("tab").map(str::to_string))) } else { None }).collect() };
    s.cmd("CUI");
    assert_eq!(asks(&s), vec![("CUI".to_string(), Some("qat".to_string()))]);
    s.cmd("CUSTOMIZE");
    assert_eq!(asks(&s), vec![("CUI".to_string(), Some("panels".to_string()))]);
    s.cmd("QUICKCUI");
    assert_eq!(asks(&s), vec![("CUI".to_string(), Some("qat".to_string()))]);
    s.cmd("CUILOAD");
    assert_eq!(asks(&s), vec![("CUILOAD".to_string(), None)], "the host chooses the file");
    s.cmd("CUIIMPORT");
    assert_eq!(asks(&s), vec![("CUIIMPORT".to_string(), None)]);
    // without a UI the customization is listed
    let h = Sim::new();
    h.h.borrow_mut().cui.set_shortcut("F6", "pline").unwrap();
    h.cmd("CUI");
    assert!(h.history_has("Main customization: 0 toolbar buttons, 1 shortcut, 0 ribbon panels"), "{}", h.log_text());
    assert!(h.history_has("F6 = PLINE"));
}
