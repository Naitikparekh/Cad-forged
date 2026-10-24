//! Drawing primitives shared by the viewport and the icon renderer: renderer [`Op`]s → egui shapes, dashing,
//! polygon filling with holes, rotated text.
use cadforge_core::picture::Decoded;
use cadforge_render::{Op, Poly};
use eframe::egui::epaint::{Mesh, PathShape, PathStroke, TextShape, Vertex};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke};

pub fn col(rgb: [u8; 3], alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        rgb[0],
        rgb[1],
        rgb[2],
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

pub fn cad_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("cad".into()))
}

fn pos(p: [f64; 2], o: Pos2) -> Pos2 {
    Pos2::new(o.x + p[0] as f32, o.y + p[1] as f32)
}

/// Split a polyline into dash segments. `dash` alternates on / off lengths (pixels); zero-length "on" entries become dots.
pub fn dash_polyline(pts: &[Pos2], closed: bool, dash: &[f32]) -> Vec<Vec<Pos2>> {
    let total: f32 = dash.iter().map(|d| d.abs()).sum();
    if dash.is_empty() || total < 0.5 {
        return vec![pts.to_vec()];
    }
    let mut pts = pts.to_vec();
    if closed && pts.len() > 1 {
        pts.push(pts[0]);
    }
    let mut out: Vec<Vec<Pos2>> = vec![];
    let mut idx = 0usize;
    let mut left = dash[0].abs();
    let mut on = true;
    let mut cur: Vec<Pos2> = vec![];
    if pts.is_empty() {
        return out;
    }
    if on {
        cur.push(pts[0]);
    }
    let mut guard = 0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = (b - a).length();
        if seg < 1e-6 {
            continue;
        }
        let dir = (b - a) / seg;
        let mut t = 0.0f32;
        while t < seg {
            guard += 1;
            if guard > 200_000 {
                return vec![pts];
            }
            let step = left.min(seg - t);
            let p1 = a + dir * (t + step);
            if on {
                cur.push(p1);
            }
            t += step;
            left -= step;
            if left <= 1e-4 {
                if on {
                    if cur.len() == 1 || (cur.len() == 2 && (cur[0] - cur[1]).length() < 0.05) {
                        let p = cur[0];
                        cur = vec![p, p + dir * 0.6];
                    }
                    out.push(std::mem::take(&mut cur));
                }
                idx = (idx + 1) % dash.len();
                on = !on;
                left = dash[idx].abs().max(if on { 0.0 } else { 0.3 });
                if on {
                    cur.push(p1);
                    if left <= 1e-4 {
                        // a dot
                        left = 1e-3;
                    }
                }
            }
        }
    }
    if on && cur.len() > 1 {
        out.push(cur);
    }
    out
}

fn signed_area(p: &[[f64; 2]]) -> f64 {
    let mut s = 0.0;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    s / 2.0
}

fn contains(poly: &[[f64; 2]], q: [f64; 2]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (pi, pj) = (poly[i], poly[j]);
        if (pi[1] > q[1]) != (pj[1] > q[1])
            && q[0] < (pj[0] - pi[0]) * (q[1] - pi[1]) / (pj[1] - pi[1]) + pi[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Fill closed loops (even-odd or non-zero via containment depth): triangulates with holes and returns a mesh.
pub fn fill_mesh(loops: &[Vec<[f64; 2]>], color: Color32, origin: Pos2) -> Mesh {
    let mut mesh = Mesh::default();
    let loops: Vec<&Vec<[f64; 2]>> = loops
        .iter()
        .filter(|l| l.len() >= 3 && l.iter().all(|p| p[0].is_finite() && p[1].is_finite()))
        .collect();
    if loops.is_empty() {
        return mesh;
    }
    // containment depth: even = outer boundary, odd = hole
    let depth: Vec<usize> = loops
        .iter()
        .enumerate()
        .map(|(i, l)| {
            loops
                .iter()
                .enumerate()
                .filter(|(j, o)| *j != i && contains(o, l[0]))
                .count()
        })
        .collect();
    for (i, outer) in loops.iter().enumerate() {
        if depth[i] % 2 != 0 {
            continue;
        }
        let holes: Vec<&Vec<[f64; 2]>> = loops
            .iter()
            .enumerate()
            .filter(|(j, h)| depth[*j] == depth[i] + 1 && contains(outer, h[0]))
            .map(|(_, h)| *h)
            .collect();
        let mut flat: Vec<f64> = vec![];
        let mut hole_idx: Vec<usize> = vec![];
        let mut verts: Vec<[f64; 2]> = vec![];
        for p in outer.iter() {
            flat.extend([p[0], p[1]]);
            verts.push(*p);
        }
        for h in &holes {
            hole_idx.push(verts.len());
            for p in h.iter() {
                flat.extend([p[0], p[1]]);
                verts.push(*p);
            }
        }
        let Ok(tris) = earcutr::earcut(&flat, &hole_idx, 2) else {
            continue;
        };
        let base = mesh.vertices.len() as u32;
        for p in &verts {
            mesh.vertices.push(Vertex {
                pos: pos(*p, origin),
                uv: eframe::egui::epaint::WHITE_UV,
                color,
            });
        }
        for t in tris.chunks_exact(3) {
            mesh.indices
                .extend([base + t[0] as u32, base + t[1] as u32, base + t[2] as u32]);
        }
        let _ = signed_area;
    }
    mesh
}

type ImgKey = (usize, [i32; 3], [u8; 3], bool);

thread_local! {
    /// Uploaded pictures. The pixels are kept alongside the texture so that the key (their address) stays unique.
    static IMAGES: std::cell::RefCell<std::collections::HashMap<ImgKey, (std::rc::Rc<Decoded>, egui::TextureHandle)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Upload (or find) the texture of a picture with its IMAGEADJUST settings applied.
fn image_texture(
    ctx: &egui::Context,
    pix: &std::rc::Rc<Decoded>,
    adj: [i32; 3],
    bg: [u8; 3],
    smooth: bool,
) -> egui::TextureHandle {
    let key: ImgKey = (std::rc::Rc::as_ptr(pix) as usize, adj, bg, smooth);
    IMAGES.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((_, t)) = c.get(&key) {
            return t.clone();
        }
        if c.len() > 24 {
            c.clear();
        }
        let img = egui::ColorImage::from_rgba_unmultiplied(
            [pix.w as usize, pix.h as usize],
            &pix.adjusted(adj[0], adj[1], adj[2], bg),
        );
        let opts = egui::TextureOptions {
            magnification: if smooth {
                egui::TextureFilter::Linear
            } else {
                egui::TextureFilter::Nearest
            },
            minification: egui::TextureFilter::Linear,
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            mipmap_mode: None,
        };
        let t = ctx.load_texture("cad-image", img, opts);
        c.insert(key, (pix.clone(), t.clone()));
        t
    })
}

/// A textured mesh for a picture; the boundary polygon (device space) is triangulated, UVs come from the inverse map.
fn image_mesh(
    tex: &egui::TextureHandle,
    pix: &Decoded,
    m: &[f64; 6],
    clip: &[[f64; 2]],
    alpha: f32,
    origin: Pos2,
) -> Option<Mesh> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-18 {
        return None;
    }
    let uv = |p: [f64; 2]| {
        let (dx, dy) = (p[0] - m[4], p[1] - m[5]);
        let (px, py) = ((dx * m[3] - dy * m[2]) / det, (m[0] * dy - m[1] * dx) / det);
        Pos2::new((px / pix.w as f64) as f32, (py / pix.h as f64) as f32)
    };
    let (w, h) = (pix.w as f64, pix.h as f64);
    let poly: Vec<[f64; 2]> = if clip.len() >= 3 {
        clip.to_vec()
    } else {
        [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]
            .iter()
            .map(|q| {
                [
                    m[0] * q[0] + m[2] * q[1] + m[4],
                    m[1] * q[0] + m[3] * q[1] + m[5],
                ]
            })
            .collect()
    };
    let flat: Vec<f64> = poly.iter().flat_map(|p| [p[0], p[1]]).collect();
    let tris = earcutr::earcut(&flat, &[], 2).ok()?;
    let tint = Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
    let mut mesh = Mesh::with_texture(tex.id());
    for p in &poly {
        mesh.vertices.push(Vertex {
            pos: pos(*p, origin),
            uv: uv(*p),
            color: tint,
        });
    }
    for t in tris.chunks_exact(3) {
        mesh.indices.extend([t[0] as u32, t[1] as u32, t[2] as u32]);
    }
    Some(mesh)
}

/// Draw renderer ops; `origin` is the screen position of device (0, 0).
pub fn paint_ops(painter: &Painter, origin: Pos2, ops: &[Op], clip_arg: Rect) {
    for op in ops {
        match op {
            Op::Stroke {
                paths,
                color,
                alpha,
                width,
                dash,
                round,
            } => {
                let c = col(*color, *alpha);
                let w = (*width as f32).max(0.5);
                let dash_f: Vec<f32> = dash.iter().map(|d| *d as f32).collect();
                let _ = round;
                for p in paths {
                    if p.pts.len() < 2 {
                        continue;
                    }
                    let pts: Vec<Pos2> = p.pts.iter().map(|q| pos(*q, origin)).collect();
                    if !visible(&pts, clip_arg) {
                        continue;
                    }
                    if dash_f.is_empty() {
                        painter.add(Shape::Path(PathShape {
                            points: pts,
                            closed: p.closed,
                            fill: Color32::TRANSPARENT,
                            stroke: PathStroke::new(w, c),
                        }));
                    } else {
                        for seg in dash_polyline(&pts, p.closed, &dash_f) {
                            if seg.len() > 1 {
                                painter.add(Shape::line(seg, Stroke::new(w, c)));
                            }
                        }
                    }
                }
            }
            Op::Fill {
                paths,
                color,
                alpha,
                ..
            } => {
                let loops: Vec<Vec<[f64; 2]>> =
                    paths.iter().map(|p: &Poly| p.pts.clone()).collect();
                let m = fill_mesh(&loops, col(*color, *alpha), origin);
                if !m.is_empty() {
                    painter.add(Shape::mesh(m));
                }
            }
            Op::Image {
                pix,
                adj,
                bg,
                m,
                clip,
                alpha,
                smooth,
            } => {
                let pts: Vec<Pos2> = [
                    [0.0, 0.0],
                    [pix.0.w as f64, 0.0],
                    [pix.0.w as f64, pix.0.h as f64],
                    [0.0, pix.0.h as f64],
                ]
                .iter()
                .map(|q| {
                    pos(
                        [
                            m[0] * q[0] + m[2] * q[1] + m[4],
                            m[1] * q[0] + m[3] * q[1] + m[5],
                        ],
                        origin,
                    )
                })
                .collect();
                if !visible(&pts, clip_arg) {
                    continue;
                }
                let tex = image_texture(painter.ctx(), &pix.0, *adj, *bg, *smooth);
                if let Some(mesh) = image_mesh(&tex, &pix.0, m, clip, *alpha, origin) {
                    painter.add(Shape::mesh(mesh));
                }
            }
            Op::Text {
                s,
                x,
                y,
                m,
                em,
                color,
                alpha,
            } => {
                let sz = (m[0].hypot(m[1]) * *em) as f32;
                if sz < 1.5 || s.is_empty() {
                    continue;
                }
                let angle = m[1].atan2(m[0]) as f32;
                let o = op.text_origin().unwrap_or([0.0, 0.0]);
                let _ = (x, y);
                let galley = painter.layout_no_wrap(s.clone(), cad_font(sz), col(*color, *alpha));
                let base = galley
                    .rows
                    .first()
                    .map(|r| r.pos.y + r.row.glyphs.first().map_or(sz * 0.8, |g| g.pos.y))
                    .unwrap_or(sz * 0.8);
                // TextShape rotates about the galley's top-left corner: place it so the baseline start lands on `o`
                let off = egui::vec2(base * angle.sin(), -base * angle.cos());
                painter.add(
                    TextShape::new(pos(o, origin) + off, galley, col(*color, *alpha))
                        .with_angle(angle),
                );
            }
        }
    }
}

fn visible(pts: &[Pos2], clip: Rect) -> bool {
    let mut b = Rect::NOTHING;
    for p in pts {
        b.extend_with(*p);
    }
    b.intersects(clip.expand(4.0))
}

/// Small helper for centred text.
pub fn text_center(painter: &Painter, c: Pos2, s: &str, font: FontId, color: Color32) {
    painter.text(c, Align2::CENTER_CENTER, s, font, color);
}
