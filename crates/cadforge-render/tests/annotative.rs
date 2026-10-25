//! Annotative text is drawn at each layout viewport's scale: the same height on the sheet at 1:50 and 1:100.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::view::View;
use cadforge_render::*;

/// The model with one note, and a sheet whose single viewport shows it at `scale` (paper per model unit).
fn sheet(scale: f64, annotative: bool) -> (Drawing, View) {
    let mut d = Drawing::new("t");
    // plain text 125 units high is 2.5 mm on a 1:50 sheet; the annotative note is 2.5 mm on paper at any scale
    let id = d.add(Entity::text(pt(500.0, 300.0), if annotative { 2.5 } else { 125.0 }, "NOTE"));
    if annotative {
        assert!(d.set_annotative(id, true));
    }
    d.add_layout("Sheet").unwrap();
    d.set_space("Sheet").unwrap();
    let l = d.layouts.iter_mut().find(|l| l.name == "Sheet").unwrap();
    l.entities.clear();
    let mut vp = ViewportData::new(pt(150.0, 105.0), 100.0, 80.0);
    vp.scale = Some(scale);
    vp.vc = pt(500.0, 300.0);
    l.entities.push(Entity::new(Kind::Viewport(vp)));
    let mut v = View::new();
    v.w = 900.0;
    v.h = 700.0;
    let p = d.layout("Sheet").unwrap().paper.clone();
    v.fit(Some(Bounds::new(0.0, 0.0, p.w, p.h)), 0.05);
    (d, v)
}

/// Height on screen of the note's text.
fn note_height(scale: f64, annotative: bool) -> f64 {
    let (d, v) = sheet(scale, annotative);
    let ops = render_layout(&d, "Sheet", &v, 900.0, 700.0, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    ops.iter().find_map(|o| if let Op::Text { s, m, em, .. } = o { (s == "NOTE").then(|| em * (m[0] * m[3] - m[1] * m[2]).abs().sqrt()) } else { None }).expect("the note is drawn")
}

#[test]
fn annotative_text_keeps_its_sheet_height_in_every_viewport_scale() {
    let (a50, a100) = (note_height(0.02, true), note_height(0.01, true));
    assert!((a50 - a100).abs() < 1e-6 * a50, "annotative: {a50} vs {a100}");
    let (p50, p100) = (note_height(0.02, false), note_height(0.01, false));
    assert!((p50 - 2.0 * p100).abs() < 1e-6 * p50, "plain text halves at half the scale: {p50} vs {p100}");
    assert!((a50 - p50).abs() < 1e-6 * a50, "at 1:50 both are 2.5 mm on the sheet: {a50} vs {p50}");
}
