use cadforge_core::sample::sample_drawing;
use cadforge_core::view::View;
use cadforge_render::*;

fn out_dir() -> std::path::PathBuf {
    let d = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn scene_renders_sample() {
    let doc = sample_drawing();
    let mut view = View::new();
    view.w = 1000.0;
    view.h = 700.0;
    let b = doc.model.entities.iter().fold(None, |a, e| cadforge_core::geom::Bounds::union_opt(a, cadforge_core::entities::bounds_of(e, &doc)));
    view.fit(b, 0.06);
    let ops = render_scene(&doc, &view, 1000.0, 700.0, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    let strokes = ops.iter().filter(|o| matches!(o, Op::Stroke { .. })).count();
    let texts = ops.iter().filter(|o| matches!(o, Op::Text { .. })).count();
    assert!(strokes > 20, "strokes {strokes}");
    assert!(texts >= 5, "texts {texts}");
}

#[test]
fn plot_pdf_and_svg() {
    let doc = sample_drawing();
    let mut o = default_plot_options(&doc);
    o.paper = "A3".into();
    let pdf = export_vector(&doc, &o, None).unwrap();
    assert!(pdf.data.starts_with(b"%PDF-1."));
    assert!(pdf.data.ends_with(b"%%EOF\n") || pdf.data.ends_with(b"%%EOF"));
    assert!(pdf.layout.fits);
    std::fs::write(out_dir().join("sample.pdf"), &pdf.data).unwrap();
    o.format = PlotFormat::Svg;
    o.mono = false;
    let svg = export_vector(&doc, &o, None).unwrap();
    let s = String::from_utf8(svg.data).unwrap();
    assert!(s.starts_with("<svg") || s.starts_with("<?xml"));
    assert!(s.contains("L-BRACKET"));
    std::fs::write(out_dir().join("sample.svg"), s).unwrap();
}

#[test]
fn empty_drawing_cannot_plot() {
    let doc = cadforge_core::drawing::Drawing::new("x");
    let o = default_plot_options(&doc);
    assert!(export_vector(&doc, &o, None).is_err());
}

#[test]
fn helvetica_widths() {
    let w = helvetica_width("A", 0.72 * 1000.0);
    assert!((w - 667.0).abs() < 1e-6, "{w}");
    assert!(helvetica_width("iii", 10.0) < helvetica_width("WWW", 10.0));
}
