//! Interoperability: DXF files written by ezdxf (tests/dxf/make-corpus.py) in every DXF version.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::{read_dxf_bytes, write_dxf};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/dxf/corpus").join(name)
}

fn load(name: &str) -> Drawing {
    let bytes = std::fs::read(corpus(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    read_dxf_bytes(&bytes, name).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn counts(d: &Drawing) -> BTreeMap<&'static str, usize> {
    let mut c = BTreeMap::new();
    for e in &d.model.entities {
        *c.entry(e.type_name()).or_default() += 1;
    }
    c
}

const VERSIONS: [&str; 6] = ["R2000", "R2004", "R2007", "R2010", "R2013", "R2018"];

#[test]
fn every_dxf_version_yields_the_same_geometry() {
    for v in VERSIONS {
        let d = load(&format!("kitchen-{v}.dxf"));
        let c = counts(&d);
        for (ty, n) in [("circle", 1), ("arc", 2), ("ellipse", 2), ("spline", 2), ("point", 1), ("xline", 1), ("ray", 1), ("leader", 1), ("pline", 2)] {
            assert_eq!(c.get(ty).copied().unwrap_or(0), n, "{v}: {ty} count in {c:?}");
        }
        // gradient hatches need R2004; the solid and trace become solid hatches
        assert_eq!(c["hatch"], if v == "R2000" { 5 } else { 6 }, "{v}: {c:?}");
        assert!(c["line"] >= 5 && c["text"] >= 3, "{v}: {c:?}");
        assert!(d.skipped.is_empty(), "{v}: skipped {:?}", d.skipped);
    }
}

#[test]
fn layers_keep_their_state_and_colours() {
    let d = load("kitchen-R2018.dxf");
    let l = |n: &str| d.layer(n).unwrap_or_else(|| panic!("layer {n}"));
    assert!(!l("Off").on, "negative colour = layer off");
    assert!(l("Frozen").frozen);
    assert_eq!(l("Rgb").color, cadforge_core::style::Color::Rgb(255, 128, 0));
    assert_eq!(l("Centre").ltype, "CENTER");
    assert_eq!(l("Outline").lw, 50);
    // an entity with a true colour keeps it
    let rgb = d.model.entities.iter().find(|e| e.layer == "Rgb").expect("entity on layer Rgb");
    assert_eq!(rgb.color, Some(cadforge_core::style::Color::Rgb(10, 200, 90)));
}

#[test]
fn text_is_decoded_from_ansi_and_utf8_files() {
    for v in ["R2000", "R2018"] {
        let d = load(&format!("kitchen-{v}.dxf"));
        let t = d.model.entities.iter().find_map(|e| match &e.kind {
            Kind::Text(t) if t.s.contains("Hello") => Some(t.clone()),
            _ => None,
        });
        let t = t.unwrap_or_else(|| panic!("{v}: text"));
        assert_eq!(t.s, "Hello \u{D8} DXF", "{v}");
        near(deg(t.rot), 20.0);
        assert_eq!(t.jh, cadforge_core::text::HJust::Center);
    }
    // MTEXT formatting codes and %% symbols
    let d = load("kitchen-R2018.dxf");
    let m = d.model.entities.iter().find_map(|e| match &e.kind {
        Kind::Text(t) if t.multi => Some(t.s.clone()),
        _ => None,
    });
    let m = m.expect("mtext");
    assert!(m.contains("Bold") && m.contains("Line two") && m.contains('\u{B0}'), "{m:?}");
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "expected {b}, got {a}");
}

#[test]
fn block_attributes_arrive_with_their_values() {
    let d = load("kitchen-R2013.dxf");
    assert!(d.blocks.contains_key("TAG") && d.blocks.contains_key("NESTED"));
    let def = &d.blocks["TAG"];
    let attdef = def.entities.iter().find_map(|e| match &e.kind {
        Kind::Text(t) => t.att.clone(),
        _ => None,
    });
    assert_eq!(attdef.expect("ATTDEF").tag, "NUMBER");
    let mut values: Vec<String> = d
        .model
        .entities
        .iter()
        .filter_map(|e| match &e.kind {
            Kind::Insert(i) if i.name == "TAG" => Some(i.attribs.iter().map(|a| a.t.s.clone()).collect::<Vec<_>>().join(",")),
            _ => None,
        })
        .collect();
    values.sort();
    assert_eq!(values, vec!["17", "18"], "{values:?}");
    let scaled = d.model.entities.iter().find_map(|e| match &e.kind {
        Kind::Insert(i) if i.name == "TAG" && i.sx != 1.0 => Some(i.clone()),
        _ => None,
    });
    let i = scaled.expect("scaled insert");
    near(i.sx, 1.5);
    near(deg(i.rot), 30.0);
}

#[test]
fn layouts_viewports_and_sheet_sizes() {
    for v in ["R2010", "R2018"] {
        let d = load(&format!("kitchen-{v}.dxf"));
        assert_eq!(d.layout_names(), vec!["Layout1", "Detail"], "{v}");
        let l1 = d.layout("Layout1").unwrap();
        near(l1.paper.w, 420.0);
        near(l1.paper.h, 297.0);
        let vps: Vec<&ViewportData> = l1.entities.iter().filter_map(|e| if let Kind::Viewport(v) = &e.kind { Some(v) } else { None }).collect();
        assert_eq!(vps.len(), 1, "{v}: the sheet's own viewport is not a viewport");
        near(vps[0].c.x, 210.0);
        near(vps[0].w, 300.0);
        near(vps[0].scale.expect("scale"), 200.0 / 120.0);
        near(vps[0].vc.x, 100.0);
        near(vps[0].vc.y, 60.0);
        assert!(l1.entities.iter().any(|e| matches!(&e.kind, Kind::Text(t) if t.s == "TITLE BLOCK")));
        let det = d.layout("Detail").unwrap();
        near(det.paper.w, 297.0);
        assert_eq!(det.entities.iter().filter(|e| matches!(e.kind, Kind::Viewport(_))).count(), 2, "{v}");
    }
}

#[test]
fn modern_annotation_and_misc_entities() {
    let d = load("annotation-R2018.dxf");
    let c = counts(&d);
    assert_eq!(c.get("leader").copied(), Some(1), "{c:?}");
    assert_eq!(c.get("wipeout").copied(), Some(1), "{c:?}");
    assert_eq!(c.get("spline").copied(), Some(1), "the helix is read as a spline");
    // MLINE (two parallel polylines sharing a group), 3DFACE and POLYLINE 3D outlines
    assert_eq!(c.get("pline").copied(), Some(4), "{c:?}");
    let leader = d.model.entities.iter().find_map(|e| match &e.kind {
        Kind::Leader { pts, s, h } => Some((pts.clone(), s.clone(), *h)),
        _ => None,
    });
    let (pts, text, h) = leader.expect("leader");
    assert_eq!(text, "Note: see detail");
    near(h, 3.0);
    assert!(pts.len() >= 2 && dist(pts[0], pt(10.0, 10.0)) < 1e-6, "{pts:?}");
    let w = d.model.entities.iter().find_map(|e| match &e.kind {
        Kind::Wipeout(w) => Some(w.pts.clone()),
        _ => None,
    });
    let b = Bounds::of_points(&w.expect("wipeout"));
    near(b.min_x, 60.0);
    near(b.max_x, 80.0);
    near(b.max_y, 20.0);
    let grouped: Vec<_> = d.model.entities.iter().filter(|e| e.grp.is_some()).collect();
    assert_eq!(grouped.len(), 2, "MLINE elements share a group");
}

#[test]
fn header_variables_are_applied() {
    let d = load("header-R2010.dxf");
    assert_eq!(d.settings.units.ins_units, 1);
    near(d.settings.ltscale, 2.5);
    assert_eq!(d.settings.pdmode, 35);
    near(d.settings.pdsize, 3.0);
}

#[test]
fn hatches_keep_islands_gradients_and_edge_paths() {
    let d = load("kitchen-R2018.dxf");
    let hatches: Vec<&HatchData> = d.model.entities.iter().filter_map(|e| if let Kind::Hatch(h) = &e.kind { Some(h) } else { None }).collect();
    assert!(hatches.iter().any(|h| h.pattern == "ANSI31" && h.loops.len() == 2), "pattern hatch with an island");
    assert!(hatches.iter().filter(|h| h.pattern == "SOLID").count() >= 3);
    let g = hatches.iter().find_map(|h| h.gradient.clone());
    // gradient fills arrive as gradients (not plain solids) once the reader keeps them
    assert!(g.is_some(), "gradient hatch");
    let g = g.unwrap();
    assert_eq!(g.c1, [255, 0, 0]);
    assert_eq!(g.c2, [0, 0, 255]);
}

fn have_ezdxf() -> bool {
    Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| o.status.success()).unwrap_or(false)
}

#[test]
fn corpus_drawings_survive_our_writer_and_pass_the_ezdxf_audit() {
    if !have_ezdxf() {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    for name in ["kitchen-R2000.dxf", "kitchen-R2010.dxf", "kitchen-R2018.dxf", "annotation-R2018.dxf"] {
        let d = load(name);
        let out = tmp.join(format!("rt-{name}"));
        std::fs::write(&out, write_dxf(&d)).unwrap();
        let script = "import sys, ezdxf\nfrom ezdxf import recover\ndoc, aud = recover.readfile(sys.argv[1])\nassert not aud.has_errors, [e.message for e in aud.errors][:5]\nprint(len(doc.modelspace()), len(list(doc.layouts)))";
        let r = Command::new("python3").args(["-I", "-c", script, out.to_str().unwrap()]).output().unwrap();
        assert!(r.status.success(), "{name}: {}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
        // and back through our reader without losing the layouts
        let back = cadforge_core::io::read_dxf(&write_dxf(&d), name).unwrap();
        assert_eq!(back.layout_names().len(), d.layout_names().len(), "{name}: layouts survive the round trip");
    }
}

#[test]
fn our_writer_keeps_sheet_sizes_viewports_and_gradients() {
    let d = load("kitchen-R2018.dxf");
    let back = cadforge_core::io::read_dxf(&write_dxf(&d), "rt").unwrap();
    assert_eq!(back.layout_names(), d.layout_names());
    for name in d.layout_names() {
        let (a, b) = (d.layout(&name).unwrap(), back.layout(&name).unwrap());
        near(a.paper.w, b.paper.w);
        near(a.paper.h, b.paper.h);
        let vps = |l: &cadforge_core::drawing::Layout| -> Vec<ViewportData> { l.entities.iter().filter_map(|e| if let Kind::Viewport(v) = &e.kind { Some(v.clone()) } else { None }).collect() };
        let (va, vb) = (vps(a), vps(b));
        assert_eq!(va.len(), vb.len(), "{name}: viewport count");
        for (x, y) in va.iter().zip(&vb) {
            near(x.c.x, y.c.x);
            near(x.c.y, y.c.y);
            near(x.w, y.w);
            near(x.h, y.h);
            near(x.scale.unwrap_or(0.0), y.scale.unwrap_or(0.0));
            assert_eq!(x.locked, y.locked, "{name}: lock");
        }
        assert_eq!(a.entities.len(), b.entities.len(), "{name}: entity count");
    }
    let grads = |d: &Drawing| d.model.entities.iter().filter(|e| matches!(&e.kind, Kind::Hatch(h) if h.gradient.is_some())).count();
    assert!(grads(&d) > 0);
    assert_eq!(grads(&d), grads(&back), "gradient hatches survive");
}

#[test]
fn raster_images_arrive_with_clip_and_adjustments() {
    let d = load("image-R2018.dxf");
    let Kind::Image(i) = &d.model.entities[0].kind else { panic!("{:?}", d.model.entities) };
    near(i.p.x, 100.0);
    near(i.p.y, 50.0);
    near(i.size().0, 80.0);
    near(i.size().1, 60.0);
    assert_eq!((i.brightness, i.contrast, i.fade), (70, 30, 15));
    // the boundary is given in pixels from the top-left corner: (100,100) is 1/8 across and 1/6 down
    assert_eq!(i.clip.len(), 3);
    let (a, c) = (i.clip[0], i.clip[2]);
    near(a.x, 100.0 + 80.0 * (100.5 / 800.0));
    near(a.y, 50.0 + 60.0 * (1.0 - 100.5 / 600.0));
    near(c.x, 100.0 + 80.0 * (400.5 / 800.0));
    let def = &d.images[&i.name];
    assert_eq!((def.w, def.h), (800, 600));
    assert_eq!(def.path, "site/aerial photo.jpg");
    assert!(def.is_missing());
}
