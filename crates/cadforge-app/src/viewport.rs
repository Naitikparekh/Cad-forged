//! The drawing viewport: scene painting from renderer ops, the interactive overlay (rubber band, snap markers,
//! tracking, selection window, grips, crosshair), pointer / wheel input, realtime pan-zoom-orbit, viewport controls,
//! navigation bar and dynamic-input tooltip.
use crate::app::CadForgeApp;
use crate::gfx::{col, paint_ops};
use crate::icons::Tint;
use cadforge_core::entities::{grips_of, GripKind};
use cadforge_core::geom::{dist, pt, Pt};
use cadforge_engine::app::App;
use cadforge_engine::runtime::{DialogReply, DialogReq, Mods, PreviewItem, PromptKind};
use cadforge_engine::snap::SnapKind;
use cadforge_render::{draw_solids, render_scene, DrawOpts, Highlight, Op, Prefs, Renderer, SolidCache, Theme};
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const DRAG_PX: f32 = 5.0;
const YELLOW: Color32 = Color32::from_rgb(0xff, 0xe1, 0x4d);

pub struct Down {
    pub abs: Pos2,
    pub world: Pt,
    pub button: u8,
    pub shift: bool,
    pub ctrl: bool,
    pub moved: bool,
}

pub struct Panning {
    pub last: Pos2,
    pub orbit: bool,
    pub mode: Option<&'static str>,
}

#[derive(Default)]
pub struct CubeState {
    pub drag: Option<(Pos2, f64, f64, bool)>,
    pub anim: Option<(f64, f64, f64, f64, f64)>,
    pub hover: Option<CubeHit>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CubeHit {
    Home,
    Arrow(usize),
    Cell { face: usize, i: i32, j: i32 },
}

#[derive(Default)]
pub struct ViewportState {
    pub rect: Option<Rect>,
    pub mouse: Option<Pos2>,
    pub down: Option<Down>,
    pub drag_rect: Option<(Pt, Pt)>,
    pub panning: Option<Panning>,
    pub realtime: Option<&'static str>,
    pub hover: Option<u64>,
    pub cube: CubeState,
    cache_key: u64,
    cache_ops: Vec<Op>,
    solid_cache: SolidCache,
    pub blocked: Vec<Rect>,
}

pub fn render_prefs(a: &App) -> Prefs {
    let sv = &a.sysvars;
    Prefs {
        lw_display: sv.int("LWDISPLAY") != 0,
        lock_fade: sv.real("LAYLOCKFADECTL"),
        transparency: sv.int("TRANSPARENCYDISPLAY") != 0,
        fill: sv.int("FILLMODE") != 0,
        grid: sv.grid_on(),
        grid_unit_x: sv.real("GRIDUNIT_X"),
        grid_unit_y: sv.real("GRIDUNIT_Y"),
        grid_major: sv.int("GRIDMAJOR"),
        ucs_icon: sv.int("UCSICON") != 0,
    }
}

pub fn render_theme(dark: bool) -> Theme {
    if dark {
        Theme::DARK
    } else {
        Theme::LIGHT
    }
}

impl CadForgeApp {
    pub fn end_realtime(&mut self) {
        if self.vp.realtime.take().is_some() {
            self.vp.panning = None;
            self.h.answer_dialog(DialogReply::Done);
        }
    }

    pub fn viewport_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        self.vp.rect = Some(rect);
        let t = self.t;
        ui.painter().rect_filled(rect, 0.0, t.canvas);
        if self.h.borrow().editor().is_none() {
            return;
        }
        // keep the camera size in sync; first-time zoom extents
        {
            let mut a = self.h.borrow_mut();
            let ed = a.editor_mut().expect("editor");
            ed.view.w = rect.width() as f64;
            ed.view.h = rect.height() as f64;
            if !ed.zoomed && rect.width() > 4.0 {
                ed.zoom_extents(0.1);
                if ed.extents().is_none() {
                    ed.view.target = cadforge_core::solid::V3::new(0.0, 0.0, 0.0);
                    ed.view.scale = 4.0;
                }
                ed.zoomed = true;
                ed.view.prev.clear();
            }
        }
        // realtime dialog requested by the running command?
        let rt_req = match self.h.borrow().pending_dialog() {
            Some(DialogReq::Realtime(m)) => Some(m),
            _ => None,
        };
        if rt_req.is_some() && self.vp.realtime.is_none() {
            self.vp.realtime = rt_req;
            self.toast(&format!("Realtime {}: drag to {}. Press Esc, Enter or right-click to exit.", rt_req.unwrap(), rt_req.unwrap()));
        } else if rt_req.is_none() && self.vp.realtime.is_some() {
            self.vp.realtime = None;
            self.vp.panning = None;
        }

        // overlay widgets first (their rectangles block drawing input)
        self.vp.blocked.clear();
        self.viewport_controls(ui, ctx, rect);

        self.viewport_input(ctx, &resp, rect);
        self.viewport_paint(ui, ctx, rect, &resp);
    }

    // ------------------------------------------------------------ input

    fn viewport_input(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect) {
        let events = ctx.input(|i| i.events.clone());
        if events.iter().any(|e| matches!(e, egui::Event::PointerButton { .. } | egui::Event::PointerMoved(_) | egui::Event::MouseWheel { .. })) {
            self.input_dirty = true;
        }
        let hover = resp.hovered();
        let blocked = self.vp.blocked.clone();
        let local = |p: Pos2| p - rect.min;
        let editing_modal = self.popup.is_some();
        for ev in events {
            match ev {
                egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                    let inside = rect.contains(pos) && !blocked.iter().any(|b| b.contains(pos));
                    if pressed {
                        if !inside || !hover || editing_modal {
                            continue;
                        }
                        self.on_down(pos, local(pos), button, modifiers);
                    } else {
                        self.on_up(pos, local(pos), button, modifiers);
                    }
                }
                egui::Event::PointerMoved(pos) => self.on_move(pos, local(pos), rect, ctx),
                egui::Event::MouseWheel { delta, modifiers, unit, .. } => {
                    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { continue };
                    if hover && rect.contains(p) && !blocked.iter().any(|b| b.contains(p)) {
                        self.on_wheel(local(p).to_pos2(), delta, modifiers, unit);
                    }
                }
                _ => {}
            }
        }
        if hover {
            if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
                if !blocked.iter().any(|b| b.contains(p)) {
                    self.vp.mouse = Some(local(p).to_pos2());
                    ctx.set_cursor_icon(egui::CursorIcon::None);
                } else if self.vp.down.is_none() && self.vp.panning.is_none() {
                    self.vp.mouse = None;
                }
            }
        } else if self.vp.down.is_none() && self.vp.panning.is_none() {
            self.vp.mouse = None;
            self.vp.hover = None;
        }
        let zoom = ctx.input(|i| i.zoom_delta());
        if hover && (zoom - 1.0).abs() > 1e-4 {
            if let Some(m) = self.vp.mouse {
                self.with_view(|v| v.zoom_at(m.x as f64, m.y as f64, zoom as f64));
            }
        }
        if resp.double_clicked() && self.vp.realtime.is_none() {
            if let Some(m) = self.vp.mouse {
                let w = self.to_world(m);
                self.h.double_click(w);
            }
        }
    }

    pub fn with_view<R>(&mut self, f: impl FnOnce(&mut cadforge_core::view::View) -> R) -> Option<R> {
        let mut a = self.h.borrow_mut();
        a.editor_mut().map(|e| {
            let r = f(&mut e.view);
            e.sync_mspace_view();
            r
        })
    }

    pub fn to_world(&self, m: Pos2) -> Pt {
        self.h.borrow().editor().map_or(pt(0.0, 0.0), |e| e.view.to_world(m.x as f64, m.y as f64))
    }

    fn on_down(&mut self, abs: Pos2, local: egui::Vec2, button: egui::PointerButton, m: egui::Modifiers) {
        let world = self.to_world(local.to_pos2());
        match button {
            egui::PointerButton::Middle => {
                self.vp.panning = Some(Panning { last: abs, orbit: m.shift, mode: None });
            }
            egui::PointerButton::Primary => {
                if let Some(mode) = self.vp.realtime {
                    self.vp.panning = Some(Panning { last: abs, orbit: false, mode: Some(mode) });
                    return;
                }
                self.vp.down = Some(Down { abs, world, button: 0, shift: m.shift, ctrl: m.command || m.ctrl, moved: false });
            }
            egui::PointerButton::Secondary => {
                self.vp.down = Some(Down { abs, world, button: 2, shift: m.shift, ctrl: m.command || m.ctrl, moved: false });
            }
            _ => {}
        }
    }

    fn on_move(&mut self, abs: Pos2, local: egui::Vec2, _rect: Rect, _ctx: &egui::Context) {
        let lp = local.to_pos2();
        if let Some(p) = &mut self.vp.panning {
            let d = abs - p.last;
            p.last = abs;
            let (orbit, mode) = (p.orbit, p.mode);
            let (w, h) = self.vp.rect.map_or((100.0, 100.0), |r| (r.width() as f64, r.height() as f64));
            self.with_view(|v| {
                if orbit || mode == Some("orbit") {
                    v.orbit(-(d.x as f64) * 0.008, d.y as f64 * 0.008);
                } else if mode == Some("zoom") {
                    v.zoom_at(w / 2.0, h / 2.0, (-(d.y as f64) * 0.01).exp());
                } else {
                    v.pan(d.x as f64, d.y as f64);
                }
            });
            return;
        }
        self.vp.mouse = Some(lp);
        let (can_draft, scale) = {
            let a = self.h.borrow();
            match a.editor() {
                Some(e) => (e.view.can_draft(), e.view.scale),
                None => return,
            }
        };
        if !can_draft && self.vp.realtime.is_none() {
            return;
        }
        let raw = self.to_world(lp);
        if let Some(d) = &mut self.vp.down {
            if d.button == 0 && self.vp.realtime.is_none() {
                let dist_px = (abs - d.abs).length();
                let (busy, kind) = {
                    let a = self.h.borrow();
                    (a.busy(), a.pending().map(|p| p.kind))
                };
                let can_drag = !busy || kind == Some(PromptKind::Select);
                if dist_px > DRAG_PX && can_drag {
                    d.moved = true;
                    self.vp.drag_rect = Some((d.world, raw));
                }
            }
        }
        self.h.mouse_move(raw);
        // hover highlight when picking objects
        let (busy, kind, aperture) = {
            let a = self.h.borrow();
            (a.busy(), a.pending().map(|p| p.kind), (a.sysvars.real("PICKBOX") * 2.0 + 2.0) / scale)
        };
        if (!busy || matches!(kind, Some(PromptKind::Select) | Some(PromptKind::Entity))) && self.vp.drag_rect.is_none() {
            let a = self.h.borrow();
            self.vp.hover = a.doc().and_then(|d| cadforge_engine::select::pick_at(d, raw, aperture, None, None));
        } else {
            self.vp.hover = None;
        }
    }

    fn on_up(&mut self, _abs: Pos2, local: egui::Vec2, button: egui::PointerButton, m: egui::Modifiers) {
        if self.vp.panning.is_some() {
            self.vp.panning = None;
            return;
        }
        let Some(d) = self.vp.down.take() else { return };
        let _ = button;
        if d.button == 2 {
            if (_abs - d.abs).length() < 4.0 {
                if self.vp.realtime.is_some() {
                    self.end_realtime();
                } else {
                    self.context_menu_pending = Some((d.abs, d.shift, d.ctrl));
                }
            }
            return;
        }
        let can_draft = self.h.borrow().editor().is_some_and(|e| e.view.can_draft());
        if !can_draft {
            self.toast("The drawing plane is edge-on in this view. Switch to Top or an isometric view to draw.");
            return;
        }
        let lp = local.to_pos2();
        if let Some((a, b)) = self.vp.drag_rect.take() {
            self.h.drag(a, b, Mods { shift: d.shift, ctrl: d.ctrl, alt: false, screen: None });
        } else if self.vp.realtime.is_none() {
            let w = self.to_world(lp);
            self.h.click(w, Mods { shift: m.shift || d.shift, ctrl: d.ctrl, alt: false, screen: Some(pt(lp.x as f64, lp.y as f64)) });
        }
    }

    fn on_wheel(&mut self, local: Pos2, delta: egui::Vec2, m: egui::Modifiers, unit: egui::MouseWheelUnit) {
        let k = match unit {
            egui::MouseWheelUnit::Line => 33.0,
            egui::MouseWheelUnit::Page => 200.0,
            egui::MouseWheelUnit::Point => 1.0,
        };
        let dy = -(delta.y as f64) * k;
        let dx = -(delta.x as f64) * k;
        let (sx, sy) = (local.x as f64, local.y as f64);
        if m.shift {
            self.with_view(|v| v.pan(0.0, -dy));
        } else if m.ctrl || m.command {
            self.with_view(|v| v.pan(-dy, 0.0));
        } else if dy.abs() > 0.0 {
            let f = 1.0015f64.powf(-dy * 1.6).clamp(0.5, 2.0);
            self.with_view(|v| v.zoom_at(sx, sy, f));
        } else if dx.abs() > 0.0 {
            self.with_view(|v| v.pan(-dx, 0.0));
        }
        let w = self.to_world(local);
        self.h.mouse_move(w);
    }

    // ------------------------------------------------------------ painting

    fn viewport_paint(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: Rect, _resp: &egui::Response) {
        let t = self.t;
        let painter = ui.painter_at(rect);
        let theme = render_theme(self.ui.dark);
        // ---- scene (cached)
        {
            let a = self.h.borrow();
            let Some(ed) = a.editor() else { return };
            let prefs = render_prefs(&a);
            let mut hs = DefaultHasher::new();
            ed.id.hash(&mut hs);
            a.rev.hash(&mut hs);
            for v in [ed.view.target.x, ed.view.target.y, ed.view.target.z, ed.view.scale, ed.view.yaw, ed.view.pitch, ed.view.w, ed.view.h] {
                v.to_bits().hash(&mut hs);
            }
            self.ui.dark.hash(&mut hs);
            ed.visual_style.hash(&mut hs);
            for id in ed.sel.iter().chain(ed.temp_sel.iter()) {
                id.hash(&mut hs);
            }
            (prefs.lw_display, prefs.fill, prefs.grid, prefs.ucs_icon, prefs.transparency).hash(&mut hs);
            prefs.grid_unit_x.to_bits().hash(&mut hs);
            prefs.grid_unit_y.to_bits().hash(&mut hs);
            prefs.grid_major.hash(&mut hs);
            ed.doc.rev.hash(&mut hs);
            ed.doc.space.hash(&mut hs);
            if let Some(m) = &ed.mspace {
                (m.vp, m.paper_view.scale.to_bits(), m.paper_view.cx.to_bits(), m.paper_view.cy.to_bits()).hash(&mut hs);
            }
            let key = hs.finish();
            if key != self.vp.cache_key || self.vp.cache_ops.is_empty() {
                let hl = Highlight { selected: ed.sel.iter().chain(ed.temp_sel.iter()).copied().collect(), hover: None };
                let style = ed.visual_style.clone();
                let sc = &mut self.vp.solid_cache;
                let draw = |rc: &mut Renderer| draw_solids(rc, sc, &hl, &style);
                // the closure needs a mutable cache; render_scene takes `&dyn Fn`, so route through a RefCell
                let cell = std::cell::RefCell::new(draw);
                let f = |rc: &mut Renderer| (cell.borrow_mut())(rc);
                self.vp.cache_ops = match &ed.mspace {
                    Some(m) => cadforge_render::render_layout(&ed.doc, &m.layout, &ed.paper_view(), ed.view.w, ed.view.h, theme, prefs, &hl, Some(m.vp)),
                    None => render_scene(&ed.doc, &ed.view, ed.view.w, ed.view.h, theme, prefs, &hl, Some(&f)),
                };
                self.vp.cache_key = key;
            }
        }
        paint_ops(&painter, rect.min, &self.vp.cache_ops, rect);
        self.paint_overlay(&painter, ctx, rect, theme);
        let _ = t;
    }

    fn paint_overlay(&mut self, painter: &egui::Painter, _ctx: &egui::Context, rect: Rect, theme: Theme) {
        let t = self.t;
        let a = self.h.borrow();
        let Some(ed) = a.editor() else { return };
        let view = &ed.view;
        let doc = &ed.doc;
        let prefs = render_prefs(&a);
        let origin = rect.min;
        let to_s = |p: Pt| -> Pos2 {
            let s = view.to_screen(p);
            Pos2::new(origin.x + s.x as f32, origin.y + s.y as f32)
        };
        let rt = &a.rt;
        let pending = a.pending();
        let kind = if !a.busy() {
            "idle"
        } else {
            match pending.map(|p| p.kind) {
                Some(PromptKind::Select) => "select",
                Some(PromptKind::Entity) => "entity",
                Some(_) => "point",
                None => "none",
            }
        };

        // hover highlight + rubber-band preview via the scene renderer
        let mut ops: Vec<Op> = vec![];
        {
            let mut rc = Renderer::new(doc, view, view.w, view.h, theme, prefs);
            if let Some(id) = self.vp.hover {
                if let Some(e) = ed.entity_by_id(id) {
                    rc.draw_entity(e, DrawOpts { hover: true, ..DrawOpts::default() });
                }
            }
            for p in &rt.preview {
                match p {
                    PreviewItem::Entity(e) => {
                        let mut e = e.clone();
                        if e.layer.is_empty() {
                            e.layer = doc.cur.layer.clone();
                        }
                        if e.color.is_none() {
                            e.color = doc.cur.color;
                        }
                        rc.draw_entity(&e, DrawOpts::default());
                    }
                    PreviewItem::Rect { a: pa, b: pb } => {
                        let corners = [Pt { x: pa.x, y: pa.y }, Pt { x: pb.x, y: pa.y }, Pt { x: pb.x, y: pb.y }, Pt { x: pa.x, y: pb.y }];
                        let pts: Vec<[f64; 2]> = corners.iter().map(|c| {
                            let s = view.to_screen(*c);
                            [s.x, s.y]
                        }).collect();
                        let c = if theme.dark { [0xe9, 0xed, 0xf1] } else { [0x22, 0x22, 0x22] };
                        rc.ops.push(Op::Stroke { paths: vec![cadforge_render::Poly { pts, closed: true }], color: c, alpha: 1.0, width: 1.0, dash: vec![], round: false });
                    }
                }
            }
            ops = rc.ops;
        }
        paint_ops(painter, origin, &ops, rect);

        // acquired object-snap tracking points
        for acq in &rt.snapper.acquired {
            let s = to_s(*acq);
            let c = Color32::from_rgba_unmultiplied(255, 225, 77, 215);
            painter.line_segment([s + vec2(-4.0, 0.0), s + vec2(4.0, 0.0)], Stroke::new(1.4, c));
            painter.line_segment([s + vec2(0.0, -4.0), s + vec2(0.0, 4.0)], Stroke::new(1.4, c));
        }
        // tracking vectors
        let track = rt.cursor.track.as_ref();
        if let Some(tr) = track {
            let c = if theme.dark { Color32::from_rgba_unmultiplied(210, 225, 240, 178) } else { Color32::from_rgba_unmultiplied(60, 70, 90, 178) };
            for l in &tr.lines {
                let (s, e) = (to_s(l.a), to_s(l.b));
                for seg in crate::gfx::dash_polyline(&[s, e], false, &[3.0, 4.0]) {
                    painter.add(Shape::line(seg, Stroke::new(1.0, c)));
                }
            }
        }

        // selection window
        let win: Option<(Pt, Pt, bool)> = if let Some((pa, pb)) = self.vp.drag_rect {
            Some((pa, pb, pb.x < pa.x))
        } else if let (Some(a0), true) = (rt.idle_window, self.vp.mouse.is_some()) {
            Some((a0, rt.cursor.raw, rt.cursor.raw.x < a0.x))
        } else if let Some(w) = pending.and_then(|p| if p.kind == PromptKind::Select { p.window.as_ref() } else { None }) {
            w.a.map(|a0| (a0, rt.cursor.raw, w.mode.map_or(rt.cursor.raw.x < a0.x, |m| m == 'C')))
        } else {
            None
        };
        if let Some((wa, wb, crossing)) = win {
            let (s0, s1) = (to_s(wa), to_s(wb));
            let r = Rect::from_two_pos(s0, s1);
            let (fill, line) = if crossing { (Color32::from_rgba_unmultiplied(80, 190, 110, 46), Color32::from_rgb(0x79, 0xe2, 0x9a)) } else { (Color32::from_rgba_unmultiplied(70, 140, 230, 51), Color32::from_rgb(0x6c, 0xb2, 0xff)) };
            painter.rect_filled(r, 0.0, fill);
            if crossing {
                for seg in crate::gfx::dash_polyline(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()], true, &[5.0, 3.0]) {
                    painter.add(Shape::line(seg, Stroke::new(1.0, line)));
                }
            } else {
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, line), StrokeKind::Inside);
            }
        }

        // grips
        let grips_on = prefs_int(&a, "GRIPS") != 0 && ed.sel.len() as i64 <= prefs_int(&a, "GRIPOBJLIMIT") && self.vp.realtime.is_none();
        if grips_on {
            let size = a.sysvars.real("GRIPSIZE") as f32 + 2.0;
            for e in ed.selected() {
                if doc.is_locked(e) {
                    continue;
                }
                for g in grips_of(e, doc) {
                    let s = to_s(g.pt);
                    if s.x < rect.left() - 10.0 || s.y < rect.top() - 10.0 || s.x > rect.right() + 10.0 || s.y > rect.bottom() + 10.0 {
                        continue;
                    }
                    let hot = rt.hot_grip.as_ref().is_some_and(|h| h.entity == e.id && h.id == g.id);
                    let hov = self.vp.mouse.is_some_and(|m| (Pos2::new(origin.x + m.x, origin.y + m.y) - s).length() <= 6.0);
                    let fill = if hot { Color32::from_rgb(0xff, 0x3b, 0x30) } else if hov { Color32::from_rgb(0xff, 0xa6, 0x3d) } else { Color32::from_rgb(0x2f, 0x7b, 0xff) };
                    let edge = if theme.dark { Color32::from_rgb(0x0a, 0x1c, 0x3d) } else { Color32::from_rgb(0x12, 0x30, 0x6b) };
                    let r = if g.kind == GripKind::Move && g.id == "m" { Rect::from_center_size(s + vec2(0.0, 0.0), vec2(size + 2.0, size - 2.0)) } else { Rect::from_center_size(s, vec2(size, size)) };
                    painter.rect_filled(r, 0.0, fill);
                    painter.rect_stroke(r, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
                }
            }
        }

        // snap marker + tooltips
        let mouse_abs = self.vp.mouse.map(|m| Pos2::new(origin.x + m.x, origin.y + m.y));
        if let Some(snap) = &rt.cursor.snap {
            let s = to_s(snap.pt);
            draw_marker(painter, snap.kind, s);
            if sv_int(&a, "AUTOSNAP") & 2 != 0 {
                tip_box(painter, snap.kind.name(), s + vec2(14.0, 12.0));
            }
        } else if let (Some(tr), Some(m)) = (track, mouse_abs) {
            if !tr.label.is_empty() && sv_int(&a, "AUTOSNAP") & 32 != 0 {
                tip_box(painter, &tr.label, m + vec2(16.0, 18.0));
            }
        }

        // crosshair / pickbox
        if let Some(m) = mouse_abs {
            if self.vp.realtime.is_none() {
                let c = col(theme.cross, 1.0);
                let (x, y) = (m.x.round() + 0.5, m.y.round() + 0.5);
                let frac = (a.sysvars.real("CURSORSIZE").clamp(1.0, 100.0) / 100.0) as f32;
                let arm = if frac >= 1.0 { 4000.0 } else { (frac * rect.width() / 2.0).min(frac * rect.height() / 2.0) };
                let pb = a.sysvars.real("PICKBOX") as f32 * 2.0 + 1.0;
                if kind != "select" {
                    painter.line_segment([Pos2::new(x - arm, y), Pos2::new(x + arm, y)], Stroke::new(1.0, c));
                    painter.line_segment([Pos2::new(x, y - arm), Pos2::new(x, y + arm)], Stroke::new(1.0, c));
                }
                if matches!(kind, "idle" | "select" | "entity") {
                    painter.rect_stroke(Rect::from_center_size(Pos2::new(x, y), vec2(pb, pb)), 0.0, Stroke::new(1.0, c), StrokeKind::Inside);
                }
            }
        }

        // dynamic input
        if let (Some(m), Some(p)) = (mouse_abs, pending) {
            if a.sysvars.dyn_mode() > 0 && self.vp.realtime.is_none() && p.kind != PromptKind::Dialog {
                let prec = doc.settings.units.precision.max(0) as usize;
                let msg = cadforge_engine::runtime::prompt_text(Some(p)).trim_end_matches(':').replace('\n', " ");
                let msg = if p.spec.options.is_empty() { msg } else { format!("{msg}  ▾") };
                let typed = if self.ui.cmd.dynamic { self.ui.cmd.text.clone() } else { String::new() };
                let cur = rt.cursor.pt;
                let field = if !typed.is_empty() {
                    Some(format!("{typed}|"))
                } else if matches!(p.kind, PromptKind::Point | PromptKind::Distance | PromptKind::Angle) && a.sysvars.pointer_input() {
                    let base = p.base();
                    if let (Some(b), true) = (base, a.sysvars.dimension_input()) {
                        let d = dist(b, cur);
                        let ang = cadforge_core::geom::deg(cadforge_core::geom::norm_angle(cadforge_core::geom::angle_of(b, cur)));
                        Some(format!("L {}   ∠ {}°", cadforge_core::style::fmt_num(d, prec, true), cadforge_core::style::fmt_num(ang, 0, true)))
                    } else {
                        Some(format!("{},  {}", cadforge_core::style::fmt_num(cur.x, prec.min(4), true), cadforge_core::style::fmt_num(cur.y, prec.min(4), true)))
                    }
                } else {
                    None
                };
                let galley = painter.layout_no_wrap(msg, FontId::proportional(11.5), t.text);
                let w1 = galley.size().x + 14.0;
                let mut x = m.x + 24.0;
                let mut y = m.y + 22.0;
                let fw = field.as_ref().map_or(0.0, |f| painter.layout_no_wrap(f.clone(), FontId::monospace(11.5), t.text).size().x + 14.0);
                let w = w1.max(fw);
                if x + w > rect.right() - 6.0 {
                    x = m.x - w - 16.0;
                }
                if y + 46.0 > rect.bottom() - 6.0 {
                    y = m.y - 54.0;
                }
                let r1 = Rect::from_min_size(Pos2::new(x.max(rect.left() + 4.0), y), vec2(w, 20.0));
                painter.rect_filled(r1, CornerRadius::same(2), Color32::from_rgba_unmultiplied(30, 36, 43, 235));
                painter.rect_stroke(r1, CornerRadius::same(2), Stroke::new(1.0, t.sep), StrokeKind::Inside);
                painter.galley(Pos2::new(r1.left() + 7.0, r1.center().y - galley.size().y / 2.0), galley, t.text);
                if let Some(f) = field {
                    let r2 = Rect::from_min_size(r1.left_bottom() + vec2(0.0, 2.0), vec2(w, 20.0));
                    painter.rect_filled(r2, CornerRadius::same(2), Color32::from_rgba_unmultiplied(20, 26, 32, 240));
                    painter.rect_stroke(r2, CornerRadius::same(2), Stroke::new(1.0, t.accent), StrokeKind::Inside);
                    painter.text(Pos2::new(r2.left() + 7.0, r2.center().y), Align2::LEFT_CENTER, f, FontId::monospace(11.5), t.accent2);
                }
            }
        }
    }
}

fn sv_int(a: &App, name: &str) -> i64 {
    a.sysvars.int(name)
}
fn prefs_int(a: &App, name: &str) -> i64 {
    a.sysvars.int(name)
}

fn tip_box(painter: &egui::Painter, text: &str, at: Pos2) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(11.0), Color32::from_rgb(0x2b, 0x2a, 0x1b));
    let r = Rect::from_min_size(at, galley.size() + vec2(10.0, 6.0));
    painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(255, 248, 205, 247));
    painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(0xb3, 0xa2, 0x4f)), StrokeKind::Inside);
    painter.galley(at + vec2(5.0, 3.0), galley, Color32::BLACK);
}

fn draw_marker(painter: &egui::Painter, kind: SnapKind, c: Pos2) {
    let s = 7.0f32;
    let st = Stroke::new(2.0, YELLOW);
    let rect_ = |x0: f32, y0: f32, w: f32, h: f32| painter.add(Shape::closed_line(vec![Pos2::new(x0, y0), Pos2::new(x0 + w, y0), Pos2::new(x0 + w, y0 + h), Pos2::new(x0, y0 + h)], st));
    let circle = |c: Pos2, r: f32| painter.add(Shape::circle_stroke(c, r, st));
    let line = |a: Pos2, b: Pos2| painter.add(Shape::line_segment([a, b], st));
    let (x, y) = (c.x, c.y);
    match kind {
        SnapKind::End => {
            rect_(x - s, y - s, s * 2.0, s * 2.0);
        }
        SnapKind::Mid => {
            painter.add(Shape::closed_line(vec![Pos2::new(x, y - s), Pos2::new(x + s, y + s * 0.8), Pos2::new(x - s, y + s * 0.8)], st));
        }
        SnapKind::Cen => {
            circle(c, s);
        }
        SnapKind::Nod => {
            circle(c, s);
            line(Pos2::new(x - s * 0.7, y - s * 0.7), Pos2::new(x + s * 0.7, y + s * 0.7));
            line(Pos2::new(x + s * 0.7, y - s * 0.7), Pos2::new(x - s * 0.7, y + s * 0.7));
        }
        SnapKind::Qua => {
            painter.add(Shape::closed_line(vec![Pos2::new(x, y - s - 1.0), Pos2::new(x + s + 1.0, y), Pos2::new(x, y + s + 1.0), Pos2::new(x - s - 1.0, y)], st));
        }
        SnapKind::Int => {
            line(Pos2::new(x - s, y - s), Pos2::new(x + s, y + s));
            line(Pos2::new(x + s, y - s), Pos2::new(x - s, y + s));
        }
        SnapKind::Ins => {
            rect_(x - s, y - s, s * 1.4, s * 1.4);
            rect_(x - s * 0.4, y - s * 0.4, s * 1.4, s * 1.4);
        }
        SnapKind::Per => {
            line(Pos2::new(x - s, y + s), Pos2::new(x - s, y - s));
            line(Pos2::new(x - s, y + s), Pos2::new(x + s, y + s));
            rect_(x - s, y + s - 5.0, 5.0, 5.0);
        }
        SnapKind::Tan => {
            circle(Pos2::new(x, y + 2.0), s - 1.0);
            line(Pos2::new(x - s - 1.0, y - s + 1.0), Pos2::new(x + s + 1.0, y - s + 1.0));
        }
        SnapKind::Nea => {
            painter.add(Shape::closed_line(vec![Pos2::new(x - s, y - s), Pos2::new(x + s, y - s), Pos2::new(x - s, y + s), Pos2::new(x + s, y + s)], st));
        }
        SnapKind::App => {
            rect_(x - s, y - s, s * 2.0, s * 2.0);
        }
    }
}

// ---------------------------------------------------------------- viewport controls, ViewCube, navigation bar

const CUBE: f32 = 118.0;
const CUBE_R: f32 = 25.0;

struct FaceDef {
    name: &'static str,
    n: [f64; 3],
    u: [f64; 3],
    v: [f64; 3],
}

const FACES: [FaceDef; 6] = [
    FaceDef { name: "TOP", n: [0.0, 0.0, 1.0], u: [1.0, 0.0, 0.0], v: [0.0, 1.0, 0.0] },
    FaceDef { name: "BOTTOM", n: [0.0, 0.0, -1.0], u: [1.0, 0.0, 0.0], v: [0.0, -1.0, 0.0] },
    FaceDef { name: "FRONT", n: [0.0, -1.0, 0.0], u: [1.0, 0.0, 0.0], v: [0.0, 0.0, 1.0] },
    FaceDef { name: "BACK", n: [0.0, 1.0, 0.0], u: [-1.0, 0.0, 0.0], v: [0.0, 0.0, 1.0] },
    FaceDef { name: "RIGHT", n: [1.0, 0.0, 0.0], u: [0.0, 1.0, 0.0], v: [0.0, 0.0, 1.0] },
    FaceDef { name: "LEFT", n: [-1.0, 0.0, 0.0], u: [0.0, -1.0, 0.0], v: [0.0, 0.0, 1.0] },
];

struct FaceGeo {
    depth: f64,
    origin: Pos2,
    uu: egui::Vec2,
    vv: egui::Vec2,
}

fn cam_rot(yaw: f64, pitch: f64, v: [f64; 3]) -> [f64; 3] {
    let (cy, sy, cp, sp) = (yaw.cos(), yaw.sin(), pitch.cos(), pitch.sin());
    let x1 = v[0] * cy - v[1] * sy;
    let y1 = v[0] * sy + v[1] * cy;
    [x1, y1 * sp + v[2] * cp, -y1 * cp + v[2] * sp]
}

fn face_geo(yaw: f64, pitch: f64, f: &FaceDef, c0: Pos2) -> FaceGeo {
    let c = cam_rot(yaw, pitch, f.n);
    let u = cam_rot(yaw, pitch, f.u);
    let v = cam_rot(yaw, pitch, f.v);
    let r = CUBE_R as f64;
    FaceGeo { depth: c[2], origin: Pos2::new(c0.x + (r * c[0]) as f32, c0.y - (r * c[1]) as f32), uu: vec2((r * u[0]) as f32, (-r * u[1]) as f32), vv: vec2((r * v[0]) as f32, (-r * v[1]) as f32) }
}

impl FaceGeo {
    fn p(&self, s: f32, t: f32) -> Pos2 {
        self.origin + self.uu * s + self.vv * t
    }
}

fn visible_faces(yaw: f64, pitch: f64, c0: Pos2) -> Vec<(usize, FaceGeo)> {
    let mut v: Vec<(usize, FaceGeo)> = FACES.iter().enumerate().map(|(i, f)| (i, face_geo(yaw, pitch, f, c0))).filter(|(_, g)| g.depth > 0.001).collect();
    v.sort_by(|a, b| a.1.depth.partial_cmp(&b.1.depth).unwrap_or(std::cmp::Ordering::Equal));
    v
}

fn cell_at(yaw: f64, pitch: f64, c0: Pos2, p: Pos2) -> Option<CubeHit> {
    for (i, g) in visible_faces(yaw, pitch, c0).into_iter().rev() {
        let d = p - g.origin;
        let det = g.uu.x * g.vv.y - g.uu.y * g.vv.x;
        if det.abs() < 1e-6 {
            continue;
        }
        let s = (d.x * g.vv.y - d.y * g.vv.x) / det;
        let t = (g.uu.x * d.y - g.uu.y * d.x) / det;
        if s.abs() <= 1.0 && t.abs() <= 1.0 {
            let ii = if s.abs() < 0.58 { 0 } else { s.signum() as i32 };
            let jj = if t.abs() < 0.58 { 0 } else { t.signum() as i32 };
            return Some(CubeHit::Cell { face: i, i: ii, j: jj });
        }
    }
    None
}

const ARROWS: [(f32, f32, i32, i32); 4] = [(-50.0, 0.0, -1, 0), (50.0, 0.0, 1, 0), (0.0, -52.0, 0, -1), (0.0, 52.0, 0, 1)];

fn cube_hit(yaw: f64, pitch: f64, local: Pos2) -> Option<CubeHit> {
    let c0 = Pos2::new(CUBE / 2.0, CUBE / 2.0);
    if (local - Pos2::new(14.0, 14.0)).length() < 11.0 {
        return Some(CubeHit::Home);
    }
    for (i, a) in ARROWS.iter().enumerate() {
        if (local - (c0 + vec2(a.0, a.1))).length() < 9.0 {
            return Some(CubeHit::Arrow(i));
        }
    }
    cell_at(yaw, pitch, c0, local)
}

impl CadForgeApp {
    fn viewport_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: Rect) {
        let t = self.t;
        let (prefs_cube, prefs_nav, prefs_ctl) = {
            let a = self.h.borrow();
            (a.sysvars.int("NAVVCUBE") != 0, a.sysvars.int("NAVBAR") != 0, a.sysvars.int("VPCONTROL") != 0)
        };
        // [-] [Top] [2D Wireframe]
        if prefs_ctl {
            let (preset, style) = {
                let a = self.h.borrow();
                let e = a.editor().expect("editor");
                (e.view.preset_name().to_string(), e.visual_style.clone())
            };
            let mut x = rect.left() + 10.0;
            let y = rect.top() + 8.0;
            let labels: [(String, u8); 3] = [("[–]".into(), 0), (format!("[{preset}]"), 1), (format!("[{style}]"), 2)];
            for (text, which) in labels {
                let galley = ui.painter().layout_no_wrap(text.clone(), FontId::proportional(12.0), t.text2);
                let r = Rect::from_min_size(Pos2::new(x, y), galley.size() + vec2(8.0, 4.0));
                let id = ui.id().with(("vpctl", which));
                let resp = ui.interact(r, id, Sense::click());
                let color = if resp.hovered() { t.accent2 } else { t.text2 };
                ui.painter().galley(r.min + vec2(4.0, 2.0), galley, color);
                self.vp.blocked.push(r);
                if resp.clicked() {
                    self.viewport_menu(ctx, which, r);
                }
                x += r.width() + 2.0;
            }
        }
        // ViewCube
        if prefs_cube {
            let cube_rect = Rect::from_min_size(Pos2::new(rect.right() - CUBE - 14.0, rect.top() + 12.0), vec2(CUBE, CUBE));
            self.viewcube(ui, ctx, cube_rect);
            self.vp.blocked.push(cube_rect.expand2(vec2(0.0, 22.0)));
        }
        // navigation bar
        if prefs_nav {
            self.navbar(ui, ctx, rect);
        }
    }

    fn viewport_menu(&mut self, ctx: &egui::Context, which: u8, anchor: Rect) {
        use crate::menu::MItem;
        let items: Vec<MItem> = match which {
            0 => {
                let prefs: Vec<(&'static str, &'static str)> = vec![("ViewCube", "NAVVCUBE"), ("Navigation Bar", "NAVBAR"), ("UCS Icon", "UCSICON")];
                let a = self.h.borrow();
                let mut v = vec![MItem::item("Maximize Viewport").disabled(true), MItem::Sep, MItem::Header("Viewport Configuration List".into()), MItem::item("Single").checked(true), MItem::Sep];
                for (label, var) in prefs {
                    let on = a.sysvars.int(var) != 0;
                    v.push(MItem::item(label).checked(on).on(move |app| {
                        let _ = app.h.borrow_mut().setvar(var, if on { 0.0 } else { 1.0 });
                    }));
                }
                v
            }
            1 => {
                let cur = self.h.borrow().editor().map(|e| e.view.preset_name()).unwrap_or("Top");
                let mut v: Vec<MItem> = vec![];
                for (l, n) in [("Top", "top"), ("Bottom", "bottom"), ("Left", "left"), ("Right", "right"), ("Front", "front"), ("Back", "back")] {
                    v.push(MItem::item(l).checked(cur == l).on(move |app| app.animate_preset(n)));
                }
                v.push(MItem::Sep);
                for (l, n) in [("SW Isometric", "swiso"), ("SE Isometric", "seiso"), ("NE Isometric", "neiso"), ("NW Isometric", "nwiso")] {
                    v.push(MItem::item(l).checked(cur == l).on(move |app| app.animate_preset(n)));
                }
                v.push(MItem::Sep);
                v.push(MItem::item("Parallel").checked(true));
                v.push(MItem::item("Perspective").disabled(true));
                v
            }
            _ => {
                let cur = self.h.borrow().editor().map(|e| e.visual_style.clone()).unwrap_or_default();
                ["2D Wireframe", "Wireframe", "Hidden", "Shaded", "Shaded with Edges"].iter().map(|n| {
                    let n = n.to_string();
                    let n2 = n.clone();
                    MItem::item(&n).checked(cur == n).on(move |app| app.set_visual_style(&n2))
                }).collect()
            }
        };
        self.dropdown(ctx, anchor, items, 170.0);
    }

    pub fn set_visual_style(&mut self, name: &str) {
        let mut a = self.h.borrow_mut();
        let Some(ed) = a.editor_mut() else { return };
        ed.visual_style = name.to_string();
        if name != "2D Wireframe" && ed.view.is_plan() && !ed.doc.solids.is_empty() {
            ed.view.set_preset("swiso");
        }
    }

    pub fn animate_preset(&mut self, name: &str) {
        let mut v = cadforge_core::view::View::new();
        v.set_preset(name);
        let (yaw, pitch) = (v.yaw, v.pitch);
        self.animate_view(yaw, pitch);
    }

    pub fn animate_view(&mut self, yaw: f64, pitch: f64) {
        let (y0, p0) = {
            let mut a = self.h.borrow_mut();
            let Some(e) = a.editor_mut() else { return };
            e.view.push();
            (e.view.yaw, e.view.pitch)
        };
        let dy = cadforge_core::geom::norm_angle_signed(yaw - y0);
        self.vp.cube.anim = Some((self.time, y0, dy, p0, pitch));
    }

    fn step_anim(&mut self, ctx: &egui::Context) {
        let Some((t0, y0, dy, p0, pitch)) = self.vp.cube.anim else { return };
        let k = ((self.time - t0) / 0.26).clamp(0.0, 1.0);
        let e = 1.0 - (1.0 - k).powi(3);
        self.with_view(|v| {
            v.yaw = y0 + dy * e;
            v.pitch = p0 + (pitch - p0) * e;
            if k >= 1.0 {
                v.yaw = cadforge_core::geom::norm_angle_signed(y0 + dy);
                v.pitch = pitch;
            }
        });
        if k >= 1.0 {
            self.vp.cube.anim = None;
        } else {
            ctx.request_repaint();
        }
    }

    fn viewcube(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, r: Rect) {
        self.step_anim(ctx);
        let t = self.t;
        let (yaw, pitch) = {
            let a = self.h.borrow();
            let e = a.editor().expect("editor");
            (e.view.yaw, e.view.pitch)
        };
        let id = ui.id().with("viewcube");
        let resp = ui.interact(r, id, Sense::click_and_drag());
        let local = resp.hover_pos().map(|p| p - r.min);
        let hover = local.and_then(|l| cube_hit(yaw, pitch, l.to_pos2()));
        if resp.hovered() {
            ctx.set_cursor_icon(if hover.is_some() { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Default });
        }
        self.vp.cube.hover = hover;
        let p = ui.painter_at(r.expand2(vec2(0.0, 26.0)));
        let dark = t.dark;
        let c0 = r.min + vec2(CUBE / 2.0, CUBE / 2.0);
        let plan = pitch.abs() > 1.05;
        if plan {
            p.circle_stroke(c0, 47.0, Stroke::new(1.2, if dark { Color32::from_rgba_unmultiplied(230, 236, 243, 140) } else { Color32::from_rgba_unmultiplied(40, 50, 60, 128) }));
            for (label, ang) in [("N", std::f64::consts::FRAC_PI_2), ("E", 0.0), ("S", -std::f64::consts::FRAC_PI_2), ("W", std::f64::consts::PI)] {
                let rr = cam_rot(yaw, pitch, [ang.cos(), ang.sin(), 0.0]);
                let h = rr[0].hypot(rr[1]);
                let (x, y) = (40.0 * rr[0] / h.max(0.4) * h, -40.0 * rr[1] / h.max(0.4) * h);
                let pos = c0 + vec2(x as f32 * 1.22, y as f32 * 1.22);
                p.text(pos, Align2::CENTER_CENTER, label, FontId::new(10.0, egui::FontFamily::Name("bold".into())), if dark { Color32::from_rgb(0xdf, 0xe5, 0xec) } else { Color32::from_rgb(0x2a, 0x33, 0x3c) });
            }
        }
        let hov_cell = if let Some(CubeHit::Cell { face, i, j }) = hover { Some((face, i, j)) } else { None };
        for (fi, g) in visible_faces(yaw, pitch, c0) {
            let shade = 0.55 + 0.45 * g.depth;
            let base = if dark { [70.0, 82.0, 96.0] } else { [214.0, 222.0, 232.0] };
            let k = if dark { shade + 0.25 } else { 0.86 + 0.14 * shade };
            let fill = Color32::from_rgb((base[0] * k).min(255.0) as u8, (base[1] * k).min(255.0) as u8, (base[2] * k).min(255.0) as u8);
            let edge = if dark { Color32::from_rgb(0x9f, 0xb0, 0xc2) } else { Color32::from_rgb(0x6c, 0x7b, 0x8b) };
            let q = vec![g.p(-1.0, -1.0), g.p(1.0, -1.0), g.p(1.0, 1.0), g.p(-1.0, 1.0)];
            p.add(Shape::Path(egui::epaint::PathShape { points: q, closed: true, fill, stroke: egui::epaint::PathStroke::new(1.0, edge) }));
            if let Some((hf, hi, hj)) = hov_cell {
                if hf == fi {
                    let lo = |i: i32| if i == 0 { -0.58 } else if i > 0 { 0.58 } else { -1.0 };
                    let hi_ = |i: i32| if i == 0 { 0.58 } else if i > 0 { 1.0 } else { -0.58 };
                    let cq = vec![g.p(lo(hi), lo(hj)), g.p(hi_(hi), lo(hj)), g.p(hi_(hi), hi_(hj)), g.p(lo(hi), hi_(hj))];
                    p.add(Shape::convex_polygon(cq, Color32::from_rgba_unmultiplied(80, 170, 255, 140), Stroke::NONE));
                }
            }
            // face label: place at the face centre, scaled along the face axes (approximation of the affine text map)
            let ang = g.uu.y.atan2(g.uu.x);
            let sz = 10.0 * (g.uu.length() / CUBE_R).max(0.55);
            let galley = p.layout_no_wrap(FACES[fi].name.to_string(), FontId::new(sz, egui::FontFamily::Name("bold".into())), if dark { Color32::from_rgb(0xee, 0xf3, 0xf8) } else { Color32::from_rgb(0x1d, 0x26, 0x30) });
            let off = vec2(-galley.size().x / 2.0, -galley.size().y / 2.0);
            let (s, c) = ang.sin_cos();
            let rot_off = vec2(off.x * c - off.y * s, off.x * s + off.y * c);
            p.add(egui::epaint::TextShape::new(g.origin + rot_off, galley, Color32::WHITE).with_angle(ang));
        }
        // home + arrows
        let home_col = if hover == Some(CubeHit::Home) { Color32::from_rgb(0x5b, 0xb8, 0xff) } else if dark { Color32::from_rgb(0xae, 0xb9, 0xc6) } else { Color32::from_rgb(0x55, 0x61, 0x6e) };
        let o = r.min;
        p.add(Shape::convex_polygon(vec![o + vec2(14.0, 7.0), o + vec2(21.0, 14.0), o + vec2(18.5, 14.0), o + vec2(18.5, 20.0), o + vec2(9.5, 20.0), o + vec2(9.5, 14.0), o + vec2(7.0, 14.0)], home_col, Stroke::NONE));
        for (i, a) in ARROWS.iter().enumerate() {
            let hc = if hover == Some(CubeHit::Arrow(i)) { Color32::from_rgb(0x5b, 0xb8, 0xff) } else if dark { Color32::from_rgb(0xae, 0xb9, 0xc6) } else { Color32::from_rgb(0x55, 0x61, 0x6e) };
            let (cx, cy) = (c0.x + a.0, c0.y + a.1);
            let (dx, dy) = (a.2 as f32, a.3 as f32);
            p.add(Shape::convex_polygon(vec![Pos2::new(cx + dx * 5.0, cy + dy * 5.0), Pos2::new(cx - dy * 5.0 - dx * 3.0, cy + dx * 5.0 - dy * 3.0), Pos2::new(cx + dy * 5.0 - dx * 3.0, cy - dx * 5.0 - dy * 3.0)], hc, Stroke::NONE));
        }
        // WCS label
        let wcs = Rect::from_center_size(Pos2::new(r.center().x, r.bottom() + 10.0), vec2(48.0, 16.0));
        let wresp = ui.interact(wcs, ui.id().with("wcs"), Sense::click());
        p.text(wcs.center(), Align2::CENTER_CENTER, "WCS ▾", FontId::proportional(11.0), if wresp.hovered() { t.accent2 } else { t.text2 });
        self.vp.blocked.push(wcs);
        if wresp.clicked() {
            use crate::menu::MItem;
            self.dropdown(ctx, wcs, vec![MItem::item("WCS").checked(true), MItem::item("New UCS...").disabled(true)], 120.0);
        }

        // interaction
        if resp.drag_started() {
            if let Some(pp) = resp.interact_pointer_pos() {
                let _ = pp;
                self.vp.cube.drag = Some((resp.interact_pointer_pos().unwrap_or(r.min), yaw, pitch, false));
            }
        }
        if resp.dragged() {
            if let (Some((start, y0, p0, _)), Some(now)) = (self.vp.cube.drag, resp.interact_pointer_pos()) {
                let d = now - start;
                if d.length() >= 4.0 || self.vp.cube.drag.is_some_and(|x| x.3) {
                    self.vp.cube.drag = Some((start, y0, p0, true));
                    self.vp.cube.anim = None;
                    self.with_view(|v| {
                        v.yaw = y0 - d.x as f64 * 0.012;
                        v.pitch = cadforge_core::geom::clamp(p0 + d.y as f64 * 0.01, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
                    });
                }
            }
        }
        if resp.drag_stopped() {
            self.vp.cube.drag = None;
        }
        if resp.clicked() {
            if let Some(h) = hover {
                match h {
                    CubeHit::Home => self.animate_preset("swiso"),
                    CubeHit::Arrow(i) => {
                        let a = ARROWS[i];
                        if a.2 != 0 {
                            self.animate_view(yaw + a.2 as f64 * std::f64::consts::FRAC_PI_2, pitch);
                        } else {
                            self.animate_view(yaw, cadforge_core::geom::clamp(pitch + a.3 as f64 * -std::f64::consts::FRAC_PI_4, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2));
                        }
                    }
                    CubeHit::Cell { face, i, j } => {
                        let f = &FACES[face];
                        let dir = [f.n[0] + i as f64 * f.u[0] + j as f64 * f.v[0], f.n[1] + i as f64 * f.u[1] + j as f64 * f.v[1], f.n[2] + i as f64 * f.u[2] + j as f64 * f.v[2]];
                        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                        let np = (dir[2] / len).asin();
                        let ny = if dir[0].hypot(dir[1]) < 1e-9 { yaw } else { -(dir[0]).atan2(-dir[1]) };
                        self.animate_view(ny, np);
                    }
                }
            }
        }
    }

    fn navbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: Rect) {
        let t = self.t;
        let w = 34.0;
        let top = rect.top() + 12.0 + CUBE + 34.0;
        let bar = Rect::from_min_size(Pos2::new(rect.right() - w - 10.0, top), vec2(w, 3.0 * 30.0 + 6.0));
        let p = ui.painter();
        p.rect_filled(bar, CornerRadius::same(4), Color32::from_rgba_unmultiplied(t.chrome2.r(), t.chrome2.g(), t.chrome2.b(), 235));
        p.rect_stroke(bar, CornerRadius::same(4), Stroke::new(1.0, t.border), StrokeKind::Inside);
        self.vp.blocked.push(bar);
        let btns: [(&str, &str, &str); 3] = [("pan", "Pan", "PAN"), ("zoomextents", "Zoom Extents", "ZOOM"), ("orbit", "Orbit", "3DORBIT")];
        for (i, (icon, title, cmd)) in btns.iter().enumerate() {
            let r = Rect::from_min_size(bar.min + vec2(3.0, 3.0 + i as f32 * 30.0), vec2(28.0, 28.0));
            let resp = ui.interact(r, ui.id().with(("nav", i)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
            }
            self.icons.paint(ui.painter(), icon, r.shrink(5.0), &t, Tint::Normal);
            let resp = resp.on_hover_text(*title);
            if resp.clicked() {
                let mut args = cadforge_engine::runtime::Args::none();
                if *cmd == "ZOOM" {
                    args = args.with("mode", cadforge_engine::runtime::ArgVal::Str("extents".into()));
                }
                self.run_command(cmd, args);
            }
            let _ = ctx;
        }
    }
}
