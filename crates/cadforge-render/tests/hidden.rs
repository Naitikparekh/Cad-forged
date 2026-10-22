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
