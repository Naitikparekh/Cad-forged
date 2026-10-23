//! -DXFOUT: the drawing as an ASCII or a binary DXF file, handed to the host.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::{is_binary_dxf, read_dxf_bytes};
use cadforge_engine::app::Event;
use cadforge_engine::runtime::ArgVal;
use cadforge_engine::runtime::Args;
use cadforge_engine::sim::Sim;

fn file(s: &Sim) -> Option<(String, Vec<u8>)> {
    s.h.borrow_mut().take_events().into_iter().find_map(|e| if let Event::SaveFile { name, data, mime } = e { (mime == "application/dxf").then_some((name, data)) } else { None })
}

fn drawing() -> Sim {
    let s = Sim::new();
    s.add(Entity::line(pt(0., 0.), pt(10., 5.)));
    s.add(Entity::circle(pt(3., 4.), 2.5));
    s
}

fn lines_of(bytes: &[u8]) -> Vec<(Pt, Pt)> {
    read_dxf_bytes(bytes, "x").unwrap().model.entities.iter().filter_map(|e| if let Kind::Line { a, b } = &e.kind { Some((*a, *b)) } else { None }).collect()
}

#[test]
fn binary_is_chosen_at_the_prompt_and_the_file_reads_back() {
    let s = drawing();
    s.cmd("-DXFOUT");
    assert!(s.prompt().contains("ASCII/Binary"), "{}", s.prompt());
    s.ty("Binary").enter();
    let (name, data) = file(&s).unwrap_or_else(|| panic!("no file: {}", s.log_text()));
    assert_eq!(name, "Drawing1.dxf");
    assert!(is_binary_dxf(&data));
    assert_eq!(lines_of(&data), vec![(pt(0., 0.), pt(10., 5.))]);
    assert!(s.history_has("binary DXF"), "{}", s.log_text());
}

#[test]
fn the_default_is_a_text_dxf() {
    let s = drawing();
    s.cmd("-DXFOUT").enter();
    let (_, data) = file(&s).expect("file");
    assert!(!is_binary_dxf(&data));
    assert!(String::from_utf8(data.clone()).unwrap().contains("SECTION"));
    assert_eq!(lines_of(&data).len(), 1);
}

#[test]
fn the_menu_passes_the_format_as_an_argument() {
    let s = drawing();
    s.cmd_args("-DXFOUT", Args::none().with("format", ArgVal::Str("Binary".into())));
    let (_, data) = file(&s).expect("file");
    assert!(is_binary_dxf(&data));
    assert!(!s.busy(), "no prompt when the format is given");
}
