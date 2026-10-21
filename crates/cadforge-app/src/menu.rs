//! Drop-down / context menu popups (icons, check marks, shortcuts, sub-menus), drawn on the foreground layer.
use crate::app::CadForgeApp;
use crate::icons::Tint;
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, Sense, Stroke};
use std::rc::Rc;

pub type Act = Rc<dyn Fn(&mut CadForgeApp)>;

#[derive(Clone, Default)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub swatch: Option<Color32>,
    pub checked: bool,
    pub disabled: bool,
    pub shortcut: Option<String>,
    pub act: Option<Act>,
    pub sub: Vec<MItem>,
}

#[derive(Clone)]
pub enum MItem {
    Item(MenuItem),
    Sep,
    Header(String),
}

impl MItem {
    pub fn item(label: &str) -> MItem {
        MItem::Item(MenuItem { label: label.to_string(), ..Default::default() })
    }
    fn with(mut self, f: impl FnOnce(&mut MenuItem)) -> MItem {
        if let MItem::Item(m) = &mut self {
            f(m);
        }
        self
    }
    pub fn icon(self, i: &str) -> MItem {
        self.with(|m| m.icon = Some(i.to_string()))
    }
    pub fn icon_opt(self, i: Option<&str>) -> MItem {
        self.with(|m| m.icon = i.filter(|s| !s.is_empty()).map(str::to_string))
    }
    pub fn swatch(self, c: Color32) -> MItem {
        self.with(|m| m.swatch = Some(c))
    }
    pub fn checked(self, v: bool) -> MItem {
        self.with(|m| m.checked = v)
    }
    pub fn disabled(self, v: bool) -> MItem {
        self.with(|m| m.disabled = v)
    }
    pub fn shortcut(self, s: &str) -> MItem {
        self.with(|m| m.shortcut = Some(s.to_string()))
    }
    pub fn on(self, f: impl Fn(&mut CadForgeApp) + 'static) -> MItem {
        self.with(|m| m.act = Some(Rc::new(f)))
    }
    pub fn sub(self, items: Vec<MItem>) -> MItem {
        self.with(|m| m.sub = items)
    }
}

pub struct Popup {
    pub id: u64,
    pub pos: Pos2,
    /// Open upwards: `pos` is then the bottom-left corner.
    pub up: bool,
    pub min_width: f32,
    pub items: Vec<MItem>,
    pub open_path: Vec<usize>,
    pub opened_frame: u64,
}

impl Popup {
    pub fn new(pos: Pos2, items: Vec<MItem>) -> Popup {
        Popup { id: 0, pos, up: false, min_width: 150.0, items, open_path: vec![], opened_frame: 0 }
    }
    pub fn up(mut self) -> Popup {
        self.up = true;
        self
    }
    pub fn min_width(mut self, w: f32) -> Popup {
        self.min_width = w;
        self
    }
}

const ROW_H: f32 = 24.0;

impl CadForgeApp {
    pub fn open_popup(&mut self, ctx: &egui::Context, mut p: Popup) {
        p.opened_frame = ctx.cumulative_frame_nr();
        p.id = p.opened_frame;
        self.popup = Some(p);
    }

    /// Drop-down below (or above, for the status bar) an anchor rectangle.
    pub fn dropdown(&mut self, ctx: &egui::Context, anchor: Rect, items: Vec<MItem>, min_width: f32) {
        let p = Popup::new(anchor.left_bottom() + vec2(0.0, 1.0), items).min_width(min_width.max(anchor.width()));
        self.open_popup(ctx, p);
    }
    pub fn dropup(&mut self, ctx: &egui::Context, anchor: Rect, items: Vec<MItem>, min_width: f32) {
        let p = Popup::new(anchor.left_top() - vec2(0.0, 1.0), items).min_width(min_width).up();
        self.open_popup(ctx, p);
    }

    pub fn show_popup(&mut self, ctx: &egui::Context) {
        let Some(mut pop) = self.popup.take() else { return };
        let mut chosen: Option<Act> = None;
        let mut rects: Vec<Rect> = vec![];
        let mut close = false;
        let items = pop.items.clone();
        let (pos, up, w) = (pop.pos, pop.up, pop.min_width);
        self.draw_level(ctx, &mut pop, 0, &items, pos, up, w, &mut chosen, &mut rects, &mut close);
        let pressed = ctx.input(|i| i.pointer.any_pressed());
        if pressed && ctx.cumulative_frame_nr() > pop.opened_frame + 1 {
            let p = ctx.input(|i| i.pointer.interact_pos());
            if !p.is_some_and(|p| rects.iter().any(|r| r.contains(p))) {
                close = true;
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if !close {
            self.popup = Some(pop);
        }
        if let Some(a) = chosen {
            a(self);
        }
        ctx.request_repaint();
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_level(&self, ctx: &egui::Context, pop: &mut Popup, level: usize, items: &[MItem], pos: Pos2, up: bool, min_w: f32, chosen: &mut Option<Act>, rects: &mut Vec<Rect>, close: &mut bool) {
        let t = self.t;
        // measure
        let font = FontId::proportional(12.5);
        let measure = |s: &str| ctx.fonts_mut(|f| f.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x);
        let mut width = min_w;
        for it in items {
            if let MItem::Item(m) = it {
                let mut w = 34.0 + measure(&m.label) + 20.0;
                if let Some(s) = &m.shortcut {
                    w += measure(s) + 28.0;
                }
                if !m.sub.is_empty() {
                    w += 18.0;
                }
                if m.swatch.is_some() {
                    w += 20.0;
                }
                width = width.max(w);
            }
        }
        let mut sub_open: Option<(usize, Rect)> = None;
        let area = egui::Area::new(egui::Id::new(("cf-popup", pop.id, level))).order(egui::Order::Foreground).fixed_pos(pos).pivot(if up { Align2::LEFT_BOTTOM } else { Align2::LEFT_TOP }).constrain(true);
        let resp = area.show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(t.raised)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(3))
                .inner_margin(Margin::symmetric(0, 4))
                .shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) });
            frame.show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.set_width(width);
                // long menus (linetypes, fonts...) scroll instead of running off the screen
                let max_h = (ctx.content_rect().height() * 0.8 - 20.0).max(160.0);
                egui::ScrollArea::vertical().max_height(max_h).auto_shrink([true, true]).show(ui, |ui| {
                ui.set_width(width);
                for (i, it) in items.iter().enumerate() {
                    match it {
                        MItem::Sep => {
                            let (r, _) = ui.allocate_exact_size(vec2(width, 9.0), Sense::hover());
                            ui.painter().line_segment([Pos2::new(r.left() + 6.0, r.center().y), Pos2::new(r.right() - 6.0, r.center().y)], Stroke::new(1.0, t.sep));
                        }
                        MItem::Header(h) => {
                            let (r, _) = ui.allocate_exact_size(vec2(width, 22.0), Sense::hover());
                            ui.painter().text(Pos2::new(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, h.to_uppercase(), FontId::proportional(10.0), t.text3);
                        }
                        MItem::Item(m) => {
                            let (r, resp) = ui.allocate_exact_size(vec2(width, ROW_H), if m.disabled { Sense::hover() } else { Sense::click() });
                            let hot = resp.hovered() && !m.disabled || pop.open_path.get(level) == Some(&i) && !m.sub.is_empty();
                            if hot {
                                ui.painter().rect_filled(r.shrink2(vec2(3.0, 0.0)), CornerRadius::same(2), t.accent_soft);
                            }
                            let fg = if m.disabled { t.text3 } else { t.text };
                            let mut x = r.left() + 8.0;
                            if let Some(ic) = &m.icon {
                                let tint = if m.disabled { Tint::Mono(t.text3) } else { Tint::Normal };
                                self.icons.paint(ui.painter(), ic, Rect::from_min_size(Pos2::new(x, r.center().y - 8.0), vec2(16.0, 16.0)), &t, tint);
                            } else if m.checked {
                                let c = Pos2::new(x + 8.0, r.center().y);
                                ui.painter().line(vec![c + vec2(-4.0, 0.0), c + vec2(-1.0, 3.5), c + vec2(4.5, -3.5)], Stroke::new(1.8, t.accent2));
                            }
                            if m.icon.is_some() && m.checked {
                                ui.painter().rect_stroke(Rect::from_min_size(Pos2::new(x - 2.0, r.center().y - 10.0), vec2(20.0, 20.0)), CornerRadius::same(2), Stroke::new(1.0, t.accent2), egui::StrokeKind::Inside);
                            }
                            x += 24.0;
                            if let Some(sw) = m.swatch {
                                let sr = Rect::from_min_size(Pos2::new(x, r.center().y - 6.0), vec2(12.0, 12.0));
                                ui.painter().rect_filled(sr, CornerRadius::same(2), sw);
                                ui.painter().rect_stroke(sr, CornerRadius::same(2), Stroke::new(1.0, t.sep), egui::StrokeKind::Inside);
                                x += 18.0;
                            }
                            ui.painter().text(Pos2::new(x, r.center().y), Align2::LEFT_CENTER, &m.label, font.clone(), fg);
                            if let Some(s) = &m.shortcut {
                                ui.painter().text(Pos2::new(r.right() - 14.0, r.center().y), Align2::RIGHT_CENTER, s, FontId::proportional(11.5), t.text3);
                            }
                            if !m.sub.is_empty() {
                                let c = Pos2::new(r.right() - 12.0, r.center().y);
                                ui.painter().line(vec![c + vec2(-2.5, -4.0), c + vec2(1.5, 0.0), c + vec2(-2.5, 4.0)], Stroke::new(1.4, t.text2));
                            }
                            if resp.hovered() && !m.disabled {
                                if !m.sub.is_empty() {
                                    pop.open_path.truncate(level);
                                    pop.open_path.push(i);
                                } else if pop.open_path.len() > level {
                                    pop.open_path.truncate(level);
                                }
                            }
                            if pop.open_path.get(level) == Some(&i) && !m.sub.is_empty() {
                                sub_open = Some((i, r));
                            }
                            if resp.clicked() && !m.disabled && m.sub.is_empty() {
                                if let Some(a) = &m.act {
                                    *chosen = Some(a.clone());
                                }
                                *close = true;
                            }
                        }
                    }
                }
                });
            });
        });
        rects.push(resp.response.rect);
        if let Some((i, r)) = sub_open {
            if let MItem::Item(m) = &items[i] {
                let sub_items = m.sub.clone();
                self.draw_level(ctx, pop, level + 1, &sub_items, Pos2::new(r.right() - 2.0, r.top() - 4.0), false, 150.0, chosen, rects, close);
            }
        }
    }

    pub fn show_toast(&mut self, ctx: &egui::Context) {
        let Some((msg, until)) = self.ui.toast.clone() else { return };
        if self.time > until {
            self.ui.toast = None;
            return;
        }
        let t = self.t;
        egui::Area::new(egui::Id::new("cf-toast")).order(egui::Order::Tooltip).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -110.0)).show(ctx, |ui| {
            egui::Frame::new().fill(t.raised).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(4)).inner_margin(Margin::symmetric(14, 8)).shadow(egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(100) }).show(ui, |ui| {
                ui.label(egui::RichText::new(msg).color(t.text));
            });
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}
