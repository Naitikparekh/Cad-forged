//! Objects hidden by HIDEOBJECTS / ISOLATEOBJECTS are left out of the drawn scene and of plots.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::view::View;
use cadforge_render::*;

fn stroke_count(d: &Drawing) -> usize {
    let v = View::default();
    render_scene(d, &v, 800.0, 600.0, Theme::DARK, Prefs::default(), &Highlight::default(), None).iter().filter(|o| matches!(o, Op::Stroke { .. })).count()
}

#[test]
fn hidden_objects_are_not_drawn_until_they_are_shown_again() {
    let mut d = Drawing::new("t");
    let a = d.add(Entity::line(pt(0., 0.), pt(10., 0.)));
    d.add(Entity::line(pt(0., 5.), pt(10., 5.)));
    let all = stroke_count(&d);
    d.hidden_objs.insert(a);
    d.touch();
    let hidden = stroke_count(&d);
    assert!(hidden < all, "{hidden} < {all}");
    d.hidden_objs.clear();
    d.touch();
    assert_eq!(stroke_count(&d), all);
}

#[test]
fn wide_polylines_are_drawn_as_filled_outlines() {
    let fills = |d: &Drawing| render_scene(d, &View::default(), 800.0, 600.0, Theme::DARK, Prefs::default(), &Highlight::default(), None).iter().filter(|o| matches!(o, Op::Fill { .. })).count();
    let mut plain = Drawing::new("t");
    plain.add(Entity::pline(vec![Vertex::new(0., 0., 0.), Vertex::new(10., 0., 0.), Vertex::new(10., 5., 0.)], false));
    let mut wide = Drawing::new("t");
    wide.add(Entity::pline(vec![Vertex::wide(0., 0., 0., 2.0, 2.0), Vertex::wide(10., 0., 0., 2.0, 2.0), Vertex::new(10., 5., 0.)], false));
    // (the scene always has a few fills of its own, such as the UCS icon's arrow heads)
    // two edges and the corner between them (two bevel triangles)
    assert_eq!(fills(&wide) - fills(&plain), 4);
}

#[test]
fn a_loaded_linetype_draws_dashed_with_its_own_pattern() {
    use cadforge_core::defs::parse_lin;
    let mut d = Drawing::new("t");
    d.define_linetype(parse_lin("*RENDER_TEST_LT, test\nA,4,-2\n").defs.remove(0)).unwrap();
    let mut line = Entity::line(pt(0., 0.), pt(100., 0.));
    line.ltype = Some("RENDER_TEST_LT".into());
    d.add(line);
    let ops = render_scene(&d, &View::default(), 800.0, 600.0, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    let dashes: Vec<&Vec<f64>> = ops.iter().filter_map(|o| if let Op::Stroke { dash, .. } = o { Some(dash).filter(|d| !d.is_empty()) } else { None }).collect();
    assert!(!dashes.is_empty(), "the line is dashed");
    assert!((dashes[0][0] / dashes[0][1].abs() - 2.0).abs() < 1e-6, "dash:gap = 4:2, got {:?}", dashes[0]);
}

#[test]
fn quick_text_draws_boxes_instead_of_glyphs() {
    let mut d = Drawing::new("t");
    d.add(Entity::text(pt(0., 0.), 5.0, "HELLO"));
    let mut v = View::default();
    v.w = 800.0;
    v.h = 600.0;
    v.fit(Some(Bounds::new(-5.0, -5.0, 30.0, 10.0)), 0.1);
    let draw = |qtext: bool| render_scene(&d, &v, 800.0, 600.0, Theme::DARK, Prefs { qtext, ..Prefs::default() }, &Highlight::default(), None);
    let texts = |ops: &[Op]| ops.iter().filter(|o| matches!(o, Op::Text { .. })).count();
    let closed_boxes = |ops: &[Op]| ops.iter().filter(|o| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| p.closed && p.pts.len() == 4))).count();
    let (normal, quick) = (draw(false), draw(true));
    // (the UCS icon has its own X / Y labels, so compare the two renderings rather than counting)
    assert_eq!(texts(&normal) - texts(&quick), 1, "the drawing's text is no longer drawn as glyphs");
    assert!(closed_boxes(&quick) > closed_boxes(&normal), "a box takes their place");
}
