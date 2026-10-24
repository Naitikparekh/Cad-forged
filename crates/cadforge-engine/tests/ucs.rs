use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;
use std::f64::consts::FRAC_PI_2;

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "expected {b}, got {a}");
}

fn line_ends(s: &Sim, i: usize) -> (Pt, Pt) {
    match s.ent(i).kind {
        Kind::Line { a, b } => (a, b),
        _ => panic!("not a line"),
    }
}

#[test]
fn world_ucs_changes_nothing() {
    let s = Sim::new();
    s.cmd("LINE").ty("10,10").ty("20,10").enter();
    let (a, b) = line_ends(&s, 0);
    near(a.x, 10.0);
    near(b.x, 20.0);
}

#[test]
fn moved_ucs_typed_points_are_relative_to_the_new_origin() {
    let s = Sim::new();
    s.cmd("UCS").ty("Move").ty("100,50");
    s.cmd("LINE").ty("0,0").ty("10,0").enter();
    let (a, b) = line_ends(&s, 0);
    near(a.x, 100.0);
    near(a.y, 50.0);
    near(b.x, 110.0);
    near(b.y, 50.0);
    // a leading # forces absolute coordinates in the UCS too (still relative to the UCS origin)
    s.cmd("UCS").ty("World");
    s.cmd("LINE").ty("0,0").ty("5,5").enter();
    let (a2, _) = line_ends(&s, 1);
    near(a2.x, 0.0);
}

#[test]
fn rotated_ucs_rotates_relative_and_polar_input() {
    let s = Sim::new();
    s.cmd("UCS").ty("New").ty("Z").ty("90");
    near(s.doc(|d| d.ucs.angle), FRAC_PI_2);
    // 10 units along the UCS X axis is 10 units along world +Y
    s.cmd("LINE").ty("0,0").ty("@10,0").enter();
    let (_, b) = line_ends(&s, 0);
    near(b.x, 0.0);
    near(b.y, 10.0);
    // polar: 5 < 90 in the UCS is 180 degrees in the world
    s.cmd("LINE").ty("0,0").ty("@5<90").enter();
    let (_, b) = line_ends(&s, 1);
    near(b.x, -5.0);
    near(b.y, 0.0);
}

#[test]
fn ucs_save_restore_prev_delete_and_object() {
    let s = Sim::new();
    s.add(Entity::line(pt(10.0, 10.0), pt(20.0, 20.0)));
    s.cmd("UCS").ty("New").ty("OBject").click(15.0, 15.0);
    let u = s.doc(|d| d.ucs);
    near(u.origin.x, 10.0);
    near(u.angle, std::f64::consts::FRAC_PI_4);
    s.cmd("UCS").ty("Save").ty("Diag");
    s.cmd("UCS").ty("World");
    assert!(s.doc(|d| d.ucs.is_world()));
    s.cmd("UCS").ty("Restore").ty("diag");
    near(s.doc(|d| d.ucs.origin.x), 10.0);
    s.cmd("UCS").ty("Prev");
    assert!(s.doc(|d| d.ucs.is_world()));
    s.cmd("UCS").ty("Del").ty("Diag");
    assert!(s.doc(|d| d.ucs_named.is_empty()));
    s.cmd("UCS").ty("Restore").ty("Diag");
    assert!(s.last_log().contains("no saved UCS"));
}

#[test]
fn three_point_ucs_and_ortho_follow_the_ucs_axes() {
    let s = Sim::new();
    s.cmd("UCS").ty("New").ty("3point").ty("0,0").ty("10,10").ty("-10,10");
    near(s.doc(|d| d.ucs.angle), std::f64::consts::FRAC_PI_4);
    s.setvar("ORTHOMODE", 1.0);
    s.cmd("LINE").ty("0,0");
    // cursor roughly along the UCS X axis (45 deg in world) but off to one side: ortho snaps onto the axis
    s.mv(20.0, 18.0);
    let p = s.h.borrow().rt.cursor.pt;
    near((p.y - p.x).abs(), 0.0);
    s.esc();
}

#[test]
fn ucs_persists_in_the_file() {
    let s = Sim::new();
    s.cmd("UCS").ty("Move").ty("3,4");
    s.cmd("UCS").ty("Save").ty("Home");
    let json = s.doc(|d| d.to_json_string());
    let back = cadforge_core::drawing::Drawing::from_json_str(&json, None).unwrap();
    near(back.ucs.origin.x, 3.0);
    assert!(back.ucs_named.contains_key("Home"));
}

#[test]
fn plan_aligns_the_view_with_the_ucs() {
    let s = Sim::new();
    s.add(Entity::line(pt(0.0, 0.0), pt(100.0, 0.0)));
    s.cmd("UCS").ty("New").ty("Z").ty("30");
    s.cmd("PLAN").enter();
    let yaw = s.h.borrow().editor().unwrap().view.yaw;
    near(yaw, -30f64.to_radians());
    s.cmd("PLAN").ty("World");
    near(s.h.borrow().editor().unwrap().view.yaw, 0.0);
}
