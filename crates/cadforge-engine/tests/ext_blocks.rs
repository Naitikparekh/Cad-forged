//! Block, attribute and external-reference commands (ATTDEF, ATTEDIT, EATTEDIT, ATTDISP, ATTSYNC, BATTMAN, BEDIT,
//! BCLOSE, BASE, XREF, XATTACH, XBIND) and attribute prompting on INSERT / BLOCK.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::AttDisp;
use cadforge_core::text::{AttDef, TextData};
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn attdef(tag: &str, prompt: &str, default: &str, at: Pt) -> Entity {
    let mut t = TextData::new(at, 2.0, default);
    t.att = Some(AttDef { tag: tag.into(), prompt: prompt.into(), invisible: false, constant: false, verify: false, preset: false });
    Entity::new(Kind::Text(t))
}

fn with_modes(mut e: Entity, invisible: bool, constant: bool, verify: bool, preset: bool) -> Entity {
    if let Kind::Text(t) = &mut e.kind {
        let a = t.att.as_mut().unwrap();
        (a.invisible, a.constant, a.verify, a.preset) = (invisible, constant, verify, preset);
    }
    e
}

/// Block DOOR: a circle with one attribute NUM (prompt "Door number", default D1) beside it.
fn door(s: &Sim) {
    s.doc_mut(|d| d.define_block("DOOR", p(0., 0.), vec![Entity::circle(p(0., 0.), 5.), attdef("NUM", "Door number", "D1", p(6., 0.))]));
}

fn attribs(e: &Entity) -> Vec<AttribData> {
    match &e.kind {
        Kind::Insert(i) => i.attribs.clone(),
        k => panic!("not a block reference: {k:?}"),
    }
}

fn values(e: &Entity) -> Vec<(String, String)> {
    attribs(e).into_iter().map(|a| (a.tag, a.t.s)).collect()
}

fn inserts(s: &Sim) -> Vec<Entity> {
    s.ents().into_iter().filter(|e| matches!(e.kind, Kind::Insert(_))).collect()
}

/// Insert DOOR at (x, y), answering the attribute prompt with `value` (Enter keeps the default).
fn insert_door(s: &Sim, x: f64, y: f64, value: &str) {
    s.cmd("INSERT").ty("DOOR").click(x, y);
    if value.is_empty() {
        s.enter();
    } else {
        s.ty(value);
    }
    assert!(!s.busy(), "{}", s.prompt());
}

/// A host drawing plus an open second drawing "plan" holding one WALLS line; the host is active.
fn with_plan() -> Sim {
    let s = Sim::new();
    s.h.borrow_mut().new_drawing(Some("plan"), false);
    s.doc_mut(|d| {
        d.add_layer("WALLS").unwrap();
    });
    s.add(Entity::line(p(0., 0.), p(10., 0.)).on_layer("WALLS"));
    s.h.borrow_mut().set_active(0);
    s
}

fn xref_info(s: &Sim, name: &str) -> Option<cadforge_core::drawing::XrefInfo> {
    s.doc(|d| d.blocks.get(name).and_then(|b| b.xref.clone()))
}

// ---------------------------------------------------------------- registry

#[test]
fn commands_and_aliases_are_registered() {
    let s = Sim::new();
    let r = s.registry();
    for name in ["ATTDEF", "ATTEDIT", "EATTEDIT", "ATTDISP", "ATTSYNC", "BATTMAN", "BEDIT", "BCLOSE", "BASE", "XREF", "XATTACH", "XBIND"] {
        let d = r.find(name).unwrap_or_else(|| panic!("{name} missing"));
        assert!(!d.title.is_empty() && !d.desc.is_empty() && d.group == "Block", "{name}");
    }
    for (alias, name) in
        [("ATT", "ATTDEF"), ("-ATT", "ATTDEF"), ("ATE", "ATTEDIT"), ("-ATE", "ATTEDIT"), ("BE", "BEDIT"), ("XA", "XATTACH"), ("XR", "XREF"), ("-XR", "XREF"), ("XB", "XBIND"), ("-XB", "XBIND")]
    {
        assert_eq!(r.find(alias).map(|d| d.name.clone()).as_deref(), Some(name), "{alias}");
    }
    // the wrapped commands keep their names and aliases
    assert_eq!(r.find("I").map(|d| d.name.clone()).as_deref(), Some("INSERT"));
    assert_eq!(r.find("B").map(|d| d.name.clone()).as_deref(), Some("BLOCK"));
}

// ---------------------------------------------------------------- ATTDEF

#[test]
fn attdef_prompts_and_creates_a_tagged_text() {
    let s = Sim::new();
    s.cmd("ATTDEF");
    assert!(s.prompt().contains("Enter an option to change"), "{}", s.prompt());
    s.ty("i").ty("v").enter(); // Invisible and Verify
    assert!(s.history_has("Invisible=Y Constant=N Verify=Y Preset=N"), "{}", s.log_text());
    assert_eq!(s.message(), "Enter attribute tag name");
    s.ty("room");
    assert_eq!(s.message(), "Enter attribute prompt");
    s.ty("Room number");
    assert_eq!(s.message(), "Enter default attribute value");
    s.ty("101");
    assert!(s.prompt().contains("Specify start point of text"), "{}", s.prompt());
    s.click(10., 5.);
    assert!(s.prompt().starts_with("Specify height"), "{}", s.prompt());
    s.ty("3");
    assert!(s.prompt().starts_with("Specify rotation angle of text"), "{}", s.prompt());
    s.ty("90");
    assert!(!s.busy());
    assert_eq!(s.n(), 1);
    let Kind::Text(t) = &s.ent(0).kind else { panic!() };
    let a = t.att.clone().expect("attribute definition");
    assert_eq!((a.tag.as_str(), a.prompt.as_str()), ("ROOM", "Room number"));
    assert_eq!((a.invisible, a.constant, a.verify, a.preset), (true, false, true, false));
    assert_eq!(t.s, "101");
    near(t.h, 3.0);
    near(t.rot, std::f64::consts::FRAC_PI_2);
    assert_eq!((t.p.x, t.p.y), (10.0, 5.0));
}

#[test]
fn attdef_constant_mode_asks_for_the_value_only_and_justify_applies() {
    let s = Sim::new();
    s.cmd("ATT").ty("c").enter().ty("rev");
    assert_eq!(s.message(), "Enter attribute value");
    s.ty("A").ty("j").ty("mc").click(0., 0.).ty("2").ty("0");
    assert!(!s.busy());
    let Kind::Text(t) = &s.ent(0).kind else { panic!() };
    let a = t.att.as_ref().unwrap();
    assert!(a.constant && a.prompt.is_empty());
    assert_eq!(t.s, "A");
    assert_eq!((t.jh, t.jv), (cadforge_core::text::HJust::Center, cadforge_core::text::VJust::Middle));
}

#[test]
fn attdef_rejects_tags_with_spaces_and_is_one_undo_step() {
    let s = Sim::new();
    s.cmd("ATTDEF").enter().ty("bad tag");
    assert!(!s.busy());
    assert!(s.history_has("cannot contain spaces"));
    assert_eq!(s.n(), 0);
    s.cmd("ATTDEF").enter().ty("ok").ty("P").ty("v").click(1., 1.).ty("2").ty("0");
    assert_eq!(s.n(), 1);
    s.undo();
    assert_eq!(s.n(), 0);
}

// ---------------------------------------------------------------- attribute values on INSERT / BLOCK

#[test]
fn insert_prompts_for_attribute_values_and_places_them() {
    let s = Sim::new();
    door(&s);
    s.cmd("INSERT").ty("DOOR").click(50., 50.);
    assert!(s.busy());
    assert_eq!(s.prompt(), "Door number <D1>:");
    s.ty("17");
    assert!(!s.busy());
    let ins = inserts(&s);
    assert_eq!(ins.len(), 1);
    let a = &attribs(&ins[0])[0];
    assert_eq!((a.tag.as_str(), a.t.s.as_str(), a.hidden), ("NUM", "17", false));
    near(a.t.p.x, 56.0);
    near(a.t.p.y, 50.0);
    near(a.t.h, 2.0);
    assert!(a.t.att.is_none());
    // Enter keeps the default
    insert_door(&s, 80., 0., "");
    assert_eq!(values(&inserts(&s)[1]), vec![("NUM".to_string(), "D1".to_string())]);
}

#[test]
fn inserted_attributes_follow_scale_and_rotation_of_the_reference() {
    let s = Sim::new();
    door(&s);
    s.cmd("INSERT").ty("DOOR").ty("s").ty("2").ty("r").ty("90").click(10., 10.).enter();
    let a = &attribs(&inserts(&s)[0])[0];
    near(a.t.p.x, 10.0);
    near(a.t.p.y, 22.0); // (6, 0) scaled by 2 and turned 90 degrees
    near(a.t.h, 4.0);
    near(a.t.rot, std::f64::consts::FRAC_PI_2);
}

#[test]
fn constant_preset_and_invisible_attributes() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.define_block(
            "TAG",
            p(0., 0.),
            vec![
                with_modes(attdef("CONST", "Never asked", "FIXED", p(0., 0.)), false, true, false, false),
                with_modes(attdef("PRE", "Never asked", "PRESET", p(0., 3.)), false, false, false, true),
                with_modes(attdef("HID", "Hidden one", "H", p(0., 6.)), true, false, false, false),
                attdef("VIS", "Visible one", "V", p(0., 9.)),
            ],
        )
    });
    s.cmd("INSERT").ty("TAG").click(0., 0.);
    assert_eq!(s.prompt(), "Hidden one <H>:"); // constants and presets are not prompted
    s.ty("hh");
    assert_eq!(s.prompt(), "Visible one <V>:");
    s.enter();
    assert!(!s.busy());
    let at = attribs(&inserts(&s)[0]);
    let got: Vec<(&str, &str, bool)> = at.iter().map(|a| (a.tag.as_str(), a.t.s.as_str(), a.hidden)).collect();
    assert_eq!(got, vec![("CONST", "FIXED", false), ("PRE", "PRESET", false), ("HID", "hh", true), ("VIS", "V", false)]);
}

#[test]
fn verify_mode_asks_twice() {
    let s = Sim::new();
    s.doc_mut(|d| d.define_block("V", p(0., 0.), vec![with_modes(attdef("T", "Value", "x", p(0., 0.)), false, false, true, false)]));
    s.cmd("INSERT").ty("V").click(0., 0.).ty("first");
    assert_eq!(s.prompt(), "Verify Value <first>:");
    s.ty("second");
    assert_eq!(values(&inserts(&s)[0])[0].1, "second");
}

#[test]
fn escape_during_attribute_prompts_cancels_the_insertion() {
    let s = Sim::new();
    door(&s);
    s.cmd("INSERT").ty("DOOR").click(0., 0.);
    s.esc();
    assert!(!s.busy());
    assert_eq!(s.n(), 0);
}

#[test]
fn insert_without_attribute_definitions_is_unchanged() {
    let s = Sim::new();
    s.doc_mut(|d| d.define_block("PLAIN", p(0., 0.), vec![Entity::circle(p(0., 0.), 1.)]));
    s.cmd("I").ty("PLAIN").click(5., 5.);
    assert!(!s.busy());
    assert!(attribs(&inserts(&s)[0]).is_empty());
}

#[test]
fn block_command_prompts_for_the_attribute_values_of_the_new_reference() {
    let s = Sim::new();
    let ids = [s.add(Entity::circle(p(0., 0.), 5.)), s.add(attdef("NUM", "Door number", "D1", p(6., 0.)))];
    s.set_sel(&ids);
    s.cmd("BLOCK").ty("DOOR").click(0., 0.);
    assert!(s.busy(), "the new reference asks for its attribute");
    assert_eq!(s.prompt(), "Door number <D1>:");
    s.ty("9");
    assert!(!s.busy());
    assert_eq!(values(&s.ent(0)), vec![("NUM".to_string(), "9".to_string())]);
    assert_eq!(s.doc(|d| d.blocks["DOOR"].entities.len()), 2);
}

#[test]
fn fill_attributes_is_reusable_and_idempotent() {
    use cadforge_engine::app::{App, Handle};
    use cadforge_engine::commands::{build_registry, ext_blocks::fill_attributes, run};
    use cadforge_engine::registry::CommandDef;
    use cadforge_engine::runtime::{ArgVal, Args};
    // a command that completes a reference the way INSERT does, asking for values or not
    let mut reg = build_registry();
    reg.define(CommandDef::new("FILLTEST", run(|c, a| async move { fill_attributes(&c, a.id("id").unwrap(), a.flag("ask")).await })));
    let h = Handle::new(App::new(std::rc::Rc::new(reg)));
    h.borrow_mut().new_drawing(None, false);
    let s = Sim { h };
    door(&s);
    let (a, b) = (s.add(Entity::insert("DOOR", p(0., 0.))), s.add(Entity::insert("DOOR", p(20., 0.))));
    let fill = |id: u64, ask: bool| Args::none().with("id", ArgVal::Id(id)).with("ask", ArgVal::Bool(ask));
    s.cmd_args("FILLTEST", fill(a, true));
    assert_eq!(s.prompt(), "Door number <D1>:");
    s.ty("X");
    assert_eq!(values(&s.ents()[0]), vec![("NUM".to_string(), "X".to_string())]);
    s.cmd_args("FILLTEST", fill(a, true)); // already complete: nothing to ask
    assert!(!s.busy());
    s.cmd_args("FILLTEST", fill(b, false)); // defaults without prompting
    assert!(!s.busy());
    assert_eq!(values(&s.ents()[1]), vec![("NUM".to_string(), "D1".to_string())]);
    assert_eq!(values(&s.ents()[0])[0].1, "X");
}

// ---------------------------------------------------------------- ATTEDIT / EATTEDIT

fn two_doors(s: &Sim) {
    door(s);
    insert_door(s, 0., 0., "A1");
    insert_door(s, 50., 0., "B2");
}

#[test]
fn attedit_one_at_a_time_replaces_a_value() {
    let s = Sim::new();
    two_doors(&s);
    s.cmd("ATTEDIT");
    assert!(s.prompt().contains("Edit attributes one at a time?"), "{}", s.prompt());
    s.enter(); // Yes
    assert_eq!(s.message(), "Enter block name specification");
    s.enter();
    assert_eq!(s.message(), "Enter attribute tag specification");
    s.enter();
    assert_eq!(s.message(), "Enter attribute value specification");
    s.enter();
    assert_eq!(s.message(), "Select Attributes");
    s.click(5., 0.).enter();
    assert!(s.history_has("1 attribute selected."));
    assert!(s.prompt().starts_with("Enter an option"), "{}", s.prompt());
    s.ty("v");
    assert!(s.prompt().contains("Enter type of value modification"), "{}", s.prompt());
    s.enter(); // Replace
    s.ty("Z9");
    assert!(s.prompt().starts_with("Enter an option"));
    s.enter(); // Next
    assert!(!s.busy());
    assert_eq!(values(&inserts(&s)[0]), vec![("NUM".to_string(), "Z9".to_string())]);
    assert_eq!(values(&inserts(&s)[1])[0].1, "B2");
    s.undo();
    assert_eq!(values(&inserts(&s)[0])[0].1, "A1");
}

#[test]
fn attedit_modify_position_height_and_angle() {
    let s = Sim::new();
    two_doors(&s);
    s.cmd("ATTEDIT").enter().enter().enter().enter().click(5., 0.).enter();
    s.ty("v").ty("m").ty("A").ty("Q"); // change string A to Q inside A1
    assert_eq!(values(&inserts(&s)[0])[0].1, "Q1");
    s.ty("p").click(1., 7.);
    s.ty("h").ty("4");
    s.ty("a").ty("45");
    s.enter();
    assert!(!s.busy());
    let a = &attribs(&inserts(&s)[0])[0];
    assert_eq!((a.t.p.x, a.t.p.y), (1.0, 7.0));
    near(a.t.h, 4.0);
    near(a.t.rot, std::f64::consts::FRAC_PI_4);
}

#[test]
fn attedit_specifications_filter_and_global_edit_changes_strings() {
    let s = Sim::new();
    two_doors(&s);
    // global edit of every value containing "1"
    s.cmd("ATTEDIT").ty("n").enter(); // not one at a time; visible only
    s.enter().enter().ty("*1*");
    s.click(5., 0.).click(55., 0.).enter();
    assert!(s.history_has("1 attribute selected."), "{}", s.log_text());
    s.ty("A").ty("Door-");
    assert!(!s.busy());
    assert_eq!(values(&inserts(&s)[0])[0].1, "Door-1");
    assert_eq!(values(&inserts(&s)[1])[0].1, "B2");
    // tag specification that matches nothing
    s.cmd("ATTEDIT").enter().enter().ty("NOPE").enter().click(5., 0.).enter();
    assert!(s.history_has("0 attributes selected."));
    assert!(!s.busy());
}

#[test]
fn attedit_skips_constant_attributes() {
    let s = Sim::new();
    s.doc_mut(|d| d.define_block("C", p(0., 0.), vec![Entity::circle(p(0., 0.), 3.), with_modes(attdef("K", "k", "FIXED", p(0., 0.)), false, true, false, false)]));
    s.cmd("INSERT").ty("C").click(0., 0.);
    s.cmd("ATTEDIT").enter().enter().enter().enter().click(3., 0.).enter();
    assert!(s.history_has("0 attributes selected."));
}

#[test]
fn eattedit_edits_by_tag_and_all() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.define_block(
            "T2",
            p(0., 0.),
            vec![
                Entity::circle(p(0., 0.), 3.),
                attdef("A", "First", "a", p(0., 5.)),
                with_modes(attdef("K", "Const", "FIXED", p(0., 8.)), false, true, false, false),
                attdef("B", "Second", "b", p(0., 11.)),
            ],
        )
    });
    s.cmd("INSERT").ty("T2").click(0., 0.).enter().enter();
    assert!(!s.busy());
    s.cmd("EATTEDIT").click(3., 0.);
    assert!(s.history_has("(constant)"), "{}", s.log_text());
    assert!(s.prompt().contains("Enter attribute tag to edit"), "{}", s.prompt());
    s.ty("b");
    assert_eq!(s.prompt(), "Enter new value for B <b>:");
    s.ty("two");
    s.ty("K");
    assert!(s.history_has("constant attribute and cannot be edited"));
    s.ty("all");
    s.ty("one");
    assert_eq!(s.prompt(), "Enter new value for B <two>:"); // the constant is skipped
    s.enter();
    s.enter(); // done
    assert!(!s.busy());
    let got: Vec<(String, String)> = values(&inserts(&s)[0]);
    assert_eq!(got, vec![("A".into(), "one".into()), ("K".into(), "FIXED".into()), ("B".into(), "two".into())]);
}

// ---------------------------------------------------------------- ATTDISP

#[test]
fn attdisp_off_on_normal_hide_and_reveal_attributes() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.define_block("M", p(0., 0.), vec![Entity::circle(p(0., 0.), 3.), attdef("SEEN", "s", "1", p(0., 5.)), with_modes(attdef("HIDDEN", "h", "2", p(0., 8.)), true, false, false, false)])
    });
    s.cmd("INSERT").ty("M").click(0., 0.).enter().enter();
    let hidden = |s: &Sim| attribs(&inserts(s)[0]).iter().map(|a| a.hidden).collect::<Vec<_>>();
    assert_eq!(hidden(&s), vec![false, true]);
    s.cmd("ATTDISP");
    assert!(s.prompt().contains("Enter attribute visibility setting"), "{}", s.prompt());
    assert!(s.prompt().contains("<Normal>"));
    s.ty("on");
    assert_eq!(hidden(&s), vec![false, false]);
    assert_eq!(s.doc(|d| d.settings.attdisp), AttDisp::On);
    s.cmd("ATTDISP");
    assert!(s.prompt().contains("<ON>"));
    s.ty("off");
    assert_eq!(hidden(&s), vec![true, true]);
    // new references follow the mode
    s.cmd("INSERT").ty("M").click(40., 0.).enter().enter();
    assert!(attribs(&inserts(&s)[1]).iter().all(|a| a.hidden));
    s.cmd("ATTDISP").ty("normal");
    assert_eq!(hidden(&s), vec![false, true]);
    assert_eq!(s.doc(|d| d.settings.attdisp), AttDisp::Normal);
    s.undo();
    assert_eq!(s.doc(|d| d.settings.attdisp), AttDisp::Off);
    assert_eq!(hidden(&s), vec![true, true]);
}

#[test]
fn attdisp_is_saved_with_the_drawing() {
    let s = Sim::new();
    s.cmd("ATTDISP").ty("on");
    let json = s.doc(|d| d.to_json_string());
    assert!(json.contains("\"attdisp\":\"on\""));
    let back = Drawing::from_json_str(&json, None).unwrap();
    assert_eq!(back.settings.attdisp, AttDisp::On);
    assert!(!Drawing::new("n").to_json_string().contains("attdisp"));
}

// ---------------------------------------------------------------- ATTSYNC

#[test]
fn attsync_adds_new_and_removes_obsolete_attributes_keeping_values() {
    let s = Sim::new();
    door(&s);
    insert_door(&s, 0., 0., "A1");
    insert_door(&s, 50., 0., "B2");
    // redefine DOOR: NUM stays, LOCK is new
    s.doc_mut(|d| d.change(|d| d.blocks.get_mut("DOOR").unwrap().entities.push(attdef("LOCK", "Lock", "std", p(6., -3.)))));
    s.cmd("ATTSYNC");
    assert!(s.prompt().contains("Enter an option"), "{}", s.prompt());
    s.enter(); // Select
    s.click(5., 0.);
    assert!(!s.busy());
    for (i, v) in [(0, "A1"), (1, "B2")] {
        assert_eq!(values(&inserts(&s)[i]), vec![("NUM".to_string(), v.to_string()), ("LOCK".to_string(), "std".to_string())], "reference {i}");
    }
    near(attribs(&inserts(&s)[1])[1].t.p.x, 56.0);
    near(attribs(&inserts(&s)[1])[1].t.p.y, -3.0);
    // drop NUM again
    s.doc_mut(|d| d.change(|d| d.blocks.get_mut("DOOR").unwrap().entities.retain(|e| !matches!(&e.kind, Kind::Text(t) if t.att.as_ref().is_some_and(|a| a.tag == "NUM")))));
    s.cmd("ATTSYNC").ty("n").ty("door");
    assert!(!s.busy());
    assert_eq!(values(&inserts(&s)[0]), vec![("LOCK".to_string(), "std".to_string())]);
    assert!(s.history_has("2 references updated"), "{}", s.log_text());
    s.undo();
    assert_eq!(attribs(&inserts(&s)[0]).len(), 2);
}

#[test]
fn attsync_lists_blocks_and_rejects_unknown_names() {
    let s = Sim::new();
    door(&s);
    insert_door(&s, 0., 0., "A1");
    s.cmd("ATTSYNC").ty("?");
    assert!(s.history_has("DOOR: 1 attribute definition, 1 reference"), "{}", s.log_text());
    s.ty("n").ty("missing");
    assert!(s.history_has("Block \"missing\" not found."));
    assert!(!s.busy());
}

// ---------------------------------------------------------------- BATTMAN

#[test]
fn battman_reorders_edits_and_removes_attribute_definitions() {
    let s = Sim::new();
    s.doc_mut(|d| d.define_block("BM", p(0., 0.), vec![Entity::circle(p(0., 0.), 3.), attdef("A", "First", "a", p(0., 5.)), attdef("B", "Second", "b", p(0., 8.))]));
    s.cmd("INSERT").ty("BM").click(0., 0.).enter().enter();
    let tags = |s: &Sim| s.doc(|d| d.blocks["BM"].entities.iter().filter_map(|e| if let Kind::Text(t) = &e.kind { t.att.as_ref().map(|a| a.tag.clone()) } else { None }).collect::<Vec<_>>());
    s.cmd("BATTMAN").click(3., 0.);
    assert!(s.history_has("Attributes of block \"BM\""));
    s.ty("d").ty("a"); // move A down
    assert_eq!(tags(&s), vec!["B", "A"]);
    s.ty("e").ty("b"); // edit B: prompt, toggle invisible, tag
    s.ty("p").ty("Renamed");
    s.ty("i");
    s.ty("t").ty("bb");
    s.enter(); // done editing
    assert_eq!(tags(&s), vec!["BB", "A"]);
    s.ty("r").ty("a"); // remove A
    assert_eq!(tags(&s), vec!["BB"]);
    s.enter(); // exit: references are synchronized
    assert!(!s.busy());
    let at = attribs(&inserts(&s)[0]);
    assert_eq!(at.len(), 1);
    assert_eq!((at[0].tag.as_str(), at[0].hidden), ("BB", true));
    let Kind::Text(t) = &s.doc(|d| d.blocks["BM"].entities[1].clone()).kind else { panic!() };
    assert_eq!(t.att.as_ref().unwrap().prompt, "Renamed");
    s.undo();
    assert_eq!(tags(&s), vec!["A", "B"]);
}

// ---------------------------------------------------------------- BEDIT / BCLOSE / BASE

#[test]
fn bedit_opens_the_block_in_a_new_tab_and_bclose_writes_it_back() {
    let s = Sim::new();
    s.doc_mut(|d| d.define_block("DOOR", p(10., 10.), vec![Entity::line(p(10., 10.), p(20., 10.)), attdef("NUM", "Door number", "D1", p(16., 12.))]));
    s.cmd("BEDIT").ty("door");
    assert!(!s.busy());
    assert_eq!(s.h.borrow().editors.len(), 2);
    assert_eq!(s.h.borrow().active, Some(1));
    assert_eq!(s.doc(|d| d.name.clone()), "*BlockEdit:DOOR");
    // the base point is the origin of the block editor, attribute definitions come along
    let ents = s.ents();
    assert_eq!(ents.len(), 2);
    let Kind::Line { a, b } = &ents[0].kind else { panic!() };
    assert_eq!((*a, *b), (p(0., 0.), p(10., 0.)));
    let Kind::Text(t) = &ents[1].kind else { panic!() };
    assert_eq!(t.att.as_ref().unwrap().tag, "NUM");
    assert_eq!(t.p, p(6., 2.));
    assert!(!s.doc(|d| d.dirty));
    // edit the block: add a circle
    s.add(Entity::circle(p(5., 5.), 2.));
    s.cmd("BCLOSE");
    assert!(s.prompt().starts_with("Save changes to DOOR?"), "{}", s.prompt());
    s.enter(); // Yes
    assert!(!s.busy());
    assert_eq!(s.h.borrow().editors.len(), 1);
    assert_eq!(s.h.borrow().active, Some(0));
    let (n, base) = s.doc(|d| (d.blocks["DOOR"].entities.len(), d.blocks["DOOR"].base));
    assert_eq!((n, base), (3, p(10., 10.)));
    let circle = s.doc(|d| d.blocks["DOOR"].entities.iter().find_map(|e| if let Kind::Circle { c, .. } = &e.kind { Some(*c) } else { None }));
    assert_eq!(circle, Some(p(15., 15.))); // back in the block's own coordinates
    let ids: Vec<u64> = s.doc(|d| d.all_entities().map(|e| e.id).collect());
    assert_eq!(ids.len(), ids.iter().collect::<std::collections::HashSet<_>>().len());
    // one undo step restores the block
    s.undo();
    assert_eq!(s.doc(|d| d.blocks["DOOR"].entities.len()), 2);
}

#[test]
fn bclose_without_changes_or_with_no_leaves_the_block_alone() {
    let s = Sim::new();
    door(&s);
    s.cmd("BEDIT").ty("DOOR");
    s.cmd("BCLOSE"); // nothing changed: no question
    assert!(!s.busy());
    assert_eq!(s.h.borrow().editors.len(), 1);
    s.cmd("BEDIT").ty("DOOR");
    s.add(Entity::line(p(0., 0.), p(1., 1.)));
    s.cmd("BCLOSE").ty("n");
    assert_eq!(s.h.borrow().editors.len(), 1);
    assert_eq!(s.doc(|d| d.blocks["DOOR"].entities.len()), 2);
    assert!(s.doc(|d| d.undo_label().is_none()), "no undo step for an unchanged drawing");
}

#[test]
fn bedit_uses_the_selected_reference_reuses_the_open_tab_and_rejects_bad_requests() {
    let s = Sim::new();
    door(&s);
    insert_door(&s, 30., 30., "");
    let id = inserts(&s)[0].id;
    s.set_sel(&[id]);
    s.cmd("BEDIT"); // no prompt: the selected block is opened
    assert!(!s.busy());
    assert_eq!(s.doc(|d| d.name.clone()), "*BlockEdit:DOOR");
    s.cmd("BEDIT");
    assert!(s.history_has("not available in the Block Editor"));
    // switching to the parent and asking again returns to the open tab
    s.h.borrow_mut().set_active(0);
    s.cmd("BEDIT").ty("DOOR");
    assert_eq!(s.h.borrow().editors.len(), 2);
    assert_eq!(s.h.borrow().active, Some(1));
    s.cmd("BCLOSE").enter();
    s.set_sel(&[]);
    s.cmd("BEDIT").ty("nope");
    assert!(s.history_has("Block \"nope\" not found."));
    s.cmd("BCLOSE");
    assert!(s.history_has("works in a Block Editor tab"));
}

#[test]
fn bclose_reports_a_closed_parent_drawing() {
    let s = Sim::new();
    door(&s);
    s.cmd("BEDIT").ty("DOOR");
    s.add(Entity::line(p(0., 0.), p(1., 1.)));
    s.h.borrow_mut().close_editor(0);
    s.cmd("BCLOSE").enter();
    assert!(s.history_has("changes were discarded"), "{}", s.log_text());
    assert!(!s.busy());
}

#[test]
fn base_sets_the_insertion_base_point_and_saves_it() {
    let s = Sim::new();
    s.cmd("BASE");
    assert_eq!(s.prompt(), "Enter base point <0,0>:");
    s.click(12., 7.);
    assert_eq!(s.doc(|d| d.base), p(12., 7.));
    let json = s.doc(|d| d.to_json_string());
    assert_eq!(Drawing::from_json_str(&json, None).unwrap().base, p(12., 7.));
    s.undo();
    assert_eq!(s.doc(|d| d.base), p(0., 0.));
    assert!(!Drawing::new("n").to_json_string().contains("\"base\""), "an origin base point is not written");
    s.cmd("BASE").ty("3,4");
    assert_eq!(s.doc(|d| d.base), p(3., 4.));
}

// ---------------------------------------------------------------- XREF / XATTACH / XBIND

#[test]
fn xattach_attaches_an_open_drawing_as_an_xref_block() {
    let s = with_plan();
    s.cmd("XATTACH");
    assert!(s.prompt().contains("Enter name of drawing to attach"), "{}", s.prompt());
    assert!(s.prompt().contains("<plan>"));
    s.enter();
    assert!(s.prompt().contains("Enter reference type"), "{}", s.prompt());
    s.enter(); // Attach
    assert!(s.prompt().starts_with("Specify insertion point"), "{}", s.prompt());
    s.click(100., 0.);
    assert!(!s.busy());
    let info = xref_info(&s, "plan").expect("xref block");
    assert_eq!((info.path.as_str(), info.overlay, info.loaded), ("plan", false, true));
    let block = s.doc(|d| d.blocks["plan"].clone());
    assert_eq!(block.entities.len(), 1);
    assert_eq!(block.entities[0].layer, "plan|WALLS");
    assert!(s.doc(|d| d.layer("plan|WALLS").is_some()));
    let ins = inserts(&s);
    assert_eq!(ins.len(), 1);
    let Kind::Insert(i) = &ins[0].kind else { panic!() };
    assert_eq!((i.name.as_str(), i.p), ("plan", p(100., 0.)));
    // the reference draws the attached geometry
    let b = s.doc(|d| bounds_of(&ins[0], d)).unwrap();
    near(b.min_x, 100.0);
    near(b.max_x, 110.0);
    // undo removes everything the command did
    s.undo();
    assert!(s.doc(|d| d.blocks.is_empty() && d.layer("plan|WALLS").is_none()));
    assert_eq!(s.n(), 0);
}

#[test]
fn xattach_overlay_uses_the_drawing_base_point_and_cancel_attaches_nothing() {
    let s = with_plan();
    s.h.borrow_mut().set_active(1);
    s.cmd("BASE").click(10., 0.);
    s.h.borrow_mut().set_active(0);
    s.cmd("XATTACH").enter().ty("o").click(0., 0.);
    assert_eq!(xref_info(&s, "plan").map(|x| x.overlay), Some(true));
    assert_eq!(s.doc(|d| d.blocks["plan"].base), p(10., 0.));
    let b = s.doc(|d| bounds_of(&inserts(&s)[0], d)).unwrap();
    near(b.min_x, -10.0); // the source's base point lands on the insertion point
    let t = Sim::new(); // cancelling at the insertion point leaves nothing behind
    t.h.borrow_mut().new_drawing(Some("plan"), false);
    t.h.borrow_mut().set_active(0);
    t.cmd("XATTACH").enter().enter().esc();
    assert!(t.doc(|d| d.blocks.is_empty()));
}

#[test]
fn xref_blocks_are_named_after_the_file_without_directory_and_extension() {
    let s = with_plan();
    s.h.borrow_mut().set_active(1);
    s.doc_mut(|d| d.file_name = Some("C:\\proj\\Site Plan.DXF".into()));
    s.h.borrow_mut().set_active(0);
    s.cmd("XATTACH").ty("site plan").enter().click(0., 0.);
    assert_eq!(xref_info(&s, "Site Plan").map(|x| x.path), Some("C:\\proj\\Site Plan.DXF".to_string()));
}

#[test]
fn xattach_errors() {
    let s = Sim::new();
    s.cmd("XATTACH");
    assert!(s.history_has("No other drawing is open"));
    let s = with_plan();
    s.cmd("XA").ty("?");
    assert!(s.history_has("plan"));
    s.ty("nothing");
    assert!(s.history_has("\"nothing\" is not open"));
    assert!(!s.busy());
    // a block of that name that is not an xref is never overwritten
    s.doc_mut(|d| d.define_block("plan", p(0., 0.), vec![Entity::circle(p(0., 0.), 1.)]));
    s.cmd("XA").ty("PLAN").enter();
    assert!(s.history_has("already exists and is not an external reference"));
    assert_eq!(xref_info(&s, "plan"), None);
}

#[test]
fn xbind_converts_the_xref_to_a_normal_block_and_renames_dependents() {
    let s = with_plan();
    s.cmd("XATTACH").enter().enter().click(0., 0.);
    s.cmd("XBIND").ty("?");
    assert!(s.history_has("plan"));
    s.ty("pl*");
    assert!(!s.busy());
    assert_eq!(xref_info(&s, "plan"), None);
    assert!(s.doc(|d| d.blocks.contains_key("plan")));
    assert_eq!(s.doc(|d| d.blocks["plan"].entities[0].layer.clone()), "plan$0$WALLS");
    assert!(s.doc(|d| d.layer("plan$0$WALLS").is_some() && d.layer("plan|WALLS").is_none()));
    assert_eq!(inserts(&s).len(), 1); // the reference stays
    s.cmd("XBIND").ty("plan");
    assert!(s.history_has("No matching xrefs found."));
}

#[test]
fn nested_blocks_of_the_source_come_along() {
    let s = Sim::new();
    s.h.borrow_mut().new_drawing(Some("site"), false);
    s.doc_mut(|d| d.define_block("TREE", p(0., 0.), vec![Entity::circle(p(0., 0.), 1.)]));
    s.add(Entity::insert("TREE", p(5., 5.)));
    s.h.borrow_mut().set_active(0);
    s.cmd("XATTACH").enter().enter().click(0., 0.);
    assert!(s.doc(|d| d.blocks.contains_key("site|TREE")));
    let Kind::Insert(i) = &s.doc(|d| d.blocks["site"].entities[0].kind.clone()) else { panic!() };
    assert_eq!(i.name, "site|TREE");
    s.cmd("XREF").ty("b").ty("site");
    assert!(s.doc(|d| d.blocks.contains_key("site$0$TREE") && !d.blocks.contains_key("site|TREE")));
    let Kind::Insert(i) = &s.doc(|d| d.blocks["site"].entities[0].kind.clone()) else { panic!() };
    assert_eq!(i.name, "site$0$TREE");
}

#[test]
fn xref_options_list_unload_reload_path_and_detach() {
    let s = with_plan();
    s.cmd("XATTACH").enter().enter().click(0., 0.);
    s.cmd("XREF");
    assert!(s.prompt().contains("Enter an option"), "{}", s.prompt());
    assert!(s.prompt().contains("<Attach>"));
    s.ty("?");
    assert!(!s.busy());
    assert!(s.history_has("Loaded") && s.history_has("Attach"), "{}", s.log_text());

    s.cmd("XREF").ty("u").ty("plan");
    assert_eq!(xref_info(&s, "plan").map(|x| x.loaded), Some(false));
    assert!(s.doc(|d| d.blocks["plan"].entities.is_empty()));
    assert_eq!(inserts(&s).len(), 1);

    // change the source drawing, then reload
    s.h.borrow_mut().set_active(1);
    s.add(Entity::line(p(0., 5.), p(10., 5.)).on_layer("WALLS"));
    s.h.borrow_mut().set_active(0);
    s.cmd("XREF").ty("r").ty("plan");
    assert_eq!(xref_info(&s, "plan").map(|x| x.loaded), Some(true));
    assert_eq!(s.doc(|d| d.blocks["plan"].entities.len()), 2);
    assert_eq!(s.doc(|d| d.layers.iter().filter(|l| l.name == "plan|WALLS").count()), 1);

    // a path that is not open cannot be loaded
    s.cmd("XREF").ty("p").ty("plan").ty("missing.dxf");
    assert_eq!(xref_info(&s, "plan").map(|x| x.path), Some("missing.dxf".to_string()));
    assert!(s.history_has("Cannot reload \"plan\""), "{}", s.log_text());
    s.cmd("XREF").ty("p").ty("plan").ty("plan.dxf");
    assert!(s.history_has("Xref \"plan\" reloaded."));

    s.cmd("XREF").ty("d").ty("plan");
    assert!(s.doc(|d| d.blocks.is_empty() && d.layer("plan|WALLS").is_none()));
    assert!(inserts(&s).is_empty());
    s.undo();
    assert!(xref_info(&s, "plan").is_some());
    assert_eq!(inserts(&s).len(), 1);
}

#[test]
fn xref_attach_option_attaches_without_asking_for_the_type() {
    let s = with_plan();
    s.cmd("XREF").enter(); // Attach is the default
    s.ty("plan");
    assert!(s.prompt().starts_with("Specify insertion point"), "{}", s.prompt());
    s.click(1., 1.);
    assert_eq!(xref_info(&s, "plan").map(|x| x.overlay), Some(false));
    s.cmd("XREF").ty("o").ty("plan").click(30., 0.);
    assert_eq!(inserts(&s).len(), 2, "a second reference to the attached xref");
}

#[test]
fn block_editor_tabs_are_not_offered_as_xref_sources() {
    let s = with_plan();
    door(&s);
    s.cmd("BEDIT").ty("DOOR");
    s.h.borrow_mut().set_active(0);
    s.cmd("XATTACH").ty("?");
    assert!(!s.history_has("BlockEdit"), "{}", s.log_text());
    s.esc();
}
