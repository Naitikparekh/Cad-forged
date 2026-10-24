//! Port of `tests/unit/ops.test.js`.
use approx::assert_relative_eq;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::boundary::find_region;
use cadforge_core::ops::fillet::*;
use cadforge_core::ops::offset::offset_entity;
use cadforge_core::ops::trim::*;

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-6, "{a} != {b}");
}
fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
    Entity::line(pt(a.0, a.1), pt(b.0, b.1))
}
fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}
fn ends(e: &Entity) -> (Pt, Pt) {
    match e.kind {
        Kind::Line { a, b } => (a, b),
        _ => panic!("not a line: {e:?}"),
    }
}
fn doc_with(list: Vec<Entity>) -> (Drawing, Vec<Entity>) {
    let mut d = Drawing::new("t");
    let es = list
        .into_iter()
        .map(|e| {
            let id = d.add(e);
            d.by_id(id).unwrap().clone()
        })
        .collect();
    (d, es)
}
fn pts(v: &[(f64, f64)]) -> Vec<Pt> {
    v.iter().map(|&(x, y)| pt(x, y)).collect()
}

#[test]
fn trim_a_line_between_two_cutters_keeps_both_outer_pieces() {
    let (d, e) = doc_with(vec![line((0., 0.), (10., 0.)), line((3., -2.), (3., 2.)), line((7., -2.), (7., 2.))]);
    let r = trim(&e[0], p(5., 0.), &[&e[1], &e[2]], &d, false).unwrap();
    assert_eq!(r.len(), 2);
    near(ends(&r[0]).1.x, 3.);
    near(ends(&r[1]).0.x, 7.);
}

#[test]
fn trim_the_end_of_a_line_past_the_last_cutter() {
    let (d, e) = doc_with(vec![line((0., 0.), (10., 0.)), line((4., -2.), (4., 2.))]);
    let r = trim(&e[0], p(8., 0.), &[&e[1]], &d, false).unwrap();
    assert_eq!(r.len(), 1);
    near(ends(&r[0]).0.x, 0.);
    near(ends(&r[0]).1.x, 4.);
}

#[test]
fn trim_errors_with_no_intersecting_edge() {
    let (d, e) = doc_with(vec![line((0., 0.), (10., 0.)), line((20., -2.), (20., 2.))]);
    assert!(trim(&e[0], p(5., 0.), &[&e[1]], &d, false).is_err());
}

#[test]
fn trim_a_circle_between_two_lines_leaves_an_arc() {
    let (d, e) = doc_with(vec![Entity::circle(p(0., 0.), 5.), line((-10., 0.), (10., 0.)), line((0., -10.), (0., 10.))]);
    let r = trim(&e[0], p(3.5, 3.5), &[&e[1], &e[2]], &d, false).unwrap();
    assert_eq!(r.len(), 1);
    match r[0].kind {
        Kind::Arc { a0, a1, .. } => near(sweep_ccw(a0, a1), 1.5 * PI),
        _ => panic!("expected an arc"),
    }
}

#[test]
fn trim_a_polyline_segment_splits_it() {
    let (d, e) = doc_with(vec![Entity::pline_pts(&pts(&[(0., 0.), (10., 0.), (10., 10.)]), false), line((5., -1.), (5., 1.))]);
    let r = trim(&e[0], p(2., 0.), &[&e[1]], &d, false).unwrap();
    assert_eq!(r.len(), 1);
    match &r[0].kind {
        Kind::Pline { v, .. } => {
            near(v[0].x, 5.);
            assert_eq!(v.len(), 3);
        }
        _ => panic!("expected a polyline"),
    }
}

#[test]
fn extend_a_line_to_a_boundary() {
    let (d, e) = doc_with(vec![line((0., 0.), (4., 0.)), line((10., -5.), (10., 5.)), line((-6., -5.), (-6., 5.))]);
    let r = extend(&e[0], p(3.5, 0.), &[&e[1]], &d, false).unwrap();
    near(ends(&r[0]).1.x, 10.);
    near(ends(&r[0]).0.x, 0.);
    let r2 = extend(&e[0], p(0.2, 0.), &[&e[2]], &d, false).unwrap();
    near(ends(&r2[0]).0.x, -6.);
    near(ends(&r2[0]).1.x, 4.);
}

#[test]
fn extend_an_arc_to_a_line() {
    let (d, e) = doc_with(vec![Entity::arc(p(0., 0.), 5., 0., PI / 2.), line((-10., 3.), (10., 3.))]);
    let r = extend(&e[0], p(0., 5.), &[&e[1]], &d, false).unwrap();
    match r[0].kind {
        Kind::Arc { c, r, a1, .. } => {
            let end = polar(c, a1, r);
            near(end.x, -4.);
            near(end.y, 3.);
        }
        _ => panic!("expected an arc"),
    }
}

#[test]
fn break_a_line_removes_the_middle() {
    let (d, e) = doc_with(vec![line((0., 0.), (10., 0.))]);
    let r = break_at(&e[0], p(3., 0.), p(6., 0.), &d).unwrap();
    assert_eq!(r.len(), 2);
    near(ends(&r[0]).1.x, 3.);
    near(ends(&r[1]).0.x, 6.);
    let s = split_at(&e[0], p(4., 0.), &d).unwrap();
    assert_eq!(s.len(), 2);
    near(ends(&s[0]).1.x, 4.);
    near(ends(&s[1]).0.x, 4.);
}

#[test]
fn join_merges_connected_lines_into_a_polyline_and_collinear_lines_into_one_line() {
    let d = Drawing::new("t");
    let (a, b, c) = (line((0., 0.), (5., 0.)), line((5., 0.), (5., 5.)), line((5., 5.), (0., 5.)));
    let r = join(&[&a, &b, &c], &d, 1e-6);
    assert_eq!(r.results.len(), 1);
    match &r.results[0].kind {
        Kind::Pline { v, .. } => assert_eq!(v.len(), 4),
        k => panic!("expected pline, got {k:?}"),
    }
    let (l1, l2) = (line((0., 0.), (5., 0.)), line((5., 0.), (9., 0.)));
    let r2 = join(&[&l1, &l2], &d, 1e-6);
    near(ends(&r2.results[0]).1.x, 9.);
    let (m1, m2) = (line((0., 0.), (1., 0.)), line((50., 50.), (51., 50.)));
    assert_eq!(join(&[&m1, &m2], &d, 1e-6).results.len(), 2);
}

#[test]
fn lengthen_a_line_by_delta_from_the_picked_end() {
    let d = Drawing::new("t");
    let l = line((0., 0.), (10., 0.));
    near(ends(&lengthen(&l, p(9., 0.), LengthenMode::Delta, 5., &d).unwrap()[0]).1.x, 15.);
    let r2 = lengthen(&l, p(1., 0.), LengthenMode::Total, 4., &d).unwrap();
    near(ends(&r2[0]).0.x, 6.);
    near(ends(&r2[0]).1.x, 10.);
}

#[test]
fn offset_line_circle_arc_and_rectangle() {
    let d = Drawing::new("t");
    let l = line((0., 0.), (10., 0.));
    let o = ends(&offset_entity(&l, 2., p(5., 5.), &d).unwrap().entity);
    near(o.0.y, 2.);
    near(o.1.y, 2.);
    near(ends(&offset_entity(&l, 2., p(5., -5.), &d).unwrap().entity).0.y, -2.);
    let c = Entity::circle(p(0., 0.), 5.);
    let rad = |r: Result<cadforge_core::ops::offset::OffsetResult, String>| match r.unwrap().entity.kind {
        Kind::Circle { r, .. } => r,
        _ => panic!(),
    };
    near(rad(offset_entity(&c, 1., p(20., 0.), &d)), 6.);
    near(rad(offset_entity(&c, 1., p(1., 0.), &d)), 4.);
    assert!(offset_entity(&c, 6., p(1., 0.), &d).is_err());
    let rect = Entity::pline_pts(&pts(&[(0., 0.), (10., 0.), (10., 6.), (0., 6.)]), true);
    let verts = |r: Result<cadforge_core::ops::offset::OffsetResult, String>| match r.unwrap().entity.kind {
        Kind::Pline { v, .. } => v,
        _ => panic!(),
    };
    let inner = verts(offset_entity(&rect, 1., p(5., 3.), &d));
    assert_eq!(inner.len(), 4);
    near(inner[0].x, 1.);
    near(inner[0].y, 1.);
    near(inner[2].x, 9.);
    near(inner[2].y, 5.);
    let outer = verts(offset_entity(&rect, 1., p(50., 50.), &d));
    near(outer[0].x, -1.);
    near(outer[0].y, -1.);
    near(outer[2].x, 11.);
    near(outer[2].y, 7.);
    match offset_entity(&rect, 4., p(5., 3.), &d) {
        Err(_) => {}
        Ok(r) => match r.entity.kind {
            Kind::Pline { v, .. } => assert!(pline_area(&v).abs() < 1.0),
            _ => panic!(),
        },
    }
}

#[test]
fn offset_a_polyline_containing_an_arc() {
    let (d, _) = doc_with(vec![]);
    let pl = Entity::pline(vec![Vertex::new(0., 0., 0.), Vertex::new(10., 0., 1.), Vertex::new(10., 10., 0.)], false);
    let r = offset_entity(&pl, 1., p(20., 5.), &d).unwrap().entity; // outside of the bulge
    let segs = segs_of(&r, &d);
    assert_eq!(segs.len(), 2);
    match segs[1] {
        Seg::A { r, .. } => near(r, 6.),
        s => panic!("expected an arc, got {s:?}"),
    }
}

#[test]
fn fillet_two_perpendicular_lines_with_a_radius() {
    let (sa, sb) = (Seg::line(p(0., 0.), p(10., 0.)), Seg::line(p(0., 0.), p(0., 10.)));
    let r = fillet_segs(&sa, p(8., 0.), &sb, p(0., 8.), 2.).unwrap();
    near(r.a.unwrap().start().x, 2.);
    near(r.a.unwrap().end().x, 10.);
    near(r.b.unwrap().start().y, 2.);
    match r.insert.unwrap() {
        Seg::A { c, r, sw, .. } => {
            near(r, 2.);
            near(c.x, 2.);
            near(c.y, 2.);
            near(sw.abs(), PI / 2.);
        }
        _ => panic!("expected an arc"),
    }
    let z = fillet_segs(&Seg::line(p(0., 0.), p(4., 0.)), p(3., 0.), &Seg::line(p(10., -5.), p(10., 5.)), p(10., 4.), 0.).unwrap();
    near(z.a.unwrap().end().x, 10.);
    near(z.b.unwrap().start().y, 0.); // radius 0: extend both to the corner
}

#[test]
fn fillet_and_chamfer_on_polylines_all_vertices() {
    let d = Drawing::new("t");
    let rect = Entity::pline_pts(&pts(&[(0., 0.), (10., 0.), (10., 10.), (0., 10.)]), true);
    let area = |e: &Entity| match &e.kind {
        Kind::Pline { v, .. } => pline_area(v),
        _ => panic!(),
    };
    let f = fillet_pline(&rect, 2., &d).unwrap();
    assert_eq!(f.applied, 4);
    match &f.entity.as_ref().unwrap().kind {
        Kind::Pline { v, .. } => assert_eq!(v.len(), 8),
        _ => panic!(),
    }
    assert_relative_eq!(area(f.entity.as_ref().unwrap()).abs(), 100. - (4. - PI) * 4., epsilon = 0.01);
    let c = chamfer_pline(&rect, 2., 2., &d).unwrap();
    assert_eq!(c.applied, 4);
    assert_relative_eq!(area(c.entity.as_ref().unwrap()).abs(), 100. - 4. * 2., epsilon = 1e-6);
}

#[test]
fn chamfer_two_lines() {
    let r = chamfer_segs(&Seg::line(p(0., 0.), p(10., 0.)), p(8., 0.), &Seg::line(p(0., 0.), p(0., 10.)), p(0., 8.), 2., 3.).unwrap();
    near(r.a.unwrap().start().x, 2.);
    near(r.b.unwrap().start().y, 3.);
    near(r.insert.unwrap().start().x, 2.);
    near(r.insert.unwrap().end().y, 3.);
}

fn area_of(l: &[Vertex]) -> f64 {
    pline_area(l).abs()
}

#[test]
fn boundary_detection_rectangle_with_an_island_circle_and_a_dangling_line() {
    let (d, _) = doc_with(vec![Entity::pline_pts(&pts(&[(0., 0.), (20., 0.), (20., 20.), (0., 20.)]), true), Entity::circle(p(10., 10.), 4.), line((20., 10.), (30., 10.))]);
    let r = find_region(&d, p(2., 2.), None).unwrap();
    assert_eq!(r.loops.len(), 2);
    let areas: Vec<f64> = r.loops.iter().map(|l| area_of(l)).collect();
    assert_relative_eq!(areas.iter().cloned().fold(0.0, f64::max), 400., epsilon = 1e-3);
    assert_relative_eq!(areas.iter().cloned().fold(f64::INFINITY, f64::min), PI * 16., epsilon = 1e-3);
    let inside = find_region(&d, p(10., 10.), None).unwrap();
    assert_eq!(inside.loops.len(), 1);
    assert_relative_eq!(area_of(&inside.loops[0]), PI * 16., epsilon = 1e-3);
    assert!(find_region(&d, p(50., 50.), None).is_err());
}

#[test]
fn boundary_detection_two_adjacent_regions_split_by_a_line() {
    let (d, _) = doc_with(vec![Entity::pline_pts(&pts(&[(0., 0.), (20., 0.), (20., 10.), (0., 10.)]), true), line((10., -2.), (10., 12.))]);
    let left = find_region(&d, p(3., 5.), None).unwrap();
    let right = find_region(&d, p(17., 5.), None).unwrap();
    assert_relative_eq!(area_of(&left.loops[0]), 100., epsilon = 1e-6);
    assert_relative_eq!(area_of(&right.loops[0]), 100., epsilon = 1e-6);
}

#[test]
fn boundary_detection_with_overlapping_circles() {
    let (d, _) = doc_with(vec![Entity::circle(p(0., 0.), 5.), Entity::circle(p(6., 0.), 5.)]);
    let lens = find_region(&d, p(3., 0.), None).unwrap();
    assert_relative_eq!(area_of(&lens.loops[0]), 2. * 25. * (3f64 / 5.).acos() - 6. * 4., epsilon = 1e-3);
}

#[test]
fn offset_a_full_ellipse_produces_a_closed_outline_around_it() {
    let d = Drawing::new("t");
    let r = offset_entity(&Entity::ellipse(p(0., 0.), p(10., 0.), 0.5, 0., TAU), 1., p(30., 0.), &d).unwrap();
    assert!(r.approximated);
    match r.entity.kind {
        Kind::Pline { v, closed } => {
            assert!(closed);
            let b = Bounds::of_points(&v.iter().map(|q| q.pt()).collect::<Vec<_>>());
            assert!((b.max_x - 11.).abs() < 0.05 && (b.min_x + 11.).abs() < 0.05 && (b.max_y - 6.).abs() < 0.05);
        }
        k => panic!("expected pline, got {k:?}"),
    }
}
