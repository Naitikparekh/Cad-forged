//! In-place text editor for TEXT / MTEXT. Multiline text gets a paragraph toolbar (alignment, bullets, numbering) that
//! applies to the paragraphs under the cursor or the selection, a width (the text wraps at it; 0 keeps each paragraph
//! on one line) and a preview drawn with the drawing's own text layout.
use super::Out;
use crate::app::CadForgeApp;
use cadforge_core::mtext::{join_paragraphs, split_paragraphs, Align, List, ParaFmt};
use cadforge_engine::runtime::{DialogReply, TextEditSpec};
use eframe::egui::{self, RichText};

pub struct TextEditState {
    pub text: String,
    pub multi: bool,
    pub prompt: String,
    pub focused: bool,
    /// One format per line of `text` (multiline text only).
    fmts: Vec<ParaFmt>,
    /// `text` as it was last frame, to keep `fmts` in step with lines typed or deleted.
    last: String,
    pub width: String,
    /// The paragraphs the cursor or selection covers (first, last).
    sel: (usize, usize),
    h: f64,
    jh: cadforge_core::text::HJust,
}

impl TextEditState {
    pub fn new(spec: TextEditSpec) -> TextEditState {
        let initial = spec.initial.unwrap_or_default();
        let (text, fmts) = if spec.multi {
            let paras = split_paragraphs(&initial);
            (paras.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n"), paras.into_iter().map(|(f, _)| f).collect())
        } else {
            (initial, vec![])
        };
        let width = cadforge_engine::sysvars::fmt_num(spec.w.unwrap_or(0.0).max(0.0));
        TextEditState { last: text.clone(), text, multi: spec.multi, prompt: spec.prompt, focused: false, fmts, width, sel: (0, 0), h: if spec.h > 0.0 { spec.h } else { 2.5 }, jh: spec.jh }
    }

    fn width_value(&self) -> f64 {
        self.width.trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.0)
    }

    /// The MTEXT contents: paragraphs with their formats.
    fn contents(&self) -> String {
        let paras: Vec<(ParaFmt, String)> = self.text.split('\n').enumerate().map(|(i, l)| (self.fmts.get(i).copied().unwrap_or_default(), l.to_string())).collect();
        join_paragraphs(&paras)
    }

    /// What the editor hands back: the text, and for multiline text its width.
    pub fn reply(&self) -> DialogReply {
        if self.multi {
            DialogReply::MText(self.contents(), self.width_value())
        } else {
            DialogReply::Text(self.text.clone())
        }
    }

    /// Keep one format per line after an edit: lines kept before and after the change keep theirs, new lines take
    /// the format of the line they were typed in (Enter continues a list or an alignment).
    fn sync_formats(&mut self) {
        if self.text == self.last {
            return;
        }
        let old: Vec<&str> = self.last.split('\n').collect();
        let new: Vec<&str> = self.text.split('\n').collect();
        let pre = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let max_suf = old.len().min(new.len()) - pre.min(old.len().min(new.len()));
        let suf = old.iter().rev().zip(new.iter().rev()).take(max_suf).take_while(|(a, b)| a == b).count();
        let mut fmts: Vec<ParaFmt> = self.fmts.iter().copied().chain(std::iter::repeat(ParaFmt::default())).take(old.len()).collect();
        let fill = fmts.get(pre).or(pre.checked_sub(1).and_then(|k| fmts.get(k))).copied().unwrap_or_default();
        let middle = new.len() - pre - suf;
        fmts.splice(pre..old.len() - suf, std::iter::repeat(fill).take(middle));
        self.fmts = fmts;
        self.last = self.text.clone();
    }

    fn set_align(&mut self, a: Align) {
        self.ensure_fmts();
        for i in self.sel.0..=self.sel.1.min(self.fmts.len().saturating_sub(1)) {
            self.fmts[i].align = a;
        }
    }

    /// Turn the list on for the selected paragraphs, or off when they all have it already.
    fn toggle_list(&mut self, l: List) {
        self.ensure_fmts();
        let range = self.sel.0..=self.sel.1.min(self.fmts.len().saturating_sub(1));
        let all = range.clone().all(|i| self.fmts[i].list == l);
        for i in range {
            self.fmts[i].list = if all { List::None } else { l };
        }
    }

    fn ensure_fmts(&mut self) {
        let n = self.text.split('\n').count();
        self.fmts.resize(n, ParaFmt::default());
    }

    /// The paragraph format the toolbar shows: the first selected paragraph's.
    fn current(&self) -> ParaFmt {
        self.fmts.get(self.sel.0).copied().unwrap_or_default()
    }
}

impl CadForgeApp {
    pub(super) fn ui_edit_text(&mut self, ui: &mut egui::Ui, s: &mut TextEditState) -> Out {
        let t = self.t;
        if !s.prompt.is_empty() {
            ui.label(RichText::new(&s.prompt).color(t.text2));
        }
        if !s.multi {
            let r = ui.add(egui::TextEdit::singleline(&mut s.text).desired_width(f32::INFINITY).font(egui::FontId::proportional(13.0)));
            if !s.focused {
                r.request_focus();
                s.focused = true;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                return Out::Ok;
            }
            return self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[]);
        }
        // Ctrl+Enter places the text: taken before the text box, which would read the Enter as a new line
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
            return Out::Ok;
        }
        // ---- paragraph toolbar
        let cur = s.current();
        let mut refocus = false;
        ui.horizontal_wrapped(|ui| {
            for (a, label, tip) in [(Align::Left, "Left", "Align the paragraph left"), (Align::Center, "Center", "Center the paragraph"), (Align::Right, "Right", "Align the paragraph right"), (Align::Justify, "Justify", "Straight edges on both sides")] {
                if ui.selectable_label(cur.align == a, label).on_hover_text(tip).clicked() {
                    s.set_align(a);
                    refocus = true;
                }
            }
            ui.separator();
            if ui.selectable_label(cur.list == List::Bullet, "• Bullets").on_hover_text("A bulleted list (click again to remove)").clicked() {
                s.toggle_list(List::Bullet);
                refocus = true;
            }
            if ui.selectable_label(cur.list == List::Number, "1. Numbering").on_hover_text("A numbered list (click again to remove)").clicked() {
                s.toggle_list(List::Number);
                refocus = true;
            }
            ui.separator();
            ui.label("Width");
            self.num_edit(ui, &mut s.width, 64.0).on_hover_text("The text wraps at this width; 0 keeps each paragraph on one line");
        });
        // ---- the text
        let out = egui::TextEdit::multiline(&mut s.text).desired_width(f32::INFINITY).desired_rows(6).font(egui::FontId::proportional(13.0)).show(ui);
        if !s.focused || refocus {
            out.response.request_focus();
            s.focused = true;
        }
        s.sync_formats();
        if let Some(r) = out.cursor_range {
            let line_of = |idx: usize| s.text.chars().take(idx).filter(|c| *c == '\n').count();
            let (a, b) = (line_of(r.primary.index.0), line_of(r.secondary.index.0));
            s.sel = (a.min(b), a.max(b));
        }
        // ---- preview, laid out as the drawing will show it
        ui.label(RichText::new("Preview").small().color(t.text3));
        let w = s.width_value();
        let mut td = cadforge_core::text::TextData::new(cadforge_core::geom::Pt::ZERO, s.h, &s.contents());
        td.multi = true;
        td.w = Some(w);
        td.jh = s.jh;
        td.jv = cadforge_core::text::VJust::Top;
        let lay = cadforge_core::text::text_layout(&td);
        let (bx0, by0, bx1, by1) = lay.bx;
        let avail = ui.available_width().max(100.0);
        let (bw, bh) = ((bx1 - bx0).max(s.h), (by1 - by0).max(s.h));
        let scale = (avail as f64 / bw).min(18.0 / s.h).max(0.5);
        // room above the first line for the letters' full height (the layout box is measured to the cap height)
        let above = ((cadforge_core::text::em_size(s.h) - s.h) * scale) as f32;
        let height = (bh * scale) as f32 + above + 14.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(avail, height.min(240.0)), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 3.0, t.input);
        let origin = rect.left_top() + egui::vec2(6.0, 6.0 + above);
        let to = |x: f64, y: f64| origin + egui::vec2(((x - bx0) * scale) as f32, ((by1 - y) * scale) as f32);
        // the wrap width: a line at the right edge of the text box
        if w > 0.0 {
            let x = to(bx0 + w, 0.0).x;
            p.vline(x, rect.y_range(), egui::Stroke::new(1.0, t.accent));
        }
        // the drawing's own face (the one the layout measures with), so the words sit where the drawing puts them
        let family = crate::gfx::family_of(cadforge_render::fonts::builtin_for(""));
        let size = |hh: f64| (cadforge_core::text::em_size(hh) * scale).clamp(4.0, 40.0) as f32;
        // text is placed by its baseline: the bottom of egui's line box is about a fifth of the size below it
        let draw = |x: f64, y: f64, txt: &str, hh: f64| {
            let sz = size(hh);
            p.text(to(x, y) + egui::vec2(0.0, sz * 0.22), egui::Align2::LEFT_BOTTOM, txt, egui::FontId::new(sz, family.clone()), t.text);
        };
        if lay.runs.is_empty() {
            for l in &lay.lines {
                draw(l.x, l.y, &l.s, s.h);
            }
        } else {
            for r in &lay.runs {
                draw(r.x, r.y, &r.s, r.h);
            }
        }
        self.note(ui, "Enter starts a new paragraph (it keeps this one's format). Ctrl+Enter or OK places the text.");
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadforge_core::text::{HJust, VJust};

    fn editor(initial: &str) -> TextEditState {
        TextEditState::new(TextEditSpec { prompt: String::new(), p: cadforge_core::geom::Pt::ZERO, h: 2.5, rot: 0.0, jh: HJust::Left, jv: VJust::Top, multi: true, w: Some(40.0), initial: Some(initial.into()), editing: None })
    }

    #[test]
    fn new_lines_keep_the_format_of_the_line_they_were_typed_in() {
        let mut s = editor("Legend\nGreen light = Motor starter is on");
        s.sel = (1, 1);
        s.toggle_list(List::Bullet);
        s.sel = (0, 0);
        s.set_align(Align::Center);
        // Enter at the end of the bullet line, then a new item
        s.text.push_str("\nRed light = fault");
        s.sync_formats();
        assert_eq!(s.fmts.iter().map(|f| (f.align, f.list)).collect::<Vec<_>>(), vec![(Align::Center, List::None), (Align::Left, List::Bullet), (Align::Left, List::Bullet)]);
        // deleting the middle line drops its format, the others keep theirs
        s.text = "Legend\nRed light = fault".into();
        s.sync_formats();
        assert_eq!(s.fmts.len(), 2);
        assert_eq!((s.fmts[0].align, s.fmts[1].list), (Align::Center, List::Bullet));
    }

    #[test]
    fn lists_toggle_and_the_reply_holds_codes_and_width() {
        let mut s = editor("a\nb");
        s.sel = (0, 1);
        s.toggle_list(List::Number);
        assert!(s.fmts.iter().all(|f| f.list == List::Number));
        s.width = "25".into();
        let DialogReply::MText(text, w) = s.reply() else { panic!() };
        assert_eq!(w, 25.0);
        assert!(text.contains("1.\ta") && text.contains("2.\tb"), "{text}");
        s.toggle_list(List::Number);
        assert!(s.fmts.iter().all(|f| f.list == List::None), "a second click removes the list");
        // opening formatted text shows the paragraphs and their formats again
        let again = editor(&text);
        assert_eq!(again.text, "a\nb");
        assert!(again.fmts.iter().all(|f| f.list == List::Number));
        assert_eq!(again.width, "40");
    }
}
