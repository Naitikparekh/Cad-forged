//! Configure Standards (STANDARDS) and Check Standards (CHECKSTANDARDS). Changes go through the commands, so each
//! one is logged and undone like the command-line version.
use super::{Dialog, Kind, Out};
use crate::app::CadForgeApp;
use cadforge_core::standards::{check, Problem};
use cadforge_engine::runtime::{ArgVal, Args};
use eframe::egui::{self, vec2, RichText};

#[derive(Default)]
pub struct StandardsState {
    pub sel: Option<String>,
}

#[derive(Default)]
pub struct CheckState {
    /// Problems passed over with Next Problem (by key), shown again when checking restarts.
    pub skipped: Vec<String>,
    /// The replacement chosen in the list (for the problem with key `for_key`).
    pub choice: Option<String>,
    pub for_key: String,
    pub fixed: usize,
}

fn opt(o: &str) -> ArgVal {
    ArgVal::Str(o.into())
}

impl CadForgeApp {
    fn row(&self, ui: &mut egui::Ui, text: &str, sub: &str, sel: bool) -> egui::Response {
        let t = self.t;
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), egui::Sense::click());
        if sel {
            ui.painter().rect_filled(r, 2.0, t.accent_soft);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 2.0, t.hover);
        }
        ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(12.5), t.text);
        ui.painter().text(r.right_center() - vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, sub, egui::FontId::proportional(11.5), t.text3);
        resp
    }

    pub(super) fn ui_standards(&mut self, ui: &mut egui::Ui, s: &mut StandardsState) -> Out {
        let files: Vec<(String, String)> = self.h.borrow().doc().map_or(vec![], |d| d.standards.files.iter().map(|f| (f.file.clone(), format!("{} layers, {} dim styles", f.layers.len(), f.dim_styles.len()))).collect());
        self.note(ui, "Standards files (.dws, .dwg or .dxf) set the layers, linetypes and styles this drawing should use.");
        ui.add_space(4.0);
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            if files.is_empty() {
                ui.label(RichText::new("No standards files are associated with this drawing.").color(self.t.text3));
            }
            for (f, sub) in &files {
                if self.row(ui, f, sub, s.sel.as_deref() == Some(f.as_str())).clicked() {
                    s.sel = Some(f.clone());
                }
            }
        });
        let o = self.ok_cancel(ui, (!files.is_empty()).then_some("Check Standards..."), Some("Close"), &["Add...", "Remove"]);
        match o {
            Out::Btn(0) => {
                self.open_files(&ui.ctx().clone(), "standards");
                Out::Open
            }
            Out::Btn(1) => {
                if let Some(f) = s.sel.take() {
                    self.run_command("STANDARDS", Args::none().with("option", opt("Remove")).with("file", ArgVal::Str(f)));
                    self.h.pump();
                }
                Out::Open
            }
            Out::Ok => {
                self.dialog = Some(Dialog::new(Kind::CheckStandards(CheckState::default())));
                Out::Open
            }
            o => o,
        }
    }

    pub(super) fn ui_checkstandards(&mut self, ui: &mut egui::Ui, s: &mut CheckState) -> Out {
        let probs: Vec<Problem> = self.h.borrow().doc().map_or(vec![], check);
        let left: Vec<&Problem> = probs.iter().filter(|p| !s.skipped.contains(&p.key())).collect();
        let Some(p) = left.first().copied() else {
            ui.label(RichText::new(if probs.is_empty() { "Checking is complete. No standards problems were found." } else { "Checking is complete. The problems you passed over are left." }).strong());
            ui.label(format!("{} fixed.", if s.fixed == 1 { "1 problem".to_string() } else { format!("{} problems", s.fixed) }));
            let o = self.ok_cancel(ui, Some("Close"), None, if probs.is_empty() { &[] } else { &["Check Again"] });
            if o == Out::Btn(0) {
                s.skipped.clear();
                return Out::Open;
            }
            return o;
        };
        if s.for_key != p.key() {
            (s.for_key, s.choice) = (p.key(), p.preferred.clone());
        }
        ui.label(RichText::new(format!("Problem {} of {}", probs.len() - left.len() + 1, probs.len())).color(self.t.text2));
        ui.label(RichText::new(p.message()).strong());
        ui.add_space(6.0);
        ui.label(RichText::new("Replace with").color(self.t.text2));
        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            for c in &p.candidates {
                let tag = if p.preferred.as_deref() == Some(c.as_str()) { "preferred fix" } else { "" };
                if self.row(ui, c, tag, s.choice.as_deref() == Some(c.as_str())).clicked() {
                    s.choice = Some(c.clone());
                }
            }
        });
        let fix_args = |key: &str| Args::none().with("key", ArgVal::Str(key.into()));
        match self.ok_cancel(ui, s.choice.is_some().then_some("Fix"), Some("Close"), &["Next Problem", "Ignore", "Fix All"]) {
            Out::Ok => {
                let with = s.choice.clone().unwrap_or_default();
                self.run_command("CHECKSTANDARDS", fix_args(&p.key()).with("with", ArgVal::Str(with)));
                self.h.pump();
                s.fixed += 1;
                Out::Open
            }
            Out::Btn(0) => {
                s.skipped.push(p.key());
                Out::Open
            }
            Out::Btn(1) => {
                self.run_command("CHECKSTANDARDS", fix_args(&p.key()).with("ignore", ArgVal::Bool(true)));
                self.h.pump();
                Out::Open
            }
            Out::Btn(2) => {
                let before = probs.len();
                self.run_command("CHECKSTANDARDS", Args::none().with("option", opt("All")));
                self.h.pump();
                s.fixed += before.saturating_sub(self.h.borrow().doc().map_or(0, |d| check(d).len()));
                Out::Open
            }
            o => o,
        }
    }
}
