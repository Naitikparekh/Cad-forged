//! Tests for the ext_system command batch (limits, layer tools, view switches, clipboard variants, reports, files).
use cadforge_core::drawing::{Block, Drawing, Layout, Paper, XrefInfo};
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::Color;
use cadforge_engine::app::Event;
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::{Sim, Step::*};

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6, "{a} != {b}");
}

/// A drawing with a host UI: commands that delegate to it emit events instead of reporting "not available".
fn with_ui() -> Sim {
    let s = Sim::new();
    s.h.borrow_mut().ui.dialogs = true;
    s
}

fn events(s: &Sim) -> Vec<Event> {
    s.h.borrow_mut().take_events()
}

fn ui_actions(s: &Sim) -> Vec<(String, Args)> {
    events(s)
        .into_iter()
        .filter_map(|e| match e {
            Event::UiAction { name, args } => Some((name, args)),
            _ => None,
        })
        .collect()
}

/// The last report line (log lines may hold several) containing `label`.
fn row(s: &Sim, label: &str) -> String {
    let h = s.h.borrow();
    h.history.iter().rev().flat_map(|l| l.text.lines().rev().map(str::to_string).collect::<Vec<_>>()).find(|l| l.contains(label)).unwrap_or_else(|| panic!("no line with {label:?}"))
}

fn layers_on(s: &Sim) -> Vec<(String, bool)> {
    s.doc(|d| d.layers.iter().map(|l| (l.name.clone(), l.on)).collect())
}

/// Layers 0, A and B with one line on each.
fn three_layers() -> Sim {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("A").unwrap();
        d.add_layer("B").unwrap();
        d.add(Entity::line(p(0., 0.), p(10., 0.)));
        d.add(Entity::line(p(0., 5.), p(10., 5.)).on_layer("A"));
        d.add(Entity::line(p(0., 10.), p(10., 10.)).on_layer("B"));
    });
    s
}

fn layer_of_line_at(s: &Sim, y: f64) -> String {
    s.ents().into_iter().find(|e| matches!(e.kind, Kind::Line { a, .. } if a.y == y)).unwrap().layer
}

// ---------------------------------------------------------------- DISTANCE and registry

#[test]
fn distance_is_the_same_command_as_dist() {
    let s = Sim::new();
    let r = s.registry();
    assert_eq!(r.find("DISTANCE").unwrap().name, "DIST");
    assert_eq!(r.find("di").unwrap().name, "DIST");
    s.cmd("DISTANCE").click(0., 0.).click(3., 4.);
    assert!(s.history_has("Distance = 5"), "{}", s.log_text());
}

#[test]
fn new_commands_resolve_with_their_acad_pgp_aliases() {
    let r = Sim::new().registry();
    for (typed, name) in [("HI", "HIDE"), ("PRE", "PREVIEW"), ("AP", "APPLOAD"), ("DV", "DVIEW"), ("SHA", "SHADEMODE"), ("VSM", "VISUALSTYLES"), ("-VP", "VPOINT")] {
        assert_eq!(r.find(typed).map(|d| d.name.clone()).as_deref(), Some(name), "{typed}");
    }
    for name in [
        "LIMITS",
        "LAYERP",
        "LAYMRG",
        "LAYTRANS",
        "LAYWALK",
        "LAYERPALL",
        "LAYVPI",
        "LAYERCLOSE",
        "CELTSCALE",
        "HIDE",
        "SHADEMODE",
        "VISUALSTYLES",
        "CLEANSCREENOFF",
        "VIEWCUBE",
        "NAVVCUBE",
        "NAVSWHEEL",
        "PLAN",
        "VPOINT",
        "DVIEW",
        "TEXTSCR",
        "GRAPHSCR",
        "CLOSEALL",
        "SAVEALL",
        "REINIT",
        "ACTRECORD",
        "APPLOAD",
        "COPYBASE",
        "PASTEBLOCK",
        "PASTEORIG",
        "COPYHIST",
        "STATUS",
        "TIME",
        "DBLIST",
        "AUDIT",
        "RECOVER",
        "PLOTSTYLE",
        "PUBLISH",
        "ETRANSMIT",
        "PREVIEW",
    ] {
        let d = r.find(name).unwrap_or_else(|| panic!("{name} is missing"));
        assert!(!d.title.is_empty() && !d.desc.is_empty() && !d.group.is_empty(), "{name} lacks title, description or group");
    }
}

#[test]
fn wrapping_the_commands_keeps_existing_aliases_and_behaviour() {
    let s = Sim::new();
    let r = s.registry();
    for (typed, name) in [("L", "LINE"), ("Z", "ZOOM"), ("CP", "COPY"), ("CO", "COPY"), ("E", "ERASE"), ("LTS", "LTSCALE")] {
        assert_eq!(r.find(typed).map(|d| d.name.clone()).as_deref(), Some(name), "{typed}");
    }
    assert_eq!(r.alias_of("HIDE").as_deref(), Some("HI"));
    s.play("L", &[Click(0., 0.), Click(5., 0.), Enter]);
    assert_eq!(s.n(), 1);
    s.cmd("U");
    assert_eq!(s.n(), 0);
}

// ---------------------------------------------------------------- LIMITS

#[test]
fn limits_set_the_corners_and_report_the_defaults_in_the_prompts() {
    let s = Sim::new();
    s.cmd("LIMITS");
    assert_eq!(s.prompt(), "Specify lower left corner or [ON/OFF] <0,0>:");
    s.ty("10,20");
    assert_eq!(s.prompt(), "Specify upper right corner <420,297>:");
    s.ty("100,200");
    assert!(!s.busy());
    let l = s.doc(|d| d.settings.limits);
    assert_eq!((l.min_x, l.min_y, l.max_x, l.max_y, l.on), (10., 20., 100., 200., false));
    s.undo();
    assert_eq!(s.doc(|d| d.settings.limits.max_x), 420.0, "one undo step");
}

#[test]
fn limits_enter_keeps_the_current_values_and_on_off_toggle_checking() {
    let s = Sim::new();
    s.cmd("LIMITS").enter().enter();
    assert!(!s.busy());
    assert_eq!(s.doc(|d| d.settings.limits.max_y), 297.0);
    s.cmd("LIMITS").ty("on");
    assert!(!s.busy());
    assert!(s.doc(|d| d.settings.limits.on));
    s.cmd("LIMITS").ty("off");
    assert!(!s.doc(|d| d.settings.limits.on));
    s.cmd("LIMITS").ty("1,1").enter(); // upper right defaults to 420,297
    assert_eq!(s.doc(|d| d.settings.limits.min_x), 1.0);
}

#[test]
fn limits_reject_an_upper_right_corner_below_or_left_of_the_lower_left() {
    let s = Sim::new();
    s.cmd("LIMITS").ty("50,50").ty("10,100");
    assert!(!s.busy());
    assert!(s.history_has("Invalid limits"));
    assert_eq!(s.doc(|d| d.settings.limits.min_x), 0.0);
}

#[test]
fn limits_are_saved_with_the_drawing_and_omitted_while_default() {
    let mut d = Drawing::new("t");
    assert!(!d.to_json_string().contains("limits"));
    d.settings.limits.max_x = 1000.0;
    d.settings.limits.on = true;
    let back = Drawing::from_json_str(&d.to_json_string(), None).unwrap();
    assert_eq!(back.settings.limits, d.settings.limits);
}

// ---------------------------------------------------------------- layers

#[test]
fn layerp_steps_back_through_layer_changes() {
    let s = three_layers();
    s.cmd("-LAYER").ty("OFF").ty("A").enter();
    s.cmd("-LAYER").ty("Freeze").ty("B").enter();
    assert!(s.doc(|d| !d.layer("A").unwrap().on && d.layer("B").unwrap().frozen));
    s.cmd("LAYERP");
    assert!(!s.busy());
    assert!(s.doc(|d| !d.layer("A").unwrap().on && !d.layer("B").unwrap().frozen), "the freeze is undone first");
    s.cmd("LAYERP");
    assert!(s.doc(|d| d.layer("A").unwrap().on));
    s.cmd("LAYERP");
    assert!(s.history_has("No previous layer settings"));
}

#[test]
fn layerp_leaves_new_and_renamed_layers_alone_and_can_itself_be_undone() {
    let s = three_layers();
    s.cmd("-LAYER").ty("OFF").ty("A").enter();
    s.cmd("-LAYER").ty("Color").ty("red").ty("A").ty("New").ty("C").ty("Rename").ty("B").ty("Bee").enter();
    s.cmd("LAYERP");
    assert_eq!(s.doc(|d| d.layer("A").unwrap().color), Color::Aci(7), "A's colour comes back");
    assert!(s.doc(|d| !d.layer("A").unwrap().on));
    assert!(s.doc(|d| d.layer("C").is_some() && d.layer("Bee").is_some() && d.layer("B").is_none()), "new and renamed layers stay");
    s.cmd("U");
    assert_eq!(s.doc(|d| d.layer("A").unwrap().color), Color::Aci(1), "LAYERP is a single undo step");
}

#[test]
fn layerp_notices_layer_changes_made_outside_commands() {
    let s = three_layers();
    s.cmd("REGEN"); // first look at the drawing
    s.doc_mut(|d| d.change(|d| d.layer_mut("A").unwrap().color = Color::Aci(1))); // e.g. the Layer Manager palette
    s.cmd("LAYERP");
    assert_eq!(s.doc(|d| d.layer("A").unwrap().color), Color::Aci(7));
}

#[test]
fn layerp_never_freezes_the_current_layer() {
    let s = three_layers();
    s.doc_mut(|d| d.change(|d| d.layer_mut("B").unwrap().frozen = true));
    s.cmd("REGEN");
    s.doc_mut(|d| {
        d.change(|d| {
            d.layer_mut("B").unwrap().frozen = false;
            d.set_current_layer("B").unwrap();
        })
    });
    s.cmd("LAYERP");
    assert!(s.doc(|d| !d.layer("B").unwrap().frozen));
    assert!(s.history_has("left thawed"));
}

#[test]
fn laymrg_moves_objects_to_the_target_layer_and_deletes_the_source() {
    let s = three_layers();
    s.cmd("LAYMRG");
    assert_eq!(s.prompt(), "Select object on layer to merge or [Name]:");
    s.click(5., 5.); // layer A
    assert_eq!(s.prompt(), "Select object on layer to merge or [Name/Undo]:");
    assert!(s.history_has("Selected layers: A"));
    s.enter();
    assert_eq!(s.prompt(), "Select object on target layer or [Name]:");
    s.click(5., 10.); // layer B
    assert!(s.prompt().contains("Merge layer \"A\" to layer \"B\"? [Yes/No] <No>"), "{}", s.prompt());
    s.ty("y");
    assert!(!s.busy());
    assert_eq!(layer_of_line_at(&s, 5.), "B");
    assert!(s.doc(|d| d.layer("A").is_none() && d.layer("B").is_some()));
    s.undo();
    assert_eq!(layer_of_line_at(&s, 5.), "A", "one undo step");
    assert!(s.doc(|d| d.layer("A").is_some()));
}

#[test]
fn laymrg_by_name_asks_for_confirmation_and_keeps_everything_when_declined() {
    let s = three_layers();
    s.cmd("LAYMRG").ty("name").ty("A,B").enter();
    s.ty("Name").ty("0");
    s.enter(); // default answer: No
    assert!(!s.busy());
    assert_eq!(layer_of_line_at(&s, 5.), "A");
    s.cmd("LAYMRG").ty("name").ty("A,B").enter().ty("name").ty("0").ty("yes");
    assert_eq!((layer_of_line_at(&s, 5.), layer_of_line_at(&s, 10.)), ("0".into(), "0".into()));
    assert_eq!(s.doc(|d| d.layers.len()), 1);
}

#[test]
fn laymrg_refuses_layer_zero_and_moves_the_current_layer_to_the_target() {
    let s = three_layers();
    s.doc_mut(|d| d.set_current_layer("A").unwrap());
    s.cmd("LAYMRG").click(5., 0.); // layer 0 cannot be merged
    assert!(s.history_has("cannot be merged"));
    s.enter(); // nothing selected: the command ends
    assert!(!s.busy());
    s.cmd("LAYMRG").click(5., 5.).ty("undo").click(5., 5.).enter().click(5., 10.).ty("y");
    assert_eq!(s.doc(|d| d.cur.layer.clone()), "B");
}

#[test]
fn laymrg_with_the_target_among_the_sources_has_nothing_to_merge() {
    let s = three_layers();
    s.cmd("LAYMRG").click(5., 5.).enter().click(5., 5.);
    assert!(!s.busy());
    assert!(s.history_has("Nothing to merge"));
}

#[test]
fn laytrans_renames_and_merges_by_name_mapping() {
    let s = three_layers();
    s.cmd("LAYTRANS");
    assert_eq!(s.prompt(), "Enter an option or [Map/List/Remove/Translate] <Translate>:");
    s.ty("map").ty("A").ty("Std");
    s.ty("map").ty("B").ty("Std");
    s.ty("list");
    assert!(s.history_has("A -> Std") && s.history_has("B -> Std"));
    s.enter(); // Translate
    assert!(!s.busy());
    assert!(s.doc(|d| d.layer("Std").is_some() && d.layer("A").is_none() && d.layer("B").is_none()));
    assert_eq!((layer_of_line_at(&s, 5.), layer_of_line_at(&s, 10.)), ("Std".into(), "Std".into()));
    assert!(s.history_has("2 layers translated"));
    s.undo();
    assert!(s.doc(|d| d.layer("A").is_some() && d.layer("B").is_some() && d.layer("Std").is_none()));
}

#[test]
fn laytrans_uses_wildcards_remove_and_rejects_bad_input() {
    let s = three_layers();
    s.cmd("LAYTRANS").ty("map").ty("?").ty("*").ty("Z");
    assert!(s.history_has("Layer \"0\" cannot be translated"));
    s.ty("list");
    assert!(s.history_has("A -> Z") && s.history_has("B -> Z") && !s.history_has("0 -> Z"));
    s.ty("map").ty("zzz");
    assert!(s.history_has("No layer matched \"zzz\""));
    s.enter(); // blank: back to the options
    s.ty("map").ty("A").ty("Bad:Name");
    assert!(s.history_has("Invalid layer name"));
    s.ty("remove").ty("A,B").enter(); // Translate with nothing mapped
    assert!(!s.busy());
    assert!(s.history_has("No layer mappings defined"));
    assert_eq!(s.doc(|d| d.layers.len()), 3);
}

#[test]
fn laywalk_shows_one_layer_at_a_time_and_restores_the_layers_on_exit() {
    let s = three_layers();
    s.doc_mut(|d| d.change(|d| d.layer_mut("A").unwrap().on = false));
    let before = layers_on(&s);
    s.cmd("LAYWALK");
    assert_eq!(layers_on(&s), [("0".into(), true), ("A".into(), false), ("B".into(), false)]);
    assert!(s.history_has("Layer \"0\": 1 object displayed."));
    assert_eq!(s.prompt(), "Enter an option or [Next/Previous/Go/eXit] <Next>:");
    s.enter(); // Next
    assert_eq!(layers_on(&s), [("0".into(), false), ("A".into(), true), ("B".into(), false)]);
    s.ty("previous").ty("previous"); // wraps around to the last layer
    assert_eq!(layers_on(&s), [("0".into(), false), ("A".into(), false), ("B".into(), true)]);
    s.ty("go").ty("0,A");
    assert_eq!(layers_on(&s), [("0".into(), true), ("A".into(), true), ("B".into(), false)]);
    s.ty("x");
    assert!(!s.busy());
    assert_eq!(layers_on(&s), before);
}

#[test]
fn laywalk_restores_the_layers_when_cancelled() {
    let s = three_layers();
    s.cmd("LAYWALK").enter();
    s.esc();
    assert!(!s.busy());
    assert!(layers_on(&s).iter().all(|(_, on)| *on));
}

#[test]
fn laywalk_skips_frozen_layers() {
    let s = three_layers();
    s.doc_mut(|d| d.layer_mut("A").unwrap().frozen = true);
    s.cmd("LAYWALK").enter();
    assert!(s.doc(|d| d.layer("B").unwrap().on && !d.layer("0").unwrap().on), "A is skipped: 0 -> B");
    s.ty("x");
}

#[test]
fn layerpall_and_layvpi_run_and_report_that_viewport_overrides_do_not_exist() {
    let s = Sim::new();
    s.cmd("LAYERPALL");
    assert!(s.history_has("no layout viewports"));
    s.cmd("LAYVPI");
    assert!(s.history_has("available only in a layout viewport"));
    s.doc_mut(|d| {
        let vp = Entity::new(Kind::Viewport(ViewportData::new(p(100., 100.), 50., 50.)));
        d.layouts.push(Layout { name: "Layout1".into(), paper: Paper { w: 420., h: 297., name: "A3".into(), landscape: true }, entities: vec![vp], fresh: false });
        d.space = "Layout1".into();
    });
    s.cmd("LAYERPALL");
    assert!(s.history_has("nothing to reset in 1 viewport"), "{}", s.log_text());
    s.cmd("LAYVPI");
    assert!(s.history_has("use LAYISO"));
}

#[test]
fn layerclose_hides_the_layer_palette() {
    let s = Sim::new();
    events(&s);
    s.cmd("LAYERCLOSE");
    assert!(events(&s).contains(&Event::Palette { name: "layers".into(), show: Some(false) }));
}

// ---------------------------------------------------------------- CELTSCALE

#[test]
fn celtscale_sets_the_scale_of_objects_drawn_afterwards() {
    let s = Sim::new();
    s.play("L", &[Click(0., 0.), Click(5., 0.), Enter]);
    s.cmd("CELTSCALE");
    assert_eq!(s.prompt(), "Enter new value for CELTSCALE <1>:");
    s.ty("0.5");
    assert!(!s.busy());
    assert_eq!(s.doc(|d| d.settings.celtscale), 0.5);
    s.play("C", &[Click(20., 0.), Type("2")]);
    assert_eq!((s.ent(0).lts, s.ent(1).lts), (None, Some(0.5)));
    // copies keep the scale of their source
    s.cmd("CO").click(2., 0.).enter().click(0., 0.).click(0., 5.).enter();
    assert_eq!(s.ent(2).lts, None);
    // the scale is part of the drawing command's own undo step
    s.undo();
    s.undo();
    assert_eq!(s.n(), 1);
}

#[test]
fn celtscale_also_applies_when_the_drawing_command_is_cancelled() {
    let s = Sim::new();
    s.setvar("CELTSCALE", 2.0);
    s.cmd("L").click(0., 0.).click(5., 0.).esc();
    assert!(!s.busy());
    assert_eq!(s.n(), 1);
    assert_eq!(s.ent(0).lts, Some(2.0));
}

#[test]
fn celtscale_one_leaves_new_objects_alone_and_rejects_nonpositive_values() {
    let s = Sim::new();
    s.cmd("CELTSCALE").ty("-2");
    assert!(s.busy(), "stays on the prompt");
    s.ty("1");
    s.play("L", &[Click(0., 0.), Click(5., 0.), Enter]);
    assert_eq!(s.ent(0).lts, None);
}

// ---------------------------------------------------------------- visual styles and view switches

#[test]
fn hide_shademode_and_visualstyles_set_the_visual_style() {
    let s = Sim::new();
    let style = |s: &Sim| s.h.borrow().editor().unwrap().visual_style.clone();
    s.cmd("HIDE");
    assert_eq!(style(&s), "Hidden");
    s.cmd("SHADEMODE");
    assert_eq!(s.prompt(), "Enter option or [2D wireframe/3D wireframe/Hidden/Realistic/Conceptual] <Hidden>:");
    s.ty("3d");
    assert_eq!(style(&s), "Wireframe");
    s.cmd("SHA").ty("realistic");
    assert_eq!(style(&s), "Shaded");
    assert!(s.history_has("Realistic is shown with the Shaded visual style"));
    s.cmd("SHA").ty("c");
    assert_eq!(style(&s), "Shaded with Edges");
    s.cmd("SHA").ty("2d");
    assert_eq!(style(&s), "2D Wireframe");
    s.cmd("VISUALSTYLES").ty("?");
    assert!(s.history_has("2D Wireframe  (current)"));
    s.cmd("VISUALSTYLES").ty("shaded");
    assert_eq!(style(&s), "Shaded");
    s.cmd("VSM").ty("e");
    assert_eq!(style(&s), "Shaded with Edges");
    s.cmd("VISUALSTYLES").enter(); // default: the current style
    assert_eq!(style(&s), "Shaded with Edges");
}

#[test]
fn cleanscreenoff_textscr_and_graphscr_delegate_to_the_host_ui() {
    let s = with_ui();
    events(&s);
    s.cmd("CLEANSCREENOFF").cmd("TEXTSCR").cmd("GRAPHSCR");
    let names: Vec<String> = ui_actions(&s).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["CLEANSCREENOFF", "TEXTSCR", "GRAPHSCR"]);
    let h = Sim::new();
    h.cmd("TEXTSCR");
    assert!(h.history_has("TEXTSCR is not available in this environment"));
}

#[test]
fn viewcube_navvcube_and_navswheel_use_the_display_flags() {
    let s = Sim::new();
    assert_eq!(s.pref("NAVVCUBE"), 1.0);
    s.cmd("VIEWCUBE");
    assert_eq!(s.pref("NAVVCUBE"), 0.0);
    assert!(s.history_has("<ViewCube off>"));
    s.cmd("NAVVCUBE");
    assert_eq!(s.prompt(), "Enter an option or [ON/OFF/Settings] <OFF>:");
    s.ty("on");
    assert_eq!(s.pref("NAVVCUBE"), 1.0);
    s.cmd("NAVVCUBE").ty("off");
    assert_eq!(s.pref("NAVVCUBE"), 0.0);
    s.setvar("NAVBAR", 0.0);
    s.cmd("NAVSWHEEL");
    assert!(s.history_has("SteeringWheels are not available"));
    assert_eq!(s.pref("NAVBAR"), 1.0);
    let u = with_ui();
    events(&u);
    u.cmd("NAVVCUBE").ty("settings");
    assert_eq!(ui_actions(&u)[0].0, "OPTIONS");
}

#[test]
fn plan_returns_to_the_top_view_and_fits_the_drawing() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add(Entity::line(p(0., 0.), p(100., 50.)));
    });
    s.with_view(|v| {
        v.set_preset("swiso");
        v.scale = 0.001;
    });
    s.cmd("PLAN");
    assert_eq!(s.prompt(), "Enter an option or [Current ucs/Ucs/World] <Current ucs>:");
    s.enter();
    assert!(s.with_view(|v| v.is_plan()));
    near(s.with_view(|v| v.target.x), 50.0);
    assert!(s.view_scale() > 1.0, "fitted to the drawing");
    assert!(s.with_view(|v| v.pop_previous()), "the previous view is kept");
    s.cmd("PLAN").ty("ucs").ty("Nope");
    assert!(s.history_has("There is no saved UCS named"));
}

#[test]
fn vpoint_sets_the_viewing_direction_from_a_vector() {
    let s = Sim::new();
    s.cmd("VPOINT");
    assert_eq!(s.prompt(), "Specify a view point or [Rotate] <display compass and tripod>:");
    s.ty("-1,-1,1");
    assert_eq!(s.with_view(|v| v.preset_name()), "SW Isometric");
    s.cmd("VPOINT").ty("0,-1,0");
    assert_eq!(s.with_view(|v| v.preset_name()), "Front");
    s.cmd("-VP").ty("0,0,1");
    assert!(s.with_view(|v| v.is_plan()));
    s.cmd("VPOINT").ty("1,0");
    assert_eq!(s.with_view(|v| v.preset_name()), "Right", "z defaults to 0");
}

#[test]
fn vpoint_rotate_uses_the_two_angles() {
    let s = Sim::new();
    s.cmd("VPOINT").ty("rotate");
    assert!(s.prompt().starts_with("Enter angle in XY plane from X axis"), "{}", s.prompt());
    s.ty("-135");
    assert!(s.prompt().starts_with("Enter angle from XY plane"), "{}", s.prompt());
    s.ty("35.2644");
    assert_eq!(s.with_view(|v| v.preset_name()), "SW Isometric");
    s.cmd("VPOINT").ty("r").ty("-90").ty("0");
    assert_eq!(s.with_view(|v| v.preset_name()), "Front");
}

#[test]
fn vpoint_reports_bad_input_and_the_missing_compass() {
    let s = Sim::new();
    s.cmd("VPOINT").enter();
    assert!(s.history_has("compass and tripod are not available"));
    s.cmd("VPOINT").ty("0,0,0");
    assert!(s.history_has("Invalid view point"));
    s.cmd("VPOINT").ty("abc");
    assert!(s.history_has("Invalid view point"));
    assert!(s.with_view(|v| v.is_plan()), "the view is unchanged");
}

#[test]
fn dview_camera_target_zoom_points_and_undo() {
    let s = Sim::new();
    s.cmd("DVIEW");
    assert!(s.prompt().starts_with("Enter option or [CAmera/TArget/Distance/POints/PAn/Zoom/TWist/CLip/Hide/Off/Undo/eXit] <eXit>"), "{}", s.prompt());
    s.ty("ca").ty("35.2644").ty("-135");
    assert_eq!(s.with_view(|v| v.preset_name()), "SW Isometric");
    s.ty("ta").ty("-35.2644").ty("45"); // looking from the camera toward the target
    assert_eq!(s.with_view(|v| v.preset_name()), "SW Isometric");
    s.ty("ca").ty("0").ty("-90");
    assert_eq!(s.with_view(|v| v.preset_name()), "Front");
    let scale = s.view_scale();
    s.ty("zoom").ty("2");
    near(s.view_scale(), scale * 2.0);
    s.ty("undo");
    near(s.view_scale(), scale);
    s.ty("undo").ty("undo").ty("undo");
    assert_eq!(s.with_view(|v| v.preset_name()), "Top");
    s.ty("undo");
    assert!(s.history_has("Nothing to undo"));
    s.ty("po").ty("10,20,0").ty("10,19,1"); // target, camera: looking from -Y and above
    near(s.with_view(|v| v.target.x), 10.0);
    near(s.with_view(|v| deg(v.pitch)), 45.0);
    s.ty("hide");
    assert_eq!(s.h.borrow().editor().unwrap().visual_style, "Hidden");
    s.enter();
    assert!(!s.busy());
}

#[test]
fn dview_reports_the_options_a_parallel_view_cannot_have() {
    let s = Sim::new();
    s.cmd("DVIEW").ty("distance");
    assert!(s.history_has("Perspective views are not supported"));
    s.ty("off");
    assert!(s.history_has("always a parallel projection"));
    s.ty("tw");
    assert!(s.history_has("twist is not supported"));
    s.ty("cl");
    assert!(s.history_has("Clipping planes are not supported"));
    s.ty("po").ty("1,1,1").ty("1,1,1");
    assert!(s.history_has("Invalid camera or target point"));
    s.esc();
}

// ---------------------------------------------------------------- reports

#[test]
fn status_reports_the_drawing_state() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add(Entity::line(p(0., 0.), p(100., 50.)));
        d.change(|d| d.settings.limits.on = true);
    });
    s.cmd("STATUS");
    let squash = |l: String| l.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(squash(row(&s, "in Drawing1")), "1 object in Drawing1");
    assert_eq!(squash(row(&s, "Model space limits")), "Model space limits are X: 0.0000 Y: 0.0000 (On)");
    assert_eq!(squash(row(&s, "Model space uses")), "Model space uses X: 0.0000 Y: 0.0000");
    assert_eq!(squash(row(&s, "Snap resolution")), "Snap resolution is X: 10.0000 Y: 10.0000");
    assert_eq!(squash(row(&s, "Current space")), "Current space: Model space");
    assert_eq!(squash(row(&s, "Current layer")), "Current layer: \"0\"");
    assert_eq!(squash(row(&s, "Current color")), "Current color: ByLayer");
    assert_eq!(squash(row(&s, "Fill")), "Fill on Grid on Ortho off Snap off Polar off");
    assert_eq!(squash(row(&s, "Object snap modes")), "Object snap modes: Endpoint, Center, Intersection, Extension");
    assert_eq!(squash(row(&s, "Free memory")), "Free memory: not available");
    assert!(s.log_text().contains("X:   420.0000  Y:   297.0000") && s.log_text().contains("X:   100.0000  Y:    50.0000"));
    s.doc_mut(|d| d.change(|d| d.model.entities.clear()));
    s.cmd("STATUS");
    assert!(s.history_has("**Nothing**"));
}

#[test]
fn time_reports_the_session_clock_and_controls_the_timer() {
    let s = Sim::new();
    let at = |ms: f64| s.h.borrow_mut().now = ms;
    let value = |label: &str| row(&s, label).split(':').skip(1).collect::<Vec<_>>().join(":").trim().to_string();
    at(1000.0);
    s.cmd("TIME");
    assert_eq!(s.prompt(), "Enter option or [Display/ON/OFF/Reset]:");
    assert_eq!(value("Current session time"), "0 days 00:00:01.000");
    assert_eq!(value("Opened"), "0 days 00:00:01.000");
    assert_eq!(value("Last updated"), "<no modifications yet>");
    assert_eq!(value("Elapsed timer (on)"), "0 days 00:00:00.000");
    at(66_500.0);
    s.ty("display");
    assert_eq!(value("Time since opened"), "0 days 00:01:05.500");
    assert_eq!(value("Elapsed timer (on)"), "0 days 00:01:05.500");
    s.ty("off");
    assert!(s.history_has("Timer is off."));
    at(100_000.0);
    s.ty("display");
    assert_eq!(value("Elapsed timer (off)"), "0 days 00:01:05.500", "a stopped timer does not advance");
    s.ty("on");
    at(101_000.0);
    s.ty("display");
    assert_eq!(value("Elapsed timer (on)"), "0 days 00:01:06.500");
    s.ty("reset");
    at(102_000.0);
    s.ty("display");
    assert_eq!(value("Elapsed timer (on)"), "0 days 00:00:01.000");
    s.enter();
    assert!(!s.busy());
}

#[test]
fn time_shows_when_the_drawing_was_last_changed() {
    let s = Sim::new();
    s.h.borrow_mut().now = 2000.0;
    s.cmd("REGEN");
    s.add(Entity::line(p(0., 0.), p(1., 1.)));
    s.h.borrow_mut().now = 9000.0;
    s.cmd("TIME");
    assert!(row(&s, "Last updated").ends_with("0 days 00:00:09.000"), "{}", s.log_text());
    s.enter();
}

#[test]
fn dblist_lists_every_object_including_those_on_frozen_layers() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("F").unwrap();
        d.add(Entity::line(p(0., 0.), p(3., 4.)));
        d.add(Entity::circle(p(5., 5.), 2.));
        d.add(Entity::text(p(1., 1.), 2.5, "hello").on_layer("F"));
        d.add(Entity::insert("Gone", p(9., 9.)));
        d.layer_mut("F").unwrap().frozen = true;
        d.layouts.push(Layout { name: "Layout1".into(), paper: Paper { w: 420., h: 297., name: "A3".into(), landscape: true }, entities: vec![Entity::point(p(7., 7.))], fresh: false });
    });
    s.play("BOX", &[Click(50., 0.), Click(60., 10.), Type("5")]);
    s.cmd("DBLIST");
    let log = s.log_text();
    for needle in ["LINE", "length 5", "CIRCLE", "radius 2", "TEXT", "\"hello\"", "Layer: \"F\"", "block \"Gone\"", "Handle = ", "at X=7  Y=7", "3D SOLID"] {
        assert!(log.contains(needle), "missing {needle:?} in\n{log}");
    }
    let t = Sim::new();
    t.cmd("DBLIST");
    assert!(t.history_has("The drawing database is empty."));
}

// ---------------------------------------------------------------- clipboard

#[test]
fn copybase_stores_the_objects_with_the_chosen_base_point() {
    let s = Sim::new();
    s.add(Entity::line(p(10., 10.), p(20., 10.)));
    s.cmd("COPYBASE");
    assert_eq!(s.prompt(), "Specify base point:");
    s.click(15., 10.);
    s.click(12., 10.).enter();
    assert!(s.history_has("1 object copied to the clipboard"));
    let clip = s.h.borrow().clipboard.clone().unwrap();
    assert_eq!(clip.base, p(15., 10.));
    assert_eq!(clip.entities.len(), 1);
    s.cmd("PASTECLIP").click(100., 100.);
    match s.ent(1).kind {
        Kind::Line { a, b } => assert_eq!((a, b), (p(95., 100.), p(105., 100.))),
        k => panic!("{k:?}"),
    }
}

#[test]
fn pasteorig_pastes_at_the_original_coordinates() {
    let s = Sim::new();
    s.add(Entity::line(p(10., 10.), p(20., 10.)));
    s.cmd("PASTEORIG");
    assert!(s.history_has("The clipboard is empty"));
    s.cmd("COPYCLIP").click(15., 10.).enter();
    s.cmd("PASTEORIG");
    assert!(!s.busy(), "no prompt");
    assert_eq!(s.n(), 2);
    assert_eq!(s.ent(0).kind, s.ent(1).kind);
    assert_eq!(s.selected(), vec![s.ent(1).id]);
    s.undo();
    assert_eq!(s.n(), 1);
}

#[test]
fn pasteblock_inserts_the_clipboard_as_a_block_reference() {
    let s = Sim::new();
    s.add(Entity::line(p(10., 10.), p(20., 10.)));
    s.cmd("COPYBASE").click(10., 10.).click(15., 10.).enter();
    s.cmd("PASTEBLOCK");
    assert_eq!(s.prompt(), "Specify insertion point:");
    s.click(100., 0.);
    let name = s.doc(|d| d.blocks.keys().next().cloned().unwrap());
    assert!(name.starts_with("A$C"), "{name}");
    assert_eq!(s.n(), 2);
    match &s.ent(1).kind {
        Kind::Insert(i) => assert_eq!((i.name.as_str(), i.p), (name.as_str(), p(100., 0.))),
        k => panic!("{k:?}"),
    }
    let b = s.doc(|d| d.blocks[&name].clone());
    assert_eq!((b.base, b.entities.len()), (p(10., 10.), 1));
    assert_eq!(s.selected(), vec![s.ent(1).id]);
    s.cmd("COPYBASE").click(10., 10.).click(15., 10.).enter();
    s.cmd("PASTEBLOCK").click(0., 50.);
    assert_eq!(s.doc(|d| d.blocks.len()), 2, "every paste gets its own block");
    s.undo();
    assert_eq!(s.doc(|d| d.blocks.len()), 1, "one undo step");
    let t = Sim::new();
    t.cmd("PASTEBLOCK");
    assert!(t.history_has("The clipboard is empty"));
}

#[test]
fn copyhist_hands_the_history_to_the_host() {
    let s = with_ui();
    s.cmd("DIST").click(0., 0.).click(3., 4.);
    events(&s);
    s.cmd("COPYHIST");
    let acts = ui_actions(&s);
    assert_eq!(acts[0].0, "COPYHIST");
    match acts[0].1 .0.get("lines") {
        Some(ArgVal::Lines(l)) => assert!(l.iter().any(|x| x.contains("Distance = 5"))),
        o => panic!("{o:?}"),
    }
    let h = Sim::new();
    h.cmd("COPYHIST");
    assert!(h.history_has("COPYHIST is not available in this environment"));
}

// ---------------------------------------------------------------- files and output

#[test]
fn audit_reports_dangling_references_and_fixes_them_on_request() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add(Entity::line(p(0., 0.), p(1., 0.)).on_layer("Ghost"));
        d.add(Entity::line(p(0., 1.), p(1., 1.)).on_layer("Ghost"));
        d.add(Entity::insert("NOPE", p(5., 5.)));
        d.add(Entity::line(p(0., 2.), p(1., 2.)));
        d.layer_mut("0").unwrap().ltype = "WEIRD".into();
        d.cur.layer = "Gone".into();
    });
    s.cmd("AUDIT");
    assert_eq!(s.prompt(), "Fix any errors detected? [Yes/No] <No>:");
    s.enter();
    assert!(s.history_has("2 objects on undefined layer \"Ghost\""), "{}", s.log_text());
    assert!(s.history_has("refers to undefined block \"NOPE\""));
    assert!(s.history_has("uses undefined linetype \"WEIRD\""));
    assert!(s.history_has("The current layer is not defined"));
    assert!(s.history_has("4 objects audited"));
    assert!(s.history_has("Total errors found 4 fixed 0"));
    assert_eq!(s.n(), 4, "nothing was changed");
    s.cmd("AUDIT").ty("y");
    assert!(s.history_has("Total errors found 4 fixed 4"));
    assert_eq!(s.n(), 3, "the dangling block reference is erased");
    assert!(s.doc(|d| d.layer("Ghost").is_some() && d.layer("0").unwrap().ltype == "Continuous" && d.cur.layer == "0"));
    s.cmd("AUDIT").ty("y");
    assert!(s.history_has("Total errors found 0 fixed 0"));
    s.undo();
    assert_eq!(s.n(), 4, "the repair is one undo step");
    assert!(s.doc(|d| d.layer("Ghost").is_none()));
}

#[test]
fn recover_repairs_without_asking() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(1., 0.)).on_layer("Ghost"));
    s.cmd("RECOVER");
    assert!(!s.busy());
    assert!(s.history_has("Recovering \"Drawing1\""));
    assert!(s.history_has("Total errors found 1 fixed 1"));
    assert!(s.doc(|d| d.layer("Ghost").is_some()));
}

#[test]
fn closeall_hands_over_to_the_host_and_reports_when_headless() {
    let s = with_ui();
    events(&s);
    s.cmd("CLOSEALL");
    assert_eq!(ui_actions(&s)[0].0, "QUIT");
    let h = Sim::new();
    h.cmd("CLOSEALL");
    assert!(h.history_has("CLOSEALL is not available in this environment"));
}

#[test]
fn saveall_saves_the_active_drawing_and_names_the_others() {
    let s = with_ui();
    s.cmd("SAVEALL");
    assert!(s.history_has("All drawings are saved"));
    s.add(Entity::line(p(0., 0.), p(1., 1.)));
    {
        let mut a = s.h.borrow_mut();
        a.new_drawing(Some("Other"), false);
        a.editor_mut().unwrap().doc.dirty = true;
        a.set_active(0);
    }
    events(&s);
    s.cmd("SAVEALL");
    assert_eq!(ui_actions(&s)[0].0, "QSAVE");
    assert!(s.history_has("Not saved: Other"), "{}", s.log_text());
}

#[test]
fn preview_opens_the_plot_dialog_in_preview_mode() {
    let s = with_ui();
    events(&s);
    s.cmd("PRE");
    let acts = ui_actions(&s);
    assert_eq!(acts[0].0, "PLOT");
    assert_eq!(acts[0].1 .0.get("preview"), Some(&ArgVal::Bool(true)));
}

#[test]
fn plotstyle_reports_what_is_unsupported() {
    let s = with_ui();
    events(&s);
    s.cmd("PLOTSTYLE");
    assert!(s.history_has("Plot styles are not supported"));
    // (PUBLISH is tested in publish.rs)
}

#[test]
fn etransmit_lists_external_references_and_declines_to_package() {
    let s = Sim::new();
    s.cmd("ETRANSMIT");
    assert!(s.history_has("External references:") && s.history_has("none"));
    assert!(s.history_has("Creating a transmittal package is not supported"));
    s.doc_mut(|d| {
        d.blocks.insert("Site".into(), Block { name: "Site".into(), base: p(0., 0.), entities: vec![], xref: Some(XrefInfo { path: "site.dwg".into(), overlay: true, loaded: false }) });
    });
    s.cmd("ETRANSMIT");
    assert!(s.history_has("Site -> site.dwg (overlay)"));
}

#[test]
fn appload_asks_the_host_for_a_lisp_or_script_file() {
    let s = with_ui();
    events(&s);
    s.cmd("AP");
    assert_eq!(ui_actions(&s)[0].0, "APPLOAD");
    let h = Sim::new();
    h.cmd("APPLOAD");
    assert!(h.history_has("APPLOAD needs a file chooser"));
}

#[test]
fn reinit_reports_the_program_parameters() {
    let s = Sim::new();
    s.cmd("REINIT");
    assert!(s.history_has("no digitizer or I/O ports"));
    assert!(s.history_has("Command aliases (the PGP file) are always current"));
}
