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

/// The STYLE dialog: the drawing's text styles, each with a font, a fixed height (0: each text has its own), a width
/// factor and an obliquing angle. OK or Apply saves the selected style; every text in it is drawn again.
pub struct TextStyleState {
    pub sel: String,
    /// The style the fields below were loaded from.
    loaded: String,
    pub font: String,
    pub height: String,
    pub width: String,
    pub oblique: String,
    pub new_name: String,
    /// TEXTSIZE: the height TEXT and MTEXT offer when the style has none.
    pub default_h: String,
    fonts: Vec<String>,
}

impl TextStyleState {
    pub fn new(app: &CadForgeApp) -> TextStyleState {
        let (sel, default_h) = app.h.borrow().doc().map_or(("Standard".into(), "2.5".into()), |d| (d.settings.cur_text().name, fnum(d.settings.text_height)));
        let mut fonts: Vec<String> = cadforge_render::fonts::COMMON_FONTS.iter().map(|s| s.to_string()).collect();
        for f in cadforge_render::fonts::system_font_names() {
            if !fonts.iter().any(|x| x.eq_ignore_ascii_case(&f)) {
                fonts.push(f);
            }
        }
        let mut s = TextStyleState { sel, loaded: String::new(), font: String::new(), height: String::new(), width: String::new(), oblique: String::new(), new_name: String::new(), default_h, fonts };
        s.load(app);
        s
    }

    fn load(&mut self, app: &CadForgeApp) {
        let st = app.h.borrow().doc().map(|d| d.settings.text_style_named(&self.sel)).unwrap_or_default();
        self.font = st.font;
        self.height = fnum(st.height);
        self.width = fnum(st.width);
        self.oblique = fnum(st.oblique);
        self.loaded = self.sel.clone();
    }

    /// The fields as a style, or what is wrong with them.
    fn style(&self) -> Result<cadforge_core::style::TextStyle, &'static str> {
        let num = |s: &str| s.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        let height = num(&self.height).filter(|v| *v >= 0.0).ok_or("The height must be 0 or more.")?;
        let width = num(&self.width).filter(|v| *v >= 0.01 && *v <= 100.0).ok_or("The width factor must be between 0.01 and 100.")?;
        let oblique = num(&self.oblique).filter(|v| v.abs() <= 85.0).ok_or("The obliquing angle must be between -85 and 85 degrees.")?;
        let font = if self.font.trim().is_empty() { "arial.ttf".to_string() } else { self.font.trim().to_string() };
        Ok(cadforge_core::style::TextStyle { name: self.loaded.clone(), font, height, width, oblique })
    }

    pub fn apply(&self, app: &mut CadForgeApp) {
        let st = match self.style() {
            Ok(st) => st,
            Err(e) => {
                app.toast(e);
                return;
            }
        };
        let dh = parse_pos(&self.default_h);
        app.edit_doc("Text style", move |d| {
            d.will_change();
            if let Some(v) = dh {
                d.settings.text_height = v;
            }
            d.settings.text_styles.retain(|k, _| !k.eq_ignore_ascii_case(&st.name));
            d.settings.text_styles.insert(st.name.clone(), st);
            d.sync_text_styles();
            d.touch();
        });
    }
}

impl CadForgeApp {
    pub(super) fn ui_textstyle(&mut self, ui: &mut egui::Ui, s: &mut TextStyleState) -> Out {
        use egui::{vec2, CornerRadius, Sense};
        let t = self.t;
        let (styles, cur, used): (Vec<String>, String, std::collections::HashSet<String>) = {
            let a = self.h.borrow();
            match a.doc() {
                Some(d) => {
                    let used = d.all_entities().filter_map(|e| if let cadforge_core::entities::Kind::Text(t) = &e.kind { t.style.as_ref().map(|s| s.to_lowercase()) } else { None }).chain(d.settings.dim_styles.values().map(|ds| ds.txsty.to_lowercase())).collect();
                    (d.settings.all_text_styles().into_iter().map(|s| s.name).collect(), d.settings.cur_text().name, used)
                }
                None => (vec!["Standard".into()], "Standard".into(), Default::default()),
            }
        };
        if !styles.iter().any(|n| n.eq_ignore_ascii_case(&s.sel)) {
            s.sel = cur.clone();
        }
        if s.loaded != s.sel {
            s.load(self);
        }
        ui.columns(2, |cols| {
            cols[0].label(RichText::new("Styles").small().color(t.text3));
            egui::ScrollArea::vertical().max_height(170.0).id_salt("ts-list").show(&mut cols[0], |ui| {
                for n in &styles {
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                    if n.eq_ignore_ascii_case(&s.sel) {
                        ui.painter().rect_filled(r, CornerRadius::same(2), t.accent_soft);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(2), t.hover);
                    }
                    ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, n, egui::FontId::proportional(12.5), t.text);
                    if n.eq_ignore_ascii_case(&cur) {
                        ui.painter().text(r.right_center() - vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, "(current)", egui::FontId::proportional(11.0), t.text3);
                    }
                    if resp.clicked() {
                        s.sel = n.clone();
                    }
                }
            });
            let ui = &mut cols[1];
            if self.btn(ui, "Set Current", false).clicked() {
                let sel = s.sel.clone();
                self.edit_doc("Text style", move |d| {
                    d.will_change();
                    d.settings.text_style = sel;
                    d.touch();
                });
            }
            if self.btn(ui, "Delete", false).clicked() {
                if s.sel.eq_ignore_ascii_case("Standard") || s.sel.eq_ignore_ascii_case(&cur) {
                    self.toast("The Standard style and the current style cannot be deleted.");
                } else if used.contains(&s.sel.to_lowercase()) {
                    self.toast("The style is used by text or a dimension style.");
                } else {
                    let sel = s.sel.clone();
                    self.edit_doc("Text style", move |d| {
                        d.will_change();
                        d.settings.text_styles.retain(|k, _| !k.eq_ignore_ascii_case(&sel));
                        d.touch();
                    });
                    s.sel = cur.clone();
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut s.new_name).hint_text("New style name").desired_width(120.0));
                if self.btn(ui, "New", false).clicked() {
                    let nn = s.new_name.trim().to_string();
                    if nn.is_empty() || styles.iter().any(|x| x.eq_ignore_ascii_case(&nn)) {
                        self.toast("Enter a unique style name.");
                    } else if let Ok(mut st) = s.style() {
                        // the new style starts as a copy of the fields shown
                        st.name = nn.clone();
                        self.edit_doc("Text style", move |d| {
                            d.will_change();
                            d.settings.text_styles.insert(st.name.clone(), st);
                            d.touch();
                        });
                        s.sel = nn;
                        s.loaded.clear();
                        s.new_name.clear();
                    }
                }
            });
        });
        ui.separator();
        egui::Grid::new("ts").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
            ui.label("Font");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut s.font).desired_width(170.0));
                egui::ComboBox::from_id_salt("ts-font").selected_text("").width(24.0).height(320.0).show_ui(ui, |ui| {
                    for f in &s.fonts {
                        if ui.selectable_label(f.eq_ignore_ascii_case(&s.font), f).clicked() {
                            s.font = f.clone();
                        }
                    }
                });
            });
            ui.end_row();
            ui.label("Height (0: set per text)");
            self.num_edit(ui, &mut s.height, 90.0);
            ui.end_row();
            ui.label("Width factor");
            self.num_edit(ui, &mut s.width, 90.0);
            ui.end_row();
            ui.label("Obliquing angle");
            self.num_edit(ui, &mut s.oblique, 90.0);
            ui.end_row();
            ui.label("Default text height");
            self.num_edit(ui, &mut s.default_h, 90.0);
            ui.end_row();
        });
        // a preview in the face the font is drawn with (the closest bundled face on screen)
        let face = cadforge_core::text::face_key(&s.font);
        let family = crate::gfx::family_of(cadforge_render::fonts::builtin_for(&face));
        let installed = matches!(cadforge_render::fonts::resolve(&face), cadforge_render::fonts::Resolved::System(..));
        egui::Frame::new().fill(t.input).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("AaBbCcDd 0123").font(egui::FontId::new(20.0, family)).color(t.text));
        });
        self.note(ui, if installed { "This font is installed: the drawing and plots use it. The preview shows the closest bundled face." } else { "This font is not installed here: it is drawn with the closest bundled face (sans-serif, serif or monospace). The drawing keeps its name." });
        match self.ok_cancel(ui, Some("OK"), Some("Cancel"), &["Apply"]) {
            // Apply saves the style and keeps the dialog open
            Out::Btn(0) => {
                s.apply(self);
                Out::Open
            }
            o => o,
        }
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
