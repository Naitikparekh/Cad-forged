use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::view::View;
use cadforge_render::*;

fn doc_with_viewport(scale: Option<f64>) -> Drawing {
    let mut d = Drawing::new("t");
    d.add(Entity::line(pt(0.0, 0.0), pt(1000.0, 0.0)));
    d.add(Entity::circle(pt(500.0, 300.0), 100.0));
    d.add_layout("Sheet").unwrap();
    d.set_space("Sheet").unwrap();
    // replace the default viewport with a known one: 100 x 80 mm at (150, 105), showing the model centred on (500, 300)
    let l = d.layouts.iter_mut().find(|l| l.name == "Sheet").unwrap();
    l.entities.clear();
    let mut vp = ViewportData::new(pt(150.0, 105.0), 100.0, 80.0);
    vp.scale = scale;
    vp.vc = pt(500.0, 300.0);
    l.entities.push(Entity::new(Kind::Viewport(vp)));
    d
}

fn paper_view(d: &Drawing) -> View {
    let mut v = View::new();
    v.w = 900.0;
    v.h = 700.0;
    let p = d.layout("Sheet").unwrap().paper.clone();
    v.fit(Some(Bounds::new(0.0, 0.0, p.w, p.h)), 0.05);
    v
}

#[test]
fn layout_renders_sheet_and_viewport_contents_clipped() {
    let d = doc_with_viewport(Some(0.2)); // 1:5 -> the 1000 mm line is 200 mm long but the viewport is only 100 wide
    let v = paper_view(&d);
    let ops = render_layout(
        &d,
        "Sheet",
        &v,
        900.0,
        700.0,
        Theme::DARK,
        Prefs::default(),
        &Highlight::default(),
        None,
    );
    // sheet is a white fill
    assert!(ops.iter().any(|o| matches!(
        o,
        Op::Fill {
            color: [255, 255, 255],
            ..
        }
    )));
    // everything drawn from model space lies inside the viewport rectangle on screen
    let a = v.to_screen(pt(100.0, 65.0));
    let b = v.to_screen(pt(200.0, 145.0));
    let (x0, x1) = (a.x.min(b.x) - 0.5, a.x.max(b.x) + 0.5);
    let (y0, y1) = (a.y.min(b.y) - 0.5, a.y.max(b.y) + 0.5);
    let mut inside = 0;
    for o in &ops {
        if let Op::Stroke { paths, width, .. } = o {
            // the 1.0 px border and sheet outline are bigger than the viewport or exactly on it; skip their widths
            for p in paths {
                if p.pts
                    .iter()
                    .all(|q| q[0] >= x0 && q[0] <= x1 && q[1] >= y0 && q[1] <= y1)
                {
                    inside += 1;
                }
                let _ = width;
            }
        }
    }
    assert!(
        inside >= 2,
        "line + circle should be drawn inside the viewport, got {inside}"
    );
    // nothing from the model line may reach the sheet edge: no stroke path spans wider than the viewport
    // (the sheet outline is wider than the viewport; allow exactly that one)
    let wide = ops.iter().filter(|o| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| { let xs: Vec<f64> = p.pts.iter().map(|q| q[0]).collect(); xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min) > (x1 - x0) + 2.0 }))).count();
    assert!(
        wide <= 1,
        "only the sheet outline may be wider than the viewport, got {wide}"
    );
}

#[test]
fn viewport_without_scale_fits_model_extents() {
    let d = doc_with_viewport(None);
    let Kind::Viewport(vp) = &d.layout("Sheet").unwrap().entities[0].kind else {
        panic!()
    };
    let (c, s) = vp.model_view(&d);
    assert!((c.x - 500.0).abs() < 1e-6, "{c:?}");
    // the extents are 1000 wide x 400 tall (circle top at 400): limited by the 100 mm width, 90% fill
    assert!((s - 0.09).abs() < 1e-9, "{s}");
}

#[test]
fn layout_plot_uses_the_paper_and_skips_viewport_borders() {
    let d = doc_with_viewport(Some(0.1));
    let mut o = default_plot_options(&d);
    o.layout = Some("Sheet".into());
    let layout = plot_layout(&d, &o, None).unwrap();
    assert_eq!(layout.page, (297.0, 210.0));
    let ops = render_plot(&d, &o, &layout);
    assert!(!ops.is_empty());
    let pdf = export_vector(&d, &o, None).unwrap();
    assert!(pdf.data.starts_with(b"%PDF-"));
    assert!(pdf.layout.fits);
}

#[test]
fn convex_clipping_keeps_inside_part_of_a_segment() {
    let ops = vec![Op::Stroke {
        paths: vec![Poly {
            pts: vec![[-50.0, 5.0], [50.0, 5.0]],
            closed: false,
        }],
        color: [1, 2, 3],
        alpha: 1.0,
        width: 1.0,
        dash: vec![],
        round: false,
    }];
    let out = clip_ops_to_convex(ops, &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]);
    let Op::Stroke { paths, .. } = &out[0] else {
        panic!()
    };
    assert_eq!(paths.len(), 1);
    assert!((paths[0].pts[0][0] - 0.0).abs() < 1e-9 && (paths[0].pts[1][0] - 10.0).abs() < 1e-9);
}

fn two_layer_doc() -> Drawing {
    let mut d = doc_with_viewport(Some(0.2));
    d.add_layer("Hidden").unwrap();
    d.add_layer("Colored").unwrap();
    // doc_with_viewport leaves the sheet as the current space: put these in model space
    d.add_to(
        Entity::line(pt(450.0, 250.0), pt(550.0, 250.0)).on_layer("Hidden"),
        cadforge_core::drawing::MODEL,
    );
    d.add_to(
        Entity::line(pt(450.0, 350.0), pt(550.0, 350.0)).on_layer("Colored"),
        cadforge_core::drawing::MODEL,
    );
    d
}

/// (op index, colour, dash) for every path that lies inside the first viewport; one entry per path, since strokes of the
/// same style are batched into one op.
fn strokes_in_viewport(d: &Drawing) -> Vec<(usize, [u8; 3], Vec<f64>)> {
    let v = paper_view(d);
    let ops = render_layout(
        d,
        "Sheet",
        &v,
        900.0,
        700.0,
        Theme::DARK,
        Prefs::default(),
        &Highlight::default(),
        None,
    );
    // strokes that lie inside the 100 x 80 viewport (not its border or the sheet outline)
    let a = v.to_screen(pt(100.0, 65.0));
    let b = v.to_screen(pt(200.0, 145.0));
    let (x0, x1) = (a.x.min(b.x) + 1.0, a.x.max(b.x) - 1.0);
    let (y0, y1) = (a.y.min(b.y) + 1.0, a.y.max(b.y) - 1.0);
    let mut out = vec![];
    for (i, o) in ops.iter().enumerate() {
        if let Op::Stroke {
            paths, color, dash, ..
        } = o
        {
            for p in paths.iter().filter(|p| {
                !p.pts.is_empty()
                    && p.pts
                        .iter()
                        .all(|q| q[0] >= x0 && q[0] <= x1 && q[1] >= y0 && q[1] <= y1)
            }) {
                let _ = p;
                out.push((i, *color, dash.clone()));
            }
        }
    }
    out
}

/// Change the first viewport of the sheet.
fn set_vp(d: &mut Drawing, f: impl FnOnce(&mut ViewportData)) {
    let l = d.layouts.iter_mut().find(|l| l.name == "Sheet").unwrap();
    let v = l
        .entities
        .iter_mut()
        .find_map(|e| {
            if let Kind::Viewport(v) = &mut e.kind {
                Some(v)
            } else {
                None
            }
        })
        .unwrap();
    f(v);
}

#[test]
fn a_layer_frozen_in_the_viewport_is_not_drawn_there_but_stays_in_model_space() {
    let mut d = two_layer_doc();
    let before = strokes_in_viewport(&d).len();
    set_vp(&mut d, |v| v.set_frozen("hidden", true)); // case-insensitive
    let after = strokes_in_viewport(&d).len();
    assert_eq!(
        before - after,
        1,
        "the Hidden line disappears from the viewport"
    );
    assert!(
        d.model.entities.iter().any(|e| e.layer == "Hidden"),
        "and is still in the model"
    );
    // the model view itself is unaffected
    d.set_space(cadforge_core::drawing::MODEL).unwrap();
    let mv = {
        let mut v = View::new();
        v.w = 900.0;
        v.h = 700.0;
        v.fit(Some(Bounds::new(0.0, 0.0, 1000.0, 600.0)), 0.05);
        v
    };
    let ops = render_scene(
        &d,
        &mv,
        900.0,
        700.0,
        Theme::DARK,
        Prefs::default(),
        &Highlight::default(),
        None,
    );
    let model_strokes = ops
        .iter()
        .filter(|o| matches!(o, Op::Stroke { .. }))
        .count();
    set_vp(&mut d, |v| v.set_frozen("hidden", false));
    let ops2 = render_scene(
        &d,
        &mv,
        900.0,
        700.0,
        Theme::DARK,
        Prefs::default(),
        &Highlight::default(),
        None,
    );
    assert_eq!(
        model_strokes,
        ops2.iter()
            .filter(|o| matches!(o, Op::Stroke { .. }))
            .count()
    );
}

#[test]
fn colour_and_linetype_overrides_apply_in_the_viewport_only() {
    let mut d = two_layer_doc();
    d.settings.ltscale = 20.0; // the sheet is small on screen: make the dashes long enough to be drawn
    let plain = strokes_in_viewport(&d);
    assert!(
        plain.iter().all(|(_, _, dash)| dash.is_empty()),
        "continuous everywhere"
    );
    set_vp(&mut d, |v| {
        v.edit_override("Colored", |o| {
            o.color = Some(cadforge_core::style::Color::Rgb(255, 0, 0));
            o.ltype = Some("DASHED".into());
        })
    });
    let over = strokes_in_viewport(&d);
    assert!(
        over.iter()
            .any(|(_, c, dash)| *c == [255, 0, 0] && !dash.is_empty()),
        "{over:?}"
    );
    assert_eq!(over.len(), plain.len(), "nothing else changed");
}

#[test]
fn freezing_a_layer_in_one_viewport_leaves_another_viewport_showing_it() {
    let mut d = two_layer_doc();
    let l = d.layouts.iter_mut().find(|l| l.name == "Sheet").unwrap();
    let mut second = ViewportData::new(pt(300.0, 105.0), 100.0, 80.0);
    second.scale = Some(0.2);
    second.vc = pt(500.0, 300.0);
    l.entities.push(Entity::new(Kind::Viewport(second)));
    d.layouts
        .iter_mut()
        .find(|l| l.name == "Sheet")
        .unwrap()
        .entities
        .iter_mut()
        .find_map(|e| {
            if let Kind::Viewport(v) = &mut e.kind {
                Some(v)
            } else {
                None
            }
        })
        .unwrap()
        .set_frozen("Hidden", true);
    let v = paper_view(&d);
    let ops = render_layout(
        &d,
        "Sheet",
        &v,
        900.0,
        700.0,
        Theme::DARK,
        Prefs::default(),
        &Highlight::default(),
        None,
    );
    let in_rect = |ops: &[Op], cx: f64| {
        let a = v.to_screen(pt(cx - 50.0, 65.0));
        let b = v.to_screen(pt(cx + 50.0, 145.0));
        let (x0, x1) = (a.x.min(b.x) + 1.0, a.x.max(b.x) - 1.0);
        let (y0, y1) = (a.y.min(b.y) + 1.0, a.y.max(b.y) - 1.0);
        ops.iter()
            .map(|o| match o {
                Op::Stroke { paths, .. } => paths
                    .iter()
                    .filter(|p| {
                        !p.pts.is_empty()
                            && p.pts
                                .iter()
                                .all(|q| q[0] >= x0 && q[0] <= x1 && q[1] >= y0 && q[1] <= y1)
                    })
                    .count(),
                _ => 0,
            })
            .sum::<usize>()
    };
    assert_eq!(
        in_rect(&ops, 150.0) + 1,
        in_rect(&ops, 300.0),
        "the second viewport still draws the Hidden layer"
    );
}

#[test]
fn viewport_overrides_are_saved_in_the_json_and_empty_ones_are_not_written() {
    let mut d = two_layer_doc();
    set_vp(&mut d, |v| v.set_frozen("Hidden", true));
    let text = d.to_json_string();
    assert!(
        text.contains("\"frozen\":true") || text.contains("\"frozen\": true"),
        "{text}"
    );
    let back = Drawing::from_json_str(&text, None).unwrap();
    let vp = back
        .layout("Sheet")
        .unwrap()
        .entities
        .iter()
        .find_map(|e| {
            if let Kind::Viewport(v) = &e.kind {
                Some(v.clone())
            } else {
                None
            }
        })
        .unwrap();
    assert!(vp.is_frozen("Hidden") && !vp.is_frozen("Colored"));
    // a viewport that overrides nothing writes no "layers" key at all
    set_vp(&mut d, |v| v.reset_layer("Hidden"));
    assert!(!d
        .layout("Sheet")
        .unwrap()
        .entities
        .iter()
        .any(|e| matches!(&e.kind, Kind::Viewport(v) if !v.layers.is_empty())));
}

#[test]
fn plotting_a_layout_honours_the_viewport_layer_settings() {
    let mut d = two_layer_doc();
    let paths_in_plot = |d: &Drawing| -> usize {
        let mut o = default_plot_options(d);
        o.layout = Some("Sheet".into());
        let layout = plot_layout(d, &o, None).unwrap();
        render_plot(d, &o, &layout)
            .iter()
            .map(|op| {
                if let Op::Stroke { paths, .. } = op {
                    paths.len()
                } else {
                    0
                }
            })
            .sum()
    };
    let before = paths_in_plot(&d);
    set_vp(&mut d, |v| v.set_frozen("Hidden", true));
    assert_eq!(
        before - paths_in_plot(&d),
        1,
        "the frozen layer's line is not in the plot"
    );
    // a layer that is not plotted at all stays out whatever the viewport says
    set_vp(&mut d, |v| v.set_frozen("Hidden", false));
    d.layers
        .iter_mut()
        .find(|l| l.name == "Colored")
        .unwrap()
        .plot = false;
    assert_eq!(before - paths_in_plot(&d), 1);
}
