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
    let b = doc.model.entities.iter().fold(None, |a, e| {
        cadforge_core::geom::Bounds::union_opt(a, cadforge_core::entities::bounds_of(e, &doc))
    });
    view.fit(b, 0.06);
    let ops = render_scene(
        &doc,
        &view,
        1000.0,
        700.0,
        Theme::DARK,
        Prefs::default(),
        &Highlight::default(),
        None,
    );
    let strokes = ops
        .iter()
        .filter(|o| matches!(o, Op::Stroke { .. }))
        .count();
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

#[test]
fn layout_fits_and_centres() {
    let doc = sample_drawing();
    let o = default_plot_options(&doc);
    let l = plot_layout(&doc, &o, None).unwrap();
    assert_eq!(l.page, (297.0, 210.0));
    let (m, s) = (l.world_m, l.mm_per_unit);
    let x0 = m[4] / 10.0 + s * l.bounds.min_x;
    let x1 = m[4] / 10.0 + s * l.bounds.max_x;
    assert!(x0 >= 4.99 && x1 <= 297.0 - 4.99, "x range {x0}..{x1}");
    assert!((x0 - (297.0 - x1)).abs() < 1e-6, "centred horizontally");
    assert!(l.fits);
    let mut p = o.clone();
    p.landscape = false;
    assert_eq!(page_size(&p), (210.0, 297.0));
}

#[test]
fn fixed_scale_uses_drawing_units() {
    let mut doc = sample_drawing();
    doc.settings.units.ins_units = 4;
    let mut o = default_plot_options(&doc);
    o.scale = Some(0.5);
    o.paper = "A2".into();
    assert_eq!(plot_layout(&doc, &o, None).unwrap().mm_per_unit, 0.5);
    doc.settings.units.ins_units = 1;
    o.scale = Some(1.0);
    o.paper = "A0".into();
    assert_eq!(plot_layout(&doc, &o, None).unwrap().mm_per_unit, 25.4);
}

#[test]
fn pdf_is_well_formed() {
    let doc = sample_drawing();
    let o = default_plot_options(&doc);
    let r = export_vector(&doc, &o, None).unwrap();
    // byte offsets are what the xref table records, so keep the data as bytes (the header has non-ASCII bytes)
    let d = &r.data;
    let find = |needle: &str, from: usize| -> Option<usize> {
        d[from..]
            .windows(needle.len())
            .position(|w| w == needle.as_bytes())
            .map(|i| i + from)
    };
    assert!(d.starts_with(b"%PDF-1.4"));
    assert!(d.ends_with(b"%%EOF\n"));
    let sx = find("startxref\n", 0).unwrap() + "startxref\n".len();
    let xref: usize = std::str::from_utf8(&d[sx..(sx + 12).min(d.len())])
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        d[xref..].starts_with(b"xref"),
        "startxref points at the xref table"
    );
    let mut at = xref;
    let mut entries = 0;
    while let Some(i) = find(" 00000 n", at) {
        let off: usize = std::str::from_utf8(&d[i - 10..i]).unwrap().parse().unwrap();
        let head = std::str::from_utf8(&d[off..off + 8]).unwrap();
        assert!(
            head.chars().next().unwrap().is_ascii_digit() && head.contains(" 0 obj"),
            "xref entry {head:?}"
        );
        entries += 1;
        at = i + 1;
    }
    assert_eq!(entries, 6);
    let s: String = d.iter().map(|&b| b as char).collect();
    assert!(s.contains("MediaBox [0 0 841.89"));
    assert!(s.contains("(L-BRACKET)"));
    assert_eq!(r.mime, "application/pdf");
}

#[test]
fn svg_has_paths_and_text() {
    let doc = sample_drawing();
    let mut o = default_plot_options(&doc);
    o.format = PlotFormat::Svg;
    let s = String::from_utf8(export_vector(&doc, &o, None).unwrap().data).unwrap();
    assert!(s.contains("width=\"297mm\""), "{}", &s[..200]);
    assert!(s.matches("<path ").count() > 20);
    assert!(s.contains(">L-BRACKET<"));
}

#[test]
fn mono_forces_black_colour_keeps_layer_colours() {
    let doc = sample_drawing();
    let mut o = default_plot_options(&doc);
    o.format = PlotFormat::Svg;
    o.mono = true;
    let mono = String::from_utf8(export_vector(&doc, &o, None).unwrap().data).unwrap();
    o.mono = false;
    let col = String::from_utf8(export_vector(&doc, &o, None).unwrap().data).unwrap();
    assert!(!mono.contains("stroke=\"#00ffff\"") && col.contains("stroke=\"#00ffff\""));
}

#[test]
fn non_plot_layers_skipped_and_lineweights_follow_option() {
    use cadforge_core::{entities::Entity, geom::pt, style::Layer};
    let mut d = cadforge_core::drawing::Drawing::new("t");
    let mut hidden = Layer::new("Hidden");
    hidden.plot = false;
    d.layers.push(hidden);
    d.add(
        Entity::line(pt(0.0, 0.0), pt(100.0, 0.0))
            .on_layer("0")
            .with_lw(50),
    );
    d.add(Entity::line(pt(0.0, 10.0), pt(100.0, 10.0)).on_layer("Hidden"));
    let mut o = default_plot_options(&d);
    o.format = PlotFormat::Svg;
    let svg = String::from_utf8(export_vector(&d, &o, None).unwrap().data).unwrap();
    assert_eq!(svg.matches("<path ").count(), 1);
    assert!(svg.contains("stroke-width=\"5\""));
    o.lineweights = false;
    let thin = String::from_utf8(export_vector(&d, &o, None).unwrap().data).unwrap();
    assert!(thin.contains("stroke-width=\"2\""));
}

#[test]
fn png_raster_has_ink_and_expected_size() {
    let font = include_bytes!("../../cadforge-app/assets/fonts/LiberationSans-Regular.ttf");
    let doc = sample_drawing();
    let mut o = default_plot_options(&doc);
    o.paper = "A4".into();
    o.dpi = 50.0;
    let layout = plot_layout(&doc, &o, None).unwrap();
    let ops = render_plot(&doc, &o, &layout);
    let png = raster::render_png(&ops, (layout.w, layout.h), UNIT_MM, o.dpi, font).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    // IHDR width/height are big-endian u32 at bytes 16..24
    let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
    let expect_w = (layout.w as f64 * UNIT_MM / 25.4 * 50.0).round() as u32;
    assert_eq!(w, expect_w);
    assert!(h > 100);
    std::fs::write(out_dir().join("sample.png"), &png).unwrap();
    // an enormous resolution is rejected rather than allocating
    assert!(raster::render_png(&ops, (layout.w, layout.h), UNIT_MM, 20000.0, font).is_err());
}
