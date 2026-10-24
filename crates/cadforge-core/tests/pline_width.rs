//! Polylines with start / end widths: the filled outline, bounds, picking and transforms.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;

fn area(poly: &[Pt]) -> f64 {
    polygon_area(poly).abs()
}

#[test]
fn a_uniform_edge_is_a_rectangle_and_a_tapered_edge_a_trapezoid() {
    let v = [
        Vertex::wide(0.0, 0.0, 0.0, 4.0, 4.0),
        Vertex::new(10.0, 0.0, 0.0),
    ];
    let o = pline_outline(&v, false);
    assert_eq!(o.len(), 1);
    assert!((area(&o[0]) - 40.0).abs() < 1e-9, "{}", area(&o[0]));
    let v = [
        Vertex::wide(0.0, 0.0, 0.0, 2.0, 6.0),
        Vertex::new(10.0, 0.0, 0.0),
    ];
    let o = pline_outline(&v, false);
    assert!(
        (area(&o[0]) - 40.0).abs() < 1e-9,
        "(2+6)/2 * 10 = 40, got {}",
        area(&o[0])
    );
    // the far end is 6 wide, centred on the line
    let ys: Vec<f64> = o[0]
        .iter()
        .filter(|p| (p.x - 10.0).abs() < 1e-9)
        .map(|p| p.y)
        .collect();
    assert!(
        ys.iter().any(|y| (y - 3.0).abs() < 1e-9) && ys.iter().any(|y| (y + 3.0).abs() < 1e-9),
        "{ys:?}"
    );
}

#[test]
fn polylines_without_widths_have_no_outline() {
    let v = [
        Vertex::new(0.0, 0.0, 0.0),
        Vertex::new(10.0, 0.0, 0.0),
        Vertex::new(10.0, 5.0, 0.0),
    ];
    assert!(!pline_is_wide(&v));
    assert!(pline_outline(&v, false).is_empty());
}

#[test]
fn an_arc_edge_gives_a_ring_sector() {
    // a half circle of radius 5 (bulge 1), 2 wide: area = pi * ((5+1)^2 - (5-1)^2) / 2 = 10 pi
    let v = [
        Vertex::wide(0.0, 0.0, 1.0, 2.0, 2.0),
        Vertex::new(10.0, 0.0, 0.0),
    ];
    let a = area(&pline_outline(&v, false)[0]);
    assert!((a - 10.0 * std::f64::consts::PI).abs() < 0.2, "{a}");
}

#[test]
fn corners_between_wide_edges_get_a_bevel_and_closed_polylines_join_up() {
    let v = [
        Vertex::wide(0.0, 0.0, 0.0, 2.0, 2.0),
        Vertex::wide(10.0, 0.0, 0.0, 2.0, 2.0),
        Vertex::wide(10.0, 10.0, 0.0, 2.0, 2.0),
        Vertex::wide(0.0, 10.0, 0.0, 2.0, 2.0),
    ];
    // open: 3 edges + 2 corners x 2 triangles; closed: 4 edges + 4 corners x 2 triangles
    assert_eq!(pline_outline(&v, false).len(), 3 + 4);
    assert_eq!(pline_outline(&v, true).len(), 4 + 8);
    // a narrow edge next to a wide one gets no corner triangle
    let mixed = [
        Vertex::wide(0.0, 0.0, 0.0, 2.0, 2.0),
        Vertex::new(10.0, 0.0, 0.0),
        Vertex::new(10.0, 10.0, 0.0),
    ];
    assert_eq!(pline_outline(&mixed, false).len(), 1);
}

#[test]
fn bounds_and_picking_include_the_width() {
    let mut d = Drawing::new("t");
    let id = d.add(Entity::pline(
        vec![
            Vertex::wide(0.0, 0.0, 0.0, 6.0, 6.0),
            Vertex::new(10.0, 0.0, 0.0),
        ],
        false,
    ));
    let e = d.by_id(id).unwrap().clone();
    let b = bounds_of(&e, &d).unwrap();
    assert!(
        (b.min_y + 3.0).abs() < 1e-9 && (b.max_y - 3.0).abs() < 1e-9,
        "{b:?}"
    );
    assert_eq!(
        pick_distance(&e, pt(5.0, 2.5), &d),
        0.0,
        "inside the drawn width"
    );
    assert!(pick_distance(&e, pt(5.0, 3.5), &d) > 0.4, "outside it");
}

#[test]
fn scaling_scales_the_widths_and_mirroring_keeps_them() {
    let mut e = Entity::pline(
        vec![
            Vertex::wide(0.0, 0.0, 0.0, 2.0, 4.0),
            Vertex::new(10.0, 0.0, 0.0),
        ],
        false,
    );
    transform_entity(&mut e, &mat::scale(2.0, 2.0, pt(0.0, 0.0)));
    let Kind::Pline { v, .. } = &e.kind else {
        panic!()
    };
    assert_eq!((v[0].sw, v[0].ew, v[1].x), (4.0, 8.0, 20.0));
    transform_entity(&mut e, &mat::mirror(pt(0.0, 0.0), pt(1.0, 0.0)));
    let Kind::Pline { v, .. } = &e.kind else {
        panic!()
    };
    assert_eq!((v[0].sw, v[0].ew), (4.0, 8.0));
}

#[test]
fn widths_stay_out_of_the_json_when_zero_and_round_trip_when_set() {
    let plain = serde_json::to_string(&Vertex::new(1.0, 2.0, 0.0)).unwrap();
    assert!(!plain.contains("sw") && !plain.contains("ew"), "{plain}");
    let wide = serde_json::to_string(&Vertex::wide(1.0, 2.0, 0.0, 3.0, 4.0)).unwrap();
    assert_eq!(
        serde_json::from_str::<Vertex>(&wide).unwrap(),
        Vertex::wide(1.0, 2.0, 0.0, 3.0, 4.0)
    );
    // drawings saved before widths existed still load
    assert_eq!(
        serde_json::from_str::<Vertex>(r#"{"x":1,"y":2,"b":0.5}"#).unwrap(),
        Vertex::new(1.0, 2.0, 0.5)
    );
}
