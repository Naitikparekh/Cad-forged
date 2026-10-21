//! PNG rasteriser for plots: draws renderer [`Op`]s with tiny-skia; text is drawn from glyph outlines of a supplied
//! TrueType font (the plot uses a Helvetica-compatible face so it matches the PDF and SVG output).
use crate::ops::Op;
use ab_glyph::{Font, FontRef, OutlineCurve};
use tiny_skia::{Color, FillRule, FilterQuality, LineCap, LineJoin, Paint, PathBuilder, Pattern, Pixmap, SpreadMode, Stroke, StrokeDash, Transform};

fn paint(rgb: [u8; 3], alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(rgb[0], rgb[1], rgb[2], (alpha.clamp(0.0, 1.0) * 255.0).round() as u8));
    p.anti_alias = true;
    p
}

/// Rasterise `ops` (device units of `unit_mm` millimetres, y down) onto a white page of `page` device units at `dpi`.
pub fn render_png(ops: &[Op], page: (f64, f64), unit_mm: f64, dpi: f64, font_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let k = (dpi / 25.4 * unit_mm) as f32;
    let (w, h) = ((page.0 as f32 * k).round().max(1.0) as u32, (page.1 as f32 * k).round().max(1.0) as u32);
    if (w as u64) * (h as u64) > 120_000_000 {
        return Err("The requested resolution is too large.".into());
    }
    let mut pm = Pixmap::new(w, h).ok_or("Could not allocate the image.")?;
    pm.fill(Color::WHITE);
    let font = FontRef::try_from_slice(font_bytes).ok();
    let sc = Transform::from_scale(k, k);
    for op in ops {
        match op {
            Op::Stroke { paths, color, alpha, width, dash, round } => {
                let mut pb = PathBuilder::new();
                for p in paths {
                    let mut it = p.pts.iter();
                    let Some(f) = it.next() else { continue };
                    pb.move_to(f[0] as f32, f[1] as f32);
                    for q in it {
                        pb.line_to(q[0] as f32, q[1] as f32);
                    }
                    if p.closed {
                        pb.close();
                    }
                }
                let Some(path) = pb.finish() else { continue };
                let mut st = Stroke { width: (*width as f32).max(0.05), line_cap: if *round { LineCap::Round } else { LineCap::Butt }, line_join: LineJoin::Round, ..Stroke::default() };
                if !dash.is_empty() && dash.iter().sum::<f64>() > 1e-6 {
                    let mut d: Vec<f32> = dash.iter().map(|v| (*v as f32).max(0.01)).collect();
                    if d.len() % 2 == 1 {
                        d.extend(d.clone());
                    }
                    st.dash = StrokeDash::new(d, 0.0);
                }
                pm.stroke_path(&path, &paint(*color, *alpha), &st, sc, None);
            }
            Op::Fill { paths, color, alpha, evenodd } => {
                let mut pb = PathBuilder::new();
                for p in paths {
                    let mut it = p.pts.iter();
                    let Some(f) = it.next() else { continue };
                    pb.move_to(f[0] as f32, f[1] as f32);
                    for q in it {
                        pb.line_to(q[0] as f32, q[1] as f32);
                    }
                    pb.close();
                }
                if let Some(path) = pb.finish() {
                    pm.fill_path(&path, &paint(*color, *alpha), if *evenodd { FillRule::EvenOdd } else { FillRule::Winding }, sc, None);
                }
            }
            Op::Image { pix, adj, bg, m, clip, alpha, smooth } => {
                let (w, h) = (pix.0.w, pix.0.h);
                let mut src = pix.0.adjusted(adj[0], adj[1], adj[2], *bg);
                // tiny-skia wants premultiplied pixels
                for px in src.chunks_exact_mut(4) {
                    let a = px[3] as u32;
                    for v in px[..3].iter_mut() {
                        *v = ((*v as u32 * a + 127) / 255) as u8;
                    }
                }
                let Some(size) = tiny_skia::IntSize::from_wh(w, h) else { continue };
                let Some(tex) = Pixmap::from_vec(src, size) else { continue };
                let quality = if *smooth { FilterQuality::Bilinear } else { FilterQuality::Nearest };
                let ts = Transform::from_row(m[0] as f32, m[1] as f32, m[2] as f32, m[3] as f32, m[4] as f32, m[5] as f32);
                let mut pt = Paint::default();
                pt.shader = Pattern::new(tex.as_ref(), SpreadMode::Pad, quality, alpha.clamp(0.0, 1.0), ts);
                pt.anti_alias = true;
                let mut pb = PathBuilder::new();
                if clip.len() >= 3 {
                    pb.move_to(clip[0][0] as f32, clip[0][1] as f32);
                    for q in &clip[1..] {
                        pb.line_to(q[0] as f32, q[1] as f32);
                    }
                } else {
                    for (i, q) in [[0.0, 0.0], [w as f64, 0.0], [w as f64, h as f64], [0.0, h as f64]].iter().enumerate() {
                        let (x, y) = (m[0] * q[0] + m[2] * q[1] + m[4], m[1] * q[0] + m[3] * q[1] + m[5]);
                        if i == 0 {
                            pb.move_to(x as f32, y as f32);
                        } else {
                            pb.line_to(x as f32, y as f32);
                        }
                    }
                }
                pb.close();
                if let Some(path) = pb.finish() {
                    // the page scale `sc` applies to the path and (after the pixel-to-device map) to the pattern
                    pm.fill_path(&path, &pt, FillRule::Winding, sc, None);
                }
            }
            Op::Text { s, x, y, m, em, color, alpha } => {
                let Some(font) = &font else { continue };
                let upm = font.units_per_em().unwrap_or(1000.0);
                let scale = (*em as f32) / upm;
                let mut pen = *x as f32;
                for ch in s.chars() {
                    let id = font.glyph_id(ch);
                    if let Some(outline) = font.outline(id) {
                        let mut pb = PathBuilder::new();
                        let mut last: Option<ab_glyph::Point> = None;
                        let map = |p: ab_glyph::Point| -> (f32, f32) {
                            // font units (y up) → text-local (y down) → device
                            let (lx, ly) = (pen + p.x * scale, *y as f32 - p.y * scale);
                            let (m0, m1, m2, m3, m4, m5) = (m[0] as f32, m[1] as f32, m[2] as f32, m[3] as f32, m[4] as f32, m[5] as f32);
                            (m0 * lx + m2 * ly + m4, m1 * lx + m3 * ly + m5)
                        };
                        for c in &outline.curves {
                            let start = match c {
                                OutlineCurve::Line(a, _) | OutlineCurve::Quad(a, _, _) | OutlineCurve::Cubic(a, _, _, _) => *a,
                            };
                            let contiguous = last.is_some_and(|l| (l.x - start.x).abs() < 1e-3 && (l.y - start.y).abs() < 1e-3);
                            if !contiguous {
                                if last.is_some() {
                                    pb.close();
                                }
                                let (a, b) = map(start);
                                pb.move_to(a, b);
                            }
                            match c {
                                OutlineCurve::Line(_, p1) => {
                                    let (a, b) = map(*p1);
                                    pb.line_to(a, b);
                                    last = Some(*p1);
                                }
                                OutlineCurve::Quad(_, p1, p2) => {
                                    let (a, b) = map(*p1);
                                    let (c2, d) = map(*p2);
                                    pb.quad_to(a, b, c2, d);
                                    last = Some(*p2);
                                }
                                OutlineCurve::Cubic(_, p1, p2, p3) => {
                                    let (a, b) = map(*p1);
                                    let (c2, d) = map(*p2);
                                    let (e, f) = map(*p3);
                                    pb.cubic_to(a, b, c2, d, e, f);
                                    last = Some(*p3);
                                }
                            }
                        }
                        if last.is_some() {
                            pb.close();
                        }
                        if let Some(path) = pb.finish() {
                            pm.fill_path(&path, &paint(*color, *alpha), FillRule::Winding, sc, None);
                        }
                    }
                    pen += font.h_advance_unscaled(id) * scale;
                }
            }
        }
    }
    pm.encode_png().map_err(|e| e.to_string())
}
