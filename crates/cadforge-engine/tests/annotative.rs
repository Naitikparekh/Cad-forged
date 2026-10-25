//! Annotative objects: CHPROP Annotative, CANNOSCALE, OBJECTSCALE, ANNOUPDATE / ANNORESET and SCALELISTEDIT, and the
//! annotative data kept through JSON, DXF and DWG.
use cadforge_core::dim::{dim_geometry, DimData, DimType};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn text_h(s: &Sim, id: u64) -> f64 {
    s.doc(|d| if let Kind::Text(t) = &d.by_id(id).unwrap().kind { t.h } else { panic!() })
}

fn set_scale(s: &Sim, name: &str) {
    s.cmd("SETVAR").ty("CANNOSCALE").ty(name);
    assert!(!s.busy(), "{}", s.log_text());
}

#[test]
fn annotative_text_is_sized_for_the_annotation_scale() {
    let s = Sim::new();
    let id = s.add(Entity::text(pt(0., 0.), 2.5, "NOTE"));
    s.cmd("CHPROP").click(1., 1.).enter().ty("Annotative").ty("Yes").enter();
    let anno = s.doc(|d| d.by_id(id).unwrap().anno.clone()).expect("annotative");
    assert_eq!((anno.paper, anno.scales.as_slice()), (2.5, ["1:1".to_string()].as_slice()), "{}", s.log_text());
    set_scale(&s, "1:50");
    assert_eq!(text_h(&s, id), 125.0);
    assert_eq!(s.h.borrow().getvar("CANNOSCALE").map(|v| v.text()), Ok("1:50".to_string()));
    s.undo();
    assert_eq!(text_h(&s, id), 2.5, "changing the scale is undone in one step");
    assert!(s.h.borrow_mut().setvar("CANNOSCALE", "3:7").is_err(), "only scales of the list");
    // OBJECTSCALE: supported scales
    s.cmd("OBJECTSCALE").click(1., 1.).enter().ty("Add").ty("1:100");
    assert!(s.history_has("1 object now supports 1:100."), "{}", s.log_text());
    s.cmd("OBJECTSCALE").click(1., 1.).enter().ty("Delete").ty("1:1");
    assert_eq!(s.doc(|d| d.by_id(id).unwrap().anno.clone().unwrap().scales), vec!["1:100".to_string()]);
    // a size changed by hand comes back with ANNOUPDATE
    s.doc_mut(|d| d.modify(id, |e| if let Kind::Text(t) = &mut e.kind { t.h = 9.0 }));
    s.cmd("ANNOUPDATE").click(1., 1.).enter();
    assert_eq!(text_h(&s, id), 2.5, "{}", s.log_text());
    s.cmd("ANNORESET").click(1., 1.).enter();
    assert!(s.history_has("1 annotative object reset to the current annotation scale."));
    // plain objects are left alone
    s.cmd("CHPROP").click(1., 1.).enter().ty("Annotative").ty("No").enter();
    set_scale(&s, "1:20");
    assert_eq!(text_h(&s, id), 2.5);
}

#[test]
fn annotative_dimensions_scale_their_text_and_arrows() {
    let s = Sim::new();
    let mut dd = DimData::new(DimType::Lin);
    (dd.p1, dd.p2, dd.dp) = (Some(pt(0., 0.)), Some(pt(100., 0.)), Some(pt(50., -10.)));
    let id = s.add(Entity::dim(dd));
    assert!(s.doc_mut(|d| d.set_annotative(id, true)));
    set_scale(&s, "1:10");
    let (sc, h) = s.doc(|d| {
        let Kind::Dim(x) = &d.by_id(id).unwrap().kind else { panic!() };
        (x.sc, dim_geometry(x, &d.settings).text.h)
    });
    assert_eq!(sc, Some(10.0));
    assert_eq!(h, 25.0, "the 2.5 text of the style is 25 units at 1:10");
}

#[test]
fn the_scale_list_can_be_edited() {
    let s = Sim::new();
    s.cmd("SCALELISTEDIT").ty("Add").ty("1:25").ty("Exit");
    assert!(s.doc(|d| d.anno_scale("1:25").is_some()), "{}", s.log_text());
    s.cmd("SCALELISTEDIT").ty("Add").ty("Detail").ty("5:1").ty("Exit");
    assert_eq!(s.doc(|d| d.anno_scale("detail").map(|x| x.factor())), Some(0.2));
    set_scale(&s, "1:25");
    s.cmd("SCALELISTEDIT").ty("Delete").ty("1:25").ty("Exit");
    assert!(s.history_has("The current annotation scale cannot be deleted."), "{}", s.log_text());
    s.cmd("SCALELISTEDIT").ty("Reset").ty("Exit");
    assert!(s.doc(|d| d.anno_scale("detail").is_none() && d.settings.cannoscale == "1:1"));
}

#[test]
fn annotative_data_survives_json_dxf_and_dwg() {
    let mut d = Drawing::new("t");
    let t = d.add(Entity::text(pt(0., 0.), 2.5, "NOTE"));
    let h = d.add(Entity::hatch(vec![vec![Vertex::new(0., 0., 0.), Vertex::new(10., 0., 0.), Vertex::new(10., 10., 0.)]], "ANSI31"));
    assert!(d.set_annotative(t, true) && d.set_annotative(h, true));
    d.settings.scales.push(cadforge_core::annotative::AnnoScale::new("1:25", 1.0, 25.0));
    d.set_annotation_scale("1:25", true).unwrap();
    let state = |d: &Drawing| {
        let annos: Vec<_> = d.entities().iter().map(|e| (e.type_name().to_string(), e.anno.clone())).collect();
        (d.settings.cannoscale.clone(), d.settings.scales.len(), annos)
    };
    let want = state(&d);
    assert_eq!(want.2[0].1.as_ref().map(|a| a.scales.clone()), Some(vec!["1:1".to_string(), "1:25".to_string()]), "the new scale was added to the objects");
    assert_eq!(state(&Drawing::from_json_str(&d.to_json_string(), None).unwrap()), want, "project file");
    assert_eq!(state(&cadforge_core::io::read_dxf(&cadforge_core::io::write_dxf(&d), "t").unwrap()), want, "DXF");
    assert_eq!(state(&cadforge_core::io::read_dwg(&cadforge_core::io::write_dwg(&d).unwrap(), "t").unwrap()), want, "DWG");
}
