//! -SAVEAS and the save formats: a drawing saved as AutoCAD 2004 / 2000 DWG, DXF or a CadForge project reads back with
//! the same objects, and the drawing remembers its DWG version for the next save.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::dwg::{write_dwg_as, Ver};
use cadforge_core::io::{read_dwg, read_dxf_bytes};
use cadforge_engine::app::Event;
use cadforge_engine::commands::ext_system::{with_ext, SaveFormat};
use cadforge_engine::runtime::{ArgVal, Args};
use cadforge_engine::sim::Sim;

fn file(s: &Sim) -> Option<(String, Vec<u8>, String)> {
    s.h.borrow_mut().take_events().into_iter().find_map(|e| if let Event::SaveFile { name, data, mime } = e { Some((name, data, mime)) } else { None })
}

fn drawing() -> Sim {
    let s = Sim::new();
    s.add(Entity::line(pt(0., 0.), pt(10., 5.)));
    s.add(Entity::circle(pt(3., 4.), 2.5));
    s
}

fn kinds(d: &cadforge_core::drawing::Drawing) -> Vec<&'static str> {
    let mut k: Vec<&'static str> = d.model.entities.iter().map(|e| e.type_name()).collect();
    k.sort();
    k
}

#[test]
fn the_default_is_an_autocad_2004_dwg() {
    let s = drawing();
    s.cmd("-SAVEAS");
    assert!(s.prompt().contains("2000/2004/DXF/CadForge"), "{}", s.prompt());
    s.enter();
    assert!(s.prompt().contains("Drawing1.dwg"), "{}", s.prompt());
    s.ty("plan").enter();
    let (name, data, mime) = file(&s).unwrap_or_else(|| panic!("no file: {}", s.log_text()));
    assert_eq!((name.as_str(), mime.as_str()), ("plan.dwg", "image/vnd.dwg"));
    assert!(data.starts_with(b"AC1018"));
    let back = read_dwg(&data, "plan").unwrap();
    assert_eq!(kinds(&back), vec!["circle", "line"]);
    s.doc(|d| {
        assert_eq!(d.file_name.as_deref(), Some("plan.dwg"));
        assert_eq!(d.name, "plan");
        assert!(!d.dirty);
        assert_eq!(SaveFormat::of("plan.dwg", d), SaveFormat::Dwg2004);
    });
}

#[test]
fn autocad_2000_is_remembered_for_the_next_save() {
    let s = drawing();
    s.cmd_args("-SAVEAS", Args::none().with("format", ArgVal::Str("2000".into())).with("name", ArgVal::Str("old.dxf".into())));
    let (name, data, _) = file(&s).expect("file");
    assert_eq!(name, "old.dwg", "the extension follows the format");
    assert!(data.starts_with(b"AC1015"));
    assert_eq!(kinds(&read_dwg(&data, "old").unwrap()), vec!["circle", "line"]);
    s.doc(|d| assert_eq!(SaveFormat::of(d.file_name.as_deref().unwrap(), d), SaveFormat::Dwg2000));
    // QSAVE's format choice for the file: saving again keeps AutoCAD 2000
    s.cmd_args("-SAVEAS", Args::none().with("name", ArgVal::Str("old".into())));
    s.enter();
    let (_, again, _) = file(&s).expect("second file");
    assert!(again.starts_with(b"AC1015"), "{}", s.log_text());
}

#[test]
fn dxf_and_cadforge_projects() {
    let s = drawing();
    s.cmd_args("-SAVEAS", Args::none().with("format", ArgVal::Str("DXF".into())).with("name", ArgVal::Str("a".into())));
    let (name, data, mime) = file(&s).expect("dxf");
    assert_eq!((name.as_str(), mime.as_str()), ("a.dxf", "application/dxf"));
    assert_eq!(kinds(&read_dxf_bytes(&data, "a").unwrap()), vec!["circle", "line"]);
    s.cmd_args("-SAVEAS", Args::none().with("format", ArgVal::Str("CadForge".into())).with("name", ArgVal::Str("a.dxf".into())));
    let (name, data, _) = file(&s).expect("json");
    assert_eq!(name, with_ext("a", SaveFormat::CadForge));
    assert_eq!(kinds(&cadforge_core::io::from_json(&String::from_utf8(data).unwrap(), "a").unwrap()), vec!["circle", "line"]);
}

#[test]
fn an_opened_autocad_2000_file_saves_as_2000() {
    let mut d = cadforge_core::drawing::Drawing::new("x");
    d.add(Entity::line(pt(0., 0.), pt(1., 1.)));
    for (ver, want) in [(Ver::R2000, SaveFormat::Dwg2000), (Ver::R2004, SaveFormat::Dwg2004)] {
        let back = read_dwg(&write_dwg_as(&d, ver).unwrap(), "x").unwrap();
        assert_eq!(SaveFormat::of("x.dwg", &back), want);
    }
    for (f, ext) in [(SaveFormat::Dwg2004, ".dwg"), (SaveFormat::Dwg2000, ".dwg"), (SaveFormat::Dxf, ".dxf")] {
        assert_eq!(with_ext("site plan.json", f), format!("site plan{ext}"));
    }
}
