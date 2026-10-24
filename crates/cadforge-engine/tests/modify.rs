//! Port of `tests/unit/modify.test.js`.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::runtime::PreviewItem;
use cadforge_engine::sim::{Sim, Step::*};

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

#[test]
fn erase_with_pick_first_selection_and_oops_restores() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(0., 5.), p(10., 5.));
    s.click(5., 0.); // idle click selects the first line
    assert_eq!(s.sel_count(), 1);
    s.cmd("E"); // pick-first: erases without prompting
    assert!(!s.busy());
    assert_eq!(s.n(), 1);
    s.cmd("OOPS");
    assert_eq!(s.n(), 2);
}

#[test]
fn move_with_preview_and_second_point_copy_makes_multiple_copies_selection_is_cleared() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("M").click(5., 0.).enter();
    s.click(0., 0.).mv(3., 4.);
    let pv = s.preview();
    assert_eq!(pv.len(), 1);
    match &pv[0] {
        PreviewItem::Entity(e) => near(ab(e).0.x, 3.),
        o => panic!("{o:?}"),
    }
    s.click(3., 4.);
    assert!(!s.busy());
    near(ab(&s.ent(0)).0.x, 3.);
    near(ab(&s.ent(0)).0.y, 4.);
    assert_eq!(s.sel_count(), 0);
    s.cmd("CO").click(8., 4.).enter();
    s.click(3., 4.).click(3., 14.).click(3., 24.).enter();
    assert!(!s.busy());
    assert_eq!(s.n(), 3);
    near(ab(&s.ent(2)).0.y, 24.);
}

#[test]
fn rotate_typed_angle_and_copy_scale_and_mirror() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("RO").click(5., 0.).enter().click(0., 0.).ty("90");
    near(ab(&s.ent(0)).1.x, 0.);
    near(ab(&s.ent(0)).1.y, 10.);
    s.cmd("SC").click(0., 5.).enter().click(0., 0.).ty("2.5");
    near(ab(&s.ent(0)).1.y, 25.);
    s.cmd("MI")
        .click(0., 12.)
        .enter()
        .click(0., 0.)
        .click(10., 0.)
        .ty("n");
    assert_eq!(s.n(), 2);
    near(ab(&s.ent(1)).1.y, -25.);
    s.cmd("MI")
        .click(0., 12.)
        .enter()
        .click(0., 0.)
        .click(10., 0.)
        .ty("y");
    assert_eq!(s.n(), 2);
    s.cmd("RO")
        .click(0., -12.)
        .enter()
        .click(0., 0.)
        .ty("c")
        .ty("90");
    assert_eq!(s.n(), 3);
}

#[test]
fn offset_distance_then_side_picks() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("O").ty("3");
    s.click(5., 0.).click(5., 8.); // above
    s.click(5., 0.).click(5., -8.); // below
    s.enter();
    assert!(!s.busy());
    assert_eq!(s.n(), 3);
    near(ab(&s.ent(1)).0.y, 3.);
    near(ab(&s.ent(2)).0.y, -3.);
    assert_eq!(s.doc(|d| d.settings.offset_gap), 3.0);
}

#[test]
fn trim_and_extend_with_cutting_edges() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(4., -3.), p(4., 3.));
    add_line(&s, p(8., -3.), p(8., 3.));
    s.cmd("TR").enter(); // all objects are cutting edges
    s.click(6., 0.).enter();
    assert!(!s.busy());
    assert_eq!(s.n(), 4); // middle piece removed: line splits in two
    let horiz = |s: &Sim| {
        s.ents()
            .into_iter()
            .filter(|e| matches!(e.kind, Kind::Line { a, b } if a.y == 0. && b.y == 0.))
            .collect::<Vec<_>>()
    };
    assert_eq!(horiz(&s).len(), 2);
    s.cmd("EX").enter();
    s.click(3.5, 0.).enter();
    assert!(!s.busy());
    let mut longer = horiz(&s);
    longer.sort_by(|a, b| ab(a).0.x.total_cmp(&ab(b).0.x));
    near(ab(&longer[0]).1.x, 8.); // the piece ends on the x=4 edge; EXTEND runs on to the next edge beyond its end
}

#[test]
fn fillet_and_chamfer_between_two_lines_polyline_fillet() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    add_line(&s, p(0., 0.), p(0., 10.));
    s.cmd("F").ty("r").ty("2");
    s.click(8., 0.).click(0., 8.);
    assert!(!s.busy());
    assert_eq!(s.n(), 3);
    match s.ent(2).kind {
        Kind::Arc { c, r, .. } => {
            near(r, 2.);
            near(c.x, 2.);
            near(c.y, 2.);
        }
        ref k => panic!("{k:?}"),
    }
    let t = Sim::new();
    add_line(&t, p(0., 0.), p(10., 0.));
    add_line(&t, p(0., 0.), p(0., 10.));
    t.cmd("CHA").ty("d").ty("2").ty("3");
    t.click(8., 0.).click(0., 8.);
    assert!(!t.busy());
    assert_eq!(t.n(), 3);
    near(ab(&t.ent(2)).0.x, 2.);
    near(ab(&t.ent(2)).1.y, 3.);
    let u = Sim::new();
    u.add(Entity::pline_pts(
        &[p(0., 0.), p(10., 0.), p(10., 10.), p(0., 10.)],
        true,
    ));
    u.doc_mut(|d| d.settings.fillet_radius = 1.0);
    u.cmd("F").ty("p").click(5., 0.);
    assert!(!u.busy());
    match &u.ent(0).kind {
        Kind::Pline { v, .. } => assert_eq!(v.len(), 8),
        k => panic!("{k:?}"),
    }
}

#[test]
fn array_rectangular_and_polar() {
    let s = Sim::new();
    s.add(Entity::circle(p(0., 0.), 1.));
    s.cmd("ARRAYRECT").click(1., 0.).enter();
    s.ty("2").ty("3").ty("10").ty("20");
    assert!(!s.busy());
    assert_eq!(s.n(), 6);
    let t = Sim::new();
    t.add(Entity::circle(p(10., 0.), 1.));
    t.cmd("ARRAYPOLAR").click(11., 0.).enter();
    t.click(0., 0.).ty("4").ty("360").ty("y");
    assert!(!t.busy());
    assert_eq!(t.n(), 4);
    match t.ent(1).kind {
        Kind::Circle { c, .. } => {
            assert!((c.x).abs() < 1e-6 && (c.y - 10.).abs() < 1e-6, "{c:?}");
        }
        ref k => panic!("{k:?}"),
    }
}

#[test]
fn stretch_moves_only_the_endpoints_inside_the_crossing_window() {
    let t = Sim::new();
    add_line(&t, p(0., 0.), p(10., 0.));
    t.cmd("S").click(12., 3.).click(8., -3.); // right-to-left = crossing
    t.enter().click(10., 0.).click(15., 5.);
    assert!(!t.busy());
    let (a, b) = ab(&t.ent(0));
    near(a.x, 0.);
    near(b.x, 15.);
    near(b.y, 5.);
}

#[test]
fn break_join_and_explode() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.cmd("BR").click(3., 0.).click(6., 0.);
    assert_eq!(s.n(), 2);
    s.cmd("J").click(1., 0.).click(9., 0.).enter();
    assert!(!s.busy());
    assert_eq!(s.n(), 2); // gap remains: cannot join disconnected lines
    let t = Sim::new();
    t.add(Entity::pline_pts(
        &[p(0., 0.), p(10., 0.), p(10., 10.)],
        false,
    ));
    t.cmd("X").click(5., 0.).enter();
    assert!(!t.busy());
    assert_eq!(t.n(), 2);
    assert_eq!(t.ent(0).type_name(), "line");
}

#[test]
fn grip_editing_stretch_an_endpoint_and_move_a_line_from_its_midpoint_grip() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(10., 0.));
    s.click(5., 0.); // select
    s.click(10., 0.); // hot grip at the endpoint
    assert_eq!(s.running().as_deref(), Some("_GRIPEDIT"));
    s.click(10., 10.);
    assert!(!s.busy());
    near(ab(&s.ent(0)).1.y, 10.);
    near(ab(&s.ent(0)).0.y, 0.);
    s.click(5., 5.); // midpoint grip (move)
    s.click(5., 8.);
    assert!(!s.busy());
    near(ab(&s.ent(0)).0.y, 3.);
    near(ab(&s.ent(0)).1.y, 13.);
    assert_eq!(s.doc(|d| d.undo_depth()), 2);
}

#[test]
fn selecting_by_window_and_crossing_at_idle_esc_clears() {
    let s = Sim::new();
    add_line(&s, p(0., 0.), p(4., 0.));
    add_line(&s, p(6., 0.), p(14., 0.));
    s.click(-1., -1.).click(5., 1.); // left-to-right window: only the first line is inside
    assert_eq!(s.sel_count(), 1);
    s.esc();
    assert_eq!(s.sel_count(), 0);
    s.click(10., 1.).click(3., -1.); // right-to-left crossing: both touched
    assert_eq!(s.sel_count(), 2);
    let _ = Click(0., 0.);
}
