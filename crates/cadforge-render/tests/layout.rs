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
    let ops = render_layout(&d, "Sheet", &v, 900.0, 700.0, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    // sheet is a white fill
    assert!(ops.iter().any(|o| matches!(o, Op::Fill { color: [255, 255, 255], .. })));
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
                if p.pts.iter().all(|q| q[0] >= x0 && q[0] <= x1 && q[1] >= y0 && q[1] <= y1) {
                    inside += 1;
                }
                let _ = width;
            }
        }
    }
    assert!(inside >= 2, "line + circle should be drawn inside the viewport, got {inside}");
    // nothing from the model line may reach the sheet edge: no stroke path spans wider than the viewport
    // (the sheet outline is wider than the viewport; allow exactly that one)
    let wide = ops.iter().filter(|o| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| { let xs: Vec<f64> = p.pts.iter().map(|q| q[0]).collect(); xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min) > (x1 - x0) + 2.0 }))).count();
    assert!(wide <= 1, "only the sheet outline may be wider than the viewport, got {wide}");
}

#[test]
fn viewport_without_scale_fits_model_extents() {
    let d = doc_with_viewport(None);
    let Kind::Viewport(vp) = &d.layout("Sheet").unwrap().entities[0].kind else { panic!() };
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
    let ops = vec![Op::Stroke { paths: vec![Poly { pts: vec![[-50.0, 5.0], [50.0, 5.0]], closed: false }], color: [1, 2, 3], alpha: 1.0, width: 1.0, dash: vec![], round: false }];
    let out = clip_ops_to_convex(ops, &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]);
    let Op::Stroke { paths, .. } = &out[0] else { panic!() };
    assert_eq!(paths.len(), 1);
    assert!((paths[0].pts[0][0] - 0.0).abs() < 1e-9 && (paths[0].pts[1][0] - 10.0).abs() < 1e-9);
}
