//! The flat icon set (32×32 grid), parsed once from `assets/icons.json`. Icons are tiny SVG fragments: `<path>`,
//! `<circle>`, `<rect>` and `<ellipse>` with a colour class. Each icon is rasterized with anti-aliasing at the exact
//! pixel size it is shown at (round caps and joins, a soft tint inside closed outlines), snapped to the pixel grid and
//! kept as a texture.
use crate::theme::Tokens;
use eframe::egui::{self, Color32, Painter, Pos2, Rect, TextureHandle, TextureOptions};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cls {
    L,
    A,
    O,
    G,
    K,
    FA,
    FO,
    FL,
    FK,
}

#[derive(Clone, Debug)]
struct Prim {
    cls: Cls,
    subs: Vec<(Vec<[f64; 2]>, bool)>,
    opacity: f32,
    /// A small outlined square in a colour: drawn solid, as a grip.
    grip: bool,
}

pub struct Icons {
    map: HashMap<String, Vec<Prim>>,
    /// Rasterized icons by (name, pixel size, palette).
    cache: RefCell<HashMap<(String, u32, [u32; 5]), TextureHandle>>,
}

/// Stroke weights (in 32-unit icon space) and the strength of the tint inside closed outlines.
pub const STROKE: f32 = 2.1;
pub const STROKE_THIN: f32 = 1.5;
pub const BODY_ALPHA: f32 = 0.15;

/// How to colour an icon.
#[derive(Clone, Copy)]
pub enum Tint {
    /// Theme colours per class.
    Normal,
    /// Everything in a single colour (status bar toggles, disabled items).
    Mono(Color32),
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')?;
    Some(&tag[i..i + j])
}

fn num(tag: &str, name: &str) -> f64 {
    attr(tag, name).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn cls_of(s: &str) -> Cls {
    match s {
        "a" => Cls::A,
        "o" => Cls::O,
        "g" => Cls::G,
        "k" => Cls::K,
        "fa" => Cls::FA,
        "fo" => Cls::FO,
        "fl" => Cls::FL,
        "fk" => Cls::FK,
        _ => Cls::L,
    }
}

fn is_fill(c: Cls) -> bool {
    matches!(c, Cls::FA | Cls::FO | Cls::FL | Cls::FK)
}

fn ellipse_pts(cx: f64, cy: f64, rx: f64, ry: f64, rot_deg: f64) -> Vec<[f64; 2]> {
    let n = 40;
    let (s, c) = rot_deg.to_radians().sin_cos();
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64 * std::f64::consts::TAU;
            let (x, y) = (rx * t.cos(), ry * t.sin());
            [cx + x * c - y * s, cy + x * s + y * c]
        })
        .collect()
}

/// Flatten an SVG arc (endpoint parameterisation) into points appended to `out` (excluding the start point).
#[allow(clippy::too_many_arguments)]
fn arc_to(out: &mut Vec<[f64; 2]>, from: [f64; 2], mut rx: f64, mut ry: f64, phi_deg: f64, large: bool, sweep: bool, to: [f64; 2]) {
    if rx == 0.0 || ry == 0.0 || from == to {
        out.push(to);
        return;
    }
    rx = rx.abs();
    ry = ry.abs();
    let phi = phi_deg.to_radians();
    let (sp, cp) = phi.sin_cos();
    let dx = (from[0] - to[0]) / 2.0;
    let dy = (from[1] - to[1]) / 2.0;
    let x1 = cp * dx + sp * dy;
    let y1 = -sp * dx + cp * dy;
    let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lam > 1.0 {
        let k = lam.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let coef = (if large == sweep { -1.0 } else { 1.0 }) * (num / den).max(0.0).sqrt();
    let cxp = coef * rx * y1 / ry;
    let cyp = -coef * ry * x1 / rx;
    let cx = cp * cxp - sp * cyp + (from[0] + to[0]) / 2.0;
    let cy = sp * cxp + cp * cyp + (from[1] + to[1]) / 2.0;
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let th1 = ang(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut dth = ang((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
    if !sweep && dth > 0.0 {
        dth -= std::f64::consts::TAU;
    } else if sweep && dth < 0.0 {
        dth += std::f64::consts::TAU;
    }
    let n = ((dth.abs() / (std::f64::consts::PI / 12.0)).ceil() as usize).max(2);
    for i in 1..=n {
        let t = th1 + dth * i as f64 / n as f64;
        let (x, y) = (rx * t.cos(), ry * t.sin());
        out.push([cp * x - sp * y + cx, sp * x + cp * y + cy]);
    }
}

fn parse_path(d: &str) -> Vec<(Vec<[f64; 2]>, bool)> {
    // tokenise: command letters and numbers (numbers may be glued: "1.5.5", "-3-4", "0 0 1")
    let b = d.as_bytes();
    let mut toks: Vec<Result<char, f64>> = vec![];
    let mut i = 0;
    while i < b.len() {
        let ch = b[i] as char;
        if ch.is_ascii_alphabetic() {
            toks.push(Ok(ch));
            i += 1;
        } else if ch == '-' || ch == '.' || ch.is_ascii_digit() {
            let s = i;
            let mut seen_dot = false;
            let mut seen_exp = false;
            i += 1;
            while i < b.len() {
                let c = b[i] as char;
                if c.is_ascii_digit() {
                    i += 1;
                } else if c == '.' && !seen_dot && !seen_exp {
                    seen_dot = true;
                    i += 1;
                } else if (c == 'e' || c == 'E') && !seen_exp {
                    seen_exp = true;
                    i += 1;
                    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            toks.push(Err(d[s..i].parse().unwrap_or(0.0)));
        } else {
            i += 1;
        }
    }
    let mut subs: Vec<(Vec<[f64; 2]>, bool)> = vec![];
    let mut cur: Vec<[f64; 2]> = vec![];
    let mut p = [0.0, 0.0];
    let mut start = [0.0, 0.0];
    let mut last_ctrl: Option<[f64; 2]> = None;
    let mut cmd = 'M';
    let mut k = 0;
    let nums = |toks: &Vec<Result<char, f64>>, k: &mut usize, n: usize| -> Option<Vec<f64>> {
        let mut v = vec![];
        for _ in 0..n {
            match toks.get(*k) {
                Some(Err(x)) => {
                    v.push(*x);
                    *k += 1;
                }
                _ => return None,
            }
        }
        Some(v)
    };
    while k < toks.len() {
        if let Ok(c) = toks[k] {
            cmd = c;
            k += 1;
            if c == 'Z' || c == 'z' {
                if !cur.is_empty() {
                    subs.push((std::mem::take(&mut cur), true));
                }
                p = start;
                last_ctrl = None;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (p[0], p[1]) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                let Some(v) = nums(&toks, &mut k, 2) else { break };
                if !cur.is_empty() {
                    subs.push((std::mem::take(&mut cur), false));
                }
                p = [ox + v[0], oy + v[1]];
                start = p;
                cur.push(p);
                cmd = if rel { 'l' } else { 'L' };
                last_ctrl = None;
            }
            'L' => {
                let Some(v) = nums(&toks, &mut k, 2) else { break };
                p = [ox + v[0], oy + v[1]];
                cur.push(p);
                last_ctrl = None;
            }
            'H' => {
                let Some(v) = nums(&toks, &mut k, 1) else { break };
                p = [ox + v[0], p[1]];
                cur.push(p);
                last_ctrl = None;
            }
            'V' => {
                let Some(v) = nums(&toks, &mut k, 1) else { break };
                p = [p[0], oy + v[0]];
                cur.push(p);
                last_ctrl = None;
            }
            'C' | 'S' => {
                let s = cmd.to_ascii_uppercase() == 'S';
                let Some(v) = nums(&toks, &mut k, if s { 4 } else { 6 }) else { break };
                let (c1, c2, e) = if s {
                    let c1 = last_ctrl.map_or(p, |l| [2.0 * p[0] - l[0], 2.0 * p[1] - l[1]]);
                    (c1, [ox + v[0], oy + v[1]], [ox + v[2], oy + v[3]])
                } else {
                    ([ox + v[0], oy + v[1]], [ox + v[2], oy + v[3]], [ox + v[4], oy + v[5]])
                };
                for i in 1..=14 {
                    let t = i as f64 / 14.0;
                    let u = 1.0 - t;
                    cur.push([u * u * u * p[0] + 3.0 * u * u * t * c1[0] + 3.0 * u * t * t * c2[0] + t * t * t * e[0], u * u * u * p[1] + 3.0 * u * u * t * c1[1] + 3.0 * u * t * t * c2[1] + t * t * t * e[1]]);
                }
                last_ctrl = Some(c2);
                p = e;
            }
            'Q' | 'T' => {
                let t_ = cmd.to_ascii_uppercase() == 'T';
                let Some(v) = nums(&toks, &mut k, if t_ { 2 } else { 4 }) else { break };
                let (c1, e) = if t_ { (last_ctrl.map_or(p, |l| [2.0 * p[0] - l[0], 2.0 * p[1] - l[1]]), [ox + v[0], oy + v[1]]) } else { ([ox + v[0], oy + v[1]], [ox + v[2], oy + v[3]]) };
                for i in 1..=10 {
                    let t = i as f64 / 10.0;
                    let u = 1.0 - t;
                    cur.push([u * u * p[0] + 2.0 * u * t * c1[0] + t * t * e[0], u * u * p[1] + 2.0 * u * t * c1[1] + t * t * e[1]]);
                }
                last_ctrl = Some(c1);
                p = e;
            }
            'A' => {
                let Some(v) = nums(&toks, &mut k, 7) else { break };
                let e = [ox + v[5], oy + v[6]];
                arc_to(&mut cur, p, v[0], v[1], v[2], v[3] != 0.0, v[4] != 0.0, e);
                p = e;
                last_ctrl = None;
            }
            _ => break,
        }
    }
    if !cur.is_empty() {
        subs.push((cur, false));
    }
    subs
}

fn parse(svg: &str) -> Vec<Prim> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(i) = rest.find('<') {
        let Some(j) = rest[i..].find("/>") else { break };
        let tag = &rest[i..i + j + 2];
        rest = &rest[i + j + 2..];
        let cls = cls_of(attr(tag, "class").unwrap_or("l"));
        let opacity = attr(tag, "opacity").and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let grip = tag.starts_with("<rect") && !is_fill(cls) && cls != Cls::K && cls != Cls::L && num(tag, "width") <= 6.0 && num(tag, "height") <= 6.0;
        let subs = if tag.starts_with("<path") {
            parse_path(attr(tag, "d").unwrap_or(""))
        } else if tag.starts_with("<circle") {
            vec![(ellipse_pts(num(tag, "cx"), num(tag, "cy"), num(tag, "r"), num(tag, "r"), 0.0), true)]
        } else if tag.starts_with("<ellipse") {
            let rot = attr(tag, "transform").and_then(|t| t.strip_prefix("rotate(")).and_then(|t| t.split([' ', ',', ')']).next()).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            vec![(ellipse_pts(num(tag, "cx"), num(tag, "cy"), num(tag, "rx"), num(tag, "ry"), rot), true)]
        } else if tag.starts_with("<rect") {
            let (x, y, w, h, r) = (num(tag, "x"), num(tag, "y"), num(tag, "width"), num(tag, "height"), num(tag, "rx"));
            let r = r.min(w / 2.0).min(h / 2.0);
            let pts = if r > 0.1 {
                let mut p = vec![];
                for (cx, cy, a0) in [(x + w - r, y + r, -90.0f64), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)] {
                    for i in 0..=4 {
                        let a = (a0 + 90.0 * i as f64 / 4.0).to_radians();
                        p.push([cx + r * a.cos(), cy + r * a.sin()]);
                    }
                }
                p
            } else {
                vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
            };
            vec![(pts, true)]
        } else {
            continue;
        };
        out.push(Prim { cls, subs, opacity, grip });
    }
    out
}

impl Icons {
    pub fn load() -> Icons {
        let raw: HashMap<String, String> = serde_json::from_str(include_str!("../assets/icons.json")).expect("icons.json");
        Icons { map: raw.iter().map(|(k, v)| (k.clone(), parse(v))).collect(), cache: RefCell::new(HashMap::new()) }
    }

    pub fn has(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.map.keys().cloned().collect();
        v.sort();
        v
    }

    /// Icon `name` as `px`×`px` premultiplied RGBA, coloured by `pal` (line, accent, orange, green, grey).
    pub fn raster(&self, name: &str, px: u32, pal: [Color32; 5]) -> Option<tiny_skia::Pixmap> {
        let prims = self.map.get(name).or_else(|| self.map.get("cursor"))?;
        let mut pm = tiny_skia::Pixmap::new(px.max(1), px.max(1))?;
        let k = px as f32 / 32.0;
        let tf = tiny_skia::Transform::from_scale(k, k);
        let path_of = |subs: &[(Vec<[f64; 2]>, bool)], closed_only: bool| -> Option<tiny_skia::Path> {
            let mut pb = tiny_skia::PathBuilder::new();
            for (pts, closed) in subs {
                if pts.len() < 2 || (closed_only && (!closed || pts.len() < 3)) {
                    continue;
                }
                pb.move_to(pts[0][0] as f32, pts[0][1] as f32);
                for p in &pts[1..] {
                    pb.line_to(p[0] as f32, p[1] as f32);
                }
                if *closed {
                    pb.close();
                }
            }
            pb.finish()
        };
        let paint_of = |c: Color32, alpha: f32| {
            let mut p = tiny_skia::Paint::default();
            p.anti_alias = true;
            p.set_color(tiny_skia::Color::from_rgba8(c.r(), c.g(), c.b(), ((c.a() as f32) * alpha).round().clamp(0.0, 255.0) as u8));
            p
        };
        for pr in prims {
            let base = match pr.cls {
                Cls::L | Cls::FL => pal[0],
                Cls::A | Cls::FA => pal[1],
                Cls::O | Cls::FO => pal[2],
                Cls::G => pal[3],
                Cls::K | Cls::FK => pal[4],
            };
            let alpha = pr.opacity * if pr.cls == Cls::FK { 0.3 } else { 1.0 };
            if is_fill(pr.cls) {
                if let Some(path) = path_of(&pr.subs, false) {
                    pm.fill_path(&path, &paint_of(base, alpha), tiny_skia::FillRule::EvenOdd, tf, None);
                }
                continue;
            }
            // grips (small squares) are solid, like the grips on the drawing
            if pr.grip {
                if let Some(path) = path_of(&pr.subs, false) {
                    pm.fill_path(&path, &paint_of(base, alpha), tiny_skia::FillRule::EvenOdd, tf, None);
                }
                continue;
            }
            // a soft tint inside closed outlines gives shapes body: stronger at the top, fading down
            if pr.cls != Cls::K {
                if let Some(path) = path_of(&pr.subs, true) {
                    let b = path.bounds();
                    let stop = |a: f32| tiny_skia::Color::from_rgba8(base.r(), base.g(), base.b(), ((base.a() as f32) * alpha * a).round().clamp(0.0, 255.0) as u8);
                    let shader = tiny_skia::LinearGradient::new(
                        tiny_skia::Point::from_xy(b.left(), b.top()),
                        tiny_skia::Point::from_xy(b.left(), b.bottom()),
                        vec![tiny_skia::GradientStop::new(0.0, stop(BODY_ALPHA * 1.7)), tiny_skia::GradientStop::new(1.0, stop(BODY_ALPHA * 0.5))],
                        tiny_skia::SpreadMode::Pad,
                        tiny_skia::Transform::identity(),
                    );
                    let mut paint = tiny_skia::Paint::default();
                    paint.anti_alias = true;
                    match shader {
                        Some(sh) => paint.shader = sh,
                        None => paint = paint_of(base, alpha * BODY_ALPHA),
                    }
                    pm.fill_path(&path, &paint, tiny_skia::FillRule::EvenOdd, tf, None);
                }
            }
            if let Some(path) = path_of(&pr.subs, false) {
                // at small sizes keep lines at least one device pixel wide
                let w = (if pr.cls == Cls::K { STROKE_THIN } else { STROKE }).max(1.0 / k);
                let stroke = tiny_skia::Stroke { width: w, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
                pm.stroke_path(&path, &paint_of(base, alpha), &stroke, tf, None);
            }
        }
        Some(pm)
    }

    /// Paint icon `name` into `rect` (a 32-unit square scaled to fit, snapped to whole device pixels).
    pub fn paint(&self, painter: &Painter, name: &str, rect: Rect, t: &Tokens, tint: Tint) {
        let pal = match tint {
            Tint::Mono(c) => [c; 5],
            Tint::Normal => [t.ic_l, t.ic_a, t.ic_o, t.ic_g, t.ic_k],
        };
        let ctx = painter.ctx();
        let ppp = ctx.pixels_per_point();
        let px = (rect.width().min(rect.height()) * ppp).round().max(4.0) as u32;
        let key = (name.to_string(), px, pal.map(|c| u32::from_le_bytes(c.to_array())));
        let tex = {
            let mut cache = self.cache.borrow_mut();
            if let Some(t) = cache.get(&key) {
                t.clone()
            } else {
                let Some(pm) = self.raster(name, px, pal) else { return };
                let img = egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pm.data());
                let t = ctx.load_texture(format!("icon:{name}:{px}:{:?}", key.2), img, TextureOptions::LINEAR);
                cache.insert(key, t.clone());
                t
            }
        };
        // whole device pixels, so a 1 px line stays 1 px
        let side = px as f32 / ppp;
        let min = Pos2::new(((rect.center().x - side / 2.0) * ppp).round() / ppp, ((rect.center().y - side / 2.0) * ppp).round() / ppp);
        painter.image(tex.id(), Rect::from_min_size(min, egui::vec2(side, side)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes a contact sheet of every icon (dark and light theme, 32 and 16 px) to `$ICON_SHEET` when set.
    #[test]
    fn every_icon_rasterizes() {
        let icons = Icons::load();
        let (d, l) = (crate::theme::DARK, crate::theme::LIGHT);
        let names = icons.names();
        for n in &names {
            let pm = icons.raster(n, 32, [d.ic_l, d.ic_a, d.ic_o, d.ic_g, d.ic_k]).unwrap();
            assert!(pm.data().chunks(4).any(|p| p[3] > 0), "{n} is empty");
        }
        let Ok(out) = std::env::var("ICON_SHEET") else { return };
        let cols = 16u32;
        let rows = (names.len() as u32).div_ceil(cols);
        let cell = 44u32;
        let mut sheet = tiny_skia::Pixmap::new(cols * cell * 2, rows * cell).unwrap();
        for (half, th) in [(0u32, d), (1, l)] {
            let bg = th.ribbon;
            let rect = tiny_skia::Rect::from_xywh((half * cols * cell) as f32, 0.0, (cols * cell) as f32, (rows * cell) as f32).unwrap();
            let mut p = tiny_skia::Paint::default();
            p.set_color_rgba8(bg.r(), bg.g(), bg.b(), 255);
            sheet.fill_rect(rect, &p, tiny_skia::Transform::identity(), None);
            for (i, n) in names.iter().enumerate() {
                let (cx, cy) = (half * cols * cell + (i as u32 % cols) * cell, (i as u32 / cols) * cell);
                let pal = [th.ic_l, th.ic_a, th.ic_o, th.ic_g, th.ic_k];
                let big = icons.raster(n, 32, pal).unwrap();
                sheet.draw_pixmap(cx as i32 + 2, cy as i32 + 2, big.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
                let small = icons.raster(n, 16, pal).unwrap();
                sheet.draw_pixmap(cx as i32 + 26, cy as i32 + 26, small.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            }
        }
        sheet.save_png(&out).unwrap();
        std::fs::write(format!("{out}.txt"), names.join("\n")).unwrap();
    }
}
