//! PUBLISH: all sheets of a drawing in one multi-page PDF.
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::app::Event;
use cadforge_engine::sim::Sim;

fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}

fn pdf(s: &Sim) -> Option<(String, Vec<u8>)> {
    s.h.borrow_mut().take_events().into_iter().find_map(|e| {
        if let Event::SaveFile { name, data, mime } = e {
            (mime == "application/pdf").then_some((name, data))
        } else {
            None
        }
    })
}

fn pages(data: &[u8]) -> usize {
    String::from_utf8_lossy(data)
        .matches("/Type /Page ")
        .count()
}

fn with_two_layouts() -> Sim {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(100., 50.)));
    s.add(Entity::circle(p(50., 25.), 20.));
    s.cmd("LAYOUT").ty("New").ty("Sheet A");
    s.cmd("LAYOUT").ty("New").ty("Sheet B");
    s.h.borrow_mut().take_events();
    s
}

#[test]
fn publish_writes_one_pdf_page_per_sheet() {
    let s = with_two_layouts();
    let n_layouts = s.doc(|d| d.layout_names().len());
    assert!(n_layouts >= 2);
    s.cmd("PUBLISH").ty("All");
    let (name, data) = pdf(&s).unwrap_or_else(|| panic!("no PDF: {}", s.log_text()));
    assert!(name.ends_with(".pdf"), "{name}");
    assert!(data.starts_with(b"%PDF-1.4"));
    assert_eq!(
        pages(&data),
        1 + n_layouts,
        "model space plus every layout: {}",
        s.log_text()
    );
    assert!(s.history_has(&format!("Published {} sheets to", 1 + n_layouts)));

    s.cmd("PUBLISH").ty("Layouts");
    assert_eq!(pages(&pdf(&s).unwrap().1), n_layouts);
    s.cmd("PUBLISH").ty("Model");
    assert_eq!(pages(&pdf(&s).unwrap().1), 1);
}

#[test]
fn a_drawing_without_layouts_publishes_model_space_without_asking() {
    let s = Sim::new();
    s.add(Entity::line(p(0., 0.), p(10., 10.)));
    if s.doc(|d| d.layout_names().is_empty()) {
        s.cmd("PUBLISH");
        assert!(!s.busy(), "no prompt: {}", s.log_text());
        assert_eq!(pages(&pdf(&s).unwrap().1), 1);
    }
}

#[test]
fn nothing_to_publish_is_reported_and_no_file_is_made() {
    let s = Sim::new();
    s.cmd("PUBLISH").ty("Model");
    assert!(
        s.history_has("There is nothing to plot."),
        "{}",
        s.log_text()
    );
    assert!(pdf(&s).is_none());
}

#[test]
fn the_published_pdf_is_valid_and_pages_have_their_own_sizes() {
    let s = with_two_layouts();
    s.doc_mut(|d| d.layouts[0].paper.w = 420.0);
    s.cmd("PUBLISH").ty("Layouts");
    let (_, data) = pdf(&s).unwrap();
    let text = String::from_utf8_lossy(&data).to_string();
    // the first layout is 420 mm wide = 1190.55 pt
    assert!(
        text.contains("/MediaBox [0 0 1190.551"),
        "{}",
        &text[..text.len().min(600)]
    );
    // every cross-reference offset points at its object
    let at = data.windows(10).rposition(|w| w == b"startxref\n").unwrap();
    let start: usize = String::from_utf8_lossy(&data[at + 10..])
        .lines()
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        data[start..].starts_with(b"xref"),
        "startxref points at the table"
    );
    // poppler, when installed, reads it back as a document with the right page count
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("publish-test.pdf");
    std::fs::write(&path, &data).unwrap();
    match std::process::Command::new("pdfinfo").arg(&path).output() {
        Ok(o) => {
            let info = String::from_utf8_lossy(&o.stdout);
            assert!(
                o.status.success(),
                "{info}{}",
                String::from_utf8_lossy(&o.stderr)
            );
            let n: usize = info
                .lines()
                .find_map(|l| l.strip_prefix("Pages:"))
                .and_then(|v| v.trim().parse().ok())
                .expect("a page count");
            assert_eq!(n, s.doc(|d| d.layout_names().len()), "{info}");
            assert!(
                info.contains("Page size:       1190.55 x"),
                "the first sheet is the 420 mm wide layout: {info}"
            );
        }
        Err(_) => eprintln!("pdfinfo not installed; skipping the poppler check"),
    }
}
