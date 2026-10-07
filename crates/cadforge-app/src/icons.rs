//! The flat icon set (32×32 grid), parsed once from `assets/icons.json` (exported from the web UI) and painted with
//! egui shapes. Icons are tiny SVG fragments: `<path>`, `<circle>`, `<rect>` and `<ellipse>` with a colour class.
use crate::gfx::fill_mesh;
use crate::theme::Tokens;
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};
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
}

pub struct Icons {
    map: HashMap<String, Vec<Prim>>,
}

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
        out.push(Prim { cls, subs, opacity });
    }
    out
}

impl Icons {
    pub fn load() -> Icons {
        let raw: HashMap<String, String> = serde_json::from_str(include_str!("../assets/icons.json")).expect("icons.json");
        Icons { map: raw.iter().map(|(k, v)| (k.clone(), parse(v))).collect() }
    }

    pub fn has(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// Paint icon `name` into `rect` (a 32-unit square scaled to fit).
    pub fn paint(&self, painter: &Painter, name: &str, rect: Rect, t: &Tokens, tint: Tint) {
        let prims = self.map.get(name).or_else(|| self.map.get("cursor"));
        let Some(prims) = prims else { return };
        let k = rect.width().min(rect.height()) / 32.0;
        let org = Pos2::new(rect.center().x - 16.0 * k, rect.center().y - 16.0 * k);
        let map = |p: &[f64; 2]| Pos2::new(org.x + p[0] as f32 * k, org.y + p[1] as f32 * k);
        for pr in prims {
            let base = match tint {
                Tint::Mono(c) => c,
                Tint::Normal => match pr.cls {
                    Cls::L | Cls::FL => t.ic_l,
                    Cls::A | Cls::FA => t.ic_a,
                    Cls::O | Cls::FO => t.ic_o,
                    Cls::G => t.ic_g,
                    Cls::K | Cls::FK => t.ic_k,
                },
            };
            let mut color = base.gamma_multiply(pr.opacity);
            if pr.cls == Cls::FK {
                color = color.gamma_multiply(0.25);
            }
            if is_fill(pr.cls) {
                let loops: Vec<Vec<[f64; 2]>> = pr.subs.iter().filter(|s| s.0.len() >= 3).map(|s| s.0.iter().map(|p| [p[0] * k as f64, p[1] * k as f64]).collect()).collect();
                let m = fill_mesh(&loops, color, org);
                if !m.is_empty() {
                    painter.add(Shape::mesh(m));
                }
            } else {
                let w = (if pr.cls == Cls::K { 1.4 } else { 1.7 } * k).max(0.9);
                for (pts, closed) in &pr.subs {
                    if pts.len() < 2 {
                        continue;
                    }
                    let v: Vec<Pos2> = pts.iter().map(map).collect();
                    painter.add(if *closed { Shape::closed_line(v, Stroke::new(w, color)) } else { Shape::line(v, Stroke::new(w, color)) });
                }
            }
        }
    }
}
