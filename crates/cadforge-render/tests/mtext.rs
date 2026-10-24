//! Drawing formatted multiline text: sizes, colours, bold, italic, decorations and fractions reach the draw operations.
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::plotstyle::{PlotStyle, PlotStyleTable};
use cadforge_core::style::{color_rgb, Color};
use cadforge_core::view::View;
use cadforge_render::*;

fn doc_with(s: &str) -> Drawing {
    let mut d = Drawing::new("t");
    d.add(Entity::mtext(pt(0.0, 0.0), 10.0, s, 0.0));
    d
}

fn view() -> View {
    let mut v = View::new();
    v.w = 800.0;
    v.h = 400.0;
    v.fit(Some(Bounds::new(-10.0, -20.0, 150.0, 30.0)), 0.05);
    v
}

/// Just the drawing: no grid and no UCS icon (it has letters and lines of its own).
fn ops_of(d: &Drawing) -> Vec<Op> {
    let prefs = Prefs { grid: false, ucs_icon: false, ..Prefs::default() };
    render_scene(d, &view(), 800.0, 400.0, Theme::DARK, prefs, &Highlight::default(), None)
}

#[derive(Debug, Clone)]
struct T {
    s: String,
    x: f64,
    y: f64,
    m: Mat,
    em: f64,
    color: Rgb,
}

fn texts(ops: &[Op]) -> Vec<T> {
    ops.iter().filter_map(|o| if let Op::Text { s, x, y, m, em, color, .. } = o { Some(T { s: s.clone(), x: *x, y: *y, m: *m, em: *em, color: *color }) } else { None }).collect()
}

fn apply(m: &Mat, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

#[test]
fn plain_multiline_text_is_drawn_exactly_as_before() {
    let ops = ops_of(&doc_with("alpha\\Pbeta"));
    let t = texts(&ops);
    assert_eq!(t.len(), 2);
    assert_eq!((t[0].s.as_str(), t[1].s.as_str()), ("alpha", "beta"));
    assert!((t[0].em - 10.0 / 0.72).abs() < 1e-9);
}

#[test]
fn a_run_with_its_own_colour_and_height_is_drawn_with_them() {
    let ops = ops_of(&doc_with("plain {\\C1;red} {\\H2x;big}"));
    let t = texts(&ops);
    let plain = t.iter().find(|x| x.s == "plain ").unwrap();
    let red = t.iter().find(|x| x.s == "red").unwrap();
    let big = t.iter().find(|x| x.s == "big").unwrap();
    assert_eq!(red.color, color_rgb(Color::Aci(1), true, false));
    assert_ne!(plain.color, red.color);
    assert!((big.em / plain.em - 2.0).abs() < 1e-9, "\\H2x; doubles the size");
    assert_eq!(plain.color, big.color, "colour does not leak out of its group");
}

#[test]
fn italic_leans_to_the_right_and_bold_is_struck_twice() {
    let ops = ops_of(&doc_with("plain {\\fArial|i1;lean} {\\fArial|b1;heavy}"));
    let t = texts(&ops);
    let plain = t.iter().find(|x| x.s == "plain ").unwrap();
    let lean = t.iter().find(|x| x.s == "lean").unwrap();
    // a point 10 units above the baseline moves right by tan(12 degrees) * 10 when the text is italic
    let above_plain = apply(&plain.m, lean.x, lean.y - 10.0);
    let above_lean = apply(&lean.m, lean.x, lean.y - 10.0);
    let on_base_plain = apply(&plain.m, lean.x, lean.y);
    let on_base_lean = apply(&lean.m, lean.x, lean.y);
    assert!((on_base_plain.0 - on_base_lean.0).abs() < 1e-9 && (on_base_plain.1 - on_base_lean.1).abs() < 1e-9, "the baseline does not move");
    let dx = above_lean.0 - above_plain.0;
    assert!((dx - 0.2126 * 10.0 * plain.m[0]).abs() < 1e-6, "dx {dx}, expected {}", 0.2126 * 10.0 * plain.m[0]);
    assert!((above_lean.1 - above_plain.1).abs() < 1e-9, "the lean is horizontal");
    let heavy: Vec<&T> = t.iter().filter(|x| x.s == "heavy").collect();
    assert_eq!(heavy.len(), 2, "bold text is drawn twice");
    assert!(heavy[1].x > heavy[0].x && heavy[1].x - heavy[0].x < 0.05 * heavy[0].em);
    assert_eq!(t.iter().filter(|x| x.s == "plain ").count(), 1);
}

#[test]
fn underline_overline_and_strike_are_extra_strokes_at_the_right_heights() {
    let count_strokes = |d: &Drawing| ops_of(d).iter().filter(|o| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| p.pts.len() == 2))).count();
    let none = count_strokes(&doc_with("word"));
    assert_eq!(count_strokes(&doc_with("\\Lword\\l")), none + 1);
    assert_eq!(count_strokes(&doc_with("\\Oword\\o")), none + 1);
    assert_eq!(count_strokes(&doc_with("\\Kword\\k")), none + 1);
    assert_eq!(count_strokes(&doc_with("%%uword%%u")), none + 1, "the percent shortcut underlines too");
    // the heights: underline below the baseline, strike through the middle, overline above the capitals
    let v = view();
    let y_of = |code: &str| -> f64 {
        let ops = ops_of(&doc_with(code));
        let base = ops_of(&doc_with("word"));
        let is_line = |o: &&Op| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| p.pts.len() == 2));
        let new: Vec<&Op> = ops.iter().filter(is_line).filter(|o| !base.iter().filter(is_line).any(|b| format!("{b:?}") == format!("{o:?}"))).collect();
        let Op::Stroke { paths, .. } = new[0] else { unreachable!() };
        let p = paths.iter().find(|p| p.pts.len() == 2).unwrap();
        v.to_world(p.pts[0][0], p.pts[0][1]).y
    };
    // MTEXT hangs from its insertion point, so find the baseline from the text itself
    let t = &texts(&ops_of(&doc_with("word")))[0];
    let dev = apply(&t.m, t.x, t.y);
    let base = v.to_world(dev.0, dev.1).y;
    let (under, strike, over) = (y_of("\\Lword\\l") - base, y_of("\\Kword\\k") - base, y_of("\\Oword\\o") - base);
    assert!(under < 0.0 && under > -3.0, "underline just below the baseline: {under}");
    assert!(strike > 2.0 && strike < 6.0, "strike through the middle of the capitals: {strike}");
    assert!(over > 10.0 && over < 14.0, "overline above the capital height: {over}");
}

#[test]
fn a_fraction_is_two_small_texts_and_a_bar() {
    let ops = ops_of(&doc_with("\\S1/2;"));
    let t = texts(&ops);
    assert_eq!(t.len(), 2);
    assert_eq!((t[0].s.as_str(), t[1].s.as_str()), ("1", "2"));
    assert!((t[0].em - 0.7 * 10.0 / 0.72).abs() < 1e-9);
    assert!(t[0].y < t[1].y, "the numerator is above the denominator (device y grows downwards): {} vs {}", t[0].y, t[1].y);
    // a bar between them
    let v = view();
    let bars: Vec<f64> = ops.iter().filter_map(|o| if let Op::Stroke { paths, .. } = o { paths.iter().find(|p| p.pts.len() == 2).map(|p| v.to_world(p.pts[0][0], p.pts[0][1]).y) } else { None }).collect();
    assert!(!bars.is_empty());
    let (n, d) = (apply(&t[0].m, t[0].x, t[0].y), apply(&t[1].m, t[1].x, t[1].y));
    assert!(bars.iter().any(|b| { let dev = v.to_screen(pt(0.0, *b)).y; dev < n.1 && dev > d.1 - 1e-9 || (dev > n.1 && dev < d.1) }), "a bar lies between the two baselines");
    // the tolerance form has no bar
    let tol = ops_of(&doc_with("\\S+1^-1;"));
    let plain = ops_of(&doc_with("x"));
    let lines = |ops: &[Op]| ops.iter().filter(|o| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| p.pts.len() == 2))).count();
    assert_eq!(lines(&tol), lines(&plain));
}

#[test]
fn plots_use_black_in_monochrome_and_the_style_table_for_run_colours() {
    let d = doc_with("a {\\C1;red} b");
    let colours = |d: &Drawing, f: &dyn Fn(&mut PlotOptions)| -> Vec<(String, Rgb)> {
        let mut o = default_plot_options(d);
        f(&mut o);
        let layout = plot_layout(d, &o, None).unwrap();
        texts(&render_plot(d, &o, &layout)).into_iter().map(|t| (t.s, t.color)).collect()
    };
    let mono = colours(&d, &|_| {});
    assert!(mono.iter().all(|(_, c)| *c == [0, 0, 0]), "{mono:?}");
    let colour = colours(&d, &|o| o.mono = false);
    assert_eq!(colour.iter().find(|(s, _)| s == "red").unwrap().1, color_rgb(Color::Aci(1), false, false).map(|v| v), "{colour:?}");
    // a plot style table recolours colour 1 to blue
    let mut styled = d.clone();
    let mut styles: Vec<PlotStyle> = (1..=255).map(|i| PlotStyle { name: format!("Color_{i}"), color: None, lineweight: None, screen: 100, pen: 0 }).collect();
    styles[0].color = Some([0, 0, 255]);
    styled.plot_styles = Some(std::rc::Rc::new(PlotStyleTable { name: "t".into(), description: String::new(), color_based: true, styles }));
    let c = colours(&styled, &|_| {});
    assert_eq!(c.iter().find(|(s, _)| s == "red").unwrap().1, [0, 0, 255], "{c:?}");
}
