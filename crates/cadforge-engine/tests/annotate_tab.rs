//! Commands of the Annotate tab added to match AutoCAD's: CENTERMARK, CENTERLINE, AIMLEADEREDITADD,
//! AIMLEADEREDITREMOVE and MLEADERALIGN.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn lines(s: &Sim) -> Vec<Entity> {
    s.ents().into_iter().filter(|e| matches!(e.kind, Kind::Line { .. })).collect()
}

#[test]
fn centermark_draws_two_grouped_center_lines() {
    let s = Sim::new();
    s.add(Entity::circle(pt(0., 0.), 10.));
    s.cmd("CENTERMARK").click(10., 0.).enter();
    let l = lines(&s);
    assert_eq!(l.len(), 2, "{}", s.log_text());
    assert!(l.iter().all(|e| e.ltype.as_deref() == Some("CENTER2") && e.grp.is_some() && e.grp == l[0].grp));
    assert!(s.history_has("1 center mark added."));
    let Kind::Line { a, b } = l[0].kind else { panic!() };
    assert!((dist(a, b) - 22.4).abs() < 1e-9, "the mark reaches 12% past the circle");
}

#[test]
fn centerline_runs_between_two_lines() {
    let s = Sim::new();
    s.add(Entity::line(pt(0., 0.), pt(20., 0.)));
    s.add(Entity::line(pt(20., 6.), pt(0., 6.)));
    s.cmd("CENTERLINE").click(10., 0.).click(10., 6.);
    let l = lines(&s);
    assert_eq!(l.len(), 3, "{}", s.log_text());
    let Kind::Line { a, b } = l[2].kind else { panic!() };
    assert!(dist(a, pt(-1., 3.)) < 1e-9 && dist(b, pt(21., 3.)) < 1e-9, "{a:?} {b:?}");
}

fn leader(pts: Vec<Pt>, s: &str, grp: &str) -> Entity {
    let mut e = Entity::leader(pts, s, 2.5);
    e.grp = Some(grp.into());
    e
}

#[test]
fn add_and_remove_leader_buttons() {
    let s = Sim::new();
    s.add(leader(vec![pt(0., 0.), pt(10., 10.), pt(14., 10.)], "NOTE", "*A1"));
    s.cmd("AIMLEADEREDITADD").click(12., 10.).click(20., 0.);
    let n = s.ents().iter().filter(|e| matches!(e.kind, Kind::Leader { .. })).count();
    assert_eq!(n, 2, "a leader line was added: {}", s.log_text());
    s.cmd("AIMLEADEREDITREMOVE").click(12., 10.).click(17., 5.);
    let n = s.ents().iter().filter(|e| matches!(e.kind, Kind::Leader { .. })).count();
    assert_eq!(n, 1, "{}", s.log_text());
}

#[test]
fn mleaderalign_lines_up_the_notes() {
    let s = Sim::new();
    s.add(leader(vec![pt(0., 0.), pt(10., 10.), pt(14., 10.)], "A", "*A1"));
    s.add(leader(vec![pt(5., -10.), pt(20., 0.), pt(24., 0.)], "B", "*A2"));
    s.cmd("MLEADERALIGN").click(12., 10.).click(22., 0.).enter().click(12., 10.);
    let b = s.ents().into_iter().find(|e| matches!(&e.kind, Kind::Leader { s, .. } if s == "B")).unwrap();
    let Kind::Leader { pts, .. } = b.kind else { panic!() };
    assert_eq!(pts.last().map(|p| p.x), Some(14.0), "{}", s.log_text());
    assert_eq!(pts[0], pt(5., -10.), "the arrowhead stays");
    assert!(s.history_has("1 multileader aligned."));
}
