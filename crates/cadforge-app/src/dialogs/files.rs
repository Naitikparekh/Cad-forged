//! A small file browser used by the native build for Open / Import / Script (the web build uses the browser's file
//! chooser).
use super::Out;
use crate::app::CadForgeApp;
use crate::platform::Picked;
use eframe::egui::{self, vec2, CornerRadius, RichText, Sense};
use std::path::PathBuf;

pub struct FileBrowserState {
    pub title: String,
    pub purpose: &'static str,
    pub dir: PathBuf,
    pub entries: Vec<(String, bool)>,
    pub sel: Option<String>,
    pub exts: Vec<&'static str>,
    pub loaded: bool,
}

impl FileBrowserState {
    pub fn open(purpose: &'static str) -> FileBrowserState {
        let exts = match purpose {
            "script" => vec!["scr", "txt"],
            "image" => vec!["png", "jpg", "jpeg"],
            "lisp" => vec!["lsp", "scr", "txt"],
            "lin" => vec!["lin", "txt"],
            "plotstyle" => vec!["ctb", "stb"],
            "pat" => vec!["pat", "txt"],
            "template" => vec!["dwt", "dwg", "dxf", "json"],
            "standards" => vec!["dws", "dwg", "dxf", "dwt"],
            "datalink" | "datalinkupdate" => vec!["csv", "txt"],
            _ => vec!["dwg", "dwt", "dxf", "json", "cadforge"],
        };
        let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        FileBrowserState { title: match purpose { "import" => "Import".into(), "script" => "Select Script".into(), "image" => "Select Image File".into(), "lisp" => "Load Application".into(), "lin" => "Load Linetypes".into(), "plotstyle" => "Load Plot Style Table".into(), "pat" => "Load Hatch Patterns".into(), "template" => "Select Template".into(), "standards" => "Select Standards File".into(), "datalink" | "datalinkupdate" => "Select Data File".into(), _ => "Open Drawing".into() }, purpose, dir, entries: vec![], sel: None, exts, loaded: false }
    }

    fn reload(&mut self) {
        self.entries.clear();
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir || self.exts.iter().any(|x| name.to_lowercase().ends_with(&format!(".{x}"))) {
                    self.entries.push((name, is_dir));
                }
            }
        }
        self.entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        self.loaded = true;
    }
}

impl CadForgeApp {
    pub(super) fn ui_file_browser(&mut self, ui: &mut egui::Ui, s: &mut FileBrowserState) -> Out {
        let t = self.t;
        if !s.loaded {
            s.reload();
        }
        let mut dir_text = s.dir.to_string_lossy().to_string();
        ui.horizontal(|ui| {
            if self.btn(ui, "Up", false).clicked() {
                if let Some(p) = s.dir.parent() {
                    s.dir = p.to_path_buf();
                    s.sel = None;
                    s.loaded = false;
                }
            }
            let r = ui.add(egui::TextEdit::singleline(&mut dir_text).desired_width(f32::INFINITY));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let p = PathBuf::from(dir_text.trim());
                if p.is_dir() {
                    s.dir = p;
                    s.loaded = false;
                }
            }
        });
        let mut open_dir: Option<String> = None;
        let mut dbl = false;
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for (name, is_dir) in &s.entries {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                let sel = s.sel.as_deref() == Some(name.as_str());
                if sel {
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.accent_soft);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.hover);
                }
                let label = if *is_dir { format!("📁 {name}") } else { name.clone() };
                ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), t.text);
                if resp.clicked() {
                    s.sel = Some(name.clone());
                }
                if resp.double_clicked() {
                    if *is_dir {
                        open_dir = Some(name.clone());
                    } else {
                        dbl = true;
                    }
                }
            }
            if s.entries.is_empty() {
                ui.label(RichText::new("No matching files in this folder.").color(t.text3));
            }
        });
        if let Some(d) = open_dir {
            s.dir = s.dir.join(d);
            s.sel = None;
            s.loaded = false;
        }
        let ok_enabled = s.sel.as_ref().is_some_and(|n| s.entries.iter().any(|e| &e.0 == n && !e.1));
        let o = self.ok_cancel(ui, if ok_enabled { Some("Open") } else { None }, Some("Cancel"), &[]);
        if dbl && ok_enabled {
            return Out::Ok;
        }
        o
    }

    pub(super) fn finish_file_browser(&mut self, s: FileBrowserState) {
        let Some(name) = s.sel else { return };
        let path = s.dir.join(&name);
        match std::fs::read(&path) {
            Ok(bytes) => {
                self.save_dir = s.dir.clone();
                self.inbox.borrow_mut().push(Picked { purpose: s.purpose, name, bytes });
            }
            Err(e) => {
                self.h.borrow_mut().log(&format!("Could not read {}: {e}", path.display()), "error");
                self.toast(&format!("Could not read {name}: {e}"));
            }
        }
    }
}
