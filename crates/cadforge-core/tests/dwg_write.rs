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

fn canon(e: &Entity) -> String {
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
    let mut e = e.clone();
    e.id = 0;
    e.grp = None;
    let mut j = serde_json::to_value(&e).unwrap();
    round(&mut j);
    j.to_string()
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
            assert_eq!(serde_json::to_string(&la.paper).unwrap(), serde_json::to_string(&lb.paper).unwrap(), "{what}: sheet of {}", la.name);
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
        assert_eq!(serde_json::to_string(&a.settings).unwrap(), serde_json::to_string(&b.settings).unwrap(), "{what}: settings");
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
    assert!(back.model.entities.len() >= 30);
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
    let mut drawings = vec![("sample".to_string(), sample_drawing())];
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
        // LibreDWG prints these two for AutoCAD's own 2004 files too: its page map checksum and section count checks
        // do not match what AutoCAD writes
        let quirk = |l: &str| l.contains("handle not found") || (l.contains("crc mismatch") && l.contains("checksum")) || l.contains("invalid sections");
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
