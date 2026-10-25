//! DWG writing (AutoCAD 2004, the default, and AutoCAD 2000). A drawing saved as DWG must read back exactly as the
//! same drawing saved as DXF reads back: the DWG writer encodes the DXF writer's output, so any difference is a DWG
//! encoding fault (apart from what AutoCAD 2000 files cannot hold: true colours and gradients).
//!
//! Independent decoders check the files when they are available: ACadSharp (a .NET checker, path in DWG_ACADSHARP,
//! see tools/dwg/README.md) and LibreDWG's dwgread (directory in LIBREDWG_BIN).
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::Entity;
use cadforge_core::geom::{pt, Vertex};
use cadforge_core::io::dwg::{write_dwg_as, Ver};
use cadforge_core::io::{is_dwg, read_dwg, read_dxf, read_dxf_bytes, write_dwg, write_dxf};
use cadforge_core::sample::sample_drawing;
use std::path::PathBuf;
use std::process::Command;

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

/// JSON with numbers rounded to 10 significant digits (DXF files keep 15, DWG files every bit).
fn rounded<T: serde::Serialize>(x: &T) -> String {
    fn round(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    let r = if f.abs() < 1e-9 {
                        0.0
                    } else {
                        let k = 10f64.powi(10 - f.abs().log10().ceil() as i32);
                        (f * k).round() / k
                    };
                    *v = serde_json::json!(r);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(round),
            serde_json::Value::Object(o) => o.values_mut().for_each(round),
            _ => {}
        }
    }
    let mut j = serde_json::to_value(x).unwrap();
    round(&mut j);
    j.to_string()
}

fn canon(e: &Entity) -> String {
    let mut e = e.clone();
    e.id = 0;
    rounded(&e)
}

/// The entities, sorted; for AutoCAD 2000 with what its files cannot hold taken out: true colours (written as their
/// nearest palette index) and gradient fills (written as solid fills).
fn ents(es: &[Entity], ver: Ver) -> Vec<String> {
    let mut v: Vec<String> = es
        .iter()
        .map(|e| {
            let mut e = e.clone();
            if ver == Ver::R2000 {
                if let Some(cadforge_core::style::Color::Rgb(r, g, b)) = e.color {
                    e.color = Some(cadforge_core::style::Color::Aci(cadforge_core::style::rgb_to_aci([r, g, b])));
                }
                if let cadforge_core::entities::Kind::Hatch(h) = &mut e.kind {
                    h.gradient = None;
                }
            }
            canon(&e)
        })
        .collect();
    v.sort();
    v
}

const VERSIONS: [(Ver, &[u8]); 2] = [(Ver::R2004, b"AC1018"), (Ver::R2000, b"AC1015")];

/// Save as DWG (both versions) and as DXF, read them back, and require the same drawing. Returns the 2004 file.
fn same_as_dxf(d: &Drawing, what: &str) -> Vec<u8> {
    let b = read_dxf(&write_dxf(d), what).unwrap();
    let mut first = vec![];
    for (ver, tag) in VERSIONS {
        let what = format!("{what} ({})", ver.label());
        let what = what.as_str();
        let dwg = write_dwg_as(d, ver).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert!(is_dwg(&dwg) && dwg.starts_with(tag), "{what}");
        let a = read_dwg(&dwg, what).unwrap_or_else(|e| panic!("{what}: reading our DWG: {e}"));
        assert_eq!(a.skipped, b.skipped, "{what}: skipped");
        assert_eq!(ents(&a.model.entities, ver), ents(&b.model.entities, ver), "{what}: model space");
        assert_eq!(a.layout_names(), b.layout_names(), "{what}: layouts");
        for (la, lb) in a.layouts.iter().zip(&b.layouts) {
            assert_eq!(ents(&la.entities, ver), ents(&lb.entities, ver), "{what}: layout {}", la.name);
            assert_eq!(rounded(&la.paper), rounded(&lb.paper), "{what}: sheet of {}", la.name);
        }
        assert_eq!(a.blocks.keys().collect::<Vec<_>>(), b.blocks.keys().collect::<Vec<_>>(), "{what}: blocks");
        for (k, ba) in &a.blocks {
            assert_eq!(ents(&ba.entities, ver), ents(&b.blocks[k].entities, ver), "{what}: block {k}");
            assert_eq!((ba.base.x, ba.base.y), (b.blocks[k].base.x, b.blocks[k].base.y), "{what}: base of {k}");
        }
        let layers = |d: &Drawing| {
            let mut l = d.layers.clone();
            for x in l.iter_mut() {
                if let (Ver::R2000, cadforge_core::style::Color::Rgb(r, g, b)) = (ver, x.color) {
                    x.color = cadforge_core::style::Color::Aci(cadforge_core::style::rgb_to_aci([r, g, b]));
                }
            }
            serde_json::to_string(&l).unwrap()
        };
        assert_eq!(layers(&a), layers(&b), "{what}: layers");
        assert_eq!(rounded(&a.settings), rounded(&b.settings), "{what}: settings");
        assert_eq!(a.images.keys().collect::<Vec<_>>(), b.images.keys().collect::<Vec<_>>(), "{what}: images");
        if first.is_empty() {
            first = dwg;
        }
    }
    first
}

#[test]
fn the_sample_drawing() {
    let d = sample_drawing();
    let dwg = same_as_dxf(&d, "sample");
    // saving again gives a file of the same size (only the save dates differ)
    assert_eq!(write_dwg(&d).unwrap().len(), dwg.len());
    let back = read_dwg(&dwg, "sample").unwrap();
    assert_eq!(back.model.entities.len(), read_dxf(&write_dxf(&d), "x").unwrap().model.entities.len());
    assert_eq!(back.model.entities.len(), d.model.entities.len());
}

#[test]
fn every_dxf_corpus_drawing() {
    for f in std::fs::read_dir(repo("tests/dxf/corpus")).unwrap() {
        let p = f.unwrap().path();
        if p.extension().is_some_and(|e| e == "dxf") {
            let d = read_dxf_bytes(&std::fs::read(&p).unwrap(), "corpus").unwrap();
            same_as_dxf(&d, &p.file_name().unwrap().to_string_lossy());
        }
    }
}

#[test]
fn autocad_drawings_saved_again_as_dwg() {
    for v in ["AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032"] {
        let d = read_dwg(&std::fs::read(repo(&format!("tests/dwg/samples/sample_{v}.dwg"))).unwrap(), v).unwrap();
        let dwg = same_as_dxf(&d, v);
        // and saving it again keeps every entity (splines read from files are resampled, as with DXF, so only kinds
        // and layers are compared)
        let again = read_dwg(&dwg, v).unwrap();
        let third = read_dwg(&write_dwg(&again).unwrap(), v).unwrap();
        let kinds = |d: &Drawing| {
            let mut k: Vec<(String, String)> = d.model.entities.iter().map(|e| (e.type_name().to_string(), e.layer.clone())).collect();
            k.sort();
            k
        };
        assert_eq!(kinds(&third), kinds(&again), "{v}: second save");
    }
}

/// Everything a drawing holds, for "the same drawing" checks: model space, layouts with their sheets, blocks with
/// their base points, layers, settings and pictures (numbers to 10 significant digits), as (part, its objects).
fn whole(d: &Drawing) -> Vec<(String, Vec<String>)> {
    let mut out = vec![("model".to_string(), ents(&d.model.entities, Ver::R2004))];
    for l in &d.layouts {
        out.push((format!("layout {}", l.name), [vec![rounded(&l.paper)], ents(&l.entities, Ver::R2004)].concat()));
    }
    for (k, b) in &d.blocks {
        out.push((format!("block {k}"), [vec![rounded(&(b.base.x, b.base.y))], ents(&b.entities, Ver::R2004)].concat()));
    }
    // (a solid without a layer is on layer 0 in DXF and DWG)
    out.push(("solids".into(), d.solids.iter().map(|s| rounded(&(&s.name, if s.layer.is_empty() { "0" } else { &s.layer }, &s.color, &s.vertices, &s.faces))).collect()));
    out.push(("layers".into(), d.layers.iter().map(rounded).collect()));
    out.push(("settings".into(), vec![rounded(&d.settings)]));
    out.push(("images".into(), d.images.keys().cloned().collect()));
    out.push(("skipped".into(), d.skipped.iter().map(|(k, n)| format!("{k} {n}")).collect()));
    out
}

fn assert_same(a: &Drawing, b: &Drawing, what: &str) {
    let (wa, wb) = (whole(a), whole(b));
    let parts = |w: &[(String, Vec<String>)]| w.iter().map(|p| p.0.clone()).collect::<Vec<_>>();
    assert_eq!(parts(&wa), parts(&wb), "{what}: parts");
    for ((part, x), (_, y)) in wa.iter().zip(&wb) {
        // the first object that differs, not the whole part
        if let Some(k) = (0..x.len().max(y.len())).find(|&k| x.get(k) != y.get(k)) {
            panic!("{what}, {part}: object {k} of {}/{}\n  first: {:?}\n second: {:?}", x.len(), y.len(), x.get(k), y.get(k));
        }
    }
}

/// A drawing with 3D solids: the L-bracket project's extrusion and a mesh of more than 10000 triangles (written as
/// several polyface meshes, joined again when read).
fn solids_drawing() -> Drawing {
    use cadforge_core::solid::{Solid, V3};
    let text = std::fs::read_to_string(repo("samples/L-Bracket.cadforge.json")).unwrap();
    let mut d = cadforge_core::io::from_json(&text, "solids").unwrap();
    let (n, m) = (120u32, 50u32);
    let mut s = Solid { id: 0, name: "Dome".into(), layer: "0".into(), color: Some(cadforge_core::style::Color::Aci(3)), vertices: vec![], faces: vec![], program: None };
    for i in 0..=n {
        for j in 0..=m {
            let (a, b) = (i as f64 / n as f64 * std::f64::consts::TAU, j as f64 / m as f64 * std::f64::consts::FRAC_PI_2);
            s.vertices.push(V3::new(300.0 + 40.0 * b.cos() * a.cos(), 40.0 * b.cos() * a.sin(), 40.0 * b.sin()));
        }
    }
    for i in 0..n {
        for j in 0..m {
            let k = i * (m + 1) + j;
            s.faces.push([k, k + m + 1, k + 1]);
            s.faces.push([k + 1, k + m + 1, k + m + 2]);
        }
    }
    assert!(s.faces.len() > 10_000);
    d.insert_solid_raw(s);
    d
}

/// Open, save as AutoCAD 2004 DWG, open again: the same drawing, object by object, for every AutoCAD sample, every
/// DXF corpus drawing and CadForge's own sample (leaders with text, splines, groups, wipeouts, images, layouts) and
/// a drawing with 3D solids.
#[test]
fn open_save_open_gives_the_same_drawing() {
    let mut inputs: Vec<(String, Drawing)> = vec![("sample".into(), sample_drawing()), ("solids".into(), solids_drawing())];
    for v in ["AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032"] {
        inputs.push((v.into(), read_dwg(&std::fs::read(repo(&format!("tests/dwg/samples/sample_{v}.dwg"))).unwrap(), v).unwrap()));
    }
    for f in std::fs::read_dir(repo("tests/dxf/corpus")).unwrap() {
        let p = f.unwrap().path();
        if p.extension().is_some_and(|e| e == "dxf") {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            inputs.push((name.clone(), read_dxf_bytes(&std::fs::read(&p).unwrap(), &name).unwrap()));
        }
    }
    for (what, d) in &inputs {
        // the drawing as CadForge holds it after opening (a CadForge-made drawing: after one save, which turns
        // what DWG cannot hold into what it can)
        let first = read_dwg(&write_dwg(d).unwrap(), what).unwrap();
        let second = read_dwg(&write_dwg(&first).unwrap(), what).unwrap();
        assert_same(&first, &second, what);
        if what == "solids" {
            // the meshes themselves, exactly (the extrusion history that rebuilds a solid from its outline is not kept)
            let part = |d: &Drawing| whole(d).into_iter().find(|p| p.0 == "solids").unwrap();
            assert_eq!(part(d), part(&first));
            assert_eq!(first.solids.len(), 2);
        }
        if what.starts_with("AC10") {
            // what CadForge could not read from AutoCAD's file (3D solids, tables...) is not in the saved file
            assert!(first.skipped.is_empty(), "{what}");
            let mut d = d.clone();
            d.skipped.clear();
            assert_same(&d, &first, what);
        }
    }
}

/// One CadForge dimension of each kind (two linear ones, the second with a moved and rotated text, a text override
/// and a break; an aligned one with an inspection frame) and the drawing holding them.
fn dims_drawing() -> (Drawing, Vec<cadforge_core::dim::DimData>) {
    use cadforge_core::dim::{DimData, DimInspect, DimType, InspShape};
    let p = |x: f64, y: f64| Some(pt(x, y));
    let mut dims = vec![];
    let mut lin = DimData::new(DimType::Lin);
    (lin.p1, lin.p2, lin.dp, lin.ang) = (p(0.0, 0.0), p(40.0, 0.0), p(20.0, 10.0), Some(0.0));
    dims.push(lin.clone());
    lin.tp = p(25.0, 14.0);
    lin.tr = Some(0.3);
    lin.txt = Some("<> \u{2300} typ".into());
    lin.brk = vec![(0, 5.0, 8.0)];
    dims.push(lin);
    let mut ali = DimData::new(DimType::Ali);
    (ali.p1, ali.p2, ali.dp) = (p(0.0, 20.0), p(30.0, 50.0), p(10.0, 45.0));
    ali.insp = Some(DimInspect { label: "A".into(), rate: Some(100), shape: InspShape::Angular });
    dims.push(ali);
    let mut rad = DimData::new(DimType::Rad);
    (rad.c, rad.pt) = (p(80.0, 0.0), p(90.0, 5.0));
    dims.push(rad.clone());
    rad.kind = DimType::Dia;
    dims.push(rad);
    let mut ang = DimData::new(DimType::Ang);
    (ang.c, ang.p1, ang.p2, ang.dp) = (p(0.0, -40.0), p(30.0, -40.0), p(0.0, -10.0), p(20.0, -20.0));
    dims.push(ang.clone());
    ang.kind = DimType::Arc;
    dims.push(ang);
    let mut jog = DimData::new(DimType::Jog);
    (jog.c, jog.p1, jog.pt, jog.p2, jog.dp) = (p(60.0, -40.0), p(40.0, -40.0), p(70.0, -40.0), p(55.0, -40.0), p(60.0, -37.0));
    dims.push(jog);
    let mut ord = DimData::new(DimType::Ord);
    (ord.c, ord.pt, ord.dp, ord.xd) = (p(0.0, 0.0), p(30.0, 20.0), p(30.0, 40.0), true);
    dims.push(ord);
    let mut d = Drawing::new("dims");
    for x in &dims {
        d.add(Entity::dim(x.clone()));
    }
    (d, dims)
}

/// Every kind of CadForge dimension, with a moved and rotated text, a text override, breaks and an inspection
/// frame, opens again as the same dimension from DWG (both versions) and DXF; a dimension changed by another
/// program (its points no longer what CadForge wrote) opens as the geometry that program drew.
#[test]
fn cadforge_dimensions_open_again_as_dimensions() {
    use cadforge_core::dim::DimData;
    let (d, dims) = dims_drawing();
    let got = |d: &Drawing| -> Vec<DimData> { d.model.entities.iter().filter_map(|e| if let cadforge_core::entities::Kind::Dim(x) = &e.kind { Some(x.clone()) } else { None }).collect() };
    let round = |v: Vec<DimData>| v.iter().map(rounded).collect::<Vec<_>>();
    for (ver, _) in VERSIONS {
        let back = read_dwg(&write_dwg_as(&d, ver).unwrap(), "dims").unwrap();
        assert_eq!(back.model.entities.len(), dims.len(), "{}", ver.label());
        assert_eq!(round(got(&back)), round(dims.clone()), "{}", ver.label());
    }
    let dxf = write_dxf(&d);
    assert_eq!(round(got(&read_dxf(&dxf, "dims").unwrap())), round(dims.clone()));
    // moved by another program: the first definition point of the first dimension is no longer CadForge's
    let at = dxf.find("AcDbAlignedDimension").unwrap();
    let k = at + dxf[at..].find("\n 13\r\n").or_else(|| dxf[at..].find("\n13\r\n")).unwrap();
    let v = k + dxf[k..].find("\r\n").unwrap() + 2;
    let edited = format!("{}1{}", &dxf[..v], &dxf[v + 1..]);
    assert_eq!(&dxf[v..v + 3], "0\r\n", "the first point of the first dimension is at x = 0");
    let back = read_dxf(&edited, "dims").unwrap();
    assert_eq!(got(&back).len(), dims.len() - 1, "the changed one is read as its geometry");
    assert!(back.model.entities.iter().any(|e| e.type_name() == "line"));
}

/// A solid over the 32767 faces a polyface mesh holds is written as several meshes and read back as one solid with
/// the same triangles (its vertices are repeated where parts meet).
#[test]
fn a_huge_solid_is_split_and_joined_again() {
    use cadforge_core::solid::{Solid, V3};
    let n = 140u32;
    let mut s = Solid { id: 0, name: "Grid".into(), layer: "0".into(), color: None, vertices: vec![], faces: vec![], program: None };
    for i in 0..=n {
        for j in 0..=n {
            s.vertices.push(V3::new(i as f64, j as f64, ((i * j) % 7) as f64 * 0.25));
        }
    }
    for i in 0..n {
        for j in 0..n {
            let k = i * (n + 1) + j;
            s.faces.push([k, k + n + 1, k + 1]);
            s.faces.push([k + 1, k + n + 1, k + n + 2]);
        }
    }
    assert!(s.faces.len() > 32_767);
    let mut d = Drawing::new("huge");
    d.insert_solid_raw(s);
    let tris = |s: &Solid| {
        let mut t: Vec<String> = s.faces.iter().map(|f| rounded(&f.map(|i| s.vertices[i as usize]))).collect();
        t.sort();
        t
    };
    for (ver, _) in VERSIONS {
        let back = read_dwg(&write_dwg_as(&d, ver).unwrap(), "huge").unwrap();
        assert_eq!(back.solids.len(), 1, "{}", ver.label());
        assert_eq!(tris(&back.solids[0]), tris(&d.solids[0]), "{}", ver.label());
    }
    let back = read_dxf(&write_dxf(&d), "huge").unwrap();
    assert_eq!(back.solids.len(), 1);
    assert_eq!(tris(&back.solids[0]), tris(&d.solids[0]));
}

/// A table (with a merged title row and a non-ASCII cell) and a region with a hole.
fn tables_drawing() -> Drawing {
    use cadforge_core::entities::{Kind, RegionData, TableData};
    let mut d = Drawing::new("tables");
    let table: TableData = serde_json::from_value(serde_json::json!({
        "p": {"x": 10.0, "y": 50.0}, "cols": [30.0, 20.0], "rows": [8.0, 8.0, 8.0],
        "cells": [["Part", "Qty"], ["Bolt \u{00D8}8", "4"], ["Nut", "4"]], "h": 2.5, "merges": [[0, 0, 0, 1]]
    }))
    .unwrap();
    d.add(Entity::new(Kind::Table(table)).on_layer("0"));
    let ring = |r: f64| (0..24).map(|i| { let a = i as f64 / 24.0 * std::f64::consts::TAU; Vertex::new(100.0 + r * a.cos(), 20.0 + r * a.sin(), 0.0) }).collect::<Vec<_>>();
    d.add(Entity::new(Kind::Region(RegionData { loops: vec![ring(10.0), ring(4.0)] })));
    d
}

/// Tables and regions open again as tables and regions (other programs see the block of their lines and texts); a
/// table moved by another program opens as that block.
#[test]
fn tables_and_regions_open_again() {
    let d = tables_drawing();
    let kinds = |d: &Drawing| d.model.entities.iter().map(|e| e.type_name().to_string()).collect::<Vec<_>>();
    for (ver, _) in VERSIONS {
        let back = read_dwg(&write_dwg_as(&d, ver).unwrap(), "t").unwrap();
        assert_eq!(kinds(&back), vec!["table", "region"], "{}", ver.label());
        assert_eq!(ents(&back.model.entities, ver), ents(&d.model.entities, ver), "{}", ver.label());
        assert!(back.blocks.is_empty(), "{}: the pictures for other programs are not kept as blocks", ver.label());
    }
    let dxf = write_dxf(&d);
    let back = read_dxf(&dxf, "t").unwrap();
    assert_eq!(ents(&back.model.entities, Ver::R2004), ents(&d.model.entities, Ver::R2004));
    // another program sees (and CadForge without its data reads) the block of lines and texts
    let plain = read_dxf(&dxf.replace("CADFORGE", "OTHERAPP"), "t").unwrap();
    assert_eq!(kinds(&plain), vec!["insert", "insert"]);
    let inside: Vec<String> = plain.blocks.values().flat_map(|b| b.entities.iter().map(|e| e.type_name().to_string())).collect();
    assert!(inside.contains(&"text".to_string()) && inside.contains(&"line".to_string()) && inside.contains(&"pline".to_string()), "{inside:?}");
}

/// What DXF and DWG have no place for: named views, the last view, the current and named UCSs, the insertion base,
/// the current colour / linetype / lineweight, loaded linetypes and hatch patterns, the plot style table, an external
/// reference and the pictures' data.
fn settings_drawing() -> Drawing {
    use cadforge_core::defs::{FamilyDef, LinetypeDef, PatternDef};
    use cadforge_core::drawing::{Current, NamedView, ViewState};
    use cadforge_core::entities::{ImageData, Kind};
    let mut d = Drawing::new("settings");
    d.add_layer("Walls").unwrap();
    d.define_linetype(LinetypeDef { name: "CF_DASHDOT".into(), desc: "dash dot".into(), pattern: vec![2.0, -1.0, 0.0, -1.0] }).unwrap();
    d.define_pattern(PatternDef { name: "CF_WEAVE".into(), desc: "weave".into(), families: vec![FamilyDef { a: 30.0, ox: 0.0, oy: 0.0, dx: 0.0, dy: 2.0, dash: vec![1.0, -0.5] }] }).unwrap();
    let mut l = Entity::line(pt(0.0, 0.0), pt(100.0, 50.0)).on_layer("Walls");
    l.ltype = Some("CF_DASHDOT".into());
    d.add(l);
    d.add(Entity::hatch(vec![vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(20.0, 0.0, 0.0), Vertex::new(20.0, 10.0, 0.0), Vertex::new(0.0, 10.0, 0.0)]], "CF_WEAVE"));
    let mut px = vec![];
    for i in 0..12 {
        px.extend([20 * i as u8, 255 - 20 * i as u8, 128, 255]);
    }
    let key = d.add_image("site photo.png", cadforge_core::picture::encode_png(4, 3, px).unwrap()).unwrap();
    d.add(Entity::new(Kind::Image(ImageData::new(&key, pt(10.0, 20.0), 40.0, 30.0, 0.0))));
    d.views.insert("Front".into(), NamedView { cx: 12.5, cy: 7.25, scale: 3.5, cz: None, yaw: None, pitch: None });
    d.views.insert("Iso".into(), NamedView { cx: 1.0, cy: 2.0, scale: 0.75, cz: Some(4.0), yaw: Some(0.7), pitch: Some(0.6) });
    d.ucs = cadforge_core::geom::Ucs { origin: pt(10.0, 20.0), angle: 0.5 };
    d.ucs_named.insert("Rot30".into(), cadforge_core::geom::Ucs { origin: pt(5.0, 5.0), angle: std::f64::consts::FRAC_PI_6 });
    d.base = pt(3.0, 4.0);
    d.cur = Current { layer: "Walls".into(), color: Some(cadforge_core::style::Color::Aci(3)), ltype: Some("CF_DASHDOT".into()), lw: Some(35) };
    d.view = Some(ViewState { cx: 50.0, cy: 25.0, cz: 0.0, scale: 2.0, yaw: 0.0, pitch: std::f64::consts::FRAC_PI_2 });
    d.plot_styles = Some(std::rc::Rc::new(cadforge_core::plotstyle::PlotStyleTable::monochrome()));
    d.define_block("SITE", pt(0.0, 0.0), vec![Entity::circle(pt(0.0, 0.0), 5.0)]);
    d.blocks.get_mut("SITE").unwrap().xref = Some(cadforge_core::drawing::XrefInfo { path: "survey/site.dwg".into(), overlay: true, loaded: true });
    d.add(Entity::insert("SITE", pt(50.0, 50.0)));
    // named dimension, multileader and table styles, a current one of each, and a dimension that uses one
    let iso = cadforge_core::style::DimStyle { name: "ISO-25".into(), txt: 3.5, asz: 3.0, dec: 1, pre: "~".into(), suf: " mm".into(), ..Default::default() };
    d.settings.dim_styles.insert("ISO-25".into(), iso);
    d.settings.cur_dim_style = "ISO-25".into();
    d.settings.mleader_styles.insert("Notes".into(), cadforge_core::style::MLeaderStyle { name: "Notes".into(), text_height: 4.0, arrow: 2.0, landing: 6.0 });
    d.settings.cur_mleader_style = "Notes".into();
    d.settings.table_styles.insert("Schedule".into(), cadforge_core::style::TableStyle { name: "Schedule".into(), text_height: 3.0, title: true });
    d.settings.cur_table_style = "Schedule".into();
    let mut dim = cadforge_core::dim::DimData::new(cadforge_core::dim::DimType::Lin);
    (dim.p1, dim.p2, dim.dp, dim.ds) = (Some(pt(0.0, -10.0)), Some(pt(30.0, -10.0)), Some(pt(15.0, -20.0)), Some("ISO-25".into()));
    d.add(Entity::dim(dim));
    // a rational spline (an exact quarter circle) and a cubic one of control vertices
    let w = std::f64::consts::FRAC_1_SQRT_2;
    d.add(Entity::nurbs(cadforge_core::nurbs::Nurbs::new(2, vec![pt(60.0, 0.0), pt(60.0, 10.0), pt(50.0, 10.0)], vec![1.0, w, 1.0], vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap()));
    d.add(Entity::nurbs(cadforge_core::nurbs::Nurbs::clamped(3, vec![pt(0.0, 60.0), pt(10.0, 75.0), pt(25.0, 55.0), pt(40.0, 70.0), pt(55.0, 60.0)])));
    d
}

#[test]
fn control_vertex_splines_open_again_exactly() {
    let d = settings_drawing();
    let splines = |d: &Drawing| -> Vec<cadforge_core::nurbs::Nurbs> { d.model.entities.iter().filter_map(|e| if let cadforge_core::entities::Kind::Spline { nurbs: Some(n), .. } = &e.kind { Some((**n).clone()) } else { None }).collect() };
    let want = splines(&d);
    assert_eq!(want.len(), 2);
    for (ver, _) in VERSIONS {
        assert_eq!(splines(&read_dwg(&write_dwg_as(&d, ver).unwrap(), "s").unwrap()), want, "{}", ver.label());
    }
    // (plain DXF writes 15 significant digits)
    let dxf = splines(&read_dxf(&write_dxf(&d), "s").unwrap());
    assert_eq!(rounded(&dxf), rounded(&want), "DXF");
}

#[test]
fn drawing_settings_and_pictures_open_again() {
    let d = settings_drawing();
    let state = |d: &Drawing| {
        let line = d.model.entities.iter().find(|e| e.type_name() == "line").map(|e| e.ltype.clone());
        let hatch = d.model.entities.iter().find_map(|e| if let cadforge_core::entities::Kind::Hatch(h) = &e.kind { Some(h.pattern.clone()) } else { None });
        let pictures: Vec<_> = d.images.values().map(|p| (p.path.clone(), p.w, p.h, p.data.as_ref().map(|b| b.0.as_ref().clone()))).collect();
        let xrefs: Vec<_> = d.blocks.iter().map(|(k, b)| (k.clone(), rounded(&b.xref))).collect();
        let dim_style = d.model.entities.iter().find_map(|e| if let cadforge_core::entities::Kind::Dim(x) = &e.kind { Some(x.ds.clone()) } else { None });
        format!(
            "{} | {} | {} | {} | {} | {} | {} | {} | {} | {line:?} {hatch:?} | {pictures:?} | {xrefs:?} | {} {} {} {} {} {} | {dim_style:?}",
            rounded(&d.views),
            rounded(&d.view),
            rounded(&d.ucs),
            rounded(&d.ucs_named),
            rounded(&(d.base.x, d.base.y)),
            rounded(&d.cur),
            rounded(&d.custom_ltypes),
            rounded(&d.custom_patterns),
            rounded(&d.plot_styles),
            rounded(&d.settings.dim_styles),
            d.settings.cur_dim_style,
            rounded(&d.settings.mleader_styles),
            d.settings.cur_mleader_style,
            rounded(&d.settings.table_styles),
            d.settings.cur_table_style,
        )
    };
    for (ver, _) in VERSIONS {
        let back = read_dwg(&write_dwg_as(&d, ver).unwrap(), "s").unwrap();
        assert_eq!(state(&back), state(&d), "{}", ver.label());
    }
    assert_eq!(state(&read_dxf(&write_dxf(&d), "s").unwrap()), state(&d), "DXF");
}

/// Objects over 32 KB (a four-byte size), more than one page of the object map, many layers and blocks.
#[test]
fn large_objects_and_many_handles() {
    let mut d = Drawing::new("big");
    let ring: Vec<Vertex> = (0..4000).map(|i| {
        let a = i as f64 / 4000.0 * std::f64::consts::TAU;
        Vertex::new(100.0 * a.cos(), 100.0 * a.sin(), if i % 7 == 0 { 0.1 } else { 0.0 })
    }).collect();
    d.add(Entity::pline(ring, true));
    for i in 0..3000 {
        let x = i as f64;
        d.add(Entity::line(pt(x, 0.0), pt(x + 0.5, 10.0 + (i % 13) as f64)));
    }
    for i in 0..40 {
        let name = format!("L{i}");
        d.add_layer(&name).unwrap();
        d.define_block(&format!("B{i}"), pt(0.0, 0.0), vec![Entity::circle(pt(0.0, 0.0), 1.0 + i as f64).on_layer(&name)]);
        d.add(Entity::insert(&format!("B{i}"), pt(i as f64 * 5.0, -20.0)).on_layer(&name));
    }
    let dwg = same_as_dxf(&d, "big");
    assert!(dwg.len() > 100_000);
}

fn checked_files() -> Vec<(String, Vec<u8>)> {
    let mut drawings = vec![("sample".to_string(), sample_drawing()), ("dims".to_string(), dims_drawing().0), ("solids".to_string(), solids_drawing()), ("tables".to_string(), tables_drawing()), ("settings".to_string(), settings_drawing())];
    for f in ["kitchen-R2018.dxf", "annotation-R2018.dxf", "image-R2018.dxf", "pline-widths-R2018.dxf"] {
        drawings.push((f.to_string(), read_dxf_bytes(&std::fs::read(repo(&format!("tests/dxf/corpus/{f}"))).unwrap(), f).unwrap()));
    }
    drawings.push(("sample_AC1018".into(), read_dwg(&std::fs::read(repo("tests/dwg/samples/sample_AC1018.dwg")).unwrap(), "x").unwrap()));
    let mut out = vec![];
    for (name, d) in &drawings {
        for (ver, _) in VERSIONS {
            out.push((format!("{name}-{}", ver.label()), write_dwg_as(d, ver).unwrap()));
        }
    }
    out
}

#[test]
fn acadsharp_reads_our_files_with_crc_checks() {
    let Ok(tool) = std::env::var("DWG_ACADSHARP") else { return };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    for (name, bytes) in checked_files() {
        let p = dir.join(format!("acs-{name}.dwg"));
        std::fs::write(&p, &bytes).unwrap();
        let out = Command::new(&tool).arg("read").arg(&p).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success() && (text.contains("version AC1018") || text.contains("version AC1015")), "{name}: {text}");
        assert!(!text.contains("[Error]") && !text.contains("[Warning]") && !text.contains("Exception"), "{name}: {text}");
    }
}

#[test]
fn libredwg_decodes_our_files_without_complaint() {
    let Ok(bin) = std::env::var("LIBREDWG_BIN") else { return };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    for (name, bytes) in checked_files() {
        let p = dir.join(format!("lib-{name}.dwg"));
        std::fs::write(&p, &bytes).unwrap();
        let out = Command::new(PathBuf::from(&bin).join("dwgread")).arg("-v3").arg(&p).output().unwrap();
        let text = String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
        // LibreDWG prints these for AutoCAD's own 2004 files too: its page map checksum and section count checks do
        // not match what AutoCAD writes, and it notes the classes it only partly supports ("unstable")
        let quirk = |l: &str| l.contains("handle not found") || (l.contains("crc mismatch") && l.contains("checksum")) || l.contains("invalid sections") || l.contains("unstable class");
        let bad: Vec<&str> = text
            .lines()
            .filter(|l| {
                let l = l.to_lowercase();
                (l.contains("error") || l.contains("invalid") || l.contains("overflow") || l.contains("mismatch") || l.contains("warning")) && !quirk(&l)
            })
            .collect();
        assert!(bad.is_empty(), "{name}: {bad:?}");
    }
}
