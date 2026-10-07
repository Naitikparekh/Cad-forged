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
            Kind::Text(t) => self.draw_text_at(t, style.color, style.alpha, o.selected, o.hover),
            Kind::Hatch(h) => self.draw_hatch(e, h, style, o.selected, o.hover),
            Kind::Dim(_) => self.draw_dim(e, style, o.selected, o.hover),
            Kind::Leader { pts, s, h } => self.draw_leader(pts, s, *h, style, o.selected, o.hover),
            Kind::Insert(ins) => self.draw_insert(e, ins, style, o),
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
        let c: Vec<Pt> = [(0.0, 0.0), (self.w, 0.0), (self.w, self.h), (0.0, self.h)].iter().map(|(x, y)| view.to_world(*x, *y)).collect();
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
                    paths.push(Poly { pts: vec![me.dev(pt(i as f64 * step, b.min_y)), me.dev(pt(i as f64 * step, b.max_y))], closed: false });
                }
                i += 1;
            }
            let mut j = (b.min_y / step_y).ceil() as i64;
            while j as f64 * step_y <= b.max_y {
                if !every.is_some_and(|e| j % e == 0) {
                    paths.push(Poly { pts: vec![me.dev(pt(b.min_x, j as f64 * step_y)), me.dev(pt(b.max_x, j as f64 * step_y))], closed: false });
                }
                j += 1;
            }
            paths
        };
        let minor = line(self, gx, gy, Some(major));
        self.ops.push(Op::Stroke { paths: minor, color: theme.grid, alpha: theme.grid_alpha, width: 1.0, dash: vec![], round: false });
        let majors = line(self, gx * major as f64, gy * major as f64, None);
        self.ops.push(Op::Stroke { paths: majors, color: theme.grid_major, alpha: theme.grid_major_alpha, width: 1.0, dash: vec![], round: false });
        let ax = vec![Poly { pts: vec![self.dev(pt(b.min_x, 0.0)), self.dev(pt(b.max_x, 0.0))], closed: false }];
        self.ops.push(Op::Stroke { paths: ax, color: [230, 90, 90], alpha: 0.55, width: 1.0, dash: vec![], round: false });
        let ay = vec![Poly { pts: vec![self.dev(pt(0.0, b.min_y)), self.dev(pt(0.0, b.max_y))], closed: false }];
        self.ops.push(Op::Stroke { paths: ay, color: [90, 200, 120], alpha: 0.55, width: 1.0, dash: vec![], round: false });
    }

    pub fn draw_ucs_icon(&mut self) {
        if !self.prefs.ucs_icon {
            return;
        }
        let (view, theme) = (self.view, self.theme);
        let o = [54.0, self.h - 52.0];
        let axis = |me: &mut Renderer, v: (f64, f64, f64), color: Rgb, label: &str| {
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
            let sq = Poly { pts: vec![[o[0] - 5.0, o[1] - 5.0], [o[0] + 5.0, o[1] - 5.0], [o[0] + 5.0, o[1] + 5.0], [o[0] - 5.0, o[1] + 5.0]], closed: true };
            self.ops.push(Op::Stroke { paths: vec![sq], color: if theme.dark { [0xcf, 0xd6, 0xdd] } else { [0x44, 0x44, 0x55] }, alpha: 1.0, width: 1.0, dash: vec![], round: false });
        } else {
            axis(self, (1.0, 0.0, 0.0), [0xff, 0x6b, 0x5e], "X");
            axis(self, (0.0, 1.0, 0.0), [0x6b, 0xdc, 0x86], "Y");
            axis(self, (0.0, 0.0, 1.0), [0x5f, 0xb4, 0xff], "Z");
        }
    }

    /// Everything in the model/paper space of the drawing, culled to the view.
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
    rc.draw_grid();
    if let Some(f) = solids {
        f(&mut rc);
    }
    rc.draw_entities(hl);
    rc.draw_ucs_icon();
    rc.ops
}
