//! Scene renderer (port of `src/render/renderer.js`): draws the drawing database into [`Op`]s using the camera's
//! affine map. Geometry is traced in world coordinates, flattened under the world→device transform, and stroked with
//! widths/dashes in device units.
use crate::ops::*;
use crate::theme::{Prefs, Theme};
use cadforge_core::dim::{arrow_lines, arrow_polygon, dim_geometry};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::style::{color_rgb, linetype, lw_pixels, Color, Layer, LW_DEFAULT};
use cadforge_core::text::{text_layout, TextData};
use cadforge_core::view::View;
use std::collections::{HashMap, HashSet};

const MAX_DEPTH: usize = 8;

/// How lineweights become stroke widths.
#[derive(Clone, Copy, Debug)]
pub enum LwMode {
    /// Screen: pixels from the lineweight (1 px when lineweight display is off).
    Screen,
    /// Plot: device units are 0.1 mm; `true` honours lineweights, `false` draws everything at 0.2 mm.
    Plot(bool),
}

/// Selection / hover highlighting for the viewport.
#[derive(Clone, Debug, Default)]
pub struct Highlight {
    pub selected: HashSet<u64>,
    pub hover: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Style {
    pub raw_color: Color,
    pub color: Rgb,
    pub ltype: String,
    pub lw: i32,
    pub lts: f64,
    pub alpha: f32,
}

#[derive(Clone, Copy, Default)]
pub struct DrawOpts<'a> {
    pub parent: Option<&'a Style>,
    pub selected: bool,
    pub hover: bool,
    pub depth: usize,
}

pub struct Renderer<'a> {
    pub doc: &'a Drawing,
    pub view: &'a View,
    pub w: f64,
    pub h: f64,
    pub theme: Theme,
    pub prefs: Prefs,
    pub world: Mat,
    pub mono: bool,
    pub lw_mode: LwMode,
    /// Flatness of curves in device units.
    pub tol: f64,
    pub ops: Vec<Op>,
    layers: HashMap<String, &'a Layer>,
    fade: f64,
    ltscale: f64,
    vis: Option<Bounds>,
}

impl<'a> Renderer<'a> {
    pub fn new(doc: &'a Drawing, view: &'a View, w: f64, h: f64, theme: Theme, prefs: Prefs) -> Renderer<'a> {
        let layers = doc.layers.iter().map(|l| (l.name.to_lowercase(), l)).collect();
        Renderer {
            doc,
            view,
            w,
            h,
            theme,
            prefs,
            world: view.matrix(),
            mono: false,
            lw_mode: LwMode::Screen,
            tol: 0.25,
            ops: vec![],
            layers,
            fade: if prefs.lock_fade > 0.0 { prefs.lock_fade / 100.0 } else { 0.0 },
            ltscale: doc.settings.ltscale,
            vis: None,
        }
    }

    /// Pixels (or device units) per drawing unit under the current world matrix.
    pub fn k(&self) -> f64 {
        mat::det(&self.world).abs().sqrt()
    }

    fn layer_of(&self, e: &Entity) -> &'a Layer {
        let name = if e.layer.is_empty() { "0".to_string() } else { e.layer.to_lowercase() };
        self.layers.get(&name).copied().or_else(|| self.layers.get("0").copied()).unwrap_or(&self.doc.layers[0])
    }

    /// Resolve colour/linetype/lineweight/transparency with BYLAYER/BYBLOCK inheritance.
    pub fn style_of(&self, e: &Entity, parent: Option<&Style>) -> Style {
        let layer = self.layer_of(e);
        let raw_color = match e.color {
            None | Some(Color::ByLayer) => layer.color,
            Some(Color::ByBlock) => parent.map_or(layer.color, |p| p.raw_color),
            Some(c) => c,
        };
        let ltype = match e.ltype.as_deref() {
            None | Some("BYLAYER") => layer.ltype.clone(),
            Some("BYBLOCK") => parent.map_or_else(|| layer.ltype.clone(), |p| p.ltype.clone()),
            Some(l) => l.to_string(),
        };
        let mut lw = match e.lw {
            None | Some(-1) => layer.lw,
            Some(-2) => parent.map_or(layer.lw, |p| p.lw),
            Some(v) => v,
        };
        if lw == -3 {
            lw = LW_DEFAULT;
        }
        let transp = e.transp.or(layer.transp).unwrap_or(0.0);
        let fade = if layer.locked && self.fade > 0.0 { (1.0 - self.fade).max(0.12) } else { 1.0 };
        let color = if self.mono { [0, 0, 0] } else { color_rgb(raw_color, self.theme.dark, self.theme.contrast) };
        Style {
            raw_color,
            color,
            ltype,
            lw,
            lts: e.lts.unwrap_or(1.0) * parent.map_or(1.0, |p| p.lts),
            alpha: ((if self.prefs.transparency { 1.0 - transp.min(90.0) / 100.0 } else { 1.0 }) * fade) as f32,
        }
    }

    // ------------------------------------------------------------ tracing

    fn dev(&self, p: Pt) -> [f64; 2] {
        let q = mat::apply(&self.world, p);
        [q.x, q.y]
    }

    fn arc_into(&self, out: &mut Vec<[f64; 2]>, c: Pt, rx: f64, ry: f64, rot: f64, a0: f64, sw: f64) {
        let k = self.k();
        let r_dev = rx.max(ry) * k;
        let step_max = if r_dev > self.tol { 2.0 * (1.0 - self.tol / r_dev).max(0.0).acos() } else { PI / 2.0 };
        let n = ((sw.abs() / step_max.max(1e-3)).ceil() as usize).clamp(4, 4000);
        let (cr, sr) = (rot.cos(), rot.sin());
        for i in 0..=n {
            let t = a0 + sw * i as f64 / n as f64;
            let (x, y) = (rx * t.cos(), ry * t.sin());
            out.push(self.dev(pt(c.x + x * cr - y * sr, c.y + x * sr + y * cr)));
        }
    }

    fn trace_segs(&self, segs: &[Seg], closed: bool) -> Vec<Poly> {
        let mut pts: Vec<[f64; 2]> = vec![];
        for s in segs {
            match *s {
                Seg::L { a, b, .. } => {
                    if pts.is_empty() {
                        pts.push(self.dev(a));
                    }
                    pts.push(self.dev(b));
                }
                Seg::A { c, r, a0, sw, .. } => {
                    self.arc_into(&mut pts, c, r, r, 0.0, a0, sw);
                }
            }
        }
        vec![Poly { pts, closed }]
    }

    fn visible_world(&mut self) -> Bounds {
        if let Some(v) = self.vis {
            return v;
        }
        let inv = mat::invert(&self.world);
        let c: Vec<Pt> = [(0.0, 0.0), (self.w, 0.0), (self.w, self.h), (0.0, self.h)].iter().map(|(x, y)| mat::apply(&inv, pt(*x, *y))).collect();
        let b = Bounds::of_points(&c).grow(1.0);
        self.vis = Some(b);
        b
    }

    fn trace_entity(&mut self, e: &Entity) -> Option<Vec<Poly>> {
        match &e.kind {
            Kind::Line { a, b } => Some(vec![Poly { pts: vec![self.dev(*a), self.dev(*b)], closed: false }]),
            Kind::Circle { c, r } => {
                let mut pts = vec![];
                self.arc_into(&mut pts, *c, *r, *r, 0.0, 0.0, TAU);
                Some(vec![Poly { pts, closed: false }])
            }
            Kind::Arc { c, r, a0, a1 } => {
                let sw = {
                    let s = sweep_ccw(*a0, *a1);
                    if s == 0.0 {
                        TAU
                    } else {
                        s
                    }
                };
                let mut pts = vec![];
                self.arc_into(&mut pts, *c, *r, *r, 0.0, *a0, sw);
                Some(vec![Poly { pts, closed: false }])
            }
            Kind::Pline { closed, .. } => Some(self.trace_segs(&segs_of(e, self.doc), *closed)),
            Kind::Ellipse(el) => {
                let (rx, rot, sw) = (len(el.m), el.m.y.atan2(el.m.x), ellipse_sweep(el));
                let mut pts = vec![];
                self.arc_into(&mut pts, el.c, rx, rx * el.ratio, rot, el.t0, sw);
                Some(vec![Poly { pts, closed: false }])
            }
            Kind::Spline { fit, closed } => {
                let pts: Vec<[f64; 2]> = spline_points(fit, *closed, 14).into_iter().map(|p| self.dev(p)).collect();
                Some(vec![Poly { pts, closed: *closed }])
            }
            Kind::Xline { p, d } | Kind::Ray { p, d } => {
                let b = self.visible_world();
                let both = matches!(e.kind, Kind::Xline { .. });
                clip_line_to_rect(*p, *d, &b, both).map(|(a, c)| vec![Poly { pts: vec![self.dev(a), self.dev(c)], closed: false }])
            }
            Kind::Leader { pts, .. } => Some(vec![Poly { pts: pts.iter().map(|p| self.dev(*p)).collect(), closed: false }]),
            _ => None,
        }
    }

    // ------------------------------------------------------------ stroking

    fn dash_for(&self, style: &Style) -> Option<Vec<f64>> {
        let lt = linetype(&style.ltype)?;
        if lt.pattern.is_empty() {
            return None;
        }
        let unit = self.ltscale * style.lts * self.k();
        // pattern alternates dash,gap; zero-length dashes (dots) remain at ~0
        let mut out: Vec<f64> = lt.pattern.iter().map(|v| if *v == 0.0 { 0.01 } else { v.abs() * unit }).collect();
        let total: f64 = out.iter().sum();
        if !(3.5..=20000.0).contains(&total) {
            return None; // too dense to be useful: draw solid
        }
        if out.len() % 2 == 1 {
            let c = out.clone();
            out.extend(c);
        }
        Some(out)
    }

    fn lw_px(&self, lw: i32) -> f64 {
        match self.lw_mode {
            LwMode::Screen => lw_pixels(lw, self.prefs.lw_display),
            LwMode::Plot(true) => (lw as f64 / 10.0).max(0.6),
            LwMode::Plot(false) => 2.0,
        }
    }

    fn stroke(&mut self, paths: Vec<Poly>, style: &Style, selected: bool, hover: bool, force_width: f64) {
        if paths.iter().all(|p| p.pts.len() < 2) {
            return;
        }
        let w = if force_width > 0.0 { force_width } else { self.lw_px(style.lw) };
        if selected {
            self.ops.push(Op::Stroke { paths: paths.clone(), color: self.theme.select, alpha: 0.38 * style.alpha, width: w + 5.0, dash: vec![], round: true });
        }
        let dash = if selected { Some(vec![7.0, 5.0]) } else { self.dash_for(style) };
        self.ops.push(Op::Stroke {
            paths,
            color: if hover { self.theme.hover } else { style.color },
            alpha: style.alpha,
            width: if hover { w + 1.2 } else { w },
            dash: dash.unwrap_or_default(),
            round: true,
        });
    }

    fn fill_poly(&mut self, pts: &[Pt], style: &Style) {
        let poly = Poly { pts: pts.iter().map(|p| self.dev(*p)).collect(), closed: true };
        self.ops.push(Op::Fill { paths: vec![poly], color: style.color, alpha: style.alpha, evenodd: false });
    }

    // ------------------------------------------------------------ entities

    pub fn draw_entity(&mut self, e: &Entity, o: DrawOpts) {
        let style = self.style_of(e, o.parent);
        self.draw_entity_styled(e, &style, o);
    }

    pub fn draw_entity_styled(&mut self, e: &Entity, style: &Style, o: DrawOpts) {
        match &e.kind {
            Kind::Line { .. } | Kind::Circle { .. } | Kind::Arc { .. } | Kind::Pline { .. } | Kind::Ellipse(_) | Kind::Spline { .. } | Kind::Xline { .. } | Kind::Ray { .. } => {
                if let Some(paths) = self.trace_entity(e) {
                    self.stroke(paths, style, o.selected, o.hover, 0.0);
                }
            }
            Kind::Point { p } => self.draw_point(*p, style, o.selected, o.hover),
            Kind::Text(t) if t.att.is_some() => {
                // an attribute template shows its tag until a block reference supplies a value
                let mut shown = t.clone();
                if let Some(a) = &t.att {
                    shown.s = a.tag.clone();
                }
                self.draw_text_at(&shown, style.color, style.alpha, o.selected, o.hover)
            }
            Kind::Text(t) => self.draw_text_at(t, style.color, style.alpha, o.selected, o.hover),
            Kind::Hatch(h) => self.draw_hatch(e, h, style, o.selected, o.hover),
            Kind::Dim(_) => self.draw_dim(e, style, o.selected, o.hover),
            Kind::Leader { pts, s, h } => self.draw_leader(pts, s, *h, style, o.selected, o.hover),
            Kind::Insert(ins) => self.draw_insert(e, ins, style, o),
            Kind::Table(t) => {
                let (segs, texts) = table_geometry(t);
                let paths: Vec<Poly> = segs.iter().flat_map(|sg| self.trace_segs(std::slice::from_ref(sg), false)).collect();
                self.stroke(paths, style, o.selected, o.hover, 0.0);
                for tx in &texts {
                    self.draw_text_at(tx, style.color, style.alpha, false, o.hover);
                }
            }
            Kind::Wipeout(w) => {
                let pts: Vec<[f64; 2]> = w.pts.iter().map(|p| self.dev(*p)).collect();
                if pts.len() >= 3 {
                    self.ops.push(Op::Fill { paths: vec![Poly { pts: pts.clone(), closed: true }], color: self.theme.canvas, alpha: 1.0, evenodd: false });
                    // the frame is shown on screen (WIPEOUTFRAME 1) but never plotted
                    if !matches!(self.lw_mode, LwMode::Plot(_)) || o.selected {
                        self.stroke(vec![Poly { pts, closed: true }], style, o.selected, o.hover, 0.0);
                    }
                }
            }
            Kind::Region(r) => {
                let paths = self.loops_paths(&r.loops);
                self.stroke(paths, style, o.selected, o.hover, 0.0);
            }
            Kind::Viewport(v) => {
                let c = v.c;
                let corners = [pt(c.x - v.w / 2.0, c.y - v.h / 2.0), pt(c.x + v.w / 2.0, c.y - v.h / 2.0), pt(c.x + v.w / 2.0, c.y + v.h / 2.0), pt(c.x - v.w / 2.0, c.y + v.h / 2.0)];
                let paths = vec![Poly { pts: corners.iter().map(|p| self.dev(*p)).collect(), closed: true }];
                self.stroke(paths, style, o.selected, false, 0.0);
            }
        }
    }

    fn draw_insert(&mut self, e: &Entity, ins: &InsertData, style: &Style, o: DrawOpts) {
        let Some(def) = self.doc.blocks.get(&ins.name) else { return };
        if o.depth > MAX_DEPTH {
            return;
        }
        let saved = self.world;
        let saved_vis = self.vis;
        self.world = mat::mul(&saved, &insert_matrix(ins, def.base));
        self.vis = None;
        for be in &def.entities {
            if matches!(&be.kind, Kind::Text(t) if t.att.is_some()) {
                continue; // the reference's own ATTRIB values are drawn below
            }
            let on_zero = be.layer.is_empty() || be.layer == "0";
            if !on_zero {
                if let Some(l) = self.layers.get(&be.layer.to_lowercase()) {
                    if !l.on || l.frozen {
                        continue;
                    }
                }
            }
            let child;
            let ent = if on_zero {
                let mut c = be.clone();
                c.layer = e.layer.clone();
                child = c;
                &child
            } else {
                be
            };
            self.draw_entity(ent, DrawOpts { parent: Some(style), selected: o.selected, hover: o.hover, depth: o.depth + 1 });
        }
        self.world = saved;
        self.vis = saved_vis;
        for a in &ins.attribs {
            if !a.hidden && !a.t.s.is_empty() {
                self.draw_text_at(&a.t, style.color, style.alpha, false, o.hover);
            }
        }
    }

    fn draw_point(&mut self, p: Pt, style: &Style, selected: bool, hover: bool) {
        let mode = self.doc.settings.pdmode;
        let base = mode & 7;
        let size = self.doc.settings.pdsize;
        let k = self.k();
        let px = if size > 0.0 {
            size * k
        } else if size < 0.0 {
            (-size / 100.0) * self.h
        } else {
            self.h * 0.05
        };
        let s = self.dev(p);
        let half = px / 2.0;
        let color = if hover { self.theme.hover } else if selected { self.theme.select } else { style.color };
        let mut paths: Vec<Poly> = vec![];
        let seg = |a: [f64; 2], b: [f64; 2]| Poly { pts: vec![a, b], closed: false };
        match base {
            0 => {
                self.ops.push(Op::Fill { paths: vec![Poly { pts: vec![[s[0] - 1.0, s[1] - 1.0], [s[0] + 1.4, s[1] - 1.0], [s[0] + 1.4, s[1] + 1.4], [s[0] - 1.0, s[1] + 1.4]], closed: true }], color, alpha: style.alpha, evenodd: false });
            }
            2 => {
                paths.push(seg([s[0] - half, s[1]], [s[0] + half, s[1]]));
                paths.push(seg([s[0], s[1] - half], [s[0], s[1] + half]));
            }
            3 => {
                paths.push(seg([s[0] - half, s[1] - half], [s[0] + half, s[1] + half]));
                paths.push(seg([s[0] + half, s[1] - half], [s[0] - half, s[1] + half]));
            }
            4 => paths.push(seg(s, [s[0], s[1] - half])),
            _ => {}
        }
        if mode & 32 != 0 {
            let n = 24;
            let pts: Vec<[f64; 2]> = (0..=n).map(|i| [s[0] + half * (TAU * i as f64 / n as f64).cos(), s[1] + half * (TAU * i as f64 / n as f64).sin()]).collect();
            paths.push(Poly { pts, closed: true });
        }
        if mode & 64 != 0 {
            paths.push(Poly { pts: vec![[s[0] - half, s[1] - half], [s[0] + half, s[1] - half], [s[0] + half, s[1] + half], [s[0] - half, s[1] + half]], closed: true });
        }
        if !paths.is_empty() {
            self.ops.push(Op::Stroke { paths, color, alpha: style.alpha, width: 1.2, dash: vec![], round: false });
        }
    }

    /// Draw a text entity (also used by dimensions and leaders).
    pub fn draw_text_at(&mut self, t: &TextData, color: Rgb, alpha: f32, selected: bool, hover: bool) {
        let lay = text_layout(t);
        let m = mat::mul(&self.world, &mat::mul(&mat::translate(t.p.x, t.p.y), &mat::rotate(t.rot, Pt::ZERO)));
        let hpx = t.h * self.k();
        if hpx < 1.6 {
            // too small to read: draw a faint baseline
            let paths: Vec<Poly> = lay
                .lines
                .iter()
                .map(|l| {
                    let (a, b) = (mat::apply(&m, pt(l.x, l.y)), mat::apply(&m, pt(l.x + l.w, l.y)));
                    Poly { pts: vec![[a.x, a.y], [b.x, b.y]], closed: false }
                })
                .collect();
            if !paths.is_empty() {
                self.ops.push(Op::Stroke { paths, color, alpha: 0.55 * alpha, width: 1.0, dash: vec![], round: false });
            }
            return;
        }
        let flipped = [m[0], m[1], -m[2], -m[3], m[4], m[5]];
        let col = if hover { self.theme.hover } else { color };
        for l in &lay.lines {
            if l.s.is_empty() {
                continue;
            }
            self.ops.push(Op::Text { s: l.s.clone(), x: l.x, y: -l.y, m: flipped, em: lay.em, color: col, alpha });
        }
        if selected || hover {
            let (x0, y0, x1, y1) = lay.bx;
            let pts: Vec<[f64; 2]> = [pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)].iter().map(|p| {
                let q = mat::apply(&m, *p);
                [q.x, q.y]
            }).collect();
            self.ops.push(Op::Stroke { paths: vec![Poly { pts, closed: true }], color: if selected { self.theme.select } else { self.theme.hover }, alpha: 1.0, width: 1.0, dash: if selected { vec![4.0, 3.0] } else { vec![] }, round: false });
        }
    }

    fn draw_leader(&mut self, pts: &[Pt], s: &str, h: f64, style: &Style, selected: bool, hover: bool) {
        let path = Poly { pts: pts.iter().map(|p| self.dev(*p)).collect(), closed: false };
        self.stroke(vec![path], style, selected, hover, 0.0);
        if pts.len() > 1 {
            let (asz, scale) = {
                let d = self.doc.settings.cur_dim();
                (if d.asz != 0.0 { d.asz } else { 2.5 }, if d.scale != 0.0 { d.scale } else { 1.0 })
            };
            let dir = unit(pts[1] - pts[0]);
            if let Some(poly) = arrow_polygon(&cadforge_core::dim::Arrow { tip: pts[0], dir, size: asz * scale, style: "closed".into() }) {
                self.fill_poly(&poly, style);
            }
        }
        if !s.is_empty() {
            let t = leader_text(pts, s, h);
            self.draw_text_at(&t, style.color, style.alpha, false, hover);
        }
    }

    fn draw_dim(&mut self, e: &Entity, style: &Style, selected: bool, hover: bool) {
        let Kind::Dim(d) = &e.kind else { return };
        let geo = dim_geometry(d, &self.doc.settings);
        let mut paths: Vec<Poly> = vec![];
        for s in &geo.segs {
            match *s {
                Seg::L { a, b, .. } => paths.push(Poly { pts: vec![self.dev(a), self.dev(b)], closed: false }),
                Seg::A { c, r, a0, sw, .. } => {
                    let mut pts = vec![];
                    self.arc_into(&mut pts, c, r, r, 0.0, a0, sw);
                    paths.push(Poly { pts, closed: false });
                }
            }
        }
        for a in &geo.arrows {
            for (p, q) in arrow_lines(a) {
                paths.push(Poly { pts: vec![self.dev(p), self.dev(q)], closed: false });
            }
        }
        self.stroke(paths, style, selected, hover, 0.0);
        for a in &geo.arrows {
            if let Some(poly) = arrow_polygon(a) {
                self.fill_poly(&poly, style);
            }
        }
        self.draw_text_at(&geo.text, style.color, style.alpha, false, hover);
    }

    /// Gradient fill drawn as overlapping flat bands (painter's algorithm), so every backend renders it.
    fn gradient_fill(&mut self, paths: &[Poly], g: &GradientData, alpha: f32) {
        let white = [255u8, 255, 255];
        let (mut c1, mut c2) = if g.one_color { (g.c1, mix_rgb(g.c1, white, g.tint.clamp(0.0, 1.0))) } else { (g.c1, g.c2) };
        if self.mono {
            let gray = |c: Rgb| {
                let v = (0.3 * c[0] as f64 + 0.59 * c[1] as f64 + 0.11 * c[2] as f64) as u8;
                [v, v, v]
            };
            c1 = gray(c1);
            c2 = gray(c2);
        }
        let all: Vec<[f64; 2]> = paths.iter().flat_map(|p| p.pts.iter().copied()).collect();
        if all.len() < 3 {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for q in &all {
            x0 = x0.min(q[0]);
            y0 = y0.min(q[1]);
            x1 = x1.max(q[0]);
            y1 = y1.max(q[1]);
        }
        let ctr = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
        let kind = g.kind.to_uppercase();
        let push = |ops: &mut Vec<Op>, ps: Vec<Poly>, color: Rgb| {
            if !ps.is_empty() {
                ops.push(Op::Fill { paths: ps, color, alpha, evenodd: true });
            }
        };
        if matches!(kind.as_str(), "SPHERICAL" | "HEMISPHERICAL" | "CURVED") {
            let n = 28;
            let focus = if kind == "HEMISPHERICAL" { [x0 + (x1 - x0) * 0.35, y0 + (y1 - y0) * 0.35] } else { ctr };
            for i in 0..n {
                let f = 1.0 - i as f64 / n as f64;
                let col = mix_rgb(c1, c2, i as f64 / (n - 1) as f64);
                let ps: Vec<Poly> = paths.iter().map(|p| Poly { pts: p.pts.iter().map(|q| [focus[0] + (q[0] - focus[0]) * f, focus[1] + (q[1] - focus[1]) * f]).collect(), closed: true }).collect();
                push(&mut self.ops, ps, col);
            }
            return;
        }
        let a = g.angle;
        let dir = [a.cos(), -a.sin()];
        let proj = |q: &[f64; 2]| (q[0] - ctr[0]) * dir[0] + (q[1] - ctr[1]) * dir[1];
        let (mut tmin, mut tmax) = (f64::MAX, f64::MIN);
        for q in &all {
            tmin = tmin.min(proj(q));
            tmax = tmax.max(proj(q));
        }
        if tmax - tmin < 1e-9 {
            return;
        }
        let (cyl, inv) = (kind == "CYLINDER", kind == "INVCYLINDER");
        if cyl || inv {
            let (ca, cb) = if inv { (c2, c1) } else { (c1, c2) };
            let n = 16;
            let half = (tmax - tmin) / 2.0;
            let mid = (tmax + tmin) / 2.0;
            push(&mut self.ops, paths.to_vec(), ca);
            for k in 1..n {
                let w = half * (1.0 - k as f64 / n as f64);
                let ps: Vec<Poly> = paths.iter().filter_map(|p| clip_band(p, dir, ctr, mid - w, mid + w)).collect();
                push(&mut self.ops, ps, mix_rgb(ca, cb, k as f64 / (n - 1) as f64));
            }
            return;
        }
        let n = 40;
        for i in 0..n {
            let t = tmin + (tmax - tmin) * i as f64 / n as f64;
            let ps: Vec<Poly> = if i == 0 { paths.to_vec() } else { paths.iter().filter_map(|p| clip_band(p, dir, ctr, t, f64::MAX)).collect() };
            push(&mut self.ops, ps, mix_rgb(c1, c2, (i as f64 + 0.5) / n as f64));
        }
    }

    fn loops_paths(&self, loops: &[Vec<Vertex>]) -> Vec<Poly> {
        let mut out = vec![];
        for l in loops {
            out.extend(self.trace_segs(&pline_segs(l, true), true));
        }
        out
    }

    fn draw_hatch(&mut self, e: &Entity, h: &HatchData, style: &Style, selected: bool, hover: bool) {
        let color = match e.color {
            Some(c) if !self.mono && !matches!(c, Color::ByLayer | Color::ByBlock) => color_rgb(c, self.theme.dark, self.theme.contrast),
            _ => style.color,
        };
        if let Some(g) = &h.gradient {
            let paths = self.loops_paths(&h.loops);
            if self.prefs.fill {
                self.gradient_fill(&paths, g, style.alpha);
            } else {
                let st = Style { color, ..style.clone() };
                self.stroke(paths, &st, false, false, 0.0);
            }
            if selected || hover {
                let paths = self.loops_paths(&h.loops);
                self.stroke(paths, style, selected, hover, 0.0);
            }
            return;
        }
        if h.pattern == "SOLID" {
            let paths = self.loops_paths(&h.loops);
            if self.prefs.fill {
                self.ops.push(Op::Fill { paths, color: if hover { self.theme.hover } else { color }, alpha: style.alpha * 0.9, evenodd: true });
            } else {
                let st = Style { color, ..style.clone() };
                self.stroke(paths, &st, false, false, 0.0);
            }
            if selected {
                let paths = self.loops_paths(&h.loops);
                self.stroke(paths, style, true, false, 0.0);
            }
            return;
        }
        let Some(lines) = hatch_segments(e, self.doc) else {
            let paths = self.loops_paths(&h.loops);
            self.ops.push(Op::Fill { paths, color, alpha: 0.35, evenodd: true });
            return;
        };
        let vis = if self.view.is_plan() { Some(self.visible_world()) } else { None };
        let mut paths = vec![];
        for l in lines.iter() {
            let [x0, y0, x1, y1] = *l;
            if let Some(v) = vis {
                if x0.max(x1) < v.min_x || x0.min(x1) > v.max_x || y0.max(y1) < v.min_y || y0.min(y1) > v.max_y {
                    continue;
                }
            }
            paths.push(Poly { pts: vec![self.dev(pt(x0, y0)), self.dev(pt(x1, y1))], closed: false });
        }
        if !paths.is_empty() {
            self.ops.push(Op::Stroke { paths, color: if hover { self.theme.hover } else { color }, alpha: style.alpha, width: if hover { 2.0 } else { 1.0 }, dash: vec![], round: true });
        }
        if selected {
            let paths = self.loops_paths(&h.loops);
            let st = Style { color: self.theme.select, ..style.clone() };
            self.stroke(paths, &st, true, false, 0.0);
        }
    }

    // ------------------------------------------------------------ grid, axes, UCS icon

    pub fn draw_grid(&mut self) {
        let (prefs, theme, view) = (self.prefs, self.theme, self.view);
        if !prefs.grid || !view.can_draft() {
            return;
        }
        let ucs = self.doc.ucs;
        let c: Vec<Pt> = [(0.0, 0.0), (self.w, 0.0), (self.w, self.h), (0.0, self.h)].iter().map(|(x, y)| ucs.from_wcs(view.to_world(*x, *y))).collect();
        let b = Bounds::of_points(&c);
        let (mut gx, mut gy) = (prefs.grid_unit_x, prefs.grid_unit_y);
        if gx <= 0.0 || gy <= 0.0 {
            return;
        }
        let major = prefs.grid_major.max(1);
        while gx * view.scale < 9.0 {
            gx *= major as f64;
            gy *= major as f64;
            if gx > 1e12 {
                return;
            }
        }
        let (nx, ny) = ((b.max_x - b.min_x) / gx, (b.max_y - b.min_y) / gy);
        if nx > 800.0 || ny > 800.0 {
            return;
        }
        let line = |me: &Renderer, step: f64, step_y: f64, every: Option<i64>| -> Vec<Poly> {
            let mut paths = vec![];
            let mut i = (b.min_x / step).ceil() as i64;
            while i as f64 * step <= b.max_x {
                if !every.is_some_and(|e| i % e == 0) {
                    paths.push(Poly { pts: vec![me.dev(ucs.to_wcs(pt(i as f64 * step, b.min_y))), me.dev(ucs.to_wcs(pt(i as f64 * step, b.max_y)))], closed: false });
                }
                i += 1;
            }
            let mut j = (b.min_y / step_y).ceil() as i64;
            while j as f64 * step_y <= b.max_y {
                if !every.is_some_and(|e| j % e == 0) {
                    paths.push(Poly { pts: vec![me.dev(ucs.to_wcs(pt(b.min_x, j as f64 * step_y))), me.dev(ucs.to_wcs(pt(b.max_x, j as f64 * step_y)))], closed: false });
                }
                j += 1;
            }
            paths
        };
        let minor = line(self, gx, gy, Some(major));
        self.ops.push(Op::Stroke { paths: minor, color: theme.grid, alpha: theme.grid_alpha, width: 1.0, dash: vec![], round: false });
        let majors = line(self, gx * major as f64, gy * major as f64, None);
        self.ops.push(Op::Stroke { paths: majors, color: theme.grid_major, alpha: theme.grid_major_alpha, width: 1.0, dash: vec![], round: false });
        let ax = vec![Poly { pts: vec![self.dev(ucs.to_wcs(pt(b.min_x, 0.0))), self.dev(ucs.to_wcs(pt(b.max_x, 0.0)))], closed: false }];
        self.ops.push(Op::Stroke { paths: ax, color: [230, 90, 90], alpha: 0.55, width: 1.0, dash: vec![], round: false });
        let ay = vec![Poly { pts: vec![self.dev(ucs.to_wcs(pt(0.0, b.min_y))), self.dev(ucs.to_wcs(pt(0.0, b.max_y)))], closed: false }];
        self.ops.push(Op::Stroke { paths: ay, color: [90, 200, 120], alpha: 0.55, width: 1.0, dash: vec![], round: false });
    }

    pub fn draw_ucs_icon(&mut self) {
        if !self.prefs.ucs_icon || !self.doc.is_model() {
            return;
        }
        let (view, theme) = (self.view, self.theme);
        let ucs = self.doc.ucs;
        let mut o = [54.0, self.h - 52.0];
        if self.prefs.ucs_origin && view.is_plan() {
            let s = view.to_screen(ucs.origin);
            if s.x > 30.0 && s.x < self.w - 30.0 && s.y > 30.0 && s.y < self.h - 30.0 {
                o = [s.x, s.y];
            }
        }
        let (c, sn) = (ucs.angle.cos(), ucs.angle.sin());
        let axis = |me: &mut Renderer, v: (f64, f64, f64), color: Rgb, label: &str| {
            // the axis direction in world coordinates (the UCS rotates the X and Y axes about Z)
            let v = (v.0 * c - v.1 * sn, v.0 * sn + v.1 * c, v.2);
            let p = if view.is_plan() {
                [v.0 * 38.0, -v.1 * 38.0]
            } else {
                use cadforge_core::solid::V3;
                let (a, z) = (view.project(V3::new(v.0, v.1, v.2)), view.project(V3::new(0.0, 0.0, 0.0)));
                [(a.x - z.x) / view.scale * 38.0, (a.y - z.y) / view.scale * 38.0]
            };
            let tip = [o[0] + p[0], o[1] + p[1]];
            me.ops.push(Op::Stroke { paths: vec![Poly { pts: vec![o, tip], closed: false }], color, alpha: 1.0, width: 1.6, dash: vec![], round: false });
            let a = p[1].atan2(p[0]);
            let head = vec![tip, [tip[0] - 7.0 * (a - 0.38).cos(), tip[1] - 7.0 * (a - 0.38).sin()], [tip[0] - 7.0 * (a + 0.38).cos(), tip[1] - 7.0 * (a + 0.38).sin()]];
            me.ops.push(Op::Fill { paths: vec![Poly { pts: head, closed: true }], color, alpha: 1.0, evenodd: false });
            let lx = tip[0] + if p[0] >= 0.0 { 6.0 } else { -14.0 } + if p[0].abs() < 4.0 { -3.0 } else { 0.0 };
            let ly = tip[1] + if p[1] <= 0.0 { -4.0 } else { 12.0 } + if p[1].abs() < 4.0 { 4.0 } else { 0.0 };
            me.ops.push(Op::Text { s: label.into(), x: lx, y: ly, m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], em: 11.0, color, alpha: 1.0 });
        };
        if view.is_plan() {
            axis(self, (1.0, 0.0, 0.0), [0xff, 0x6b, 0x5e], "X");
            axis(self, (0.0, 1.0, 0.0), [0x6b, 0xdc, 0x86], "Y");
            // the box marks the world coordinate system
            if ucs.is_world() {
                let sq = Poly { pts: vec![[o[0] - 5.0, o[1] - 5.0], [o[0] + 5.0, o[1] - 5.0], [o[0] + 5.0, o[1] + 5.0], [o[0] - 5.0, o[1] + 5.0]], closed: true };
                self.ops.push(Op::Stroke { paths: vec![sq], color: if theme.dark { [0xcf, 0xd6, 0xdd] } else { [0x44, 0x44, 0x55] }, alpha: 1.0, width: 1.0, dash: vec![], round: false });
            }
        } else {
            axis(self, (1.0, 0.0, 0.0), [0xff, 0x6b, 0x5e], "X");
            axis(self, (0.0, 1.0, 0.0), [0x6b, 0xdc, 0x86], "Y");
            axis(self, (0.0, 0.0, 1.0), [0x5f, 0xb4, 0xff], "Z");
        }
    }

    /// Everything in the model/paper space of the drawing, culled to the view.
    /// Paper space: the sheet, the layout's own objects and its viewports showing model space.
    pub fn draw_layout(&mut self, name: &str, hl: &Highlight, active_vp: Option<u64>) {
        let doc = self.doc;
        let Some(layout) = doc.layout(name) else { return };
        let (pw, ph) = (layout.paper.w, layout.paper.h);
        let sheet: Vec<[f64; 2]> = [pt(0.0, 0.0), pt(pw, 0.0), pt(pw, ph), pt(0.0, ph)].iter().map(|p| self.dev(*p)).collect();
        let shadow: Vec<[f64; 2]> = sheet.iter().map(|q| [q[0] + 4.0, q[1] + 4.0]).collect();
        self.ops.push(Op::Fill { paths: vec![Poly { pts: shadow, closed: true }], color: [0, 0, 0], alpha: 0.35, evenodd: false });
        self.ops.push(Op::Fill { paths: vec![Poly { pts: sheet.clone(), closed: true }], color: [255, 255, 255], alpha: 1.0, evenodd: false });
        self.ops.push(Op::Stroke { paths: vec![Poly { pts: sheet, closed: true }], color: [90, 96, 104], alpha: 1.0, width: 1.0, dash: vec![], round: false });
        // layout objects are drawn on white: use the light colour mapping, keep the highlight colours
        let saved = self.theme;
        self.theme = Theme { dark: false, contrast: true, canvas: [255, 255, 255], ..saved };
        for e in &layout.entities {
            if !doc.is_visible(e) {
                continue;
            }
            let selected = hl.selected.contains(&e.id);
            match &e.kind {
                Kind::Viewport(v) => self.draw_viewport(e, v, selected, active_vp == Some(e.id)),
                _ => self.draw_entity(e, DrawOpts { selected, hover: hl.hover == Some(e.id), ..DrawOpts::default() }),
            }
        }
        self.theme = saved;
    }

    /// A layout as plotted: layout objects and viewport contents on white paper, no sheet edge and no viewport borders.
    pub fn draw_layout_plot(&mut self, name: &str) {
        let doc = self.doc;
        let Some(layout) = doc.layout(name) else { return };
        for e in &layout.entities {
            if !doc.is_visible(e) || !doc.layer_of(e).plot {
                continue;
            }
            match &e.kind {
                Kind::Viewport(v) => self.draw_viewport_content(v),
                _ => self.draw_entity(e, DrawOpts::default()),
            }
        }
    }

    fn draw_viewport(&mut self, e: &Entity, v: &ViewportData, selected: bool, active: bool) {
        let outline: Vec<[f64; 2]> = v.outline().iter().map(|p| self.dev(*p)).collect();
        self.draw_viewport_content(v);
        // viewport border: shown on screen, thicker while active or selected
        if !v.off || selected {
            let st = Style { raw_color: Color::Aci(7), color: if active || selected { self.theme.select } else { [120, 128, 140] }, ltype: "Continuous".into(), lw: LW_DEFAULT, lts: 1.0, alpha: 1.0 };
            self.stroke(vec![Poly { pts: outline, closed: true }], &st, false, false, if active { 2.5 } else { 1.0 });
        }
        let _ = e;
    }

    fn draw_viewport_content(&mut self, v: &ViewportData) {
        let doc = self.doc;
        let outline: Vec<[f64; 2]> = v.outline().iter().map(|p| self.dev(*p)).collect();
        if !v.off && outline.len() >= 3 {
            let (vc, s) = v.model_view(doc);
            let from_model = mat::mul(&mat::translate(v.c.x, v.c.y), &mat::mul(&mat::scale(s, s, Pt::ZERO), &mat::translate(-vc.x, -vc.y)));
            let mut sub = Renderer::new(doc, self.view, self.w, self.h, self.theme, self.prefs);
            sub.world = mat::mul(&self.world, &from_model);
            sub.mono = self.mono;
            sub.lw_mode = self.lw_mode;
            sub.tol = self.tol;
            sub.ltscale = self.ltscale;
            let inv = mat::invert(&sub.world);
            let vis = Bounds::of_points(&outline.iter().map(|q| mat::apply(&inv, pt(q[0], q[1]))).collect::<Vec<_>>()).grow(1.0);
            let plotting = matches!(self.lw_mode, LwMode::Plot(_));
            for me in &doc.model.entities {
                if !doc.is_visible(me) || (plotting && !doc.layer_of(me).plot) {
                    continue;
                }
                if let Some(b) = bounds_of(me, doc) {
                    if b.max_x < vis.min_x || b.min_x > vis.max_x || b.max_y < vis.min_y || b.min_y > vis.max_y {
                        continue;
                    }
                }
                sub.draw_entity(me, DrawOpts::default());
            }
            self.ops.extend(clip_ops_to_convex(sub.ops, &outline));
        }
    }

    pub fn draw_entities(&mut self, hl: &Highlight) {
        let doc = self.doc;
        let plan = self.view.is_plan();
        for e in doc.entities() {
            if !doc.is_visible(e) {
                continue;
            }
            if plan {
                if let Some(b) = bounds_of(e, doc) {
                    let v = self.visible_world();
                    if b.max_x < v.min_x || b.min_x > v.max_x || b.max_y < v.min_y || b.min_y > v.max_y {
                        continue;
                    }
                }
            }
            let selected = hl.selected.contains(&e.id);
            self.draw_entity(e, DrawOpts { selected, hover: hl.hover == Some(e.id), ..DrawOpts::default() });
        }
    }
}

/// One frame of the drawing area: grid, solids (via `solids`), entities and the UCS icon. The canvas fill is left to
/// the backend (`theme.canvas`).
pub fn render_scene(doc: &Drawing, view: &View, w: f64, h: f64, theme: Theme, prefs: Prefs, hl: &Highlight, solids: Option<&dyn Fn(&mut Renderer)>) -> Vec<Op> {
    let mut rc = Renderer::new(doc, view, w, h, theme, prefs);
    if !doc.is_model() {
        rc.draw_layout(&doc.space, hl, None);
        return rc.ops;
    }
    rc.draw_grid();
    if let Some(f) = solids {
        f(&mut rc);
    }
    rc.draw_entities(hl);
    rc.draw_ucs_icon();
    rc.ops
}

fn mix_rgb(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let m = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t.clamp(0.0, 1.0)).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Clip a closed polygon to the band `lo <= (p - o)·dir <= hi` (Sutherland–Hodgman against two half-planes).
fn clip_band(p: &Poly, dir: [f64; 2], o: [f64; 2], lo: f64, hi: f64) -> Option<Poly> {
    let f = |q: &[f64; 2]| (q[0] - o[0]) * dir[0] + (q[1] - o[1]) * dir[1];
    let mut pts = p.pts.clone();
    for (bound, keep_ge) in [(lo, true), (hi, false)] {
        if pts.len() < 3 || (!keep_ge && bound == f64::MAX) {
            continue;
        }
        let inside = |q: &[f64; 2]| if keep_ge { f(q) >= bound } else { f(q) <= bound };
        let mut out: Vec<[f64; 2]> = vec![];
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let (ia, ib) = (inside(&a), inside(&b));
            if ia {
                out.push(a);
            }
            if ia != ib {
                let (fa, fb) = (f(&a), f(&b));
                let t = (bound - fa) / (fb - fa);
                out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            }
        }
        pts = out;
    }
    if pts.len() >= 3 {
        Some(Poly { pts, closed: true })
    } else {
        None
    }
}

/// One frame of a layout (paper space) seen through `view`, with an optionally active viewport.
pub fn render_layout(doc: &Drawing, name: &str, view: &View, w: f64, h: f64, theme: Theme, prefs: Prefs, hl: &Highlight, active_vp: Option<u64>) -> Vec<Op> {
    let mut rc = Renderer::new(doc, view, w, h, theme, prefs);
    rc.draw_layout(name, hl, active_vp);
    rc.ops
}

fn convex_oriented(poly: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let area: f64 = (0..poly.len()).map(|i| poly[i][0] * poly[(i + 1) % poly.len()][1] - poly[(i + 1) % poly.len()][0] * poly[i][1]).sum();
    if area < 0.0 {
        poly.iter().rev().copied().collect()
    } else {
        poly.to_vec()
    }
}

/// Clip a segment to a convex polygon given counter-clockwise (Cyrus–Beck).
fn clip_segment(poly: &[[f64; 2]], a: [f64; 2], b: [f64; 2]) -> Option<([f64; 2], [f64; 2])> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let d = [b[0] - a[0], b[1] - a[1]];
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        let n = [-(q[1] - p[1]), q[0] - p[0]]; // inward normal for a CCW polygon in a y-up frame
        let num = n[0] * (a[0] - p[0]) + n[1] * (a[1] - p[1]);
        let den = n[0] * d[0] + n[1] * d[1];
        if den.abs() < 1e-12 {
            if num < 0.0 {
                return None;
            }
        } else {
            let t = -num / den;
            if den > 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(([a[0] + d[0] * t0, a[1] + d[1] * t0], [a[0] + d[0] * t1, a[1] + d[1] * t1]))
}

fn inside_convex(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    (0..poly.len()).all(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) >= -1e-9
    })
}

fn clip_polygon_convex(subject: &[[f64; 2]], clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = subject.to_vec();
    for i in 0..clip.len() {
        if out.len() < 3 {
            return vec![];
        }
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        let side = |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let mut next: Vec<[f64; 2]> = vec![];
        for j in 0..out.len() {
            let (p, q) = (out[j], out[(j + 1) % out.len()]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                next.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                next.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
            }
        }
        out = next;
    }
    out
}

/// Keep only the parts of `ops` inside a convex polygon (device space). Text is kept when its origin is inside.
pub fn clip_ops_to_convex(ops: Vec<Op>, polygon: &[[f64; 2]]) -> Vec<Op> {
    // device space is y-down, so a visually counter-clockwise polygon has negative area there: normalise to the
    // orientation the half-plane tests above expect (positive shoelace area in the same coordinates)
    let poly = convex_oriented(polygon);
    let mut out = vec![];
    for op in ops {
        match op {
            Op::Stroke { paths, color, alpha, width, dash, round } => {
                let mut clipped: Vec<Poly> = vec![];
                for p in &paths {
                    let n = p.pts.len();
                    let segs = if p.closed { n } else { n.saturating_sub(1) };
                    let mut cur: Vec<[f64; 2]> = vec![];
                    for i in 0..segs {
                        let (a, b) = (p.pts[i], p.pts[(i + 1) % n]);
                        match clip_segment(&poly, a, b) {
                            Some((ca, cb)) => {
                                if cur.last().map_or(true, |l| (l[0] - ca[0]).abs() > 1e-9 || (l[1] - ca[1]).abs() > 1e-9) {
                                    if cur.len() >= 2 {
                                        clipped.push(Poly { pts: std::mem::take(&mut cur), closed: false });
                                    }
                                    cur.clear();
                                    cur.push(ca);
                                }
                                cur.push(cb);
                            }
                            None => {
                                if cur.len() >= 2 {
                                    clipped.push(Poly { pts: std::mem::take(&mut cur), closed: false });
                                }
                                cur.clear();
                            }
                        }
                    }
                    if cur.len() >= 2 {
                        clipped.push(Poly { pts: cur, closed: false });
                    }
                }
                if !clipped.is_empty() {
                    out.push(Op::Stroke { paths: clipped, color, alpha, width, dash, round });
                }
            }
            Op::Fill { paths, color, alpha, evenodd } => {
                let clipped: Vec<Poly> = paths.iter().filter_map(|p| {
                    let pts = clip_polygon_convex(&p.pts, &poly);
                    (pts.len() >= 3).then_some(Poly { pts, closed: true })
                }).collect();
                if !clipped.is_empty() {
                    out.push(Op::Fill { paths: clipped, color, alpha, evenodd });
                }
            }
            t @ Op::Text { .. } => {
                if t.text_origin().map_or(false, |o| inside_convex(&poly, o)) {
                    out.push(t);
                }
            }
        }
    }
    out
}
