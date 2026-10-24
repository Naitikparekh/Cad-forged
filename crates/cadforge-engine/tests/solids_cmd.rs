//! Port of `tests/unit/solids-cmd.test.js`.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::solid::mesh::*;
use cadforge_core::solid::Solid;
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::{Sim, Step::*};

fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "expected {b}, got {a}");
}
fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}
fn vol(s: &Solid) -> f64 {
    mesh_volume(&s.mesh())
}

#[test]
fn box_by_two_corners_and_height_cube_length_center_and_undo() {
    let s = Sim::new();
    s.play("BOX", &[Click(0., 0.), Click(10., 20.), Type("30")]);
    let so = s.solids();
    assert_eq!(so.len(), 1);
    near(vol(&so[0]), 6000., 1e-6);
    assert_eq!(so[0].layer, "0");
    assert!(so[0].id > 0);
    s.play("BOX", &[Click(50., 0.), Type("C"), Type("10")]);
    near(vol(&s.solids()[1]), 1000., 1e-6);
    s.play("BOX", &[Click(80., 0.), Type("L"), Type("4"), Type("5"), Type("6")]);
    near(vol(&s.solids()[2]), 120., 1e-6);
    s.play("BOX", &[Type("Center"), Click(100., 0.), Click(105., 4.), Type("2")]);
    near(vol(&s.solids()[3]), 10. * 8. * 2., 1e-6);
    s.cmd("U");
    assert_eq!(s.solids().len(), 3); // undo covers solids
}

#[test]
fn cylinder_cone_sphere_and_wedge() {
    let s = Sim::new();
    s.play("CYLINDER", &[Click(0., 0.), Type("5"), Type("10")]);
    near(vol(&s.solids()[0]), PI * 25. * 10., 4.);
    s.play("CYLINDER", &[Click(20., 0.), Type("D"), Type("10"), Type("-4")]);
    let b = mesh_bounds(&s.solids()[1].vertices);
    assert!(b.max.z <= 1e-9 && b.min.z < -3.9);
    s.play("CONE", &[Click(40., 0.), Type("6"), Type("12")]);
    near(vol(&s.solids()[2]), PI * 36. * 12. / 3., 5.);
    s.play("CONE", &[Click(60., 0.), Type("6"), Type("T"), Type("3"), Type("12")]);
    near(vol(&s.solids()[3]), PI * 12. / 3. * (36. + 18. + 9.), 6.);
    s.play("SPHERE", &[Click(80., 0.), Type("10")]);
    near(vol(&s.solids()[4]) / (4. / 3. * PI * 1000.), 1., 0.02);
    s.play("WEDGE", &[Click(100., 0.), Click(110., 20.), Type("30")]);
    near(vol(&s.solids()[5]), 3000., 1e-6);
}

#[test]
fn extrude_a_closed_polyline_a_circle_and_a_chain_of_lines_taper_errors() {
    let s = Sim::new();
    s.add(Entity::pline_pts(&[p(0., 0.), p(10., 0.), p(10., 20.), p(0., 20.)], true));
    s.add(Entity::circle(p(50., 10.), 5.));
    s.cmd("EXTRUDE").click(5., 0.).enter(); // pick the polyline edge, then Enter
    s.ty("30");
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 1);
    near(vol(&s.solids()[0]), 6000., 1e-6);
    assert_eq!(s.solids()[0].program.as_ref().unwrap().features[0].profile, vec![1]);
    s.cmd("EXTRUDE").click(55., 10.).enter().ty("10");
    near(vol(&s.solids()[1]), PI * 25. * 10., 5.);
    // lines forming a closed outline
    let pts = [p(100., 0.), p(110., 0.), p(110., 10.), p(100., 10.)];
    for i in 0..4 {
        s.add(Entity::line(pts[i], pts[(i + 1) % 4]));
    }
    s.cmd("EXTRUDE").ty("ALL").ty("").ty("1");
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 3);
    // open outline reports an error and creates nothing
    let t = Sim::new();
    t.add(Entity::line(p(0., 0.), p(10., 0.)));
    t.cmd("EXTRUDE").click(5., 0.).enter();
    assert!(!t.busy());
    assert_eq!(t.solids().len(), 0);
    assert!(t.history_has("closed"));
    // taper
    let u = Sim::new();
    u.add(Entity::pline_pts(&[p(0., 0.), p(20., 0.), p(20., 20.), p(0., 20.)], true));
    u.cmd("EXTRUDE").click(10., 0.).enter().ty("T").ty("10").ty("10");
    assert!(!u.busy());
    assert_eq!(u.solids().len(), 1);
    let so = u.solids();
    let v = vol(&so[0]);
    assert!(v < 4000. && v > 2500., "tapered volume {v}");
    near(mesh_bounds(&so[0].vertices).max.z, 10., 1e-9);
}

#[test]
fn union_subtract_and_intersect_pick_solids_on_screen() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.play("BOX", &[Click(5., 0.), Click(15., 10.), Type("10")]);
    let ids: Vec<u64> = s.solids().iter().map(|x| x.id).collect();
    s.cmd("UNION").click(2., 5.).click(13., 5.).enter();
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 1);
    near(vol(&s.solids()[0]), 1500., 1e-4);
    assert!(!ids.contains(&s.solids()[0].id));
    s.cmd("U");
    assert_eq!(s.solids().len(), 2);
    s.cmd("SUBTRACT").click(2., 5.).enter().click(13., 5.).enter();
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 1);
    near(vol(&s.solids()[0]), 500., 1e-4);
    s.cmd("U");
    s.cmd("INTERSECT").click(2., 5.).click(13., 5.).enter();
    near(vol(&s.solids()[0]), 500., 1e-4);
}

#[test]
fn window_selecting_solids_with_the_pick_first_set_erase_and_3dmove() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.play("BOX", &[Click(30., 0.), Click(40., 10.), Type("10")]);
    s.drag((-2., -2.), (12., 12.));
    assert_eq!(s.selected_solid_count(), 1);
    s.cmd("3DMOVE").click(0., 0.).click(5., 5.).ty("7");
    assert!(!s.busy());
    let bb = mesh_bounds(&s.solids()[0].vertices);
    near(bb.min.x, 5., 1e-6);
    near(bb.min.z, 7., 1e-6);
    s.drag((-2., -2.), (60., 30.));
    assert_eq!(s.selected_solid_count(), 2);
    s.cmd("ERASE");
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 0);
}

#[test]
fn move_copy_rotate_scale_and_mirror_act_on_solids() {
    let s = Sim::new();
    s.set_visual_style("Shaded");
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.drag((-2., -2.), (12., 12.));
    s.cmd("MOVE").click(0., 0.).click(20., 5.);
    let b = mesh_bounds(&s.solids()[0].vertices);
    near(b.min.x, 20., 1e-6);
    near(b.min.y, 5., 1e-6);
    s.drag((18., 3.), (32., 17.));
    s.cmd("COPY").click(20., 5.).click(40., 5.).enter();
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 2);
    near(mesh_bounds(&s.solids()[1].vertices).min.x, 40., 1e-6);
    assert_ne!(s.solids()[0].id, s.solids()[1].id);
    s.set_sel(&[s.solids()[0].id]);
    s.cmd("SCALE").click(20., 5.).ty("2");
    near(vol(&s.solids()[0]), 8000., 1e-6);
    near(mesh_bounds(&s.solids()[0].vertices).max.z, 20., 1e-6);
    s.set_sel(&[s.solids()[1].id]);
    s.cmd("MIRROR").click(0., 0.).click(0., 10.).enter(); // mirror about the Y axis, keep the original
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 3);
    near(vol(&s.solids()[2]), 1000., 1e-6);
    assert!(mesh_bounds(&s.solids()[2].vertices).max.x <= -39.99);
    s.set_sel(&[s.solids()[1].id]);
    s.cmd("ROTATE").click(40., 5.).ty("90");
    near(vol(&s.solids()[1]), 1000., 1e-6);
}

#[test]
fn presspull_massprop_updateextrusion_and_subtractive_extrusions() {
    let s = Sim::new();
    s.add(Entity::pline_pts(&[p(0., 0.), p(40., 0.), p(40., 40.), p(0., 40.)], true));
    s.add(Entity::circle(p(20., 20.), 5.));
    s.cmd("PRESSPULL").click(5., 5.).ty("8"); // area with a circular hole
    near(vol(&s.solids()[0]), (1600. - PI * 25.) * 8., 4.);
    s.set_visual_style("Shaded");
    s.cmd("MASSPROP").click(5., 5.).enter();
    assert!(s.history_has("Volume:"));
    // extrude a circle as a subtractive feature through the plate, then edit the circle and update
    let hole = s.add(Entity::circle(p(10., 10.), 3.));
    s.cmd_args("EXTRUDE", Args::none().with("op", ArgVal::Str("subtract".into())));
    s.click(13., 10.).enter().ty("20").click(5., 5.).enter();
    assert!(!s.busy());
    let v1 = vol(&s.solids()[0]);
    assert!(v1 < (1600. - PI * 25.) * 8. - 150., "drilled {v1}");
    let prog = s.solids()[0].program.clone().unwrap();
    assert_eq!(prog.features.len(), 1);
    assert!(prog.base.is_some(), "the plate before the cut is kept as the base");
    s.doc_mut(|d| {
        d.change(|d| {
            if let Some(Entity { kind: Kind::Circle { r, .. }, .. }) = d.by_id_mut(hole) {
                *r = 5.;
            }
        })
    });
    s.set_sel(&[s.solids()[0].id]);
    s.cmd("UPDATEEXTRUSION");
    assert!(vol(&s.solids()[0]) < v1 - 100., "a larger hole removes more material");
    s.doc_mut(|d| d.remove(hole));
    s.set_sel(&[s.solids()[0].id]);
    s.cmd("UPDATEEXTRUSION");
    assert!(s.history_has("deleted"), "missing source outline is reported");
}

#[test]
fn vscurrent_sets_the_visual_style_and_solids_on_frozen_layers_cannot_be_picked() {
    let s = Sim::new();
    s.cmd("VSCURRENT").ty("Shaded");
    assert_eq!(s.h.borrow().editor().unwrap().visual_style, "Shaded");
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    assert!(s.solid_at(5., 5.).is_some());
    s.doc_mut(|d| d.layers[0].frozen = true);
    assert!(s.solid_at(5., 5.).is_none());
}

#[test]
fn erase_all_removes_solids_too_but_not_those_on_locked_layers() {
    let s = Sim::new();
    s.play("BOX", &[Click(0., 0.), Click(10., 10.), Type("10")]);
    s.play("LINE", &[Click(0., 0.), Click(5., 5.), Enter]);
    assert_eq!(s.solids().len(), 1);
    s.cmd("ERASE").ty("ALL").enter();
    assert!(!s.busy());
    assert_eq!(s.solids().len(), 0);
    assert_eq!(s.n(), 0);
}
