//! Plotting with a plot style table: colours, lineweights and screening come from the table.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::plotstyle::{PlotStyle, PlotStyleTable};
use cadforge_core::style::Color;
use cadforge_render::*;

/// Colour 1 (red) prints blue at 0.50 mm; colour 2 (yellow) keeps its colour but is screened to 40 %; colour 3 (green) is
/// untouched.
fn table() -> PlotStyleTable {
    let mut styles: Vec<PlotStyle> = (1..=255).map(|i| PlotStyle { name: format!("Color_{i}"), color: None, lineweight: None, screen: 100, pen: 0 }).collect();
    styles[0].color = Some([0, 0, 255]);
    styles[0].lineweight = Some(50);
    styles[1].screen = 40;
    PlotStyleTable { name: "t".into(), description: String::new(), color_based: true, styles }
}

fn doc() -> Drawing {
    let mut d = Drawing::new("t");
    for (i, aci) in [1u8, 2, 3].iter().enumerate() {
        let mut l = Entity::line(pt(0., i as f64 * 10.), pt(50., i as f64 * 10.));
        l.color = Some(Color::Aci(*aci));
        d.add(l);
    }
    d
}

/// (colour, width, alpha) of every stroke, in the order the lines were added.
fn strokes(d: &Drawing, f: impl FnOnce(&mut PlotOptions)) -> Vec<([u8; 3], f64, f32)> {
    let mut o = default_plot_options(d);
    f(&mut o);
    let layout = plot_layout(d, &o, None).unwrap();
    let mut out: Vec<([u8; 3], f64, f32)> = render_plot(d, &o, &layout).iter().filter_map(|op| if let Op::Stroke { color, width, alpha, paths, .. } = op { Some(paths.iter().map(|_| (*color, *width, *alpha)).collect::<Vec<_>>()) } else { None }).flatten().collect();
    // plot order is drawing order; keep it
    out.truncate(3);
    out
}

#[test]
fn without_a_table_the_monochrome_option_decides() {
    let d = doc();
    let s = strokes(&d, |_| {});
    assert!(s.iter().all(|(c, _, a)| *c == [0, 0, 0] && *a == 1.0), "{s:?}");
}

#[test]
fn a_table_changes_colour_lineweight_and_screening_of_what_it_covers() {
    let mut d = doc();
    d.plot_styles = Some(std::rc::Rc::new(table()));
    let s = strokes(&d, |_| {});
    assert_eq!(s.len(), 3, "{s:?}");
    // colour 1 (red) -> blue, 0.50 mm = 5.0 device units of 0.1 mm
    assert_eq!(s[0].0, [0, 0, 255]);
    assert!((s[0].1 - 5.0).abs() < 1e-9, "{:?}", s[0]);
    assert_eq!(s[0].2, 1.0);
    // colour 2 keeps its own colour (the table does not force black: it is not monochrome) but prints at 40 %
    assert_ne!(s[1].0, [0, 0, 0]);
    assert!((s[1].2 - 0.4).abs() < 1e-6, "{:?}", s[1]);
    // colour 3 is untouched: its own colour, normal strength
    assert_ne!(s[2].0, [0, 0, 0]);
    assert_eq!(s[2].2, 1.0);
    assert!(s[2].1 < 5.0, "the default lineweight is thinner than 0.50 mm: {:?}", s[2]);
}

#[test]
fn the_table_replaces_the_monochrome_option_and_can_be_switched_off_per_plot() {
    let mut d = doc();
    d.plot_styles = Some(std::rc::Rc::new(table()));
    let with = strokes(&d, |o| o.mono = true);
    assert_ne!(with[2].0, [0, 0, 0], "mono = true, but the table is in charge of colour");
    let without = strokes(&d, |o| o.use_styles = false);
    assert!(without.iter().all(|(c, _, a)| *c == [0, 0, 0] && *a == 1.0), "use_styles = false plots as if there was no table: {without:?}");
}

#[test]
fn monochrome_table_prints_everything_black_even_without_the_mono_option() {
    let mut d = doc();
    d.plot_styles = Some(std::rc::Rc::new(PlotStyleTable::monochrome()));
    let s = strokes(&d, |o| o.mono = false);
    assert!(s.iter().all(|(c, _, _)| *c == [0, 0, 0]), "{s:?}");
}

#[test]
fn the_table_applies_inside_layout_viewports_and_to_true_colour_objects() {
    let mut d = Drawing::new("t");
    let mut red = Entity::line(pt(0., 0.), pt(100., 0.));
    red.color = Some(Color::Rgb(250, 5, 5)); // nearest ACI colour is 1
    d.add(red);
    d.add_layout("Sheet").unwrap();
    d.set_space("Sheet").unwrap();
    let l = d.layouts.iter_mut().find(|l| l.name == "Sheet").unwrap();
    l.entities.clear();
    let mut vp = ViewportData::new(pt(150.0, 105.0), 100.0, 80.0);
    vp.scale = Some(0.5);
    vp.vc = pt(50.0, 0.0);
    l.entities.push(Entity::new(Kind::Viewport(vp)));
    d.plot_styles = Some(std::rc::Rc::new(table()));
    let mut o = default_plot_options(&d);
    o.layout = Some("Sheet".into());
    let layout = plot_layout(&d, &o, None).unwrap();
    let ops = render_plot(&d, &o, &layout);
    assert!(ops.iter().any(|op| matches!(op, Op::Stroke { color: [0, 0, 255], width, .. } if (*width - 5.0).abs() < 1e-9)), "the line inside the viewport plots as colour 1's style says");
}
