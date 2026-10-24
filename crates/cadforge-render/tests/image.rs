//! Raster images through the scene, the PNG / SVG / PDF plots (the PDF is rasterised with poppler when available).
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::picture::encode_png;
use cadforge_core::view::View;
use cadforge_render::*;
use std::process::Command;

const FONT: &[u8] = include_bytes!("../../cadforge-app/assets/fonts/LiberationSans-Regular.ttf");

/// 4 x 2 pixels: the left half red, the right half blue, the bottom-right pixel fully transparent.
fn picture() -> Vec<u8> {
    let mut px = vec![];
    for y in 0..2 {
        for x in 0..4 {
            px.extend(if x < 2 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, if y == 1 && x == 3 { 0 } else { 255 }]
            });
        }
    }
    encode_png(4, 2, px).unwrap()
}

fn doc() -> Drawing {
    let mut d = Drawing::new("pic");
    let key = d.add_image("pic.png", picture()).unwrap();
    d.add(Entity::new(Kind::Image(ImageData::new(
        &key,
        pt(0.0, 0.0),
        40.0,
        20.0,
        0.0,
    ))));
    d
}

fn plot_opts(d: &Drawing) -> PlotOptions {
    let mut o = default_plot_options(d);
    o.paper = "A4".into();
    o.mono = false;
    o
}

/// World points whose pictures colour we know: (point, expected RGB)
fn probes() -> Vec<(Pt, [u8; 3])> {
    vec![
        (pt(10.0, 15.0), [255, 0, 0]),
        (pt(30.0, 15.0), [0, 0, 255]),
        (pt(10.0, 5.0), [255, 0, 0]),
        (pt(35.0, 5.0), [255, 255, 255]),
    ]
}

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter()
        .zip(&b)
        .all(|(x, y)| (*x as i32 - *y as i32).abs() <= 24)
}

#[test]
fn the_scene_maps_pixels_onto_the_screen() {
    let d = doc();
    let mut v = View::new();
    v.w = 800.0;
    v.h = 600.0;
    v.fit(Some(Bounds::new(0.0, 0.0, 40.0, 20.0)), 0.1);
    let ops = render_scene(
        &d,
        &v,
        800.0,
        600.0,
        Theme::DARK,
        Prefs::default(),
        &Highlight::default(),
        None,
    );
    let Some(Op::Image {
        pix, m, clip, adj, ..
    }) = ops.iter().find(|o| matches!(o, Op::Image { .. }))
    else {
        panic!("no image op")
    };
    assert!(clip.is_empty() && *adj == [50, 50, 0]);
    // pixel (0,0) is the top-left corner of the picture = world (0,20); (4,2) the bottom-right = world (40,0)
    let apply = |x: f64, y: f64| [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
    for (px, world) in [
        ([0.0, 0.0], pt(0.0, 20.0)),
        ([pix.0.w as f64, pix.0.h as f64], pt(40.0, 0.0)),
    ] {
        let (got, want) = (apply(px[0], px[1]), v.to_screen(world));
        assert!(
            (got[0] - want.x).abs() < 1e-6 && (got[1] - want.y).abs() < 1e-6,
            "{got:?} vs {want:?}"
        );
    }
    // the frame is stroked on screen
    assert!(ops.iter().any(|o| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| p.closed && p.pts.len() == 4))));
}

#[test]
fn a_missing_file_is_drawn_as_a_marked_frame() {
    let mut d = Drawing::new("m");
    d.images.insert(
        "gone".into(),
        cadforge_core::picture::ImageDef::missing("site/plan.jpg", 100, 50),
    );
    d.add(Entity::new(Kind::Image(ImageData::new(
        "gone",
        pt(0.0, 0.0),
        100.0,
        50.0,
        0.0,
    ))));
    let mut v = View::new();
    v.w = 400.0;
    v.h = 300.0;
    v.fit(Some(Bounds::new(0.0, 0.0, 100.0, 50.0)), 0.1);
    let ops = render_scene(
        &d,
        &v,
        400.0,
        300.0,
        Theme::DARK,
        Prefs::default(),
        &Highlight::default(),
        None,
    );
    assert!(!ops.iter().any(|o| matches!(o, Op::Image { .. })));
    assert!(ops.iter().any(
        |o| matches!(o, Op::Text { s, .. } if s.contains("Missing image") && s.contains("plan.jpg"))
    ));
}

#[test]
fn imageframe_zero_hides_the_frame_and_two_hides_it_when_plotting() {
    let mut d = doc();
    let mut v = View::new();
    v.w = 800.0;
    v.h = 600.0;
    v.fit(Some(Bounds::new(0.0, 0.0, 40.0, 20.0)), 0.1);
    let quads = |d: &Drawing| {
        render_scene(d, &v, 800.0, 600.0, Theme::DARK, Prefs::default(), &Highlight::default(), None).iter().filter(|o| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| p.closed && p.pts.len() == 4))).count()
    };
    let with_frame = quads(&d);
    d.settings.image_frame = 0;
    assert_eq!(
        quads(&d),
        with_frame - 1,
        "the frame is gone, the picture stays"
    );
    d.settings.image_frame = 2;
    let o = plot_opts(&d);
    let l = plot_layout(&d, &o, None).unwrap();
    let plot = render_plot(&d, &o, &l);
    assert!(!plot.iter().any(|o| matches!(o, Op::Stroke { paths, .. } if paths.iter().any(|p| p.closed && p.pts.len() == 4))), "frame option 2 is not plotted");
}

#[test]
fn png_plot_has_the_right_colours_in_the_right_places() {
    let d = doc();
    let mut o = plot_opts(&d);
    o.format = PlotFormat::Png;
    let l = plot_layout(&d, &o, None).unwrap();
    let ops = render_plot(&d, &o, &l);
    let png = raster::render_png(&ops, (l.w, l.h), UNIT_MM, 100.0, FONT).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_rgb8();
    let k = 100.0 / 25.4; // pixels per mm
    for (p, want) in probes() {
        let (x, y) = (
            (l.world_m[0] * p.x + l.world_m[4]) / 10.0 * k,
            (l.world_m[3] * p.y + l.world_m[5]) / 10.0 * k,
        );
        let got = img.get_pixel(x as u32, y as u32).0;
        assert!(close(got, want), "at {p:?}: {got:?} vs {want:?}");
    }
}

#[test]
fn svg_embeds_the_picture() {
    let d = doc();
    let mut o = plot_opts(&d);
    o.format = PlotFormat::Svg;
    let svg = String::from_utf8(export_vector(&d, &o, None).unwrap().data).unwrap();
    assert!(
        svg.contains("<image") && svg.contains("data:image/png;base64,"),
        "{}",
        &svg[..svg.len().min(600)]
    );
}

#[test]
fn clipped_pictures_are_cut_in_png_and_svg() {
    let mut d = doc();
    let id = d.model.entities[0].id;
    d.modify(id, |e| {
        if let Kind::Image(i) = &mut e.kind {
            i.clip = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 20.0), pt(0.0, 20.0)];
            // only the red half
        }
    });
    d.add(Entity::line(pt(0.0, 0.0), pt(40.0, 20.0))); // keeps the plot extents at the full picture
    let mut o = plot_opts(&d);
    o.format = PlotFormat::Png;
    let l = plot_layout(&d, &o, None).unwrap();
    let ops = render_plot(&d, &o, &l);
    let png = raster::render_png(&ops, (l.w, l.h), UNIT_MM, 100.0, FONT).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_rgb8();
    let k = 100.0 / 25.4;
    let at = |p: Pt| {
        img.get_pixel(
            ((l.world_m[0] * p.x + l.world_m[4]) / 10.0 * k) as u32,
            ((l.world_m[3] * p.y + l.world_m[5]) / 10.0 * k) as u32,
        )
        .0
    };
    assert!(close(at(pt(10.0, 15.0)), [255, 0, 0]));
    assert!(
        close(at(pt(25.0, 15.0)), [255, 255, 255]),
        "the blue half is clipped away: {:?}",
        at(pt(25.0, 15.0))
    );
    o.format = PlotFormat::Svg;
    assert!(String::from_utf8(export_vector(&d, &o, None).unwrap().data)
        .unwrap()
        .contains("clip-path"));
}

#[test]
fn pdf_embeds_the_picture_and_a_viewer_draws_it() {
    let d = doc();
    let o = plot_opts(&d);
    let pdf = export_vector(&d, &o, None).unwrap();
    let text = String::from_utf8_lossy(&pdf.data);
    assert!(
        text.contains("/Subtype /Image")
            && text.contains("/FlateDecode")
            && text.contains("/SMask"),
        "image XObject with a soft mask for the transparent pixel"
    );
    let have = |c: &str| Command::new(c).arg("-v").output().is_ok();
    if !have("pdftoppm")
        || !Command::new("python3")
            .args(["-I", "-c", "import PIL"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    {
        eprintln!("poppler / PIL not available; skipping the rasterised check");
        return;
    }
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let file = dir.join("image-plot.pdf");
    std::fs::write(&file, &pdf.data).unwrap();
    let prefix = dir.join("image-plot");
    let r = Command::new("pdftoppm")
        .args(["-r", "100", "-png", "-singlefile"])
        .arg(&file)
        .arg(&prefix)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let l = &pdf.layout;
    let k = 100.0 / 25.4;
    let coords: Vec<String> = probes()
        .iter()
        .map(|(p, _)| {
            format!(
                "{},{}",
                ((l.world_m[0] * p.x + l.world_m[4]) / 10.0 * k) as u32,
                ((l.world_m[3] * p.y + l.world_m[5]) / 10.0 * k) as u32
            )
        })
        .collect();
    let script = "import sys\nfrom PIL import Image\nim = Image.open(sys.argv[1]).convert('RGB')\nfor c in sys.argv[2:]:\n    x, y = map(int, c.split(','))\n    print(*im.getpixel((x, y)))";
    let out = Command::new("python3")
        .args(["-I", "-c", script])
        .arg(prefix.with_extension("png"))
        .args(&coords)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got: Vec<[u8; 3]> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| {
            let v: Vec<u8> = l.split(' ').map(|n| n.parse().unwrap()).collect();
            [v[0], v[1], v[2]]
        })
        .collect();
    for ((p, want), g) in probes().iter().zip(&got) {
        assert!(
            close(*g, *want),
            "poppler drew {g:?} at {p:?}, expected {want:?}"
        );
    }
}

#[test]
fn a_clipped_block_reference_draws_only_inside_its_boundary() {
    let mut d = Drawing::new("xclip");
    d.define_block(
        "RULER",
        pt(0.0, 0.0),
        vec![
            Entity::line(pt(0.0, 0.0), pt(100.0, 0.0)),
            Entity::circle(pt(50.0, 0.0), 10.0),
        ],
    );
    let mut ins = Entity::insert("RULER", pt(0.0, 0.0));
    if let Kind::Insert(i) = &mut ins.kind {
        i.clip = vec![pt(20.0, -5.0), pt(40.0, -5.0), pt(40.0, 5.0), pt(20.0, 5.0)];
    }
    d.add(ins);
    let mut v = View::new();
    v.w = 800.0;
    v.h = 400.0;
    v.fit(Some(Bounds::new(0.0, -20.0, 100.0, 20.0)), 0.05);
    let ops = render_scene(
        &d,
        &v,
        800.0,
        400.0,
        Theme::DARK,
        Prefs {
            grid: false,
            ucs_icon: false,
            ..Prefs::default()
        },
        &Highlight::default(),
        None,
    );
    let (lo, hi) = (
        v.to_screen(pt(20.0, 0.0)).x - 0.01,
        v.to_screen(pt(40.0, 0.0)).x + 0.01,
    );
    let mut drawn = 0;
    for o in &ops {
        if let Op::Stroke { paths, color, .. } = o {
            // block geometry is drawn in the layer colour; skip anything outside the strip the test looks at
            for p in paths.iter().filter(|p| {
                p.pts.iter().all(|q| {
                    q[1] > v.to_screen(pt(0.0, 6.0)).y - 1.0
                        && q[1] < v.to_screen(pt(0.0, -6.0)).y + 1.0
                })
            }) {
                let _ = color;
                for q in &p.pts {
                    assert!(
                        q[0] >= lo && q[0] <= hi,
                        "stroke point at x={} outside [{lo}, {hi}]",
                        q[0]
                    );
                }
                drawn += 1;
            }
        }
    }
    assert!(drawn >= 1, "the part inside the boundary is drawn");
}
