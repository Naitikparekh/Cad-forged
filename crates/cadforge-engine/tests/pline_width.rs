//! PLINE Width / Halfwidth, the PLINEWID default, and PEDIT Width / Reverse on wide polylines.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn verts(s: &Sim, i: usize) -> (Vec<Vertex>, bool) {
    match s.ent(i).kind {
        Kind::Pline { v, closed } => (v, closed),
        k => panic!("{k:?}\n{}", s.log_text()),
    }
}

fn widths(v: &[Vertex]) -> Vec<(f64, f64)> {
    v.iter().map(|q| (q.sw, q.ew)).collect()
}

#[test]
fn width_sets_the_taper_of_the_next_edge_and_the_end_width_carries_on() {
    let s = Sim::new();
    s.cmd("PLINE")
        .click(0., 0.)
        .ty("Width")
        .ty("2")
        .ty("6")
        .click(10., 0.)
        .click(10., 10.)
        .enter();
    let (v, closed) = verts(&s, 0);
    assert!(!closed);
    assert_eq!(
        widths(&v),
        vec![(2.0, 6.0), (6.0, 6.0), (0.0, 0.0)],
        "{}",
        s.log_text()
    );
}

#[test]
fn the_ending_width_defaults_to_the_starting_width() {
    let s = Sim::new();
    s.cmd("PLINE")
        .click(0., 0.)
        .ty("Width")
        .ty("3")
        .enter()
        .click(10., 0.)
        .enter();
    assert_eq!(
        widths(&verts(&s, 0).0),
        vec![(3.0, 3.0), (0.0, 0.0)],
        "{}",
        s.log_text()
    );
}

#[test]
fn halfwidth_doubles_what_is_typed() {
    let s = Sim::new();
    s.cmd("PLINE")
        .click(0., 0.)
        .ty("Halfwidth")
        .ty("1")
        .ty("2")
        .click(10., 0.)
        .enter();
    assert_eq!(widths(&verts(&s, 0).0)[0], (2.0, 4.0), "{}", s.log_text());
}

#[test]
fn plinewid_is_the_default_width() {
    let s = Sim::new();
    s.setvar("PLINEWID", 3.0);
    s.cmd("PLINE")
        .click(0., 0.)
        .click(10., 0.)
        .click(10., 10.)
        .enter();
    assert_eq!(
        widths(&verts(&s, 0).0),
        vec![(3.0, 3.0), (3.0, 3.0), (0.0, 0.0)]
    );
}

#[test]
fn closing_uses_the_current_width_for_the_closing_edge() {
    let s = Sim::new();
    s.cmd("PLINE")
        .click(0., 0.)
        .click(10., 0.)
        .click(10., 10.)
        .ty("Width")
        .ty("1")
        .ty("1")
        .ty("Close");
    let (v, closed) = verts(&s, 0);
    assert!(closed);
    assert_eq!(
        widths(&v),
        vec![(0.0, 0.0), (0.0, 0.0), (1.0, 1.0)],
        "{}",
        s.log_text()
    );
}

#[test]
fn undo_inside_pline_restores_the_previous_width() {
    let s = Sim::new();
    s.cmd("PLINE")
        .click(0., 0.)
        .ty("Width")
        .ty("2")
        .ty("2")
        .click(10., 0.)
        .ty("Width")
        .ty("5")
        .ty("5")
        .click(10., 10.)
        .ty("Undo")
        .click(20., 0.)
        .enter();
    // the third point was undone; the next edge from (10,0) gets the width in force when it was drawn
    let (v, _) = verts(&s, 0);
    assert_eq!(v.len(), 3, "{}", s.log_text());
    assert_eq!((v[0].sw, v[0].ew), (2.0, 2.0));
}

#[test]
fn arc_segments_take_widths_too() {
    let s = Sim::new();
    s.cmd("PLINE")
        .click(0., 0.)
        .ty("Arc")
        .ty("Width")
        .ty("2")
        .ty("2")
        .click(10., 10.)
        .enter();
    let (v, _) = verts(&s, 0);
    assert_eq!((v[0].sw, v[0].ew), (2.0, 2.0), "{}", s.log_text());
    assert!(v[0].b != 0.0, "still an arc edge");
}

#[test]
fn pedit_width_sets_every_edge_and_reverse_swaps_the_taper() {
    let s = Sim::new();
    s.add(Entity::pline(
        vec![
            Vertex::new(0., 0., 0.),
            Vertex::new(10., 0., 0.),
            Vertex::new(10., 10., 0.),
        ],
        false,
    ));
    s.cmd("PEDIT").click(5., 0.).ty("Width").ty("4").enter();
    assert!(
        widths(&verts(&s, 0).0).iter().all(|w| *w == (4.0, 4.0)),
        "{:?}\n{}",
        widths(&verts(&s, 0).0),
        s.log_text()
    );
    s.undo();

    let t = Sim::new();
    t.add(Entity::pline(
        vec![
            Vertex::wide(0., 0., 0., 1.0, 2.0),
            Vertex::wide(10., 0., 0., 2.0, 5.0),
            Vertex::new(10., 10., 0.),
        ],
        false,
    ));
    t.cmd("PEDIT").click(5., 0.).ty("Reverse").enter();
    let (v, _) = verts(&t, 0);
    assert_eq!((v[0].x, v[0].y), (10., 10.), "{}", t.log_text());
    // the old last edge (2 -> 5) now comes first and tapers 5 -> 2, then 2 -> 1
    assert_eq!(widths(&v)[..2], [(5.0, 2.0), (2.0, 1.0)]);
}

#[test]
fn a_wide_polyline_can_be_picked_inside_its_width_and_moved_with_its_widths() {
    let s = Sim::new();
    s.add(Entity::pline(
        vec![Vertex::wide(0., 0., 0., 6.0, 6.0), Vertex::new(10., 0., 0.)],
        false,
    ));
    s.click(5., 2.0); // 2 units off the centre line, inside the 6-wide band
    assert_eq!(s.sel_count(), 1, "{}", s.log_text());
    s.cmd("MOVE").click(0., 0.).click(5., 5.);
    let (v, _) = verts(&s, 0);
    assert_eq!((v[0].x, v[0].y, v[0].sw, v[0].ew), (5., 5., 6.0, 6.0));
    let _ = p(0., 0.);
}
