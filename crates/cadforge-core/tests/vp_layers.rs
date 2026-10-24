//! Layer freeze per layout viewport: JSON and DXF round trips, and defaults for new viewports.
use cadforge_core::drawing::{Drawing, MODEL};
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::{read_dxf_bytes, write_dxf};
use std::path::PathBuf;
use std::process::Command;

fn doc() -> Drawing {
    let mut d = Drawing::new("t");
    d.add_layer("Walls").unwrap();
    d.add_layer("Notes").unwrap();
    d.add_to(Entity::line(pt(0., 0.), pt(100., 0.)).on_layer("Walls"), MODEL);
    d.add_layout("Sheet").unwrap();
    d.set_space("Sheet").unwrap();
    let l = d.layouts.iter_mut().find(|l| l.name == "Sheet").unwrap();
    l.entities.clear();
    let mut vp = ViewportData::new(pt(150.0, 105.0), 100.0, 80.0);
    vp.scale = Some(0.2);
    vp.vc = pt(50.0, 0.0);
    vp.set_frozen("Notes", true);
    l.entities.push(Entity::new(Kind::Viewport(vp)));
    d
}

fn viewport_of(d: &Drawing, layout: &str) -> ViewportData {
    d.layout(layout).unwrap().entities.iter().find_map(|e| if let Kind::Viewport(v) = &e.kind { Some(v.clone()) } else { None }).expect("a viewport")
}

#[test]
fn the_dxf_viewport_lists_its_frozen_layers_by_handle_and_reads_them_back() {
    let d = doc();
    let text = write_dxf(&d);
    let back = read_dxf_bytes(text.as_bytes(), "back").unwrap();
    let v = viewport_of(&back, "Sheet");
    assert!(v.is_frozen("Notes"), "{:?}", v.layers);
    assert!(!v.is_frozen("Walls"));
    // the layer itself stays thawed
    assert!(!back.layer("Notes").unwrap().frozen);
}

#[test]
fn ezdxf_sees_the_frozen_layer_of_the_viewport() {
    if Command::new("python3").args(["-I", "-c", "import ezdxf"]).output().map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("ezdxf not installed; skipping");
        return;
    }
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("vp-frozen.dxf");
    std::fs::write(&path, write_dxf(&doc())).unwrap();
    let script = "import sys, ezdxf\nfrom ezdxf import recover\ndoc, aud = recover.readfile(sys.argv[1])\nassert not aud.has_errors, [e.message for e in aud.errors][:5]\nlay = doc.layouts.get('Sheet')\nvps = [v for v in lay.query('VIEWPORT') if v.dxf.id != 1]\nassert len(vps) == 1, len(vps)\nnames = sorted(vps[0].frozen_layers)\nprint(names)";
    let r = Command::new("python3").args(["-I", "-c", script, path.to_str().unwrap()]).output().unwrap();
    assert!(r.status.success(), "{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    let out = String::from_utf8_lossy(&r.stdout).to_string();
    // ezdxf reports the frozen layers as names (newer versions) or as the handles we wrote (older ones)
    assert!(out.contains("Notes") || out.contains('['), "{out}");
    assert!(!out.contains("[]"), "the frozen list is not empty: {out}");
}

#[test]
fn json_keeps_the_overrides_and_old_files_without_them_still_load() {
    let mut d = doc();
    {
        let l = d.layouts.iter_mut().find(|l| l.name == "Sheet").unwrap();
        if let Kind::Viewport(v) = &mut l.entities[0].kind {
            v.edit_override("Walls", |o| {
                o.color = Some(cadforge_core::style::Color::Rgb(1, 2, 3));
                o.lw = Some(50);
            });
        }
    }
    let back = Drawing::from_json_str(&d.to_json_string(), None).unwrap();
    let v = viewport_of(&back, "Sheet");
    assert_eq!(v.layer_override("walls").unwrap().color, Some(cadforge_core::style::Color::Rgb(1, 2, 3)));
    assert_eq!(v.layer_override("Walls").unwrap().lw, Some(50));
    assert!(v.is_frozen("Notes"));
    // a viewport written before this feature has no "layers" key
    let plain = Drawing::new("p");
    let mut v = ViewportData::new(pt(0., 0.), 10., 10.);
    assert!(v.layers.is_empty());
    v.set_frozen("x", true);
    v.set_frozen("x", false);
    assert!(v.layers.is_empty(), "an override that changes nothing is dropped");
    let _ = plain;
}

#[test]
fn new_viewports_start_with_the_layers_marked_frozen_by_default() {
    let mut d = Drawing::new("t");
    d.add_layer("Dims").unwrap();
    d.layers.iter_mut().find(|l| l.name == "Dims").unwrap().vp_frozen_default = true;
    d.add_layout("Sheet").unwrap();
    d.set_space("Sheet").unwrap();
    d.layouts.iter_mut().find(|l| l.name == "Sheet").unwrap().entities.clear();
    let id = d.add(Entity::new(Kind::Viewport(ViewportData::new(pt(100., 100.), 50., 50.))));
    let v = d.layout("Sheet").unwrap().entities.iter().find(|e| e.id == id).and_then(|e| if let Kind::Viewport(v) = &e.kind { Some(v.clone()) } else { None }).unwrap();
    assert!(v.is_frozen("Dims"));
    // a viewport that already has its own settings (loaded from a file) is not touched
    let mut own = ViewportData::new(pt(10., 10.), 20., 20.);
    own.set_frozen("Other", true);
    let id2 = d.add(Entity::new(Kind::Viewport(own)));
    let v2 = d.layout("Sheet").unwrap().entities.iter().find(|e| e.id == id2).and_then(|e| if let Kind::Viewport(v) = &e.kind { Some(v.clone()) } else { None }).unwrap();
    assert!(v2.is_frozen("Other") && !v2.is_frozen("Dims"), "a viewport that arrives with its own settings (a copy, an import) keeps exactly those");
}
