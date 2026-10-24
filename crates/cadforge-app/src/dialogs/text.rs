//! In-place text editor for TEXT / MTEXT.
use super::Out;
use crate::app::CadForgeApp;
use cadforge_engine::runtime::TextEditSpec;
use eframe::egui::{self, RichText};

pub struct TextEditState {
    pub text: String,
    pub multi: bool,
    pub prompt: String,
    pub focused: bool,
}

impl TextEditState {
    pub fn new(spec: TextEditSpec) -> TextEditState {
        TextEditState { text: spec.initial.unwrap_or_default(), multi: spec.multi, prompt: spec.prompt, focused: false }
    }
}

impl CadForgeApp {
    pub(super) fn ui_edit_text(&mut self, ui: &mut egui::Ui, s: &mut TextEditState) -> Out {
        let t = self.t;
        if !s.prompt.is_empty() {
            ui.label(RichText::new(&s.prompt).color(t.text2));
        }
        let r = if s.multi {
            ui.add(egui::TextEdit::multiline(&mut s.text).desired_width(f32::INFINITY).desired_rows(6).font(egui::FontId::proportional(13.0)))
        } else {
            ui.add(egui::TextEdit::singleline(&mut s.text).desired_width(f32::INFINITY).font(egui::FontId::proportional(13.0)))
        };
        if !s.focused {
            r.request_focus();
            s.focused = true;
        }
        if !s.multi && r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            return Out::Ok;
        }
        if s.multi {
            self.note(ui, "Press Ctrl+Enter or OK to place the text. Use Enter for a new line.");
            if r.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) {
                return Out::Ok;
            }
        }
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}
