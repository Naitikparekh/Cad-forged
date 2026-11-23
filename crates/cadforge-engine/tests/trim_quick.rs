//! TRIM and EXTEND in Quick mode (TRIMEXTENDMODE 1, the default as in AutoCAD 2021+): every object is an edge, the
//! object is clicked straight away, the hover preview shows the piece that goes (or the extension), and an object that
//! crosses nothing is erased.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::runtime::PreviewItem;
use cadforge_engine::sim::Sim;

fn lines(s: &Sim) -> Vec<(Pt, Pt)> {
    s.ents().into_iter().filter_map(|e| if let Kind::Line { a, b } = e.kind { Some((a, b)) } else { None }).collect()
}

fn grid() -> Sim {
    let s = Sim::new();
    s.add(Entity::line(pt(0., 0.), pt(10., 0.)));
    s.add(Entity::line(pt(4., -3.), pt(4., 3.)));
    s.add(Entity::line(pt(8., -3.), pt(8., 3.)));
    s
}

#[test]
fn quick_trim_needs_no_edges_and_previews_the_piece() {
    let s = grid();
    s.cmd("TRIM");
    s.mv(6., 0.);
    let pv: Vec<Entity> = s.preview().into_iter().filter_map(|p| if let PreviewItem::Entity(e) = p { Some(e) } else { None }).collect();
    assert_eq!(pv.len(), 1, "the piece that goes");
    let Kind::Line { a, b } = pv[0].kind else { panic!("{:?}", pv[0].kind) };
    assert!(dist(a, pt(4., 0.)) < 1e-9 && dist(b, pt(8., 0.)) < 1e-9, "{a:?} {b:?}");
    s.click(6., 0.);
    assert!(s.busy(), "TRIM goes on for the next object");
    let horiz: Vec<(Pt, Pt)> = lines(&s).into_iter().filter(|(a, b)| a.y == 0. && b.y == 0.).collect();
    assert_eq!(horiz.len(), 2, "the middle piece is gone");
    // a vertical line's end beyond the horizontal one
    s.click(4., 2.);
    assert!(lines(&s).iter().any(|(a, b)| a.x == 4. && (a.y - -3.).abs() < 1e-9 && b.y.abs() < 1e-9), "{:?}", lines(&s));
    s.enter();
    assert!(!s.busy());
    s.undo();
    assert_eq!(lines(&s).len(), 3, "one undo step for the whole TRIM");
}

#[test]
fn quick_trim_erases_an_object_that_crosses_nothing() {
    let s = grid();
    s.add(Entity::line(pt(20., 20.), pt(30., 20.)));
    s.cmd("TRIM").click(25., 20.).enter();
    assert_eq!(lines(&s).len(), 3, "{}", s.log_text());
}

#[test]
fn quick_extend_previews_and_extends_to_the_next_edge() {
    let s = Sim::new();
    s.add(Entity::line(pt(0., 0.), pt(3., 0.)));
    s.add(Entity::line(pt(8., -3.), pt(8., 3.)));
    s.cmd("EXTEND");
    s.mv(2.5, 0.);
    assert!(s.preview().iter().any(|p| matches!(p, PreviewItem::Entity(e) if matches!(e.kind, Kind::Line { b, .. } if (b.x - 8.).abs() < 1e-9))), "the extended line is previewed");
    s.click(2.5, 0.).enter();
    assert!(lines(&s).iter().any(|(a, b)| a.x == 0. && (b.x - 8.).abs() < 1e-9), "{:?}", lines(&s));
}

#[test]
fn mode_option_switches_to_standard() {
    let s = grid();
    s.cmd("TRIM").ty("O").ty("S");
    assert_eq!(s.h.borrow().sysvars.int("TRIMEXTENDMODE"), 0);
    assert!(s.prompt().contains("cutting edges"), "{}", s.prompt());
    s.esc();
}

#[test]
fn quick_trim_does_not_cut_at_dimensions() {
    let s = grid();
    // a dimension whose extension lines cross the line to trim
    s.cmd("DIMLINEAR").click(6., -3.).click(6., 3.).click(6.5, 0.);
    s.cmd("TRIM").click(6., 0.).enter();
    let horiz: Vec<(Pt, Pt)> = lines(&s).into_iter().filter(|(a, b)| a.y == 0. && b.y == 0.).collect();
    assert_eq!(horiz.len(), 2, "cut at the two lines, not at the dimension: {horiz:?}");
    assert!(horiz.iter().any(|(_, b)| (b.x - 4.).abs() < 1e-9) && horiz.iter().any(|(a, _)| (a.x - 8.).abs() < 1e-9), "{horiz:?}");
}
