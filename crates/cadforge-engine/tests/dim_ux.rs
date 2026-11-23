//! The DIM command as in AutoCAD: hovering an object previews its dimension, a click places it, a click on empty space
//! starts a dimension between two points, and DIM stays active until Enter or Esc. Also: a typed command is echoed
//! once in the history.
use cadforge_core::dim::DimType;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::runtime::PreviewItem;
use cadforge_engine::sim::Sim;

fn dims(s: &Sim) -> Vec<cadforge_core::dim::DimData> {
    s.ents().into_iter().filter_map(|e| if let Kind::Dim(d) = e.kind { Some(d) } else { None }).collect()
}

fn preview_dims(s: &Sim) -> Vec<DimType> {
    s.preview().into_iter().filter_map(|p| if let PreviewItem::Entity(e) = p { if let Kind::Dim(d) = e.kind { Some(d.kind) } else { None } } else { None }).collect()
}

fn drawing() -> Sim {
    let s = Sim::new();
    s.add(Entity::line(pt(0., 0.), pt(40., 0.)));
    s.add(Entity::line(pt(0., 20.), pt(30., 40.)));
    s.add(Entity::circle(pt(80., 20.), 10.));
    s
}

#[test]
fn hovering_shows_the_dimension_each_object_would_get() {
    let s = drawing();
    s.cmd("DIM");
    s.mv(20., 0.);
    assert_eq!(preview_dims(&s), vec![DimType::Lin], "a horizontal line: a linear dimension");
    s.mv(15., 30.);
    assert_eq!(preview_dims(&s), vec![DimType::Ali], "a sloped line: an aligned dimension");
    s.mv(90., 20.);
    assert_eq!(preview_dims(&s), vec![DimType::Rad], "a circle: a radius");
    s.mv(60., 60.);
    assert!(preview_dims(&s).is_empty(), "nothing under the cursor, nothing previewed");
}

#[test]
fn dim_places_one_dimension_after_another_until_enter() {
    let s = drawing();
    s.cmd("DIM").click(20., 0.).click(20., -8.);
    assert_eq!(dims(&s).len(), 1, "{}", s.log_text());
    assert!(s.busy(), "DIM stays active for the next dimension");
    s.click(90., 20.).click(100., 30.);
    let d = dims(&s);
    assert_eq!(d.len(), 2);
    assert_eq!(d[1].kind, DimType::Rad);
    // a click on empty space: a dimension between two points
    s.click(0., -30.).click(25., -30.).click(12., -40.);
    let d = dims(&s);
    assert_eq!(d.len(), 3, "{}", s.log_text());
    assert_eq!((d[2].p1, d[2].p2), (Some(pt(0., -30.)), Some(pt(25., -30.))));
    s.enter();
    assert!(!s.busy(), "Enter ends DIM");
}

#[test]
fn diameter_option_on_a_circle() {
    let s = drawing();
    s.cmd("DIM").click(90., 20.).ty("D").click(95., 35.).enter();
    assert_eq!(dims(&s).iter().map(|d| d.kind).collect::<Vec<_>>(), vec![DimType::Dia], "{}", s.log_text());
}

#[test]
fn a_typed_command_is_echoed_once() {
    let s = Sim::new();
    let count = |s: &Sim, line: &str| s.h.borrow().history.iter().filter(|l| l.text == line).count();
    s.ty("LINE");
    assert_eq!(count(&s, "Command: LINE"), 1, "{}", s.log_text());
    s.esc();
    s.ty("L");
    assert_eq!((count(&s, "Command: L"), count(&s, "LINE")), (1, 1), "an alias: the name follows on its own line");
    s.esc();
}

#[test]
fn a_dimension_line_below_part_of_a_horizontal_line_stays_horizontal() {
    // AutoCAD's rule: above or below the box of the two points is horizontal, to its side vertical
    let s = Sim::new();
    s.add(Entity::line(pt(20., 30.), pt(120., 30.)));
    s.cmd("DIM").click(40., 30.).click(40., 20.).enter();
    let d = dims(&s);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].ang, Some(0.0), "horizontal, not a 0.00 vertical dimension");
    s.cmd("DIMLINEAR").click(0., 0.).click(10., 5.).click(20., 2.);
    assert_eq!(dims(&s)[1].ang, Some(std::f64::consts::FRAC_PI_2), "to the right of both points: vertical");
}
