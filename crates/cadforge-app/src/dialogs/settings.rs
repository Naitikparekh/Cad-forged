//! Settings dialogs: drafting settings, options, units, text style and quick select.
use super::Out;
use crate::app::CadForgeApp;
use cadforge_engine::parse::{os_bit, os_full};
use eframe::egui::{self, RichText};

const OSNAP_MODES: [&str; 10] = ["END", "MID", "CEN", "NOD", "QUA", "INT", "INS", "PER", "TAN", "NEA"];

fn fnum(v: f64) -> String {
    cadforge_engine::sysvars::fmt_num(v)
}

fn parse_pos(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| *v > 0.0)
}

// ---------------------------------------------------------------- drafting settings

pub struct DsState {
    pub tab: usize,
    pub ready: bool,
    pub snap_on: bool,
    pub snap_x: String,
    pub snap_y: String,
    pub grid_on: bool,
    pub grid_x: String,
    pub grid_y: String,
    pub major: String,
    pub iso: bool,
    pub polar_on: bool,
    pub inc: f64,
    pub add_angles: String,
    pub otrack: bool,
    pub os_on: bool,
    pub modes: [bool; 10],
    pub dyn_on: bool,
    pub dyn_ptr: bool,
    pub dyn_dim: bool,
}

impl DsState {
    pub fn new(tab: &str) -> DsState {
        let tab = match tab {
            "polar" => 1,
            "osnap" => 2,
            "dyn" => 3,
            _ => 0,
        };
        DsState { tab, ready: false, snap_on: false, snap_x: String::new(), snap_y: String::new(), grid_on: true, grid_x: String::new(), grid_y: String::new(), major: String::new(), iso: false, polar_on: false, inc: 90.0, add_angles: String::new(), otrack: true, os_on: true, modes: [false; 10], dyn_on: true, dyn_ptr: true, dyn_dim: true }
    }
    pub fn load(&mut self, app: &CadForgeApp) {
        let a = app.h.borrow();
        let sv = &a.sysvars;
        self.snap_on = sv.int("SNAPMODE") != 0;
        self.snap_x = fnum(sv.real("SNAPUNIT_X"));
        self.snap_y = fnum(sv.real("SNAPUNIT_Y"));
        self.grid_on = sv.int("GRIDMODE") != 0;
        self.grid_x = fnum(sv.real("GRIDUNIT_X"));
        self.grid_y = fnum(sv.real("GRIDUNIT_Y"));
        self.major = fnum(sv.real("GRIDMAJOR"));
        self.iso = sv.isometric();
        self.polar_on = sv.int("POLARMODE") != 0;
        self.inc = sv.real("POLARANG");
        self.add_angles = sv.string("POLARADDANG");
        self.otrack = sv.int("OTRACKMODE") != 0;
        let os = sv.int("OSMODE") as u32;
        self.os_on = os & 16384 == 0;
        for (i, k) in OSNAP_MODES.iter().enumerate() {
            self.modes[i] = os & os_bit(k) != 0;
        }
        let d = sv.int("DYNMODE");
        self.dyn_on = d > 0;
        self.dyn_ptr = (d.abs() & 1) != 0 || d == 0;
        self.dyn_dim = (d.abs() & 2) != 0 || d == 0;
        self.ready = true;
    }
    pub fn apply(&self, app: &mut CadForgeApp) {
        let (Some(sx), Some(sy), Some(gx), Some(gy)) = (parse_pos(&self.snap_x), parse_pos(&self.snap_y), parse_pos(&self.grid_x), parse_pos(&self.grid_y)) else {
            app.toast("Spacing values must be positive numbers.");
            return;
        };
        app.set_sv("SNAPMODE", self.snap_on as i64 as f64);
        app.set_sv("SNAPUNIT_X", sx);
        app.set_sv("SNAPUNIT_Y", sy);
        app.set_sv("GRIDMODE", self.grid_on as i64 as f64);
        app.set_sv("GRIDUNIT_X", gx);
        app.set_sv("GRIDUNIT_Y", gy);
        app.set_sv("GRIDMAJOR", self.major.trim().parse::<f64>().ok().map_or(5.0, |v| v.round().max(1.0)));
        app.set_sv("SNAPSTYL", self.iso as i64 as f64);
        app.set_sv("POLARANG", self.inc);
        let extra: Vec<String> = cadforge_engine::sysvars::polar_add_angles(&self.add_angles).into_iter().map(fnum).collect();
        let _ = app.h.borrow_mut().setvar("POLARADDANG", extra.join(";"));
        app.set_sv("POLARMODE", self.polar_on as i64 as f64);
        app.set_sv("OTRACKMODE", self.otrack as i64 as f64);
        if self.polar_on {
            app.set_sv("ORTHOMODE", 0.0);
        }
        let mut m: u32 = if self.os_on { 0 } else { 16384 };
        for (i, k) in OSNAP_MODES.iter().enumerate() {
            if self.modes[i] {
                m |= os_bit(k);
            }
        }
        app.set_sv("OSMODE", m as f64);
        let bits = (self.dyn_ptr as i64) | ((self.dyn_dim as i64) << 1);
        let bits = if bits == 0 { 3 } else { bits };
        app.set_sv("DYNMODE", if self.dyn_on { bits as f64 } else { -(bits as f64) });
    }
}

impl CadForgeApp {
    pub(super) fn ui_dsettings(&mut self, ui: &mut egui::Ui, s: &mut DsState) -> Out {
        self.tab_strip(ui, &["Snap and Grid", "Polar Tracking", "Object Snap", "Dynamic Input"], &mut s.tab);
        match s.tab {
            0 => {
                ui.columns(2, |cols| {
                    self.group(&mut cols[0], "Snap spacing", |ui| {
                        ui.checkbox(&mut s.snap_on, "Snap On (F9)");
                        egui::Grid::new("ds-snap").num_columns(2).show(ui, |ui| {
                            ui.label("Snap X spacing");
                            self.num_edit(ui, &mut s.snap_x, 80.0);
                            ui.end_row();
                            ui.label("Snap Y spacing");
                            self.num_edit(ui, &mut s.snap_y, 80.0);
                            ui.end_row();
                        });
                    });
                    self.group(&mut cols[1], "Grid", |ui| {
                        ui.checkbox(&mut s.grid_on, "Grid On (F7)");
                        egui::Grid::new("ds-grid").num_columns(2).show(ui, |ui| {
                            ui.label("Grid X spacing");
                            self.num_edit(ui, &mut s.grid_x, 80.0);
                            ui.end_row();
                            ui.label("Grid Y spacing");
                            self.num_edit(ui, &mut s.grid_y, 80.0);
                            ui.end_row();
                            ui.label("Major line every");
                            self.num_edit(ui, &mut s.major, 80.0);
                            ui.end_row();
                        });
                    });
                });
                self.group(ui, "Snap type", |ui| {
                    ui.radio_value(&mut s.iso, false, "Rectangular snap");
                    ui.radio_value(&mut s.iso, true, "Isometric snap (F5 switches the isoplane)");
                });
            }
            1 => {
                self.group(ui, "Polar angle settings", |ui| {
                    ui.checkbox(&mut s.polar_on, "Polar Tracking On (F10)");
                    ui.horizontal(|ui| {
                        ui.label("Increment angle");
                        egui::ComboBox::from_id_salt("ds-inc").selected_text(fnum(s.inc)).show_ui(ui, |ui| {
                            for v in [90.0, 45.0, 30.0, 22.5, 18.0, 15.0, 10.0, 5.0] {
                                ui.selectable_value(&mut s.inc, v, fnum(v));
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Additional angles");
                        ui.add(egui::TextEdit::singleline(&mut s.add_angles).desired_width(140.0).hint_text("e.g. 5;35"));
                    });
                });
                self.group(ui, "Object snap tracking", |ui| {
                    ui.checkbox(&mut s.otrack, "Object Snap Tracking On (F11)");
                });
            }
            2 => {
                ui.checkbox(&mut s.os_on, "Object Snap On (F3)");
                self.group(ui, "Object Snap modes", |ui| {
                    egui::Grid::new("ds-os").num_columns(2).spacing([40.0, 4.0]).show(ui, |ui| {
                        for (i, k) in OSNAP_MODES.iter().enumerate() {
                            ui.checkbox(&mut s.modes[i], os_full(k));
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    if self.btn(ui, "Select All", false).clicked() {
                        s.modes = [true; 10];
                    }
                    if self.btn(ui, "Clear All", false).clicked() {
                        s.modes = [false; 10];
                    }
                });
            }
            _ => {
                ui.checkbox(&mut s.dyn_on, "Enable Dynamic Input (F12)");
                self.group(ui, "Pointer input", |ui| {
                    ui.checkbox(&mut s.dyn_ptr, "Enable Pointer Input");
                });
                self.group(ui, "Dimension input", |ui| {
                    ui.checkbox(&mut s.dyn_dim, "Enable Dimension Input where possible");
                });
            }
        }
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}

// ---------------------------------------------------------------- options

pub struct OptionsState {
    pub tab: usize,
    pub dark: bool,
    pub cursor: String,
    pub lw: bool,
    pub cube: bool,
    pub nav: bool,
    pub vpc: bool,
    pub ucs: bool,
    pub tips: bool,
    pub pick: String,
    pub grip: String,
    pub grips: bool,
    pub pickfirst: bool,
    pub apt: String,
    pub auto: bool,
    pub sc_default: bool,
    pub sc_edit: bool,
    pub sc_cmd: bool,
    /// LOCALE: the interface language.
    pub locale: String,
}

impl OptionsState {
    pub fn new(app: &CadForgeApp) -> OptionsState {
        let a = app.h.borrow();
        let sv = &a.sysvars;
        let sc = sv.int("SHORTCUTMENU");
        OptionsState {
            tab: 0,
            dark: app.ui.dark,
            cursor: fnum(sv.real("CURSORSIZE")),
            lw: sv.int("LWDISPLAY") != 0,
            cube: sv.int("NAVVCUBE") != 0,
            nav: sv.int("NAVBAR") != 0,
            vpc: sv.int("VPCONTROL") != 0,
            ucs: sv.int("UCSICON") != 0,
            tips: sv.int("TOOLTIPS") != 0,
            pick: fnum(sv.real("PICKBOX")),
            grip: fnum(sv.real("GRIPSIZE")),
            grips: sv.int("GRIPS") != 0,
            pickfirst: sv.int("PICKFIRST") != 0,
            apt: fnum(sv.real("APERTURE")),
            auto: sv.int("AUTOCOMPLETE") != 0,
            sc_default: sc & 1 != 0,
            sc_edit: sc & 2 != 0,
            sc_cmd: sc & 4 != 0,
            locale: sv.string("LOCALE"),
        }
    }
    pub fn apply(&self, app: &mut CadForgeApp) {
        let n = |s: &str, lo: f64, hi: f64| s.trim().parse::<f64>().map_or(lo, |v| v.round().clamp(lo, hi));
        app.pending_theme = Some(self.dark);
        app.set_sv("CURSORSIZE", n(&self.cursor, 1.0, 100.0));
        app.set_sv("LWDISPLAY", self.lw as i64 as f64);
        app.set_sv("NAVVCUBE", self.cube as i64 as f64);
        app.set_sv("NAVBAR", self.nav as i64 as f64);
        app.set_sv("VPCONTROL", self.vpc as i64 as f64);
        app.set_sv("UCSICON", self.ucs as i64 as f64);
        app.set_sv("TOOLTIPS", self.tips as i64 as f64);
        app.set_sv("PICKBOX", n(&self.pick, 0.0, 50.0));
        app.set_sv("GRIPSIZE", n(&self.grip, 1.0, 20.0));
        app.set_sv("GRIPS", if self.grips { 2.0 } else { 0.0 });
        app.set_sv("PICKFIRST", self.pickfirst as i64 as f64);
        app.set_sv("APERTURE", n(&self.apt, 1.0, 50.0));
        app.set_sv("AUTOCOMPLETE", self.auto as i64 as f64);
        let sc = (self.sc_default as i64) | ((self.sc_edit as i64) << 1) | ((self.sc_cmd as i64) << 2) | 8;
        app.set_sv("SHORTCUTMENU", sc as f64);
        let _ = app.h.borrow_mut().setvar("LOCALE", self.locale.as_str());
    }
}

impl CadForgeApp {
    pub(super) fn ui_options(&mut self, ui: &mut egui::Ui, s: &mut OptionsState) -> Out {
        self.tab_strip(ui, &["Display", "Selection", "Drafting", "User Preferences"], &mut s.tab);
        match s.tab {
            0 => {
                self.group(ui, "Colour scheme", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Theme");
                        ui.selectable_value(&mut s.dark, true, "Dark");
                        ui.selectable_value(&mut s.dark, false, "Light");
                    });
                });
                self.group(ui, "Language", |ui| {
                    ui.horizontal(|ui| {
                        for (_, code, name) in crate::i18n::LANGS {
                            ui.selectable_value(&mut s.locale, code.to_string(), name);
                        }
                    });
                });
                self.group(ui, "Crosshair", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Size (% of screen)");
                        self.num_edit(ui, &mut s.cursor, 60.0);
                    });
                });
                self.group(ui, "Display elements", |ui| {
                    ui.checkbox(&mut s.lw, "Display lineweights in model space");
                    ui.checkbox(&mut s.cube, "Show ViewCube");
                    ui.checkbox(&mut s.nav, "Show navigation bar");
                    ui.checkbox(&mut s.vpc, "Show viewport controls");
                    ui.checkbox(&mut s.ucs, "Show UCS icon");
                    ui.checkbox(&mut s.tips, "Show rich tooltips");
                });
            }
            1 => {
                self.group(ui, "Pickbox and grips", |ui| {
                    egui::Grid::new("opt-pick").num_columns(2).show(ui, |ui| {
                        ui.label("Pickbox size (px)");
                        self.num_edit(ui, &mut s.pick, 60.0);
                        ui.end_row();
                        ui.label("Grip size (px)");
                        self.num_edit(ui, &mut s.grip, 60.0);
                        ui.end_row();
                    });
                    ui.checkbox(&mut s.grips, "Enable grips");
                });
                self.group(ui, "Selection modes", |ui| {
                    ui.checkbox(&mut s.pickfirst, "Allow selection before the command (noun/verb)");
                });
            }
            2 => {
                self.group(ui, "AutoSnap", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Aperture size (px)");
                        self.num_edit(ui, &mut s.apt, 60.0);
                    });
                });
                self.group(ui, "Command line", |ui| {
                    ui.checkbox(&mut s.auto, "AutoComplete command names");
                });
            }
            _ => {
                self.group(ui, "Right-click customization", |ui| {
                    ui.checkbox(&mut s.sc_default, "Default mode menu (no objects selected)");
                    ui.checkbox(&mut s.sc_edit, "Edit mode menu (objects selected)");
                    ui.checkbox(&mut s.sc_cmd, "Command mode menu (right-click shows a menu; otherwise it acts as Enter)");
                });
                if self.btn(ui, "Edit Aliases...", false).clicked() {
                    self.dialog = Some(super::Dialog::aliasedit());
                }
            }
        }
        let o = self.ok_cancel(ui, Some("OK"), Some("Cancel"), &["Reset"]);
        if o == Out::Btn(0) {
            let d = cadforge_engine::sysvars::pref_defaults();
            for (k, dst) in [("CURSORSIZE", &mut s.cursor), ("PICKBOX", &mut s.pick), ("GRIPSIZE", &mut s.grip), ("APERTURE", &mut s.apt)] {
                if let Some(v) = d.get(k).and_then(|v| v.num()) {
                    *dst = fnum(v);
                }
            }
            return Out::Open;
        }
        o
    }
}

// ---------------------------------------------------------------- units

pub struct UnitsState {
    pub precision: i32,
    pub ang: i32,
    pub ins: i32,
}

impl UnitsState {
    pub fn new(app: &CadForgeApp) -> UnitsState {
        let a = app.h.borrow();
        let u = a.doc().map(|d| d.settings.units.clone()).unwrap_or_default();
        UnitsState { precision: u.precision, ang: u.ang_precision, ins: u.ins_units }
    }
    pub fn apply(&self, app: &mut CadForgeApp) {
        let (p, a, i) = (self.precision, self.ang, self.ins);
        app.edit_doc("Units", move |d| {
            d.will_change();
            d.settings.units.precision = p;
            d.settings.units.ang_precision = a;
            d.settings.units.ins_units = i;
            d.touch();
        });
    }
}

fn prec_label(v: i32) -> String {
    if v == 0 {
        "0".into()
    } else {
        format!("0.{}", "0".repeat(v as usize))
    }
}

impl CadForgeApp {
    pub(super) fn ui_units(&mut self, ui: &mut egui::Ui, s: &mut UnitsState) -> Out {
        ui.columns(2, |cols| {
            self.group(&mut cols[0], "Length", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Type");
                    ui.label(RichText::new("Decimal").strong());
                });
                ui.horizontal(|ui| {
                    ui.label("Precision");
                    egui::ComboBox::from_id_salt("un-prec").selected_text(prec_label(s.precision)).show_ui(ui, |ui| {
                        for v in 0..=8 {
                            ui.selectable_value(&mut s.precision, v, prec_label(v));
                        }
                    });
                });
            });
            self.group(&mut cols[1], "Angle", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Type");
                    ui.label(RichText::new("Decimal Degrees").strong());
                });
                ui.horizontal(|ui| {
                    ui.label("Precision");
                    egui::ComboBox::from_id_salt("un-ang").selected_text(prec_label(s.ang)).show_ui(ui, |ui| {
                        for v in 0..=4 {
                            ui.selectable_value(&mut s.ang, v, prec_label(v));
                        }
                    });
                });
                self.note(ui, "Angles measure counter-clockwise from the +X axis.");
            });
        });
        self.group(ui, "Insertion scale", |ui| {
            ui.horizontal(|ui| {
                ui.label("Units to scale inserted content");
                let names = [(0, "Unitless"), (1, "Inches"), (2, "Feet"), (4, "Millimeters"), (5, "Centimeters"), (6, "Meters")];
                let cur = names.iter().find(|n| n.0 == s.ins).map_or("Unitless", |n| n.1);
                egui::ComboBox::from_id_salt("un-ins").selected_text(cur).show_ui(ui, |ui| {
                    for (v, l) in names {
                        ui.selectable_value(&mut s.ins, v, l);
                    }
                });
            });
        });
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}

// ---------------------------------------------------------------- text style

pub struct TextStyleState {
    pub height: String,
}

impl TextStyleState {
    pub fn new(app: &CadForgeApp) -> TextStyleState {
        TextStyleState { height: app.h.borrow().doc().map_or("2.5".into(), |d| fnum(d.settings.text_height)) }
    }
    pub fn apply(&self, app: &mut CadForgeApp) {
        let Some(v) = parse_pos(&self.height) else {
            app.toast("The text height must be positive.");
            return;
        };
        app.edit_doc("Text style", move |d| {
            d.will_change();
            d.settings.text_height = v;
            d.touch();
        });
    }
}

impl CadForgeApp {
    pub(super) fn ui_textstyle(&mut self, ui: &mut egui::Ui, s: &mut TextStyleState) -> Out {
        egui::Grid::new("ts").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
            ui.label("Style name");
            ui.label(RichText::new("Standard").strong());
            ui.end_row();
            ui.label("Font");
            ui.label("Sans-serif (Liberation Sans)");
            ui.end_row();
            ui.label("Default height");
            self.num_edit(ui, &mut s.height, 90.0);
            ui.end_row();
        });
        self.note(ui, "Text is drawn with a Helvetica-compatible sans-serif font. The default height is used by TEXT and MTEXT.");
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}

// ---------------------------------------------------------------- quick select

pub struct QSelectState {
    pub prop: usize,
    pub neg: bool,
    pub value: String,
    pub include: bool,
    pub append: bool,
}

const QS_PROPS: [&str; 5] = ["Type", "Layer", "Color", "Linetype", "Lineweight"];

fn prop_of(e: &cadforge_core::entities::Entity, k: usize) -> String {
    match k {
        0 => e.label().to_string(),
        1 => e.layer.clone(),
        2 => cadforge_core::style::describe_color(e.color),
        3 => e.ltype.clone().unwrap_or_else(|| "ByLayer".into()),
        _ => cadforge_core::style::lw_display(e.lw),
    }
}

impl QSelectState {
    pub fn new(app: &CadForgeApp) -> QSelectState {
        let mut s = QSelectState { prop: 0, neg: false, value: String::new(), include: true, append: false };
        s.value = s.values(app).first().cloned().unwrap_or_default();
        s
    }
    fn values(&self, app: &CadForgeApp) -> Vec<String> {
        let a = app.h.borrow();
        let Some(d) = a.doc() else { return vec![] };
        let set: std::collections::BTreeSet<String> = d.entities().iter().map(|e| prop_of(e, self.prop)).collect();
        set.into_iter().collect()
    }
    pub fn apply(&self, app: &mut CadForgeApp) {
        let mut a = app.h.borrow_mut();
        let Some(ed) = a.editor_mut() else { return };
        let doc = &ed.doc;
        let hit: Vec<u64> = doc
            .entities()
            .iter()
            .filter(|e| doc.is_visible(e) && !doc.is_locked(e))
            .filter(|e| {
                let m = (prop_of(e, self.prop) == self.value) != self.neg;
                if self.include {
                    m
                } else {
                    !m
                }
            })
            .map(|e| e.id)
            .collect();
        if !self.append {
            ed.sel.clear();
        }
        let n = hit.len();
        ed.sel.extend(hit);
        a.log(&format!("{n} found"), "info");
        a.emit(cadforge_engine::app::Event::Change);
    }
}

impl CadForgeApp {
    pub(super) fn ui_qselect(&mut self, ui: &mut egui::Ui, s: &mut QSelectState) -> Out {
        let values = s.values(self);
        egui::Grid::new("qs").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
            ui.label("Apply to");
            ui.label("Entire drawing");
            ui.end_row();
            ui.label("Property");
            let before = s.prop;
            egui::ComboBox::from_id_salt("qs-prop").selected_text(QS_PROPS[s.prop]).show_ui(ui, |ui| {
                for (i, p) in QS_PROPS.iter().enumerate() {
                    ui.selectable_value(&mut s.prop, i, *p);
                }
            });
            if s.prop != before {
                s.value = String::new();
            }
            ui.end_row();
            ui.label("Operator");
            egui::ComboBox::from_id_salt("qs-op").selected_text(if s.neg { "<> Not equal to" } else { "= Equals" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.neg, false, "= Equals");
                ui.selectable_value(&mut s.neg, true, "<> Not equal to");
            });
            ui.end_row();
            ui.label("Value");
            if s.value.is_empty() || !values.contains(&s.value) {
                s.value = values.first().cloned().unwrap_or_default();
            }
            egui::ComboBox::from_id_salt("qs-val").selected_text(&s.value).show_ui(ui, |ui| {
                for v in &values {
                    ui.selectable_value(&mut s.value, v.clone(), v);
                }
            });
            ui.end_row();
            ui.label("How to apply");
            egui::ComboBox::from_id_salt("qs-how").selected_text(if s.include { "Include in new selection set" } else { "Exclude from new selection set" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.include, true, "Include in new selection set");
                ui.selectable_value(&mut s.include, false, "Exclude from new selection set");
            });
            ui.end_row();
        });
        ui.checkbox(&mut s.append, "Append to current selection set");
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}
