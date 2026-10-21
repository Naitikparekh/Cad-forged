//! Tests for the extra 3D solid commands (`commands/ext_solids.rs`).
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::solid::ext::repair::check;
use cadforge_core::solid::mesh::*;
use cadforge_core::solid::{Solid, V3};
use cadforge_engine::sim::{Sim, Step::*};

fn near(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= rel * b.abs().max(1e-9), "expected {b}, got {a}");
}
fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}
fn vol(s: &Solid) -> f64 {
    mesh_volume(&s.mesh())
}
fn bounds(s: &Solid) -> Bounds3 {
    mesh_bounds(&s.vertices)
}

/// A simulator holding one box (corner at the origin) in shaded style, selected.
fn with_box(w: f64, d: f64, h: f64) -> Sim {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.cmd("BOX").click(0., 0.).click(w, d).ty(&h.to_string());
    let id = s.solids()[0].id;
    s.set_sel(&[id]);
    s
}

fn rect(s: &Sim, x0: f64, y0: f64, x1: f64, y1: f64) -> u64 {
    s.add(Entity::pline_pts(&[p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)], true))
}

#[test]
fn every_command_is_registered_in_its_group() {
    let s = Sim::new();
    let reg = s.registry();
    for (name, group) in [
        ("REVOLVE", "3D"),
        ("SLICE", "3D"),
        ("TORUS", "3D"),
        ("SWEEP", "3D"),
        ("LOFT", "3D"),
        ("3DROTATE", "3D"),
        ("FILLETEDGE", "3D"),
        ("CHAMFEREDGE", "3D"),
        ("SOLIDEDIT", "3D"),
        ("PYRAMID", "3D"),
        ("POLYSOLID", "3D"),
        ("SHELL", "3D"),
        ("THICKEN", "3D"),
        ("3DSCALE", "3D"),
        ("3DALIGN", "3D"),
        ("3DARRAY", "3D"),
        ("INTERFERE", "3D"),
        ("CONVTOSOLID", "3D"),
        ("MIRROR3D", "Modify"),
        ("ROTATE3D", "Modify"),
        ("SOLIDHIST", "3D"),
        ("SECTIONPLANE", "3D"),
        ("FLATSHOT", "3D"),
    ] {
        let d = reg.get(name).unwrap_or_else(|| panic!("{name} is missing"));
        assert_eq!(d.group, group, "{name}");
        assert!(!d.title.is_empty() && !d.desc.is_empty(), "{name} needs a title and description");
    }
    for (alias, name) in [("REV", "REVOLVE"), ("SL", "SLICE"), ("TOR", "TORUS"), ("PYR", "PYRAMID"), ("PSOLID", "POLYSOLID"), ("3A", "3DARRAY"), ("3DMIRROR", "MIRROR3D"), ("INF", "INTERFERE")] {
        assert_eq!(reg.find(alias).map(|d| d.name.clone()), Some(name.to_string()), "{alias}");
    }
}

// ---------------------------------------------------------------- REVOLVE / TORUS

#[test]
fn revolve_a_rectangle_about_the_y_axis_gives_a_hollow_cylinder() {
    let s = Sim::new();
    let id = rect(&s, 2., 0., 5., 10.);
    s.set_sel(&[id]);
    s.cmd("REVOLVE");
    assert!(s.prompt().starts_with("Specify axis start point or define axis by [Object/X/Y/Z] <Object>"), "{}", s.prompt());
    s.ty("Y");
    assert!(s.prompt().starts_with("Specify angle of revolution or [STart angle/Reverse] <360>"), "{}", s.prompt());
    s.enter();
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 1);
    near(vol(&s.solids()[0]), PI * (25. - 4.) * 10., 0.01);
    assert_eq!(s.solids()[0].layer, "0");
    s.cmd("U");
    assert_eq!(s.solids().len(), 0);
}

#[test]
fn revolve_by_two_points_partial_angle_start_angle_and_reverse() {
    let s = Sim::new();
    let id = rect(&s, 2., 0., 5., 10.);
    s.set_sel(&[id]);
    s.cmd("REVOLVE").click(0., 0.).click(0., 10.).ty("180");
    near(vol(&s.solids()[0]), PI * 21. * 10. / 2., 0.01);
    s.set_sel(&[id]);
    s.cmd("REVOLVE").ty("Y").ty("ST").ty("90").ty("90");
    let b = bounds(&s.solids()[1]);
    assert!(b.max.x < 1e-6 && b.max.z < 1e-6, "start angle 90 + sweep 90 lies in x<=0, z<=0: {b:?}");
    s.set_sel(&[id]);
    s.cmd("REVOLVE").ty("Y").ty("R").ty("90");
    let b = bounds(&s.solids()[2]);
    assert!(b.min.z > -1e-6, "reversed sweep goes to +z: {b:?}");
    near(vol(&s.solids()[2]), PI * 21. * 10. / 4., 0.01);
}

#[test]
fn revolve_about_an_object_axis_and_error_cases() {
    let s = Sim::new();
    s.add(Entity::line(p(0., -5.), p(0., 15.)));
    let id = rect(&s, 3., 0., 6., 4.);
    s.set_sel(&[id]);
    s.cmd("REVOLVE").enter().click(0., 5.);
    assert!(s.prompt().starts_with("Specify angle of revolution"));
    s.enter();
    near(vol(&s.solids()[0]), PI * (36. - 9.) * 4., 0.01);
    // a profile crossing the axis is rejected
    let t = Sim::new();
    let crossing = rect(&t, -1., 0., 2., 3.);
    t.set_sel(&[crossing]);
    t.cmd("REVOLVE").ty("Y").enter();
    assert!(!t.busy());
    assert!(t.solids().is_empty());
    assert!(t.history_has("crosses the axis"));
    // open outline
    let u = Sim::new();
    let l = u.add(Entity::line(p(0., 0.), p(5., 0.)));
    u.set_sel(&[l]);
    u.cmd("REVOLVE");
    assert!(u.history_has("closed"));
    // Escape leaves nothing behind
    let v = Sim::new();
    let r = rect(&v, 2., 0., 5., 10.);
    v.set_sel(&[r]);
    v.cmd("REVOLVE").esc();
    assert!(!v.busy() && v.solids().is_empty());
}

#[test]
fn revolve_a_circle_gives_a_torus_like_ring_and_a_region_works_too() {
    let s = Sim::new();
    let c = s.add(Entity::circle(p(10., 0.), 3.));
    s.set_sel(&[c]);
    s.cmd("REVOLVE").ty("Y").enter();
    near(vol(&s.solids()[0]), 2. * PI * 10. * PI * 9., 0.02);
    let r = s.add(Entity::new(Kind::Region(RegionData { loops: vec![vec![Vertex::new(2., 20., 0.), Vertex::new(4., 20., 0.), Vertex::new(4., 24., 0.), Vertex::new(2., 24., 0.)]] })));
    s.set_sel(&[r]);
    s.cmd("REVOLVE").ty("Y").enter();
    near(vol(&s.solids()[1]), PI * (16. - 4.) * 4., 0.01);
}

#[test]
fn torus_dimensions_diameter_option_and_validation() {
    let s = Sim::new();
    s.play("TORUS", &[Click(0., 0.), Type("10"), Type("2")]);
    near(vol(&s.solids()[0]), 2. * PI * PI * 10. * 4., 0.025);
    let b = bounds(&s.solids()[0]);
    near(b.max.z, 2., 0.01);
    near(b.min.z, -2., 0.01);
    near(b.max.x, 12., 0.01);
    assert!(check(&s.solids()[0].mesh()).valid());
    s.play("TORUS", &[Click(50., 0.), Type("D"), Type("20"), Type("D"), Type("4")]);
    near(vol(&s.solids()[1]), vol(&s.solids()[0]), 0.001);
    s.play("TORUS", &[Click(100., 0.), Type("5"), Type("6")]);
    assert_eq!(s.solids().len(), 2);
    assert!(s.history_has("smaller than the torus radius"));
}

// ---------------------------------------------------------------- SWEEP / LOFT / PYRAMID / POLYSOLID

#[test]
fn sweep_a_circle_along_a_line_with_scale_and_twist_options() {
    let s = Sim::new();
    let circle = s.add(Entity::circle(p(50., 10.), 1.));
    s.add(Entity::line(p(0., 0.), p(20., 0.)));
    s.set_sel(&[circle]);
    s.cmd("SWEEP");
    assert!(s.prompt().starts_with("Select sweep path or [Alignment/Base point/Scale/Twist]"), "{}", s.prompt());
    s.click(10., 0.);
    assert!(!s.busy());
    let so = s.solids();
    near(vol(&so[0]), PI * 20., 0.02);
    let b = bounds(&so[0]);
    assert!(b.min.x.abs() < 1e-6 && (b.max.x - 20.).abs() < 1e-6, "{b:?}");
    near(b.max.y, 1., 0.02);
    near(b.max.z, 1., 0.02);
    s.set_sel(&[circle]);
    s.cmd("SWEEP").ty("S").ty("2").click(10., 0.);
    let cone = vol(&s.solids()[1]);
    near(cone, PI * 20. / 3. * (1. + 4. + 2.), 0.02);
    s.set_sel(&[circle]);
    s.cmd("SWEEP").ty("A").ty("N").click(10., 0.);
    assert!(s.history_has("no volume"), "an unaligned sweep of a flat outline along a flat path has no volume");
}

#[test]
fn sweep_around_a_polyline_corner_and_along_a_closed_path() {
    let s = Sim::new();
    let sq = rect(&s, -50., -50., -48., -48.);
    let path = s.add(Entity::pline_pts(&[p(0., 0.), p(10., 0.), p(10., 10.)], false));
    s.set_sel(&[sq]);
    s.cmd("SWEEP").click(5., 0.);
    near(vol(&s.solids()[0]), 4. * 20., 0.001);
    assert!(check(&s.solids()[0].mesh()).valid());
    s.doc_mut(|d| d.remove(path));
    let ring = s.add(Entity::pline_pts(&[p(0., 0.), p(10., 0.), p(10., 10.), p(0., 10.)], true));
    s.set_sel(&[sq]);
    s.cmd("SWEEP").click(5., 0.);
    let _ = ring;
    near(vol(&s.solids()[1]), 4. * 40., 0.001);
}

#[test]
fn loft_through_two_squares_and_options() {
    let s = Sim::new();
    let a = rect(&s, 0., 0., 10., 10.);
    let b = rect(&s, 3., 3., 7., 7.);
    s.set_sel(&[a, b]);
    s.cmd("LOFT");
    assert!(s.prompt().starts_with("Enter an option or [Guides/Path/Cross-sections only/Settings] <Cross-sections only>"), "{}", s.prompt());
    s.enter();
    assert!(s.prompt().starts_with("Specify distance between cross-sections"));
    s.ty("6");
    near(vol(&s.solids()[0]), 6. / 3. * (100. + 16. + 40.), 0.001);
    assert!(check(&s.solids()[0].mesh()).valid());
    // guides are reported as unsupported, settings choose ruled lofts
    s.set_sel(&[a, b]);
    s.cmd("LOFT").ty("G");
    assert!(s.history_has("guides"));
    s.ty("S").ty("R").enter().ty("6");
    near(vol(&s.solids()[1]), vol(&s.solids()[0]), 0.001);
    // one section is not enough
    let t = Sim::new();
    let only = rect(&t, 0., 0., 4., 4.);
    t.set_sel(&[only]);
    t.cmd("LOFT");
    assert!(t.history_has("at least two"));
}

#[test]
fn loft_three_circles_smoothly() {
    let s = Sim::new();
    let ids: Vec<u64> = [(2., 0.), (4., 20.), (2., 40.)].iter().map(|(r, x)| s.add(Entity::circle(p(*x, 0.), *r))).collect();
    s.set_sel(&ids);
    s.cmd("LOFT").enter().ty("5");
    let v = vol(&s.solids()[0]);
    assert!(v > PI * 4. * 10. && v < PI * 16. * 10., "{v}");
    assert!(check(&s.solids()[0].mesh()).valid());
}

#[test]
fn pyramid_sides_edge_inscribed_and_top_radius() {
    let s = Sim::new();
    s.play("PYRAMID", &[Click(0., 0.), Type("5"), Type("9")]);
    near(vol(&s.solids()[0]), 100. * 9. / 3., 0.001);
    s.play("PYRAMID", &[Type("S"), Type("6"), Click(30., 0.), Type("5"), Type("9")]);
    near(vol(&s.solids()[1]), 6. * 25. * (PI / 6.).tan() * 9. / 3., 0.001);
    s.play("PYRAMID", &[Type("E"), Click(60., 0.), Click(70., 0.), Type("6")]);
    near(vol(&s.solids()[2]), 100. * 6. / 3., 0.001);
    let b = bounds(&s.solids()[2]);
    near(b.min.x, 60., 1e-6);
    assert!(b.max.y > 9.99, "the base lies to the left of the edge direction: {b:?}");
    s.play("PYRAMID", &[Click(100., 0.), Type("I"), Type("5"), Type("9")]);
    near(vol(&s.solids()[3]), 50. * 9. / 3., 0.001);
    s.play("PYRAMID", &[Click(130., 0.), Type("5"), Type("T"), Type("2"), Type("9")]);
    let (a1, a2): (f64, f64) = (100., 16.);
    near(vol(&s.solids()[4]), 9. / 3. * (a1 + a2 + (a1 * a2).sqrt()), 0.001);
    s.play("PYRAMID", &[Click(160., 0.), Type("5"), Type("-9")]);
    assert!(bounds(&s.solids()[5]).max.z < 1e-9, "a negative height builds downwards");
}

#[test]
fn polysolid_path_options_and_object_conversion() {
    let s = Sim::new();
    s.play("POLYSOLID", &[Click(0., 0.), Click(10., 0.), Click(10., 10.), Enter]);
    near(vol(&s.solids()[0]), 0.25 * 4. * 20., 0.001);
    let b = bounds(&s.solids()[0]);
    near(b.max.z, 4., 1e-6);
    s.play("POLYSOLID", &[Type("H"), Type("2"), Type("W"), Type("1"), Click(30., 0.), Click(50., 0.), Enter]);
    near(vol(&s.solids()[1]), 40., 0.001);
    // right justification keeps the wall on the right-hand side of the path
    s.play("POLYSOLID", &[Type("J"), Type("R"), Click(0., 30.), Click(10., 30.), Enter]);
    let b = bounds(&s.solids()[2]);
    assert!(b.min.y > 29.999 && b.max.y > 30.2, "{b:?}");
    // closed path and object conversion
    s.play("POLYSOLID", &[Click(0., 50.), Click(10., 50.), Click(10., 60.), Click(0., 60.), Type("C")]);
    near(vol(&s.solids()[3]), 0.25 * 4. * 40., 0.001);
    s.add(Entity::circle(p(100., 100.), 5.));
    s.play("POLYSOLID", &[Enter, Click(105., 100.)]);
    assert_eq!(s.solids().len(), 5);
    near(vol(&s.solids()[4]), 0.25 * 4. * 2. * PI * 5., 0.03);
    // an arc segment after a line
    s.play("POLYSOLID", &[Click(0., 80.), Click(10., 80.), Type("A"), Click(20., 90.), Enter]);
    assert_eq!(s.solids().len(), 6);
    assert!(check(&s.solids()[5].mesh()).valid());
}

// ---------------------------------------------------------------- SLICE / SECTIONPLANE

#[test]
fn slice_with_the_xy_plane_keeps_both_halves() {
    let s = with_box(10., 10., 10.);
    s.cmd("SLICE");
    assert!(s.prompt().starts_with("Specify first point on slicing plane or define plane by [Object/Zaxis/View/XY/YZ/ZX/3points] <3points>"), "{}", s.prompt());
    s.ty("XY").click(0., 0.).ty("4");
    assert!(s.prompt().starts_with("Specify a point on desired side or [keep Both sides"), "{}", s.prompt());
    s.ty("B");
    assert!(!s.busy());
    let so = s.solids();
    assert_eq!(so.len(), 2);
    let mut v: Vec<f64> = so.iter().map(vol).collect();
    v.sort_by(f64::total_cmp);
    near(v[0], 400., 1e-6);
    near(v[1], 600., 1e-6);
    assert!(so.iter().all(|x| check(&x.mesh()).valid() && x.layer == "0"));
    s.cmd("U");
    assert_eq!(s.solids().len(), 1);
    near(vol(&s.solids()[0]), 1000., 1e-6);
}

#[test]
fn slice_keeps_the_picked_side_or_the_chosen_sign() {
    let s = with_box(10., 10., 10.);
    s.cmd("SLICE").ty("YZ").click(5., 0.).click(7., 3.);
    assert_eq!(s.solids().len(), 1);
    near(vol(&s.solids()[0]), 500., 1e-6);
    assert!(bounds(&s.solids()[0]).min.x > 4.999);
    let t = with_box(10., 10., 10.);
    t.cmd("SLICE").ty("ZX").click(0., 4.).ty("N");
    assert_eq!(t.solids().len(), 1);
    near(vol(&t.solids()[0]), 400., 1e-6);
    let u = with_box(10., 10., 10.);
    u.cmd("SLICE").ty("XY").click(0., 0.).ty("4").ty("P");
    near(vol(&u.solids()[0]), 600., 1e-6);
    // a point on the plane cannot say which side to keep
    let v = with_box(10., 10., 10.);
    v.cmd("SLICE").ty("YZ").click(5., 0.).click(5., 2.);
    assert_eq!(v.solids().len(), 1);
    assert!(v.history_has("lies on the slicing plane"));
}

#[test]
fn slice_by_z_axis_three_points_and_planes_that_miss() {
    let s = with_box(10., 10., 10.);
    s.cmd("SLICE").ty("Z").click(0., 5.).click(0., 10.).ty("B");
    assert_eq!(s.solids().len(), 2);
    near(s.solids().iter().map(vol).sum::<f64>(), 1000., 1e-6);
    // three points on the drawing plane make the plane z = 0, which only touches the box
    let t = with_box(10., 10., 10.);
    t.cmd("SLICE").click(0., 0.).click(5., 0.).click(5., 5.).ty("B");
    assert_eq!(t.solids().len(), 1);
    assert!(t.history_has("does not cut"));
    let u = with_box(10., 10., 10.);
    u.cmd("SLICE").click(0., 0.).click(5., 0.).click(10., 0.);
    assert!(u.history_has("do not define a plane"));
}

#[test]
fn slice_acts_on_every_selected_solid() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.play("BOX", &[Click(20., 0.), Click(30., 10.), Type("10")]);
    let ids: Vec<u64> = s.solids().iter().map(|x| x.id).collect();
    s.set_sel(&ids);
    s.cmd("SL").ty("XY").click(0., 0.).ty("5").ty("B");
    assert_eq!(s.solids().len(), 4);
}

#[test]
fn sectionplane_draws_the_cut_outline_flat() {
    let s = with_box(10., 6., 10.);
    s.cmd("SECTIONPLANE").ty("XY").click(0., 0.).ty("5").click(50., 50.);
    assert!(!s.busy());
    let ents = s.ents();
    assert_eq!(ents.len(), 2);
    assert!(ents.iter().all(|e| e.layer == "SECTION"));
    let Kind::Pline { v, closed } = &ents[0].kind else { panic!("expected a polyline") };
    assert!(*closed);
    let pts: Vec<Pt> = v.iter().map(|q| q.pt()).collect();
    near(polygon_area(&pts).abs(), 60., 1e-6);
    assert!(pts.iter().all(|q| q.x >= 49.999 && q.y >= 49.999));
    let t = with_box(10., 6., 10.);
    t.cmd("SECTIONPLANE").ty("XY").click(0., 0.).ty("50");
    assert!(t.history_has("does not cut"));
}

// ---------------------------------------------------------------- 3D transforms

#[test]
fn rotate_3d_about_the_x_axis_with_the_gizmo_command() {
    let s = with_box(10., 20., 30.);
    s.cmd("3DROTATE").click(0., 0.);
    assert!(s.prompt().starts_with("Pick a rotation axis or [X/Y/Z] <Z>"), "{}", s.prompt());
    s.ty("X").ty("90");
    assert!(!s.busy());
    let b = bounds(&s.solids()[0]);
    near(b.min.y, -30., 1e-6);
    near(b.max.z, 20., 1e-6);
    near(vol(&s.solids()[0]), 6000., 1e-6);
    s.cmd("U");
    near(bounds(&s.solids()[0]).max.z, 30., 1e-6);
}

#[test]
fn rotate_3d_about_z_moves_drawing_objects_too_and_leaves_them_for_other_axes() {
    let s = with_box(10., 10., 10.);
    let line = s.add(Entity::line(p(20., 0.), p(30., 0.)));
    s.set_sel(&[s.solids()[0].id, line]);
    s.cmd("3DROTATE").click(0., 0.).enter().ty("90");
    let Kind::Line { a, b } = s.ent(0).kind else { panic!() };
    assert!(dist(a, p(0., 20.)) < 1e-9 && dist(b, p(0., 30.)) < 1e-9);
    let b = bounds(&s.solids()[0]);
    near(b.min.x, -10., 1e-6);
    // about X the line cannot leave the drawing plane
    s.set_sel(&[s.solids()[0].id, line]);
    s.cmd("3DROTATE").click(0., 0.).ty("X").ty("90");
    assert!(s.history_has("left unchanged"));
    let Kind::Line { a, .. } = s.ent(0).kind else { panic!() };
    assert!(dist(a, p(0., 20.)) < 1e-9);
    assert!(bounds(&s.solids()[0]).max.z > 9.99 || bounds(&s.solids()[0]).min.z < -9.99);
}

#[test]
fn rotate_3d_start_and_end_angle_points() {
    let s = with_box(10., 10., 10.);
    s.cmd("3DROTATE").click(0., 0.).enter().click(10., 0.).click(0., 10.);
    let b = bounds(&s.solids()[0]);
    near(b.min.x, -10., 1e-6);
    near(b.max.y, 10., 1e-6);
}

#[test]
fn rotate3d_command_axes_and_reference_angle() {
    let s = with_box(10., 20., 30.);
    s.cmd("ROTATE3D").click(0., 0.).click(0., 10.).ty("90");
    let b = bounds(&s.solids()[0]);
    near(b.max.x, 30., 1e-6);
    near(b.min.z, -10., 1e-6);
    let t = with_box(10., 20., 30.);
    t.cmd("ROTATE3D");
    assert!(t.prompt().starts_with("Specify first point on axis or define axis by [Object/View/Xaxis/Yaxis/Zaxis/2points]"), "{}", t.prompt());
    t.ty("X").click(0., 0.).ty("R").ty("30").ty("120");
    let b = bounds(&t.solids()[0]);
    near(b.min.y, -30., 1e-6);
    near(b.max.z, 20., 1e-6);
    let u = with_box(10., 20., 30.);
    u.cmd("ROTATE3D").ty("Z").click(5., 5.).ty("180");
    let b = bounds(&u.solids()[0]);
    assert!(b.min.x.abs() < 1e-6 && (b.max.x - 10.).abs() < 1e-6 && (b.min.y + 10.).abs() < 1e-6 && (b.max.y - 10.).abs() < 1e-6, "{b:?}");
}

#[test]
fn mirror_3d_about_planes() {
    let s = with_box(10., 10., 10.);
    s.cmd("MIRROR3D").ty("YZ").click(0., 0.).enter();
    assert_eq!(s.solids().len(), 2);
    near(bounds(&s.solids()[1]).max.x, 0., 1e-6);
    near(bounds(&s.solids()[1]).min.x, -10., 1e-6);
    near(vol(&s.solids()[1]), 1000., 1e-6);
    assert!(check(&s.solids()[1].mesh()).valid(), "mirroring re-winds the faces");
    // delete the source and mirror in the XY plane at z = 12
    let t = with_box(10., 10., 10.);
    t.cmd("3DMIRROR").ty("XY").click(0., 0.).ty("12").ty("Y");
    assert_eq!(t.solids().len(), 1);
    near(bounds(&t.solids()[0]).min.z, 14., 1e-6);
    // drawing objects follow a vertical mirror plane
    let u = with_box(10., 10., 10.);
    let line = u.add(Entity::line(p(2., 3.), p(4., 3.)));
    u.set_sel(&[u.solids()[0].id, line]);
    u.cmd("MIRROR3D").ty("YZ").click(0., 0.).ty("Y");
    let Kind::Line { a, .. } = u.ent(0).kind else { panic!() };
    assert!(dist(a, p(-2., 3.)) < 1e-9);
}

#[test]
fn scale_3d_uniformly_and_along_axes() {
    let s = with_box(10., 10., 10.);
    s.cmd("3DSCALE").click(0., 0.).enter().ty("2");
    near(vol(&s.solids()[0]), 8000., 1e-6);
    let t = with_box(10., 10., 10.);
    t.cmd("3DSCALE").click(0., 0.).ty("Z").ty("3");
    near(bounds(&t.solids()[0]).max.z, 30., 1e-6);
    near(bounds(&t.solids()[0]).max.x, 10., 1e-6);
    let u = with_box(10., 10., 10.);
    u.cmd("3DSCALE").click(0., 0.).ty("XY").ty("C").ty("0.5");
    assert_eq!(u.solids().len(), 2);
    near(vol(&u.solids()[1]), 250., 1e-6);
    let v = with_box(10., 10., 10.);
    v.cmd("3DSCALE").click(0., 0.).enter().ty("R").ty("5").ty("15");
    near(vol(&v.solids()[0]), 27000., 1e-6);
}

#[test]
fn align_3d_with_one_two_and_three_point_pairs() {
    let s = with_box(10., 10., 10.);
    s.cmd("3DALIGN").click(0., 0.).enter().click(30., 5.);
    assert!(!s.busy());
    near(bounds(&s.solids()[0]).min.x, 30., 1e-6);
    near(bounds(&s.solids()[0]).min.y, 5., 1e-6);
    let t = with_box(10., 10., 10.);
    t.cmd("3DALIGN").click(0., 0.).click(10., 0.).enter().click(0., 20.).click(0., 30.);
    let b = bounds(&t.solids()[0]);
    near(b.min.x, -10., 1e-6);
    near(b.min.y, 20., 1e-6);
    near(b.max.y, 30., 1e-6);
    // copy: the original stays
    let u = with_box(10., 10., 10.);
    u.cmd("3DALIGN").ty("C").click(0., 0.).enter().click(40., 0.);
    assert_eq!(u.solids().len(), 2);
    near(bounds(&u.solids()[1]).min.x, 40., 1e-6);
}

#[test]
fn array_3d_rectangular_with_levels_and_polar() {
    let s = with_box(2., 2., 2.);
    s.cmd("3DARRAY").enter().ty("2").ty("3").ty("2").ty("5").ty("4").ty("10");
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 12);
    let b = mesh_bounds(&s.solids().iter().flat_map(|x| x.vertices.clone()).collect::<Vec<V3>>());
    near(b.max.x, 2. + 2. * 4., 1e-6);
    near(b.max.y, 2. + 5., 1e-6);
    near(b.max.z, 2. + 10., 1e-6);
    let t = with_box(2., 2., 2.);
    t.cmd("3DARRAY").ty("P").ty("4").enter().enter().click(10., 0.).enter();
    assert_eq!(t.solids().len(), 4);
    for x in t.solids() {
        let c = mesh_centroid(&x.mesh());
        near(dist(pt(c.x, c.y), p(10., 0.)), (81f64 + 1.).sqrt(), 1e-6);
        near(vol(&x), 8., 1e-6);
    }
    // not rotating keeps each copy's orientation
    let u = with_box(2., 2., 2.);
    u.cmd("3DARRAY").ty("P").ty("2").ty("180").ty("N").click(11., 1.).enter();
    assert_eq!(u.solids().len(), 2);
    near(bounds(&u.solids()[1]).min.x, 20., 1e-6);
    near(bounds(&u.solids()[1]).max.x, 22., 1e-6);
}

// ---------------------------------------------------------------- FILLETEDGE / CHAMFEREDGE / SHELL / SOLIDEDIT

#[test]
fn fillet_a_box_edge() {
    let s = with_box(20., 20., 20.);
    s.set_sel(&[]);
    s.cmd("FILLETEDGE");
    assert!(s.prompt().starts_with("Select an edge or [Chain/Loop/Radius]"), "{}", s.prompt());
    s.ty("R").ty("3").click(10., 0.);
    assert!(s.history_has("1 edge selected"));
    s.enter();
    assert!(s.prompt().starts_with("Press Enter to accept the fillet or [Radius]"), "{}", s.prompt());
    s.enter();
    assert!(!s.busy());
    let removed = 8000. - vol(&s.solids()[0]);
    near(removed, (1. - PI / 4.) * 9. * 20., 0.02);
    s.cmd("U");
    near(vol(&s.solids()[0]), 8000., 1e-6);
}

#[test]
fn fillet_radius_change_chain_loop_and_misses() {
    let s = with_box(20., 20., 20.);
    s.set_sel(&[]);
    s.cmd("FILLETEDGE").click(10., 10.);
    assert!(s.history_has("No sharp edge"));
    s.click(10., 0.).enter().ty("R").ty("2").enter();
    near(8000. - vol(&s.solids()[0]), (1. - PI / 4.) * 4. * 20., 0.02);
    // chain: every connected sharp edge, so all four edges of the top outline
    let t = with_box(20., 20., 20.);
    t.set_sel(&[]);
    t.cmd("FILLETEDGE").ty("C").click(10., 0.);
    assert!(t.history_has("4 edges selected"), "{}", t.log_text());
    // loop: the outline of the face nearest the viewer
    let u = with_box(20., 20., 20.);
    u.set_sel(&[]);
    u.cmd("FILLETEDGE").ty("L").click(10., 0.);
    assert!(u.history_has("4 edges selected"), "{}", u.log_text());
    u.enter().enter();
    assert!(8000. - vol(&u.solids()[0]) > 3.5 * (1. - PI / 4.) * 20.);
    // a radius too large for the faces is an error and changes nothing
    let v = with_box(20., 20., 2.);
    v.set_sel(&[]);
    v.cmd("FILLETEDGE").ty("R").ty("15").click(10., 0.).enter().enter();
    near(vol(&v.solids()[0]), 800., 1e-6);
    assert!(v.history_has("too large"));
}

#[test]
fn chamfer_a_box_edge_with_distances() {
    let s = with_box(20., 20., 20.);
    s.set_sel(&[]);
    s.cmd("CHAMFEREDGE");
    assert!(s.prompt().starts_with("Select an edge on a solid or [Loop/Distance]"), "{}", s.prompt());
    s.ty("D").ty("2").ty("2").click(10., 0.).enter();
    assert!(s.prompt().starts_with("Press Enter to accept the chamfer or [Distance]"), "{}", s.prompt());
    s.enter();
    near(8000. - vol(&s.solids()[0]), 0.5 * 4. * 20., 1e-4);
    // unequal distances: distance 1 goes on the face that looks at the viewer (the top face in plan view)
    let t = with_box(20., 20., 20.);
    t.set_sel(&[]);
    t.cmd("CHAMFEREDGE").ty("D").ty("2").ty("6").click(10., 0.).enter().enter();
    near(8000. - vol(&t.solids()[0]), 0.5 * 12. * 20., 1e-4);
    let b = bounds(&t.solids()[0]);
    near(b.max.z, 20., 1e-6);
}

#[test]
fn fillet_the_rim_of_a_cylinder_and_a_picked_edge_in_an_oblique_view() {
    let s = Sim::new();
    s.play("CYLINDER", &[Click(0., 0.), Type("10"), Type("20")]);
    s.cmd("FILLETEDGE").ty("R").ty("2").click(10., 0.).enter().enter();
    let removed = vol(&s.solids()[0]) - 0.;
    let full = PI * 100. * 20.;
    assert!(removed < full && removed > full - 70., "{removed}");
    let t = with_box(20., 20., 20.);
    t.set_sel(&[]);
    t.with_view(|v| {
        v.pitch = 0.0;
        v.yaw = 0.0;
        v.scale = 50.;
    });
    // front view: x to the right, z up. The top front edge is at height 20.
    let top_front = t.with_view(|v| v.project(V3::new(10., 0., 20.)));
    let wp = t.with_view(|v| v.to_world(top_front.x, top_front.y));
    t.cmd("FILLETEDGE").click(wp.x, wp.y);
    t.enter().enter();
    assert!(vol(&t.solids()[0]) < 8000. - 10.);
}

#[test]
fn shell_removes_a_face_and_hollows_the_solid() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.cmd("SHELL").click(5., 5.);
    assert!(s.prompt().starts_with("Remove faces or [Undo/Add/ALL]"), "{}", s.prompt());
    s.click(5., 5.).enter();
    assert!(s.prompt().starts_with("Enter the shell offset distance"), "{}", s.prompt());
    s.ty("1");
    assert!(!s.busy());
    near(vol(&s.solids()[0]), 1000. - 8. * 8. * 9., 1e-6);
    assert!(check(&s.solids()[0].mesh()).valid());
    s.cmd("U");
    near(vol(&s.solids()[0]), 1000., 1e-6);
    // no faces removed gives a sealed hollow
    s.cmd("SHELL").click(5., 5.).enter().ty("2");
    near(vol(&s.solids()[0]), 1000. - 216., 1e-6);
    // too thick
    let t = Sim::new();
    t.set_visual_style("Shaded");
    t.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    t.cmd("SHELL").click(5., 5.).enter().ty("6");
    assert!(t.history_has("too large"));
    near(vol(&t.solids()[0]), 1000., 1e-6);
}

#[test]
fn solidedit_body_clean_check_separate_shell_and_undo() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.cmd("SOLIDEDIT");
    assert!(s.prompt().starts_with("Enter a solids editing option or [Face/Edge/Body/Undo/eXit] <eXit>"), "{}", s.prompt());
    s.ty("B");
    assert!(s.prompt().starts_with("Enter a body editing option or [Imprint/seParate solids/Shell/cLean/Check/Undo/eXit] <eXit>"), "{}", s.prompt());
    s.ty("C").click(5., 5.);
    assert!(s.history_has("valid 3DSOLID"));
    s.ty("L").click(5., 5.);
    assert!(s.history_has("Cleaned"));
    s.ty("I");
    assert!(s.history_has("Imprinting is not supported"));
    s.ty("S").click(5., 5.).click(5., 5.).enter().ty("1");
    near(vol(&s.solids()[0]), 1000. - 8. * 8. * 9., 1e-6);
    s.ty("U");
    near(vol(&s.solids()[0]), 1000., 1e-6);
    s.enter().enter();
    assert!(!s.busy());
    // face options report that they are not available for meshes
    let t = Sim::new();
    t.cmd("SOLIDEDIT").ty("F").ty("E");
    assert!(t.history_has("not supported for mesh solids"));
}

#[test]
fn solidedit_separates_disjoint_bodies() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    let a = box_mesh(0., 0., 0., 10., 10., 10., "Box");
    let b = box_mesh(30., 0., 0., 10., 10., 10., "Box");
    let off = a.vertices.len() as u32;
    let mut both = a.clone();
    both.vertices.extend(b.vertices.iter().copied());
    both.faces.extend(b.faces.iter().map(|f| [f[0] + off, f[1] + off, f[2] + off]));
    s.doc_mut(|d| d.add_solid(both));
    assert_eq!(s.solids().len(), 1);
    s.cmd("SOLIDEDIT").ty("B").ty("P").click(5., 5.);
    s.enter().enter();
    assert_eq!(s.solids().len(), 2);
    near(vol(&s.solids()[0]), 1000., 1e-6);
    near(vol(&s.solids()[1]), 1000., 1e-6);
    s.cmd("SOLIDEDIT").ty("B").ty("P").click(5., 5.);
    assert!(s.history_has("single body"));
}

// ---------------------------------------------------------------- THICKEN / CONVTOSOLID / INTERFERE / SOLIDHIST / FLATSHOT

#[test]
fn thicken_regions_and_convtosolid_outlines() {
    let s = Sim::new();
    let r = s.add(Entity::new(Kind::Region(RegionData { loops: vec![vec![Vertex::new(0., 0., 0.), Vertex::new(10., 0., 0.), Vertex::new(10., 5., 0.), Vertex::new(0., 5., 0.)]] })));
    s.set_sel(&[r]);
    s.cmd("THICKEN").ty("4");
    assert!(!s.busy());
    near(vol(&s.solids()[0]), 200., 1e-6);
    let o = s.add(Entity::circle(p(50., 0.), 3.));
    s.set_sel(&[o]);
    s.cmd("CONVTOSOLID").ty("2");
    near(vol(&s.solids()[1]), PI * 9. * 2., 0.02);
    assert!(s.history_has("thickness"));
    // open outlines are reported, nothing is created
    let l = s.add(Entity::line(p(0., 20.), p(5., 20.)));
    s.set_sel(&[l]);
    s.cmd("THICKEN").ty("2");
    assert_eq!(s.solids().len(), 2);
    assert!(s.history_has("closed"));
}

#[test]
fn interfere_reports_pairs_and_creates_the_common_volume() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.play("BOX", &[Click(5., 0.), Click(15., 10.), Type("10")]);
    s.play("BOX", &[Click(40., 0.), Click(50., 10.), Type("10")]);
    s.cmd("INTERFERE").click(2., 5.).click(12., 5.).click(45., 5.).enter();
    assert!(s.prompt().starts_with("Select second set of objects"), "{}", s.prompt());
    s.enter();
    assert!(s.history_has("Number of interfering pairs: 1"), "{}", s.log_text());
    assert!(s.history_has("Interfering objects in first set: 2"));
    s.ty("Y");
    assert_eq!(s.solids().len(), 4);
    near(vol(&s.solids()[3]), 500., 1e-4);
    assert_eq!(s.solids()[3].name, "Interference");
    // two sets
    let t = Sim::new();
    t.set_visual_style("Shaded");
    t.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    t.play("BOX", &[Click(5., 0.), Click(15., 10.), Type("10")]);
    t.cmd("INTERFERE").click(2., 5.).enter().click(12., 5.).enter();
    assert!(t.history_has("Interfering objects in second set: 1"));
    t.ty("N");
    assert_eq!(t.solids().len(), 2);
    // apart: nothing
    let u = Sim::new();
    u.set_visual_style("Shaded");
    u.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    u.play("BOX", &[Click(20., 0.), Click(30., 10.), Type("10")]);
    u.cmd("INTERFERE").click(2., 5.).click(25., 5.).enter().enter();
    assert!(u.history_has("Number of interfering pairs: 0"));
    assert!(!u.busy());
}

#[test]
fn solidhist_removes_the_extrusion_history() {
    let s = Sim::new();
    rect(&s, 0., 0., 10., 10.);
    s.cmd("EXTRUDE").click(5., 0.).enter().ty("5");
    assert!(s.solids()[0].program.is_some());
    let id = s.solids()[0].id;
    s.set_sel(&[id]);
    s.cmd("SOLIDHIST").ty("R");
    assert!(s.history_has("cannot be recorded") || s.history_has("already record"));
    assert!(s.solids()[0].program.is_some());
    s.set_sel(&[id]);
    s.cmd("SOLIDHIST").enter();
    assert!(s.solids()[0].program.is_none());
    near(vol(&s.solids()[0]), 500., 1e-6);
}

#[test]
fn a_modified_solid_loses_its_history_so_updates_cannot_undo_the_edit() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    rect(&s, 0., 0., 20., 20.);
    s.cmd("EXTRUDE").click(10., 0.).enter().ty("20");
    s.set_sel(&[]);
    s.cmd("FILLETEDGE").click(10., 0.).enter().enter();
    assert!(s.solids()[0].program.is_none());
}

#[test]
fn flatshot_creates_a_block_of_lines_in_plan_view() {
    let s = Sim::new();
    s.play("BOX", &[Click(0., 0.), Click(10., 6.), Type("4")]);
    s.cmd("FLATSHOT").enter().click(100., 0.);
    assert!(!s.busy());
    let ents = s.ents();
    assert_eq!(ents.len(), 1);
    let Kind::Insert(ins) = &ents[0].kind else { panic!("expected a block insert") };
    assert_eq!(ins.name, "FLATSHOT1");
    let n = s.doc(|d| d.blocks[&ins.name].entities.len());
    assert_eq!(n, 4, "the outline of the top face");
    // obscured lines are added dashed on request
    s.cmd("FLATSHOT").ty("Y").click(200., 0.);
    let ents = s.ents();
    let Kind::Insert(ins) = &ents[1].kind else { panic!() };
    assert_eq!(ins.name, "FLATSHOT2");
    let hidden = s.doc(|d| d.blocks[&ins.name].entities.iter().filter(|e| e.ltype.as_deref() == Some("HIDDEN")).count());
    assert_eq!(hidden, 4);
    // nothing to flatten
    let t = Sim::new();
    t.cmd("FLATSHOT");
    assert!(t.history_has("no 3D solids"));
}

#[test]
fn commands_cancel_cleanly_with_escape() {
    for cmd in ["SLICE", "FILLETEDGE", "CHAMFEREDGE", "3DROTATE", "ROTATE3D", "MIRROR3D", "3DSCALE", "3DALIGN", "3DARRAY", "SOLIDEDIT", "SHELL", "PYRAMID", "POLYSOLID", "TORUS", "INTERFERE", "SOLIDHIST", "SECTIONPLANE", "FLATSHOT"] {
        let s = with_box(10., 10., 10.);
        s.cmd(cmd).esc();
        assert!(!s.busy(), "{cmd}");
        assert_eq!(s.solids().len(), 1, "{cmd}");
        near(vol(&s.solids()[0]), 1000., 1e-9);
    }
}
