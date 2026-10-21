//! Tests for the ext_modify batch: SELECT, FILTER, MULTIPLE, GROUPEDIT, GROUPS, CHPROP, CHANGE, ALIGN, OVERKILL,
//! FLATTEN, REGION (and region UNION/SUBTRACT/INTERSECT), GRADIENT, WIPEOUT, REVCLOUD, MLINE, SKETCH, BLEND,
//! SPLINEDIT, ARRAYPATH.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::polybool::{area_of, Loop};
use cadforge_core::style::Color;
use cadforge_core::text::TextData;
use cadforge_engine::sim::Sim;

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6, "{a} != {b}");
}
fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}
fn add_line(s: &Sim, a: Pt, b: Pt) -> u64 {
    s.add(Entity::line(a, b))
}
fn ab(e: &Entity) -> (Pt, Pt) {
    match e.kind {
        Kind::Line { a, b } => (a, b),
        _ => panic!("not a line: {e:?}"),
    }
}
fn layer(s: &Sim, name: &str) {
    s.doc_mut(|d| {
        d.add_layer(name).unwrap();
    });
}
fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Entity {
    Entity::pline_pts(&[p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)], true)
}
fn regions(s: &Sim) -> Vec<Vec<Loop>> {
    s.ents().iter().filter_map(|e| if let Kind::Region(r) = &e.kind { Some(r.loops.clone()) } else { None }).collect()
}
fn region_area(loops: &[Loop]) -> f64 {
    area_of(&loops.to_vec())
}

// ---------------------------------------------------------------- SELECT

#[test]
fn select_leaves_a_pickfirst_set_and_the_previous_set() {
    let s = Sim::new();
    for y in [0., 5., 10.] {
        add_line(&s, p(0., y), p(10., y));
    }
    s.cmd("SELECT");
    assert_eq!(s.prompt(), "Select objects:");
    s.click(5., 0.).click(5., 5.).enter();
    assert!(!s.busy());
    assert_eq!(s.sel_count(), 2);
    s.cmd("ERASE"); // pick-first: no prompt
    assert!(!s.busy());
    assert_eq!(s.n(), 1);
    s.undo();
    assert_eq!(s.n(), 3);

    s.esc(); // clear the pick-first set; the Previous set survives
    assert_eq!(s.sel_count(), 0);
    s.cmd("ERASE").ty("P").enter();
    assert_eq!(s.n(), 1, "P selects the objects chosen with SELECT");
}

#[test]
fn select_without_any_pick_changes_nothing() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("SELECT").enter();
    assert!(!s.busy());
    assert_eq!(s.sel_count(), 0);
}

// ---------------------------------------------------------------- FILTER

#[test]
fn filter_builds_a_selection_from_criteria_and_remembers_the_list() {
    let s = Sim::new();
    layer(&s, "A");
    add_line(&s, p(0., 0.), p(10., 0.));
    s.add(Entity::circle(p(20., 0.), 3.).with_color(Color::Aci(1)));
    let big = s.add(Entity::circle(p(40., 0.), 5.).on_layer("A"));
    s.add(Entity::text(p(0., 20.), 2.5, "HELLO"));
    s.cmd("FILTER");
    assert_eq!(s.prompt(), "Enter filter option or [ADd to list/Delete/Clear list/List/Apply/eXit] <Apply>:");
    s.enter(); // nothing to apply yet
    assert!(s.history_has("Add at least one entry"));
    s.ty("ad");
    assert_eq!(s.message(), "Enter filter type");
    s.ty("o").enter().ty("circle");
    assert!(s.history_has("Added: Object = circle"));
    s.ty("ad").ty("r").ty(">").ty("4");
    assert!(s.history_has("Added: Radius > 4"));
    s.enter(); // Apply
    assert_eq!(s.message(), "Select objects");
    s.ty("all").enter();
    assert!(!s.busy());
    assert_eq!(s.selected(), vec![big], "only the circle with a radius over 4 passes both entries");

    // the list survives to the next run; layers and colours filter too
    s.esc();
    s.cmd("FILTER").ty("clear").ty("ad").ty("l").enter().ty("A");
    s.enter().ty("all").enter();
    assert_eq!(s.selected(), vec![big]);
    s.esc();
    s.cmd("FILTER").ty("clear").ty("ad").ty("c").enter().ty("red").enter().ty("all").enter();
    assert_eq!(s.sel_count(), 1);
    s.esc();
    s.cmd("FILTER").ty("clear").ty("ad").ty("t").enter().ty("hel*").enter().ty("all").enter();
    assert_eq!(s.sel_count(), 1);
    s.esc();
    s.cmd("FILTER").ty("clear").ty("ad").ty("l").ty("!=").ty("A").enter().ty("all").enter();
    assert_eq!(s.sel_count(), 3, "layer is not A: the line, the red circle and the text");
}

#[test]
fn filter_rejects_bad_values_and_deletes_entries() {
    let s = Sim::new();
    s.cmd("FILTER").ty("clear").ty("ad").ty("o").enter().ty("banana");
    assert!(s.history_has("Unknown object type"));
    s.ty("ad").ty("lw").enter().ty("wide");
    assert!(s.history_has("Requires a numeric value"));
    s.ty("ad").ty("l").enter().ty("X");
    s.ty("delete").ty("1");
    assert!(s.history_has("Deleted: Layer = X"));
    s.ty("list");
    s.ty("x");
    assert!(!s.busy());
}

// ---------------------------------------------------------------- MULTIPLE

#[test]
fn multiple_repeats_a_command_until_cancelled() {
    let s = Sim::new();
    s.cmd("MULTIPLE");
    assert_eq!(s.prompt(), "Enter command name to repeat:");
    s.ty("circle").click(0., 0.).click(5., 0.);
    assert_eq!(s.n(), 1);
    assert!(s.busy(), "CIRCLE starts again");
    s.click(20., 0.).click(23., 0.).click(40., 0.);
    s.esc();
    assert!(!s.busy());
    assert_eq!(s.n(), 2);
    s.undo();
    assert_eq!(s.n(), 0, "the whole session is one undo step");
}

#[test]
fn multiple_refuses_unknown_and_non_interactive_commands() {
    let s = Sim::new();
    s.cmd("MULTIPLE").ty("nosuchcommand");
    assert!(!s.busy());
    assert!(s.history_has("Unknown command"));
    s.cmd("MULTIPLE").ty("multiple");
    assert!(s.history_has("cannot repeat itself"));
    s.cmd("MULTIPLE").ty("oops");
    assert!(!s.busy(), "a command that never waits is not looped forever");
    assert!(s.history_has("does not wait for input"));
}

// ---------------------------------------------------------------- GROUPEDIT / GROUPS

#[test]
fn groupedit_adds_and_removes_members_and_groups_lists_them() {
    let s = Sim::new();
    let a = add_line(&s, p(0., 0.), p(10., 0.));
    let b = add_line(&s, p(0., 5.), p(10., 5.));
    let c = add_line(&s, p(0., 10.), p(10., 10.));
    s.cmd("GROUP").click(5., 0.).click(5., 5.).enter();
    let gid = s.doc(|d| d.by_id(a).unwrap().grp.clone()).expect("grouped");
    assert_eq!(s.doc(|d| d.by_id(b).unwrap().grp.clone()), Some(gid.clone()));
    s.cmd("GROUPS");
    assert!(s.history_has("2 object(s)"));

    s.cmd("GROUPEDIT").click(5., 0.);
    assert_eq!(s.prompt(), "Enter a group option or [Add objects/Remove objects]:");
    s.ty("a").click(5., 10.).enter();
    assert_eq!(s.doc(|d| d.by_id(c).unwrap().grp.clone()), Some(gid.clone()));
    s.ty("r").click(5., 10.).enter();
    assert_eq!(s.doc(|d| d.by_id(c).unwrap().grp.clone()), None);
    assert_eq!(s.doc(|d| d.by_id(a).unwrap().grp.clone()), Some(gid), "the others stay grouped");
    s.enter();
    assert!(!s.busy());
}

#[test]
fn groupedit_finds_a_group_by_name_and_reports_ungrouped_objects() {
    let s = Sim::new();
    let a = add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(0., 5.), p(10., 5.));
    add_line(&s, p(0., 20.), p(10., 20.));
    s.cmd("GROUPEDIT").click(5., 20.);
    assert!(s.history_has("not in a group"));
    s.esc();
    s.cmd("GROUP").click(5., 0.).click(5., 5.).enter();
    s.cmd("GROUPEDIT").ty("name").ty("*A1");
    assert_eq!(s.message(), "Enter a group option");
    s.enter();
    s.cmd("GROUPEDIT").ty("name").ty("zzz");
    assert!(s.history_has("not found"));
    s.esc();
    assert!(s.doc(|d| d.by_id(a).unwrap().grp.is_some()));
}

// ---------------------------------------------------------------- CHPROP / CHANGE

#[test]
fn chprop_changes_color_layer_linetype_lineweight_scale_and_transparency() {
    let s = Sim::new();
    layer(&s, "Walls");
    let a = add_line(&s, p(0., 0.), p(10., 0.));
    let b = add_line(&s, p(0., 5.), p(10., 5.));
    s.cmd("CHPROP").click(5., 0.).click(5., 5.).enter();
    assert_eq!(s.prompt(), "Enter property to change or [Color/LAyer/LType/ltScale/LWeight/Thickness/TRansparency]:");
    s.ty("c");
    assert_eq!(s.prompt(), "Enter new color [Truecolor/COlorbook] <BYLAYER>:");
    s.ty("red").ty("la").ty("Walls").ty("lt").ty("dashed").ty("lw").ty("0.5").ty("s").ty("2").ty("tr").ty("40").enter();
    assert!(!s.busy());
    for id in [a, b] {
        let e = s.doc(|d| d.by_id(id).unwrap().clone());
        assert_eq!(e.color, Some(Color::Aci(1)));
        assert_eq!(e.layer, "Walls");
        assert_eq!(e.ltype.as_deref(), Some("DASHED"));
        assert_eq!(e.lw, Some(50));
        assert_eq!(e.lts, Some(2.0));
        assert_eq!(e.transp, Some(40.0));
    }
    assert_eq!(s.doc(|d| d.undo_depth()), 1, "one undo step");
    s.undo();
    assert_eq!(s.doc(|d| d.by_id(a).unwrap().layer.clone()), "0");
}

#[test]
fn chprop_validates_input_and_can_reset_to_bylayer() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(10., 0.)).with_color(Color::Aci(3)).with_lw(35));
    s.cmd("CHPROP").click(5., 0.).enter();
    s.ty("la").ty("Nowhere");
    assert!(s.history_has("Layer \"Nowhere\" not found"));
    s.ty("lt").ty("nope");
    assert!(s.history_has("Linetype \"nope\" not found"));
    s.ty("lw").ty("0.77");
    assert!(s.history_has("Invalid lineweight"));
    s.ty("tr").ty("95");
    assert!(s.history_has("Transparency must be between 0 and 90"));
    s.ty("t").ty("5");
    assert!(s.history_has("Thickness is not stored"));
    s.ty("c").enter().ty("lw").enter().enter();
    let e = s.doc(|d| d.by_id(a).unwrap().clone());
    assert_eq!(e.color, None);
    assert_eq!(e.lw, None);
}

#[test]
fn chprop_colors_and_layers_solids_too() {
    let s = Sim::new();
    layer(&s, "Walls");
    s.set_visual_style("Shaded");
    s.play("BOX", &[cadforge_engine::sim::Step::Click(0., 0.), cadforge_engine::sim::Step::Click(10., 10.), cadforge_engine::sim::Step::Type("10")]);
    s.cmd("CHPROP").click(5., 5.).enter().ty("c").ty("blue").ty("la").ty("Walls").enter();
    let so = s.solids();
    assert_eq!(so[0].color, Some(Color::Aci(5)));
    assert_eq!(so[0].layer, "Walls");
}

#[test]
fn change_moves_line_ends_resizes_circles_and_edits_text() {
    let s = Sim::new();
    let l = add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("CHANGE").click(5., 0.).enter();
    assert_eq!(s.prompt(), "Specify change point or [Properties]:");
    s.click(14., 3.);
    assert_eq!(ab(&s.doc(|d| d.by_id(l).unwrap().clone())), (p(0., 0.), p(14., 3.)), "the end nearest the change point moves");
    let c1 = s.add(Entity::circle(p(0., 20.), 3.));
    let c2 = s.add(Entity::circle(p(30., 20.), 3.));
    s.cmd("CHANGE").click(3., 20.).click(30., 23.).enter().click(0., 27.);
    assert_eq!(s.prompt(), "Specify new circle radius <no change>:");
    s.ty("4");
    let r = |id: u64| match s.doc(|d| d.by_id(id).unwrap().kind.clone()) {
        Kind::Circle { r, .. } => r,
        k => panic!("{k:?}"),
    };
    near(r(c1), 7.);
    near(r(c2), 4.);
    let t = s.add(Entity::text(p(0., 40.), 2.5, "OLD"));
    s.cmd("CHANGE").click(1., 40.5).enter().click(5., 45.);
    assert_eq!(s.message(), "Specify new height");
    s.ty("4").ty("30").ty("NEW");
    let Kind::Text(td) = s.doc(|d| d.by_id(t).unwrap().kind.clone()) else { panic!() };
    assert_eq!((td.p, td.h, td.s.as_str()), (p(5., 45.), 4., "NEW"));
    near(td.rot, rad(30.));
}

#[test]
fn change_moves_and_turns_block_references_with_their_attributes() {
    let s = Sim::new();
    s.doc_mut(|d| d.define_block("B", p(0., 0.), vec![Entity::circle(p(0., 0.), 5.)]));
    let mut ins = Entity::insert("B", p(0., 0.));
    if let Kind::Insert(i) = &mut ins.kind {
        i.attribs.push(AttribData { tag: "T".into(), t: TextData::new(p(6., 0.), 2., "X"), hidden: false });
    }
    let id = s.add(ins);
    s.cmd("CHANGE").click(5., 0.).enter().click(10., 0.);
    assert_eq!(s.message(), "Enter new block rotation angle");
    s.ty("90");
    let Kind::Insert(i) = s.doc(|d| d.by_id(id).unwrap().kind.clone()) else { panic!() };
    assert_eq!(i.p, p(10., 0.));
    near(i.rot, rad(90.));
    let a = &i.attribs[0].t;
    near(a.p.x, 10.);
    near(a.p.y, 6.);
    near(a.rot, rad(90.));
}

#[test]
fn change_properties_option_and_unsupported_objects() {
    let s = Sim::new();
    let a = add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("CHANGE").click(5., 0.).enter().ty("p").ty("c").ty("3").enter();
    assert_eq!(s.doc(|d| d.by_id(a).unwrap().color), Some(Color::Aci(3)));
    let pl = s.add(square(20., 0., 30., 10.));
    s.cmd("CHANGE").click(25., 0.).enter().click(40., 40.);
    assert!(s.history_has("CHANGE does not move the change point of a polyline"));
    assert!(s.doc(|d| d.by_id(pl).is_some()));
}

// ---------------------------------------------------------------- ALIGN

#[test]
fn align_moves_and_rotates_between_point_pairs_and_can_scale() {
    let s = Sim::new();
    let l = add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("ALIGN").click(5., 0.).enter();
    s.click(0., 0.).click(5., 5.).click(10., 0.).click(5., 15.).enter();
    assert_eq!(s.prompt(), "Scale objects based on alignment points? or [Yes/No] <No>:");
    s.enter();
    assert!(!s.busy());
    let (a, b) = ab(&s.doc(|d| d.by_id(l).unwrap().clone()));
    near(a.x, 5.);
    near(a.y, 5.);
    near(b.x, 5.);
    near(b.y, 15.);
    s.undo();
    s.cmd("ALIGN").click(5., 0.).enter().click(0., 0.).click(5., 5.).click(10., 0.).click(5., 25.).enter().ty("y");
    let (a, b) = ab(&s.doc(|d| d.by_id(l).unwrap().clone()));
    near(a.y, 5.);
    near(b.y, 25.);
}

#[test]
fn align_previews_the_move_and_the_rotation() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("ALIGN").click(5., 0.).enter().click(0., 0.).mv(3., 4.);
    let pv = s.preview();
    assert_eq!(pv.len(), 1);
    match &pv[0] {
        cadforge_engine::runtime::PreviewItem::Entity(e) => assert_eq!(ab(e), (p(3., 4.), p(13., 4.))),
        o => panic!("{o:?}"),
    }
    s.click(3., 4.).click(10., 0.).mv(3., 14.);
    match &s.preview()[0] {
        cadforge_engine::runtime::PreviewItem::Entity(e) => {
            let (a, b) = ab(e);
            near(a.x, 3.);
            near(b.x, 3.);
            near(b.y, 14.);
        }
        o => panic!("{o:?}"),
    }
    s.esc();
    assert_eq!(s.ent(0).kind, Kind::Line { a: p(0., 0.), b: p(10., 0.) }, "Escape leaves the object where it was");
}

#[test]
fn align_with_one_pair_moves_and_three_points_are_reported() {
    let s = Sim::new();
    let l = add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("ALIGN").click(5., 0.).enter().click(0., 0.).click(3., 4.).enter();
    assert!(!s.busy());
    let (a, b) = ab(&s.doc(|d| d.by_id(l).unwrap().clone()));
    assert_eq!((a, b), (p(3., 4.), p(13., 4.)));
    s.cmd("ALIGN").click(8., 4.).enter().click(3., 4.).click(0., 0.).click(13., 4.).click(10., 0.).click(0., 5.);
    assert!(s.history_has("not supported for 2D"));
    s.enter();
    assert!(!s.busy());
    assert_eq!(ab(&s.doc(|d| d.by_id(l).unwrap().clone())), (p(0., 0.), p(10., 0.)));
}

// ---------------------------------------------------------------- OVERKILL

fn overkill(s: &Sim) {
    s.cmd("OVERKILL").ty("all").enter();
    s.enter(); // Proceed
}

#[test]
fn overkill_removes_duplicates_but_honours_layer_and_colour() {
    let s = Sim::new();
    layer(&s, "A");
    add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(10., 0.), p(0., 0.)); // same line, other direction
    s.add(Entity::line(p(0., 0.), p(10., 0.)).on_layer("A")); // other layer: kept
    s.add(Entity::line(p(0., 0.), p(10., 0.)).with_color(Color::Aci(1))); // other colour: kept
    s.add(Entity::circle(p(0., 20.), 3.));
    s.add(Entity::circle(p(0., 20.), 3.));
    s.add(square(30., 0., 40., 10.));
    s.add(Entity::pline_pts(&[p(40., 10.), p(30., 10.), p(30., 0.), p(40., 0.)], true)); // same polygon, other start/direction
    overkill(&s);
    assert_eq!(s.n(), 5);
    assert!(s.history_has("3 duplicate object(s) deleted"));
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn overkill_ignore_properties_makes_layers_match() {
    let s = Sim::new();
    layer(&s, "A");
    add_line(&s, p(0., 0.), p(10., 0.));
    s.add(Entity::line(p(0., 0.), p(10., 0.)).on_layer("A"));
    s.cmd("OVERKILL").ty("all").enter().ty("i").ty("l").enter().enter();
    assert!(s.history_has("Layer: ignored"));
    assert_eq!(s.n(), 1);
}

#[test]
fn overkill_combines_overlapping_and_touching_collinear_lines_and_arcs() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(6., 0.), p(20., 0.)); // partial overlap
    add_line(&s, p(20., 0.), p(30., 0.)); // end to end
    add_line(&s, p(2., 0.), p(4., 0.)); // fully covered
    add_line(&s, p(0., 5.), p(10., 6.)); // not collinear
    s.add(Entity::arc(p(0., 50.), 5., 0., rad(90.)));
    s.add(Entity::arc(p(0., 50.), 5., rad(60.), rad(150.)));
    overkill(&s);
    assert_eq!(s.n(), 3);
    let lines: Vec<(Pt, Pt)> = s.ents().iter().filter(|e| matches!(e.kind, Kind::Line { .. })).map(ab).collect();
    assert!(lines.contains(&(p(0., 0.), p(30., 0.))));
    let arcs: Vec<(f64, f64)> = s.ents().iter().filter_map(|e| if let Kind::Arc { a0, a1, .. } = e.kind { Some((a0, a1)) } else { None }).collect();
    assert_eq!(arcs.len(), 1);
    near(arcs[0].0, 0.);
    near(arcs[0].1, rad(150.));
    assert!(s.history_has("overlapping object(s) combined or deleted"));
}

#[test]
fn overkill_options_switch_the_combining_rules() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(10., 0.), p(20., 0.));
    s.cmd("OVERKILL").ty("all").enter().ty("e");
    assert!(s.history_has("end to end: No"));
    s.enter();
    assert_eq!(s.n(), 2, "touching lines stay apart when end-to-end combining is off");
    s.cmd("OVERKILL").ty("all").enter().ty("e").ty("e").enter();
    assert_eq!(s.n(), 1);
    s.cmd("OVERKILL").ty("all").enter().enter();
    assert!(s.history_has("No duplicate or overlapping objects found."));
}

#[test]
fn overkill_merges_arcs_that_cover_the_whole_circle_into_a_circle() {
    let s = Sim::new();
    s.add(Entity::arc(p(0., 0.), 5., 0., rad(200.)));
    s.add(Entity::arc(p(0., 0.), 5., rad(180.), rad(20.)));
    s.add(Entity::arc(p(0., 0.), 6., 0., rad(90.))); // another circle: untouched
    overkill(&s);
    assert_eq!(s.n(), 2);
    assert!(s.ents().iter().any(|e| matches!(e.kind, Kind::Circle { r, .. } if (r - 5.).abs() < 1e-9)));
}

#[test]
fn overkill_tolerance_decides_what_counts_as_the_same() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(0., 0.01), p(10., 0.01));
    overkill(&s);
    assert_eq!(s.n(), 2);
    s.cmd("OVERKILL").ty("all").enter().ty("t").ty("0.05").enter();
    assert_eq!(s.n(), 1);
}

// ---------------------------------------------------------------- FLATTEN

#[test]
fn flatten_reports_that_2d_data_is_already_flat() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("FLATTEN").click(5., 0.).enter();
    assert!(!s.busy());
    assert!(s.history_has("already lie in the XY plane"));
    assert_eq!(s.n(), 1);
}

// ---------------------------------------------------------------- REGION and region booleans

#[test]
fn region_converts_closed_loops_and_joined_chains() {
    let s = Sim::new();
    s.add(square(0., 0., 10., 10.));
    s.add(Entity::circle(p(30., 5.), 4.));
    add_line(&s, p(50., 0.), p(60., 0.));
    add_line(&s, p(60., 0.), p(60., 10.));
    add_line(&s, p(60., 10.), p(50., 0.)); // a triangle from three lines
    add_line(&s, p(100., 0.), p(110., 0.)); // an open stray
    s.cmd("REGION").ty("all").enter();
    assert!(!s.busy());
    let rs = regions(&s);
    assert_eq!(rs.len(), 3);
    near(region_area(&rs[0]), 100.);
    near(region_area(&rs[1]), std::f64::consts::PI * 16.);
    near(region_area(&rs[2]).abs(), 50.);
    assert_eq!(s.n(), 4, "three regions plus the stray line: the sources are deleted");
    assert!(s.history_has("3 loops extracted"));
    assert!(s.history_has("3 Regions created"));
    assert!(s.history_has("1 object not part of a closed"));
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn region_ignores_self_intersecting_polylines_and_keeps_properties() {
    let s = Sim::new();
    layer(&s, "A");
    s.add(Entity::pline_pts(&[p(0., 0.), p(10., 10.), p(10., 0.), p(0., 10.)], true)); // bow tie
    s.add(square(20., 0., 30., 10.).on_layer("A").with_color(Color::Aci(2)));
    s.cmd("REGION").ty("all").enter();
    let rs: Vec<Entity> = s.ents().into_iter().filter(|e| matches!(e.kind, Kind::Region(_))).collect();
    assert_eq!(rs.len(), 1);
    assert_eq!((rs[0].layer.as_str(), rs[0].color), ("A", Some(Color::Aci(2))));
    assert!(s.history_has("not part of a closed, non-self-intersecting loop"));
    s.cmd("REGION").ty("all").enter();
    assert!(s.history_has("No closed loops were found."));
}

#[test]
fn union_subtract_and_intersect_work_on_regions() {
    let s = Sim::new();
    s.add(square(0., 0., 10., 10.));
    s.add(square(5., 5., 15., 15.));
    s.cmd("REGION").ty("all").enter();
    assert_eq!(regions(&s).len(), 2);
    s.cmd("UNION");
    assert!(s.message().contains("regions"));
    s.ty("all").enter();
    let rs = regions(&s);
    assert_eq!(rs.len(), 1);
    near(region_area(&rs[0]), 175.);
    s.undo();
    assert_eq!(regions(&s).len(), 2);

    s.cmd("INTERSECT").ty("all").enter();
    let rs = regions(&s);
    assert_eq!(rs.len(), 1);
    near(region_area(&rs[0]), 25.);
    s.undo();

    // pick the first region, then the one to subtract
    s.cmd("SUBTRACT").click(0., 5.).enter().click(15., 10.).enter();
    let rs = regions(&s);
    assert_eq!(rs.len(), 1);
    near(region_area(&rs[0]), 75.);
    assert_eq!(s.n(), 1);
}

#[test]
fn subtracting_a_circle_from_a_region_keeps_true_arcs_and_holes() {
    let s = Sim::new();
    s.add(square(0., 0., 20., 20.));
    s.add(Entity::circle(p(10., 10.), 4.));
    s.cmd("REGION").ty("all").enter();
    s.cmd("SUBTRACT").click(0., 10.).enter().click(14., 10.).enter();
    let rs = regions(&s);
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0].len(), 2, "outer boundary plus a hole");
    near(region_area(&rs[0]), 400. - std::f64::consts::PI * 16.);
    assert!(rs[0][1].iter().any(|v| v.b != 0.), "the hole is a true arc");
}

#[test]
fn region_booleans_report_empty_results_and_bad_input() {
    let s = Sim::new();
    s.add(square(0., 0., 10., 10.));
    s.add(square(20., 0., 30., 10.));
    s.cmd("REGION").ty("all").enter();
    s.cmd("INTERSECT").ty("all").enter();
    assert!(s.history_has("The result is empty; no region remains."));
    assert!(regions(&s).is_empty());
    s.undo();
    s.cmd("UNION").click(5., 0.).enter();
    assert!(s.history_has("At least two regions must be selected."));
    assert_eq!(regions(&s).len(), 2);
    s.cmd("SUBTRACT").click(5., 0.).enter().click(5., 0.).enter();
    assert!(s.history_has("Nothing to subtract."));
}

#[test]
fn stock_solid_booleans_still_work_when_regions_exist() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.add(square(100., 100., 110., 110.));
    s.cmd("REGION").ty("all").enter();
    use cadforge_engine::sim::Step::*;
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.play("BOX", &[Click(5., 0.), Click(15., 10.), Type("10")]);
    s.cmd("UNION").click(2., 5.).click(13., 5.).enter();
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 1);
    s.undo();
    s.cmd("SUBTRACT").click(2., 5.).enter().click(13., 5.).enter();
    assert_eq!(s.solids().len(), 1);
    s.undo();
    s.cmd("INTERSECT").click(2., 5.).click(13., 5.).enter();
    assert_eq!(s.solids().len(), 1);
    assert_eq!(regions(&s).len(), 1, "the region is untouched");
    s.cmd("UNION").ty("all").enter();
    assert!(s.history_has("Regions and solids cannot be combined"));
}

#[test]
fn the_aliases_of_the_boolean_commands_survive() {
    let s = Sim::new();
    let reg = s.registry();
    assert_eq!(reg.find("UNI").unwrap().name, "UNION");
    assert_eq!(reg.find("SU").unwrap().name, "SUBTRACT");
    assert_eq!(reg.find("IN").unwrap().name, "INTERSECT");
    assert_eq!(reg.find("UNION").unwrap().group, "3D");
}

// ---------------------------------------------------------------- GRADIENT

fn gradient_of(e: &Entity) -> GradientData {
    match &e.kind {
        Kind::Hatch(h) => h.gradient.clone().expect("a gradient"),
        k => panic!("{k:?}"),
    }
}

#[test]
fn gradient_fills_a_picked_area_with_the_default_two_color_fill() {
    let s = Sim::new();
    s.add(square(0., 0., 10., 10.));
    s.cmd("GRADIENT");
    assert_eq!(s.prompt(), "Pick internal point or [Select objects/seTtings/Undo]:");
    s.click(5., 5.).enter();
    assert!(!s.busy());
    let h = s.ents().into_iter().find(|e| matches!(e.kind, Kind::Hatch(_))).expect("hatch");
    let g = gradient_of(&h);
    assert_eq!((g.kind.as_str(), g.c1, g.c2, g.one_color), ("LINEAR", [0, 0, 255], [255, 255, 255], false));
    let Kind::Hatch(hd) = &h.kind else { panic!() };
    assert_eq!(hd.pattern, "SOLID");
    assert_eq!(hd.loops.len(), 1);
    assert!(s.history_has("Gradient created."));
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn gradient_settings_pattern_colors_angle_tint_and_one_color() {
    let s = Sim::new();
    s.add(square(0., 0., 10., 10.));
    s.cmd("GRADIENT").ty("t");
    assert_eq!(s.message(), "Enter a gradient option");
    s.ty("p").ty("sph").ty("c1").ty("red").ty("c2").ty("0,255,0").ty("a").ty("45").enter();
    s.click(5., 5.).enter();
    let g = gradient_of(&s.ents().into_iter().find(|e| matches!(e.kind, Kind::Hatch(_))).unwrap());
    assert_eq!((g.kind.as_str(), g.c1, g.c2), ("SPHERICAL", [255, 0, 0], [0, 255, 0]));
    near(g.angle, rad(45.));
    // settings stick for the next run; inverted spherical swaps the two colours; one color derives c2 from the tint
    s.cmd("GRADIENT").ty("t").ty("p").ty("invs").enter();
    s.click(5., 5.).enter();
    let g = gradient_of(&s.ents().into_iter().rev().find(|e| matches!(e.kind, Kind::Hatch(_))).unwrap());
    assert_eq!((g.kind.as_str(), g.c1, g.c2), ("SPHERICAL", [0, 255, 0], [255, 0, 0]));
    s.cmd("GRADIENT").ty("t").ty("p").ty("l").ty("o").ty("one").ty("tint").ty("0.25").enter();
    s.click(5., 5.).enter();
    let g = gradient_of(&s.ents().into_iter().rev().find(|e| matches!(e.kind, Kind::Hatch(_))).unwrap());
    assert!(g.one_color);
    near(g.tint, 0.25);
    assert_eq!(g.c1, [255, 0, 0]);
}

#[test]
fn gradient_rejects_bylayer_colors_and_out_of_range_tints() {
    let s = Sim::new();
    s.cmd("GRADIENT").ty("t").ty("c1").ty("bylayer");
    assert!(s.history_has("needs a specific color"));
    s.ty("tint").ty("2");
    assert!(s.history_has("The tint must be between 0 and 1."));
    s.esc();
}

#[test]
fn gradient_select_objects_undo_and_no_boundary() {
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 5.));
    s.cmd("GRADIENT").click(50., 50.);
    assert!(s.history_has("Unable to determine a valid closed boundary") || s.history_has("No closed boundary"));
    s.ty("s").click(5., 0.).enter();
    s.ty("u");
    s.enter();
    assert_eq!(s.ents().iter().filter(|e| matches!(e.kind, Kind::Hatch(_))).count(), 0, "undo cleared the pending loops");
    s.cmd("GRADIENT").ty("s").click(5., 0.).enter().enter();
    assert_eq!(s.ents().iter().filter(|e| matches!(e.kind, Kind::Hatch(_))).count(), 1);
}

// ---------------------------------------------------------------- WIPEOUT

#[test]
fn wipeout_from_points() {
    let s = Sim::new();
    s.cmd("WIPEOUT");
    assert_eq!(s.prompt(), "Specify first point or [Frames/Polyline] <Polyline>:");
    s.click(0., 0.);
    assert_eq!(s.prompt(), "Specify next point or [Undo]:");
    s.click(10., 0.).click(10., 10.);
    assert_eq!(s.prompt(), "Specify next point or [Close/Undo]:");
    s.click(0., 10.).ty("u").click(5., 12.).enter();
    assert!(!s.busy());
    let Kind::Wipeout(w) = s.ent(0).kind else { panic!() };
    assert_eq!(w.pts, vec![p(0., 0.), p(10., 0.), p(10., 10.), p(5., 12.)]);
}

#[test]
fn wipeout_from_a_polyline_can_erase_it_and_needs_three_points() {
    let s = Sim::new();
    s.add(square(0., 0., 10., 10.));
    s.cmd("WIPEOUT").enter().click(5., 0.).ty("n");
    assert_eq!(s.n(), 2);
    assert!(matches!(s.ent(1).kind, Kind::Wipeout(ref w) if w.pts.len() == 4));
    s.cmd("WIPEOUT").enter().click(5., 0.).ty("y");
    assert_eq!(s.n(), 2, "the wipeout replaced the polyline");
    s.cmd("WIPEOUT").click(0., 0.).click(5., 5.).enter();
    assert!(s.history_has("at least three points"));
    s.cmd("WIPEOUT").ty("f").ty("off");
    assert!(s.history_has("Hiding wipeout frames is not supported"));
    s.esc();
}

#[test]
fn wipeout_from_a_polyline_with_arcs_is_tessellated() {
    let s = Sim::new();
    s.add(Entity::pline(vec![Vertex::new(0., 0., 1.), Vertex::new(10., 0., 1.)], true));
    s.cmd("WIPEOUT").enter().click(5., 5.).ty("n");
    let Kind::Wipeout(w) = s.ent(1).kind else { panic!() };
    assert!(w.pts.len() > 8, "arcs become many points, got {}", w.pts.len());
}

// ---------------------------------------------------------------- REVCLOUD

fn plines(s: &Sim) -> Vec<(Vec<Vertex>, bool)> {
    s.ents().iter().filter_map(|e| if let Kind::Pline { v, closed } = &e.kind { Some((v.clone(), *closed)) } else { None }).collect()
}
fn set_arc_length(s: &Sim, len: &str) {
    s.ty("a").ty(len).enter();
}

#[test]
fn revcloud_rectangular_makes_a_closed_cloud_bulging_outwards() {
    let s = Sim::new();
    s.cmd("REVCLOUD");
    assert_eq!(s.prompt(), "Specify start point or [Arc length/Object/Rectangular/Polygonal/Freehand/Style/Modify] <Object>:");
    set_arc_length(&s, "2");
    s.ty("r");
    assert_eq!(s.message(), "Specify first corner point");
    s.click(0., 0.).click(10., 6.);
    assert!(!s.busy());
    let pl = plines(&s);
    assert_eq!(pl.len(), 1);
    let (v, closed) = &pl[0];
    assert!(*closed);
    assert_eq!(v.len(), 16, "5 + 3 + 5 + 3 arcs of length 2");
    let sweep = |b: f64| deg(4.0 * b.atan());
    assert!(v.iter().all(|x| (sweep(x.b) - 120.).abs() < 1e-6));
    // the first arc runs along the bottom edge and bulges below it
    let first = bulge_seg(v[0].pt(), v[1].pt(), v[0].b);
    assert!(first.at(0.5).y < -0.1, "outwards, got {:?}", first.at(0.5));
    assert!(s.history_has("Revision cloud finished."));
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn revcloud_is_outward_whichever_way_the_corners_are_picked() {
    let s = Sim::new();
    s.cmd("REVCLOUD");
    set_arc_length(&s, "2");
    s.ty("r").click(10., 6.).click(0., 0.);
    let (v, _) = plines(&s)[0].clone();
    // every arc midpoint lies outside the rectangle
    for i in 0..v.len() {
        let m = bulge_seg(v[i].pt(), v[(i + 1) % v.len()].pt(), v[i].b).at(0.5);
        let inside = m.x > 1e-6 && m.x < 10. - 1e-6 && m.y > 1e-6 && m.y < 6. - 1e-6;
        assert!(!inside, "arc {i} midpoint {m:?} is inside");
    }
}

#[test]
fn revcloud_polygonal_and_freehand() {
    let s = Sim::new();
    s.cmd("REVCLOUD");
    set_arc_length(&s, "2");
    s.ty("p").click(0., 0.).click(10., 0.);
    assert_eq!(s.prompt(), "Specify next point or [Undo]:");
    s.click(5., 8.).enter();
    let (v, closed) = plines(&s)[0].clone();
    assert!(closed);
    assert_eq!(v.len(), 5 + 5 + 5, "10 + 9.43 + 9.43 long edges in arcs of 2");
    s.cmd("REVCLOUD").ty("p").click(0., 0.).click(5., 0.).enter();
    assert!(s.history_has("needs at least three points"));

    // freehand: a start point begins it; returning to the start closes it
    s.cmd("REVCLOUD").click(0., 30.).click(10., 30.).click(10., 40.).click(0., 40.).click(0.5, 30.5).enter();
    let (v, closed) = plines(&s)[1].clone();
    assert!(closed);
    assert_eq!(v.len(), 20, "four 10-unit edges in arcs of 2, the closing point is dropped");
    // not back at the start: the cloud stays open and its last vertex carries no bulge
    s.cmd("REVCLOUD").click(0., 60.).click(10., 60.).click(10., 70.).enter();
    let (v, closed) = plines(&s)[2].clone();
    assert!(!closed);
    assert_eq!(v.len(), 11);
    assert_eq!(v[v.len() - 1].b, 0.0);
}

#[test]
fn revcloud_converts_objects_and_can_reverse_them() {
    let s = Sim::new();
    layer(&s, "A");
    s.cmd("REVCLOUD");
    set_arc_length(&s, "2");
    let circle = s.add(Entity::circle(p(0., 0.), 5.).on_layer("A"));
    s.enter().click(5., 0.);
    assert_eq!(s.message(), "Reverse direction?");
    s.enter();
    assert!(!s.busy());
    assert!(s.doc(|d| d.by_id(circle).is_none()), "the source object is replaced");
    let e = s.ent(0);
    assert_eq!(e.layer, "A");
    let Kind::Pline { v, closed } = &e.kind else { panic!() };
    assert!(*closed);
    assert_eq!(v.len(), 16, "a circle of length 31.4 in arcs of 2");
    assert!(v.iter().all(|x| x.b > 0.), "outwards for a counter-clockwise path");

    let s2 = Sim::new();
    s2.add(Entity::circle(p(0., 0.), 5.));
    s2.cmd("REVCLOUD").enter().click(5., 0.).ty("y");
    let Kind::Pline { v, .. } = &s2.ent(0).kind else { panic!() };
    assert!(v.iter().all(|x| x.b < 0.), "Reverse direction flips the bulges");
}

#[test]
fn revcloud_follows_ellipses_and_splines_with_evenly_spaced_arcs() {
    let s = Sim::new();
    s.add(Entity::ellipse(p(0., 0.), p(10., 0.), 0.5, 0., TAU));
    s.cmd("REVCLOUD");
    set_arc_length(&s, "2");
    s.enter().click(10., 0.).enter();
    let (v, closed) = plines(&s)[0].clone();
    assert!(closed);
    // ellipse perimeter is about 48.4, so about 24 arcs, not one per tessellation step
    assert!((22..=26).contains(&v.len()), "{} arcs", v.len());
}

#[test]
fn revcloud_modify_reverses_and_style_and_arc_length_are_checked() {
    let s = Sim::new();
    s.cmd("REVCLOUD");
    set_arc_length(&s, "2");
    s.ty("r").click(0., 0.).click(10., 6.);
    let before = plines(&s)[0].0.clone();
    s.cmd("REVCLOUD").ty("m").click(1., -0.577);
    assert_eq!(s.message(), "Reverse direction?");
    s.ty("y");
    let after = plines(&s)[0].0.clone();
    assert!(before.iter().zip(&after).all(|(a, b)| a.b == -b.b && a.x == b.x));
    s.cmd("REVCLOUD").ty("s").ty("c");
    assert!(s.history_has("Calligraphy needs polyline widths"));
    s.ty("a").ty("1").ty("9");
    assert!(s.history_has("cannot exceed three times the minimum"));
    s.cmd("REVCLOUD");
    assert!(s.history_has("Minimum arc length: 1  Maximum arc length: 3"));
    s.esc();
}

// ---------------------------------------------------------------- MLINE

#[test]
fn mline_draws_two_parallel_polylines_as_one_group() {
    let s = Sim::new();
    s.cmd("MLINE");
    assert_eq!(s.prompt(), "Specify start point or [Justification/Scale/STyle]:");
    assert!(s.history_has("Current settings: Justification = Top, Scale = 20.00, Style = STANDARD"));
    s.click(0., 0.);
    assert_eq!(s.prompt(), "Specify next point or [Undo]:");
    s.click(100., 0.).click(100., 100.);
    assert_eq!(s.prompt(), "Specify next point or [Close/Undo]:");
    s.enter();
    assert!(!s.busy());
    let pl = plines(&s);
    assert_eq!(pl.len(), 2);
    let ys: Vec<f64> = pl.iter().map(|(v, _)| v[0].y).collect();
    assert_eq!(ys, vec![0., -20.], "Top justification: the picked line is the top element");
    assert_eq!(pl[0].0.len(), 3);
    let grp: Vec<_> = s.ents().iter().map(|e| e.grp.clone()).collect();
    assert!(grp[0].is_some() && grp[0] == grp[1]);
    // the path turns left, so the right-hand element is on the outside and its corner is mitred outwards
    assert_eq!((pl[1].0[1].x, pl[1].0[1].y), (120., -20.));
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
    // picking one selects both
    s.cmd("ERASE").click(50., 0.).enter();
    assert_eq!(s.n(), 0);
}

#[test]
fn mline_justification_scale_close_undo_and_style() {
    let s = Sim::new();
    s.cmd("MLINE").ty("j").ty("z").ty("s").ty("10");
    s.click(0., 0.).click(40., 0.).click(40., 40.).click(60., 60.).ty("u").click(0., 40.).ty("c");
    assert!(!s.busy());
    let pl = plines(&s);
    assert_eq!(pl.len(), 2);
    assert!(pl.iter().all(|(v, closed)| *closed && v.len() == 4));
    let (min, max) = pl.iter().fold((f64::MAX, f64::MIN), |(a, b), (v, _)| (a.min(v[0].y), b.max(v[0].y)));
    assert!((min - -5.).abs() < 1e-6 && (max - 5.).abs() < 1e-6, "Zero justification centres the elements: {min} {max}");
    s.cmd("MLINE").ty("st").ty("WALLS");
    assert!(s.history_has("only STANDARD is available"));
    s.ty("st").ty("?");
    assert!(s.history_has("Available multiline styles: STANDARD"));
    s.ty("j").enter();
    assert!(s.history_has("Justification = Zero, Scale = 10.00"));
    s.esc();
    s.cmd("MLINE").click(0., 0.).enter();
    assert!(!s.busy());
    assert_eq!(plines(&s).len(), 2, "a single point draws nothing");
}

// ---------------------------------------------------------------- SKETCH

#[test]
fn sketch_records_points_at_least_the_increment_apart() {
    let s = Sim::new();
    s.cmd("SKETCH");
    assert_eq!(s.prompt(), "Specify sketch or [Type/Increment]:");
    s.click(0., 0.).click(0.5, 0.);
    assert!(s.history_has("Point not recorded"));
    assert_eq!(s.prompt(), "Specify next point or [Type/Increment/Undo]:");
    s.click(2., 0.).click(3., 5.).click(8., 5.).ty("u").click(9., 9.).enter();
    assert!(!s.busy());
    assert_eq!(s.n(), 1);
    let (v, closed) = plines(&s)[0].clone();
    assert!(!closed);
    assert_eq!(v.iter().map(|x| (x.x, x.y)).collect::<Vec<_>>(), vec![(0., 0.), (2., 0.), (3., 5.), (9., 9.)]);
    assert!(s.history_has("Polyline with 3 edges recorded."));
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn sketch_increment_and_type_options() {
    let s = Sim::new();
    s.cmd("SKETCH").ty("i").ty("5").ty("t").ty("l");
    s.click(0., 0.).click(3., 0.).click(6., 0.).click(12., 0.).enter();
    assert_eq!(s.n(), 2, "lines: points under 5 apart are skipped");
    assert!(s.history_has("2 lines recorded."));
    s.cmd("SKETCH").ty("t").ty("s").click(0., 20.).click(10., 25.).click(20., 20.).enter();
    let sp = s.ents().into_iter().find(|e| matches!(e.kind, Kind::Spline { .. })).expect("spline");
    assert!(matches!(&sp.kind, Kind::Spline { fit, .. } if fit.len() == 3));
    s.cmd("SKETCH").enter();
    assert!(s.history_has("Nothing sketched."));
    s.cmd("SKETCH").click(0., 0.).esc();
    assert_eq!(s.n(), 3, "Escape records nothing");
}

// ---------------------------------------------------------------- BLEND

fn fit_of(e: &Entity) -> Vec<Pt> {
    match &e.kind {
        Kind::Spline { fit, .. } => fit.clone(),
        k => panic!("{k:?}"),
    }
}

#[test]
fn blend_joins_two_lines_tangentially() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(20., 10.), p(30., 10.));
    s.cmd("BLEND");
    assert_eq!(s.prompt(), "Select first object or [CONtinuity]:");
    s.click(9., 0.);
    assert_eq!(s.message(), "Select second point on object");
    s.click(21., 10.);
    assert!(!s.busy());
    assert_eq!(s.n(), 3);
    let fit = fit_of(&s.ent(2));
    assert_eq!(fit.len(), 17);
    assert_eq!((fit[0], fit[16]), (p(10., 0.), p(20., 10.)));
    // leaves along +x and arrives along +x
    let (d0, d1) = (fit[1] - fit[0], fit[16] - fit[15]);
    assert!(d0.y.abs() < 0.1 * d0.x && d0.x > 0.);
    assert!(d1.y.abs() < 0.1 * d1.x && d1.x > 0.);
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn blend_picks_the_nearest_ends_and_supports_smooth_continuity() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.add(Entity::arc(p(30., 0.), 10., 0., rad(90.))); // from (40,0) up to (30,10)
    // the near end of the line is its start when picked at x=1; the arc end near (30,10) when picked there
    s.cmd("BLEND").ty("con").ty("s").click(1., 0.).click(30., 10.);
    let fit = fit_of(&s.ent(2));
    assert_eq!((fit[0], fit[16]), (p(0., 0.), p(30., 10.)));
    // leaving the line start it heads along -x
    assert!(fit[1].x < fit[0].x);
    // the other continuity gives a different curve
    let t = Sim::new();
    add_line(&t, p(0., 0.), p(10., 0.));
    t.add(Entity::arc(p(30., 0.), 10., 0., rad(90.)));
    t.cmd("BLEND").click(1., 0.).click(30., 10.);
    let tangent = fit_of(&t.ent(2));
    assert_ne!(tangent[8], fit[8]);
}

#[test]
fn blend_refuses_closed_curves_the_same_curve_and_meeting_ends() {
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 5.));
    add_line(&s, p(20., 0.), p(30., 0.));
    s.cmd("BLEND").click(5., 0.);
    assert!(s.history_has("Nothing selected"), "circles are not blendable");
    s.click(25., 0.).click(25., 0.);
    assert!(s.history_has("Select two different objects."));
    assert!(!s.busy());
    let t = Sim::new();
    add_line(&t, p(0., 0.), p(10., 0.));
    add_line(&t, p(10., 0.), p(10., 10.));
    t.cmd("BLEND").click(9., 0.).click(10., 1.);
    assert!(t.history_has("already meet"));
    assert_eq!(t.n(), 2);
}

// ---------------------------------------------------------------- SPLINEDIT

fn spline4(s: &Sim) -> u64 {
    s.add(Entity::spline(vec![p(0., 0.), p(5., 5.), p(10., 0.), p(15., 5.)], false))
}
fn spline_state(s: &Sim, id: u64) -> (Vec<Pt>, bool) {
    match s.doc(|d| d.by_id(id).unwrap().kind.clone()) {
        Kind::Spline { fit, closed } => (fit, closed),
        k => panic!("{k:?}"),
    }
}

#[test]
fn splineedit_closes_opens_reverses_and_undoes() {
    let s = Sim::new();
    let id = spline4(&s);
    s.cmd("SPLINEDIT").click(0., 0.);
    assert_eq!(s.prompt(), "Enter an option or [Fit data/Close/rEverse/Undo/eXit] <eXit>:");
    s.ty("c");
    assert!(spline_state(&s, id).1);
    assert_eq!(s.prompt(), "Enter an option or [Fit data/Open/rEverse/Undo/eXit] <eXit>:");
    s.ty("e");
    assert_eq!(spline_state(&s, id).0[0], p(15., 5.));
    s.ty("u").ty("u");
    assert_eq!(spline_state(&s, id), (vec![p(0., 0.), p(5., 5.), p(10., 0.), p(15., 5.)], false));
    s.ty("u");
    assert!(s.history_has("Nothing to undo."));
    s.ty("c").ty("o");
    assert!(!spline_state(&s, id).1);
    s.enter();
    assert!(!s.busy());
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn splineedit_uses_the_selected_spline_and_checks_the_fit_point_count() {
    let s = Sim::new();
    let id = s.add(Entity::spline(vec![p(0., 0.), p(10., 0.)], false));
    s.click(5., 0.); // pick-first
    s.cmd("SPLINEDIT");
    assert_eq!(s.message(), "Enter an option");
    s.ty("c");
    assert!(s.history_has("needs at least three fit points"));
    assert!(!spline_state(&s, id).1);
    s.ty("f").ty("d");
    assert!(s.history_has("A spline needs at least 2 fit points."));
    s.esc();
}

#[test]
fn splineedit_fit_data_move_delete_and_add() {
    let s = Sim::new();
    let id = spline4(&s);
    s.cmd("SPLINEDIT").click(0., 0.).ty("f");
    assert_eq!(s.prompt(), "Enter a fit data option or [Add/Delete/Move/eXit] <eXit>:");
    s.ty("m");
    assert_eq!(s.prompt(), "Specify new location or [Next/Previous/Select point/eXit] <N>:");
    s.enter(); // next: fit point 1
    s.click(5., 9.); // moves it, then the next one is current
    assert_eq!(spline_state(&s, id).0[1], p(5., 9.));
    s.click(10., -4.);
    assert_eq!(spline_state(&s, id).0[2], p(10., -4.));
    s.ty("p").ty("p").ty("s").click(15.1, 5.1).click(16., 7.);
    assert_eq!(spline_state(&s, id).0[3], p(16., 7.), "Select point makes the nearest fit point current");
    s.ty("x");
    assert_eq!(s.message(), "Enter a fit data option");
    s.ty("d").click(0.2, 0.);
    assert_eq!(spline_state(&s, id).0.len(), 3);
    s.ty("a").click(5., 9.).click(7., 12.);
    let (fit, _) = spline_state(&s, id);
    assert_eq!(fit, vec![p(5., 9.), p(7., 12.), p(10., -4.), p(16., 7.)]);
    s.ty("x").ty("x");
    assert!(!s.busy());
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
}

#[test]
fn splineedit_fit_move_previews_the_edited_curve() {
    let s = Sim::new();
    spline4(&s);
    s.cmd("SPLINEDIT").click(0., 0.).ty("f").ty("m").mv(3., 8.);
    let pv = s.preview();
    assert_eq!(pv.len(), 1);
    match &pv[0] {
        cadforge_engine::runtime::PreviewItem::Entity(e) => assert_eq!(fit_of(e)[0], p(3., 8.)),
        o => panic!("{o:?}"),
    }
    s.esc();
}

// ---------------------------------------------------------------- ARRAYPATH

fn centers(s: &Sim) -> Vec<Pt> {
    s.ents().iter().filter_map(|e| if let Kind::Circle { c, .. } = e.kind { Some(c) } else { None }).collect()
}

#[test]
fn arraypath_divides_a_line_into_items() {
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 2.));
    add_line(&s, p(0., 0.), p(100., 0.));
    s.cmd("ARRAYPATH").click(0., 2.).enter();
    assert_eq!(s.message(), "Select path curve");
    s.click(50., 0.);
    assert_eq!(s.prompt(), "Enter path method or [Divide/Measure/Base point] <Measure>:");
    s.ty("d");
    assert_eq!(s.message(), "Enter number of items along path");
    s.ty("5");
    assert_eq!(s.prompt(), "Align items to match the orientation of the path or [Yes/No] <Yes>:");
    s.enter();
    assert!(!s.busy());
    let xs: Vec<f64> = centers(&s).iter().map(|c| c.x).collect();
    assert_eq!(xs.len(), 5);
    for (i, x) in xs.iter().enumerate() {
        near(*x, 25. * i as f64);
    }
    assert_eq!(s.sel_count(), 0);
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
    assert!(s.history_has("5 items in the path array."));
}

#[test]
fn arraypath_measures_a_distance_along_the_path() {
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 2.));
    add_line(&s, p(0., 0.), p(100., 0.));
    s.cmd("ARRAYPATH").click(0., 2.).enter().click(50., 0.).enter();
    assert_eq!(s.message(), "Specify the distance between items along path");
    assert_eq!(s.prompt(), "Specify the distance between items along path <4>:", "the default is the width of the objects");
    s.ty("30").enter();
    let xs: Vec<f64> = centers(&s).iter().map(|c| c.x).collect();
    assert_eq!(xs.len(), 4);
    near(xs[3], 90.);
}

#[test]
fn arraypath_aligns_items_to_an_arc_and_moves_the_base_point_onto_the_path() {
    let s = Sim::new();
    add_line(&s, p(98., 0.), p(102., 0.));
    s.add(Entity::arc(p(0., 0.), 100., 0., rad(90.)));
    s.cmd("ARRAYPATH").click(98.5, 0.).enter().click(0., 100.).ty("d").ty("3").enter();
    let lines: Vec<(Pt, Pt)> = s.ents().iter().filter(|e| matches!(e.kind, Kind::Line { .. })).map(ab).collect();
    assert_eq!(lines.len(), 3);
    // the last item sits at the arc end, turned a quarter turn: vertical and centred on (0,100)
    let last = lines[2];
    near((last.0.x + last.1.x) / 2., 0.);
    near((last.0.y + last.1.y) / 2., 100.);
    near((last.0.x - last.1.x).abs(), 0.);
    near((last.0.y - last.1.y).abs(), 4.);
    // the middle item is turned 45 degrees
    let mid = lines[1];
    near(angle_of(mid.0, mid.1).rem_euclid(PI), rad(45.).rem_euclid(PI));
}

#[test]
fn arraypath_without_alignment_and_with_a_base_point() {
    let s = Sim::new();
    s.add(Entity::circle(p(0., 10.), 2.));
    add_line(&s, p(0., 0.), p(0., 100.));
    s.cmd("ARRAYPATH").click(2., 10.).enter().click(0., 50.).ty("b").click(0., 0.).ty("d").ty("3").ty("n");
    let cs = centers(&s);
    assert_eq!(cs.len(), 3);
    // the base point (0,0) rides on the path, so the circle keeps its offset of 10 above it
    let mut ys: Vec<f64> = cs.iter().map(|c| c.y).collect();
    ys.sort_by(|a, b| a.total_cmp(b));
    near(ys[0], 10.);
    near(ys[1], 60.);
    near(ys[2], 110.);
}

#[test]
fn arraypath_around_a_closed_path_does_not_repeat_the_start() {
    let s = Sim::new();
    s.add(Entity::circle(p(10., 0.), 1.));
    s.add(Entity::circle(p(0., 0.), 10.));
    s.cmd("ARRAYPATH").click(11., 0.).enter().click(0., 10.).ty("d").ty("4").ty("n");
    let mut cs: Vec<Pt> = centers(&s).into_iter().filter(|c| dist(*c, p(0., 0.)) > 1.).collect();
    assert_eq!(cs.len(), 4);
    cs.sort_by(|a, b| angle_of(Pt::ZERO, *a).total_cmp(&angle_of(Pt::ZERO, *b)));
    let angles: Vec<f64> = cs.iter().map(|c| deg(angle_of(Pt::ZERO, *c)).round()).collect();
    assert_eq!(angles, vec![-90., 0., 90., 180.]);
}

#[test]
fn arraypath_reports_a_zero_length_path_and_bad_counts() {
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 2.));
    add_line(&s, p(0., 5.), p(100., 5.));
    s.cmd("ARRAYPATH").click(2., 0.).enter().click(50., 5.).ty("d").ty("1");
    assert!(s.history_has("*Invalid*") || s.history_has("Requires"));
    s.esc();
    assert_eq!(centers(&s).len(), 1);
}

// ---------------------------------------------------------------- registry

#[test]
fn every_command_of_the_batch_is_registered_with_its_aliases() {
    let s = Sim::new();
    let reg = s.registry();
    for name in ["SELECT", "FILTER", "MULTIPLE", "GROUPEDIT", "GROUPS", "CHPROP", "CHANGE", "ALIGN", "OVERKILL", "FLATTEN", "REGION", "GRADIENT", "WIPEOUT", "REVCLOUD", "MLINE", "SKETCH", "BLEND", "SPLINEDIT", "ARRAYPATH", "ARRAYRECT", "ARRAYPOLAR"] {
        let d = reg.find(name).unwrap_or_else(|| panic!("{name} is missing"));
        assert!(!d.title.is_empty() && !d.desc.is_empty() && !d.group.is_empty(), "{name} needs a title, description and group");
    }
    for (alias, name) in [("AL", "ALIGN"), ("FI", "FILTER"), ("REG", "REGION"), ("SPE", "SPLINEDIT"), ("ML", "MLINE"), ("AR", "ARRAY")] {
        assert_eq!(reg.find(alias).unwrap().name, name, "{alias}");
    }
}
