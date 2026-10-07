//! Colour, linetype, lineweight, insert and hatch-settings dialogs.
use super::{ColorTarget, Out};
use crate::app::CadForgeApp;
use cadforge_core::style::{aci_hex, describe_color, lw_display, parse_hex, Color, LINETYPES, LINEWEIGHTS};
use eframe::egui::{self, vec2, Color32, CornerRadius, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};

// ---------------------------------------------------------------- colour

pub struct ColorState {
    pub value: Option<Color>,
    pub allow_by_layer: bool,
    pub target: ColorTarget,
    pub tab: usize,
    pub rgb: [u8; 3],
    pub hex: String,
}

impl ColorState {
    pub fn new(value: Option<Color>, allow_by_layer: bool, target: ColorTarget) -> ColorState {
        let rgb = match value {
            Some(Color::Rgb(r, g, b)) => [r, g, b],
            Some(Color::Aci(i)) => parse_hex(aci_hex(i)),
            _ => [255, 255, 255],
        };
        ColorState { value, allow_by_layer, target, tab: if matches!(value, Some(Color::Rgb(..))) { 1 } else { 0 }, rgb, hex: format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]) }
    }
}

fn aci_color(i: u8) -> Color32 {
    let c = parse_hex(aci_hex(i));
    Color32::from_rgb(c[0], c[1], c[2])
}

impl CadForgeApp {
    pub(super) fn ui_color(&mut self, ui: &mut egui::Ui, s: &mut ColorState) -> Out {
        let t = self.t;
        self.tab_strip(ui, &["Index Color", "True Color"], &mut s.tab);
        let mut dbl = false;
        let mut cell = |ui: &mut egui::Ui, value: &mut Option<Color>, i: u8, size: f32, dbl: &mut bool| {
            let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
            ui.painter().rect_filled(r, 0.0, aci_color(i));
            let sel = *value == Some(Color::Aci(i));
            if sel {
                ui.painter().rect_stroke(r.expand(1.0), 0.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Outside);
                ui.painter().rect_stroke(r.expand(2.0), 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Outside);
            }
            if resp.clicked() {
                *value = Some(Color::Aci(i));
            }
            if resp.double_clicked() {
                *dbl = true;
            }
            resp.on_hover_text(format!("{i}"));
        };
        if s.tab == 0 {
            self.note(ui, "AutoCAD Color Index (ACI)");
            ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
            for row in 0..10 {
                ui.horizontal(|ui| {
                    for hue in 0..24 {
                        cell(ui, &mut s.value, (10 + hue * 10 + row) as u8, 18.0, &mut dbl);
                    }
                });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for i in 1..=7u8 {
                    cell(ui, &mut s.value, i, 24.0, &mut dbl);
                }
                ui.add_space(20.0);
                for i in 250..=255u8 {
                    cell(ui, &mut s.value, i, 24.0, &mut dbl);
                }
            });
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        } else {
            ui.horizontal(|ui| {
                ui.label("Hex");
                let r = ui.add(egui::TextEdit::singleline(&mut s.hex).desired_width(90.0).font(egui::FontId::monospace(12.0)));
                if r.changed() {
                    if let Some(Color::Rgb(a, b, c)) = Color::from_hex(s.hex.trim()) {
                        s.rgb = [a, b, c];
                        s.value = Some(Color::Rgb(a, b, c));
                    }
                }
                if ui.color_edit_button_srgb(&mut s.rgb).changed() {
                    s.value = Some(Color::Rgb(s.rgb[0], s.rgb[1], s.rgb[2]));
                    s.hex = format!("#{:02x}{:02x}{:02x}", s.rgb[0], s.rgb[1], s.rgb[2]);
                }
            });
            self.note(ui, "Any RGB colour. It is written to DXF as the nearest colour index plus a true-colour value.");
        }
        if s.allow_by_layer {
            ui.horizontal(|ui| {
                if self.btn(ui, "ByLayer", false).clicked() {
                    s.value = None;
                }
                if self.btn(ui, "ByBlock", false).clicked() {
                    s.value = Some(Color::ByBlock);
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label("Color:");
            self.color_chip(ui, s.value);
            ui.label(RichText::new(describe_color(s.value)).strong());
        });
        let _ = t;
        let o = self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[]);
        if dbl {
            return Out::Ok;
        }
        o
    }
}

// ---------------------------------------------------------------- linetype

pub struct LinetypeState {
    pub sel: Option<String>,
    pub pick: bool,
}

impl LinetypeState {
    pub fn new(value: Option<String>, pick: bool) -> LinetypeState {
        let sel = match value.as_deref() {
            None | Some("BYLAYER") => Some("Continuous".to_string()),
            Some(v) => Some(v.to_string()),
        };
        LinetypeState { sel, pick }
    }
}

impl CadForgeApp {
    pub(super) fn ui_linetype(&mut self, ui: &mut egui::Ui, s: &mut LinetypeState) -> Out {
        let t = self.t;
        self.note(ui, if s.pick { "Choose a linetype." } else { "Choose the current linetype for new objects." });
        let mut dbl = false;
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for l in LINETYPES.iter() {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                let sel = s.sel.as_deref() == Some(l.name);
                if sel {
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.accent_soft);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.hover);
                }
                ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, cadforge_core::style::ltype_display(l.name), egui::FontId::proportional(12.5), t.text);
                ui.painter().text(r.left_center() + vec2(110.0, 0.0), egui::Align2::LEFT_CENTER, l.desc, egui::FontId::proportional(11.5), t.text3);
                crate::ribbon::lt_sample(ui.painter(), Pos2::new(r.right() - 140.0, r.center().y), l.name, 120.0, t.text);
                if resp.clicked() {
                    s.sel = Some(l.name.to_string());
                }
                if resp.double_clicked() {
                    dbl = true;
                }
            }
        });
        let o = self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[]);
        if dbl {
            return Out::Ok;
        }
        o
    }
}

// ---------------------------------------------------------------- lineweight

pub struct LweightState {
    pub sel: Option<i32>,
    pub pick: bool,
    pub display: bool,
    pub display_init: bool,
}

impl LweightState {
    pub fn new(value: Option<i32>, pick: bool) -> LweightState {
        LweightState { sel: value, pick, display: false, display_init: false }
    }
}

impl CadForgeApp {
    pub(super) fn ui_lweight(&mut self, ui: &mut egui::Ui, s: &mut LweightState) -> Out {
        let t = self.t;
        if !s.display_init {
            s.display = self.sv("LWDISPLAY") != 0;
            s.display_init = true;
            if s.sel.is_none() && !s.pick {
                s.sel = self.h.borrow().doc().and_then(|d| d.cur.lw);
            }
        }
        let mut opts: Vec<(Option<i32>, String)> = vec![(None, "ByLayer".into()), (Some(-2), "ByBlock".into()), (Some(-3), "Default".into())];
        opts.extend(LINEWEIGHTS.iter().map(|w| (Some(*w), lw_display(Some(*w)))));
        let mut dbl = false;
        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            for (v, label) in &opts {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                let sel = s.sel == *v;
                if sel {
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.accent_soft);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.hover);
                }
                ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), t.text);
                let h = (v.filter(|x| *x > 0).unwrap_or(25) as f32 / 100.0 * 1.6).max(1.0);
                ui.painter().rect_filled(Rect::from_min_size(Pos2::new(r.right() - 120.0, r.center().y - h / 2.0), vec2(104.0, h)), 1.0, t.text);
                if resp.clicked() {
                    s.sel = *v;
                }
                if resp.double_clicked() {
                    dbl = true;
                }
            }
        });
        ui.checkbox(&mut s.display, "Display lineweight");
        let o = self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[]);
        if dbl {
            return Out::Ok;
        }
        o
    }
}

// ---------------------------------------------------------------- insert

pub struct InsertState {
    pub names: Vec<String>,
    pub sel: Option<String>,
}

impl InsertState {
    pub fn new(names: Vec<String>) -> InsertState {
        InsertState { sel: names.first().cloned(), names }
    }
}

impl CadForgeApp {
    pub(super) fn ui_insert(&mut self, ui: &mut egui::Ui, s: &mut InsertState) -> Out {
        let t = self.t;
        if s.names.is_empty() {
            self.note(ui, "No blocks are defined in this drawing. Use BLOCK to create one.");
            return self.ok_cancel(ui, None, Some("Close"), &[]);
        }
        self.note(ui, "Choose a block definition to insert.");
        let mut dbl = false;
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for n in &s.names {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                let sel = s.sel.as_deref() == Some(n.as_str());
                if sel {
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.accent_soft);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(2), t.hover);
                }
                ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, n, egui::FontId::proportional(12.5), t.text);
                if resp.clicked() {
                    s.sel = Some(n.clone());
                }
                if resp.double_clicked() {
                    dbl = true;
                }
            }
        });
        let o = self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[]);
        if dbl {
            return Out::Ok;
        }
        o
    }
}

// ---------------------------------------------------------------- hatch settings

pub struct HatchDlgState {
    pub ready: bool,
    pub pattern: String,
    pub scale: String,
    pub angle: String,
    pub color: Option<Color>,
    pub token: u32,
}

impl HatchDlgState {
    pub fn new() -> HatchDlgState {
        HatchDlgState { ready: false, pattern: "ANSI31".into(), scale: "1".into(), angle: "0".into(), color: None, token: 0 }
    }
    pub fn load(&mut self, app: &mut CadForgeApp) {
        if let Some(s) = app.h.borrow().hatch_session.as_ref() {
            let m = s.settings.borrow();
            self.pattern = m.pattern.clone();
            self.scale = cadforge_engine::sysvars::fmt_num(m.scale);
            self.angle = cadforge_engine::sysvars::fmt_num(m.angle);
            self.color = m.color;
        }
        app.token_counter += 1;
        self.token = app.token_counter;
        self.ready = true;
    }
    pub fn apply(&self, app: &mut CadForgeApp) {
        let Some(sc) = self.scale.trim().parse::<f64>().ok().filter(|v| *v > 0.0) else {
            app.toast("The hatch scale must be positive.");
            return;
        };
        if let Some(s) = app.h.borrow().hatch_session.as_ref() {
            let mut m = s.settings.borrow_mut();
            m.pattern = self.pattern.clone();
            m.scale = sc;
            m.angle = self.angle.trim().parse().unwrap_or(0.0);
            m.color = self.color;
        }
        app.refresh_hatch_preview();
    }
}

impl Default for HatchDlgState {
    fn default() -> Self {
        Self::new()
    }
}

impl CadForgeApp {
    pub(super) fn ui_hatch(&mut self, ui: &mut egui::Ui, s: &mut HatchDlgState) -> Out {
        if let Some((tok, c)) = self.ret_color {
            if tok == s.token {
                s.color = c;
                self.ret_color = None;
            }
        }
        let pats: Vec<(&'static str, &'static str)> = cadforge_core::hatch::all_patterns().iter().map(|p| (p.name, p.desc)).collect();
        egui::Grid::new("hd").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
            ui.label("Pattern");
            egui::ComboBox::from_id_salt("hd-pat").selected_text(&s.pattern).show_ui(ui, |ui| {
                for (n, _) in &pats {
                    ui.selectable_value(&mut s.pattern, n.to_string(), *n);
                }
            });
            ui.end_row();
            ui.label("Scale");
            self.num_edit(ui, &mut s.scale, 90.0);
            ui.end_row();
            ui.label("Angle");
            self.num_edit(ui, &mut s.angle, 90.0);
            ui.end_row();
            ui.label("Color");
            ui.horizontal(|ui| {
                self.color_chip(ui, s.color);
                if self.btn(ui, &describe_color(s.color), false).clicked() {
                    self.dialog = Some(super::Dialog::color_for(s.color, true, ColorTarget::Field(s.token)));
                }
            });
            ui.end_row();
        });
        if let Some((_, d)) = pats.iter().find(|p| p.0 == s.pattern) {
            self.note(ui, d);
        }
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}
