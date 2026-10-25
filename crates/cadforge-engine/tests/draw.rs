//! Port of `tests/unit/draw.test.js`.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::{Sim, Step::*};

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6, "{a} != {b}");
}
fn near_t(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} != {b}");
}
fn line_ab(e: &Entity) -> (Pt, Pt) {
    match e.kind {
        Kind::Line { a, b } => (a, b),
        _ => panic!("not a line {e:?}"),
    }
}
fn circ(e: &Entity) -> (Pt, f64) {
    match e.kind {
        Kind::Circle { c, r } => (c, r),
        _ => panic!("not a circle {e:?}"),
    }
}
fn arc(e: &Entity) -> (Pt, f64, f64, f64) {
    match e.kind {
        Kind::Arc { c, r, a0, a1 } => (c, r, a0, a1),
        _ => panic!("not an arc {e:?}"),
    }
}

#[test]
fn line_draws_segments_supports_undo_and_close_and_the_prompt_lists_options() {
    let s = Sim::new();
    s.cmd("LINE");
    assert_eq!(s.prompt(), "Specify first point:");
    s.click(0., 0.);
    assert_eq!(s.prompt(), "Specify next point:");
    s.click(10., 0.).click(10., 10.);
    assert_eq!(s.prompt(), "Specify next point or [Close/Undo]:");
    s.click(0., 10.);
    s.ty("u");
    assert_eq!(s.n(), 2);
    s.click(5., 12.);
    s.ty("c");
    assert!(!s.busy());
    assert_eq!(s.n(), 4);
    assert_eq!(line_ab(&s.ent(3)).1, pt(0., 0.));
    assert_eq!(s.doc(|d| d.undo_depth()), 1);
    s.undo();
    assert_eq!(s.n(), 0);
}

#[test]
fn line_accepts_typed_absolute_relative_polar_and_direct_distance_entry() {
    let s = Sim::new();
    s.cmd("L");
    s.ty("1,1").ty("@5,0").ty("@5<90");
    s.mv(11., 8.); // direct distance entry uses the cursor direction from the last point
    s.ty("2");
    s.enter();
    assert!(!s.busy());
    assert_eq!(s.n(), 3);
    let e = s.ents();
    near(line_ab(&e[0]).1.x, 6.);
    near(line_ab(&e[0]).1.y, 1.);
    near(line_ab(&e[1]).1.x, 6.);
    near(line_ab(&e[1]).1.y, 6.);
    let (a, b) = line_ab(&e[2]);
    near(len(b - a), 2.);
}

#[test]
fn escape_cancels_but_keeps_entities_already_created_enter_repeats_the_last_command() {
    let s = Sim::new();
    s.cmd("LINE").click(0., 0.).click(5., 0.).esc();
    assert!(!s.busy());
    assert_eq!(s.n(), 1);
    s.ty(""); // Enter at idle repeats LINE
    assert_eq!(s.message(), "Specify first point");
    s.esc();
    assert!(!s.busy());
}

#[test]
fn circle_centre_radius_by_click_typed_radius_diameter_2p_3p() {
    let s = Sim::new();
    s.play("C", &[Click(0., 0.), Click(3., 4.)]);
    near(circ(&s.ent(0)).1, 5.);
    s.play("CIRCLE", &[Click(20., 0.), Type("2.5")]);
    near(circ(&s.ent(1)).1, 2.5);
    s.play("CIRCLE", &[Click(40., 0.), Type("d"), Type("8")]);
    near(circ(&s.ent(2)).1, 4.);
    s.play("CIRCLE", &[Type("2p"), Click(0., 20.), Click(10., 20.)]);
    near(circ(&s.ent(3)).1, 5.);
    near(circ(&s.ent(3)).0.x, 5.);
    s.play("CIRCLE", &[Type("3p"), Click(10., 40.), Click(0., 50.), Click(-10., 40.)]);
    near(circ(&s.ent(4)).1, 10.);
    near(circ(&s.ent(4)).0.y, 40.);
    // default radius is remembered
    s.cmd("C").click(100., 100.);
    assert!(s.prompt().contains("<10>"), "{}", s.prompt());
    s.enter();
    assert!(!s.busy());
    near(circ(&s.ent(5)).1, 10.);
}

#[test]
fn rectang_with_fillet_option_produces_a_rounded_closed_polyline() {
    let s = Sim::new();
    s.play("REC", &[Type("f"), Type("2"), Click(0., 0.), Click(20., 10.)]);
    match &s.ent(0).kind {
        Kind::Pline { v, closed } => {
            assert!(*closed);
            assert_eq!(v.len(), 8);
            near_t(pline_area(v).abs(), 200. - (4. - PI) * 4., 1e-6);
        }
        k => panic!("{k:?}"),
    }
    s.play("REC", &[Type("f"), Type("0"), Click(50., 0.), Type("d"), Type("10"), Type("6"), Click(60., 10.)]);
    match &s.ent(1).kind {
        Kind::Pline { v, .. } => near(pline_area(v).abs(), 60.),
        k => panic!("{k:?}"),
    }
}

#[test]
fn polygon_ellipse_and_arc_three_point_start_center_end() {
    let s = Sim::new();
    s.play("POL", &[Type("6"), Click(0., 0.), Type("c"), Click(5., 0.)]); // circumscribed about a circle radius 5
    let v = match &s.ent(0).kind {
        Kind::Pline { v, .. } => v.clone(),
        k => panic!("{k:?}"),
    };
    assert_eq!(v.len(), 6);
    near_t(dist(pt(0., 0.), mid(v[0].pt(), v[1].pt())), 5., 1e-6);
    s.play("EL", &[Click(100., 0.), Click(110., 0.), Click(100., 3.)]);
    match &s.ent(1).kind {
        Kind::Ellipse(e) => {
            near(e.ratio, 0.6);
            near(e.c.x, 105.);
        }
        k => panic!("{k:?}"),
    }
    s.play("ARC", &[Click(10., 0.), Click(0., 10.), Click(-10., 0.)]);
    let (_, r, a0, a1) = arc(&s.ent(2));
    near(r, 10.);
    near(sweep_ccw(a0, a1), PI);
    s.play("ARC", &[Type("c"), Click(0., 0.), Click(5., 0.), Click(0., 9.)]);
    let (_, r, a0, a1) = arc(&s.ent(3));
    near(r, 5.);
    near(a0, 0.);
    near(a1, PI / 2.);
}

#[test]
fn pline_with_an_arc_segment_undo_and_close() {
    let s = Sim::new();
    s.cmd("PL").click(0., 0.).click(10., 0.);
    s.ty("a").click(10., 10.); // tangent arc up to (10,10)
    s.ty("l").click(0., 10.);
    s.ty("c");
    assert!(!s.busy());
    match &s.ent(0).kind {
        Kind::Pline { v, closed } => {
            assert!(*closed);
            assert_eq!(v.len(), 4);
            assert!(v[1].b.abs() > 0.1);
            near(v[1].x, 10.);
            near(v[1].y, 0.);
        }
        k => panic!("{k:?}"),
    }
}

#[test]
fn spline_point_xline_and_donut_create_objects() {
    let s = Sim::new();
    s.play("SPL", &[Click(0., 0.), Click(5., 5.), Click(10., 0.), Enter]);
    match &s.ent(0).kind {
        Kind::Spline { fit, .. } => assert_eq!(fit.len(), 3),
        k => panic!("{k:?}"),
    }
    s.play("PO", &[Click(3., 3.)]);
    assert_eq!(s.ent(1).type_name(), "point");
    s.play("XL", &[Type("h"), Click(0., 7.), Enter]);
    match s.ent(2).kind {
        Kind::Xline { d, .. } => near(d.y, 0.),
        ref k => panic!("{k:?}"),
    }
    s.play("DONUT", &[Type("2"), Type("6"), Click(0., 0.), Enter]);
    match &s.ent(3).kind {
        Kind::Hatch(h) => assert_eq!(h.loops.len(), 2),
        k => panic!("{k:?}"),
    }
}

#[test]
fn spline_by_control_vertices_and_its_editing() {
    use cadforge_core::entities::{grips_of, Kind};
    let p = cadforge_core::geom::pt;
    let s = Sim::new();
    s.setvar("OSMODE", 16384.0);
    s.cmd("SPLINE").ty("Method").ty("CV").ty("Degree").ty("2");
    s.ty("0,0").ty("10,10").ty("20,0").ty("30,10").enter();
    let Kind::Spline { fit, nurbs: Some(n), .. } = s.ent(0).kind else { panic!("a control-vertex spline: {}", s.log_text()) };
    assert!(fit.is_empty());
    assert_eq!((n.deg, n.cv.len()), (2, 4));
    assert_eq!(n.knots, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]);
    let pts = n.sample(8);
    assert_eq!((pts[0], *pts.last().unwrap()), (p(0., 0.), p(30., 10.)), "clamped: it starts and ends at the end vertices");
    assert_eq!(s.pref("SPLMETHOD"), 1.0, "the method is remembered");
    // the grips are the control vertices; moving one changes the vertex, not a fit point
    let gr = s.doc(|d| grips_of(&s.ent(0), d));
    assert_eq!(gr.len(), 4);
    // SPLINEDIT: move the second vertex and give the third a weight
    s.cmd("SPLINEDIT").click(0., 0.).ty("Edit vertex").ty("Move").ty("Next").ty("10,20").ty("eXit").ty("Weight").ty("20,0").ty("3").ty("eXit").ty("eXit");
    let Kind::Spline { nurbs: Some(n), .. } = s.ent(0).kind else { panic!() };
    assert_eq!(n.cv[1], p(10., 20.), "{}", s.log_text());
    assert_eq!(n.w, vec![1.0, 1.0, 3.0, 1.0]);
    // a closed control-vertex spline wraps around smoothly
    s.cmd("SPLINE").ty("0,50").ty("10,60").ty("20,50").ty("Close");
    let Kind::Spline { nurbs: Some(c), closed, .. } = s.ent(1).kind else { panic!("{}", s.log_text()) };
    assert!(closed && c.closed(), "{c:?}");
    s.cmd("SPLINE").ty("Method").ty("Fit").ty("0,80").ty("10,90").enter();
    assert!(matches!(s.ent(2).kind, Kind::Spline { nurbs: None, .. }), "back to fit points");
}
