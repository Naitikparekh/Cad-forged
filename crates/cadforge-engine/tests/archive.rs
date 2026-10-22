//! ARCHIVE and ETRANSMIT: a ZIP of the drawing, its pictures and a report, handed to the host.
use cadforge_core::drawing::{Block, XrefInfo};
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::picture::encode_png;
use cadforge_engine::app::Event;
use cadforge_engine::sim::Sim;
use std::process::Command;

fn zip_event(s: &Sim) -> Option<(String, Vec<u8>)> {
    s.h.borrow_mut().take_events().into_iter().find_map(|e| if let Event::SaveFile { name, data, mime } = e { (mime == "application/zip").then_some((name, data)) } else { None })
}

fn names(zip: &[u8]) -> Vec<String> {
    // read the central directory ourselves: signature PK\x01\x02, name length at +28, name at +46
    let mut out = vec![];
    let mut i = 0;
    while i + 46 <= zip.len() {
        if &zip[i..i + 4] == b"PK\x01\x02" {
            let n = u16::from_le_bytes([zip[i + 28], zip[i + 29]]) as usize;
            out.push(String::from_utf8_lossy(&zip[i + 46..i + 46 + n]).to_string());
            i += 46 + n;
        } else {
            i += 1;
        }
    }
    out
}

fn with_content() -> Sim {
    let s = Sim::new();
    s.add(Entity::line(pt(0., 0.), pt(10., 5.)));
    let mut px = vec![];
    for _ in 0..4 {
        px.extend([200u8, 30, 30, 255]);
    }
    let png = encode_png(2, 2, px).unwrap();
    s.doc_mut(|d| {
        d.add_image("site/photo.png", png).unwrap();
        d.blocks.insert("Site".into(), Block { name: "Site".into(), base: pt(0., 0.), entities: vec![], xref: Some(XrefInfo { path: "site.dwg".into(), overlay: false, loaded: false }) });
    });
    s
}

#[test]
fn archive_packs_the_drawing_the_pictures_and_a_report() {
    let s = with_content();
    s.cmd("ARCHIVE");
    let (name, data) = zip_event(&s).unwrap_or_else(|| panic!("no zip: {}", s.log_text()));
    assert_eq!(name, "Drawing1-archive.zip");
    let n = names(&data);
    assert_eq!(n, vec!["Drawing1.cadforge.json", "Drawing1.dxf", "pictures/photo.png", "transmittal.txt"], "{n:?}");
    assert!(s.history_has("Archive: 4 files in Drawing1-archive.zip"), "{}", s.log_text());
}

#[test]
fn etransmit_makes_the_same_package_and_the_report_names_what_is_missing() {
    let s = with_content();
    s.cmd("ETRANSMIT");
    let (name, data) = zip_event(&s).unwrap();
    assert_eq!(name, "Drawing1-transmittal.zip");
    // the report is stored uncompressed, so its text is visible in the archive
    let text = String::from_utf8_lossy(&data).to_string();
    assert!(text.contains("Site -> site.dwg"), "the external reference is listed");
    assert!(text.contains("referenced drawings are not copied"));
    assert!(text.contains("site/photo.png"));
}

#[test]
fn the_zip_is_valid_for_other_tools() {
    let s = with_content();
    s.cmd("ARCHIVE");
    let (_, data) = zip_event(&s).unwrap();
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("archive-test.zip");
    std::fs::write(&path, &data).unwrap();
    let script = "import sys, zipfile\nz = zipfile.ZipFile(sys.argv[1])\nassert z.testzip() is None\nj = z.read('Drawing1.cadforge.json')\nassert b'\"cadforge\"' in j\nprint(sorted(z.namelist()))";
    match Command::new("python3").args(["-I", "-c", script, path.to_str().unwrap()]).output() {
        Ok(o) => {
            assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            assert!(String::from_utf8_lossy(&o.stdout).contains("pictures/photo.png"));
        }
        Err(_) => eprintln!("python3 not available; skipping"),
    }
}

#[test]
fn a_picture_without_its_file_is_reported_not_packed() {
    let s = Sim::new();
    s.doc_mut(|d| {
        d.images.insert("gone.png".into(), cadforge_core::picture::ImageDef::missing("maps/gone.png", 100, 50));
    });
    s.cmd("ARCHIVE");
    let (_, data) = zip_event(&s).unwrap();
    assert!(!names(&data).iter().any(|n| n.starts_with("pictures/")));
    assert!(String::from_utf8_lossy(&data).contains("maps/gone.png (the file is not available"));
}
