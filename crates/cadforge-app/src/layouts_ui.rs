//! Layout tabs (Model / Layout1 / Layout2 / +) under the drawing area and the paper / model space toggle.
use crate::app::CadForgeApp;
use crate::icons::Tint;
use crate::menu::MItem;
use cadforge_core::drawing::MODEL;
use cadforge_engine::app::Event;
use cadforge_engine::runtime::{ArgVal, Args};
use eframe::egui::{self, vec2, Align2, CornerRadius, FontId, Pos2, Rect, Sense, Stroke};

pub const LAYOUT_TABS_H: f32 = 26.0;

impl CadForgeApp {
    /// `MODEL` or `PAPER` for the status bar (a viewport made active counts as model space).
    pub fn space_label(&self) -> String {
        let a = self.h.borrow();
        match a.editor() {
            Some(e) if e.doc.is_model() => "MODEL".into(),
            Some(_) => "PAPER".into(),
            None => "MODEL".into(),
        }
    }

    /// Status bar button: toggle between the sheet and the active viewport.
    pub fn toggle_space(&mut self) {
        let in_mspace = self.h.borrow().editor().is_some_and(|e| e.mspace.is_some());
        self.run_command(if in_mspace { "PSPACE" } else { "MSPACE" }, Args::none());
    }

    fn switch_tab(&mut self, name: &str) {
        if self.h.borrow().busy() {
            self.h.escape();
        }
        let mut a = self.h.borrow_mut();
        if let Some(ed) = a.editor_mut() {
            if let Err(m) = ed.switch_space(name) {
                drop(a);
                self.toast(&m);
                return;
            }
        }
        a.emit(Event::Regen);
        a.emit(Event::Change);
    }

    fn layout_action(&mut self, option: &str, name: Option<&str>) {
        let mut args = Args::none().with("option", ArgVal::Str(option.into()));
        if let Some(n) = name {
            args = args.with("name", ArgVal::Str(n.into()));
        }
        self.run_command("LAYOUT", args);
    }

    pub fn layout_tabs(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, t.border));
        let (tabs, active) = {
            let a = self.h.borrow();
            match a.editor() {
                Some(e) => (e.doc.layout_names(), e.current_tab()),
                None => return,
            }
        };
        let mut x = rect.left() + 6.0;
        let mut clicked: Option<String> = None;
        let mut menu_for: Option<Option<String>> = None;
        let mut entries: Vec<(String, bool)> = vec![("Model".into(), true)];
        entries.extend(tabs.iter().map(|n| (n.clone(), false)));
        for (label, is_model) in &entries {
            let g = p.layout_no_wrap(label.clone(), FontId::proportional(12.0), t.text);
            let w = g.size().x + 26.0;
            let r = Rect::from_min_size(Pos2::new(x, rect.top() + 2.0), vec2(w, LAYOUT_TABS_H - 3.0));
            let resp = ui.interact(r, ui.id().with(("ltab", label.as_str())), Sense::click());
            let is_active = if *is_model { active.eq_ignore_ascii_case(MODEL) } else { active.eq_ignore_ascii_case(label) };
            let fill = if is_active { t.canvas } else if resp.hovered() { t.hover } else { t.chrome };
            p.rect_filled(r, CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, fill);
            p.rect_stroke(r, CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            if is_active {
                p.line_segment([Pos2::new(r.left() + 1.0, r.top()), Pos2::new(r.right() - 1.0, r.top())], Stroke::new(2.0, t.accent2));
            }
            p.galley(Pos2::new(r.left() + 13.0, r.center().y - g.size().y / 2.0), g, if is_active { t.text } else { t.text2 });
            if resp.clicked() {
                clicked = Some(label.clone());
            }
            if resp.secondary_clicked() {
                menu_for = Some(if *is_model { None } else { Some(label.clone()) });
            }
            x += w + 2.0;
        }
        let pr = Rect::from_min_size(Pos2::new(x + 2.0, rect.top() + 2.0), vec2(26.0, LAYOUT_TABS_H - 3.0));
        let presp = ui.interact(pr, ui.id().with("ltab-plus"), Sense::click());
        if presp.hovered() {
            p.rect_filled(pr, CornerRadius::same(3), t.hover);
        }
        self.icons.paint(&p, "plus", Rect::from_center_size(pr.center(), vec2(12.0, 12.0)), &t, Tint::Mono(t.text2));
        let presp = presp.on_hover_text("New layout");
        if presp.clicked() {
            self.layout_action("New", None);
        }
        // hint on the right: current sheet
        if let Some(info) = self.h.borrow().editor().and_then(|e| e.doc.paper_of(&e.current_tab()).map(|pp| format!("{} {} × {} mm", pp.name, pp.w, pp.h))) {
            p.text(Pos2::new(rect.right() - 12.0, rect.center().y), Align2::RIGHT_CENTER, info, FontId::proportional(11.5), t.text3);
        }
        if let Some(name) = clicked {
            self.switch_tab(&name);
        }
        if let Some(target) = menu_for {
            let items = match target {
                Some(n) => {
                    let (n1, n2, n3, n4, n5) = (n.clone(), n.clone(), n.clone(), n.clone(), n.clone());
                    vec![
                        MItem::item("New Layout").on(|app| app.layout_action("New", None)),
                        MItem::item("Copy Layout").on(move |app| app.layout_action("Copy", Some(&n1))),
                        MItem::item("Rename").on(move |app| app.layout_action("Rename", Some(&n2))),
                        MItem::item("Delete").on(move |app| app.layout_action("Delete", Some(&n3))),
                        MItem::Sep,
                        MItem::item("Page Setup Manager...").on(move |app| {
                            app.switch_tab(&n4);
                            app.run_command("PAGESETUP", Args::none());
                        }),
                        MItem::item("Plot...").on(move |app| {
                            app.switch_tab(&n5);
                            app.run_command("PLOT", Args::none());
                        }),
                    ]
                }
                None => vec![MItem::item("New Layout").on(|app| app.layout_action("New", None)), MItem::item("Plot...").on(|app| app.run_command("PLOT", Args::none()))],
            };
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                self.open_popup(ctx, crate::menu::Popup::new(pos, items));
            }
        }
    }
}
