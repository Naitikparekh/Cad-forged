//! Associative dimensions: a dimension made on geometry follows it when the geometry is moved, stretched or scaled;
//! DIMDISASSOCIATE frees it, DIMREASSOCIATE ties it again, DIMASSOC 0 makes plain dimensions, and the links survive
//! JSON, DXF and DWG.
use cadforge_core::dim::DimData;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn sim() -> Sim {
    let s = Sim::new();
    s.setvar("OSMODE", 16384.0);
    s
}

fn dims(s: &Sim) -> Vec<(u64, DimData)> {
    s.ents().into_iter().filter_map(|e| if let Kind::Dim(d) = e.kind { Some((e.id, d)) } else { None }).collect()
}

fn near(a: Option<Pt>, b: Pt) -> bool {
    a.is_some_and(|a| dist(a, b) < 1e-9)
}

#[test]
fn a_linear_dimension_follows_its_line() {
    let s = sim();
    let line = s.add(Entity::line(p(0., 0.), p(30., 0.)));
    s.cmd("DIMLINEAR").click(0., 0.).click(30., 0.).click(15., -10.);
    let (id, d) = dims(&s).remove(0);
    assert_eq!(d.assoc.len(), 2, "both extension line origins are tied to the line: {:?}", d.assoc);
    assert!(d.assoc.iter().all(|r| r.ent == line && r.kind == Some(SnapType::End)));
    // moving the line moves the whole dimension with it
    s.cmd("MOVE").click(15., 0.).enter().click(0., 0.).click(5., 20.);
    let d = s.doc(|doc| if let Kind::Dim(d) = &doc.by_id(id).unwrap().kind { d.clone() } else { panic!() });
    assert!(near(d.p1, p(5., 20.)) && near(d.p2, p(35., 20.)), "{:?} {:?}", d.p1, d.p2);
    assert!(near(d.dp, p(20., 10.)), "the dimension line goes along: {:?}", d.dp);
    // stretching one end changes the measurement; the dimension line stays where it is
    s.cmd("LENGTHEN").ty("DElta").ty("10").click(34., 20.).enter();
    let d = s.doc(|doc| if let Kind::Dim(d) = &doc.by_id(id).unwrap().kind { d.clone() } else { panic!() });
    assert!(near(d.p2, p(45., 20.)), "{:?} {}", d.p2, s.log_text());
    assert!(near(d.dp, p(20., 10.)));
    // each edit is one undo step with its dimension update
    s.undo();
    let d = s.doc(|doc| if let Kind::Dim(d) = &doc.by_id(id).unwrap().kind { d.clone() } else { panic!() });
    assert!(near(d.p2, p(35., 20.)));
    // erasing the line leaves the dimension, no longer associative
    s.cmd("ERASE").click(20., 20.).enter();
    let (_, d) = dims(&s).remove(0);
    assert!(d.assoc.is_empty() && near(d.p1, p(5., 20.)), "{:?}", d);
}

#[test]
fn a_radius_dimension_follows_its_circle() {
    let s = sim();
    s.add(Entity::circle(p(60., 20.), 8.));
    s.cmd("DIMRADIUS").click(68., 20.).click(75., 30.);
    let (id, d) = dims(&s).remove(0);
    assert_eq!(d.assoc.len(), 2, "{:?}", d.assoc);
    assert!(d.assoc.iter().any(|r| r.key == "pt" && r.radial));
    s.cmd("SCALE").click(60., 28.).enter().click(60., 20.).ty("2");
    let d = s.doc(|doc| if let Kind::Dim(d) = &doc.by_id(id).unwrap().kind { d.clone() } else { panic!() });
    assert!(near(d.c, p(60., 20.)) && near(d.pt, p(76., 20.)), "the point stays on the circle, now radius 16: {:?} {}", d.pt, s.log_text());
}

#[test]
fn disassociate_reassociate_regen_and_dimassoc() {
    let s = sim();
    s.add(Entity::line(p(0., 0.), p(30., 0.)));
    s.cmd("DIMALIGNED").click(0., 0.).click(30., 0.).click(15., -10.);
    s.cmd("DIMDISASSOCIATE").click(15., -10.).enter();
    assert!(s.history_has("1 dimension disassociated."), "{}", s.log_text());
    s.cmd("MOVE").click(15., 0.).enter().click(0., 0.).click(0., 5.);
    let (id, d) = dims(&s).remove(0);
    assert!(d.assoc.is_empty() && near(d.p1, p(0., 0.)), "a free dimension stays put");
    // reassociate: pick the points again on the moved line
    s.cmd("DIMREASSOCIATE").click(15., -10.).enter().click(0., 5.).click(30., 5.);
    let d = s.doc(|doc| if let Kind::Dim(d) = &doc.by_id(id).unwrap().kind { d.clone() } else { panic!() });
    assert_eq!(d.assoc.len(), 2, "{}", s.log_text());
    assert!(near(d.p1, p(0., 5.)) && near(d.p2, p(30., 5.)));
    s.cmd("DIMREGEN");
    assert!(s.history_has("1 dimension associative; 0 updated."), "{}", s.log_text());
    // DIMASSOC 0: new dimensions are plain
    s.setvar("DIMASSOC", 0.0);
    s.cmd("DIMALIGNED").click(0., 5.).click(30., 5.).click(15., 15.);
    assert!(dims(&s).iter().any(|(_, d)| d.assoc.is_empty()), "{}", s.log_text());
}

#[test]
fn links_survive_json_dxf_and_dwg() {
    let s = sim();
    s.add(Entity::circle(p(80., 0.), 5.));
    s.add(Entity::line(p(0., 0.), p(30., 0.)));
    s.cmd("DIMLINEAR").click(0., 0.).click(30., 0.).click(15., -10.);
    s.cmd("DIMDIAMETER").click(85., 0.).click(95., 10.);
    let d0 = s.doc(|d| d.clone());
    let refs = |d: &Drawing| -> Vec<Vec<(String, String, Option<SnapType>, bool)>> {
        d.entities().iter().filter_map(|e| if let Kind::Dim(x) = &e.kind { Some(x.assoc.iter().map(|r| (r.key.clone(), d.by_id(r.ent).map(|g| g.type_name().to_string()).unwrap_or_default(), r.kind, r.radial)).collect()) } else { None }).collect()
    };
    let want = refs(&d0);
    assert_eq!(want.iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 2]);
    let json = Drawing::from_json_str(&d0.to_json_string(), None).unwrap();
    assert_eq!(refs(&json), want, "project file");
    let dxf = cadforge_core::io::read_dxf(&cadforge_core::io::write_dxf(&d0), "t").unwrap();
    assert_eq!(refs(&dxf), want, "DXF");
    let dwg = cadforge_core::io::read_dwg(&cadforge_core::io::write_dwg(&d0).unwrap(), "t").unwrap();
    assert_eq!(refs(&dwg), want, "DWG");
    // the reopened dimension still follows: move the line inside a command
    let mut doc = dwg;
    let line = doc.entities().iter().find(|e| e.type_name() == "line").unwrap().id;
    doc.begin_command();
    doc.modify(line, |e| if let Kind::Line { a, b } = &mut e.kind { *a = *a + p(0., 7.); *b = *b + p(0., 7.) });
    doc.end_command("MOVE");
    let lin = doc.entities().iter().find_map(|e| if let Kind::Dim(x) = &e.kind { (x.p2.is_some()).then(|| x.clone()) } else { None }).unwrap();
    assert!(near(lin.p1, p(0., 7.)) && near(lin.p2, p(30., 7.)), "{:?}", lin);
}
