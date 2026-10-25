//! CAD standards: STANDARDS associates a standards drawing, CHECKSTANDARDS finds and fixes what differs from it, and
//! the association survives saving as JSON, DXF and DWG.
use cadforge_core::dim::{DimData, DimType};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::standards::{check, Standard, Table};
use cadforge_core::style::{Color, Layer};
use cadforge_engine::app::Event;
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::Sim;

fn layer(name: &str, aci: u8, ltype: &str) -> Layer {
    Layer { ltype: ltype.into(), ..Layer::new(name).with_color(Color::Aci(aci)) }
}

fn dim_on(style: &str) -> Entity {
    let mut d = DimData::new(DimType::Lin);
    (d.p1, d.p2, d.dp, d.ds) = (Some(pt(0.0, 0.0)), Some(pt(10.0, 0.0)), Some(pt(5.0, 5.0)), Some(style.into()));
    Entity::dim(d)
}

/// A drawing open next to "office-std", the standards drawing: WALLS has the wrong colour, Doors matches DOORS,
/// MyNotes has the properties of DOORS under another name, Misc has no standard counterpart, ISO-25 has the wrong
/// text height and Mine is not a standard dimension style.
fn with_standards_drawing() -> Sim {
    let s = Sim::new();
    s.h.borrow_mut().new_drawing(Some("office-std"), false);
    s.doc_mut(|d| {
        d.layers.push(Layer { lw: 50, ..layer("WALLS", 1, "Continuous") });
        d.layers.push(layer("DOORS", 3, "DASHED"));
        let mut iso = d.settings.cur_dim().clone();
        (iso.name, iso.txt) = ("ISO-25".into(), 3.5);
        d.settings.dim_styles.insert("ISO-25".into(), iso);
    });
    s.h.borrow_mut().set_active(0);
    s.doc_mut(|d| {
        d.layers.push(layer("WALLS", 5, "Continuous"));
        d.layers.push(layer("Doors", 3, "DASHED"));
        d.layers.push(layer("MyNotes", 3, "DASHED"));
        d.layers.push(layer("Misc", 7, "Continuous"));
        let mut iso = d.settings.cur_dim().clone();
        (iso.name, iso.txt) = ("ISO-25".into(), 2.5);
        d.settings.dim_styles.insert("ISO-25".into(), iso.clone());
        d.settings.dim_styles.insert("Mine".into(), cadforge_core::style::DimStyle { name: "Mine".into(), txt: 5.0, ..iso });
    });
    s.add(Entity::line(pt(0.0, 0.0), pt(10.0, 0.0)).on_layer("WALLS"));
    s.add(Entity::line(pt(0.0, 1.0), pt(10.0, 1.0)).on_layer("MyNotes"));
    s.add(Entity::line(pt(0.0, 2.0), pt(10.0, 2.0)).on_layer("Misc"));
    s.add(dim_on("Mine"));
    s
}

fn keys(s: &Sim) -> Vec<String> {
    s.doc(check).iter().map(|p| p.key()).collect()
}

#[test]
fn standards_associates_an_open_drawing_and_lists_it() {
    let s = with_standards_drawing();
    s.cmd("STANDARDS").ty("List");
    assert!(s.history_has("No standards files are associated"), "{}", s.log_text());
    s.cmd("STANDARDS").ty("Add");
    assert!(s.prompt().contains("<office-std>"), "{}", s.prompt());
    s.enter();
    assert!(s.history_has("Standards file associated with the drawing. office-std: 3 layers"), "{}", s.log_text());
    assert_eq!(s.doc(|d| d.standards.files.len()), 1);
    // adding it again replaces it
    s.cmd_args("STANDARDS", Args::none().with("option", ArgVal::Str("Add".into())).with("drawing", ArgVal::Str("office-std".into())));
    assert_eq!(s.doc(|d| d.standards.files.len()), 1);
    s.cmd("STANDARDS").ty("Remove").enter();
    assert_eq!(s.doc(|d| d.standards.files.len()), 0, "{}", s.log_text());
    assert!(s.history_has("Standards file \"office-std\" removed."));
}

#[test]
fn checking_reports_non_standard_names_and_properties() {
    let s = with_standards_drawing();
    s.cmd("CHECKSTANDARDS");
    assert!(s.history_has("No standards files are associated with this drawing"), "{}", s.log_text());
    s.cmd_args("STANDARDS", Args::none().with("option", ArgVal::Str("Add".into())).with("drawing", ArgVal::Str("office-std".into())));
    assert_eq!(keys(&s), ["Layer|WALLS", "Layer|MYNOTES", "Layer|MISC", "Dimension style|ISO-25", "Dimension style|MINE"], "Doors matches DOORS whatever the case");
    let probs = s.doc(check);
    assert_eq!(probs[0].table, Table::Layer);
    assert_eq!(probs[0].diffs, ["Color: Blue (standard: Red)", "Lineweight: 0.25 mm (standard: 0.50 mm)"], "{:?}", probs[0]);
    assert_eq!(probs[0].file.as_deref(), Some("office-std"));
    assert_eq!(probs[1].preferred.as_deref(), Some("DOORS"), "MyNotes has exactly the properties of DOORS");
    assert_eq!(probs[2].preferred, None);
    assert!(probs[3].diffs.iter().any(|d| d.starts_with("txt: 2.5 (standard: 3.5)")), "{:?}", probs[3].diffs);
    s.cmd("CHECKSTANDARDS").ty("None");
    assert!(s.history_has("5 problems found:"), "{}", s.log_text());
    assert!(s.history_has("Problem 2 of 5: Layer \"MyNotes\" is not in the standards files. Preferred fix: replace with \"DOORS\"."), "{}", s.log_text());
}

#[test]
fn fix_all_applies_the_preferred_fixes_in_one_undo_step() {
    let s = with_standards_drawing();
    s.cmd_args("STANDARDS", Args::none().with("option", ArgVal::Str("Add".into())).with("drawing", ArgVal::Str("office-std".into())));
    s.cmd("CHECKSTANDARDS").ty("All");
    assert!(s.history_has("3 problems fixed."), "{}", s.log_text());
    assert_eq!(keys(&s), ["Layer|MISC", "Dimension style|MINE"], "no preferred fix for these");
    let walls = s.doc(|d| d.layer("WALLS").cloned().unwrap());
    assert_eq!((walls.color, walls.lw), (Color::Aci(1), 50), "WALLS takes the standard properties");
    assert!(s.doc(|d| d.layer("MyNotes").is_none()));
    assert_eq!(s.ent(1).layer, "DOORS", "what was on MyNotes moved to DOORS");
    assert_eq!(s.doc(|d| d.settings.dim_styles["ISO-25"].txt), 3.5);
    s.undo();
    assert_eq!(keys(&s).len(), 5, "one undo step: {:?}", keys(&s));
    assert_eq!(s.ent(1).layer, "MyNotes");
}

#[test]
fn fixing_each_problem_lets_you_choose_the_replacement_or_ignore_it() {
    let s = with_standards_drawing();
    s.cmd_args("STANDARDS", Args::none().with("option", ArgVal::Str("Add".into())).with("drawing", ArgVal::Str("office-std".into())));
    s.cmd("CHECKSTANDARDS").ty("Each");
    // WALLS: Enter takes the preferred fix (the standard properties)
    assert!(s.prompt().contains("Replace with standard") && s.prompt().contains("<WALLS>"), "{}", s.prompt());
    s.enter();
    // MyNotes: skip it this time
    s.ty("Skip");
    // Misc: no preferred fix, so choose one
    assert!(s.prompt().contains("<Skip>"), "{}", s.prompt());
    s.ty("WALLS");
    // ISO-25: take the standard; Mine: ignore from now on
    s.enter();
    s.ty("Ignore");
    assert!(!s.busy(), "{}", s.log_text());
    assert!(s.history_has("Checking is complete. 3 problems fixed."), "{}", s.log_text());
    assert_eq!(s.ent(2).layer, "WALLS", "Misc was replaced by WALLS");
    assert!(s.doc(|d| d.layer("Misc").is_none()));
    assert_eq!(keys(&s), ["Layer|MYNOTES"], "Mine is ignored");
    // the host's dialog fixes one problem by its key
    s.cmd_args("CHECKSTANDARDS", Args::none().with("key", ArgVal::Str("Layer|MYNOTES".into())).with("with", ArgVal::Str("WALLS".into())));
    assert!(keys(&s).is_empty(), "{}", s.log_text());
    assert_eq!(s.ent(1).layer, "WALLS");
    s.cmd("CHECKSTANDARDS");
    assert!(s.history_has("Checking is complete. No standards problems were found."));
}

#[test]
fn a_dimension_style_replaced_by_a_standard_one_moves_its_dimensions() {
    let s = with_standards_drawing();
    s.cmd_args("STANDARDS", Args::none().with("option", ArgVal::Str("Add".into())).with("drawing", ArgVal::Str("office-std".into())));
    s.cmd_args("CHECKSTANDARDS", Args::none().with("key", ArgVal::Str("Dimension style|MINE".into())).with("with", ArgVal::Str("iso-25".into())));
    let Kind::Dim(d) = s.ent(3).kind else { panic!() };
    assert_eq!(d.ds.as_deref(), Some("ISO-25"), "{}", s.log_text());
    assert!(s.doc(|d| !d.settings.dim_styles.contains_key("Mine")));
    s.cmd_args("CHECKSTANDARDS", Args::none().with("key", ArgVal::Str("Layer|MISC".into())).with("with", ArgVal::Str("NOPE".into())));
    assert!(s.history_has("\"NOPE\" is not a standard layer."), "{}", s.log_text());
}

#[test]
fn the_ui_gets_the_dialogs_and_the_host_can_add_a_file_it_read() {
    let s = with_standards_drawing();
    s.h.borrow_mut().ui.dialogs = true;
    s.cmd("STANDARDS");
    let ev: Vec<String> = s.h.borrow_mut().take_events().into_iter().filter_map(|e| if let Event::UiAction { name, .. } = e { Some(name) } else { None }).collect();
    assert_eq!(ev, ["STANDARDS"]);
    // the host read a .dws file into a drawing and hands over its definitions
    let mut dws = Drawing::new("site");
    dws.layers.push(layer("WALLS", 5, "Continuous"));
    let json = serde_json::to_string(&Standard::from_drawing("C:/standards/site.dws", &dws)).unwrap();
    s.cmd_args("STANDARDS", Args::none().with("option", ArgVal::Str("Add".into())).with("json", ArgVal::Str(json)));
    assert_eq!(s.doc(|d| d.standards.files[0].file.clone()), "C:/standards/site.dws");
    assert!(!keys(&s).contains(&"Layer|WALLS".to_string()), "WALLS matches site.dws");
    s.cmd("CHECKSTANDARDS");
    let ev: Vec<String> = s.h.borrow_mut().take_events().into_iter().filter_map(|e| if let Event::UiAction { name, .. } = e { Some(name) } else { None }).collect();
    assert_eq!(ev, ["CHECKSTANDARDS"]);
}

#[test]
fn the_standards_files_are_saved_with_the_drawing() {
    let s = with_standards_drawing();
    s.cmd_args("STANDARDS", Args::none().with("option", ArgVal::Str("Add".into())).with("drawing", ArgVal::Str("office-std".into())));
    s.cmd_args("CHECKSTANDARDS", Args::none().with("key", ArgVal::Str("Layer|MISC".into())).with("ignore", ArgVal::Bool(true)));
    let d = s.doc(|d| d.clone());
    let want = d.standards.clone();
    assert_eq!(want.ignored, ["Layer|MISC"]);
    let json = Drawing::from_json_str(&d.to_json_string(), None).unwrap();
    assert_eq!(json.standards, want, "project file");
    let dxf = cadforge_core::io::read_dxf(&cadforge_core::io::write_dxf(&d), "t").unwrap();
    assert_eq!(dxf.standards, want, "DXF");
    let dwg = cadforge_core::io::read_dwg(&cadforge_core::io::write_dwg(&d).unwrap(), "t").unwrap();
    assert_eq!(dwg.standards, want, "DWG");
    let k = |d: &Drawing| check(d).iter().map(|p| p.key()).collect::<Vec<_>>();
    assert_eq!(k(&dxf), s.doc(k), "the reopened DXF has the same problems");
    assert_eq!(k(&dwg), s.doc(k), "the reopened DWG has the same problems");
}
