//! Window chrome: layout, title bar (application button, Quick Access Toolbar, InfoCenter), document tabs, status bar
//! and command line.
use crate::app::CadForgeApp;
use crate::icons::Tint;
use crate::menu::MItem;
use cadforge_engine::runtime::{ArgVal, Args};
use eframe::egui::{
    self, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Margin, Pos2, Rect, Response,
    RichText, Sense, Shape, Stroke, StrokeKind,
};

pub const TITLE_H: f32 = 34.0;
pub const TABS_H: f32 = 26.0;
pub const BODY_H: f32 = 90.0;
pub const DOCTABS_H: f32 = 28.0;
pub const STATUS_H: f32 = 30.0;

impl CadForgeApp {
    pub fn layout(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let clean = self.ui.clean;
        let frame = |fill: Color32| {
            egui::Frame::new()
                .fill(fill)
                .inner_margin(Margin::ZERO)
                .stroke(Stroke::NONE)
        };
        if !clean {
            egui::Panel::top("titlebar")
                .exact_size(TITLE_H)
                .resizable(false)
                .show_separator_line(false)
                .frame(frame(t.chrome))
                .show(ui, |ui| self.titlebar(ui, ctx));
            if self.ui.show_menubar {
                egui::Panel::top("menubar")
                    .exact_size(24.0)
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(frame(t.chrome2))
                    .show(ui, |ui| self.menubar(ui, ctx));
            }
            let rh = match self.ui.ribbon_mode {
                0 => TABS_H + BODY_H + 2.0,
                1 => TABS_H + 1.0,
                _ => TABS_H + 22.0,
            };
            if self.ui.show_ribbon {
                egui::Panel::top("ribbon")
                    .exact_size(rh)
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(frame(t.ribbon))
                    .show(ui, |ui| self.ribbon_ui(ui, ctx));
            }
            egui::Panel::top("doctabs")
                .exact_size(DOCTABS_H)
                .resizable(false)
                .show_separator_line(false)
                .frame(frame(t.chrome))
                .show(ui, |ui| self.doctabs(ui, ctx));
            egui::Panel::bottom("statusbar")
                .exact_size(STATUS_H)
                .resizable(false)
                .show_separator_line(false)
                .frame(frame(t.chrome))
                .show(ui, |ui| self.statusbar(ui, ctx));
            if self.ui.show_cmd {
                let lines = self.ui.cmd.lines as f32;
                let h = lines * 17.0 + 36.0;
                egui::Panel::bottom("cmdline")
                    .exact_size(h)
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(frame(t.chrome2))
                    .show(ui, |ui| self.cmdline_ui(ui, ctx));
            }
            egui::Panel::bottom("layouttabs")
                .exact_size(crate::layouts_ui::LAYOUT_TABS_H)
                .resizable(false)
                .show_separator_line(false)
                .frame(frame(t.chrome))
                .show(ui, |ui| self.layout_tabs(ui, ctx));
            self.palettes_ui(ui, ctx);
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            if self.ui.showing_start || self.h.borrow().editors.is_empty() {
                self.start_page(ui, ctx);
            } else {
                self.viewport_ui(ui, ctx);
            }
        });
        self.show_dialogs(ctx);
        self.show_context_menu(ctx);
    }

    // ------------------------------------------------------------ small widget helpers

    /// A flat icon button at an exact rectangle.
    pub fn icon_button(
        &self,
        ui: &mut egui::Ui,
        id: impl std::hash::Hash + std::fmt::Debug,
        rect: Rect,
        icon: &str,
        on: bool,
        enabled: bool,
    ) -> Response {
        let t = self.t;
        let resp = ui.interact(
            rect,
            ui.id().with(id),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let bg = if !enabled {
            None
        } else if resp.is_pointer_button_down_on() {
            Some(t.press)
        } else if on {
            Some(t.accent_soft)
        } else if resp.hovered() {
            Some(t.hover)
        } else {
            None
        };
        if let Some(bg) = bg {
            ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
        }
        let s = (rect.height() - 8.0).clamp(12.0, 24.0);
        let tint = if enabled {
            Tint::Normal
        } else {
            Tint::Mono(t.text3)
        };
        self.icons.paint(
            ui.painter(),
            icon,
            Rect::from_center_size(rect.center(), vec2(s, s)),
            &t,
            tint,
        );
        resp
    }

    pub fn cmd_tip(&self, resp: Response, cmd: &str, label: Option<&str>) -> Response {
        let (title, desc, name, alias) = {
            let a = self.h.borrow();
            match a.registry.get(cmd) {
                Some(d) => (
                    label.map(str::to_string).unwrap_or_else(|| d.title.clone()),
                    d.desc.clone(),
                    d.name.clone(),
                    a.registry.alias_of(&d.name),
                ),
                None => (
                    label.unwrap_or(cmd).to_string(),
                    String::new(),
                    cmd.to_string(),
                    None,
                ),
            }
        };
        let t = self.t;
        resp.on_hover_ui(move |ui| {
            ui.set_max_width(280.0);
            ui.label(
                RichText::new(title.replace('\n', " "))
                    .strong()
                    .color(t.text),
            );
            if !desc.is_empty() {
                ui.label(RichText::new(desc).color(t.text2));
            }
            let mut line = format!("Command: {name}");
            if let Some(a) = alias {
                line.push_str(&format!("   Alias: {a}"));
            }
            ui.label(RichText::new(line).small().color(t.text3));
        })
    }

    // ------------------------------------------------------------ title bar

    fn titlebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        p.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, t.border),
        );
        let mut x = rect.left() + 4.0;
        // application button (logo)
        let ab = Rect::from_min_size(Pos2::new(x, rect.top() + 2.0), vec2(34.0, 30.0));
        let resp = ui.interact(ab, ui.id().with("appbtn"), Sense::click());
        if resp.hovered() {
            p.rect_filled(ab, CornerRadius::same(4), t.hover);
        }
        logo(&p, ab.center(), 22.0);
        if resp.clicked() {
            self.app_menu(ctx, ab);
        }
        x += 38.0;
        // Quick Access Toolbar
        let (can_undo, can_redo) = {
            let a = self.h.borrow();
            a.doc()
                .map_or((false, false), |d| (d.can_undo(), d.can_redo()))
        };
        let qat: [(&str, &str, &str); 5] = [
            ("NEW", "new", "New"),
            ("OPEN", "open", "Open"),
            ("QSAVE", "save", "Save"),
            ("SAVEAS", "saveas", "Save As"),
            ("PLOT", "plot", "Plot"),
        ];
        for (cmd, icon, label) in qat {
            let r = Rect::from_min_size(Pos2::new(x, rect.top() + 4.0), vec2(26.0, 26.0));
            let resp = self.icon_button(ui, ("qat", cmd), r, icon, false, true);
            let resp = self.cmd_tip(resp, cmd, Some(label));
            if resp.clicked() {
                self.run_command(cmd, Args::none());
            }
            x += 27.0;
        }
        p.line_segment(
            [
                Pos2::new(x + 4.0, rect.center().y - 9.0),
                Pos2::new(x + 4.0, rect.center().y + 9.0),
            ],
            Stroke::new(1.0, t.sep),
        );
        x += 10.0;
        for (cmd, icon, label, enabled) in [
            ("U", "undo", "Undo", can_undo),
            ("REDO", "redo", "Redo", can_redo),
        ] {
            let r = Rect::from_min_size(Pos2::new(x, rect.top() + 4.0), vec2(26.0, 26.0));
            let resp = self.icon_button(ui, ("qat", cmd), r, icon, false, enabled);
            let resp = self.cmd_tip(resp, cmd, Some(label));
            if resp.clicked() {
                self.run_command(cmd, Args::none());
            }
            x += 27.0;
            if cmd == "U" {
                let ar =
                    Rect::from_min_size(Pos2::new(x - 1.0, rect.top() + 4.0), vec2(14.0, 26.0));
                let aresp = ui.interact(
                    ar,
                    ui.id().with("undo-arrow"),
                    if can_undo {
                        Sense::click()
                    } else {
                        Sense::hover()
                    },
                );
                if aresp.hovered() && can_undo {
                    p.rect_filled(ar, CornerRadius::same(3), t.hover);
                }
                chevron(
                    &p,
                    ar.center(),
                    4.0,
                    false,
                    if can_undo { t.text2 } else { t.text3 },
                );
                if aresp.clicked() {
                    let labels = self
                        .h
                        .borrow()
                        .doc()
                        .map(|d| d.undo_labels(12))
                        .unwrap_or_default();
                    let items: Vec<MItem> = labels
                        .into_iter()
                        .enumerate()
                        .map(|(i, l)| {
                            MItem::item(&l).on(move |app| {
                                for _ in 0..=i {
                                    app.run_command("U", Args::none());
                                }
                            })
                        })
                        .collect();
                    self.dropdown(ctx, ar.translate(vec2(-12.0, 0.0)), items, 170.0);
                }
                x += 14.0;
            }
        }
        p.line_segment(
            [
                Pos2::new(x + 4.0, rect.center().y - 9.0),
                Pos2::new(x + 4.0, rect.center().y + 9.0),
            ],
            Stroke::new(1.0, t.sep),
        );
        x += 10.0;
        // workspace switcher
        let ws = self.ui.workspace.clone();
        let wg = p.layout_no_wrap(ws.clone(), FontId::proportional(12.0), t.text2);
        let wr = Rect::from_min_size(
            Pos2::new(x, rect.top() + 4.0),
            vec2(wg.size().x + 30.0, 26.0),
        );
        let wresp = ui.interact(wr, ui.id().with("ws"), Sense::click());
        if wresp.hovered() {
            p.rect_filled(wr, CornerRadius::same(3), t.hover);
        }
        p.galley(
            Pos2::new(wr.left() + 6.0, wr.center().y - wg.size().y / 2.0),
            wg,
            if wresp.hovered() { t.text } else { t.text2 },
        );
        chevron(
            &p,
            Pos2::new(wr.right() - 10.0, wr.center().y),
            4.0,
            false,
            t.text2,
        );
        if wresp.clicked() {
            self.workspace_menu(ctx, wr, false);
        }
        x = wr.right() + 6.0;
        // document title
        let title = {
            let a = self.h.borrow();
            match a.doc() {
                Some(d) => format!(
                    "CadForge   {}{}",
                    d.file_name.clone().unwrap_or_else(|| d.name.clone()),
                    if d.dirty { " *" } else { "" }
                ),
                None => "CadForge".into(),
            }
        };
        p.text(
            Pos2::new(x + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            title,
            FontId::proportional(12.0),
            t.text2,
        );

        // right side: help + search
        let help = Rect::from_min_size(
            Pos2::new(rect.right() - 34.0, rect.top() + 4.0),
            vec2(28.0, 26.0),
        );
        let hresp = self.icon_button(ui, "help", help, "help", false, true);
        let hresp = self.cmd_tip(hresp, "HELP", Some("Help (F1)"));
        if hresp.clicked() {
            self.run_command("HELP", Args::none());
        }
        let search = Rect::from_min_size(
            Pos2::new(help.left() - 282.0, rect.top() + 5.0),
            vec2(270.0, 24.0),
        );
        let edit = egui::TextEdit::singleline(&mut self.ui.search)
            .hint_text("Type a keyword or phrase")
            .desired_width(search.width())
            .margin(Margin::symmetric(8, 4))
            .text_color(t.text);
        let resp = ui.put(search, edit);
        self.icons.paint(
            ui.painter(),
            "search",
            Rect::from_center_size(
                Pos2::new(search.right() - 14.0, search.center().y),
                vec2(14.0, 14.0),
            ),
            &t,
            Tint::Mono(t.text3),
        );
        if resp.has_focus() && !self.ui.search.trim().is_empty() {
            self.infocenter_results(ctx, search);
        }
        if resp.has_focus() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.ui.search.clear();
            resp.surrender_focus();
        }
    }

    fn infocenter_results(&mut self, ctx: &egui::Context, anchor: Rect) {
        let q = self.ui.search.trim().to_lowercase();
        let hits: Vec<(String, String, String, String)> = {
            let a = self.h.borrow();
            a.registry
                .defs()
                .filter(|d| {
                    !d.name.starts_with('_')
                        && !d.name.starts_with("OSNAP_")
                        && !d.title.is_empty()
                        && (d.name.to_lowercase().contains(&q)
                            || d.title.to_lowercase().contains(&q)
                            || d.desc.to_lowercase().contains(&q))
                })
                .take(10)
                .map(|d| {
                    (
                        d.name.clone(),
                        d.title.clone(),
                        d.desc.clone(),
                        if d.icon.is_empty() {
                            d.name.to_lowercase()
                        } else {
                            d.icon.clone()
                        },
                    )
                })
                .collect()
        };
        let t = self.t;
        let mut run: Option<String> = None;
        egui::Area::new(egui::Id::new("infocenter-results"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor.left_bottom() + vec2(0.0, 4.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t.chrome2)
                    .stroke(Stroke::new(1.0, t.border))
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(Margin::symmetric(0, 4))
                    .show(ui, |ui| {
                        ui.set_width(420.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        let head = if hits.is_empty() {
                            "NO RESULTS"
                        } else {
                            "COMMANDS"
                        };
                        let (r, _) = ui.allocate_exact_size(vec2(420.0, 20.0), Sense::hover());
                        ui.painter().text(
                            Pos2::new(r.left() + 10.0, r.center().y),
                            Align2::LEFT_CENTER,
                            head,
                            FontId::proportional(10.5),
                            t.text3,
                        );
                        for (name, title, desc, icon) in &hits {
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(420.0, 28.0), Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(r, 0.0, t.hover);
                            }
                            self.icons.paint(
                                ui.painter(),
                                icon,
                                Rect::from_min_size(
                                    Pos2::new(r.left() + 10.0, r.center().y - 9.0),
                                    vec2(18.0, 18.0),
                                ),
                                &t,
                                Tint::Normal,
                            );
                            ui.painter().text(
                                Pos2::new(r.left() + 38.0, r.center().y),
                                Align2::LEFT_CENTER,
                                title,
                                FontId::new(12.0, FontFamily::Name("bold".into())),
                                t.text,
                            );
                            let tw = ui
                                .painter()
                                .layout_no_wrap(
                                    title.clone(),
                                    FontId::new(12.0, FontFamily::Name("bold".into())),
                                    t.text,
                                )
                                .size()
                                .x;
                            let d: String = desc.chars().take(48).collect();
                            ui.painter().text(
                                Pos2::new(r.left() + 46.0 + tw, r.center().y),
                                Align2::LEFT_CENTER,
                                d,
                                FontId::proportional(11.5),
                                t.text3,
                            );
                            if resp.clicked() {
                                run = Some(name.clone());
                            }
                        }
                    });
            });
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if enter && run.is_none() {
            run = hits.first().map(|h| h.0.clone());
        }
        if let Some(name) = run {
            self.ui.search.clear();
            self.run_command(&name, Args::none());
        }
    }

    pub fn workspace_menu(&mut self, ctx: &egui::Context, anchor: Rect, up: bool) {
        let cur = self.ui.workspace.clone();
        let items: Vec<MItem> = ["Drafting & Annotation", "3D Modeling", "CadForge Classic"]
            .iter()
            .map(|n| {
                let n = n.to_string();
                let n2 = n.clone();
                MItem::item(&n)
                    .checked(cur == n)
                    .on(move |app| app.set_workspace(&n2))
            })
            .collect();
        if up {
            self.dropup(ctx, anchor, items, 190.0);
        } else {
            self.dropdown(ctx, anchor, items, 190.0);
        }
    }

    pub fn set_workspace(&mut self, name: &str) {
        self.ui.workspace = name.to_string();
        match name {
            "Drafting & Annotation" => {
                self.ui.ribbon_mode = 0;
                self.ui.ribbon_tab = "home".into();
                self.ui.show_menubar = false;
                self.ui.show_props = false;
            }
            "3D Modeling" => {
                self.ui.ribbon_mode = 0;
                self.ui.ribbon_tab = "solid".into();
                self.ui.show_menubar = false;
                self.ui.show_props = true;
                self.animate_preset("swiso");
            }
            _ => {
                self.ui.ribbon_mode = 1;
                self.ui.show_menubar = true;
                self.ui.show_props = true;
            }
        }
    }

    // ------------------------------------------------------------ document tabs

    fn doctabs(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        p.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, t.border),
        );
        let mut x = rect.left() + 4.0;
        let bottom = rect.bottom();
        let tab_shape = |p: &egui::Painter, r: Rect, fill: Color32, stroke: Color32| {
            p.rect_filled(
                r,
                CornerRadius {
                    nw: 4,
                    ne: 4,
                    sw: 0,
                    se: 0,
                },
                fill,
            );
            p.rect_stroke(
                r,
                CornerRadius {
                    nw: 4,
                    ne: 4,
                    sw: 0,
                    se: 0,
                },
                Stroke::new(1.0, stroke),
                StrokeKind::Inside,
            );
        };
        // start tab
        let sr = Rect::from_min_size(Pos2::new(x, bottom - 26.0), vec2(30.0, 25.0));
        let sresp = ui.interact(sr, ui.id().with("start-tab"), Sense::click());
        let start_on = self.ui.showing_start;
        tab_shape(
            &p,
            sr,
            if start_on {
                t.canvas
            } else if sresp.hovered() {
                t.ribbon_title
            } else {
                t.chrome2
            },
            t.border,
        );
        self.icons.paint(
            &p,
            "home",
            Rect::from_center_size(sr.center(), vec2(14.0, 14.0)),
            &t,
            Tint::Mono(if start_on { t.text } else { t.text2 }),
        );
        if sresp.clicked() {
            self.ui.showing_start = true;
        }
        x = sr.right() + 2.0;
        let editors: Vec<(u64, String, bool)> = self
            .h
            .borrow()
            .editors
            .iter()
            .map(|e| (e.id, e.doc.name.clone(), e.doc.dirty))
            .collect();
        let active = self.h.borrow().active;
        let mut close: Option<usize> = None;
        let mut activate: Option<usize> = None;
        for (i, (id, name, dirty)) in editors.iter().enumerate() {
            let label = format!("{}{}", name, if *dirty { "*" } else { "" });
            let g = p.layout_no_wrap(label, FontId::proportional(12.0), t.text);
            let w = (g.size().x + 50.0).clamp(90.0, 220.0);
            let is_active = active == Some(i) && !self.ui.showing_start;
            let r = Rect::from_min_size(
                Pos2::new(
                    x,
                    if is_active {
                        bottom - 27.0
                    } else {
                        bottom - 26.0
                    },
                ),
                vec2(w, if is_active { 27.0 } else { 25.0 }),
            );
            let resp = ui.interact(r, ui.id().with(("tab", *id)), Sense::click());
            tab_shape(
                &p,
                r,
                if is_active {
                    t.canvas
                } else if resp.hovered() {
                    t.ribbon_title
                } else {
                    t.chrome2
                },
                t.border,
            );
            let clip = Rect::from_min_max(r.min, Pos2::new(r.right() - 24.0, r.max.y));
            p.with_clip_rect(clip).galley(
                Pos2::new(r.left() + 12.0, r.center().y - g.size().y / 2.0),
                g,
                if is_active { t.text } else { t.text2 },
            );
            let xr =
                Rect::from_center_size(Pos2::new(r.right() - 14.0, r.center().y), vec2(16.0, 16.0));
            let xresp = ui.interact(xr, ui.id().with(("tabx", *id)), Sense::click());
            if xresp.hovered() {
                p.rect_filled(xr, CornerRadius::same(3), t.hover);
            }
            self.icons
                .paint(&p, "x", xr.shrink(3.5), &t, Tint::Mono(t.text2));
            if xresp.clicked() {
                close = Some(i);
            } else if resp.clicked() {
                activate = Some(i);
            }
            if resp.middle_clicked() {
                close = Some(i);
            }
            if resp.secondary_clicked() {
                let idx = i;
                let items = vec![
                    MItem::item("New").on(|app| app.run_command("NEW", Args::none())),
                    MItem::item("Open...").on(|app| app.run_command("OPEN", Args::none())),
                    MItem::item("Save").on(move |app| {
                        app.h.borrow_mut().set_active(idx);
                        app.run_command("QSAVE", Args::none());
                    }),
                    MItem::Sep,
                    MItem::item("Close").on(move |app| app.close_drawing(idx)),
                    MItem::item("Close All Other Drawings").on(move |app| {
                        let n = app.h.borrow().editors.len();
                        for k in (0..n).rev() {
                            if k != idx {
                                app.close_drawing(k);
                            }
                        }
                    }),
                ];
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    self.open_popup(ctx, crate::menu::Popup::new(pos, items));
                }
            }
            x += w + 2.0;
        }
        let pr = Rect::from_min_size(Pos2::new(x, bottom - 26.0), vec2(30.0, 25.0));
        let presp = ui.interact(pr, ui.id().with("plus-tab"), Sense::click());
        tab_shape(
            &p,
            pr,
            if presp.hovered() {
                t.ribbon_title
            } else {
                t.chrome2
            },
            t.border,
        );
        self.icons.paint(
            &p,
            "plus",
            Rect::from_center_size(pr.center(), vec2(12.0, 12.0)),
            &t,
            Tint::Mono(t.text2),
        );
        presp
            .on_hover_text("New drawing (Ctrl+N)")
            .clone()
            .clicked()
            .then(|| self.run_command("NEW", Args::none()));
        if let Some(i) = activate {
            self.ui.showing_start = false;
            self.h.borrow_mut().set_active(i);
            self.h.pump();
        }
        if let Some(i) = close {
            self.close_drawing(i);
        }
    }

    pub fn close_drawing(&mut self, idx: usize) {
        let dirty = self
            .h
            .borrow()
            .editors
            .get(idx)
            .is_some_and(|e| e.doc.dirty);
        if dirty {
            self.dialog = Some(crate::dialogs::Dialog::confirm_close(idx));
        } else {
            self.h.close_editor(idx);
            if self.h.borrow().editors.is_empty() {
                self.ui.showing_start = true;
            }
        }
    }

    // ------------------------------------------------------------ status bar

    fn statusbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        p.line_segment(
            [rect.left_top(), rect.right_top()],
            Stroke::new(1.0, t.border),
        );
        // coordinates
        let coords = self.coords_text();
        let cr = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.top() + 3.0),
            vec2(220.0, 24.0),
        );
        let cresp = ui.interact(cr, ui.id().with("coords"), Sense::click());
        if cresp.hovered() {
            p.rect_filled(cr, CornerRadius::same(3), t.hover);
        }
        p.text(
            Pos2::new(cr.left() + 8.0, cr.center().y),
            Align2::LEFT_CENTER,
            coords,
            FontId::monospace(12.0),
            t.text2,
        );
        if cresp.clicked() {
            self.ui.coord_relative = !self.ui.coord_relative;
        }
        let mut x = cr.right() + 4.0;
        p.line_segment(
            [
                Pos2::new(x, rect.center().y - 9.0),
                Pos2::new(x, rect.center().y + 9.0),
            ],
            Stroke::new(1.0, t.sep),
        );
        x += 5.0;
        let items = self.status_items();
        for it in items {
            if self.ui.status_hidden.contains(it.id) {
                continue;
            }
            let on = (it.on)(self);
            let label_w = if let Some(l) = it.label {
                p.layout_no_wrap(
                    l.to_string(),
                    FontId::new(11.5, FontFamily::Name("bold".into())),
                    t.text,
                )
                .size()
                .x + 8.0
            } else {
                0.0
            };
            let text = it.text.map(|f| f(self));
            let text_w = text.as_ref().map_or(0.0, |s| {
                p.layout_no_wrap(s.clone(), FontId::proportional(12.0), t.text)
                    .size()
                    .x
                    + 8.0
            });
            let w = if it.icon.is_some() { 28.0 } else { 10.0 } + label_w + text_w;
            let r = Rect::from_min_size(Pos2::new(x, rect.top() + 3.0), vec2(w, 24.0));
            let resp = ui.interact(r, ui.id().with(("sb", it.id)), Sense::click());
            let model = it.id == "model";
            let bg = if model {
                Some(t.accent_soft)
            } else if on {
                Some(t.accent_soft)
            } else if resp.hovered() {
                Some(t.hover)
            } else {
                None
            };
            if let Some(bg) = bg {
                p.rect_filled(r, CornerRadius::same(3), bg);
            }
            let ic = if on || model { t.accent2 } else { t.ic_k };
            let mut cx = r.left() + 5.0;
            if let Some(icon) = it.icon {
                self.icons.paint(
                    &p,
                    icon,
                    Rect::from_center_size(
                        Pos2::new(r.left() + 14.0, r.center().y),
                        vec2(16.0, 16.0),
                    ),
                    &t,
                    Tint::Mono(ic),
                );
                cx = r.left() + 24.0;
            }
            if let Some(l) = it.label {
                p.text(
                    Pos2::new(cx, r.center().y),
                    Align2::LEFT_CENTER,
                    l,
                    FontId::new(11.5, FontFamily::Name("bold".into())),
                    t.text,
                );
            }
            if let Some(s) = &text {
                p.text(
                    Pos2::new(cx, r.center().y),
                    Align2::LEFT_CENTER,
                    s,
                    FontId::proportional(12.0),
                    t.text,
                );
            }
            let extra = if it.key.is_empty() {
                String::new()
            } else {
                format!("Shortcut: {}", it.key)
            };
            let (title, desc) = (it.title, it.desc);
            let resp = resp.on_hover_ui(move |ui| {
                ui.set_max_width(280.0);
                ui.label(RichText::new(title).strong().color(t.text));
                ui.label(RichText::new(desc).color(t.text2));
                if !extra.is_empty() {
                    ui.label(RichText::new(extra).small().color(t.text3));
                }
            });
            if resp.clicked() {
                (it.act)(self, ctx, r);
            }
            x = r.right() + 1.0;
            if let Some(menu) = it.menu {
                let ar = Rect::from_min_size(Pos2::new(x, rect.top() + 3.0), vec2(12.0, 24.0));
                let aresp = ui.interact(ar, ui.id().with(("sb-arrow", it.id)), Sense::click());
                if aresp.hovered() {
                    p.rect_filled(ar, CornerRadius::same(3), t.hover);
                }
                chevron(&p, ar.center(), 3.5, true, t.text3);
                if aresp.clicked() {
                    menu(self, ctx, r);
                }
                x = ar.right() + 1.0;
            }
        }
        // customization
        let cu = Rect::from_min_size(
            Pos2::new(rect.right() - 34.0, rect.top() + 3.0),
            vec2(28.0, 24.0),
        );
        let uresp = ui.interact(cu, ui.id().with("sb-cust"), Sense::click());
        if uresp.hovered() {
            p.rect_filled(cu, CornerRadius::same(3), t.hover);
        }
        self.icons.paint(
            &p,
            "menubar",
            Rect::from_center_size(cu.center(), vec2(14.0, 14.0)),
            &t,
            Tint::Mono(t.ic_k),
        );
        if uresp.on_hover_text("Customization").clicked() {
            let all = self.status_items();
            let hidden = self.ui.status_hidden.clone();
            let mut v = vec![MItem::Header("Status bar items".into())];
            for it in all.iter().filter(|i| i.id != "model") {
                let id = it.id;
                v.push(
                    MItem::item(it.title)
                        .checked(!hidden.contains(id))
                        .on(move |app| {
                            if !app.ui.status_hidden.remove(id) {
                                app.ui.status_hidden.insert(id.to_string());
                            }
                        }),
                );
            }
            self.dropup(ctx, cu, v, 220.0);
        }
    }

    fn coords_text(&self) -> String {
        use cadforge_core::style::fmt_num;
        let a = self.h.borrow();
        let ucs = a.ucs();
        let c = ucs.from_wcs(a.rt.cursor.pt);
        let prec = a
            .doc()
            .map_or(4, |d| d.settings.units.precision.max(0) as usize)
            .min(4);
        if self.ui.coord_relative {
            if let Some(b) = a.pending().and_then(|p| p.base()) {
                let b = ucs.from_wcs(b);
                return format!(
                    "{} < {}°, {}",
                    fmt_num(cadforge_core::geom::dist(b, c), prec, false),
                    fmt_num(
                        cadforge_core::geom::deg(cadforge_core::geom::norm_angle(
                            cadforge_core::geom::angle_of(b, c)
                        )),
                        0,
                        false
                    ),
                    fmt_num(0.0, prec, false)
                );
            }
        }
        format!(
            "{}, {}, {}",
            fmt_num(c.x, prec, false),
            fmt_num(c.y, prec, false),
            fmt_num(0.0, prec, false)
        )
    }
}

pub type StatusAct = fn(&mut CadForgeApp, &egui::Context, Rect);

pub struct StatusItem {
    pub id: &'static str,
    pub icon: Option<&'static str>,
    pub title: &'static str,
    pub desc: &'static str,
    pub key: &'static str,
    pub label: Option<&'static str>,
    pub text: Option<fn(&CadForgeApp) -> String>,
    pub on: fn(&CadForgeApp) -> bool,
    pub act: StatusAct,
    pub menu: Option<StatusAct>,
}

impl CadForgeApp {
    pub fn sv(&self, name: &str) -> i64 {
        self.h.borrow().sysvars.int(name)
    }
    pub fn set_sv(&self, name: &str, v: f64) {
        let _ = self.h.borrow_mut().setvar(name, v);
        self.h
            .borrow_mut()
            .emit(cadforge_engine::app::Event::Overlay);
    }
    pub fn toggle_sv(&self, name: &str) {
        let v = self.sv(name);
        self.set_sv(name, if v != 0 { 0.0 } else { 1.0 });
    }

    fn status_items(&self) -> Vec<StatusItem> {
        vec![
            StatusItem { id: "model", icon: Some("viewcube"), title: "Model or Paper Space", desc: "Shows whether the sheet (PAPER) or model space (MODEL) is active. In a layout, click to switch between the sheet and the active viewport.", key: "", label: None, text: Some(|a| a.space_label()), on: |_| true, act: |a, _, _| a.toggle_space(), menu: None },
            StatusItem { id: "grid", icon: Some("grid"), title: "Display Grid", desc: "Displays a reference grid in the drawing area (F7).", key: "F7", label: None, text: None, on: |a| a.sv("GRIDMODE") != 0, act: |a, _, _| a.toggle_sv("GRIDMODE"), menu: Some(|a, ctx, r| a.grid_menu(ctx, r)) },
            StatusItem { id: "snap", icon: Some("snap"), title: "Snap Mode", desc: "Restricts cursor movement to specified intervals (F9).", key: "F9", label: None, text: None, on: |a| a.sv("SNAPMODE") != 0, act: |a, _, _| a.toggle_sv("SNAPMODE"), menu: Some(|a, ctx, r| a.snap_menu(ctx, r)) },
            StatusItem {
                id: "dyn",
                icon: Some("dyn"),
                title: "Dynamic Input",
                desc: "Displays command prompts and coordinate input near the cursor (F12).",
                key: "F12",
                label: None,
                text: None,
                on: |a| a.sv("DYNMODE") > 0,
                act: |a, _, _| a.toggle_dyn(),
                menu: None,
            },
            StatusItem { id: "ortho", icon: Some("ortho"), title: "Ortho Mode", desc: "Restricts cursor movement to horizontal or vertical (F8).", key: "F8", label: None, text: None, on: |a| a.sv("ORTHOMODE") != 0, act: |a, _, _| a.toggle_ortho(), menu: None },
            StatusItem { id: "polar", icon: Some("polar"), title: "Polar Tracking", desc: "Restricts cursor movement to specified angle increments (F10).", key: "F10", label: None, text: None, on: |a| a.sv("POLARMODE") != 0, act: |a, _, _| a.toggle_polar(), menu: Some(|a, ctx, r| a.polar_menu(ctx, r)) },
            StatusItem { id: "otrack", icon: Some("otrack"), title: "Object Snap Tracking", desc: "Tracks from acquired object snap points (F11).", key: "F11", label: None, text: None, on: |a| a.sv("OTRACKMODE") != 0, act: |a, _, _| a.toggle_sv("OTRACKMODE"), menu: None },
            StatusItem { id: "osnap", icon: Some("osnap"), title: "Object Snap", desc: "Snaps to geometric points on objects (F3).", key: "F3", label: None, text: None, on: |a| a.sv("OSMODE") & 16384 == 0, act: |a, _, _| a.toggle_osnap(), menu: Some(|a, ctx, r| a.osnap_menu(ctx, r)) },
            StatusItem { id: "lwt", icon: Some("lweight"), title: "Show/Hide Lineweight", desc: "Displays objects with their assigned lineweights.", key: "", label: None, text: None, on: |a| a.sv("LWDISPLAY") != 0, act: |a, _, _| a.toggle_sv("LWDISPLAY"), menu: None },
            StatusItem { id: "qp", icon: Some("qprops"), title: "Quick Properties", desc: "Displays a Quick Properties panel for selected objects.", key: "", label: None, text: None, on: |a| a.ui.show_props, act: |a, _, _| a.ui.show_props = !a.ui.show_props, menu: None },
            StatusItem {
                id: "units",
                icon: None,
                title: "Drawing Units",
                desc: "Sets linear and angular units.",
                key: "",
                label: None,
                text: Some(|a| format!("Decimal ({})", a.h.borrow().doc().map_or(4, |d| d.settings.units.precision))),
                on: |_| false,
                act: |a, ctx, r| a.units_menu(ctx, r),
                menu: None,
            },
            StatusItem { id: "ws", icon: Some("gear"), title: "Workspace Switching", desc: "Switches between workspaces.", key: "", label: None, text: None, on: |_| false, act: |a, ctx, r| a.workspace_menu(ctx, r, true), menu: None },
            StatusItem { id: "clean", icon: Some("cleanscreen"), title: "Clean Screen", desc: "Clears the screen of toolbars and dockable windows (Ctrl+0).", key: "Ctrl+0", label: None, text: None, on: |a| a.ui.clean, act: |a, _, _| a.ui.clean = !a.ui.clean, menu: None },
        ]
    }

    pub fn toggle_ortho(&self) {
        let on = self.sv("ORTHOMODE") != 0;
        self.set_sv("ORTHOMODE", if on { 0.0 } else { 1.0 });
        if !on {
            self.set_sv("POLARMODE", 0.0);
        }
    }
    pub fn toggle_polar(&self) {
        let on = self.sv("POLARMODE") != 0;
        self.set_sv("POLARMODE", if on { 0.0 } else { 1.0 });
        if !on {
            self.set_sv("ORTHOMODE", 0.0);
        }
    }
    pub fn toggle_osnap(&self) {
        self.set_sv("OSMODE", (self.sv("OSMODE") ^ 16384) as f64);
    }
    pub fn toggle_dyn(&self) {
        let d = self.sv("DYNMODE");
        self.set_sv(
            "DYNMODE",
            if d > 0 {
                -(d as f64)
            } else if d != 0 {
                d.abs() as f64
            } else {
                3.0
            },
        );
    }

    fn grid_menu(&mut self, ctx: &egui::Context, r: Rect) {
        let cur = self.h.borrow().sysvars.real("GRIDUNIT_X");
        let mut v = vec![
            MItem::item("Grid Settings...").on(|a| a.open_dsettings("snap")),
            MItem::Sep,
        ];
        for s in [1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0] {
            v.push(
                MItem::item(&format!("Spacing {s}"))
                    .checked((cur - s).abs() < 1e-9)
                    .on(move |a| {
                        a.set_sv("GRIDUNIT_X", s);
                        a.set_sv("GRIDUNIT_Y", s);
                    }),
            );
        }
        self.dropup(ctx, r, v, 180.0);
    }
    fn snap_menu(&mut self, ctx: &egui::Context, r: Rect) {
        let cur = self.h.borrow().sysvars.real("SNAPUNIT_X");
        let mut v = vec![
            MItem::item("Snap Settings...").on(|a| a.open_dsettings("snap")),
            MItem::Sep,
        ];
        for s in [1.0, 2.5, 5.0, 10.0, 25.0] {
            v.push(
                MItem::item(&format!("Snap spacing {s}"))
                    .checked((cur - s).abs() < 1e-9)
                    .on(move |a| {
                        a.set_sv("SNAPUNIT_X", s);
                        a.set_sv("SNAPUNIT_Y", s);
                        a.set_sv("SNAPMODE", 1.0);
                    }),
            );
        }
        self.dropup(ctx, r, v, 180.0);
    }
    fn polar_menu(&mut self, ctx: &egui::Context, r: Rect) {
        let cur = self.h.borrow().sysvars.real("POLARANG");
        let mut v = vec![
            MItem::item("Tracking Settings...").on(|a| a.open_dsettings("polar")),
            MItem::Sep,
        ];
        for s in [90.0, 45.0, 30.0, 22.5, 18.0, 15.0, 10.0, 5.0] {
            v.push(
                MItem::item(&format!("{s}"))
                    .checked((cur - s).abs() < 1e-9)
                    .on(move |a| {
                        a.set_sv("POLARANG", s);
                        a.set_sv("POLARMODE", 1.0);
                        a.set_sv("ORTHOMODE", 0.0);
                    }),
            );
        }
        self.dropup(ctx, r, v, 150.0);
    }
    fn osnap_menu(&mut self, ctx: &egui::Context, r: Rect) {
        let mode = self.sv("OSMODE") as u32;
        let mut v = vec![
            MItem::item("Object Snap Settings...").on(|a| a.open_dsettings("osnap")),
            MItem::Sep,
        ];
        for code in [
            "END", "MID", "CEN", "NOD", "QUA", "INT", "INS", "PER", "TAN", "NEA",
        ] {
            let bit = cadforge_engine::parse::os_bit(code);
            v.push(
                MItem::item(cadforge_engine::parse::os_full(code))
                    .checked(mode & bit != 0)
                    .on(move |a| a.set_sv("OSMODE", (a.sv("OSMODE") as u32 ^ bit) as f64)),
            );
        }
        v.push(MItem::Sep);
        v.push(MItem::item("Clear All").on(|a| a.set_sv("OSMODE", 16384.0)));
        self.dropup(ctx, r, v, 200.0);
    }
    fn units_menu(&mut self, ctx: &egui::Context, r: Rect) {
        let cur = self
            .h
            .borrow()
            .doc()
            .map_or(4, |d| d.settings.units.precision);
        let mut v = vec![MItem::Header("Linear precision".into())];
        for p in 0..=8 {
            let label = if p == 0 {
                "0".to_string()
            } else {
                format!("0.{}", "0".repeat(p as usize))
            };
            v.push(
                MItem::item(&label)
                    .checked(cur == p)
                    .on(move |a| a.set_sv("LUPREC", p as f64)),
            );
        }
        v.push(MItem::Sep);
        v.push(MItem::item("Units...").on(|a| a.run_command("UNITS", Args::none())));
        self.dropup(ctx, r, v, 150.0);
    }

    pub fn open_dsettings(&mut self, tab: &str) {
        self.dialog = Some(crate::dialogs::Dialog::dsettings(tab));
    }

    // ------------------------------------------------------------ command line

    fn cmdline_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        p.line_segment(
            [rect.left_top(), rect.right_top()],
            Stroke::new(1.0, t.border),
        );
        // resize grip on the left edge
        let grip = Rect::from_min_size(rect.left_top(), vec2(14.0, rect.height()));
        let gresp = ui.interact(grip, ui.id().with("cl-grip"), Sense::drag());
        for i in 0..3 {
            let y = rect.center().y - 6.0 + i as f32 * 5.0;
            p.line_segment(
                [
                    Pos2::new(rect.left() + 4.0, y),
                    Pos2::new(rect.left() + 9.0, y),
                ],
                Stroke::new(1.0, if gresp.hovered() { t.text2 } else { t.sep }),
            );
        }
        if gresp.hovered() || gresp.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if gresp.dragged() {
            let d = -gresp.drag_delta().y;
            self.ui.cmd.lines_f = (self.ui.cmd.lines_f + d / 17.0).clamp(1.0, 14.0);
            self.ui.cmd.lines = self.ui.cmd.lines_f.round() as usize;
        }
        // buttons (close / customize)
        let bx = rect.right() - 22.0;
        let cb = Rect::from_min_size(Pos2::new(bx, rect.top() + 3.0), vec2(18.0, 18.0));
        let mb = Rect::from_min_size(Pos2::new(bx - 20.0, rect.top() + 3.0), vec2(18.0, 18.0));
        for (r, icon, id) in [(cb, "x", "cl-close"), (mb, "menubar", "cl-menu")] {
            let resp = ui.interact(r, ui.id().with(id), Sense::click());
            if resp.hovered() {
                p.rect_filled(r, CornerRadius::same(3), t.hover);
            }
            self.icons
                .paint(&p, icon, r.shrink(4.0), &t, Tint::Mono(t.text3));
            if resp.clicked() {
                if id == "cl-close" {
                    self.ui.show_cmd = false;
                } else {
                    self.cmdline_menu(ctx, r);
                }
            }
        }
        // history
        let lines = self.ui.cmd.lines;
        let hist_rect = Rect::from_min_size(
            rect.left_top() + vec2(18.0, 3.0),
            vec2(rect.width() - 18.0 - 50.0, lines as f32 * 17.0),
        );
        let history: Vec<(String, cadforge_engine::app::LogKind)> = {
            let a = self.h.borrow();
            let n = a.history.len();
            let skip = n.saturating_sub(400);
            a.history[skip..]
                .iter()
                .map(|l| (l.text.clone(), l.kind))
                .collect()
        };
        let mut hui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(hist_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        hui.set_clip_rect(hist_rect);
        egui::ScrollArea::vertical()
            .id_salt("cl-history")
            .max_height(hist_rect.height())
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(&mut hui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for (text, kind) in &history {
                    use cadforge_engine::app::LogKind;
                    let color = match kind {
                        LogKind::Cmd => t.text,
                        LogKind::Input => t.accent2,
                        LogKind::Error => t.err,
                        LogKind::Prompt => t.text2,
                        LogKind::Info => t.text2,
                    };
                    for line in text.split('\n') {
                        let (r, _) = ui
                            .allocate_exact_size(vec2(ui.available_width(), 17.0), Sense::hover());
                        ui.painter().text(
                            Pos2::new(r.left(), r.center().y),
                            Align2::LEFT_CENTER,
                            line,
                            FontId::monospace(12.5),
                            color,
                        );
                    }
                }
            });
        // input row
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + 18.0, hist_rect.bottom() + 3.0),
            vec2(rect.width() - 24.0, 24.0),
        );
        let rresp = ui.interact(row, ui.id().with("cl-row"), Sense::click());
        p.rect_filled(row, CornerRadius::same(2), t.input);
        let focused = !ctx.egui_wants_keyboard_input();
        p.rect_stroke(
            row,
            CornerRadius::same(2),
            Stroke::new(1.0, if focused { t.accent } else { t.border }),
            StrokeKind::Inside,
        );
        self.icons.paint(
            &p,
            "chevright",
            Rect::from_center_size(
                Pos2::new(row.left() + 14.0, row.center().y),
                vec2(11.0, 11.0),
            ),
            &t,
            Tint::Mono(t.text3),
        );
        // prompt text with clickable options
        let (busy, name, prompt, options) = {
            let a = self.h.borrow();
            let busy = a.busy();
            let pending = a.pending();
            let name = a
                .top_task()
                .and_then(|id| a.rt.tasks.get(&id))
                .map(|ts| ts.def.name.clone())
                .unwrap_or_default();
            (
                busy,
                name,
                cadforge_engine::runtime::prompt_text(pending).replace('\n', " "),
                pending.map(|p| p.spec.options.clone()).unwrap_or_default(),
            )
        };
        let mut x = row.left() + 26.0;
        let mono = FontId::monospace(12.5);
        let mut clicked_kw: Option<String> = None;
        if !busy {
            let g = p.layout_no_wrap("Command:".into(), mono.clone(), t.text3);
            let w = g.size().x;
            p.galley(Pos2::new(x, row.center().y - g.size().y / 2.0), g, t.text3);
            x += w + 8.0;
        } else {
            if !name.is_empty() && !name.starts_with('_') {
                let g = p.layout_no_wrap(
                    name.clone(),
                    FontId::new(12.5, FontFamily::Name("bold".into())),
                    t.text,
                );
                let w = g.size().x;
                p.galley(Pos2::new(x, row.center().y - g.size().y / 2.0), g, t.text);
                x += w + 6.0;
            }
            // render [A/B/C] options as clickable words
            if let (Some(open), Some(close)) = (prompt.find('['), prompt.rfind(']')) {
                if !options.is_empty() && close > open {
                    let pre = &prompt[..open + 1];
                    let post = &prompt[close..];
                    let put = |s: &str, color: Color32, x: &mut f32| {
                        let g = p.layout_no_wrap(s.to_string(), mono.clone(), color);
                        let w = g.size().x;
                        p.galley(Pos2::new(*x, row.center().y - g.size().y / 2.0), g, color);
                        *x += w;
                    };
                    put(pre, t.text2, &mut x);
                    for (i, o) in options.iter().enumerate() {
                        if i > 0 {
                            put("/", t.text2, &mut x);
                        }
                        let label = o.split(" (").next().unwrap_or(o).to_string();
                        let g = p.layout_no_wrap(label.clone(), mono.clone(), t.accent2);
                        let r = Rect::from_min_size(
                            Pos2::new(x, row.center().y - g.size().y / 2.0),
                            g.size(),
                        );
                        let resp = ui.interact(r, ui.id().with(("cl-opt", i)), Sense::click());
                        let color = if resp.hovered() {
                            Color32::WHITE
                        } else {
                            t.accent2
                        };
                        if resp.hovered() {
                            p.rect_filled(r, 0.0, t.accent);
                        }
                        p.line_segment(
                            [
                                r.left_bottom() + vec2(0.0, -1.0),
                                r.right_bottom() + vec2(0.0, -1.0),
                            ],
                            Stroke::new(1.0, color),
                        );
                        p.galley(r.min, p.layout_no_wrap(label, mono.clone(), color), color);
                        if resp.clicked() {
                            clicked_kw = Some(o.clone());
                        }
                        x += r.width();
                    }
                    put(post, t.text2, &mut x);
                } else {
                    let g = p.layout_no_wrap(prompt.clone(), mono.clone(), t.text2);
                    let w = g.size().x;
                    p.galley(Pos2::new(x, row.center().y - g.size().y / 2.0), g, t.text2);
                    x += w;
                }
            } else {
                let g = p.layout_no_wrap(prompt.clone(), mono.clone(), t.text2);
                let w = g.size().x;
                p.galley(Pos2::new(x, row.center().y - g.size().y / 2.0), g, t.text2);
                x += w;
            }
            x += 8.0;
        }
        if let Some(kw) = clicked_kw {
            self.h.keyword(&kw);
        }
        // typed text with caret
        let text = self.ui.cmd.text.clone();
        let clip = Rect::from_min_max(
            Pos2::new(x.min(row.right() - 40.0), row.top()),
            row.right_bottom() - vec2(6.0, 0.0),
        );
        let tp = p.with_clip_rect(clip);
        if text.is_empty() && !busy {
            tp.text(
                Pos2::new(clip.left(), row.center().y),
                Align2::LEFT_CENTER,
                "Type a command",
                mono.clone(),
                t.text3,
            );
        }
        let g = tp.layout_no_wrap(text.clone(), mono.clone(), t.text);
        let caret_x = {
            let before: String = text.chars().take(self.ui.cmd.caret).collect();
            tp.layout_no_wrap(before, mono.clone(), t.text).size().x
        };
        tp.galley(
            Pos2::new(clip.left(), row.center().y - g.size().y / 2.0),
            g,
            t.text,
        );
        if focused && ((self.time * 1.6) as i64) % 2 == 0 {
            tp.line_segment(
                [
                    Pos2::new(clip.left() + caret_x, row.center().y - 8.0),
                    Pos2::new(clip.left() + caret_x, row.center().y + 8.0),
                ],
                Stroke::new(1.0, t.text),
            );
        }
        if focused {
            ctx.request_repaint_after(std::time::Duration::from_millis(400));
        }
        if rresp.clicked() {
            // typing always goes to the command line unless another field has focus
            ctx.memory_mut(|m| m.stop_text_input());
        }
        self.last_cmd_row = Some(row);
        // suggestions popup
        self.cmd_suggestions(ctx, row);
        let _ = (Shape::Noop, ArgVal::Num(0.0));
    }

    fn cmd_suggestions(&mut self, ctx: &egui::Context, row: Rect) {
        let sugs = self.ui.cmd.sugs.clone();
        if sugs.is_empty() {
            return;
        }
        let t = self.t;
        let sel = self.ui.cmd.sug_idx;
        let mut pick: Option<String> = None;
        let rows: Vec<(String, String, Option<String>)> = {
            let a = self.h.borrow();
            sugs.iter()
                .map(|n| match a.registry.get(n) {
                    Some(d) => (
                        n.clone(),
                        if d.title != *n {
                            d.title.clone()
                        } else {
                            String::new()
                        },
                        Some(if d.icon.is_empty() {
                            n.to_lowercase()
                        } else {
                            d.icon.clone()
                        }),
                    ),
                    None => (n.clone(), "System variable".into(), None),
                })
                .collect()
        };
        egui::Area::new(egui::Id::new("cl-suggest"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(row.left(), row.top() - 2.0))
            .pivot(Align2::LEFT_BOTTOM)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t.chrome2)
                    .stroke(Stroke::new(1.0, t.border))
                    .inner_margin(Margin::ZERO)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 18,
                        spread: 0,
                        color: Color32::from_black_alpha(110),
                    })
                    .show(ui, |ui| {
                        ui.set_width(340.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        for (i, (name, title, icon)) in rows.iter().enumerate() {
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(340.0, 22.0), Sense::click());
                            if sel == Some(i) || resp.hovered() {
                                ui.painter().rect_filled(r, 0.0, t.accent_soft);
                            }
                            match icon {
                                Some(ic) => self.icons.paint(
                                    ui.painter(),
                                    ic,
                                    Rect::from_center_size(
                                        Pos2::new(r.left() + 16.0, r.center().y),
                                        vec2(14.0, 14.0),
                                    ),
                                    &t,
                                    Tint::Normal,
                                ),
                                None => {
                                    let b = Rect::from_center_size(
                                        Pos2::new(r.left() + 16.0, r.center().y),
                                        vec2(14.0, 14.0),
                                    );
                                    ui.painter().rect_filled(b, CornerRadius::same(2), t.raised);
                                    ui.painter().text(
                                        b.center(),
                                        Align2::CENTER_CENTER,
                                        "V",
                                        FontId::proportional(9.0),
                                        t.text,
                                    );
                                }
                            }
                            ui.painter().text(
                                Pos2::new(r.left() + 32.0, r.center().y),
                                Align2::LEFT_CENTER,
                                name,
                                FontId::monospace(12.0),
                                t.text,
                            );
                            ui.painter().text(
                                Pos2::new(r.left() + 150.0, r.center().y),
                                Align2::LEFT_CENTER,
                                title,
                                FontId::proportional(11.5),
                                t.text3,
                            );
                            if resp.clicked() {
                                pick = Some(name.clone());
                            }
                        }
                    });
            });
        if let Some(n) = pick {
            self.ui.cmd.text = n;
            self.ui.cmd.sugs.clear();
            self.submit_cmd();
        }
    }

    fn cmdline_menu(&mut self, ctx: &egui::Context, r: Rect) {
        let recent: Vec<String> = self.h.borrow().rt.recent_commands.clone();
        let autoc = self.sv("AUTOCOMPLETE") != 0;
        let lines = self.ui.cmd.lines;
        let items = vec![
            MItem::item("Recent Commands").sub(if recent.is_empty() {
                vec![MItem::item("No recent commands").disabled(true)]
            } else {
                recent
                    .iter()
                    .map(|c| {
                        let c2 = c.clone();
                        MItem::item(c).on(move |a| a.run_command(&c2, Args::none()))
                    })
                    .collect()
            }),
            MItem::item("Input Settings").sub(vec![
                MItem::item("AutoComplete")
                    .checked(autoc)
                    .on(|a| a.toggle_sv("AUTOCOMPLETE")),
                MItem::item("Search system variables")
                    .checked(true)
                    .disabled(true),
            ]),
            MItem::Sep,
            MItem::item("Command line lines").sub(
                [1usize, 2, 3, 5, 8, 12]
                    .iter()
                    .map(|n| {
                        let n = *n;
                        MItem::item(&format!("{n} line{}", if n > 1 { "s" } else { "" }))
                            .checked(lines == n)
                            .on(move |a| {
                                a.ui.cmd.lines = n;
                                a.ui.cmd.lines_f = n as f32;
                            })
                    })
                    .collect(),
            ),
            MItem::item("Copy History").on(|a| {
                let text =
                    a.h.borrow()
                        .history
                        .iter()
                        .map(|l| l.text.clone())
                        .collect::<Vec<_>>()
                        .join("\n");
                a.copy_text(&text);
            }),
            MItem::item("Clear History").on(|a| {
                a.h.borrow_mut().history.clear();
            }),
            MItem::Sep,
            MItem::item("Close")
                .shortcut("Ctrl+9")
                .on(|a| a.ui.show_cmd = false),
        ];
        self.dropup(ctx, r, items, 190.0);
    }
}

// ---------------------------------------------------------------- shared drawing helpers

pub fn chevron(p: &egui::Painter, c: Pos2, r: f32, up: bool, color: Color32) {
    let d = if up { -1.0 } else { 1.0 };
    p.add(Shape::line(
        vec![
            c + vec2(-r, -r * 0.5 * d),
            c + vec2(0.0, r * 0.5 * d),
            c + vec2(r, -r * 0.5 * d),
        ],
        Stroke::new(1.3, color),
    ));
}

pub fn logo(p: &egui::Painter, c: Pos2, size: f32) {
    let k = size / 28.0;
    let o = c - vec2(14.0 * k, 14.0 * k);
    let pt = |x: f32, y: f32| o + vec2(x * k, y * k);
    p.add(Shape::convex_polygon(
        vec![
            pt(14.0, 2.5),
            pt(24.0, 8.2),
            pt(24.0, 19.8),
            pt(14.0, 25.5),
            pt(4.0, 19.8),
            pt(4.0, 8.2),
        ],
        Color32::from_rgb(0x2b, 0x8f, 0xe0),
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(
        vec![pt(14.0, 2.5), pt(24.0, 8.2), pt(14.0, 14.0), pt(4.0, 8.2)],
        Color32::from_rgb(0x5b, 0xb8, 0xff),
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(
        vec![pt(14.0, 14.0), pt(14.0, 25.5), pt(4.0, 19.8), pt(4.0, 8.2)],
        Color32::from_rgb(0x1a, 0x6f, 0xb5),
        Stroke::NONE,
    ));
    let mut pts = vec![];
    let start = -25.7f32.to_radians();
    let sweep = -(360.0f32 - 51.4).to_radians();
    for i in 0..=20 {
        let a = start + sweep * i as f32 / 20.0;
        pts.push(pt(14.0 + 6.0 * a.cos(), 14.0 + 6.0 * a.sin()));
    }
    p.add(Shape::line(pts, Stroke::new(2.2 * k, Color32::WHITE)));
}
