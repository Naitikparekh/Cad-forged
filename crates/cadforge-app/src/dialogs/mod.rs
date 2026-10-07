//! Modal dialogs. A [`Dialog`] is a small state machine shown inside an egui `Modal`; command-requested dialogs
//! (`DialogReq`) answer the waiting command when closed.
use crate::app::CadForgeApp;
use cadforge_core::style::Color;
use cadforge_engine::runtime::{DialogReply, DialogReq, TextEditSpec};
use eframe::egui::{self, vec2, Color32, CornerRadius, FontFamily, FontId, Margin, RichText, Stroke};

mod colors;
mod dimstyle;
mod files;
mod plot;
mod settings;
mod simple;
mod text;

pub use colors::*;
pub use dimstyle::*;
pub use files::*;
pub use plot::*;
pub use settings::*;
pub use simple::*;
pub use text::*;

/// What happens after a confirmation dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum Then {
    None,
    CloseAll,
    CloseDrawing(usize),
}

/// Where a colour chosen in the colour dialog goes.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorTarget {
    /// Answer the command waiting for the colour dialog.
    Command,
    /// Layer / ribbon property of the selection (or the current setting).
    Property,
    /// The live hatch settings.
    Hatch,
    /// A field of another dialog, identified by a token.
    Field(u32),
    /// A layer's colour (name).
    Layer(String),
}

pub enum Kind {
    Confirm { title: String, message: String, buttons: Vec<String>, then: Then },
    SaveAs { name: String },
    About,
    Help(HelpState),
    Calc(CalcState),
    Purge(Option<PurgeState>),
    LayerStates(LayerStatesState),
    AliasEdit(AliasState),
    QSelect(Option<QSelectState>),
    TextStyle(Option<TextStyleState>),
    Units(Option<UnitsState>),
    Options(Option<OptionsState>),
    DSettings(DsState),
    DimStyle(Option<DimStyleState>),
    DimStyleEdit(Box<DimEditState>),
    Plot(PlotState),
    Color(ColorState),
    Linetype(LinetypeState),
    Lweight(LweightState),
    Insert(InsertState),
    Hatch(HatchDlgState),
    EditText(TextEditState),
    FileBrowser(FileBrowserState),
}

pub struct Dialog {
    pub kind: Kind,
    /// Opened for a waiting command: closing it answers the command.
    pub from_cmd: bool,
    pub id: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    Open,
    Ok,
    Cancel,
    Btn(usize),
}

impl Dialog {
    fn new(kind: Kind) -> Dialog {
        Dialog { kind, from_cmd: false, id: 0 }
    }
    pub fn confirm(title: &str, message: &str, buttons: &[&str], then: Then) -> Dialog {
        Dialog::new(Kind::Confirm { title: title.into(), message: message.into(), buttons: buttons.iter().map(|s| s.to_string()).collect(), then })
    }
    pub fn confirm_close(idx: usize) -> Dialog {
        Dialog::new(Kind::Confirm { title: "CadForge".into(), message: "Save changes to this drawing?".into(), buttons: vec!["Save".into(), "Don't Save".into(), "Cancel".into()], then: Then::CloseDrawing(idx) })
    }
    pub fn save_as(name: &str) -> Dialog {
        Dialog::new(Kind::SaveAs { name: name.into() })
    }
    pub fn about() -> Dialog {
        Dialog::new(Kind::About)
    }
}

impl CadForgeApp {
    pub fn open_dialog(&mut self, d: Dialog) {
        self.dialog = Some(d);
    }

    /// A command asked for a dialog: mirror it as UI.
    fn sync_command_dialog(&mut self) {
        let req = self.h.borrow().pending_dialog();
        let have_cmd_dialog = self.dialogs.iter().any(|d| d.from_cmd);
        match req {
            Some(req) if !have_cmd_dialog => {
                let kind = match req {
                    DialogReq::Insert { names } => Some(Kind::Insert(InsertState::new(names))),
                    DialogReq::Color { value, allow_by_layer } => Some(Kind::Color(ColorState::new(value, allow_by_layer, ColorTarget::Command))),
                    DialogReq::Linetype => Some(Kind::Linetype(LinetypeState::new(None, false))),
                    DialogReq::Lweight => Some(Kind::Lweight(LweightState::new(None, false))),
                    DialogReq::Hatch => Some(Kind::Hatch(HatchDlgState::new())),
                    DialogReq::DSettings { tab } => Some(Kind::DSettings(DsState::new(&tab))),
                    DialogReq::EditText(spec) => Some(Kind::EditText(TextEditState::new(spec))),
                    DialogReq::HatchEdit | DialogReq::Realtime(_) => None,
                };
                if let Some(k) = kind {
                    let mut d = Dialog::new(k);
                    d.from_cmd = true;
                    self.dialogs.push(d);
                }
            }
            None if have_cmd_dialog => {
                // the command went away (Esc): drop its dialog
                self.dialogs.retain(|d| !d.from_cmd);
            }
            _ => {}
        }
    }

    pub fn dialog_escape(&mut self) {
        if let Some(d) = self.dialogs.last_mut() {
            d.id = u64::MAX; // marks "cancel requested"
        }
    }

    pub fn show_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut d) = self.dialog.take() {
            self.dialog_counter += 1;
            d.id = self.dialog_counter;
            self.dialogs.push(d);
        }
        self.sync_command_dialog();
        if self.dialogs.is_empty() {
            return;
        }
        // draw bottom to top; only the last is interactive (Modal handles that)
        let mut finished: Vec<(usize, Out)> = vec![];
        let n = self.dialogs.len();
        for i in 0..n {
            let mut d = std::mem::replace(&mut self.dialogs[i], Dialog { kind: Kind::About, from_cmd: false, id: 0 });
            let cancel_requested = d.id == u64::MAX && i == n - 1;
            if cancel_requested {
                d.id = self.dialog_counter + 1000 + i as u64;
            }
            let out = self.show_dialog(ctx, &mut d, i == n - 1, cancel_requested);
            self.dialogs[i] = d;
            if out != Out::Open {
                finished.push((i, out));
            }
        }
        for (i, out) in finished.into_iter().rev() {
            let d = self.dialogs.remove(i);
            self.finish_dialog(ctx, d, out);
        }
    }

    fn show_dialog(&mut self, ctx: &egui::Context, d: &mut Dialog, top: bool, cancel: bool) -> Out {
        // lazy state that needs the application
        self.init_dialog(d);
        let t = self.t;
        let (title, width) = dialog_title_width(&d.kind);
        let frame = egui::Frame::new().fill(t.ribbon).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(4)).inner_margin(Margin::ZERO).shadow(egui::epaint::Shadow { offset: [0, 10], blur: 32, spread: 0, color: Color32::from_black_alpha(150) });
        let mut out = Out::Open;
        let resp = egui::Modal::new(egui::Id::new(("dialog", d.id))).backdrop_color(if top { Color32::from_black_alpha(110) } else { Color32::from_black_alpha(0) }).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            // title bar
            let (tr, _) = ui.allocate_exact_size(vec2(width, 30.0), egui::Sense::hover());
            ui.painter().rect_filled(tr, CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, t.ribbon_title);
            ui.painter().text(tr.left_center() + vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, &title, FontId::new(13.0, FontFamily::Name("bold".into())), t.text);
            let xr = egui::Rect::from_center_size(tr.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));
            let xresp = ui.interact(xr, ui.id().with("dlg-x"), egui::Sense::click());
            if xresp.hovered() {
                ui.painter().rect_filled(xr, CornerRadius::same(3), t.hover);
            }
            self.icons.paint(ui.painter(), "x", xr.shrink(5.0), &t, crate::icons::Tint::Mono(t.text2));
            if xresp.clicked() {
                out = Out::Cancel;
            }
            egui::Frame::new().inner_margin(Margin::symmetric(14, 12)).show(ui, |ui| {
                ui.set_width(width - 28.0);
                let o = self.dialog_body(ctx, ui, d);
                if o != Out::Open {
                    out = o;
                }
            });
        });
        if top && (resp.should_close() || cancel) && out == Out::Open {
            out = Out::Cancel;
        }
        out
    }

    fn init_dialog(&mut self, d: &mut Dialog) {
        match &mut d.kind {
            Kind::Purge(s) if s.is_none() => *s = Some(PurgeState::new(self)),
            Kind::QSelect(s) if s.is_none() => *s = Some(QSelectState::new(self)),
            Kind::TextStyle(s) if s.is_none() => *s = Some(TextStyleState::new(self)),
            Kind::Units(s) if s.is_none() => *s = Some(UnitsState::new(self)),
            Kind::Options(s) if s.is_none() => *s = Some(OptionsState::new(self)),
            Kind::DimStyle(s) if s.is_none() => *s = Some(DimStyleState::new(self)),
            Kind::DSettings(s) if !s.ready => s.load(self),
            Kind::Plot(s) if !s.ready => s.load(self),
            Kind::Hatch(s) if !s.ready => s.load(self),
            _ => {}
        }
    }

    fn dialog_body(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, d: &mut Dialog) -> Out {
        match &mut d.kind {
            Kind::Confirm { message, buttons, .. } => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(message.clone()));
                });
                ui.add_space(8.0);
                let mut out = Out::Open;
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for (i, b) in buttons.iter().enumerate().rev() {
                            if self.btn(ui, b, i == 0).clicked() {
                                out = Out::Btn(i);
                            }
                        }
                    });
                });
                out
            }
            Kind::SaveAs { name } => self.ui_save_as(ui, name),
            Kind::About => self.ui_about(ui),
            Kind::Help(s) => self.ui_help(ui, s),
            Kind::Calc(s) => self.ui_calc(ui, s),
            Kind::Purge(s) => self.ui_purge(ui, s.as_mut().expect("state")),
            Kind::LayerStates(s) => self.ui_layerstates(ui, s),
            Kind::AliasEdit(s) => self.ui_aliasedit(ui, s),
            Kind::QSelect(s) => self.ui_qselect(ui, s.as_mut().expect("state")),
            Kind::TextStyle(s) => self.ui_textstyle(ui, s.as_mut().expect("state")),
            Kind::Units(s) => self.ui_units(ui, s.as_mut().expect("state")),
            Kind::Options(s) => self.ui_options(ui, s.as_mut().expect("state")),
            Kind::DSettings(s) => self.ui_dsettings(ui, s),
            Kind::DimStyle(s) => {
                let (o, push) = self.ui_dimstyle(ui, s.as_mut().expect("state"));
                if let Some(p) = push {
                    self.dialog = Some(Dialog::new(Kind::DimStyleEdit(Box::new(p))));
                }
                o
            }
            Kind::DimStyleEdit(s) => self.ui_dimstyle_edit(ui, s),
            Kind::Plot(s) => self.ui_plot(ctx, ui, s),
            Kind::Color(s) => self.ui_color(ui, s),
            Kind::Linetype(s) => self.ui_linetype(ui, s),
            Kind::Lweight(s) => self.ui_lweight(ui, s),
            Kind::Insert(s) => self.ui_insert(ui, s),
            Kind::Hatch(s) => self.ui_hatch(ui, s),
            Kind::EditText(s) => self.ui_edit_text(ui, s),
            Kind::FileBrowser(s) => self.ui_file_browser(ui, s),
        }
    }

    /// Apply the result of a closed dialog.
    fn finish_dialog(&mut self, _ctx: &egui::Context, d: Dialog, out: Out) {
        let from_cmd = d.from_cmd;
        let mut reply: Option<DialogReply> = None;
        let ok = out == Out::Ok;
        match d.kind {
            Kind::Confirm { then, .. } => match (then, out) {
                (Then::CloseAll, Out::Btn(0)) => {
                    let n = self.h.borrow().editors.len();
                    for i in (0..n).rev() {
                        self.h.close_editor(i);
                    }
                    self.ui.showing_start = true;
                }
                (Then::CloseDrawing(idx), Out::Btn(0)) => {
                    self.h.borrow_mut().set_active(idx);
                    let name = self.h.borrow().doc().and_then(|d| d.file_name.clone());
                    match name {
                        Some(n) => {
                            self.write_doc(&n);
                            self.h.close_editor(idx);
                        }
                        None => {
                            // needs a file name: ask, then the user closes again
                            let n = self.h.borrow().doc().map(|d| format!("{}{}", d.name, cadforge_core::io::FILE_EXT)).unwrap_or_default();
                            self.dialog = Some(Dialog::save_as(&n));
                        }
                    }
                    if self.h.borrow().editors.is_empty() {
                        self.ui.showing_start = true;
                    }
                }
                (Then::CloseDrawing(idx), Out::Btn(1)) => {
                    self.h.close_editor(idx);
                    if self.h.borrow().editors.is_empty() {
                        self.ui.showing_start = true;
                    }
                }
                _ => {}
            },
            Kind::SaveAs { name } => {
                if ok {
                    let mut n = name.trim().to_string();
                    let l = n.to_lowercase();
                    if !(l.ends_with(".json") || l.ends_with(".dxf") || l.ends_with(".cadforge")) {
                        n.push_str(cadforge_core::io::FILE_EXT);
                    }
                    if !n.is_empty() {
                        self.write_doc(&n);
                    }
                }
            }
            Kind::Purge(Some(s)) => {
                if ok {
                    s.apply(self);
                }
            }
            Kind::LayerStates(s) => {
                if ok {
                    s.restore(self);
                }
            }
            Kind::QSelect(Some(s)) => {
                if ok {
                    s.apply(self);
                }
            }
            Kind::TextStyle(Some(s)) => {
                if ok {
                    s.apply(self);
                }
            }
            Kind::Units(Some(s)) => {
                if ok {
                    s.apply(self);
                }
            }
            Kind::Options(Some(s)) => {
                if ok {
                    s.apply(self);
                }
            }
            Kind::DSettings(s) => {
                if ok {
                    s.apply(self);
                }
                if from_cmd {
                    reply = Some(DialogReply::Done);
                }
            }
            Kind::DimStyleEdit(s) => {
                if ok {
                    s.apply(self);
                }
            }
            Kind::Plot(s) => {
                if ok {
                    s.run(self);
                }
            }
            Kind::Color(s) => {
                let value = s.value;
                match (&s.target, ok) {
                    (ColorTarget::Command, true) => reply = Some(DialogReply::Color(value)),
                    (ColorTarget::Command, false) => reply = Some(DialogReply::Cancelled),
                    (ColorTarget::Property, true) => self.set_property("color", value, None, None),
                    (ColorTarget::Hatch, true) => {
                        if let Some(sess) = self.h.borrow().hatch_session.as_ref() {
                            sess.settings.borrow_mut().color = value;
                        }
                        self.refresh_hatch_preview();
                    }
                    (ColorTarget::Field(tok), true) => self.ret_color = Some((*tok, value)),
                    (ColorTarget::Layer(name), true) => {
                        let (n, c) = (name.clone(), value);
                        self.edit_doc("Layer color", move |d| {
                            d.will_change();
                            if let (Some(l), Some(c)) = (d.layer_mut(&n), c) {
                                l.color = c;
                            }
                            d.touch();
                        });
                    }
                    _ => {}
                }
            }
            Kind::Linetype(s) => {
                if ok {
                    if let Some(sel) = s.sel.clone() {
                        if s.pick {
                            self.ret_text = Some(("linetype".into(), sel));
                        } else {
                            self.set_property("ltype", None, None, if sel == "BYLAYER" { None } else { Some(sel) });
                        }
                    }
                }
                if from_cmd {
                    reply = Some(if ok { DialogReply::Done } else { DialogReply::Cancelled });
                }
            }
            Kind::Lweight(s) => {
                if ok {
                    self.set_sv("LWDISPLAY", if s.display { 1.0 } else { 0.0 });
                    if s.pick {
                        self.ret_lw = Some(s.sel);
                    } else {
                        self.set_property("lw", None, s.sel, None);
                    }
                }
                if from_cmd {
                    reply = Some(if ok { DialogReply::Done } else { DialogReply::Cancelled });
                }
            }
            Kind::Insert(s) => {
                reply = Some(if ok { s.sel.map_or(DialogReply::Cancelled, DialogReply::Text) } else { DialogReply::Cancelled });
            }
            Kind::Hatch(s) => {
                if ok {
                    s.apply(self);
                }
                if from_cmd {
                    reply = Some(DialogReply::Done);
                }
            }
            Kind::EditText(s) => {
                reply = Some(if ok { DialogReply::Text(s.text) } else { DialogReply::Cancelled });
            }
            Kind::FileBrowser(s) => {
                if ok {
                    self.finish_file_browser(s);
                }
            }
            _ => {}
        }
        if from_cmd {
            if let Some(r) = reply {
                self.h.answer_dialog(r);
            }
        }
    }

    // ------------------------------------------------------------ shared form widgets

    pub fn btn(&self, ui: &mut egui::Ui, label: &str, primary: bool) -> egui::Response {
        let t = self.t;
        let text = RichText::new(label).color(if primary { Color32::WHITE } else { t.text });
        let b = egui::Button::new(text).min_size(vec2(78.0, 24.0)).fill(if primary { t.accent } else { t.raised }).stroke(Stroke::new(1.0, if primary { t.accent2 } else { t.sep }));
        ui.add(b)
    }

    /// OK / Cancel (and extra) button row.
    pub fn ok_cancel(&self, ui: &mut egui::Ui, ok: Option<&str>, cancel: Option<&str>, extra: &[&str]) -> Out {
        let mut out = Out::Open;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (i, e) in extra.iter().enumerate() {
                if self.btn(ui, e, false).clicked() {
                    out = Out::Btn(i);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(c) = cancel {
                    if self.btn(ui, c, false).clicked() {
                        out = Out::Cancel;
                    }
                }
                if let Some(o) = ok {
                    if self.btn(ui, o, true).clicked() {
                        out = Out::Ok;
                    }
                }
            });
        });
        out
    }

    pub fn note(&self, ui: &mut egui::Ui, text: &str) {
        ui.label(RichText::new(text).small().color(self.t.text3));
    }

    pub fn group<R>(&self, ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let t = self.t;
        egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(3)).inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
            ui.label(RichText::new(title).color(t.text2).small());
            add(ui)
        }).inner
    }

    pub fn tab_strip(&self, ui: &mut egui::Ui, tabs: &[&str], cur: &mut usize) {
        let t = self.t;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, name) in tabs.iter().enumerate() {
                let active = *cur == i;
                let b = egui::Button::new(RichText::new(*name).color(if active { t.text } else { t.text2 })).fill(if active { t.raised } else { Color32::TRANSPARENT }).stroke(Stroke::new(1.0, if active { t.sep } else { Color32::TRANSPARENT })).min_size(vec2(0.0, 24.0));
                if ui.add(b).clicked() {
                    *cur = i;
                }
            }
        });
        ui.separator();
    }

    pub fn num_edit(&self, ui: &mut egui::Ui, text: &mut String, width: f32) -> egui::Response {
        ui.add(egui::TextEdit::singleline(text).desired_width(width).font(FontId::monospace(12.0)))
    }

    pub fn color_chip(&self, ui: &mut egui::Ui, c: Option<Color>) {
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
        crate::ribbon::swatch(ui.painter(), r, crate::ribbon::color_rgb_of(c, &None, self.t.dark), &self.t);
    }
}

fn dialog_title_width(k: &Kind) -> (String, f32) {
    match k {
        Kind::Confirm { title, .. } => (title.clone(), 380.0),
        Kind::SaveAs { .. } => ("Save Drawing As".into(), 440.0),
        Kind::About => ("About CadForge".into(), 420.0),
        Kind::Help(_) => ("Help".into(), 660.0),
        Kind::Calc(_) => ("QuickCalc".into(), 440.0),
        Kind::Purge(_) => ("Purge".into(), 380.0),
        Kind::LayerStates(_) => ("Layer States Manager".into(), 420.0),
        Kind::AliasEdit(_) => ("Edit Aliases".into(), 500.0),
        Kind::QSelect(_) => ("Quick Select".into(), 420.0),
        Kind::TextStyle(_) => ("Text Style".into(), 380.0),
        Kind::Units(_) => ("Drawing Units".into(), 420.0),
        Kind::Options(_) => ("Options".into(), 540.0),
        Kind::DSettings(_) => ("Drafting Settings".into(), 540.0),
        Kind::DimStyle(_) => ("Dimension Style Manager".into(), 500.0),
        Kind::DimStyleEdit(s) => (s.title.clone(), 460.0),
        Kind::Plot(_) => ("Plot — Model".into(), 780.0),
        Kind::Color(_) => ("Select Color".into(), 520.0),
        Kind::Linetype(_) => ("Linetype Manager".into(), 520.0),
        Kind::Lweight(_) => ("Lineweight Settings".into(), 380.0),
        Kind::Insert(_) => ("Insert".into(), 360.0),
        Kind::Hatch(_) => ("Hatch Settings".into(), 380.0),
        Kind::EditText(s) => (if s.multi { "Multiline Text Editor".into() } else { "Single Line Text".into() }, 460.0),
        Kind::FileBrowser(s) => (s.title.clone(), 560.0),
    }
}

// constructors used by the rest of the application ---------------------------------------------------------------

impl Dialog {
    pub fn dsettings(tab: &str) -> Dialog {
        Dialog::new(Kind::DSettings(DsState::new(tab)))
    }
    pub fn options() -> Dialog {
        Dialog::new(Kind::Options(None))
    }
    pub fn units() -> Dialog {
        Dialog::new(Kind::Units(None))
    }
    pub fn dimstyle() -> Dialog {
        Dialog::new(Kind::DimStyle(None))
    }
    pub fn plot(format: &str) -> Dialog {
        Dialog::new(Kind::Plot(PlotState::new(format)))
    }
    pub fn qselect() -> Dialog {
        Dialog::new(Kind::QSelect(None))
    }
    pub fn aliasedit() -> Dialog {
        Dialog::new(Kind::AliasEdit(AliasState::default()))
    }
    pub fn textstyle() -> Dialog {
        Dialog::new(Kind::TextStyle(None))
    }
    pub fn layerstate() -> Dialog {
        Dialog::new(Kind::LayerStates(LayerStatesState::default()))
    }
    pub fn help() -> Dialog {
        Dialog::new(Kind::Help(HelpState::default()))
    }
    pub fn calc() -> Dialog {
        Dialog::new(Kind::Calc(CalcState::default()))
    }
    pub fn purge() -> Dialog {
        Dialog::new(Kind::Purge(None))
    }
    pub fn color_for_property() -> Dialog {
        Dialog::new(Kind::Color(ColorState::new(None, true, ColorTarget::Property)))
    }
    pub fn color_for_hatch() -> Dialog {
        Dialog::new(Kind::Color(ColorState::new(None, true, ColorTarget::Hatch)))
    }
    pub fn color_for(value: Option<Color>, allow_by_layer: bool, target: ColorTarget) -> Dialog {
        Dialog::new(Kind::Color(ColorState::new(value, allow_by_layer, target)))
    }
    pub fn linetype_pick(value: Option<String>) -> Dialog {
        Dialog::new(Kind::Linetype(LinetypeState::new(value, true)))
    }
    pub fn lweight_pick(value: Option<i32>) -> Dialog {
        Dialog::new(Kind::Lweight(LweightState::new(value, true)))
    }
    pub fn file_open(purpose: &'static str) -> Dialog {
        Dialog::new(Kind::FileBrowser(FileBrowserState::open(purpose)))
    }
    pub fn edit_text(spec: TextEditSpec) -> Dialog {
        Dialog::new(Kind::EditText(TextEditState::new(spec)))
    }
}
