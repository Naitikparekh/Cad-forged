//! Tests for the ext_tail command batch (TRACE, hatch boundaries, SETBYLAYER, TXT2MTXT, LISTSELECT, VIEWGO, CSV extracts, palettes).
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::Color;
use cadforge_core::text::{HJust, VJust};
use cadforge_engine::app::Event;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn events(s: &Sim) -> Vec<Event> {
    s.h.borrow_mut().take_events()
}

fn saved_file(s: &Sim) -> Option<(String, String)> {
    events(s).into_iter().find_map(|e| match e {
        Event::SaveFile { name, data, .. } => Some((name, String::from_utf8(data).unwrap())),
        _ => None,
    })
}

fn text(h: f64, s: &str) -> Entity {
    Entity::new(Kind::Text(cadforge_core::text::TextData { p: p(0., 0.), h, s: s.into(), rot: 0.0, jh: HJust::Left, jv: VJust::Baseline, w: None, multi: false, att: None }))
}

#[test]
fn trace_draws_one_solid_quad_per_segment_with_mitred_joins() {
    let s = Sim::new();
    s.cmd("TRACE").ty("2").click(0., 0.).click(10., 0.).click(10., 10.).enter();
    assert!(!s.busy());
    assert_eq!(s.n(), 2, "two segments");
    let loops: Vec<Vec<Vertex>> = s.ents().iter().map(|e| match &e.kind { Kind::Hatch(h) => h.loops[0].clone(), k => panic!("{k:?}") }).collect();
    // first quad: from (0,0) to the mitred corner at (10,0), half width 1 on each side
    let xs: Vec<f64> = loops[0].iter().map(|v| v.x).collect();
    assert!(xs.iter().any(|x| x.abs() < 1e-9) && xs.iter().any(|x| (x - 11.0).abs() < 1e-9 || (x - 9.0).abs() < 1e-9), "{:?}", loops[0]);
    // both quads share the two mitre corners at the joint
    let corner = |l: &Vec<Vertex>| l.iter().filter(|v| (v.x - 9.0).abs() < 1e-9 && (v.y - 1.0).abs() < 1e-9 || (v.x - 11.0).abs() < 1e-9 && (v.y + 1.0).abs() < 1e-9).count();
    assert_eq!(corner(&loops[0]), 2, "{:?}", loops[0]);
    assert_eq!(corner(&loops[1]), 2, "{:?}", loops[1]);
    assert_eq!(s.pref("TRACEWID"), 2.0, "the width is remembered");
}

#[test]
fn hatch_boundary_becomes_closed_polylines_on_the_hatchs_layer() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.add_layer("Fill").unwrap();
    });
    let ring = |x0: f64, x1: f64| vec![Vertex::new(x0, x0, 0.0), Vertex::new(x1, x0, 0.0), Vertex::new(x1, x1, 0.0), Vertex::new(x0, x1, 0.0)];
    s.add(Entity::new(Kind::Hatch(HatchData { loops: vec![ring(0., 10.), ring(3., 6.)], pattern: "SOLID".into(), ps: 1.0, pa: 0.0, gradient: None })).on_layer("Fill"));
    s.cmd("HATCHGENERATEBOUNDARY").click(1., 5.);
    let plines: Vec<Entity> = s.ents().into_iter().filter(|e| matches!(e.kind, Kind::Pline { .. })).collect();
    assert_eq!(plines.len(), 2, "outer boundary and island");
    assert!(plines.iter().all(|e| e.layer == "Fill" && matches!(e.kind, Kind::Pline { closed: true, .. })));
}

#[test]
fn setbylayer_clears_colour_linetype_lineweight_and_transparency() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(10., 0.)).with_color(Color::Aci(1)).with_lw(50));
    let b = s.add(Entity::line(p(0., 5.), p(10., 5.)).with_color(Color::ByBlock));
    s.doc_mut(|d| d.by_id_mut(a).unwrap().ltype = Some("DASHED".into()));
    s.cmd("SETBYLAYER").click(5., 0.).click(5., 5.).enter();
    s.enter(); // all properties
    s.ty("Yes");
    assert!(!s.busy(), "{}", s.prompt());
    for id in [a, b] {
        let e = s.doc(|d| d.by_id(id).unwrap().clone());
        assert_eq!((e.color, e.ltype.clone(), e.lw), (None, None, None));
    }
    assert!(s.history_has("2 objects changed to ByLayer"));
}

#[test]
fn setbylayer_can_keep_byblock_and_limit_the_properties() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(10., 0.)).with_color(Color::ByBlock).with_lw(50));
    s.cmd("SETBYLAYER").click(5., 0.).enter();
    s.ty("Color").ty("No");
    let e = s.doc(|d| d.by_id(a).unwrap().clone());
    assert_eq!(e.color, Some(Color::ByBlock), "ByBlock stays when asked");
    assert_eq!(e.lw, Some(50), "only the colour was in scope");
}

#[test]
fn txt2mtxt_converts_single_line_text() {
    let s = Sim::new();
    let id = s.add(text(2.5, "HELLO"));
    s.cmd("TXT2MTXT").click(1., 0.5).enter();
    match s.doc(|d| d.by_id(id).unwrap().kind.clone()) {
        Kind::Text(t) => assert!(t.multi && t.s == "HELLO"),
        k => panic!("{k:?}"),
    }
    assert!(s.history_has("1 text object converted to multiline text"));
}

#[test]
fn listselect_lists_the_current_selection_only() {
    let s = Sim::new();
    let a = s.add(Entity::line(p(0., 0.), p(10., 0.)));
    s.add(Entity::new(Kind::Circle { c: p(50., 50.), r: 5.0 }));
    s.cmd("LISTSELECT");
    assert!(s.history_has("No objects are selected"));
    s.select_ids(&[a]).cmd("LISTSELECT");
    let log = s.log_text();
    assert!(log.contains("LINE") && !log.contains("CIRCLE"), "{log}");
}

#[test]
fn viewgo_restores_a_saved_view() {
    let s = Sim::new();
    s.cmd("VIEWGO");
    assert!(s.history_has("no named views"));
    s.cmd("-VIEW").ty("Save").ty("home");
    s.with_view(|v| {
        v.target.x = 500.0;
        v.target.y = 500.0;
    });
    s.cmd("VIEWGO").ty("HOME");
    assert!(s.with_view(|v| v.target.x.abs() < 400.0), "view restored");
}

#[test]
fn navigation_synonyms_resolve() {
    let r = Sim::new().registry();
    for (alias, cmd) in [("3DPAN", "PAN"), ("3DZOOM", "ZOOM"), ("3DFORBIT", "3DORBIT")] {
        assert_eq!(r.find(alias).map(|d| d.name.clone()), Some(cmd.to_string()), "{alias}");
    }
}

#[test]
fn tableexport_writes_quoted_csv() {
    let s = Sim::new();
    s.add(Entity::new(Kind::Table(TableData { p: p(0., 0.), cols: vec![20., 20.], rows: vec![8., 8.], cells: vec![vec!["Item".into(), "Note, \"quoted\"".into()], vec!["A".into(), "1".into()]], h: 2.5, merges: vec![], style: String::new(), title: false, rot: 0.0 })));
    s.cmd("TABLEEXPORT").click(20., -4.);
    let (name, csv) = saved_file(&s).expect("a file");
    assert!(name.ends_with("-table.csv"), "{name}");
    assert_eq!(csv, "Item,\"Note, \"\"quoted\"\"\"\r\nA,1\r\n");
}

fn tagged_insert(name: &str, x: f64, tag: &str, value: &str) -> Entity {
    let t = match text(2.5, value).kind { Kind::Text(t) => t, _ => unreachable!() };
    Entity::new(Kind::Insert(InsertData { name: name.into(), p: p(x, 0.), sx: 1.0, sy: 1.0, rot: 0.0, attribs: vec![AttribData { tag: tag.into(), t, hidden: false }] }))
}

#[test]
fn attext_and_dataextraction_report_attribute_values_and_counts() {
    let s = Sim::new();
    s.doc_mut(|d| d.define_block("DOOR", p(0., 0.), vec![Entity::line(p(0., 0.), p(1., 0.))]));
    s.add(tagged_insert("DOOR", 0.0, "MARK", "D1"));
    s.add(tagged_insert("DOOR", 10.0, "MARK", "D1"));
    s.add(tagged_insert("DOOR", 20.0, "MARK", "D2"));
    s.cmd("ATTEXT").enter();
    let (name, csv) = saved_file(&s).expect("attext file");
    assert!(name.ends_with("-attributes.csv"));
    let lines: Vec<&str> = csv.trim_end().split("\r\n").collect();
    assert_eq!(lines[0], "BLOCK,X,Y,LAYER,MARK");
    assert_eq!(lines.len(), 4);
    assert!(lines[3].starts_with("DOOR,20,0,0,D2"), "{}", lines[3]);

    s.cmd("DATAEXTRACTION").enter();
    let (_, csv) = saved_file(&s).expect("extraction file");
    let lines: Vec<&str> = csv.trim_end().split("\r\n").collect();
    assert_eq!(lines, vec!["Count,Name,MARK", "2,DOOR,D1", "1,DOOR,D2"]);

    s.cmd("DATAEXTRACTION").ty("Objects");
    let (_, csv) = saved_file(&s).expect("object counts");
    assert!(csv.contains("Count,Name,Layer") && csv.contains("3,insert,0"), "{csv}");
}

#[test]
fn palette_and_workspace_commands_emit_host_events() {
    let s = Sim::new();
    s.h.borrow_mut().ui.dialogs = true;
    s.cmd("PROPERTIESCLOSE").cmd("RIBBONCLOSE").cmd("HIDEPALETTES");
    let pal: Vec<(String, Option<bool>)> = events(&s).into_iter().filter_map(|e| if let Event::Palette { name, show } = e { Some((name, show)) } else { None }).collect();
    assert_eq!(pal, vec![("properties".into(), Some(false)), ("ribbon".into(), Some(false)), ("hide-all".into(), None)]);
    s.cmd("WORKSPACE").ty("3D");
    let act: Vec<_> = events(&s).into_iter().filter_map(|e| if let Event::UiAction { name, args } = e { Some((name, args.str("name").map(String::from))) } else { None }).collect();
    assert_eq!(act, vec![("WORKSPACE".to_string(), Some("3D Modeling".to_string()))]);
}

#[test]
fn wblock_saves_a_dxf_with_the_blocks_it_uses() {
    let s = Sim::new();
    s.doc_mut(|d| d.define_block("BOLT", p(0., 0.), vec![Entity::new(Kind::Circle { c: p(0., 0.), r: 1.0 })]));
    s.add(Entity::new(Kind::Insert(InsertData { name: "BOLT".into(), p: p(10., 10.), sx: 1.0, sy: 1.0, rot: 0.0, attribs: vec![] })));
    s.cmd("WBLOCK").click(11., 10.).enter().click(10., 10.).ty("bolt-part");
    let (name, dxf) = saved_file(&s).expect("wblock file");
    assert_eq!(name, "bolt-part.dxf");
    let back = cadforge_core::io::read_dxf(&dxf, &name).unwrap();
    assert!(back.blocks.contains_key("BOLT"), "block definition travels along");
    match &back.entities()[0].kind {
        Kind::Insert(i) => assert!(i.p.x.abs() < 1e-9 && i.p.y.abs() < 1e-9, "shifted to the base point: {:?}", i.p),
        k => panic!("{k:?}"),
    }
}
