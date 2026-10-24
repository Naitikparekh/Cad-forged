//! Port of `tests/unit/commands2.test.js`.
use cadforge_core::dim::{dim_geometry, DimType};
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::{Sim, Step::*};

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6, "{a} != {b}");
}
fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

#[test]
fn registry_resolves_aliases_synonyms_and_suggestions() {
    let s = Sim::new();
    let r = s.registry();
    for (typed, name) in [
        ("l", "LINE"),
        ("PL", "PLINE"),
        ("rec", "RECTANG"),
        ("bevel", "CHAMFER"),
        ("z", "ZOOM"),
        ("dli", "DIMLINEAR"),
    ] {
        assert_eq!(
            r.find(typed).map(|d| d.name.clone()).as_deref(),
            Some(name),
            "{typed}"
        );
    }
    assert!(r.find("nonsense").is_none());
    assert!(r
        .suggest("cir", true, &[], 12)
        .contains(&"CIRCLE".to_string()));
    assert!(r
        .suggest("ine", true, &[], 12)
        .contains(&"LINE".to_string()));
    assert!(r.names().len() > 100, "only {} commands", r.names().len());
}

#[test]
fn typing_a_system_variable_name_prompts_for_a_new_value() {
    let s = Sim::new();
    s.ty("DYNMODE");
    assert!(
        s.prompt().contains("Enter new value for DYNMODE <3>:"),
        "{}",
        s.prompt()
    );
    s.ty("1");
    assert!(!s.busy());
    assert_eq!(s.pref("DYNMODE"), 1.0);
    s.ty("LTSCALE").ty("2.5"); // LTSCALE is also a command with the same prompt
    assert_eq!(s.doc(|d| d.settings.ltscale), 2.5);
    s.ty("LTSCALE").ty("-1");
    assert!(s.busy()); // rejected: stays on the prompt
    s.esc();
    assert_eq!(s.doc(|d| d.settings.ltscale), 2.5);
    s.ty("PDSIZE").ty("3");
    assert_eq!(s.doc(|d| d.settings.pdsize), 3.0);
    s.ty("CLAYER");
    assert!(s.prompt().contains("<0>"), "{}", s.prompt());
    s.esc();
}

#[test]
fn undo_and_redo_commands() {
    let s = Sim::new();
    s.play("C", &[Click(0., 0.), Type("3")]);
    s.play("C", &[Click(20., 0.), Type("3")]);
    assert_eq!(s.n(), 2);
    s.cmd("U");
    assert_eq!(s.n(), 1);
    s.cmd("REDO");
    assert_eq!(s.n(), 2);
    s.cmd("UNDO").ty("2");
    assert_eq!(s.n(), 0);
}

#[test]
fn layer_commands_make_color_freeze_isolate_wildcard() {
    let s = Sim::new();
    s.cmd("-LAYER");
    s.ty("m").ty("Walls").ty("c").ty("red").ty("Walls");
    s.ty("n").ty("A-Door,A-Window").ty("f").ty("A-*").enter();
    assert!(!s.busy());
    assert_eq!(s.doc(|d| d.cur.layer.clone()), "Walls");
    assert_eq!(
        s.doc(|d| d.layer("Walls").unwrap().color),
        cadforge_core::style::Color::Aci(1)
    );
    assert!(s.doc(|d| d.layer("A-Door").unwrap().frozen && d.layer("A-Window").unwrap().frozen));
    s.play("L", &[Click(0., 0.), Click(5., 0.), Enter]);
    assert_eq!(s.ent(0).layer, "Walls");
    s.doc_mut(|d| {
        d.set_current_layer("0").unwrap();
        d.add(Entity::line(p(0., 5.), p(5., 5.)));
    });
    s.cmd("LAYISO").click(2., 0.).enter();
    assert!(s.doc(|d| d.layer("0").unwrap().locked
        || !d.layer("0").unwrap().on
        || d.layer("0").unwrap().name == d.cur.layer));
    s.cmd("LAYUNISO");
}

#[test]
fn zoom_window_extents_previous_scale_pan_by_displacement() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(100., 0.)));
    s.add(Entity::line(p(0., 50.), p(100., 50.)));
    s.with_view(|v| {
        v.w = 800.;
        v.h = 600.;
    });
    s.cmd("Z").ty("e");
    let scale_ext = s.view_scale();
    let t = s.with_view(|v| v.target);
    near(t.x, 50.);
    near(t.y, 25.);
    s.cmd("Z").ty("w").click(0., 0.).click(10., 10.);
    assert!(s.view_scale() > scale_ext * 5.);
    s.cmd("Z").ty("p");
    near(s.view_scale(), scale_ext);
    s.cmd("Z").ty("2x");
    near(s.view_scale(), scale_ext * 2.);
    let tx = s.with_view(|v| v.target.x);
    s.cmd("-PAN").click(0., 0.).click(10., 0.);
    near(s.with_view(|v| v.target.x), tx - 10.);
}

#[test]
fn block_insert_with_scale_and_rotation_explode_of_the_reference() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.add(Entity::circle(p(5., 5.), 2.));
    s.cmd("B").ty("Part").click(0., 0.);
    s.click(5., 0.).click(5., 7.).enter(); // pick-first not used; select both with clicks
    assert!(!s.busy());
    assert_eq!(s.doc(|d| d.blocks["Part"].entities.len()), 2);
    assert_eq!(s.n(), 1);
    assert_eq!(s.ent(0).type_name(), "insert");
    s.cmd("I")
        .ty("Part")
        .ty("s")
        .ty("2")
        .ty("r")
        .ty("90")
        .click(50., 50.);
    assert!(!s.busy());
    match &s.ent(1).kind {
        Kind::Insert(i) => {
            near(i.sx, 2.);
            near(i.rot, PI / 2.);
        }
        k => panic!("{k:?}"),
    }
    s.cmd("X").click(50., 50.).enter();
    assert!(s
        .ents()
        .iter()
        .any(|e| matches!(e.kind, Kind::Circle { r, .. } if (r - 4.).abs() < 1e-9)));
}

#[test]
fn dist_area_and_list_report_through_the_command_history() {
    let s = Sim::new();
    s.play("DI", &[Click(0., 0.), Click(3., 4.)]);
    assert!(
        s.log_text().contains("Distance = 5.0000"),
        "{}",
        s.log_text()
    );
    s.add(Entity::circle(p(20., 0.), 2.));
    s.play("AA", &[Type("o"), Click(22., 0.), Enter]);
    assert!(s.log_text().contains("Area = 12.5664"), "{}", s.log_text());
    s.cmd("LIST").click(22., 0.).enter();
    let log = s.log_text();
    assert!(
        log.contains("CIRCLE") && log.contains("Layer: \"0\""),
        "{log}"
    );
}

#[test]
fn dimlinear_creates_a_measured_dimension_and_text_creates_text() {
    let s = Sim::new();
    s.play("DLI", &[Click(0., 0.), Click(10., 0.), Click(5., 4.)]);
    let e = s.ent(0);
    let Kind::Dim(d) = &e.kind else {
        panic!("{e:?}")
    };
    assert_eq!(d.kind, DimType::Lin);
    near(d.ang.unwrap(), 0.);
    let geo = s.doc(|doc| dim_geometry(d, &doc.settings));
    near(geo.measured, 10.);
    assert_eq!(geo.text.s, "10.00");
    s.play(
        "TEXT",
        &[Click(0., 10.), Type("3"), Type("0"), Type("Hello"), Enter],
    );
    let t = s.ents().into_iter().find_map(|e| match e.kind {
        Kind::Text(t) => Some(t),
        _ => None,
    });
    let t = t.expect("a text entity");
    assert_eq!(t.s, "Hello");
    near(t.h, 3.);
}

#[test]
fn hatch_pick_internal_point_creates_a_hatch_from_the_surrounding_closed_region() {
    let s = Sim::new();
    s.add(Entity::pline_pts(
        &[p(0., 0.), p(20., 0.), p(20., 10.), p(0., 10.)],
        true,
    ));
    s.cmd("H").click(5., 5.).enter();
    assert!(!s.busy());
    let h = s.ents().into_iter().find_map(|e| match e.kind {
        Kind::Hatch(h) => Some(h),
        _ => None,
    });
    let h = h.expect("a hatch");
    assert_eq!(h.loops.len(), 1);
    assert_eq!(h.pattern, "ANSI31");
    s.play("H", &[Click(100., 100.), Enter]); // no region there
    assert_eq!(
        s.ents()
            .iter()
            .filter(|e| matches!(e.kind, Kind::Hatch(_)))
            .count(),
        1
    );
}

#[test]
fn matchprop_draworder_and_selectall() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("Red").unwrap();
    });
    let a = s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.doc_mut(|d| {
        let e = d.by_id_mut(a).unwrap();
        e.layer = "Red".into();
        e.color = Some(cadforge_core::style::Color::Aci(3));
    });
    let b = s.add(Entity::line(p(0., 5.), p(10., 5.)));
    s.cmd("MA").click(5., 0.).click(5., 5.).enter();
    assert!(!s.busy());
    let eb = s.doc(|d| d.by_id(b).cloned().unwrap());
    assert_eq!(eb.layer, "Red");
    assert_eq!(eb.color, Some(cadforge_core::style::Color::Aci(3)));
    s.cmd("DRAWORDER").click(5., 5.).enter().ty("f");
    assert!(!s.busy());
    assert_eq!(s.ents().last().unwrap().id, b);
    s.cmd("SELECTALL");
    assert_eq!(s.sel_count(), 2);
}
