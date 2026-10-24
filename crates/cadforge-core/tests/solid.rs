//! Port of `tests/unit/solid.test.js`.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::Entity;
use cadforge_core::geom::*;
use cadforge_core::solid::boolean::*;
use cadforge_core::solid::mesh::*;
use cadforge_core::solid::Mesh;

fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "expected {b}, got {a}");
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> Loop {
    vec![pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)]
}
fn rev(mut l: Loop) -> Loop {
    l.reverse();
    l
}
fn tri_area(flat: &[f64], t: &[usize]) -> f64 {
    t.chunks(3)
        .map(|c| {
            ((flat[c[1] * 2] - flat[c[0] * 2]) * (flat[c[2] * 2 + 1] - flat[c[0] * 2 + 1])
                - (flat[c[1] * 2 + 1] - flat[c[0] * 2 + 1]) * (flat[c[2] * 2] - flat[c[0] * 2]))
                .abs()
                / 2.0
        })
        .sum()
}
fn vol(s: &cadforge_core::solid::Solid) -> f64 {
    mesh_volume(&s.mesh())
}
fn wt(s: &cadforge_core::solid::Solid) -> bool {
    is_watertight(&s.mesh())
}

#[test]
fn earcut_triangulates_convex_concave_and_holed_polygons_with_exact_area() {
    let l = [0., 0., 10., 0., 10., 5., 5., 5., 5., 10., 0., 10.];
    let t = earcutr::earcut(&l, &[], 2).unwrap();
    assert_eq!(t.len() / 3, 4);
    near(tri_area(&l, &t), 75.0, 1e-6);
    let plate = [
        0., 0., 100., 0., 100., 50., 0., 50., 20., 20., 30., 20., 30., 30., 20., 30., 60., 10.,
        80., 10., 80., 40., 60., 40.,
    ];
    let t2 = earcutr::earcut(&plate, &[4, 8], 2).unwrap();
    near(tri_area(&plate, &t2), 5000. - 100. - 600., 1e-6);
}

#[test]
fn extrusion_of_a_rectangle_is_watertight_with_exact_volume() {
    let m = extrude_loops(&[rect(0., 0., 10., 20.)], 30., 0., "Extrusion", 0.).unwrap();
    assert_eq!((m.vertices.len(), m.faces.len()), (8, 12));
    assert!(wt(&m));
    near(vol(&m), 6000., 1e-6);
    let down = extrude_loops(&[rect(0., 0., 10., 20.)], -30., 5., "Extrusion", 0.).unwrap();
    let b = mesh_bounds(&down.vertices);
    near(b.min.z, -25., 1e-9);
    near(b.max.z, 5., 1e-9);
    near(vol(&down), 6000., 1e-6);
    assert!(extrude_loops(&[rect(0., 0., 1., 1.)], 0., 0., "x", 0.).is_err());
}

#[test]
fn extrusion_handles_concave_outlines_and_holes_even_odd() {
    let concave = vec![
        pt(0., 0.),
        pt(10., 0.),
        pt(10., 5.),
        pt(5., 5.),
        pt(5., 10.),
        pt(0., 10.),
    ];
    let c = extrude_loops(&[concave], 3., 0., "c", 0.).unwrap();
    assert!(wt(&c));
    near(vol(&c), 225., 1e-6);
    let hole = extrude_loops(
        &[
            rect(0., 0., 100., 50.),
            rev(rect(20., 20., 10., 10.)),
            rect(60., 10., 20., 30.),
        ],
        4.,
        0.,
        "h",
        0.,
    )
    .unwrap();
    assert!(wt(&hole));
    near(vol(&hole), (5000. - 100. - 600.) * 4., 1e-6);
    let nested = extrude_loops(
        &[
            rect(0., 0., 100., 100.),
            rect(10., 10., 80., 80.),
            rect(40., 40., 20., 20.),
        ],
        1.,
        0.,
        "n",
        0.,
    )
    .unwrap(); // plate, hole, island inside the hole
    assert!(wt(&nested));
    near(vol(&nested), 10000. - 6400. + 400., 1e-6);
}

#[test]
fn taper_narrows_the_solid() {
    let m = extrude_loops(&[rect(0., 0., 10., 10.)], 10., 0., "t", 5.).unwrap();
    assert!(vol(&m) < 1000. && vol(&m) > 500.);
    assert!(wt(&m));
}

#[test]
fn profile_loops_closed_entities_circles_and_chains() {
    let mut d = Drawing::new("t");
    let pl = d.add(Entity::pline_pts(&rect(0., 0., 10., 10.), true));
    let c = d.add(Entity::circle(pt(50., 50.), 5.));
    let get = |d: &Drawing, ids: &[u64]| {
        ids.iter()
            .map(|i| d.by_id(*i).unwrap().clone())
            .collect::<Vec<_>>()
    };
    let ents = get(&d, &[pl, c]);
    let r1 = profile_loops(&ents.iter().collect::<Vec<_>>(), &d, 0.02);
    assert_eq!(r1.loops.len(), 2);
    let corners = rect(100., 0., 20., 10.);
    let lines: Vec<u64> = (0..4)
        .map(|i| d.add(Entity::line(corners[i], corners[(i + 1) % 4])))
        .collect();
    let mut le = get(&d, &lines);
    le.reverse();
    let r2 = profile_loops(&le.iter().collect::<Vec<_>>(), &d, 0.02);
    assert!(r2.error.is_none());
    assert_eq!(r2.loops.len(), 1);
    near(
        vol(&extrude_loops(&r2.loops, 1., 0., "x", 0.).unwrap()).abs(),
        200.,
        1e-6,
    );
    let open = get(&d, &lines[..3]);
    assert!(profile_loops(&open.iter().collect::<Vec<_>>(), &d, 0.02)
        .error
        .unwrap()
        .contains("closed"));
    let stray = d.add(Entity::line(pt(0., 0.), pt(1., 1.)));
    let bad = get(&d, &[stray]);
    assert!(profile_loops(&bad.iter().collect::<Vec<_>>(), &d, 0.02)
        .error
        .is_some());
    let circ = get(&d, &[c]);
    let cl = profile_loops(&circ.iter().collect::<Vec<_>>(), &d, 0.02).loops;
    near(
        vol(&extrude_loops(&cl, 10., 0., "c", 0.).unwrap()),
        PI * 25. * 10.,
        5.,
    );
}

#[test]
fn primitives_have_the_right_volume_and_are_watertight() {
    let b = box_mesh(0., 0., 0., 10., 20., 30., "Box");
    near(vol(&b), 6000., 1e-6);
    assert!(wt(&b));
    let w = wedge_mesh(0., 0., 0., 10., 20., 30., "Wedge");
    near(vol(&w), 3000., 1e-6);
    assert!(wt(&w));
    let cy = cylinder_mesh(0., 0., 0., 5., 10., 96, "C");
    near(vol(&cy), PI * 25. * 10., 2.5);
    assert!(wt(&cy));
    let cone = cone_mesh(0., 0., 0., 6., 0., 12., 96, "Cone");
    near(vol(&cone), PI * 36. * 12. / 3., 4.);
    assert!(wt(&cone));
    let fr = cone_mesh(0., 0., 0., 6., 3., 12., 96, "F");
    near(vol(&fr), PI * 12. / 3. * (36. + 18. + 9.), 5.);
    assert!(wt(&fr));
    let sph = sphere_mesh(1., 2., 3., 10., 64, "S");
    near(vol(&sph) / (4. / 3. * PI * 1000.), 1., 0.012);
    assert!(wt(&sph));
    let c = mesh_centroid(&b.mesh());
    near(c.x, 5., 1e-9);
    near(c.y, 10., 1e-9);
    near(c.z, 15., 1e-9);
    near(mesh_area(&b.mesh()), 2. * (200. + 300. + 600.), 1e-9);
}

#[test]
fn transforms_keep_solids_outward_facing_including_mirrors() {
    let b = box_mesh(0., 0., 0., 10., 20., 30., "Box");
    let mv = transform_mesh(&b, &m3::translate(5., 5., 5.));
    near(mesh_bounds(&mv.vertices).min.z, 5., 1e-9);
    near(vol(&mv), 6000., 1e-6);
    let rot = transform_mesh(&b, &m3::rotate_z(PI / 2., pt(0., 0.)));
    near(mesh_bounds(&rot.vertices).max.x, 0., 1e-9);
    near(mesh_bounds(&rot.vertices).min.x, -20., 1e-9);
    near(vol(&rot), 6000., 1e-6);
    let mir = transform_mesh(&b, &m3::mirror_xy(pt(0., 0.), pt(0., 1.)));
    near(mesh_bounds(&mir.vertices).max.x, 0., 1e-9);
    near(vol(&mir), 6000., 1e-6);
    assert!(wt(&mir));
    let sc = transform_mesh(
        &b,
        &m3::scale(2., cadforge_core::solid::V3::new(0., 0., 0.)),
    );
    near(vol(&sc), 48000., 1e-6);
}

#[test]
fn obj_and_stl_export() {
    let obj = to_obj(&[&box_mesh(0., 0., 0., 1., 1., 1., "My box")]);
    assert!(obj.lines().any(|l| l == "o My_box"));
    assert_eq!(obj.lines().filter(|l| l.starts_with("v ")).count(), 8);
    assert_eq!(obj.lines().filter(|l| l.starts_with("f ")).count(), 12);
    let stl = to_stl(&[&box_mesh(0., 0., 0., 1., 1., 1., "b")]);
    assert_eq!(stl.matches("facet normal").count(), 12);
    assert!(stl.contains("endsolid"));
}

fn bx(x: f64, y: f64, z: f64, w: f64, d: f64, h: f64) -> Mesh {
    box_mesh(x, y, z, w, d, h, "b").mesh()
}

#[test]
fn boolean_volumes_of_overlapping_boxes() {
    let (a, b) = (bx(0., 0., 0., 10., 10., 10.), bx(5., 0., 0., 10., 10., 10.));
    for (op, v) in [
        (BoolOp::Union, 1500.),
        (BoolOp::Subtract, 500.),
        (BoolOp::Intersect, 500.),
    ] {
        near(mesh_volume(&boolean_sync(&a, &b, op).unwrap()), v, 1e-5);
    }
    assert_eq!(
        boolean_sync(&a, &a, BoolOp::Subtract).unwrap().faces.len(),
        0
    );
    let far = bx(105., 0., 0., 10., 10., 10.);
    assert_eq!(
        boolean_sync(&a, &far, BoolOp::Intersect)
            .unwrap()
            .faces
            .len(),
        0
    );
    near(
        mesh_volume(&boolean_sync(&a, &far, BoolOp::Union).unwrap()),
        2000.,
        1e-5,
    );
}

#[test]
fn drilling_a_cylindrical_hole_through_a_plate() {
    let plate = bx(-20., -20., 0., 40., 40., 10.);
    let drill = cylinder_mesh(0., 0., -5., 6., 20., 48, "d").mesh();
    let r = boolean_sync(&plate, &drill, BoolOp::Subtract).unwrap();
    let cyl = mesh_volume(&cylinder_mesh(0., 0., 0., 6., 10., 48, "c").mesh());
    near(mesh_volume(&r), 16000. - cyl, 1e-3); // BSP output may contain T-junctions, so only the volume is checked
}

#[test]
fn sphere_intersect_box() {
    let b = bx(-10., -10., -10., 20., 20., 20.);
    let s = sphere_mesh(0., 0., 0., 12., 32, "s").mesh();
    let r = boolean_sync(&b, &s, BoolOp::Intersect).unwrap();
    let v = mesh_volume(&r);
    assert!(v > 0. && v < 8000.);
    assert!(BoolOp::parse("bogus").is_none());
}
