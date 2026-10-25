//! Tables, wipeouts, regions, attributes and gradients: geometry, transforms and DXF round trips.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::{read_dxf, write_dxf};
use cadforge_core::text::{AttDef, TextData};

fn table() -> TableData {
    TableData { p: pt(10.0, 100.0), cols: vec![20.0, 30.0, 25.0], rows: vec![8.0, 6.0, 6.0], cells: vec![vec!["Title".into(), String::new(), String::new()], vec!["A".into(), "B".into(), "C".into()], vec![String::new(); 3]], h: 2.5, merges: vec![[0, 0, 0, 2]], style: String::new(), title: true, rot: 0.0, link: None }
}

#[test]
fn table_geometry_grid_text_and_merge() {
    let t = table();
    let (segs, texts) = table_geometry(&t);
    // the title row is one merged cell: no vertical lines inside row 0, but the outer frame stays
    let verticals: Vec<_> = segs.iter().filter_map(|s| if let Seg::L { a, b, .. } = s { (a.x == b.x).then_some((a.x, a.y.max(b.y), a.y.min(b.y))) } else { None }).collect();
    assert!(verticals.iter().any(|v| v.0 == 10.0 && v.1 == 100.0 && v.2 == 80.0), "outer left edge should span the whole table: {verticals:?}");
    assert!(!verticals.iter().any(|v| v.0 == 30.0 && v.1 > 99.0), "no divider inside the merged title cell");
    assert!(verticals.iter().any(|v| v.0 == 30.0 && v.1 == 92.0), "dividers continue below the title row");
    assert_eq!(texts.len(), 4); // Title + A, B, C
    let title = texts.iter().find(|t| t.s == "Title").unwrap();
    near(title.p.x, 10.0 + 75.0 / 2.0);
    near(title.p.y, 96.0);
    near(title.h, 2.5 * 1.25);
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
}

#[test]
fn table_bounds_transform_and_json() {
    let mut d = Drawing::new("t");
    let id = d.add(Entity::new(Kind::Table(table())));
    let b = bounds_of(d.by_id(id).unwrap(), &d).unwrap();
    near(b.min_x, 10.0);
    near(b.max_x, 85.0);
    near(b.min_y, 80.0);
    assert!(b.max_y >= 100.0);
    let mut e = d.by_id(id).unwrap().clone();
    transform_entity(&mut e, &mat::mul(&mat::translate(5.0, 5.0), &mat::scale(2.0, 2.0, Pt::ZERO)));
    let Kind::Table(t) = &e.kind else { panic!() };
    near(t.cols[0], 40.0);
    near(t.h, 5.0);
    let json = serde_json::to_string(&e).unwrap();
    assert!(json.contains("\"type\":\"table\""));
    let back: Entity = serde_json::from_str(&json).unwrap();
    assert_eq!(back, e);
}

#[test]
fn wipeout_and_region_segments() {
    let mut d = Drawing::new("t");
    let w = d.add(Entity::new(Kind::Wipeout(WipeoutData { pts: vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0), pt(0.0, 10.0)] })));
    assert_eq!(segs_of(d.by_id(w).unwrap(), &d).len(), 4);
    let r = d.add(Entity::new(Kind::Region(RegionData { loops: vec![vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(4.0, 0.0, 0.0), Vertex::new(4.0, 4.0, 0.0)]] })));
    assert_eq!(segs_of(d.by_id(r).unwrap(), &d).len(), 3);
}

fn attdef(tag: &str, p: Pt) -> Entity {
    let mut t = TextData::new(p, 2.0, "DEFAULT");
    t.att = Some(AttDef { tag: tag.into(), prompt: format!("Enter {tag}"), invisible: false, constant: false, verify: false, preset: false });
    Entity::new(Kind::Text(t))
}

#[test]
fn attributes_round_trip_through_dxf() {
    let mut d = Drawing::new("t");
    d.define_block("TAGBLK", pt(0.0, 0.0), vec![Entity::circle(pt(0.0, 0.0), 5.0), attdef("NUMBER", pt(6.0, 0.0))]);
    let mut ins = Entity::insert("TAGBLK", pt(50.0, 50.0));
    if let Kind::Insert(i) = &mut ins.kind {
        i.attribs.push(AttribData { tag: "NUMBER".into(), t: TextData::new(pt(56.0, 50.0), 2.0, "17"), hidden: false });
    }
    d.add(ins);
    let text = write_dxf(&d);
    assert!(text.contains("ATTDEF") && text.contains("ATTRIB") && text.contains("SEQEND"));
    let back = read_dxf(&text, "t").unwrap();
    let def = back.blocks.get("TAGBLK").expect("block");
    let Some(Kind::Text(t)) = def.entities.iter().map(|e| &e.kind).find(|k| matches!(k, Kind::Text(_))) else { panic!("attdef lost") };
    assert_eq!(t.att.as_ref().unwrap().tag, "NUMBER");
    assert_eq!(t.att.as_ref().unwrap().prompt, "Enter NUMBER");
    let Some(Kind::Insert(i)) = back.model.entities.iter().map(|e| &e.kind).find(|k| matches!(k, Kind::Insert(_))) else { panic!("insert lost") };
    assert_eq!(i.attribs.len(), 1);
    assert_eq!(i.attribs[0].tag, "NUMBER");
    assert_eq!(i.attribs[0].t.s, "17");
}

#[test]
fn insert_texts_use_attribute_values_not_templates() {
    let mut d = Drawing::new("t");
    d.define_block("B", pt(0.0, 0.0), vec![attdef("T", pt(0.0, 0.0))]);
    let mut ins = Entity::insert("B", pt(0.0, 0.0));
    if let Kind::Insert(i) = &mut ins.kind {
        i.attribs.push(AttribData { tag: "T".into(), t: TextData::new(pt(0.0, 0.0), 2.0, "VALUE"), hidden: false });
    }
    let id = d.add(ins);
    let texts = texts_of(d.by_id(id).unwrap(), &d);
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].s, "VALUE");
}

#[test]
fn gradient_hatch_serialises() {
    let mut h = Entity::hatch(vec![vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(10.0, 0.0, 0.0), Vertex::new(10.0, 10.0, 0.0)]], "SOLID");
    if let Kind::Hatch(hd) = &mut h.kind {
        hd.gradient = Some(GradientData { c1: [255, 0, 0], c2: [0, 0, 255], angle: 0.0, kind: "LINEAR".into(), one_color: false, tint: 0.5 });
    }
    let json = serde_json::to_string(&h).unwrap();
    assert!(json.contains("gradient"));
    assert_eq!(serde_json::from_str::<Entity>(&json).unwrap(), h);
}

#[test]
fn extended_output_validates_with_ezdxf() {
    use std::process::Command;
    if !Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let mut d = Drawing::new("t");
    d.define_block("TAGBLK", pt(0.0, 0.0), vec![Entity::circle(pt(0.0, 0.0), 5.0), attdef("NUMBER", pt(6.0, 0.0))]);
    let mut ins = Entity::insert("TAGBLK", pt(50.0, 50.0));
    if let Kind::Insert(i) = &mut ins.kind {
        i.attribs.push(AttribData { tag: "NUMBER".into(), t: TextData::new(pt(56.0, 50.0), 2.0, "17"), hidden: false });
    }
    d.add(ins);
    d.add(Entity::new(Kind::Table(table())));
    d.add(Entity::new(Kind::Wipeout(WipeoutData { pts: vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0)] })));
    let file = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ext.dxf");
    std::fs::write(&file, write_dxf(&d)).unwrap();
    let script = "import sys, ezdxf\nfrom ezdxf import recover\ndoc, auditor = recover.readfile(sys.argv[1])\nassert not auditor.has_errors, [e.message for e in auditor.errors]\nms = doc.modelspace()\nins = [e for e in ms if e.dxftype() == 'INSERT'][0]\nassert [a.dxf.tag for a in ins.attribs] == ['NUMBER'] and ins.attribs[0].dxf.text == '17'\nblk = doc.blocks.get('TAGBLK')\nassert any(e.dxftype() == 'ATTDEF' for e in blk)\nprint('ok')";
    let out = Command::new("python3").args(["-I", "-c", script, file.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
