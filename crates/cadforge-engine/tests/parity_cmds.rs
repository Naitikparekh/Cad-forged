//! COUNT, BURST, NCOPY, BLOCKREPLACE, REVERSE, TEXTTOFRONT, HATCHTOBACK, DIMOVERRIDE and DIMJOGLINE.
use cadforge_core::dim::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::text::{AttDef, TextData};
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn near(a: Pt, b: Pt) {
    assert!(dist(a, b) < 1e-6, "{a:?} != {b:?}");
}

fn attdef(tag: &str, at: Pt) -> Entity {
    let mut t = TextData::new(at, 2.0, "default");
    t.att = Some(AttDef { tag: tag.into(), prompt: tag.into(), invisible: false, constant: false, verify: false, preset: false });
    Entity::new(Kind::Text(t))
}

fn with_blocks(s: &Sim) {
    s.doc_mut(|d| {
        d.define_block("BOLT", p(0., 0.), vec![Entity::circle(p(0., 0.), 1.), attdef("SIZE", p(2., 0.))]);
        d.define_block("NUT", p(0., 0.), vec![Entity::pline_pts(&[p(-1., -1.), p(1., -1.), p(1., 1.), p(-1., 1.)], true), attdef("SIZE", p(2., 0.))]);
    });
}

fn bolt(s: &Sim, at: Pt, size: &str) -> u64 {
    let mut e = Entity::insert("BOLT", at);
    if let Kind::Insert(i) = &mut e.kind {
        let mut t = TextData::new(at + p(2., 0.), 2.0, size);
        t.att = None;
        i.attribs.push(AttribData { tag: "SIZE".into(), t, hidden: false });
    }
    s.add(e)
}

#[test]
fn count_selects_every_reference_of_the_picked_block() {
    let s = Sim::new();
    with_blocks(&s);
    let a = bolt(&s, p(0., 0.), "M6");
    let b = bolt(&s, p(20., 0.), "M8");
    s.add(Entity::insert("NUT", p(40., 0.)));
    s.add(Entity::line(p(0., 20.), p(10., 20.)));
    s.cmd("COUNT").click(1., 0.);
    let mut sel = s.selected();
    sel.sort();
    assert_eq!(sel, vec![a, b], "{}", s.log_text());
    assert!(s.log_text().contains("Block \"BOLT\": 2"), "{}", s.log_text());
    s.cmd("COUNTLIST");
    assert!(s.log_text().contains("NUT") && s.log_text().contains("3 references in all"), "{}", s.log_text());
}

#[test]
fn count_finds_identical_objects_that_are_not_blocks() {
    let s = Sim::new();
    let a = s.add(Entity::circle(p(0., 0.), 2.));
    let b = s.add(Entity::circle(p(30., 5.), 2.));
    s.add(Entity::circle(p(60., 0.), 3.));
    s.cmd("COUNT").click(2., 0.);
    let mut sel = s.selected();
    sel.sort();
    assert_eq!(sel, vec![a, b], "{}", s.log_text());
}

#[test]
fn counttable_inserts_a_table_of_the_blocks() {
    let s = Sim::new();
    with_blocks(&s);
    bolt(&s, p(0., 0.), "M6");
    bolt(&s, p(20., 0.), "M6");
    s.add(Entity::insert("NUT", p(40., 0.)));
    s.cmd("COUNTTABLE").click(100., 100.);
    let t = s.ents().into_iter().find_map(|e| if let Kind::Table(t) = e.kind { Some(t) } else { None }).expect("a table");
    assert_eq!(t.cells[2], vec!["BOLT".to_string(), "2".to_string()]);
    assert_eq!(t.cells[3], vec!["NUT".to_string(), "1".to_string()]);
}

#[test]
fn burst_keeps_attribute_values_as_text() {
    let s = Sim::new();
    with_blocks(&s);
    let id = bolt(&s, p(10., 10.), "M12");
    s.set_sel(&[id]);
    s.cmd("BURST");
    let texts: Vec<String> = s.ents().into_iter().filter_map(|e| if let Kind::Text(t) = e.kind { Some(t.s) } else { None }).collect();
    assert_eq!(texts, vec!["M12".to_string()], "the value, not the definition: {}", s.log_text());
    assert!(s.ents().iter().any(|e| matches!(e.kind, Kind::Circle { c, .. } if dist(c, p(10., 10.)) < 1e-9)));
    assert!(!s.ents().iter().any(|e| matches!(e.kind, Kind::Insert(_))));
}

#[test]
fn ncopy_copies_a_nested_object_out_of_a_block() {
    let s = Sim::new();
    with_blocks(&s);
    s.add(Entity::insert("NUT", p(50., 50.)));
    s.cmd("NCOPY").click(51., 50.2).enter().click(0., 0.).click(10., 0.);
    let plines: Vec<Entity> = s.ents().into_iter().filter(|e| matches!(e.kind, Kind::Pline { .. })).collect();
    assert_eq!(plines.len(), 1, "{}", s.log_text());
    let Kind::Pline { v, .. } = &plines[0].kind else { unreachable!() };
    near(p(v[0].x, v[0].y), p(59., 49.));
    assert_eq!(s.n(), 2, "the reference stays");
}

#[test]
fn blockreplace_swaps_references_and_purges_the_old_block() {
    let s = Sim::new();
    with_blocks(&s);
    bolt(&s, p(0., 0.), "M6");
    bolt(&s, p(20., 0.), "M8");
    s.add(Entity::insert("NUT", p(40., 0.)));
    s.cmd("BLOCKREPLACE").click(1., 0.).click(41., 0.).enter();
    let names: Vec<(String, Vec<String>)> = s.ents().into_iter().filter_map(|e| if let Kind::Insert(i) = e.kind { Some((i.name, i.attribs.into_iter().map(|a| a.t.s).collect())) } else { None }).collect();
    assert!(names.iter().all(|(n, _)| n == "NUT"), "{names:?} {}", s.log_text());
    assert_eq!(names[0].1, vec!["M6".to_string()], "attribute values of shared tags are kept");
    assert!(!s.doc(|d| d.blocks.contains_key("BOLT")), "{}", s.log_text());
}

#[test]
fn reverse_turns_lines_polylines_and_splines_around() {
    let s = Sim::new();
    let l = s.add(Entity::line(p(0., 0.), p(10., 0.)));
    let mut v = vec![Vertex::new(0., 0., 0.5), Vertex::new(10., 0., 0.), Vertex::new(10., 10., 0.)];
    v[1].sw = 1.0;
    v[1].ew = 2.0;
    let pl = s.add(Entity::pline(v, false));
    s.set_sel(&[l, pl]);
    s.cmd("REVERSE");
    assert!(matches!(s.ent(0).kind, Kind::Line { a, .. } if dist(a, p(10., 0.)) < 1e-9));
    let Kind::Pline { v, .. } = s.ent(1).kind else { panic!() };
    near(p(v[0].x, v[0].y), p(10., 10.));
    // the edge 10,0 -> 10,10 now runs 10,10 -> 10,0 with its widths swapped; the arc edge keeps its shape
    assert_eq!((v[0].sw, v[0].ew, v[0].b), (2.0, 1.0, 0.0));
    assert_eq!(v[1].b, -0.5);
    assert_eq!(v[2].b, 0.0);
    let orig = pline_segs(&[Vertex::new(0., 0., 0.5), Vertex::new(10., 0., 0.)], false)[0];
    let back = pline_segs(&v[1..], false)[0];
    near(orig.at(0.5), back.at(0.5));
}

#[test]
fn texttofront_and_hatchtoback_reorder_by_type() {
    let s = Sim::new();
    let t = s.add(Entity::text(p(0., 0.), 2.0, "label"));
    let h = s.add(Entity::line(p(0., 0.), p(5., 5.)));
    let hatch = s.add(Entity::hatch(vec![vec![Vertex::new(0., 0., 0.), Vertex::new(5., 0., 0.), Vertex::new(5., 5., 0.)]], "SOLID"));
    s.cmd("TEXTTOFRONT").enter();
    let ids: Vec<u64> = s.ents().iter().map(|e| e.id).collect();
    assert_eq!(ids, vec![h, hatch, t]);
    s.cmd("HATCHTOBACK");
    let ids: Vec<u64> = s.ents().iter().map(|e| e.id).collect();
    assert_eq!(ids, vec![hatch, h, t]);
}

fn linear(s: &Sim) -> u64 {
    let mut d = DimData::new(DimType::Lin);
    d.p1 = Some(p(0., 0.));
    d.p2 = Some(p(100., 0.));
    d.dp = Some(p(50., 10.));
    d.ang = Some(0.0);
    s.add(Entity::dim(d))
}

fn dim(s: &Sim, id: u64) -> DimData {
    s.doc(|d| match &d.by_id(id).unwrap().kind {
        Kind::Dim(dd) => dd.clone(),
        _ => panic!(),
    })
}

#[test]
fn dimoverride_changes_one_dimension_and_clear_takes_it_back() {
    let s = Sim::new();
    let a = linear(&s);
    let b = linear(&s);
    s.doc_mut(|d| d.modify(b, |e| transform_entity(e, &mat::translate(0., 50.))));
    let h0 = s.doc(|d| dim_geometry(&dim(&s, a), &d.settings).text.h);
    s.cmd("DIMOVERRIDE").ty("DIMTXT").ty("7").ty("dec").ty("0").enter();
    s.click(20., 10.).enter();
    assert_eq!(dim(&s, a).ovr, vec![("DIMTXT".to_string(), "7".to_string()), ("DIMDEC".to_string(), "0".to_string())], "{}", s.log_text());
    assert!(dim(&s, b).ovr.is_empty());
    let geo = s.doc(|d| dim_geometry(&dim(&s, a), &d.settings));
    assert_eq!(geo.text.h, 7.0 * h0 / 2.5);
    assert_eq!(geo.text.s, "100");
    // the override survives a DXF round trip
    let text = s.doc(cadforge_core::io::dxf_write::write_dxf);
    let back = cadforge_core::io::dxf_read::read_dxf(&text, "t").unwrap();
    assert!(back.entities().iter().any(|e| matches!(&e.kind, Kind::Dim(d) if !d.ovr.is_empty())));
    s.cmd("DIMOVERRIDE").ty("C").click(20., 10.).enter();
    assert!(dim(&s, a).ovr.is_empty(), "{}", s.log_text());
}

#[test]
fn dimjogline_adds_a_jog_and_removes_it() {
    let s = Sim::new();
    let id = linear(&s);
    let plain = s.doc(|d| dim_geometry(&dim(&s, id), &d.settings).segs.len());
    s.cmd("DIMJOGLINE").click(30., 10.).click(30., 10.);
    let dd = dim(&s, id);
    assert!(dd.jl.is_some(), "{}", s.log_text());
    let geo = s.doc(|d| dim_geometry(&dd, &d.settings));
    assert_eq!(geo.segs.len(), plain + 4, "the dimension line is split around a three-edge jog");
    let grips = s.doc(|d| grips_of(&s.ent(0), d));
    assert!(grips.iter().any(|g| g.id == "jl"));
    s.cmd("DIMJOGLINE").ty("R").click(30., 10.);
    assert!(dim(&s, id).jl.is_none(), "{}", s.log_text());
}
