//! Parametric constraints: geometric and dimensional constraints, parameters with expressions, AUTOCONSTRAIN, and
//! the geometry following edits.
use cadforge_core::constraints::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn line_of(s: &Sim, id: u64) -> (Pt, Pt) {
    s.doc(|d| match d.by_id(id).map(|e| e.kind.clone()) {
        Some(Kind::Line { a, b }) => (a, b),
        k => panic!("{k:?}"),
    })
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn horizontal_and_perpendicular_straighten_lines() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(10., 1.)));
    let b = s.add(Entity::line(p(10., 1.), p(12., 9.)));
    s.cmd("GCHORIZONTAL").click(5., 0.5);
    let (a0, a1) = line_of(&s, a);
    close(a0.y, a1.y);
    s.cmd("GCPERPENDICULAR").click(5., a0.y).click(11., 5.);
    let (b0, b1) = line_of(&s, b);
    close(dot(a1 - a0, b1 - b0), 0.0);
    // the first line picked stays where it was
    assert_eq!(line_of(&s, a), (a0, a1), "{}", s.log_text());
    assert_eq!(s.doc(|d| d.model.constraints.list.len()), 2);
}

#[test]
fn a_conflicting_constraint_is_refused() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.cmd("GCHORIZONTAL").click(5., 0.);
    s.cmd("GCVERTICAL").click(5., 0.);
    assert!(s.log_text().contains("Vertical constraint not applied"), "{}", s.log_text());
    assert_eq!(line_of(&s, a), (p(0., 0.), p(10., 0.)));
    assert_eq!(s.doc(|d| d.model.constraints.list.len()), 1);
}

#[test]
fn dimensional_constraints_drive_the_geometry_and_follow_parameters() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(10., 0.)));
    let b = s.add(Entity::line(p(0., 5.), p(8., 5.)));
    s.cmd("GCHORIZONTAL").click(5., 0.);
    s.cmd("DCALIGNED").enter().click(5., 0.).click(5., -3.).ty("width=25");
    let (a0, a1) = line_of(&s, a);
    close(dist(a0, a1), 25.0);
    s.cmd("DCALIGNED").enter().click(4., 5.).click(4., 8.).ty("width/2");
    let (b0, b1) = line_of(&s, b);
    close(dist(b0, b1), 12.5);
    s.cmd("PARAMETERS").ty("Edit").ty("width").ty("40").enter();
    let (a0, a1) = line_of(&s, a);
    close(dist(a0, a1), 40.0);
    let (b0, b1) = line_of(&s, b);
    close(dist(b0, b1), 20.0);
    s.cmd("PARAMETERS").ty("?").enter();
    assert!(s.log_text().contains("width = 40"), "{}", s.log_text());
    // a parameter that refers to a missing name is refused and nothing changes
    s.cmd("PARAMETERS").ty("Edit").ty("width").ty("nosuch*2").enter();
    let (a0, a1) = line_of(&s, a);
    close(dist(a0, a1), 40.0);
    // undo takes back the last change
    s.undo();
    let (a0, a1) = line_of(&s, a);
    close(dist(a0, a1), 25.0);
}

#[test]
fn radius_and_concentric_constraints() {
    let s = Sim::new();
    let c1 = s.add(Entity::circle(p(0., 0.), 5.));
    let c2 = s.add(Entity::circle(p(1., 1.), 2.));
    s.cmd("GCCONCENTRIC").click(5., 0.).click(3., 1.);
    s.cmd("DCRADIUS").click(0., 5.).click(8., 8.).ty("7");
    let circ = |id: u64| s.doc(|d| match d.by_id(id).map(|e| e.kind.clone()) {
        Some(Kind::Circle { c, r }) => (c, r),
        k => panic!("{k:?}"),
    });
    let ((ca, ra), (cb, _)) = (circ(c1), circ(c2));
    close(ra, 7.0);
    close(dist(ca, cb), 0.0);
    assert!(dist(ca, p(0., 0.)) < 1e-9, "the first circle stays: {ca:?} {}", s.log_text());
}

fn rectangle(s: &Sim) -> [u64; 4] {
    [s.add(Entity::line(p(0., 0.), p(20., 0.))), s.add(Entity::line(p(20., 0.), p(20., 10.))), s.add(Entity::line(p(20., 10.), p(0., 10.))), s.add(Entity::line(p(0., 10.), p(0., 0.)))]
}

#[test]
fn autoconstrain_keeps_a_rectangle_closed_and_square_when_a_side_moves() {
    let s = Sim::new();
    let [bottom, right, top, left] = rectangle(&s);
    s.cmd("AUTOCONSTRAIN").click(10., 0.).click(20., 5.).click(10., 10.).click(0., 5.).enter();
    let kinds: Vec<CKind> = s.doc(|d| d.model.constraints.list.iter().map(|c| c.kind).collect());
    assert_eq!(kinds.iter().filter(|k| **k == CKind::Coincident).count(), 4, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| **k == CKind::Horizontal).count(), 2, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| **k == CKind::Vertical).count(), 2, "{kinds:?}");
    // MOVE the top side up: the sides stretch to stay joined and upright, the bottom stays
    s.set_sel(&[top]);
    s.cmd("MOVE").click(0., 0.).click(0., 5.);
    let (t0, t1) = line_of(&s, top);
    close(t0.y, 15.0);
    close(t1.y, 15.0);
    assert_eq!(line_of(&s, bottom), (p(0., 0.), p(20., 0.)), "{}", s.log_text());
    let (r0, r1) = line_of(&s, right);
    close(r1.y, 15.0);
    close(r0.x, r1.x);
    let (l0, _) = line_of(&s, left);
    close(l0.y, 15.0);
    // DELCONSTRAINT frees an object
    s.cmd("DELCONSTRAINT").click(10., 15.).enter();
    assert!(s.doc(|d| d.model.constraints.list.iter().all(|c| c.refs.iter().all(|r| r.ent != top))));
}

#[test]
fn coincident_fix_and_erase() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(10., 0.)));
    let b = s.add(Entity::line(p(11., 1.), p(20., 5.)));
    s.cmd("GCCOINCIDENT").click(9.9, 0.).click(11.1, 1.1);
    let (b0, _) = line_of(&s, b);
    assert!(dist(b0, p(10., 0.)) < 1e-9, "{b0:?} {}", s.log_text());
    s.cmd("GCFIX").click(0.1, 0.);
    // a fixed point does not move: MOVE of the fixed line leaves it where it was
    s.set_sel(&[a]);
    s.cmd("MOVE").click(0., 0.).click(3., 0.);
    let (a0, _) = line_of(&s, a);
    assert!(dist(a0, p(0., 0.)) < 1e-9, "{a0:?} {}", s.log_text());
    // erasing an object drops its constraints at the next solve
    s.set_sel(&[b]);
    s.cmd("ERASE");
    s.add(Entity::circle(p(50., 50.), 1.));
    s.set_sel(&[a]);
    s.cmd("MOVE").click(0., 0.).click(0., 0.);
    assert!(s.doc(|d| d.model.constraints.list.iter().all(|c| c.refs.iter().all(|r| r.ent != b))));
}

#[test]
fn constraints_are_saved_with_the_drawing_and_shown() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.add(Entity::circle(p(30., 0.), 4.));
    s.cmd("GCHORIZONTAL").click(5., 0.);
    s.cmd("DCDIAMETER").click(34., 0.).click(40., 6.).ty("d=10");
    let json = s.doc(cadforge_core::io::to_json);
    let back = cadforge_core::io::from_json(&json, "t").unwrap();
    assert_eq!(back.model.constraints.list.len(), 2);
    assert_eq!(back.model.constraints.list[1].expr, "10");
    let shown = s.doc(|d| display(d, 0.1));
    assert!(shown.iter().any(|e| matches!(&e.kind, Kind::Dim(dd) if dd.txt.as_deref() == Some("d=10"))), "{shown:?}");
    assert!(shown.iter().any(|e| matches!(&e.kind, Kind::Text(t) if t.s == "H")));
    s.cmd("DCDISPLAY").ty("Hide");
    assert!(!s.doc(|d| display(d, 0.1)).iter().any(|e| matches!(e.kind, Kind::Dim(_))));
}

#[test]
fn expressions() {
    let mut v = std::collections::HashMap::new();
    v.insert("d1".to_string(), 10.0);
    assert_eq!(eval_expr("d1*2 + 3^2 - (4/2)", &v).unwrap(), 27.0);
    assert!((eval_expr("sin(30)*D1", &v).unwrap() - 5.0).abs() < 1e-12);
    assert_eq!(eval_expr("max(1, d1, 3)", &v).unwrap(), 10.0);
    assert!(eval_expr("d2", &v).is_err());
    assert!(eval_expr("1/0", &v).is_err());
    assert!(eval_expr("2 3", &v).is_err());
}
