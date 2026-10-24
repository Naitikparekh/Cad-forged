//! One scenario per everyday command, so that every registered command is exercised by a test that checks its result.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn types(s: &Sim) -> Vec<&'static str> {
    s.ents().iter().map(|e| e.type_name()).collect()
}

fn square(s: &Sim) -> Vec<u64> {
    vec![s.add(Entity::line(p(0., 0.), p(10., 0.))), s.add(Entity::line(p(10., 0.), p(10., 10.))), s.add(Entity::line(p(10., 10.), p(0., 10.))), s.add(Entity::line(p(0., 10.), p(0., 0.)))]
}

#[test]
fn boundary_traces_a_closed_polyline_around_the_picked_point() {
    let s = Sim::new();
    square(&s);
    s.cmd("BOUNDARY").click(5., 5.).enter();
    let plines: Vec<Entity> = s.ents().into_iter().filter(|e| matches!(e.kind, Kind::Pline { .. })).collect();
    assert_eq!(plines.len(), 1, "{}", s.log_text());
    assert!(matches!(&plines[0].kind, Kind::Pline { closed: true, v } if v.len() == 4));
}

#[test]
fn breakatpoint_splits_a_line_in_two() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.cmd("BREAKATPOINT").click(2., 0.).click(4., 0.);
    assert_eq!(types(&s), vec!["line", "line"], "{}", s.log_text());
}

#[test]
fn divide_and_measure_place_points_along_an_object() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(12., 0.)));
    s.cmd("DIVIDE").click(6., 0.).ty("4");
    let pts: Vec<f64> = s.ents().iter().filter_map(|e| if let Kind::Point { p } = &e.kind { Some(p.x) } else { None }).collect();
    assert_eq!(pts.len(), 3, "{}", s.log_text());
    near(pts[0], 3.0);
    near(pts[2], 9.0);
    let m = Sim::new();
    m.add(Entity::line(p(0., 0.), p(10., 0.)));
    m.cmd("MEASURE").click(5., 0.).ty("3");
    let marks: Vec<f64> = m.ents().iter().filter_map(|e| if let Kind::Point { p } = &e.kind { Some(p.x) } else { None }).collect();
    assert_eq!(marks.len(), 3);
    near(marks[1], 6.0);
}

#[test]
fn explode_breaks_polylines_and_blocks_into_their_parts() {
    let s = Sim::new();
    let id = s.add(Entity::pline_pts(&[p(0., 0.), p(10., 0.), p(10., 10.), p(0., 10.)], true));
    s.set_sel(&[id]);
    s.cmd("EXPLODE");
    assert_eq!(types(&s), vec!["line", "line", "line", "line"]);
    s.doc_mut(|d| d.define_block("B", p(0., 0.), vec![Entity::circle(p(0., 0.), 1.), Entity::line(p(0., 0.), p(5., 0.))]));
    let ins = s.add(Entity::insert("B", p(50., 50.)));
    s.set_sel(&[ins]);
    s.cmd("EXPLODE");
    assert_eq!(s.n(), 6, "the reference is replaced by its two parts");
}

#[test]
fn fillet_joins_two_lines_with_and_without_a_radius() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.add(Entity::line(p(10., 5.), p(10., 20.)));
    s.cmd("FILLET").click(8., 0.).click(10., 12.);
    // radius 0: the lines are trimmed / extended to meet at the corner
    let Kind::Line { b, .. } = s.ent(0).kind else { panic!() };
    near(b.x, 10.0);
    near(b.y, 0.0);
    let t = Sim::new();
    t.setvar("FILLETRAD", 2.0);
    t.add(Entity::line(p(0., 0.), p(10., 0.)));
    t.add(Entity::line(p(10., 0.), p(10., 10.)));
    t.cmd("FILLET").click(8., 0.).click(10., 8.);
    assert!(t.ents().iter().any(|e| matches!(e.kind, Kind::Arc { r, .. } if (r - 2.0).abs() < 1e-9)), "{}", t.log_text());
}

#[test]
fn join_merges_collinear_lines_and_chains_into_a_polyline() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(5., 0.)));
    let b = s.add(Entity::line(p(5., 0.), p(10., 0.)));
    s.set_sel(&[a, b]);
    s.cmd("JOIN");
    assert_eq!(s.n(), 1, "{}", s.log_text());
    let t = Sim::new();
    let ids = [t.add(Entity::line(p(0., 0.), p(5., 0.))), t.add(Entity::line(p(5., 0.), p(5., 5.)))];
    t.set_sel(&ids);
    t.cmd("JOIN");
    assert_eq!(types(&t), vec!["pline"]);
}

#[test]
fn lengthen_offset_and_stretch_change_geometry() {
    let s = Sim::new();
    let id = s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.cmd("LENGTHEN").ty("DElta").ty("5").click(9., 0.).enter();
    let Kind::Line { b, .. } = s.ent(0).kind else { panic!() };
    near(b.x, 15.0);
    let o = Sim::new();
    o.add(Entity::line(p(0., 0.), p(10., 0.)));
    o.cmd("OFFSET").ty("3").click(5., 0.).click(5., 10.);
    assert_eq!(o.n(), 2);
    let Kind::Line { a, .. } = o.ent(1).kind else { panic!() };
    near(a.y, 3.0);
    let t = Sim::new();
    t.add(Entity::line(p(0., 0.), p(10., 0.)));
    t.cmd("STRETCH").click(12., 5.).click(7., -5.).enter().click(0., 0.).click(0., 4.);
    let Kind::Line { a, b } = t.ent(0).kind else { panic!() };
    assert_eq!((a, b), (p(0., 0.), p(10., 4.)), "{}", t.log_text());
    let _ = id;
}

#[test]
fn pedit_joins_lines_into_a_polyline() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(5., 0.)));
    s.add(Entity::line(p(5., 0.), p(5., 5.)));
    s.cmd("PEDIT").click(2., 0.).ty("Y");
    s.ty("Join").click(5., 3.).enter().enter();
    assert!(types(&s).contains(&"pline"), "{}", s.log_text());
}

#[test]
fn tofront_and_toback_change_the_draw_order() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(1., 0.)));
    let b = s.add(Entity::line(p(0., 1.), p(1., 1.)));
    s.set_sel(&[a]);
    s.cmd("TOFRONT");
    assert_eq!(s.ents().last().unwrap().id, a);
    s.set_sel(&[a]);
    s.cmd("TOBACK");
    assert_eq!(s.ents()[0].id, a);
    let _ = b;
}

#[test]
fn group_and_ungroup() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(1., 0.)));
    let b = s.add(Entity::line(p(0., 5.), p(1., 5.)));
    s.set_sel(&[a, b]);
    s.cmd("GROUP");
    assert!(s.ents().iter().all(|e| e.grp.is_some()), "{}", s.log_text());
    s.cmd("UNGROUP").click(0.5, 0.).enter();
    assert!(s.ents().iter().all(|e| e.grp.is_none()), "{}", s.log_text());
}

#[test]
fn ray_and_array_create_geometry() {
    let s = Sim::new();
    s.cmd("RAY").click(0., 0.).click(10., 10.).enter();
    assert_eq!(types(&s), vec!["ray"]);
    let a = Sim::new();
    let id = a.add(Entity::circle(p(0., 0.), 1.));
    a.set_sel(&[id]);
    a.cmd("ARRAY").ty("Rectangular").ty("2").ty("3").ty("10").ty("10");
    assert_eq!(a.n(), 6, "{}", a.log_text());
}

#[test]
fn cutclip_removes_the_objects_and_pasteclip_brings_them_back() {
    let s = Sim::new();
    let id = s.add(Entity::circle(p(5., 5.), 2.));
    s.set_sel(&[id]);
    s.cmd("CUTCLIP");
    assert_eq!(s.n(), 0);
    s.cmd("PASTECLIP").click(20., 20.);
    assert_eq!(types(&s), vec!["circle"]);
}

#[test]
fn id_and_measuregeom_report_values() {
    let s = Sim::new();
    s.cmd("ID").click(3., 4.);
    assert!(s.history_has("X = 3"), "{}", s.log_text());
    s.cmd("MEASUREGEOM").ty("Distance").click(0., 0.).click(3., 4.);
    assert!(s.history_has("5"), "{}", s.log_text());
}

#[test]
fn selectsimilar_adds_objects_of_the_same_type_and_layer() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("A").unwrap();
    });
    let a = s.add(Entity::circle(p(0., 0.), 1.));
    s.add(Entity::circle(p(10., 0.), 1.));
    s.add(Entity::circle(p(20., 0.), 1.).on_layer("A"));
    s.add(Entity::line(p(0., 5.), p(1., 5.)));
    s.set_sel(&[a]);
    s.cmd("SELECTSIMILAR");
    assert_eq!(s.sel_count(), 2, "{}", s.log_text());
}

#[test]
fn text_and_leaders_can_be_created_and_edited() {
    let s = Sim::new();
    let id = s.add(Entity::text(p(0., 0.), 2.5, "OLD"));
    s.cmd("TEXTEDIT").click(1., 1.).ty("NEW");
    assert!(matches!(&s.doc(|d| d.by_id(id).unwrap().kind.clone()), Kind::Text(t) if t.s == "NEW"), "{}", s.log_text());
    let m = Sim::new();
    m.cmd("MLEADER").click(10., 10.).click(20., 20.).ty("NOTE");
    assert!(m.ents().iter().any(|e| matches!(e.kind, Kind::Leader { .. })), "{}", m.log_text());
}

#[test]
fn dimension_commands_make_dimensions() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(30., 0.)));
    s.add(Entity::line(p(0., 0.), p(0., 30.)));
    s.add(Entity::circle(p(60., 20.), 8.));
    s.cmd("DIMALIGNED").click(0., 0.).click(30., 0.).click(15., -8.);
    s.cmd("DIMRADIUS").click(68., 20.).click(75., 30.);
    s.cmd("DIMDIAMETER").click(60., 28.).click(70., 35.);
    s.cmd("DIMANGULAR").click(15., 0.).click(0., 15.).click(10., 10.);
    let dims: Vec<Entity> = s.ents().into_iter().filter(|e| matches!(e.kind, Kind::Dim(_))).collect();
    assert_eq!(dims.len(), 4, "{}", s.log_text());
    // a baseline and a continued dimension build on the first one
    s.cmd("DIMLINEAR").click(0., 0.).click(10., 0.).click(5., -12.);
    s.cmd("DIMCONTINUE").click(20., 0.).enter().enter();
    s.cmd("DIMBASELINE").click(30., 0.).enter().enter();
    assert!(s.ents().iter().filter(|e| matches!(e.kind, Kind::Dim(_))).count() >= 6, "{}", s.log_text());
    s.cmd("DIM").click(0., 0.);
    s.esc();
}

#[test]
fn purge_removes_unused_layers_and_blocks() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("Unused").unwrap();
        d.define_block("LOOSE", p(0., 0.), vec![Entity::line(p(0., 0.), p(1., 1.))]);
    });
    s.cmd("PURGE").ty("All").ty("*").ty("No");
    assert!(s.doc(|d| d.layer("Unused").is_none() && !d.blocks.contains_key("LOOSE")), "{}", s.log_text());
}

#[test]
fn layer_commands_change_layer_state() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("A").unwrap();
        d.add_layer("B").unwrap();
    });
    let a = s.add(Entity::line(p(0., 0.), p(1., 0.)).on_layer("A"));
    s.add(Entity::line(p(0., 5.), p(1., 5.)).on_layer("B"));
    let state = |s: &Sim, n: &str| s.doc(|d| d.layer(n).map(|l| (l.on, l.frozen, l.locked)).unwrap());
    s.cmd("LAYOFF").click(0.5, 0.);
    assert!(!state(&s, "A").0, "{}", s.log_text());
    s.cmd("LAYON");
    assert!(state(&s, "A").0);
    s.cmd("LAYFRZ").click(0.5, 5.);
    assert!(state(&s, "B").1, "{}", s.log_text());
    s.cmd("LAYTHW");
    assert!(!state(&s, "B").1);
    s.cmd("LAYLCK").click(0.5, 0.);
    assert!(state(&s, "A").2);
    s.cmd("LAYULK").click(0.5, 0.);
    assert!(!state(&s, "A").2, "{}", s.log_text());
    s.cmd("LAYMCUR").click(0.5, 5.);
    assert_eq!(s.doc(|d| d.cur.layer.clone()), "B");
    // LAYCUR moves the selection onto the current layer, LAYMCH onto the layer of a destination object
    s.set_sel(&[a]);
    s.cmd("LAYCUR");
    assert_eq!(s.doc(|d| d.by_id(a).unwrap().layer.clone()), "B", "{}", s.log_text());
    s.doc_mut(|d| d.by_id_mut(a).unwrap().layer = "0".into());
    s.set_sel(&[a]);
    s.cmd("LAYMCH").click(0.5, 5.);
    assert_eq!(s.doc(|d| d.by_id(a).unwrap().layer.clone()), "B", "{}", s.log_text());
    // LAYDEL removes the layer and what is on it
    s.doc_mut(|d| {
        d.add_layer("Gone").unwrap();
    });
    let g = s.add(Entity::line(p(0., 9.), p(1., 9.)).on_layer("Gone"));
    s.cmd("LAYDEL").click(0.5, 9.).ty("Yes");
    assert!(s.doc(|d| d.layer("Gone").is_none() && d.by_id(g).is_none()), "{}", s.log_text());
}

#[test]
fn vports_and_vpclip_work_on_layouts() {
    let s = Sim::new();
    s.cmd("LAYOUT").ty("New").ty("Sheet");
    s.cmd("VPORTS").ty("2").enter();
    s.cmd("VPCLIP");
    s.esc();
    assert!(s.doc(|d| d.layout("Sheet").is_some()));
}
