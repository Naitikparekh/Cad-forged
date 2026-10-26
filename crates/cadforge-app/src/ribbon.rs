//! The ribbon: tabs, panels (with slide-outs, launchers and overflow collapsing), large / small / split buttons,
//! stacks and combo controls.
use crate::app::CadForgeApp;
use crate::chrome::{chevron, BODY_H, TABS_H};
use crate::data::{args_from_json, Item, Menu, MenuEntry, Panel, Tab};
use crate::icons::Tint;
use crate::menu::MItem;
use cadforge_core::style::{aci_hex, describe_color, linetype, parse_hex, Color, all_linetypes, LINEWEIGHTS};
use cadforge_engine::runtime::Args;
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind};

const LARGE_H: f32 = 72.0;
const SMALL_H: f32 = 23.0;
const PANEL_TITLE_H: f32 = 17.0;

/// Colour of the ribbon button face for the running command.
fn plain(s: &str) -> String {
    s.replace('\n', " ")
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sz {
    pub w: f32,
    pub h: f32,
}

pub enum Varies<T> {
    Varies,
    Val(T),
}

pub struct CurProps {
    pub layer: Varies<String>,
    pub layer_obj: Option<cadforge_core::style::Layer>,
    pub color: Varies<Option<Color>>,
    pub lw: Varies<Option<i32>>,
    pub ltype: Varies<Option<String>>,
    pub lw_resolved: i32,
    pub ltype_resolved: String,
}

impl CadForgeApp {
    /// Layer, colour, lineweight and linetype of the selection (or the current settings when nothing is selected).
    pub fn current_props(&self) -> Option<CurProps> {
        let a = self.h.borrow();
        let ed = a.editor()?;
        let doc = &ed.doc;
        let mut rows: Vec<(String, Option<Color>, Option<i32>, Option<String>)> = vec![];
        for e in ed.selected() {
            rows.push((if e.layer.is_empty() { "0".into() } else { e.layer.clone() }, e.color, e.lw, e.ltype.clone()));
        }
        for s in ed.selected_solids() {
            rows.push((if s.layer.is_empty() { "0".into() } else { s.layer.clone() }, s.color, None, None));
        }
        fn one<T: Clone + PartialEq>(rows: &[T]) -> Varies<T> {
            match rows.first() {
                Some(f) if rows.iter().all(|r| r == f) => Varies::Val(f.clone()),
                _ => Varies::Varies,
            }
        }
        let (layer, color, lw, ltype) = if rows.is_empty() {
            (Varies::Val(doc.cur.layer.clone()), Varies::Val(doc.cur.color), Varies::Val(doc.cur.lw), Varies::Val(doc.cur.ltype.clone()))
        } else {
            (one(&rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>()), one(&rows.iter().map(|r| r.1).collect::<Vec<_>>()), one(&rows.iter().map(|r| r.2).collect::<Vec<_>>()), one(&rows.iter().map(|r| r.3.clone()).collect::<Vec<_>>()))
        };
        let layer_obj = match &layer {
            Varies::Val(n) => doc.layer(n).cloned(),
            Varies::Varies => None,
        };
        let lw_resolved = match &lw {
            Varies::Val(Some(v)) if *v >= 0 => *v,
            _ => layer_obj.as_ref().map_or(25, |l| l.lw),
        };
        let ltype_resolved = match &ltype {
            Varies::Val(Some(l)) if l != "BYBLOCK" => l.clone(),
            _ => layer_obj.as_ref().map_or("Continuous".into(), |l| l.ltype.clone()),
        };
        Some(CurProps { layer, layer_obj, color, lw, ltype, lw_resolved, ltype_resolved })
    }

    /// Edit the document outside a command with a single undo step.
    pub fn edit_doc(&mut self, label: &str, f: impl FnOnce(&mut cadforge_core::drawing::Drawing)) {
        let mut a = self.h.borrow_mut();
        let Some(ed) = a.editor_mut() else { return };
        let nested = ed.doc.command_active();
        if !nested {
            ed.doc.begin_command();
        }
        f(&mut ed.doc);
        if !nested {
            ed.doc.end_command(label);
        }
        ed.prune();
        a.emit(cadforge_engine::app::Event::Change);
    }

    /// Set layer / colour / lineweight / linetype for the selection, or the current setting when nothing is selected.
    pub fn set_property(&mut self, kind: &'static str, color: Option<Color>, lw: Option<i32>, text: Option<String>) {
        let (ids, solid_ids) = {
            let a = self.h.borrow();
            let Some(ed) = a.editor() else { return };
            let doc = &ed.doc;
            let ids: Vec<u64> = ed.selected().into_iter().filter(|e| !doc.is_locked(e)).map(|e| e.id).collect();
            let sids: Vec<u64> = ed.selected_solids().into_iter().map(|s| s.id).collect();
            (ids, sids)
        };
        let text_c = text.clone();
        let apply = move |e_color: &mut Option<Color>, e_lw: Option<&mut Option<i32>>, e_lt: Option<&mut Option<String>>, e_layer: Option<&mut String>| match kind {
            "color" => *e_color = color,
            "lw" => {
                if let Some(l) = e_lw {
                    *l = lw;
                }
            }
            "ltype" => {
                if let Some(l) = e_lt {
                    *l = text_c.clone();
                }
            }
            "layer" => {
                if let (Some(l), Some(t)) = (e_layer, text_c.clone()) {
                    *l = t;
                }
            }
            _ => {}
        };
        if ids.is_empty() && solid_ids.is_empty() {
            let (c, l, t2) = (color, lw, text.clone());
            self.edit_doc("Properties", move |d| match kind {
                "layer" => {
                    if let Some(n) = &t2 {
                        let _ = d.set_current_layer(n);
                    }
                }
                "color" => d.cur.color = c,
                "lw" => d.cur.lw = l,
                "ltype" => d.cur.ltype = t2,
                _ => {}
            });
        } else {
            self.edit_doc("Properties", move |d| {
                d.will_change();
                for id in &ids {
                    if let Some(e) = d.by_id_mut(*id) {
                        let (mut c, mut lw_, mut lt, mut ly) = (e.color, e.lw, e.ltype.clone(), e.layer.clone());
                        apply(&mut c, Some(&mut lw_), Some(&mut lt), Some(&mut ly));
                        e.color = c;
                        e.lw = lw_;
                        e.ltype = lt;
                        e.layer = ly;
                    }
                }
                for id in &solid_ids {
                    if let Some(s) = d.solid_by_id_mut(*id) {
                        let (mut c, mut ly) = (s.color, s.layer.clone());
                        apply(&mut c, None, None, Some(&mut ly));
                        s.color = c;
                        s.layer = ly;
                    }
                }
                d.touch();
            });
        }
    }

    // ------------------------------------------------------------ tabs / panels

    fn ribbon_tabs(&self) -> Vec<Tab> {
        let mut v: Vec<Tab> = self.rib.tabs.iter().filter(|t| !self.ui.ribbon_hidden.contains(&t.id)).cloned().collect();
        if let Some(s) = self.h.borrow().hatch_session.as_ref() {
            let mut t = if s.editing.is_some() { self.rib.hatchedit.clone() } else { self.rib.hatch.clone() };
            t.id = if s.editing.is_some() { "hatchedit".into() } else { "hatchcreate".into() };
            v.push(t);
        }
        v
    }

    pub fn ribbon_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        let tabs = self.ribbon_tabs();
        if !tabs.iter().any(|x| x.id == self.ui.ribbon_tab) {
            self.ui.ribbon_tab = tabs[0].id.clone();
        }
        // ---- tab strip
        let strip = Rect::from_min_size(rect.left_top(), vec2(rect.width(), TABS_H));
        p.rect_filled(strip, 0.0, t.chrome);
        let mut x = rect.left() + 6.0;
        for tab in &tabs {
            let g = p.layout_no_wrap(crate::i18n::tr(&tab.title).to_string(), FontId::proportional(12.5), t.text);
            let w = g.size().x + 28.0;
            let r = Rect::from_min_size(Pos2::new(x, strip.top() + 2.0), vec2(w, TABS_H - 2.0));
            let resp = ui.interact(r, ui.id().with(("rtab", &tab.id)), Sense::click());
            let active = tab.id == self.ui.ribbon_tab;
            let corner = CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 };
            if active {
                p.rect_filled(r, corner, t.ribbon);
                p.line_segment([r.left_top() + vec2(0.0, 1.0), r.right_top() + vec2(0.0, 1.0)], Stroke::new(2.0, if tab.contextual { Color32::from_rgb(0xf0, 0xa5, 0x1a) } else { t.accent }));
            } else if resp.hovered() {
                p.rect_filled(r, corner, t.chrome2);
            }
            let color = if tab.contextual { Color32::from_rgb(0xff, 0xd3, 0x7a) } else if active || resp.hovered() { t.text } else { t.text2 };
            let font = if active { FontId::new(12.5, FontFamily::Name("bold".into())) } else { FontId::proportional(12.5) };
            p.text(r.center() + vec2(0.0, 1.0), Align2::CENTER_CENTER, crate::i18n::tr(&tab.title), font, color);
            if resp.clicked() {
                self.ui.ribbon_tab = tab.id.clone();
                if self.ui.ribbon_mode == 1 {
                    self.ui.flyout = if self.ui.flyout.as_deref() == Some(&tab.id) { None } else { Some(tab.id.clone()) };
                }
            }
            if resp.secondary_clicked() {
                self.ribbon_tab_menu(ctx, r);
            }
            x += w + 1.0;
        }
        // minimise toggle
        let mr = Rect::from_min_size(Pos2::new(rect.right() - 30.0, strip.top() + 2.0), vec2(24.0, 22.0));
        let mresp = ui.interact(mr, ui.id().with("rmin"), Sense::click());
        if mresp.hovered() {
            p.rect_filled(mr, CornerRadius::same(3), t.hover);
        }
        chevron(&p, mr.center(), 4.5, self.ui.ribbon_mode == 0, t.text2);
        let tip = ["Minimize to Tabs", "Minimize to Panel Titles", "Show Full Ribbon"][self.ui.ribbon_mode as usize % 3];
        if mresp.on_hover_text(tip).clicked() {
            self.ui.ribbon_mode = (self.ui.ribbon_mode + 1) % 3;
            self.ui.flyout = None;
        }
        // ---- body
        p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, t.border));
        if self.ui.ribbon_mode == 1 {
            if let Some(f) = self.ui.flyout.clone() {
                if let Some(tab) = tabs.iter().find(|x| x.id == f) {
                    self.ribbon_flyout_body(ctx, rect, tab.clone());
                }
            }
            return;
        }
        let Some(tab) = tabs.iter().find(|x| x.id == self.ui.ribbon_tab).cloned() else { return };
        let body = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + TABS_H), rect.right_bottom());
        self.ribbon_body(ui, ctx, body, &tab, false);
    }

    fn ribbon_flyout_body(&mut self, ctx: &egui::Context, rect: Rect, tab: Tab) {
        let t = self.t;
        let body = Rect::from_min_size(Pos2::new(rect.left(), rect.top() + TABS_H), vec2(rect.width(), BODY_H + 4.0));
        let mut close = false;
        egui::Area::new(egui::Id::new("ribbon-flyout")).order(egui::Order::Foreground).fixed_pos(body.min).show(ctx, |ui| {
            egui::Frame::new().fill(t.ribbon).stroke(Stroke::new(1.0, t.border)).shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(body.width(), BODY_H + 4.0), Sense::hover());
                let rect = r;
                self.ribbon_body(ui, ctx, rect, &tab, true);
            });
        });
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                if p.y > body.bottom() + 8.0 {
                    close = true;
                }
            }
        }
        if close {
            self.ui.flyout = None;
        }
    }

    fn ribbon_tab_menu(&mut self, ctx: &egui::Context, r: Rect) {
        let hidden = self.ui.ribbon_hidden.clone();
        let tabs = self.rib.tabs.clone();
        let active = self.ui.ribbon_tab.clone();
        let items = vec![
            MItem::item("Show Tabs").sub(
                tabs.iter()
                    .map(|t| {
                        let id = t.id.clone();
                        let act = active.clone();
                        MItem::item(&t.title).checked(!hidden.contains(&t.id)).on(move |app| {
                            if !app.ui.ribbon_hidden.remove(&id) && id != act {
                                app.ui.ribbon_hidden.insert(id.clone());
                            }
                        })
                    })
                    .collect(),
            ),
            MItem::item("Show Panels").sub(tabs.iter().find(|t| t.id == active).map(|t| t.panels.iter().map(|p| MItem::item(&p.title).checked(true).disabled(true)).collect()).unwrap_or_default()),
            MItem::Sep,
            MItem::item("Minimize Ribbon").checked(self.ui.ribbon_mode != 0).on(|app| app.ui.ribbon_mode = if app.ui.ribbon_mode == 0 { 1 } else { 0 }),
        ];
        self.dropdown(ctx, r, items, 190.0);
    }

    // ------------------------------------------------------------ measuring

    fn item_size(&self, p: &Painter, it: &Item) -> Sz {
        match it {
            Item::Btn { cmd, size, label, icon: _, menu, .. } => {
                let Some(def) = self.h.borrow().registry.get(cmd) else { return Sz::default() };
                let lab = plain_label(label.as_deref(), &def.title);
                match size.as_str() {
                    "large" => {
                        let w = lab.lines().map(|l| p.layout_no_wrap(l.to_string(), FontId::proportional(11.5), Color32::WHITE).size().x).fold(0.0, f32::max) + 10.0;
                        Sz { w: w.max(if menu.is_some() { 54.0 } else { 52.0 }), h: LARGE_H }
                    }
                    "icon" => Sz { w: 24.0, h: SMALL_H },
                    _ => {
                        let w = 6.0 + 16.0 + 6.0 + p.layout_no_wrap(plain(&lab), FontId::proportional(12.0), Color32::WHITE).size().x + 6.0 + if menu.is_some() { 14.0 } else { 0.0 };
                        Sz { w, h: SMALL_H }
                    }
                }
            }
            Item::Stack { items } | Item::Col { items } => {
                let sizes: Vec<Sz> = items.iter().map(|i| self.item_size(p, i)).collect();
                Sz { w: sizes.iter().map(|s| s.w).fold(0.0, f32::max), h: sizes.iter().map(|s| s.h).sum::<f32>() + (sizes.len().saturating_sub(1)) as f32 }
            }
            Item::Row { items } => {
                let sizes: Vec<Sz> = items.iter().map(|i| self.item_size(p, i)).collect();
                Sz { w: sizes.iter().map(|s| s.w).sum(), h: sizes.iter().map(|s| s.h).fold(0.0, f32::max) }
            }
            Item::Combo { kind } => Sz { w: if kind == "visualstyle" { 160.0 } else { 190.0 }, h: SMALL_H },
            Item::HatchPick => Sz { w: 58.0, h: LARGE_H },
            Item::Patterns => Sz { w: 7.0 * 60.0 + 6.0 * 3.0, h: 72.0 },
            Item::HatchProps => Sz { w: 190.0, h: 72.0 },
            Item::HatchClose => Sz { w: 70.0, h: LARGE_H },
        }
    }

    fn panel_width(&self, p: &Painter, panel: &Panel) -> f32 {
        let w: f32 = panel.items.iter().map(|i| self.item_size(p, i).w).sum::<f32>() + 2.0 * panel.items.len().saturating_sub(1) as f32 + 4.0;
        let tw = p.layout_no_wrap(crate::i18n::tr(&panel.title).to_string(), FontId::proportional(11.0), Color32::WHITE).size().x + 30.0;
        w.max(tw)
    }

    // ------------------------------------------------------------ drawing

    fn ribbon_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, body: Rect, tab: &Tab, in_flyout: bool) {
        let t = self.t;
        let p = ui.painter().clone();
        let mut x = body.left() + 4.0;
        let top = body.top() + 3.0;
        let titles_only = self.ui.ribbon_mode == 2 && !in_flyout;
        // which panels collapse? (lowest priority first among panels after the second)
        let widths: Vec<f32> = tab.panels.iter().map(|pl| self.panel_width(&p, pl) + 8.0).collect();
        // a collapsed panel is a button wide enough for its (translated) title
        let narrow: Vec<f32> = tab.panels.iter().map(|pl| (p.layout_no_wrap(crate::i18n::tr(&pl.title).to_string(), FontId::proportional(11.5), Color32::WHITE).size().x + 12.0).max(58.0)).collect();
        let mut collapsed = vec![false; tab.panels.len()];
        if !titles_only {
            let avail = body.width() - 8.0;
            let mut total: f32 = widths.iter().sum();
            let mut order: Vec<usize> = (2..tab.panels.len()).collect();
            order.sort_by_key(|i| std::cmp::Reverse(tab.panels[*i].prio.unwrap_or(*i as i32)));
            for i in order {
                if total <= avail {
                    break;
                }
                collapsed[i] = true;
                total -= widths[i] - narrow[i] - 4.0;
            }
        }
        for (i, panel) in tab.panels.iter().enumerate() {
            let pw = if collapsed[i] { narrow[i] } else { widths[i] - 8.0 };
            let prect = Rect::from_min_size(Pos2::new(x, top), vec2(pw, body.bottom() - top - 1.0));
            if collapsed[i] {
                self.collapsed_panel(ui, ctx, prect, panel);
            } else {
                self.panel(ui, ctx, prect, panel, titles_only);
            }
            x += pw + 2.0;
            // separator
            p.line_segment([Pos2::new(x + 1.0, top + 2.0), Pos2::new(x + 1.0, body.bottom() - 6.0)], Stroke::new(1.0, t.sep));
            x += 4.0;
        }
        let _ = Painter::clone;
    }

    fn collapsed_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, r: Rect, panel: &Panel) {
        let t = self.t;
        let p = ui.painter().clone();
        let btn = Rect::from_min_size(r.min, vec2(r.width(), LARGE_H));
        let resp = ui.interact(btn, ui.id().with(("collapsed", &panel.title)), Sense::click());
        if resp.hovered() {
            p.rect_filled(btn, CornerRadius::same(3), t.hover);
            p.rect_stroke(btn, CornerRadius::same(3), Stroke::new(1.0, t.sep), StrokeKind::Inside);
        }
        let icon = self.first_icon(&panel.items);
        self.icons.paint(&p, &icon, Rect::from_center_size(Pos2::new(btn.center().x, btn.top() + 22.0), vec2(32.0, 32.0)), &t, Tint::Normal);
        p.text(Pos2::new(btn.center().x, btn.top() + 46.0), Align2::CENTER_CENTER, crate::i18n::tr(&panel.title), FontId::proportional(11.5), t.text);
        chevron(&p, Pos2::new(btn.center().x, btn.top() + 60.0), 3.5, false, t.text2);
        if resp.clicked() {
            self.ui.flyout_panel = if self.ui.flyout_panel.as_ref().map(|x| x.0.as_str()) == Some(panel.title.as_str()) { None } else { Some((panel.title.clone(), btn)) };
        }
        if self.ui.flyout_panel.as_ref().map(|x| x.0.as_str()) == Some(panel.title.as_str()) {
            let panel = panel.clone();
            let mut close = false;
            let w = self.panel_width(&p, &panel) + 10.0;
            egui::Area::new(egui::Id::new("collapsed-flyout")).order(egui::Order::Foreground).fixed_pos(btn.left_bottom() + vec2(0.0, 2.0)).show(ctx, |ui| {
                egui::Frame::new().fill(t.ribbon).stroke(Stroke::new(1.0, t.border)).shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
                    let (rr, _) = ui.allocate_exact_size(vec2(w, LARGE_H + PANEL_TITLE_H + 8.0), Sense::hover());
                    let inner = Rect::from_min_size(rr.min + vec2(4.0, 3.0), vec2(w - 8.0, rr.height() - 4.0));
                    self.panel(ui, ctx, inner, &panel, false);
                });
            });
            if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(pp) = ctx.input(|i| i.pointer.interact_pos()) {
                    let area = Rect::from_min_size(btn.left_bottom(), vec2(w, LARGE_H + 30.0));
                    if !area.contains(pp) && !btn.contains(pp) {
                        close = true;
                    }
                }
            }
            if close || self.popup.is_some() {
                // keep open while a popup of one of its buttons is shown
                if close {
                    self.ui.flyout_panel = None;
                }
            }
        }
    }

    fn first_icon(&self, items: &[Item]) -> String {
        for it in items {
            match it {
                Item::Btn { cmd, icon, .. } => {
                    if let Some(d) = self.h.borrow().registry.get(cmd) {
                        return icon.clone().unwrap_or_else(|| if d.icon.is_empty() { d.name.to_lowercase() } else { d.icon.clone() });
                    }
                }
                Item::Stack { items } | Item::Col { items } | Item::Row { items } => {
                    let r = self.first_icon(items);
                    if r != "properties" {
                        return r;
                    }
                }
                _ => {}
            }
        }
        "properties".into()
    }

    fn panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, r: Rect, panel: &Panel, titles_only: bool) {
        let t = self.t;
        let p = ui.painter().clone();
        let title_rect = Rect::from_min_size(Pos2::new(r.left(), r.bottom() - PANEL_TITLE_H), vec2(r.width(), PANEL_TITLE_H));
        let has_slide = !panel.slideout.is_empty();
        let tresp = ui.interact(title_rect, ui.id().with(("ptitle", &panel.title)), if has_slide { Sense::click() } else { Sense::hover() });
        let open = self.ui.slideout.as_deref() == Some(panel.title.as_str());
        if has_slide && (tresp.hovered() || open) {
            p.rect_filled(title_rect, 0.0, t.ribbon_title);
        }
        let tcolor = if has_slide && (tresp.hovered() || open) { t.text } else { t.text3 };
        let g = p.layout_no_wrap(crate::i18n::tr(&panel.title).to_string(), FontId::proportional(11.0), tcolor);
        let tw = g.size().x;
        p.galley(Pos2::new(title_rect.center().x - tw / 2.0 - if has_slide { 6.0 } else { 0.0 }, title_rect.center().y - g.size().y / 2.0), g, tcolor);
        if has_slide {
            chevron(&p, Pos2::new(title_rect.center().x + tw / 2.0 + 4.0, title_rect.center().y), 3.0, false, tcolor);
            if tresp.clicked() {
                self.ui.slideout = if open { None } else { Some(panel.title.clone()) };
            }
        }
        if let Some(l) = &panel.launcher {
            if self.h.borrow().registry.has_command(l) {
                let lr = Rect::from_min_size(Pos2::new(title_rect.right() - 14.0, title_rect.top() + 2.0), vec2(13.0, 13.0));
                let lresp = ui.interact(lr, ui.id().with(("launch", &panel.title)), Sense::click());
                self.icons.paint(&p, "launcher", lr, &t, Tint::Mono(if lresp.hovered() { t.text } else { t.text3 }));
                let cmd = l.clone();
                if lresp.on_hover_text(crate::i18n::tr(&panel.title)).clicked() {
                    self.run_command(&cmd, Args::none());
                }
            }
        }
        if titles_only {
            return;
        }
        // items
        let mut x = r.left() + 2.0;
        for it in &panel.items {
            let sz = self.item_size(&p, it);
            if sz.w <= 0.0 {
                continue;
            }
            let ir = Rect::from_min_size(Pos2::new(x, r.top()), vec2(sz.w, sz.h));
            self.draw_item(ui, ctx, it, ir);
            x += sz.w + 2.0;
        }
        // slide-out
        if open {
            let items = panel.slideout.clone();
            let mut close = false;
            let widths: f32 = items.iter().map(|i| self.item_size(&p, i).w).fold(r.width(), f32::max) + 12.0;
            let n = items.len() as f32;
            let ar = Area::new_slide(title_rect.left_bottom(), widths);
            egui::Area::new(egui::Id::new(("slideout", &panel.title))).order(egui::Order::Foreground).fixed_pos(ar.0).show(ctx, |ui| {
                egui::Frame::new().fill(t.ribbon).stroke(Stroke::new(1.0, t.border)).shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
                    let (rr, _) = ui.allocate_exact_size(vec2(ar.1, n * (SMALL_H + 1.0) + 8.0), Sense::hover());
                    let mut y = rr.top() + 4.0;
                    for it in &items {
                        let sz = self.item_size(ui.painter(), it);
                        let ir = Rect::from_min_size(Pos2::new(rr.left() + 6.0, y), vec2(rr.width() - 12.0, sz.h));
                        self.draw_item(ui, ctx, it, ir);
                        y += sz.h + 1.0;
                    }
                });
            });
            if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(pp) = ctx.input(|i| i.pointer.interact_pos()) {
                    let area = Rect::from_min_size(title_rect.left_bottom(), vec2(ar.1, n * (SMALL_H + 1.0) + 10.0));
                    if !area.contains(pp) && !title_rect.contains(pp) {
                        close = true;
                    }
                }
            }
            if close {
                self.ui.slideout = None;
            }
        }
    }

    fn draw_item(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, it: &Item, r: Rect) {
        match it {
            Item::Btn { cmd, size, label, icon, args, menu } => self.ribbon_button(ui, ctx, r, cmd, size, label.as_deref(), icon.as_deref(), args.as_ref(), menu.as_ref()),
            Item::Stack { items } | Item::Col { items } => {
                let mut y = r.top();
                for c in items {
                    let sz = self.item_size(ui.painter(), c);
                    let cr = Rect::from_min_size(Pos2::new(r.left(), y), vec2(if matches!(it, Item::Stack { .. }) { r.width() } else { sz.w }, sz.h));
                    self.draw_item(ui, ctx, c, cr);
                    y += sz.h + 1.0;
                }
            }
            Item::Row { items } => {
                let mut x = r.left();
                for c in items {
                    let sz = self.item_size(ui.painter(), c);
                    self.draw_item(ui, ctx, c, Rect::from_min_size(Pos2::new(x, r.top()), vec2(sz.w, sz.h)));
                    x += sz.w;
                }
            }
            Item::Combo { kind } => self.combo(ui, ctx, r, kind),
            Item::HatchPick => self.hatch_pick(ui, r),
            Item::Patterns => self.pattern_gallery(ui, r),
            Item::HatchProps => self.hatch_props(ui, ctx, r),
            Item::HatchClose => self.hatch_close(ui, r),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn ribbon_button(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, r: Rect, cmd: &str, size: &str, label: Option<&str>, icon: Option<&str>, args: Option<&serde_json::Value>, menu: Option<&Menu>) {
        let t = self.t;
        let (title, def_icon, exists) = {
            let a = self.h.borrow();
            match a.registry.get(cmd) {
                Some(d) => (d.title.clone(), if d.icon.is_empty() { d.name.to_lowercase() } else { d.icon.clone() }, true),
                None => (cmd.to_string(), String::new(), false),
            }
        };
        if !exists {
            return;
        }
        let icon = icon.map(str::to_string).unwrap_or(def_icon);
        let lab = plain_label(label, &title);
        let p = ui.painter().clone();
        // is this the running command (with the same arguments)?
        let running = {
            let a = self.h.borrow();
            a.top_task().and_then(|id| a.rt.tasks.get(&id)).map(|ts| ts.def.name == cmd.to_uppercase()).unwrap_or(false)
        };
        let id_salt = (cmd, size, label, args.map(|a| a.to_string()), r.min.x as i32, r.min.y as i32);
        let large = size == "large";
        let has_menu = menu.is_some();
        let face = if large && has_menu { Rect::from_min_size(r.min, vec2(r.width(), 56.0)) } else if has_menu { Rect::from_min_max(r.min, Pos2::new(r.right() - 14.0, r.bottom())) } else { r };
        let resp = ui.interact(face, ui.id().with(("rb", &id_salt)), Sense::click());
        let down = resp.is_pointer_button_down_on();
        let bg = if down || running { Some((t.press, t.accent)) } else if resp.hovered() { Some((t.hover, t.sep)) } else { None };
        let corner = if large && has_menu { CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 } } else if has_menu { CornerRadius { nw: 3, ne: 0, sw: 3, se: 0 } } else { CornerRadius::same(3) };
        if let Some((fill, line)) = bg {
            p.rect_filled(face, corner, fill);
            p.rect_stroke(face, corner, Stroke::new(1.0, line), StrokeKind::Inside);
        }
        match size {
            "large" => {
                self.icons.paint(&p, &icon, Rect::from_center_size(Pos2::new(face.center().x, face.top() + 22.0), vec2(32.0, 32.0)), &t, Tint::Normal);
                let lines: Vec<&str> = lab.lines().collect();
                let mut y = face.top() + 41.0;
                for l in &lines {
                    p.text(Pos2::new(face.center().x, y), Align2::CENTER_CENTER, l, FontId::proportional(11.5), t.text);
                    y += 12.5;
                }
                if has_menu && lines.len() == 1 {
                    // the caret sits on the second line in web; here it lives in the arrow below the face
                }
            }
            "icon" => {
                self.icons.paint(&p, &icon, Rect::from_center_size(face.center(), vec2(16.0, 16.0)), &t, Tint::Normal);
            }
            _ => {
                self.icons.paint(&p, &icon, Rect::from_center_size(Pos2::new(face.left() + 14.0, face.center().y), vec2(16.0, 16.0)), &t, Tint::Normal);
                p.text(Pos2::new(face.left() + 28.0, face.center().y), Align2::LEFT_CENTER, plain(&lab), FontId::proportional(12.0), t.text);
            }
        }
        let resp = self.cmd_tip(resp, cmd, Some(&lab));
        if resp.clicked() {
            self.ui.slideout = None;
            self.ui.flyout_panel = None;
            self.run_command(cmd, args_from_json(args));
        }
        if resp.secondary_clicked() {
            let (c, ic, a) = (cmd.to_string(), icon.clone(), args.cloned());
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                self.open_popup(ctx, crate::menu::Popup::new(pos, vec![MItem::item("Add to Quick Access Toolbar").on(move |app| app.qat_add(&c, a.clone(), &ic))]));
            }
        }
        // split arrow
        if let Some(m) = menu {
            let ar = if large { Rect::from_min_max(Pos2::new(r.left(), r.top() + 56.0), r.right_bottom()) } else { Rect::from_min_max(Pos2::new(r.right() - 14.0, r.top()), r.right_bottom()) };
            let aresp = ui.interact(ar, ui.id().with(("rb-arrow", &id_salt)), Sense::click());
            let corner = if large { CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 } } else { CornerRadius { nw: 0, ne: 3, sw: 0, se: 3 } };
            if aresp.hovered() {
                p.rect_filled(ar, corner, t.hover);
                p.rect_stroke(ar, corner, Stroke::new(1.0, t.sep), StrokeKind::Inside);
            }
            chevron(&p, ar.center(), 3.5, false, if aresp.hovered() { t.text } else { t.text2 });
            if aresp.clicked() {
                let items = self.menu_items(m);
                self.dropdown(ctx, if large { Rect::from_min_max(r.left_top(), r.right_bottom()) } else { r }, items, 190.0);
            }
        }
    }

    fn menu_items(&self, m: &Menu) -> Vec<MItem> {
        match m {
            Menu::Named(_) => {
                let names: Vec<String> = self.h.borrow().doc().map(|d| d.blocks.keys().cloned().collect()).unwrap_or_default();
                let mut v = vec![MItem::item("Insert Block...").icon("insert").on(|a| a.run_command("INSERT", Args::none())), MItem::Sep];
                if names.is_empty() {
                    v.push(MItem::item("No blocks defined").disabled(true));
                } else {
                    for n in names {
                        let n2 = n.clone();
                        v.push(MItem::data(&n).icon("block").on(move |a| a.run_command("INSERT", Args::none().with("name", cadforge_engine::runtime::ArgVal::Str(n2.clone())))));
                    }
                }
                v
            }
            Menu::Items(list) => list.iter().filter_map(|e| self.menu_entry(e)).collect(),
        }
    }

    fn menu_entry(&self, e: &MenuEntry) -> Option<MItem> {
        if e.sep {
            return Some(MItem::Sep);
        }
        let a = self.h.borrow();
        let def = a.registry.get(&e.cmd)?;
        let label = e.label.clone().unwrap_or_else(|| def.title.clone());
        let icon = e.icon.clone().unwrap_or_else(|| if def.icon.is_empty() { def.name.to_lowercase() } else { def.icon.clone() });
        let (cmd, args) = (e.cmd.clone(), e.args.clone());
        Some(MItem::item(&label).icon(&icon).on(move |app| app.run_command(&cmd, args_from_json(args.as_ref()))))
    }

    // ------------------------------------------------------------ combos

    fn combo(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, r: Rect, kind: &str) {
        let t = self.t;
        let p = ui.painter().clone();
        let resp = ui.interact(r, ui.id().with(("combo", kind)), Sense::click());
        p.rect_filled(r, CornerRadius::same(3), t.input);
        p.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, if resp.hovered() { t.accent } else { t.border }), StrokeKind::Inside);
        let mut x = r.left() + 6.0;
        let cy = r.center().y;
        let props = self.current_props();
        let font = FontId::proportional(12.0);
        match kind {
            "layer" => {
                if let Some(pr) = &props {
                    if let Some(l) = &pr.layer_obj {
                        for (icon, col) in [(if l.on { "eye" } else { "eyeoff" }, t.ic_l), (if l.frozen { "snow" } else { "sun" }, t.ic_l), (if l.locked { "lock" } else { "unlock" }, t.ic_l)] {
                            self.icons.paint(&p, icon, Rect::from_center_size(Pos2::new(x + 7.0, cy), vec2(14.0, 14.0)), &t, Tint::Normal);
                            let _ = col;
                            x += 16.0;
                        }
                        let (rgb, _) = layer_swatch(l);
                        swatch(&p, Rect::from_center_size(Pos2::new(x + 6.0, cy), vec2(12.0, 12.0)), rgb, &t);
                        x += 18.0;
                    }
                    let name = match &pr.layer {
                        Varies::Val(n) => n.clone(),
                        Varies::Varies => "*VARIES*".into(),
                    };
                    p.with_clip_rect(Rect::from_min_max(r.min, r.right_bottom() - vec2(18.0, 0.0))).text(Pos2::new(x, cy), Align2::LEFT_CENTER, name, font.clone(), t.text);
                }
            }
            "color" => {
                if let Some(pr) = &props {
                    let (text, rgb) = match &pr.color {
                        Varies::Val(c) => (describe_color(*c), color_rgb_of(*c, &pr.layer_obj, t.dark)),
                        Varies::Varies => ("*VARIES*".to_string(), None),
                    };
                    swatch(&p, Rect::from_center_size(Pos2::new(x + 6.0, cy), vec2(12.0, 12.0)), rgb, &t);
                    p.text(Pos2::new(x + 18.0, cy), Align2::LEFT_CENTER, text, font.clone(), t.text);
                }
            }
            "lweight" => {
                if let Some(pr) = &props {
                    let text = match &pr.lw {
                        Varies::Val(v) => cadforge_core::style::lw_display(*v),
                        Varies::Varies => "*VARIES*".into(),
                    };
                    let h = (pr.lw_resolved as f32 / 100.0 * 1.6).max(1.0);
                    p.rect_filled(Rect::from_min_size(Pos2::new(x, cy - h / 2.0), vec2(34.0, h)), 1.0, t.text);
                    p.text(Pos2::new(x + 42.0, cy), Align2::LEFT_CENTER, text, font.clone(), t.text);
                }
            }
            "ltype" => {
                if let Some(pr) = &props {
                    let text = match &pr.ltype {
                        Varies::Val(Some(l)) => l.clone(),
                        Varies::Val(None) => "ByLayer".into(),
                        Varies::Varies => "*VARIES*".into(),
                    };
                    lt_sample(&p, Pos2::new(x, cy), &pr.ltype_resolved, 44.0, t.text);
                    p.text(Pos2::new(x + 52.0, cy), Align2::LEFT_CENTER, text, font.clone(), t.text);
                }
            }
            _ => {
                let style = self.h.borrow().editor().map(|e| e.visual_style.clone()).unwrap_or_default();
                p.text(Pos2::new(x, cy), Align2::LEFT_CENTER, style, font.clone(), t.text);
            }
        }
        chevron(&p, Pos2::new(r.right() - 11.0, cy), 3.5, false, t.text2);
        if resp.clicked() {
            self.combo_menu(ctx, kind, r);
        }
    }

    fn combo_menu(&mut self, ctx: &egui::Context, kind: &str, r: Rect) {
        match kind {
            "layer" => {
                self.ui.layer_dd = Some(r);
                self.ui.layer_dd_frame = ctx.cumulative_frame_nr();
            }
            "color" => {
                let mut items = vec![MItem::item("ByLayer").on(|a| a.set_property("color", None, None, None)), MItem::item("ByBlock").on(|a| a.set_property("color", Some(Color::ByBlock), None, None)), MItem::Sep];
                for (n, i) in [("Red", 1u8), ("Yellow", 2), ("Green", 3), ("Cyan", 4), ("Blue", 5), ("Magenta", 6), ("White", 7)] {
                    let [rr, gg, bb] = parse_hex(aci_hex(i));
                    items.push(MItem::item(n).swatch(Color32::from_rgb(rr, gg, bb)).on(move |a| a.set_property("color", Some(Color::Aci(i)), None, None)));
                }
                items.push(MItem::Sep);
                items.push(MItem::item("Select Color...").on(|a| a.dialog = Some(crate::dialogs::Dialog::color_for_property())));
                self.dropdown(ctx, r, items, 160.0);
            }
            "lweight" => {
                let mut items = vec![MItem::item("ByLayer").on(|a| a.set_property("lw", None, None, None)), MItem::item("ByBlock").on(|a| a.set_property("lw", None, Some(-2), None)), MItem::item("Default").on(|a| a.set_property("lw", None, Some(-3), None)), MItem::Sep];
                for w in LINEWEIGHTS {
                    items.push(MItem::item(&format!("{:.2} mm", w as f64 / 100.0)).on(move |a| a.set_property("lw", None, Some(w), None)));
                }
                self.dropdown(ctx, r, items, 150.0);
            }
            "ltype" => {
                let mut items = vec![MItem::item("ByLayer").on(|a| a.set_property("ltype", None, None, None)), MItem::item("ByBlock").on(|a| a.set_property("ltype", None, None, Some("BYBLOCK".into()))), MItem::Sep];
                for l in all_linetypes().iter() {
                    let n = l.name.to_string();
                    let disp = cadforge_core::style::ltype_display(l.name);
                    items.push(MItem::data(&disp).on(move |a| a.set_property("ltype", None, None, Some(n.clone()))));
                }
                items.push(MItem::Sep);
                items.push(MItem::item("Other...").on(|a| a.run_command("LINETYPE", Args::none())));
                self.dropdown(ctx, r, items, 190.0);
            }
            _ => {
                let cur = self.h.borrow().editor().map(|e| e.visual_style.clone()).unwrap_or_default();
                let items = ["2D Wireframe", "Wireframe", "Hidden", "Shaded", "Shaded with Edges"]
                    .iter()
                    .map(|n| {
                        let n = n.to_string();
                        let n2 = n.clone();
                        MItem::item(&n).checked(cur == n).on(move |a| a.set_visual_style(&n2))
                    })
                    .collect();
                self.dropdown(ctx, r, items, 160.0);
            }
        }
    }

    /// The layer drop-down (rows with visibility / freeze / lock toggles) stays open while toggling.
    pub fn layer_dropdown(&mut self, ctx: &egui::Context) {
        let Some(anchor) = self.ui.layer_dd else { return };
        let t = self.t;
        let (layers, cur) = {
            let a = self.h.borrow();
            let Some(d) = a.doc() else { return };
            let mut l = d.layers.clone();
            l.sort_by(|a, b| natord(&a.name, &b.name));
            (l, d.cur.layer.clone())
        };
        let mut choose: Option<String> = None;
        let mut toggle: Option<(String, u8)> = None;
        let w = anchor.width().max(230.0);
        let resp = egui::Area::new(egui::Id::new("layer-dd")).order(egui::Order::Foreground).fixed_pos(anchor.left_bottom() + vec2(0.0, 1.0)).show(ctx, |ui| {
            egui::Frame::new().fill(t.raised).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(3)).inner_margin(egui::Margin::symmetric(0, 4)).shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
                ui.set_width(w);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for l in &layers {
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r.shrink2(vec2(3.0, 0.0)), CornerRadius::same(2), t.accent_soft);
                        }
                        let mut x = r.left() + 8.0;
                        let mut hit_state = false;
                        for (k, (icon, state)) in [(if l.on { "eye" } else { "eyeoff" }, l.on), (if l.frozen { "snow" } else { "sun" }, l.frozen), (if l.locked { "lock" } else { "unlock" }, l.locked)].into_iter().enumerate() {
                            let ir = Rect::from_center_size(Pos2::new(x + 7.0, r.center().y), vec2(18.0, 18.0));
                            let iresp = ui.interact(ir, ui.id().with(("ldd", &l.name, k)), Sense::click());
                            if iresp.hovered() {
                                ui.painter().rect_filled(ir, CornerRadius::same(2), t.hover);
                            }
                            let _ = state;
                            self.icons.paint(ui.painter(), icon, ir.shrink(2.0), &t, Tint::Normal);
                            if iresp.clicked() {
                                toggle = Some((l.name.clone(), k as u8));
                                hit_state = true;
                            }
                            x += 20.0;
                        }
                        let (rgb, _) = layer_swatch(l);
                        swatch(ui.painter(), Rect::from_center_size(Pos2::new(x + 6.0, r.center().y), vec2(12.0, 12.0)), rgb, &t);
                        x += 18.0;
                        let font = if l.name == cur { FontId::new(12.0, FontFamily::Name("bold".into())) } else { FontId::proportional(12.0) };
                        ui.painter().text(Pos2::new(x, r.center().y), Align2::LEFT_CENTER, &l.name, font, t.text);
                        if resp.clicked() && !hit_state {
                            choose = Some(l.name.clone());
                        }
                    }
                });
            });
        });
        let rect = resp.response.rect;
        let mut close = false;
        if ctx.input(|i| i.pointer.any_pressed()) && ctx.cumulative_frame_nr() > self.ui.layer_dd_frame + 1 {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                if !rect.contains(p) && !anchor.contains(p) {
                    close = true;
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if let Some((name, k)) = toggle {
            self.layer_toggle(&name, k);
        }
        if let Some(n) = choose {
            self.choose_layer(&n);
            close = true;
        }
        if close {
            self.ui.layer_dd = None;
        }
    }

    pub fn choose_layer(&mut self, name: &str) {
        let (frozen, nosel) = {
            let a = self.h.borrow();
            let Some(ed) = a.editor() else { return };
            (ed.doc.layer(name).is_some_and(|l| l.frozen), ed.sel.is_empty())
        };
        if nosel && frozen {
            self.toast("Cannot make a frozen layer current.");
            return;
        }
        self.set_property("layer", None, None, Some(name.to_string()));
    }

    pub fn layer_toggle(&mut self, name: &str, key: u8) {
        let cur = self.h.borrow().doc().map(|d| d.cur.layer.clone()).unwrap_or_default();
        let (on, frozen) = {
            let a = self.h.borrow();
            a.doc().and_then(|d| d.layer(name)).map_or((true, false), |l| (l.on, l.frozen))
        };
        if key == 1 && !frozen && name == cur {
            self.toast("Cannot freeze the current layer.");
            return;
        }
        if key == 0 && on && name == cur {
            self.toast("The current layer is turned off.");
        }
        let n = name.to_string();
        self.edit_doc("Layer", move |d| {
            d.will_change();
            if let Some(l) = d.layer_mut(&n) {
                match key {
                    0 => l.on = !l.on,
                    1 => l.frozen = !l.frozen,
                    _ => l.locked = !l.locked,
                }
            }
            d.touch();
        });
    }

    pub fn qat_add(&mut self, cmd: &str, args: Option<serde_json::Value>, icon: &str) {
        let key = (cmd.to_string(), args.clone(), icon.to_string());
        if !self.ui.qat_custom.iter().any(|q| q.0 == key.0 && q.1 == key.1) {
            self.ui.qat_custom.push(key);
        }
    }
}

struct Area;
impl Area {
    fn new_slide(pos: Pos2, w: f32) -> (Pos2, f32) {
        (pos, w)
    }
}

/// A button's label in the interface language: its own, or its command's title.
fn plain_label(label: Option<&str>, title: &str) -> String {
    crate::i18n::tr(label.unwrap_or(title)).to_string()
}

fn natord(a: &str, b: &str) -> std::cmp::Ordering {
    // numeric-aware, case-insensitive compare
    let (la, lb) = (a.to_lowercase(), b.to_lowercase());
    let mut ia = la.chars().peekable();
    let mut ib = lb.chars().peekable();
    loop {
        match (ia.peek().copied(), ib.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = ia.peek().copied().filter(|c| c.is_ascii_digit()) {
                    na.push(c);
                    ia.next();
                }
                let mut nb = String::new();
                while let Some(c) = ib.peek().copied().filter(|c| c.is_ascii_digit()) {
                    nb.push(c);
                    ib.next();
                }
                let (va, vb): (u128, u128) = (na.parse().unwrap_or(0), nb.parse().unwrap_or(0));
                if va != vb {
                    return va.cmp(&vb);
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                ia.next();
                ib.next();
            }
        }
    }
}

pub fn layer_swatch(l: &cadforge_core::style::Layer) -> (Option<[u8; 3]>, bool) {
    (Some(cadforge_core::style::color_rgb(l.color, true, false)), false)
}

pub fn color_rgb_of(c: Option<Color>, layer: &Option<cadforge_core::style::Layer>, dark: bool) -> Option<[u8; 3]> {
    match c {
        None | Some(Color::ByLayer) => layer.as_ref().map(|l| cadforge_core::style::color_rgb(l.color, dark, !dark)),
        Some(Color::ByBlock) => None,
        Some(c) => Some(cadforge_core::style::color_rgb(c, dark, !dark)),
    }
}

pub fn swatch(p: &Painter, r: Rect, rgb: Option<[u8; 3]>, t: &crate::theme::Tokens) {
    match rgb {
        Some(c) => {
            p.rect_filled(r, CornerRadius::same(2), Color32::from_rgb(c[0], c[1], c[2]));
        }
        None => {
            // ByBlock / unknown: grey diagonal split
            p.rect_filled(r, CornerRadius::same(2), Color32::from_gray(0x88));
            p.add(Shape::convex_polygon(vec![r.right_top(), r.right_bottom(), r.left_bottom()], Color32::from_gray(0xbb), Stroke::NONE));
        }
    }
    p.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0, if t.dark { Color32::from_white_alpha(115) } else { Color32::from_black_alpha(90) }), StrokeKind::Inside);
}

pub fn lt_sample(p: &Painter, at: Pos2, name: &str, w: f32, color: Color32) {
    let pat = linetype(name).map(|l| l.pattern).unwrap_or(&[]);
    let a = at;
    let b = at + vec2(w, 0.0);
    if pat.is_empty() {
        p.line_segment([a, b], Stroke::new(1.4, color));
    } else {
        let dash: Vec<f32> = pat.iter().map(|v| (v.abs() * 8.0).max(0.1) as f32).collect();
        for seg in crate::gfx::dash_polyline(&[a, b], false, &dash) {
            p.add(Shape::line(seg, Stroke::new(1.4, color)));
        }
    }
}

// ---------------------------------------------------------------- hatch creation widgets

impl CadForgeApp {
    fn hatch_pick(&mut self, ui: &mut egui::Ui, r: Rect) {
        let t = self.t;
        let resp = ui.interact(r, ui.id().with("hatchpick"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
        }
        self.icons.paint(ui.painter(), "hatch", Rect::from_center_size(Pos2::new(r.center().x, r.top() + 22.0), vec2(32.0, 32.0)), &t, Tint::Normal);
        ui.painter().text(Pos2::new(r.center().x, r.top() + 46.0), Align2::CENTER_CENTER, "Select", FontId::proportional(11.5), t.text);
        if resp.clicked() {
            self.h.keyword("Select objects");
        }
    }

    fn pattern_gallery(&mut self, ui: &mut egui::Ui, r: Rect) {
        let t = self.t;
        let cur = self.h.borrow().hatch_session.as_ref().map(|s| s.settings.borrow().pattern.clone());
        let names: Vec<(&'static str, &'static str)> = cadforge_core::hatch::all_patterns().iter().map(|p| (p.name, p.desc)).collect();
        let mut set: Option<&'static str> = None;
        for (i, (name, desc)) in names.iter().enumerate() {
            let (col, row) = (i % 7, i / 7);
            let cr = Rect::from_min_size(r.min + vec2(col as f32 * 63.0, row as f32 * 34.0), vec2(60.0, 31.0));
            let resp = ui.interact(cr, ui.id().with(("pat", *name)), Sense::click());
            let sel = cur.as_deref() == Some(*name);
            ui.painter().rect_filled(cr, CornerRadius::same(3), Color32::from_rgb(0x10, 0x15, 0x1a));
            self.paint_pattern_thumb(ui.painter(), cr.shrink(3.0), name);
            ui.painter().rect_stroke(cr, CornerRadius::same(3), Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { t.accent2 } else if resp.hovered() { t.accent } else { t.border }), StrokeKind::Inside);
            let resp = resp.on_hover_text(format!("{name} — {desc}"));
            if resp.clicked() {
                set = Some(name);
            }
        }
        if let Some(n) = set {
            if let Some(s) = self.h.borrow().hatch_session.as_ref() {
                s.settings.borrow_mut().pattern = n.to_string();
            }
            self.refresh_hatch_preview();
        }
    }

    fn paint_pattern_thumb(&self, p: &Painter, r: Rect, name: &str) {
        use cadforge_core::geom::pt;
        let pat = cadforge_core::hatch::pattern(name);
        let col = Color32::from_rgb(0xd9, 0xde, 0xe4);
        let Some(pat) = pat else { return };
        if pat.solid {
            p.rect_filled(r, 0.0, col);
            return;
        }
        let (w, h) = (r.width() as f64, r.height() as f64);
        let sc = 0.6 * if name == "BRICK" { 0.5 } else { 1.0 } / 2.4;
        let lp = vec![vec![pt(0.0, 0.0), pt(w, 0.0), pt(w, h), pt(0.0, h)]];
        if let Some(lines) = cadforge_core::hatch::hatch_lines(&lp, name, sc, 0.0) {
            let mut shapes = vec![];
            for l in lines.iter().take(400) {
                shapes.push(Shape::line_segment([Pos2::new(r.left() + l[0] as f32, r.top() + l[1] as f32), Pos2::new(r.left() + l[2] as f32, r.top() + l[3] as f32)], Stroke::new(1.0, col)));
            }
            p.extend(shapes);
        }
        p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(0x7f, 0x8b, 0x98)), StrokeKind::Inside);
    }

    pub fn refresh_hatch_preview(&mut self) {
        let mut a = self.h.borrow_mut();
        a.refresh_preview(false);
        a.emit(cadforge_engine::app::Event::Change);
    }

    fn hatch_props(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, r: Rect) {
        let t = self.t;
        let Some(sess) = self.h.borrow().hatch_session.clone() else { return };
        let mut changed = false;
        let p = ui.painter().clone();
        // colour
        let row = |i: f32| Rect::from_min_size(r.min + vec2(0.0, i * 24.0), vec2(r.width(), 21.0));
        p.text(Pos2::new(r.left(), row(0.0).center().y), Align2::LEFT_CENTER, "Color", FontId::proportional(12.0), t.text2);
        let cr = Rect::from_min_size(Pos2::new(r.left() + 46.0, row(0.0).top()), vec2(130.0, 21.0));
        let cresp = ui.interact(cr, ui.id().with("hatch-color"), Sense::click());
        p.rect_filled(cr, CornerRadius::same(3), t.input);
        p.rect_stroke(cr, CornerRadius::same(3), Stroke::new(1.0, if cresp.hovered() { t.accent } else { t.border }), StrokeKind::Inside);
        let col = sess.settings.borrow().color;
        swatch(&p, Rect::from_center_size(Pos2::new(cr.left() + 12.0, cr.center().y), vec2(12.0, 12.0)), color_rgb_of(col, &None, t.dark), &t);
        p.text(Pos2::new(cr.left() + 24.0, cr.center().y), Align2::LEFT_CENTER, describe_color(col), FontId::proportional(12.0), t.text);
        if cresp.clicked() {
            self.dialog = Some(crate::dialogs::Dialog::color_for_hatch());
        }
        // scale / angle
        let mut scale = sess.settings.borrow().scale;
        let mut angle = sess.settings.borrow().angle;
        for (i, (label, is_scale)) in [("Scale", true), ("Angle", false)].into_iter().enumerate() {
            let rr = row(i as f32 + 1.0);
            p.text(Pos2::new(r.left(), rr.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0), t.text2);
            let ir = Rect::from_min_size(Pos2::new(r.left() + 46.0, rr.top()), vec2(80.0, 21.0));
            let drag = if is_scale { egui::DragValue::new(&mut scale).speed(0.05).range(0.0001..=1.0e6) } else { egui::DragValue::new(&mut angle).speed(1.0) };
            if ui.put(ir, drag).changed() {
                changed = true;
            }
        }
        if changed {
            {
                let mut s = sess.settings.borrow_mut();
                s.scale = scale;
                s.angle = angle;
            }
            self.refresh_hatch_preview();
        }
        let _ = ctx;
    }

    fn hatch_close(&mut self, ui: &mut egui::Ui, r: Rect) {
        let t = self.t;
        let resp = ui.interact(r, ui.id().with("hatchclose"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
        }
        self.icons.paint(ui.painter(), "check", Rect::from_center_size(Pos2::new(r.center().x, r.top() + 22.0), vec2(32.0, 32.0)), &t, Tint::Mono(t.ok));
        ui.painter().text(Pos2::new(r.center().x, r.top() + 41.0), Align2::CENTER_CENTER, "Close Hatch", FontId::proportional(11.5), t.text);
        ui.painter().text(Pos2::new(r.center().x, r.top() + 53.0), Align2::CENTER_CENTER, "Creation", FontId::proportional(11.5), t.text);
        if resp.clicked() {
            self.h.enter();
        }
    }
}
