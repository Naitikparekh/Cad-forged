//! Dimension style manager and editor.
use super::Out;
use crate::app::CadForgeApp;
use cadforge_core::style::DimStyle;
use eframe::egui::{self, vec2, CornerRadius, RichText, Sense};

pub struct DimStyleState {
    pub sel: String,
    pub new_name: String,
}

impl DimStyleState {
    pub fn new(app: &CadForgeApp) -> DimStyleState {
        DimStyleState {
            sel: app
                .h
                .borrow()
                .doc()
                .map(|d| d.settings.cur_dim_style.clone())
                .unwrap_or_default(),
            new_name: String::new(),
        }
    }
}

pub struct DimEditState {
    pub title: String,
    pub name: String,
    pub tab: usize,
    pub s: DimStyle,
    pub txt: String,
    pub gap: String,
    pub asz: String,
    pub exo: String,
    pub exe: String,
    pub cen: String,
    pub scale: String,
    pub dec: String,
    pub lfac: String,
}

fn n(v: f64) -> String {
    cadforge_engine::sysvars::fmt_num(v)
}

impl DimEditState {
    pub fn new(name: &str, s: DimStyle) -> DimEditState {
        DimEditState {
            title: format!("Modify Dimension Style: {name}"),
            name: name.into(),
            tab: 0,
            txt: n(s.txt),
            gap: n(s.gap),
            asz: n(s.asz),
            exo: n(s.exo),
            exe: n(s.exe),
            cen: n(s.cen),
            scale: n(s.scale),
            dec: s.dec.to_string(),
            lfac: n(s.lfac),
            s,
        }
    }
    pub fn apply(&self, app: &mut CadForgeApp) {
        let p = |s: &str| s.trim().parse::<f64>().ok();
        let pos = |s: &str| p(s).filter(|v| *v > 0.0);
        let (Some(txt), Some(asz), Some(scale), Some(lfac)) = (
            pos(&self.txt),
            pos(&self.asz),
            pos(&self.scale),
            pos(&self.lfac),
        ) else {
            app.toast("Text height, arrow size, scale and scale factor must be positive.");
            return;
        };
        let mut st = self.s.clone();
        st.txt = txt;
        st.asz = asz;
        st.scale = scale;
        st.lfac = lfac;
        st.gap = p(&self.gap).unwrap_or(0.0);
        st.exo = p(&self.exo).unwrap_or(0.0);
        st.exe = p(&self.exe).unwrap_or(0.0);
        st.cen = p(&self.cen).unwrap_or(0.0);
        st.dec = self
            .dec
            .trim()
            .parse::<f64>()
            .map_or(0, |v| v.round().clamp(0.0, 8.0) as i32);
        let name = self.name.clone();
        app.edit_doc("Dimension style", move |d| {
            d.will_change();
            if let Some(slot) = d.settings.dim_styles.get_mut(&name) {
                *slot = st;
            }
            d.touch();
        });
    }
}

impl CadForgeApp {
    pub(super) fn ui_dimstyle(
        &mut self,
        ui: &mut egui::Ui,
        s: &mut DimStyleState,
    ) -> (Out, Option<DimEditState>) {
        let t = self.t;
        let (styles, cur): (Vec<String>, String) = {
            let a = self.h.borrow();
            match a.doc() {
                Some(d) => (
                    d.settings.dim_styles.keys().cloned().collect(),
                    d.settings.cur_dim_style.clone(),
                ),
                None => (vec![], String::new()),
            }
        };
        if !styles.contains(&s.sel) {
            s.sel = cur.clone();
        }
        let mut push: Option<DimEditState> = None;
        ui.columns(2, |cols| {
            cols[0].label(RichText::new("Styles").small().color(t.text3));
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .id_salt("dsm-list")
                .show(&mut cols[0], |ui| {
                    for n in &styles {
                        let (r, resp) = ui
                            .allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                        if *n == s.sel {
                            ui.painter()
                                .rect_filled(r, CornerRadius::same(2), t.accent_soft);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(2), t.hover);
                        }
                        ui.painter().text(
                            r.left_center() + vec2(8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            n,
                            egui::FontId::proportional(12.5),
                            t.text,
                        );
                        if *n == cur {
                            ui.painter().text(
                                r.right_center() - vec2(8.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                "(current)",
                                egui::FontId::proportional(11.0),
                                t.text3,
                            );
                        }
                        if resp.clicked() {
                            s.sel = n.clone();
                        }
                    }
                });
            let ui = &mut cols[1];
            ui.label(RichText::new("Preview").small().color(t.text3));
            let st = self
                .h
                .borrow()
                .doc()
                .and_then(|d| d.settings.dim_styles.get(&s.sel).cloned());
            if let Some(st) = &st {
                egui::Frame::new()
                    .fill(t.input)
                    .stroke(egui::Stroke::new(1.0, t.border))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(RichText::new(&s.sel).strong());
                        ui.label(format!(
                            "Text height {} · Arrow size {} · {} decimals",
                            n(st.txt),
                            n(st.asz),
                            st.dec
                        ));
                        ui.label(format!(
                            "Extension offset {} · beyond {} · Scale {}",
                            n(st.exo),
                            n(st.exe),
                            n(st.scale)
                        ));
                    });
            }
            ui.add_space(6.0);
            if self.btn(ui, "Set Current", false).clicked() {
                let sel = s.sel.clone();
                self.edit_doc("Dimension style", move |d| {
                    d.will_change();
                    d.settings.cur_dim_style = sel;
                    d.touch();
                });
            }
            if self.btn(ui, "Modify...", false).clicked() {
                if let Some(st) = st.clone() {
                    push = Some(DimEditState::new(&s.sel, st));
                }
            }
            if self.btn(ui, "Delete", false).clicked() {
                if styles.len() < 2 || s.sel == cur {
                    self.toast("The current style and the last style cannot be deleted.");
                } else {
                    let sel = s.sel.clone();
                    self.edit_doc("Dimension style", move |d| {
                        d.will_change();
                        d.settings.dim_styles.shift_remove(&sel);
                        d.touch();
                    });
                    s.sel = cur.clone();
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut s.new_name)
                    .hint_text("New style name")
                    .desired_width(180.0),
            );
            if self.btn(ui, "Create", false).clicked() {
                let nn = s.new_name.trim().to_string();
                if nn.is_empty() || styles.contains(&nn) {
                    self.toast("Enter a unique style name.");
                } else {
                    let base = self
                        .h
                        .borrow()
                        .doc()
                        .and_then(|d| d.settings.dim_styles.get(&s.sel).cloned());
                    let nn2 = nn.clone();
                    self.edit_doc("Dimension style", move |d| {
                        d.will_change();
                        let mut st = base.unwrap_or_default();
                        st.name = nn2.clone();
                        d.settings.dim_styles.insert(nn2, st);
                        d.touch();
                    });
                    s.sel = nn;
                    s.new_name.clear();
                }
            }
        });
        (self.ok_cancel(ui, Some("Close"), None, &[]), push)
    }

    pub(super) fn ui_dimstyle_edit(&mut self, ui: &mut egui::Ui, s: &mut DimEditState) -> Out {
        self.tab_strip(
            ui,
            &[
                "Lines",
                "Symbols and Arrows",
                "Text",
                "Fit",
                "Primary Units",
            ],
            &mut s.tab,
        );
        egui::Grid::new("dse")
            .num_columns(2)
            .spacing([14.0, 8.0])
            .show(ui, |ui| match s.tab {
                0 => {
                    ui.label("Extend beyond dim lines");
                    self.num_edit(ui, &mut s.exe, 90.0);
                    ui.end_row();
                    ui.label("Offset from origin");
                    self.num_edit(ui, &mut s.exo, 90.0);
                    ui.end_row();
                }
                1 => {
                    ui.label("Arrowhead");
                    let names = [
                        ("closed", "Closed filled"),
                        ("open", "Open"),
                        ("tick", "Architectural tick"),
                        ("dot", "Dot"),
                        ("none", "None"),
                    ];
                    let cur = names
                        .iter()
                        .find(|x| x.0 == s.s.blk)
                        .map_or("Closed filled", |x| x.1);
                    egui::ComboBox::from_id_salt("dse-blk")
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            for (v, l) in names {
                                ui.selectable_value(&mut s.s.blk, v.to_string(), l);
                            }
                        });
                    ui.end_row();
                    ui.label("Arrow size");
                    self.num_edit(ui, &mut s.asz, 90.0);
                    ui.end_row();
                    ui.label("Center mark size");
                    self.num_edit(ui, &mut s.cen, 90.0);
                    ui.end_row();
                }
                2 => {
                    ui.label("Text height");
                    self.num_edit(ui, &mut s.txt, 90.0);
                    ui.end_row();
                    ui.label("Vertical placement");
                    egui::ComboBox::from_id_salt("dse-tad")
                        .selected_text(if s.s.tad {
                            "Above dimension line"
                        } else {
                            "Centered"
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut s.s.tad, true, "Above dimension line");
                            ui.selectable_value(&mut s.s.tad, false, "Centered");
                        });
                    ui.end_row();
                    ui.label("Offset from dim line");
                    self.num_edit(ui, &mut s.gap, 90.0);
                    ui.end_row();
                }
                3 => {
                    ui.label("Overall scale");
                    self.num_edit(ui, &mut s.scale, 90.0);
                    ui.end_row();
                }
                _ => {
                    ui.label("Precision (decimal places)");
                    self.num_edit(ui, &mut s.dec, 90.0);
                    ui.end_row();
                    ui.label("Linear scale factor");
                    self.num_edit(ui, &mut s.lfac, 90.0);
                    ui.end_row();
                    ui.label("Prefix");
                    ui.add(egui::TextEdit::singleline(&mut s.s.pre).desired_width(90.0));
                    ui.end_row();
                    ui.label("Suffix");
                    ui.add(egui::TextEdit::singleline(&mut s.s.suf).desired_width(90.0));
                    ui.end_row();
                    ui.label("");
                    let mut trail = s.s.zin & 8 != 0;
                    if ui.checkbox(&mut trail, "Suppress trailing zeros").changed() {
                        s.s.zin = if trail { s.s.zin | 8 } else { s.s.zin & !8 };
                    }
                    ui.end_row();
                }
            });
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}
