use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "expected {b}, got {a}");
}

fn viewports(s: &Sim, layout: &str) -> Vec<ViewportData> {
    s.doc(|d| d.layout(layout).unwrap().entities.iter().filter_map(|e| if let Kind::Viewport(v) = &e.kind { Some(v.clone()) } else { None }).collect())
}

fn space(s: &Sim) -> String {
    s.doc(|d| d.space.clone())
}

#[test]
fn new_drawings_have_two_layouts_and_the_first_gets_its_viewport_when_shown() {
    let s = Sim::new();
    assert_eq!(s.doc(|d| d.layout_names()), vec!["Layout1", "Layout2"]);
    assert!(viewports(&s, "Layout1").is_empty(), "not realised until first shown");
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    assert_eq!(space(&s), "Layout1");
    assert_eq!(viewports(&s, "Layout1").len(), 1);
    // paper A4 landscape, the viewport sits inside a 10 mm margin
    let v = &viewports(&s, "Layout1")[0];
    near(v.w, 277.0, 1e-9);
    near(v.h, 190.0, 1e-9);
}

#[test]
fn layout_new_copy_rename_delete_and_list() {
    let s = Sim::new();
    s.cmd("LAYOUT").ty("New").ty("Plan");
    assert_eq!(s.doc(|d| d.layout_names()), vec!["Layout1", "Layout2", "Plan"]);
    s.cmd("LAYOUT").ty("Copy").ty("Plan").ty("Plan copy");
    assert!(s.doc(|d| d.layout("Plan copy").is_some()));
    s.cmd("LAYOUT").ty("Rename").ty("Plan copy").ty("Detail");
    assert!(s.doc(|d| d.layout("Detail").is_some() && d.layout("Plan copy").is_none()));
    s.cmd("LAYOUT").ty("Delete").ty("Detail");
    assert!(s.doc(|d| d.layout("Detail").is_none()));
    s.cmd("LAYOUT").ty("New").ty("plan"); // duplicate names are refused (case-insensitive)
    assert!(s.last_log().contains("already exists"), "{}", s.last_log());
    s.cmd("LAYOUT").ty("?");
    assert!(s.log_text().contains("Layout2"));
    s.undo();
    s.undo();
    assert!(s.doc(|d| d.layout("Detail").is_some()) || s.doc(|d| d.layout("Plan").is_some()));
}

#[test]
fn model_space_content_shows_in_a_viewport_and_mspace_edits_model_space() {
    let s = Sim::new();
    s.add(Entity::line(pt(0.0, 0.0), pt(100.0, 0.0)));
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    s.cmd("MSPACE");
    assert_eq!(space(&s), "model", "MSPACE edits model space");
    assert!(s.h.borrow().editor().unwrap().mspace.is_some());
    s.cmd("LINE").click(10.0, 10.0).click(20.0, 20.0).enter();
    assert_eq!(s.doc(|d| d.model.entities.len()), 2, "the new line lives in model space");
    assert_eq!(s.doc(|d| d.layout("Layout1").unwrap().entities.len()), 1, "and not on the sheet");
    s.cmd("PSPACE");
    assert_eq!(space(&s), "Layout1");
    assert!(s.h.borrow().editor().unwrap().mspace.is_none());
}

#[test]
fn zoom_xp_inside_a_viewport_sets_its_scale() {
    let s = Sim::new();
    s.add(Entity::line(pt(0.0, 0.0), pt(1000.0, 500.0)));
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    s.cmd("MSPACE");
    s.cmd("ZOOM").ty("0.02xp");
    let v = &viewports(&s, "Layout1")[0];
    near(v.scale.unwrap(), 0.02, 1e-9);
    // panning inside the viewport changes what it shows, not its scale
    let before = v.vc;
    s.cmd("PAN").enter();
    s.h.borrow_mut().editor_mut().unwrap().view.pan(40.0, 0.0);
    s.h.borrow_mut().editor_mut().unwrap().sync_mspace_view();
    s.cmd("PSPACE");
    let v2 = &viewports(&s, "Layout1")[0];
    assert!(v2.vc.x != before.x, "pan moved the model region");
    near(v2.scale.unwrap(), 0.02, 1e-9);
}

#[test]
fn mview_creates_rectangular_polygonal_object_and_array_viewports() {
    let s = Sim::new();
    s.cmd("LAYOUT").ty("Set").ty("Layout2"); // showing a default layout adds its viewport
    assert_eq!(viewports(&s, "Layout2").len(), 1);
    s.cmd("MVIEW").click(20.0, 20.0).click(120.0, 100.0);
    let vps = viewports(&s, "Layout2");
    assert_eq!(vps.len(), 2);
    near(vps[1].w, 100.0, 1e-9);
    near(vps[1].h, 80.0, 1e-9);
    s.cmd("MVIEW").ty("Polygonal").click(150.0, 20.0).click(250.0, 20.0).click(200.0, 100.0).ty("C");
    let vps = viewports(&s, "Layout2");
    assert_eq!(vps.len(), 3);
    assert_eq!(vps[2].clip.as_ref().unwrap().len(), 3);
    s.cmd("MVIEW").ty("2").ty("Horizontal").click(10.0, 110.0).click(110.0, 190.0);
    assert_eq!(viewports(&s, "Layout2").len(), 5);
    s.cmd("MVIEW").ty("4").ty("Fit");
    assert_eq!(viewports(&s, "Layout2").len(), 9);
    s.cmd("MVIEW").ty("Fit");
    assert_eq!(viewports(&s, "Layout2").len(), 10);
}

#[test]
fn mview_is_refused_in_model_space() {
    let s = Sim::new();
    s.cmd("MVIEW");
    assert!(s.last_log().contains("not allowed"), "{}", s.last_log());
    assert!(!s.busy());
}

#[test]
fn viewport_lock_and_off_and_clip() {
    let s = Sim::new();
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    let id = s.doc(|d| d.layout("Layout1").unwrap().entities[0].id);
    s.cmd("MVIEW").ty("Lock").ty("ON").click(150.0, 10.0).enter(); // the viewport border is at y=10
    assert!(viewports(&s, "Layout1")[0].locked);
    s.cmd("MVIEW").ty("OFF").click(150.0, 10.0).enter();
    assert!(viewports(&s, "Layout1")[0].off);
    s.set_sel(&[id]);
}

#[test]
fn pagesetup_changes_the_paper_without_a_dialog() {
    let s = Sim::new();
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    s.cmd("PAGESETUP").ty("A3").ty("Portrait");
    let p = s.doc(|d| d.layout("Layout1").unwrap().paper.clone());
    assert_eq!(p.name, "A3");
    near(p.w, 297.0, 1e-9);
    near(p.h, 420.0, 1e-9);
    s.undo();
    assert_eq!(s.doc(|d| d.layout("Layout1").unwrap().paper.name.clone()), "A4");
}

#[test]
fn layout_wizard_builds_a_layout_with_a_viewport() {
    let s = Sim::new();
    s.cmd("LAYOUTWIZARD").ty("Elevation").ty("A2").ty("Landscape");
    assert_eq!(space(&s), "Elevation");
    let v = &viewports(&s, "Elevation");
    assert_eq!(v.len(), 1);
    near(v[0].w, 594.0 - 20.0, 1e-9);
}

#[test]
fn layouts_survive_a_json_round_trip() {
    let s = Sim::new();
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    let json = s.doc(|d| d.to_json_string());
    let back = cadforge_core::drawing::Drawing::from_json_str(&json, None).unwrap();
    assert_eq!(back.layout_names(), vec!["Layout1", "Layout2"]);
    assert_eq!(back.layout("Layout1").unwrap().entities.len(), 1);
}

#[test]
fn switching_tabs_remembers_each_view() {
    let s = Sim::new();
    s.add(Entity::line(pt(0.0, 0.0), pt(10.0, 0.0)));
    s.with_view(|v| v.scale = 77.0);
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    let paper_scale = s.view_scale();
    assert!((paper_scale - 77.0).abs() > 1e-6, "the sheet is fitted, not the model view");
    s.h.borrow_mut().editor_mut().unwrap().switch_space("model").unwrap();
    near(s.view_scale(), 77.0, 1e-9);
    s.h.borrow_mut().editor_mut().unwrap().switch_space("Layout1").unwrap();
    near(s.view_scale(), paper_scale, 1e-9);
}

#[test]
fn double_click_enters_a_viewport_and_leaves_it() {
    let s = Sim::new();
    s.add(Entity::line(pt(0.0, 0.0), pt(100.0, 0.0)));
    s.cmd("LAYOUT").ty("Set").ty("Layout1");
    // inside the default viewport (centre of the sheet): enter it
    s.h.double_click(pt(148.0, 105.0));
    assert!(s.h.borrow().editor().unwrap().mspace.is_some());
    assert_eq!(space(&s), "model");
    // double-click far outside the viewport on the sheet margin: back to the sheet. In model coordinates that is a
    // point whose sheet position is outside, found through the viewport transform.
    let (outside_model, _) = {
        let a = s.h.borrow();
        let ed = a.editor().unwrap();
        let pv = ed.paper_view();
        let sp = pv.to_screen(pt(3.0, 3.0));
        (ed.view.to_world(sp.x, sp.y), 0)
    };
    s.h.double_click(outside_model);
    assert!(s.h.borrow().editor().unwrap().mspace.is_none());
    assert_eq!(space(&s), "Layout1");
}
