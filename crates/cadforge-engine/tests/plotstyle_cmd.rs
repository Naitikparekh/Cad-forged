//! PLOTSTYLE: load, list, monochrome and remove the drawing's plot style table.
use base64::Engine;
use cadforge_engine::app::Event;
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::Sim;

fn table_text() -> String {
    let mut t = String::from("description=\"Office table\"\naci_table_available=TRUE\nlineweight_table{\n\t0=0.000000\n\t1=0.250000\n}\naci_table{\n");
    for i in 0..255 {
        t.push_str(&format!("\t{i}=\"Color_{}\"\n", i + 1));
    }
    t.push_str("}\nplot_style{\n");
    for i in 0..255 {
        let (color, lw) = if i == 0 { (-1006632960_i64, 1) } else { (-1, 0) };
        t.push_str(&format!("\t{i}{{\n\t\tname=\"Color_{}\"\n\t\tcolor={color}\n\t\tscreen=100\n\t\tlineweight={lw}\n\t}}\n", i + 1));
    }
    t.push_str("}\n");
    t
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn load(s: &Sim, name: &str, bytes: &[u8]) {
    s.cmd_args("PLOTSTYLE", Args::none().with("option", ArgVal::Str("Load".into())).with("name", ArgVal::Str(name.into())).with("data", ArgVal::Str(b64(bytes))));
}

#[test]
fn a_drawing_starts_without_a_table_and_says_what_that_means() {
    let s = Sim::new();
    s.cmd("PLOTSTYLE").ty("?");
    assert!(s.history_has("No plot style table"), "{}", s.log_text());
    assert!(s.doc(|d| d.plot_styles.is_none()));
}

#[test]
fn load_reads_a_table_from_the_file_bytes_and_names_it_after_the_file() {
    let s = Sim::new();
    load(&s, "C:\\plot styles\\Office.ctb", table_text().as_bytes());
    assert!(s.history_has("Plot style table \"Office\" (colour dependent, 255 styles) will be used when plotting."), "{}", s.log_text());
    let t = s.doc(|d| d.plot_styles.clone()).unwrap();
    assert_eq!(t.name, "Office");
    assert_eq!(t.styles[0].color, Some([0, 0, 0]));
    assert_eq!(t.styles[0].lineweight, Some(25));
    assert!(s.doc(|d| d.dirty), "loading changes the drawing");
    s.cmd("PLOTSTYLE").ty("?");
    assert!(s.history_has("Plot style table \"Office\": colour dependent, 255 styles. Office table"), "{}", s.log_text());
    assert!(s.history_has("colour   1: 0,0,0, 0.25 mm, screening 100%"), "{}", s.log_text());
}

#[test]
fn a_bad_file_leaves_the_drawing_alone_and_says_why() {
    let s = Sim::new();
    load(&s, "junk.ctb", b"definitely not a plot style table");
    assert!(s.history_has("no plot styles"), "{}", s.log_text());
    assert!(s.doc(|d| d.plot_styles.is_none()));
    s.cmd_args("PLOTSTYLE", Args::none().with("option", ArgVal::Str("Load".into())).with("data", ArgVal::Str("???".into())));
    assert!(s.history_has("not valid base64"));
}

#[test]
fn monochrome_and_none_and_undo() {
    let s = Sim::new();
    s.cmd("PLOTSTYLE").ty("Monochrome");
    assert_eq!(s.doc(|d| d.plot_styles.as_ref().map(|t| t.name.clone())), Some("monochrome".to_string()));
    s.cmd("PLOTSTYLE").ty("None");
    assert!(s.doc(|d| d.plot_styles.is_none()));
    assert!(s.history_has("The plot style table was removed"));
    s.cmd("PLOTSTYLE").ty("None");
    assert!(s.history_has("The drawing has no plot style table."));
    s.undo();
    assert!(s.doc(|d| d.plot_styles.is_some()), "undo brings the table back");
}

#[test]
fn a_named_table_loads_but_warns_that_objects_cannot_use_it() {
    let s = Sim::new();
    let named = "description=\"named\"\nplot_style{\n\t0{\n\t\tname=\"Normal\"\n\t\tcolor=-1\n\t\tscreen=100\n\t\tlineweight=0\n\t}\n}\n";
    load(&s, "Named.stb", named.as_bytes());
    assert!(s.history_has("(named, 1 style)"), "{}", s.log_text());
    assert!(s.history_has("objects have no plot style assignment"), "{}", s.log_text());
}

#[test]
fn without_data_the_host_is_asked_for_a_file() {
    let s = Sim::new();
    s.h.borrow_mut().ui.dialogs = true;
    s.h.borrow_mut().take_events();
    s.cmd_args("PLOTSTYLE", Args::none().with("option", ArgVal::Str("Load".into())));
    let asks: Vec<String> = s.h.borrow_mut().take_events().into_iter().filter_map(|e| if let Event::UiAction { name, .. } = e { Some(name) } else { None }).collect();
    assert_eq!(asks, vec!["PLOTSTYLELOAD".to_string()]);
    let h = Sim::new();
    h.cmd_args("PLOTSTYLE", Args::none().with("option", ArgVal::Str("Load".into())));
    assert!(h.history_has("needs a file chooser"), "{}", h.log_text());
}
