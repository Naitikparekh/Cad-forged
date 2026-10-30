//! The CadForge logo: a shaded isometric cube in white on a blue rounded tile, with an amber grip on its front corner.
//! Drawn here (64-unit design grid) so the title bar, the window icon, the Windows program icon (`assets/logo`,
//! written by the test below) and the browser tab icon all come from one place.
use eframe::egui::{self, Color32, Painter, Pos2, Rect, TextureHandle, TextureOptions};
use std::cell::RefCell;

const TILE_TOP: (u8, u8, u8) = (0x35, 0xa0, 0xff);
const TILE_BOTTOM: (u8, u8, u8) = (0x0b, 0x4f, 0xb0);
const GRIP: (u8, u8, u8) = (0xff, 0xb3, 0x00);
const INK: (u8, u8, u8) = (0x08, 0x2f, 0x6b);

fn poly(pts: &[(f32, f32)]) -> tiny_skia::Path {
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish().expect("polygon")
}

fn solid((r, g, b): (u8, u8, u8), a: f32) -> tiny_skia::Paint<'static> {
    let mut p = tiny_skia::Paint::default();
    p.anti_alias = true;
    p.set_color_rgba8(r, g, b, (a * 255.0).round() as u8);
    p
}

/// The logo as `px`×`px` premultiplied RGBA. `tile` false leaves out the blue tile (the mark alone, for dark chrome).
pub fn render(px: u32, tile: bool) -> tiny_skia::Pixmap {
    let mut pm = tiny_skia::Pixmap::new(px.max(1), px.max(1)).expect("pixmap");
    let k = px as f32 / 64.0;
    let tf = tiny_skia::Transform::from_scale(k, k);
    if tile {
        let rr = tiny_skia::PathBuilder::from_rect(tiny_skia::Rect::from_xywh(2.0, 2.0, 60.0, 60.0).expect("rect"));
        // rounded tile: a rect with its corners cut by circles
        let mut pb = tiny_skia::PathBuilder::new();
        let (x0, y0, x1, y1, r) = (2.0f32, 2.0f32, 62.0f32, 62.0f32, 14.0f32);
        let c = 0.5523 * r;
        pb.move_to(x0 + r, y0);
        pb.line_to(x1 - r, y0);
        pb.cubic_to(x1 - r + c, y0, x1, y0 + r - c, x1, y0 + r);
        pb.line_to(x1, y1 - r);
        pb.cubic_to(x1, y1 - r + c, x1 - r + c, y1, x1 - r, y1);
        pb.line_to(x0 + r, y1);
        pb.cubic_to(x0 + r - c, y1, x0, y1 - r + c, x0, y1 - r);
        pb.line_to(x0, y0 + r);
        pb.cubic_to(x0, y0 + r - c, x0 + r - c, y0, x0 + r, y0);
        pb.close();
        let tile_path = pb.finish().unwrap_or(rr);
        let mut paint = tiny_skia::Paint::default();
        paint.anti_alias = true;
        let col = |(r, g, b): (u8, u8, u8)| tiny_skia::Color::from_rgba8(r, g, b, 255);
        if let Some(sh) = tiny_skia::LinearGradient::new(tiny_skia::Point::from_xy(8.0, 2.0), tiny_skia::Point::from_xy(56.0, 62.0), vec![tiny_skia::GradientStop::new(0.0, col(TILE_TOP)), tiny_skia::GradientStop::new(1.0, col(TILE_BOTTOM))], tiny_skia::SpreadMode::Pad, tiny_skia::Transform::identity()) {
            paint.shader = sh;
        }
        pm.fill_path(&tile_path, &paint, tiny_skia::FillRule::Winding, tf, None);
        // a faint sheen on the upper half
        let sheen = poly(&[(2.0, 2.0), (62.0, 2.0), (62.0, 26.0), (2.0, 38.0)]);
        let mut mask = tiny_skia::Mask::new(px.max(1), px.max(1)).expect("mask");
        mask.fill_path(&tile_path, tiny_skia::FillRule::Winding, true, tf);
        pm.fill_path(&sheen, &solid((255, 255, 255), 0.07), tiny_skia::FillRule::Winding, tf, Some(&mask));
    }
    // the cube: top, left and right faces in three whites
    let (t, ul, ur, c, ll, lr, b) = ((32.0, 13.0), (14.5, 23.0), (49.5, 23.0), (32.0, 33.0), (14.5, 43.0), (49.5, 43.0), (32.0, 53.0));
    let face_alpha = if tile { [1.0, 0.62, 0.34] } else { [1.0, 0.7, 0.42] };
    pm.fill_path(&poly(&[t, ur, c, ul]), &solid((255, 255, 255), face_alpha[0]), tiny_skia::FillRule::Winding, tf, None);
    pm.fill_path(&poly(&[ul, c, b, ll]), &solid((255, 255, 255), face_alpha[1]), tiny_skia::FillRule::Winding, tf, None);
    pm.fill_path(&poly(&[c, ur, lr, b]), &solid((255, 255, 255), face_alpha[2]), tiny_skia::FillRule::Winding, tf, None);
    // the grip on the front corner
    let g = 4.2;
    let grip = poly(&[(c.0 - g, c.1 - g), (c.0 + g, c.1 - g), (c.0 + g, c.1 + g), (c.0 - g, c.1 + g)]);
    pm.fill_path(&grip, &solid(GRIP, 1.0), tiny_skia::FillRule::Winding, tf, None);
    let stroke = tiny_skia::Stroke { width: 1.8, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
    pm.stroke_path(&grip, &solid(INK, 0.9), &stroke, tf, None);
    pm
}

thread_local! {
    static TEX: RefCell<Vec<(u32, bool, TextureHandle)>> = const { RefCell::new(Vec::new()) };
}

/// Paint the logo centred on `c`, `size` points wide, at the screen's pixel density.
pub fn paint(p: &Painter, c: Pos2, size: f32, tile: bool) {
    let ctx = p.ctx();
    let ppp = ctx.pixels_per_point();
    let px = (size * ppp).round().max(8.0) as u32;
    let tex = TEX.with(|t| {
        let mut t = t.borrow_mut();
        if let Some((_, _, h)) = t.iter().find(|(s, w, _)| *s == px && *w == tile) {
            return h.clone();
        }
        let pm = render(px, tile);
        let img = egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pm.data());
        let h = ctx.load_texture(format!("logo:{px}:{tile}"), img, TextureOptions::LINEAR);
        t.push((px, tile, h.clone()));
        h
    });
    let side = px as f32 / ppp;
    let min = Pos2::new(((c.x - side / 2.0) * ppp).round() / ppp, ((c.y - side / 2.0) * ppp).round() / ppp);
    p.image(tex.id(), Rect::from_min_size(min, egui::vec2(side, side)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
}

/// Straight (not premultiplied) RGBA, for the window icon.
pub fn rgba(px: u32) -> Vec<u8> {
    render(px, true).pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect()
}

/// A Windows .ico holding PNG images of the given sizes.
pub fn ico(sizes: &[u32]) -> Vec<u8> {
    let pngs: Vec<Vec<u8>> = sizes.iter().map(|s| render(*s, true).encode_png().expect("png")).collect();
    let mut out = vec![0u8, 0, 1, 0];
    out.extend((sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, png) in sizes.iter().zip(&pngs) {
        let d = if *s >= 256 { 0 } else { *s as u8 };
        out.extend([d, d, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((png.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in pngs {
        out.extend(png);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files in `assets/logo` are made from this code: `LOGO_OUT=assets/logo cargo test -p
    /// cadforge-app logo` writes them again. Without it the test checks that they are up to date.
    #[test]
    fn logo_files_are_current() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/logo");
        let files: Vec<(String, Vec<u8>)> = [16u32, 32, 48, 64, 128, 256, 512].iter().map(|s| (format!("cadforge-{s}.png"), render(*s, true).encode_png().unwrap())).chain([("cadforge.ico".to_string(), ico(&[16, 24, 32, 48, 64, 128, 256]))]).collect();
        if let Ok(out) = std::env::var("LOGO_OUT") {
            std::fs::create_dir_all(&out).unwrap();
            for (n, b) in &files {
                std::fs::write(std::path::Path::new(&out).join(n), b).unwrap();
            }
            return;
        }
        for (n, b) in &files {
            assert_eq!(std::fs::read(dir.join(n)).ok().as_ref(), Some(b), "{n} is out of date: run with LOGO_OUT=assets/logo (relative to the crate)");
        }
        let px = render(64, true);
        assert!(px.pixel(32, 33).unwrap().alpha() == 255, "the grip is solid");
        assert!(px.pixel(0, 0).unwrap().alpha() == 0, "the corners are transparent");
    }
}
