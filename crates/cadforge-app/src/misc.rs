//! Application menu, classic menu bar, start page, context menus, UI actions delegated by commands, and file handling.
use crate::app::CadForgeApp;
use crate::icons::Tint;
use crate::menu::{MItem, Popup};
use crate::platform::{self, Picked};
use cadforge_core::drawing::Drawing;
use cadforge_core::io::{from_json, read_dxf, strip_ext, to_json, write_dxf, FILE_EXT};
use cadforge_engine::runtime::{ArgVal, Args, PromptKind};
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Margin, Pos2, Rect, Sense, Stroke, StrokeKind};

const MAX_RECENT: usize = 6;
const MAX_RECENT_BYTES: usize = 400_000;

impl CadForgeApp {
    pub fn copy_text(&mut self, text: &str) {
        platform::copy_to_clipboard(text);
    }

    // ------------------------------------------------------------ UI actions delegated by commands

    pub fn ui_action(&mut self, ctx: &egui::Context, name: &str, args: Args) {
        use crate::dialogs::Dialog;
        match name {
            "NEW" => {
                let imperial = args.str("template") == Some("imperial");
                self.new_drawing(imperial);
            }
            "OPEN" => self.open_files(ctx, "open"),
            "IMPORT" => self.open_files(ctx, "import"),
            "QSAVE" => self.save(ctx),
            "SAVEAS" => self.save_as(ctx),
            "DXFOUT" => self.export_dxf(ctx),
            "EXPORT" => self.export_menu(ctx),
            "EXPORTPDF" => self.dialog = Some(Dialog::plot("pdf")),
            "CLOSE" => {
                let a = self.h.borrow().active;
                if let Some(i) = a {
                    self.close_drawing(i);
                }
            }
            "QUIT" => self.dialog = Some(Dialog::confirm("CadForge", "Discard unsaved changes and close all drawings?", &["Close All", "Cancel"], crate::dialogs::Then::CloseAll)),
            "DSETTINGS" => self.dialog = Some(Dialog::dsettings(args.str("tab").unwrap_or("snap"))),
            "OPTIONS" => self.dialog = Some(Dialog::options()),
            "UNITS" => self.dialog = Some(Dialog::units()),
            "DIMSTYLE" => self.dialog = Some(Dialog::dimstyle()),
            "PLOT" => self.dialog = Some(Dialog::plot("pdf")),
            "PAGESETUP" => self.dialog = Some(Dialog::pagesetup()),
            "QSELECT" => self.dialog = Some(Dialog::qselect()),
            "ALIASEDIT" => self.dialog = Some(Dialog::aliasedit()),
            "STYLE" => self.dialog = Some(Dialog::textstyle()),
            "LAYERSTATE" => self.dialog = Some(Dialog::layerstate()),
            "ABOUT" => self.dialog = Some(Dialog::about()),
            "HELP" => self.dialog = Some(Dialog::help()),
            "QUICKCALC" => self.dialog = Some(Dialog::calc()),
            "PURGEDIALOG" => self.dialog = Some(Dialog::purge()),
            "MENUBAR" => self.ui.show_menubar = !self.ui.show_menubar,
            "CLEANSCREENON" => self.ui.clean = !self.ui.clean,
            "SCRIPT" => self.open_files(ctx, "script"),
            _ => self.toast(&format!("{name} is not available in this environment.")),
        }
    }

    // ------------------------------------------------------------ files

    pub fn new_drawing(&mut self, imperial: bool) {
        self.ui.showing_start = false;
        self.h.borrow_mut().new_drawing(None, imperial);
        self.h.pump();
    }

    pub fn adopt(&mut self, doc: Drawing) {
        self.ui.showing_start = false;
        let name = doc.name.clone();
        {
            let mut a = self.h.borrow_mut();
            a.add_drawing(doc);
            if let Some(ed) = a.editor_mut() {
                ed.zoomed = false;
            }
            a.log(&format!("Opened {name}."), "info");
        }
        self.h.pump();
    }

    pub fn open_sample(&mut self) {
        self.adopt(cadforge_core::sample::sample_drawing());
    }

    pub fn open_files(&mut self, ctx: &egui::Context, purpose: &'static str) {
        #[cfg(target_arch = "wasm32")]
        {
            let accept = if purpose == "script" { ".scr,.txt" } else { ".json,.cadforge,.dxf" };
            platform::pick_files(accept, purpose == "open", purpose, self.inbox.clone(), ctx.clone());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ctx;
            self.dialog = Some(crate::dialogs::Dialog::file_open(purpose));
        }
    }

    /// Files delivered by the platform (browser chooser or the native file dialog).
    pub fn process_inbox(&mut self) {
        let items: Vec<Picked> = std::mem::take(&mut *self.inbox.borrow_mut());
        for p in items {
            let text = String::from_utf8_lossy(&p.bytes).to_string();
            match p.purpose {
                "open" => match self.parse_drawing(&p.name, &text) {
                    Ok(mut doc) => {
                        doc.file_name = Some(p.name.clone());
                        doc.dirty = false;
                        self.adopt(doc);
                    }
                    Err(e) => {
                        self.h.borrow_mut().log(&format!("Could not open {}: {e}", p.name), "error");
                        self.toast(&format!("Could not open {}: {e}", p.name));
                    }
                },
                "import" => match self.parse_drawing(&p.name, &text) {
                    Ok(src) => self.import_into_current(&p.name, src),
                    Err(e) => self.h.borrow_mut().log(&format!("Could not import {}: {e}", p.name), "error"),
                },
                "script" => {
                    let lines: Vec<String> = text.lines().map(str::to_string).collect();
                    self.h.borrow_mut().log(&format!("Running script ({} lines)", lines.len()), "info");
                    self.h.borrow_mut().queue_script(lines);
                    self.h.pump();
                }
                _ => {}
            }
        }
    }

    fn parse_drawing(&self, name: &str, text: &str) -> Result<Drawing, String> {
        let base = strip_ext(name);
        if name.to_lowercase().ends_with(".dxf") {
            read_dxf(text, &base)
        } else {
            from_json(text, &base)
        }
    }

    fn import_into_current(&mut self, name: &str, src: Drawing) {
        let mut n = 0;
        self.edit_doc("Import", |d| {
            d.will_change();
            for l in &src.layers {
                if d.layer(&l.name).is_none() {
                    d.layers.push(l.clone());
                }
            }
            for (k, b) in &src.blocks {
                if !d.blocks.contains_key(k) {
                    d.blocks.insert(k.clone(), b.clone());
                }
            }
            for e in &src.model.entities {
                let mut c = e.clone();
                c.id = 0;
                d.add(c);
                n += 1;
            }
            d.touch();
        });
        let mut a = self.h.borrow_mut();
        a.log(&format!("Imported {n} objects from {name}."), "info");
        if let Some(ed) = a.editor_mut() {
            ed.zoom_extents(0.1);
        }
    }

    fn remember(&mut self, doc: &Drawing) {
        let json = to_json(doc);
        let mut list: Vec<serde_json::Value> = platform::store_get("recent").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        list.retain(|r| r.get("name").and_then(|n| n.as_str()) != Some(doc.name.as_str()));
        let time = (self.time * 1000.0) as i64;
        list.insert(0, serde_json::json!({ "name": doc.name, "time": time, "json": if json.len() < MAX_RECENT_BYTES { Some(json) } else { None } }));
        list.truncate(MAX_RECENT);
        platform::store_set("recent", &serde_json::to_string(&list).unwrap_or_default());
    }

    pub fn recent(&self) -> Vec<(String, Option<String>)> {
        let list: Vec<serde_json::Value> = platform::store_get("recent").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        list.iter().filter_map(|r| Some((r.get("name")?.as_str()?.to_string(), r.get("json").and_then(|j| j.as_str()).map(str::to_string)))).collect()
    }

    pub fn open_recent(&mut self, name: &str, json: Option<String>) {
        match json {
            None => self.toast(&format!("{name} is too large to reopen from history. Use Open to choose the file.")),
            Some(j) => match from_json(&j, name) {
                Ok(d) => self.adopt(d),
                Err(e) => self.toast(&e),
            },
        }
    }

    pub fn save(&mut self, ctx: &egui::Context) {
        let Some(name) = self.h.borrow().doc().map(|d| d.file_name.clone()) else { return };
        match name {
            Some(n) => self.write_doc(&n),
            None => self.save_as(ctx),
        }
    }

    pub fn save_as(&mut self, _ctx: &egui::Context) {
        let Some((name, file)) = self.h.borrow().doc().map(|d| (d.name.clone(), d.file_name.clone())) else { return };
        let suggested = match file {
            Some(f) => f,
            None => format!("{name}{FILE_EXT}"),
        };
        self.dialog = Some(crate::dialogs::Dialog::save_as(&suggested));
    }

    /// Serialise the active drawing under `file_name` (DXF when the name ends in .dxf).
    pub fn write_doc(&mut self, file_name: &str) {
        let lower = file_name.to_lowercase();
        let (bytes, mime, name) = {
            let a = self.h.borrow();
            let Some(doc) = a.doc() else { return };
            if lower.ends_with(".dxf") {
                (write_dxf(doc).into_bytes(), "application/dxf", file_name.to_string())
            } else {
                (to_json(doc).into_bytes(), "application/json", file_name.to_string())
            }
        };
        self.deliver_file(&name, bytes, mime);
        let doc_clone = {
            let mut a = self.h.borrow_mut();
            let Some(ed) = a.editor_mut() else { return };
            ed.doc.file_name = Some(name.clone());
            ed.doc.name = strip_ext(&name);
            ed.doc.dirty = false;
            let d = ed.doc.to_json_string();
            let n = ed.doc.name.clone();
            a.log(&format!("Saved {name}."), "info");
            a.emit(cadforge_engine::app::Event::Change);
            (n, d)
        };
        if let Ok(d) = from_json(&doc_clone.1, &doc_clone.0) {
            self.remember(&d);
        }
    }

    /// Hand a produced file to the user: a browser download, or a write next to the working directory on native.
    pub fn deliver_file(&mut self, name: &str, bytes: Vec<u8>, mime: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            platform::download(name, &bytes, mime);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = mime;
            let path = self.save_dir.join(name);
            match std::fs::write(&path, &bytes) {
                Ok(()) => self.h.borrow_mut().log(&format!("Wrote {}", path.display()), "info"),
                Err(e) => self.h.borrow_mut().log(&format!("Could not write {}: {e}", path.display()), "error"),
            }
        }
    }

    pub fn export_dxf(&mut self, _ctx: &egui::Context) {
        let Some((name, text)) = self.h.borrow().doc().map(|d| (d.name.clone(), write_dxf(d))) else { return };
        self.deliver_file(&format!("{name}.dxf"), text.into_bytes(), "application/dxf");
        self.h.borrow_mut().log(&format!("Exported {name}.dxf."), "info");
    }

    pub fn export_mesh(&mut self, fmt: &str) {
        let (name, text, n) = {
            let a = self.h.borrow();
            let Some(doc) = a.doc() else { return };
            let solids: Vec<&cadforge_core::solid::Solid> = doc.solids.iter().collect();
            if solids.is_empty() {
                (doc.name.clone(), String::new(), 0)
            } else if fmt == "stl" {
                (doc.name.clone(), cadforge_core::solid::mesh::to_stl(&solids), solids.len())
            } else {
                (doc.name.clone(), cadforge_core::solid::mesh::to_obj(&solids), solids.len())
            }
        };
        if n == 0 {
            self.toast("There are no solids to export.");
            return;
        }
        self.deliver_file(&format!("{name}.{fmt}"), text.into_bytes(), "text/plain");
        self.h.borrow_mut().log(&format!("Exported {n} solid{} to {name}.{fmt}.", if n == 1 { "" } else { "s" }), "info");
    }

    pub fn export_entities(&mut self, ents: Vec<cadforge_core::entities::Entity>) {
        let (layers, settings) = {
            let a = self.h.borrow();
            let Some(d) = a.doc() else { return };
            (d.layers.clone(), d.settings.clone())
        };
        let mut d = Drawing::new("Block");
        d.layers = layers;
        d.settings = settings;
        for mut e in ents {
            e.id = 0;
            d.add(e);
        }
        self.deliver_file(&format!("Block{FILE_EXT}"), to_json(&d).into_bytes(), "application/json");
    }

    pub fn export_menu(&mut self, ctx: &egui::Context) {
        let rect = self.vp.rect.unwrap_or(Rect::from_min_size(Pos2::new(100.0, 100.0), vec2(10.0, 10.0)));
        let items = vec![
            MItem::item("DXF (*.dxf)").icon("dxf").on(|a| a.run_command("DXFOUT", Args::none())),
            MItem::item("PDF (*.pdf)").icon("pdf").on(|a| a.dialog = Some(crate::dialogs::Dialog::plot("pdf"))),
            MItem::item("SVG (*.svg)").on(|a| a.dialog = Some(crate::dialogs::Dialog::plot("svg"))),
            MItem::item("PNG image (*.png)").on(|a| a.dialog = Some(crate::dialogs::Dialog::plot("png"))),
            MItem::item("OBJ mesh (*.obj)").on(|a| a.export_mesh("obj")),
            MItem::item("STL mesh (*.stl)").on(|a| a.export_mesh("stl")),
            MItem::item("CadForge project (*.json)").on(|a| {
                let n = a.h.borrow().doc().map(|d| d.name.clone()).unwrap_or_default();
                a.write_doc(&format!("{n}{FILE_EXT}"));
            }),
        ];
        self.open_popup(ctx, Popup::new(rect.min + vec2(40.0, 40.0), items));
    }

    // ------------------------------------------------------------ application menu

    pub fn app_menu(&mut self, ctx: &egui::Context, anchor: Rect) {
        self.ui.app_menu = Some((anchor, None));
        let _ = ctx;
    }

    pub fn show_app_menu(&mut self, ctx: &egui::Context) {
        let Some((anchor, mut sub)) = self.ui.app_menu else { return };
        let t = self.t;
        struct Item {
            icon: &'static str,
            label: &'static str,
            cmd: Option<&'static str>,
            sub: Vec<(&'static str, &'static str, &'static str)>,
        }
        let items = vec![
            Item { icon: "new", label: "New", cmd: Some("NEW"), sub: vec![] },
            Item { icon: "open", label: "Open", cmd: Some("OPEN"), sub: vec![] },
            Item { icon: "save", label: "Save", cmd: Some("QSAVE"), sub: vec![] },
            Item { icon: "saveas", label: "Save As", cmd: Some("SAVEAS"), sub: vec![("saveas", "CadForge Drawing (*.json)", "SAVEAS"), ("dxf", "DXF Drawing (*.dxf)", "DXFOUT")] },
            Item { icon: "import", label: "Import", cmd: Some("IMPORT"), sub: vec![] },
            Item { icon: "export", label: "Export", cmd: None, sub: vec![("dxf", "DXF", "DXFOUT"), ("pdf", "PDF", "EXPORTPDF"), ("export", "Other Formats...", "EXPORT")] },
            Item { icon: "plot", label: "Print", cmd: Some("PLOT"), sub: vec![] },
            Item { icon: "purge", label: "Drawing Utilities", cmd: None, sub: vec![("units", "Units...", "UNITS"), ("purge", "Purge...", "PURGEDIALOG"), ("rename", "Rename...", "-RENAME")] },
            Item { icon: "close", label: "Close", cmd: None, sub: vec![("close", "Current Drawing", "CLOSE")] },
        ];
        let recent = self.recent();
        let mut run: Option<(&'static str, Option<Args>)> = None;
        let mut open_recent: Option<(String, Option<String>)> = None;
        let mut close = false;
        let pos = anchor.left_bottom() + vec2(0.0, 2.0);
        let resp = egui::Area::new(egui::Id::new("app-menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::new().fill(t.raised).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(4)).shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(140) }).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.set_width(560.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    // left column
                    let (lr, _) = ui.allocate_exact_size(vec2(230.0, 9.0 * 40.0 + 12.0), Sense::hover());
                    ui.painter().rect_filled(lr, CornerRadius { nw: 4, ne: 0, sw: 0, se: 0 }, t.ribbon);
                    for (i, it) in items.iter().enumerate() {
                        let r = Rect::from_min_size(lr.min + vec2(6.0, 6.0 + i as f32 * 40.0), vec2(lr.width() - 12.0, 38.0));
                        let resp = ui.interact(r, ui.id().with(("am", i)), Sense::click());
                        if resp.hovered() {
                            sub = if it.sub.is_empty() { None } else { Some(i) };
                        }
                        if resp.hovered() || sub == Some(i) {
                            ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                        }
                        self.icons.paint(ui.painter(), it.icon, Rect::from_center_size(Pos2::new(r.left() + 22.0, r.center().y), vec2(24.0, 24.0)), &t, Tint::Normal);
                        ui.painter().text(Pos2::new(r.left() + 46.0, r.center().y), Align2::LEFT_CENTER, it.label, FontId::proportional(13.0), t.text);
                        if !it.sub.is_empty() {
                            let c = Pos2::new(r.right() - 12.0, r.center().y);
                            ui.painter().add(egui::Shape::line(vec![c + vec2(-2.5, -4.5), c + vec2(2.0, 0.0), c + vec2(-2.5, 4.5)], Stroke::new(1.3, t.text2)));
                        }
                        if resp.clicked() {
                            if let Some(c) = it.cmd {
                                run = Some((c, None));
                                close = true;
                            } else {
                                sub = Some(i);
                            }
                        }
                    }
                    // right column
                    let (rr, _) = ui.allocate_exact_size(vec2(330.0, 9.0 * 40.0 + 12.0), Sense::hover());
                    let head = if let Some(s) = sub { items[s].label } else { "Recent Documents" };
                    ui.painter().text(Pos2::new(rr.left() + 16.0, rr.top() + 18.0), Align2::LEFT_CENTER, head, FontId::new(12.5, FontFamily::Name("bold".into())), t.text);
                    let mut y = rr.top() + 40.0;
                    if let Some(s) = sub {
                        for (k, (icon, label, cmd)) in items[s].sub.iter().enumerate() {
                            let r = Rect::from_min_size(Pos2::new(rr.left() + 8.0, y), vec2(rr.width() - 16.0, 30.0));
                            let resp = ui.interact(r, ui.id().with(("am-sub", s, k)), Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                            }
                            self.icons.paint(ui.painter(), icon, Rect::from_center_size(Pos2::new(r.left() + 18.0, r.center().y), vec2(20.0, 20.0)), &t, Tint::Normal);
                            ui.painter().text(Pos2::new(r.left() + 38.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), t.text);
                            if resp.clicked() {
                                run = Some((cmd, None));
                                close = true;
                            }
                            y += 32.0;
                        }
                    } else if recent.is_empty() {
                        ui.painter().text(Pos2::new(rr.left() + 16.0, y + 10.0), Align2::LEFT_CENTER, "No recent documents", FontId::proportional(12.0), t.text3);
                    } else {
                        for (k, (name, json)) in recent.iter().enumerate() {
                            let r = Rect::from_min_size(Pos2::new(rr.left() + 8.0, y), vec2(rr.width() - 16.0, 30.0));
                            let resp = ui.interact(r, ui.id().with(("am-rec", k)), Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                            }
                            self.icons.paint(ui.painter(), "dxf", Rect::from_center_size(Pos2::new(r.left() + 18.0, r.center().y), vec2(20.0, 20.0)), &t, Tint::Normal);
                            ui.painter().text(Pos2::new(r.left() + 38.0, r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.5), t.text);
                            if resp.clicked() {
                                open_recent = Some((name.clone(), json.clone()));
                                close = true;
                            }
                            y += 32.0;
                        }
                    }
                });
                // footer
                let (fr, _) = ui.allocate_exact_size(vec2(560.0, 38.0), Sense::hover());
                ui.painter().rect_filled(fr, CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, t.ribbon_title);
                let b1 = Rect::from_min_size(Pos2::new(fr.right() - 250.0, fr.top() + 7.0), vec2(110.0, 24.0));
                let b2 = Rect::from_min_size(Pos2::new(fr.right() - 132.0, fr.top() + 7.0), vec2(120.0, 24.0));
                for (r, label, cmd) in [(b1, "Options", "OPTIONS"), (b2, "Exit CadForge", "QUIT")] {
                    let resp = ui.interact(r, ui.id().with(("am-foot", label)), Sense::click());
                    ui.painter().rect_filled(r, CornerRadius::same(3), if resp.hovered() { t.hover } else { t.raised });
                    ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, t.sep), StrokeKind::Inside);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.5), t.text);
                    if resp.clicked() {
                        run = Some((cmd, None));
                        close = true;
                    }
                }
            });
        });
        let rect = resp.response.rect;
        if let Some((a, _)) = self.ui.app_menu {
            self.ui.app_menu = Some((a, sub));
        }
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                if !rect.contains(p) && !anchor.contains(p) {
                    close = true;
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if close {
            self.ui.app_menu = None;
        }
        if let Some((name, args)) = run {
            self.run_command(name, args.unwrap_or_else(Args::none));
        }
        if let Some((n, j)) = open_recent {
            self.open_recent(&n, j);
        }
    }

    // ------------------------------------------------------------ classic menu bar

    pub fn menubar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, t.border));
        let mut x = rect.left();
        for (title, entries) in menubar_defs() {
            let g = p.layout_no_wrap(title.to_string(), FontId::proportional(12.5), t.text);
            let w = g.size().x + 24.0;
            let r = Rect::from_min_size(Pos2::new(x, rect.top()), vec2(w, rect.height()));
            let resp = ui.interact(r, ui.id().with(("mb", title)), Sense::click());
            let open = self.popup.as_ref().is_some_and(|pp| (pp.pos - r.left_bottom()).length() < 2.0);
            if resp.hovered() || open {
                p.rect_filled(r, 0.0, t.hover);
            }
            p.galley(Pos2::new(r.left() + 12.0, r.center().y - g.size().y / 2.0), g, t.text);
            if resp.clicked() || (resp.hovered() && self.popup.is_some() && !open) {
                let items: Vec<MItem> = entries
                    .iter()
                    .filter_map(|e| match e {
                        MbEntry::Sep => Some(MItem::Sep),
                        MbEntry::Cmd { cmd, label, args, shortcut } => {
                            let def = self.h.borrow().registry.get(cmd)?;
                            let mut it = MItem::item(label).icon_opt(Some(&def.icon)).on({
                                let (cmd, args) = (cmd.to_string(), *args);
                                move |app| {
                                    let mut a = Args::none();
                                    if let Some(m) = args {
                                        a = a.with("mode", ArgVal::Str(m.into()));
                                    }
                                    app.run_command(&cmd, a)
                                }
                            });
                            if let Some(s) = shortcut {
                                it = it.shortcut(s);
                            }
                            Some(it)
                        }
                    })
                    .collect();
                self.open_popup(ctx, Popup::new(r.left_bottom(), items));
            }
            x += w;
        }
    }

    // ------------------------------------------------------------ start page

    pub fn start_page(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, t.canvas);
        let col_x = rect.left() + 48.0;
        let p = ui.painter().clone();
        p.text(Pos2::new(col_x, rect.top() + 48.0), Align2::LEFT_CENTER, "Start", FontId::new(26.0, FontFamily::Name("bold".into())), t.text);
        let tiles: [(&str, &str, &str, u8); 4] = [("new", "Start Drawing", "Metric drawing, 420 × 297", 0), ("new", "Imperial Drawing", "Inches", 1), ("open", "Open Files", "DXF or CadForge JSON", 2), ("box", "Sample", "L-bracket", 3)];
        let mut action: Option<u8> = None;
        for (i, (icon, title, sub, id)) in tiles.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(col_x + i as f32 * 186.0, rect.top() + 80.0), vec2(176.0, 120.0));
            let resp = ui.interact(r, ui.id().with(("tile", id)), Sense::click());
            let fill = if *id == 0 { t.accent_soft } else { t.raised };
            p.rect_filled(r, CornerRadius::same(6), if resp.hovered() { t.hover } else { fill });
            p.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, if *id == 0 { t.accent } else { t.border }), StrokeKind::Inside);
            self.icons.paint(&p, icon, Rect::from_min_size(r.min + vec2(14.0, 14.0), vec2(40.0, 40.0)), &t, Tint::Normal);
            p.text(r.min + vec2(14.0, 74.0), Align2::LEFT_CENTER, title, FontId::new(13.5, FontFamily::Name("bold".into())), t.text);
            p.text(r.min + vec2(14.0, 96.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.5), t.text3);
            if resp.clicked() {
                action = Some(*id);
            }
        }
        p.text(Pos2::new(col_x, rect.top() + 236.0), Align2::LEFT_CENTER, "Recent Documents", FontId::new(16.0, FontFamily::Name("bold".into())), t.text);
        let recent = self.recent();
        let mut open: Option<(String, Option<String>)> = None;
        if recent.is_empty() {
            p.text(Pos2::new(col_x, rect.top() + 268.0), Align2::LEFT_CENTER, "Drawings you save appear here.", FontId::proportional(12.5), t.text3);
        }
        for (i, (name, json)) in recent.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(col_x, rect.top() + 256.0 + i as f32 * 40.0), vec2(360.0, 36.0));
            let resp = ui.interact(r, ui.id().with(("recent", i)), Sense::click());
            if resp.hovered() {
                p.rect_filled(r, CornerRadius::same(4), t.hover);
            }
            self.icons.paint(&p, "dxf", Rect::from_center_size(Pos2::new(r.left() + 22.0, r.center().y), vec2(22.0, 22.0)), &t, Tint::Normal);
            p.text(Pos2::new(r.left() + 44.0, r.center().y), Align2::LEFT_CENTER, name, FontId::new(12.5, FontFamily::Name("bold".into())), t.text);
            if resp.clicked() {
                open = Some((name.clone(), json.clone()));
            }
        }
        // tips
        let tx = col_x + 760.0;
        p.text(Pos2::new(tx, rect.top() + 236.0), Align2::LEFT_CENTER, "Tips", FontId::new(16.0, FontFamily::Name("bold".into())), t.text);
        let tips = [
            "Type commands such as L (Line), C (Circle), REC (Rectangle) at the command line.",
            "Press Space or Enter to repeat the last command.",
            "Use F3, F8 and F10 to toggle Osnap, Ortho and Polar.",
            "Type ALIASEDIT to define your own command aliases.",
        ];
        for (i, tip) in tips.iter().enumerate() {
            p.text(Pos2::new(tx, rect.top() + 266.0 + i as f32 * 24.0), Align2::LEFT_CENTER, format!("•  {tip}"), FontId::proportional(12.5), t.text2);
        }
        let _ = Margin::ZERO;
        match action {
            Some(0) => self.new_drawing(false),
            Some(1) => self.new_drawing(true),
            Some(2) => {
                let ctx = ui.ctx().clone();
                self.open_files(&ctx, "open")
            }
            Some(3) => self.open_sample(),
            _ => {}
        }
        if let Some((n, j)) = open {
            self.open_recent(&n, j);
        }
    }

    // ------------------------------------------------------------ context menus

    pub fn show_context_menu(&mut self, ctx: &egui::Context) {
        self.show_app_menu(ctx);
        self.layer_dropdown(ctx);
        self.process_inbox();
        let Some((pos, shift, _ctrl)) = self.context_menu_pending.take() else { return };
        let origin = self.vp.rect.map_or(pos, |r| r.min);
        let _ = origin;
        let a = self.h.borrow();
        let (busy, kind, opts, recent_input, has_hot, sel, can_undo, can_redo, last, recent_cmds, has_clip) = {
            let pending = a.pending();
            (
                a.busy(),
                pending.map(|p| p.kind),
                pending.map(|p| p.spec.options.clone()).unwrap_or_default(),
                a.rt.recent_input.clone(),
                a.rt.hot_grip.is_some(),
                a.editor().map_or(0, |e| e.sel.len()),
                a.doc().is_some_and(|d| d.can_undo()),
                a.doc().is_some_and(|d| d.can_redo()),
                a.rt.last_command.clone(),
                a.rt.recent_commands.clone(),
                a.clipboard.is_some(),
            )
        };
        let shortcut_mode = a.sysvars.int("SHORTCUTMENU");
        let has_pending = a.pending().is_some();
        drop(a);
        let cmd = |c: &'static str| move |app: &mut CadForgeApp| app.run_command(c, Args::none());
        if shift {
            let items = self.osnap_items();
            self.open_popup(ctx, Popup::new(pos, items));
            return;
        }
        if busy {
            if !has_pending {
                return;
            }
            let has_opts = !opts.is_empty() && kind != Some(PromptKind::Select);
            if kind == Some(PromptKind::Select) || !((shortcut_mode & 4 != 0) || (has_opts && (shortcut_mode & 8 != 0))) {
                self.h.enter();
                return;
            }
            let mut items = vec![
                MItem::item("Enter").shortcut("Enter").on(|a| a.h.enter()),
                MItem::item("Cancel").shortcut("Esc").on(|a| a.escape()),
                MItem::item("Recent Input").sub(if recent_input.is_empty() { vec![MItem::item("No recent input").disabled(true)] } else { recent_input.iter().map(|t| { let t2 = t.clone(); MItem::item(t).on(move |a| a.h.text(&t2, false)) }).collect() }),
                MItem::Sep,
            ];
            if has_opts {
                for o in &opts {
                    let o2 = o.clone();
                    items.push(MItem::item(o.split(" (").next().unwrap_or(o)).on(move |a| a.h.keyword(&o2)));
                }
                items.push(MItem::Sep);
            }
            items.push(MItem::item("Snap Overrides").sub(self.osnap_items()));
            items.push(MItem::Sep);
            items.push(MItem::item("Pan").icon("pan").on(cmd("PAN")));
            items.push(MItem::item("Zoom").icon("zoom").on(cmd("ZOOM")));
            self.open_popup(ctx, Popup::new(pos, items));
            return;
        }
        if has_hot {
            let items = vec![
                MItem::item("Enter").on(|a| a.h.enter()),
                MItem::item("Move").on(|a| a.h.text("MO", false)),
                MItem::item("Mirror").on(|a| a.h.text("MI", false)),
                MItem::item("Rotate").on(|a| a.h.text("RO", false)),
                MItem::item("Scale").on(|a| a.h.text("SC", false)),
                MItem::item("Stretch").on(|a| a.h.text("ST", false)),
                MItem::Sep,
                MItem::item("Exit").on(|a| a.escape()),
            ];
            self.open_popup(ctx, Popup::new(pos, items));
            return;
        }
        let paste = MItem::item("Paste").icon("paste").shortcut("Ctrl+V").disabled(!has_clip).on(cmd("PASTECLIP"));
        let clipboard = MItem::item("Clipboard").sub(vec![MItem::item("Cut").icon("cutclip").shortcut("Ctrl+X").disabled(sel == 0).on(cmd("CUTCLIP")), MItem::item("Copy").icon("copyclip").shortcut("Ctrl+C").disabled(sel == 0).on(cmd("COPYCLIP")), paste]);
        let recent = MItem::item("Recent Input").sub(if recent_cmds.is_empty() { vec![MItem::item("No recent commands").disabled(true)] } else { recent_cmds.iter().map(|c| { let c2 = c.clone(); MItem::item(c).on(move |a| a.run_command(&c2, Args::none())) }).collect() });
        let repeat = last.map(|l| { let l2 = l.clone(); MItem::item(&format!("Repeat {l}")).on(move |a| a.run_command(&l2, Args::none())) });
        let mut items: Vec<MItem> = vec![];
        if let Some(r) = repeat {
            items.push(r);
        }
        items.push(recent);
        items.push(MItem::Sep);
        items.push(clipboard);
        items.push(MItem::Sep);
        if sel == 0 {
            items.extend([
                MItem::item("Undo").icon("undo").shortcut("Ctrl+Z").disabled(!can_undo).on(cmd("U")),
                MItem::item("Redo").icon("redo").shortcut("Ctrl+Y").disabled(!can_redo).on(cmd("REDO")),
                MItem::Sep,
                MItem::item("Pan").icon("pan").on(cmd("PAN")),
                MItem::item("Zoom").icon("zoom").on(cmd("ZOOM")),
                MItem::Sep,
                MItem::item("Select All").shortcut("Ctrl+A").on(cmd("SELECTALL")),
                MItem::item("Quick Select...").icon("qselect").on(cmd("QSELECT")),
                MItem::Sep,
                MItem::item("Options...").icon("options").on(cmd("OPTIONS")),
            ]);
        } else {
            items.extend([
                MItem::item("Erase").icon("erase").shortcut("Del").on(cmd("ERASE")),
                MItem::item("Move").icon("move").on(cmd("MOVE")),
                MItem::item("Copy Selection").icon("copy").on(cmd("COPY")),
                MItem::item("Scale").icon("scale").on(cmd("SCALE")),
                MItem::item("Rotate").icon("rotate").on(cmd("ROTATE")),
                MItem::item("Draw Order").sub(vec![MItem::item("Bring to Front").on(cmd("TOFRONT")), MItem::item("Send to Back").on(cmd("TOBACK"))]),
                MItem::Sep,
                MItem::item("Select Similar").on(cmd("SELECTSIMILAR")),
                MItem::item("Deselect All").on(|a| {
                    let mut app = a.h.borrow_mut();
                    if let Some(e) = app.editor_mut() {
                        e.clear_sel();
                    }
                    app.emit(cadforge_engine::app::Event::Change);
                }),
                MItem::item("Quick Select...").icon("qselect").on(cmd("QSELECT")),
                MItem::Sep,
                MItem::item("Quick Properties").on(|a| a.ui.show_props = true),
                MItem::item("Properties").icon("properties").shortcut("Ctrl+1").on(|a| a.ui.show_props = true),
            ]);
        }
        self.open_popup(ctx, Popup::new(pos, items));
    }

    fn osnap_items(&self) -> Vec<MItem> {
        let send = |c: &'static str| {
            move |a: &mut CadForgeApp| {
                let ok = a.h.borrow().pending().is_some_and(|p| matches!(p.kind, PromptKind::Point | PromptKind::Distance | PromptKind::Angle));
                if ok {
                    a.h.text(c, false);
                }
            }
        };
        let mk = |k: &'static str| MItem::item(cadforge_engine::parse::os_full(k)).on(send(k));
        vec![
            MItem::item("Temporary track point").disabled(true),
            MItem::item("From").on(send("FROM")),
            MItem::Sep,
            mk("END"),
            mk("MID"),
            mk("INT"),
            mk("CEN"),
            mk("QUA"),
            mk("TAN"),
            mk("PER"),
            mk("INS"),
            mk("NOD"),
            mk("NEA"),
            MItem::Sep,
            MItem::item("None").on(send("NON")),
            MItem::item("Osnap Settings...").on(|a| a.open_dsettings("osnap")),
        ]
    }
}

pub enum MbEntry {
    Sep,
    Cmd { cmd: &'static str, label: &'static str, args: Option<&'static str>, shortcut: Option<&'static str> },
}

fn c(cmd: &'static str, label: &'static str) -> MbEntry {
    MbEntry::Cmd { cmd, label, args: None, shortcut: None }
}
fn cs(cmd: &'static str, label: &'static str, shortcut: &'static str) -> MbEntry {
    MbEntry::Cmd { cmd, label, args: None, shortcut: Some(shortcut) }
}
fn ca(cmd: &'static str, label: &'static str, mode: &'static str) -> MbEntry {
    MbEntry::Cmd { cmd, label, args: Some(mode), shortcut: None }
}

fn menubar_defs() -> Vec<(&'static str, Vec<MbEntry>)> {
    use MbEntry::Sep as S;
    vec![
        ("File", vec![cs("NEW", "New", "Ctrl+N"), cs("OPEN", "Open...", "Ctrl+O"), S, cs("QSAVE", "Save", "Ctrl+S"), cs("SAVEAS", "Save As...", "Ctrl+Shift+S"), S, c("IMPORT", "Import..."), c("DXFOUT", "Export DXF..."), c("EXPORTPDF", "Export PDF..."), S, cs("PLOT", "Plot...", "Ctrl+P"), S, c("CLOSE", "Close"), c("QUIT", "Exit")]),
        ("Edit", vec![cs("U", "Undo", "Ctrl+Z"), cs("REDO", "Redo", "Ctrl+Y"), S, cs("CUTCLIP", "Cut", "Ctrl+X"), cs("COPYCLIP", "Copy", "Ctrl+C"), cs("PASTECLIP", "Paste", "Ctrl+V"), S, cs("SELECTALL", "Select All", "Ctrl+A"), c("QSELECT", "Quick Select...")]),
        (
            "View",
            vec![c("REGEN", "Regen"), c("REGENALL", "Regen All"), S, ca("ZOOM", "Zoom Window", "window"), ca("ZOOM", "Zoom Previous", "previous"), ca("ZOOM", "Zoom Extents", "extents"), c("PAN", "Pan"), S, c("3DORBIT", "3D Orbit"), c("-VIEW", "Named Views..."), S, cs("PROPERTIES", "Properties", "Ctrl+1"), cs("TOOLPALETTES", "Tool Palettes", "Ctrl+3"), cs("COMMANDLINE", "Command Line", "Ctrl+9"), cs("CLEANSCREENON", "Clean Screen", "Ctrl+0")],
        ),
        ("Insert", vec![c("INSERT", "Block..."), c("BLOCK", "Make Block..."), c("WBLOCK", "Write Block..."), S, c("IMPORT", "Import...")]),
        ("Format", vec![c("LAYER", "Layer..."), c("COLOR", "Color..."), c("LINETYPE", "Linetype..."), c("LWEIGHT", "Lineweight..."), S, c("STYLE", "Text Style..."), c("DIMSTYLE", "Dimension Style..."), c("UNITS", "Units...")]),
        ("Tools", vec![c("PROPERTIES", "Properties"), c("TOOLPALETTES", "Tool Palettes"), S, c("DSETTINGS", "Drafting Settings..."), c("ALIASEDIT", "Edit Aliases..."), c("SCRIPT", "Run Script..."), S, c("OPTIONS", "Options...")]),
        (
            "Draw",
            vec![
                c("LINE", "Line"),
                c("XLINE", "Construction Line"),
                c("RAY", "Ray"),
                c("PLINE", "Polyline"),
                c("POLYGON", "Polygon"),
                c("RECTANG", "Rectangle"),
                S,
                ca("ARC", "Arc", "3p"),
                c("CIRCLE", "Circle"),
                c("DONUT", "Donut"),
                c("SPLINE", "Spline"),
                c("ELLIPSE", "Ellipse"),
                S,
                c("HATCH", "Hatch..."),
                c("BOUNDARY", "Boundary..."),
                S,
                c("POINT", "Point"),
                c("MTEXT", "Multiline Text..."),
                c("TEXT", "Single Line Text"),
            ],
        ),
        ("Dimension", vec![c("DIMLINEAR", "Linear"), c("DIMALIGNED", "Aligned"), c("DIMANGULAR", "Angular"), c("DIMRADIUS", "Radius"), c("DIMDIAMETER", "Diameter"), S, c("DIMBASELINE", "Baseline"), c("DIMCONTINUE", "Continue"), c("MLEADER", "Multileader"), S, c("DIMSTYLE", "Dimension Style...")]),
        (
            "Modify",
            vec![
                c("PROPERTIES", "Properties"),
                c("MATCHPROP", "Match Properties"),
                S,
                c("ERASE", "Erase"),
                c("COPY", "Copy"),
                c("MIRROR", "Mirror"),
                c("OFFSET", "Offset"),
                c("ARRAYRECT", "Rectangular Array"),
                c("ARRAYPOLAR", "Polar Array"),
                c("MOVE", "Move"),
                c("ROTATE", "Rotate"),
                c("SCALE", "Scale"),
                c("STRETCH", "Stretch"),
                c("LENGTHEN", "Lengthen"),
                c("TRIM", "Trim"),
                c("EXTEND", "Extend"),
                c("BREAK", "Break"),
                c("JOIN", "Join"),
                c("CHAMFER", "Chamfer"),
                c("FILLET", "Fillet"),
                c("EXPLODE", "Explode"),
            ],
        ),
        ("Help", vec![cs("HELP", "Help", "F1"), c("ABOUT", "About CadForge")]),
    ]
}
